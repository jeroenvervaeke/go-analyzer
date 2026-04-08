// NOTE: add_method and modify_method edit actions are deferred per spec.
// They require constructing AST nodes from JSON input, which needs careful
// schema design. Add these as a follow-up task once the core edit actions are working.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use go_model::{Block, Ident, TopLevelDecl, TypeExpr};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::output::EditResult;
use crate::selection_builder::{Filter, SelectKind};
use crate::state::ServerState;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct EditInput {
    pub select: SelectKind,
    #[serde(default)]
    pub filters: Vec<Filter>,
    pub action: EditAction,
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EditAction {
    Delete,
    Rename(String),
    ReplaceBody(String),
    AddField { name: String, ty: String },
    RemoveField(String),
}

#[derive(Debug, thiserror::Error)]
pub enum EditError {
    #[error("state error: {0}")]
    State(String),
    #[error("no items matched (select: {select}, filters: {filters})")]
    EmptySelection { select: String, filters: String },
    #[error("invalid action for this selection: {0}")]
    InvalidAction(String),
    #[error("parse failed: {0}")]
    ParseFailed(String),
    #[error("failed to write changes: {0}")]
    CommitFailed(String),
}

pub fn handle_edit(state: &mut ServerState, input: &EditInput) -> Result<EditResult, EditError> {
    let repo = state.repo().map_err(|e| EditError::State(e.to_string()))?;
    let changes = build_changes(repo, &input.select, &input.filters, &input.action)?;

    if changes.is_empty() {
        return Err(EditError::EmptySelection {
            select: format!("{:?}", input.select),
            filters: format!("{:?}", input.filters),
        });
    }

    let applied = repo.apply(changes);
    let modified = applied.dry_run();
    let diff = build_diff(&modified);
    let files_modified: Vec<PathBuf> = applied
        .affected_files()
        .iter()
        .map(|p| p.to_path_buf())
        .collect();
    let edits_applied = applied.edit_count();

    if !input.dry_run {
        applied
            .commit()
            .map_err(|e| EditError::CommitFailed(e.to_string()))?;
        state
            .reload()
            .map_err(|e| EditError::State(e.to_string()))?;
    }

    Ok(EditResult {
        diff,
        files_modified,
        edits_applied,
    })
}

fn build_changes(
    repo: &go_analyzer::Repo,
    select: &SelectKind,
    filters: &[Filter],
    action: &EditAction,
) -> Result<go_analyzer::Changes, EditError> {
    match select {
        SelectKind::Functions => build_function_changes(repo, filters, action),
        SelectKind::Methods => build_method_changes(repo, filters, action),
        SelectKind::Structs | SelectKind::Interfaces | SelectKind::Types => {
            build_type_changes(repo, select, filters, action)
        }
    }
}

fn build_function_changes(
    repo: &go_analyzer::Repo,
    filters: &[Filter],
    action: &EditAction,
) -> Result<go_analyzer::Changes, EditError> {
    let mut sel = repo.functions();

    for f in filters {
        match f {
            Filter::Named(name) => sel = sel.named(name),
            Filter::InPackage(pkg) => sel = sel.in_package(pkg),
            Filter::Exported(true) => sel = sel.exported(),
            Filter::Exported(false) => sel = sel.unexported(),
            Filter::ExcludingTests(true) => sel = sel.excluding_tests(),
            Filter::ExcludingTests(false) | Filter::OnType(_) | Filter::Implementing(_) => {}
        }
    }

    match action {
        EditAction::Delete => Ok(sel.delete()),
        EditAction::Rename(new_name) => Ok(sel.rename(new_name)),
        EditAction::ReplaceBody(body_src) => {
            let block = parse_go_block(body_src)?;
            Ok(sel.replace_body(block))
        }
        EditAction::AddField { .. } => Err(EditError::InvalidAction(
            "add_field is not valid for functions".to_owned(),
        )),
        EditAction::RemoveField(_) => Err(EditError::InvalidAction(
            "remove_field is not valid for functions".to_owned(),
        )),
    }
}

fn build_method_changes(
    repo: &go_analyzer::Repo,
    filters: &[Filter],
    action: &EditAction,
) -> Result<go_analyzer::Changes, EditError> {
    let mut sel = repo.methods();

    for f in filters {
        match f {
            Filter::Named(name) => sel = sel.named(name),
            Filter::InPackage(pkg) => sel = sel.in_package(pkg),
            Filter::Exported(true) => sel = sel.exported(),
            Filter::Exported(false) => sel = sel.unexported(),
            Filter::ExcludingTests(true) => sel = sel.excluding_tests(),
            Filter::OnType(type_name) => sel = sel.on_type(type_name),
            Filter::ExcludingTests(false) | Filter::Implementing(_) => {}
        }
    }

    match action {
        EditAction::Delete => Ok(sel.delete()),
        EditAction::Rename(new_name) => Ok(sel.rename(new_name)),
        EditAction::ReplaceBody(body_src) => {
            let block = parse_go_block(body_src)?;
            Ok(sel.replace_body(block))
        }
        EditAction::AddField { .. } => Err(EditError::InvalidAction(
            "add_field is not valid for methods".to_owned(),
        )),
        EditAction::RemoveField(_) => Err(EditError::InvalidAction(
            "remove_field is not valid for methods".to_owned(),
        )),
    }
}

fn build_type_changes(
    repo: &go_analyzer::Repo,
    select: &SelectKind,
    filters: &[Filter],
    action: &EditAction,
) -> Result<go_analyzer::Changes, EditError> {
    let mut sel = match select {
        SelectKind::Structs => repo.structs(),
        SelectKind::Interfaces => repo.interfaces(),
        SelectKind::Types => repo.types(),
        // Already matched in the caller; unreachable for Functions/Methods
        _ => unreachable!(),
    };

    for f in filters {
        match f {
            Filter::Named(name) => sel = sel.named(name),
            Filter::InPackage(pkg) => sel = sel.in_package(pkg),
            Filter::Exported(true) => sel = sel.exported(),
            Filter::Exported(false) => sel = sel.unexported(),
            Filter::ExcludingTests(true) => sel = sel.excluding_tests(),
            Filter::Implementing(iface) => sel = sel.implementing(iface),
            Filter::ExcludingTests(false) | Filter::OnType(_) => {}
        }
    }

    match action {
        EditAction::Delete => Ok(sel.delete()),
        EditAction::Rename(new_name) => Ok(sel.rename(new_name)),
        EditAction::ReplaceBody(_) => Err(EditError::InvalidAction(
            "replace_body is not valid for types".to_owned(),
        )),
        EditAction::AddField { name, ty } => {
            let type_expr = TypeExpr::Named(Ident::synthetic(ty));
            Ok(sel.add_field(name, type_expr))
        }
        EditAction::RemoveField(field_name) => Ok(sel.remove_field(field_name)),
    }
}

fn parse_go_block(body_src: &str) -> Result<Block, EditError> {
    let wrapped = format!("package p\nfunc _() {{\n{body_src}\n}}");
    let ast = go_analyzer::walker::parse_and_walk(wrapped.as_bytes())
        .map_err(|e| EditError::ParseFailed(format!("failed to parse body: {e}")))?;
    for decl in &ast.decls {
        if let TopLevelDecl::Func(f) = decl
            && let Some(body) = &f.body
        {
            return Ok(body.clone());
        }
    }
    Err(EditError::ParseFailed(
        "could not extract block".to_string(),
    ))
}

fn build_diff(modified: &HashMap<PathBuf, String>) -> String {
    let mut diff = String::new();
    for (path, new_content) in modified {
        if let Ok(original) = std::fs::read_to_string(path) {
            let file_diff = unified_diff(path, &original, new_content);
            if !file_diff.is_empty() {
                diff.push_str(&file_diff);
            }
        }
    }
    diff
}

fn unified_diff(path: &Path, old: &str, new: &str) -> String {
    use similar::{ChangeTag, TextDiff};

    let diff = TextDiff::from_lines(old, new);
    let mut output = String::new();

    let has_changes = diff.iter_all_changes().any(|c| c.tag() != ChangeTag::Equal);
    if !has_changes {
        return output;
    }

    output.push_str(&format!("--- a/{}\n", path.display()));
    output.push_str(&format!("+++ b/{}\n", path.display()));

    for hunk in diff.unified_diff().context_radius(3).iter_hunks() {
        output.push_str(&hunk.to_string());
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../go-analyzer/tests/fixture_repo")
            .canonicalize()
            .expect("fixture repo must exist")
    }

    fn fixture_state() -> ServerState {
        ServerState::new(fixture_path())
    }

    fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());
            if src_path.is_dir() {
                copy_dir_recursive(&src_path, &dst_path)?;
            } else {
                std::fs::copy(&src_path, &dst_path)?;
            }
        }
        Ok(())
    }

    #[test]
    fn test_edit_dry_run_returns_diff_without_writing() {
        let mut state = fixture_state();
        let input = EditInput {
            select: SelectKind::Functions,
            filters: vec![Filter::Named("helperFunc".to_owned())],
            action: EditAction::Rename("renamedHelper".to_owned()),
            dry_run: true,
        };

        let result = handle_edit(&mut state, &input).unwrap();

        assert!(
            result.diff.contains("renamedHelper"),
            "diff should contain the new name, got:\n{}",
            result.diff
        );
        assert!(result.edits_applied > 0);

        // Verify the original file was NOT modified
        let models_go = fixture_path().join("alpha/models.go");
        let content = std::fs::read_to_string(&models_go).unwrap();
        assert!(
            content.contains("helperFunc"),
            "original file should still contain helperFunc after dry_run"
        );
    }

    #[test]
    fn test_edit_empty_selection_returns_error() {
        let mut state = fixture_state();
        let input = EditInput {
            select: SelectKind::Functions,
            filters: vec![Filter::Named("nonexistent_function_xyz".to_owned())],
            action: EditAction::Delete,
            dry_run: true,
        };

        let result = handle_edit(&mut state, &input);

        assert!(result.is_err(), "expected error for empty selection");
        let err = result.unwrap_err();
        assert!(
            matches!(err, EditError::EmptySelection { .. }),
            "expected EmptySelection error, got: {err}"
        );
    }

    #[test]
    fn test_edit_write_modifies_file_and_reloads() {
        let tmp = tempfile::tempdir().expect("failed to create tempdir");
        let tmp_repo = tmp.path().to_path_buf();
        copy_dir_recursive(&fixture_path(), &tmp_repo).expect("failed to copy fixture");

        let mut state = ServerState::new(tmp_repo.clone());
        let input = EditInput {
            select: SelectKind::Functions,
            filters: vec![Filter::Named("helperFunc".to_owned())],
            action: EditAction::Rename("renamedHelper".to_owned()),
            dry_run: false,
        };

        let result = handle_edit(&mut state, &input).unwrap();

        assert!(result.edits_applied > 0);
        assert!(!result.files_modified.is_empty());

        // Verify the file on disk changed
        let models_go = tmp_repo.join("alpha/models.go");
        let content = std::fs::read_to_string(&models_go).unwrap();
        assert!(
            content.contains("renamedHelper"),
            "file should contain renamedHelper after commit"
        );
        assert!(
            !content.contains("helperFunc"),
            "file should no longer contain helperFunc after rename"
        );

        // Verify the state was reloaded and sees the renamed function
        let repo = state.repo().unwrap();
        let found = repo.functions().named("renamedHelper").count();
        assert_eq!(found, 1, "reloaded repo should find renamedHelper");

        let old = repo.functions().named("helperFunc").count();
        assert_eq!(old, 0, "reloaded repo should not find helperFunc");
    }
}
