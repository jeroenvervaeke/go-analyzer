use std::path::{Path, PathBuf};

use go_analyzer::Repo;

pub struct ServerState {
    repo: Option<Repo>,
    repo_path: PathBuf,
}

impl ServerState {
    pub fn new(repo_path: PathBuf) -> Self {
        Self {
            repo: None,
            repo_path,
        }
    }

    /// Return a reference to the loaded repo, loading it on first access.
    pub fn repo(&mut self) -> Result<&Repo, StateError> {
        if self.repo.is_none() {
            self.load()?;
        }
        // Safety: load() sets self.repo to Some, so this cannot be None here.
        Ok(self.repo.as_ref().unwrap())
    }

    /// Reload the repo from disk. Called after edits are applied.
    pub fn reload(&mut self) -> Result<(), StateError> {
        self.load()
    }

    pub fn repo_path(&self) -> &Path {
        &self.repo_path
    }

    fn load(&mut self) -> Result<(), StateError> {
        let repo = Repo::load(&self.repo_path).map_err(|e| StateError::LoadFailed {
            path: self.repo_path.clone(),
            message: e.to_string(),
        })?;
        self.repo = Some(repo);
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StateError {
    #[error("failed to load repo at {path}: {message}")]
    LoadFailed { path: PathBuf, message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lazy_load_on_first_access() {
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../go-analyzer/tests/fixture_repo");
        let mut state = ServerState::new(fixture);
        assert!(state.repo.is_none());
        let repo = state.repo().unwrap();
        assert!(repo.file_count() > 0);
    }

    #[test]
    fn test_load_nonexistent_path_returns_error() {
        let mut state = ServerState::new(PathBuf::from("/nonexistent/path"));
        assert!(state.repo().is_err());
    }

    #[test]
    fn test_reload_refreshes_repo() {
        let fixture =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../go-analyzer/tests/fixture_repo");
        let mut state = ServerState::new(fixture);
        let _ = state.repo().unwrap();
        state.reload().unwrap();
        assert!(state.repo.is_some());
    }
}
