use std::process;

use clap::{Parser, Subcommand};

use go_analyzer::{Repo, build};

#[derive(Parser)]
#[command(name = "go-analyzer", about = "Analyze and transform Go source code")]
struct Cli {
    /// Path to the Go repository root
    #[arg(long, default_value = ".", global = true)]
    path: String,

    /// Preview changes without writing to disk
    #[arg(long, global = true)]
    dry_run: bool,

    /// Filter to a specific package
    #[arg(long, global = true)]
    package: Option<String>,

    /// Only include exported items
    #[arg(long, global = true)]
    exported: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List all structs in the repo
    Structs,
    /// List all functions in the repo
    Functions,
    /// List all methods in the repo
    Methods,
    /// Add String() method to all exported structs that don't have one
    AddStringMethod,
    /// Delete methods with the given name
    DeleteMethod {
        /// Name of the method to delete
        name: String,
    },
}

fn main() {
    let cli = Cli::parse();

    let repo = match Repo::load(&cli.path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: failed to load repo at '{}': {e}", cli.path);
            process::exit(1);
        }
    };

    match cli.command {
        Command::Structs => cmd_structs(&repo, &cli),
        Command::Functions => cmd_functions(&repo, &cli),
        Command::Methods => cmd_methods(&repo, &cli),
        Command::AddStringMethod => cmd_add_string_method(&repo, &cli),
        Command::DeleteMethod { ref name } => cmd_delete_method(&repo, &cli, name),
    }
}

fn cmd_structs(repo: &Repo, cli: &Cli) {
    let mut sel = repo.structs();
    if let Some(pkg) = &cli.package {
        sel = sel.in_package(pkg);
    }
    if cli.exported {
        sel = sel.exported();
    }
    for si in sel.collect() {
        println!("{}", si.item.name().name);
    }
}

fn cmd_functions(repo: &Repo, cli: &Cli) {
    let mut sel = repo.functions();
    if let Some(pkg) = &cli.package {
        sel = sel.in_package(pkg);
    }
    if cli.exported {
        sel = sel.exported();
    }
    for si in sel.collect() {
        println!("{}", si.item.name.name);
    }
}

fn cmd_methods(repo: &Repo, cli: &Cli) {
    let mut sel = repo.methods();
    if let Some(pkg) = &cli.package {
        sel = sel.in_package(pkg);
    }
    if cli.exported {
        sel = sel.exported();
    }
    for si in sel.collect() {
        let receiver_str = format_receiver_type(&si.item.receiver.ty);
        println!("({receiver_str}).{}", si.item.name.name);
    }
}

fn format_receiver_type(ty: &go_analyzer::go_model::TypeExpr) -> String {
    use go_analyzer::go_model::TypeExpr;
    match ty {
        TypeExpr::Named(id) => id.name.clone(),
        TypeExpr::Pointer(inner) => format!("*{}", format_receiver_type(inner)),
        TypeExpr::Generic { base, args } => {
            let args_str: Vec<_> = args.iter().map(format_receiver_type).collect();
            format!("{}[{}]", format_receiver_type(base), args_str.join(", "))
        }
        _ => format!("{ty:?}"),
    }
}

fn cmd_add_string_method(repo: &Repo, cli: &Cli) {
    let mut sel = repo.structs().exported();
    if let Some(pkg) = &cli.package {
        sel = sel.in_package(pkg);
    }

    let changes = sel.method("String").absent().or_add(|ts| {
        let type_name = &ts.name().name;
        build::method(
            build::pointer_receiver("x", type_name),
            "String",
            vec![],
            vec![build::unnamed_param(build::named("string"))],
            build::block(vec![build::ret(vec![build::call(
                build::selector(build::ident("fmt"), "Sprintf"),
                vec![build::string("%+v"), build::deref(build::ident("x"))],
            )])]),
        )
    });

    apply_changes(repo, changes, cli);
}

fn cmd_delete_method(repo: &Repo, cli: &Cli, name: &str) {
    let mut sel = repo.methods().named(name);
    if let Some(pkg) = &cli.package {
        sel = sel.in_package(pkg);
    }
    if cli.exported {
        sel = sel.exported();
    }

    let changes = sel.delete();
    apply_changes(repo, changes, cli);
}

fn apply_changes(repo: &Repo, changes: go_analyzer::Changes, cli: &Cli) {
    if changes.is_empty() {
        println!("No changes to apply.");
        return;
    }

    let applied = repo.apply(changes);
    let edit_count = applied.edit_count();

    if cli.dry_run {
        applied.preview();
        println!("(dry run) {edit_count} edit(s) would be applied.");
    } else {
        match applied.commit() {
            Ok(summary) => {
                println!(
                    "Applied {} edit(s) across {} file(s).",
                    summary.edits_applied, summary.files_modified,
                );
            }
            Err(e) => {
                eprintln!("error: failed to commit changes: {e}");
                process::exit(1);
            }
        }
    }
}
