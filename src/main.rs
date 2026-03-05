#![allow(dead_code)]

use std::io;
use std::path::{Path, PathBuf};

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};

mod commands;
mod config;
mod core;
mod db;
mod error;
mod git;
mod id;
mod output;
mod provider;
mod types;
mod version;

use commands::{
    conflicts::ConflictsCommand, env::EnvCommand, promote::PromoteCommand, rebuild::RebuildCommand,
    repo::RepoCommand, topic::TopicCommand,
};
use output::Printer;

#[derive(Parser)]
#[command(name = "restack")]
#[command(version)]
#[command(
    about = "Restack - Topic branch integration manager",
    long_about = r#"
Restack - Manage topic branches across integration environments.

Features:
  • Track topic branches (PRs) across dev/staging/production
  • Two-phase rebuild: staging topics merged first, then dev-only
  • Conflict detection with automatic topic removal
  • Environment promotion/demotion workflow

Environment:
  RESTACK_DB_PATH   Override database location
  NO_COLOR          Disable colored output
"#
)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Output in JSON format (for programmatic use)
    #[arg(long, global = true)]
    json: bool,

    /// Disable colored output
    #[arg(long, global = true)]
    no_color: bool,

    /// Override database path (default: .restack/workspace.db)
    #[arg(long, global = true)]
    db: Option<PathBuf>,

    /// Show what would happen without making changes
    #[arg(long, global = true)]
    dry_run: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Initialize a restack workspace in the current directory
    Init,

    /// Refresh: fetch origin, discover new branches, sync CI status, close deleted branches
    Refresh {
        /// Repo ID to refresh (defaults to all repos)
        #[arg(long)]
        repo: Option<String>,
    },

    /// Repository management
    #[command(subcommand)]
    Repo(RepoCommand),

    /// Topic branch tracking
    #[command(subcommand)]
    Topic(TopicCommand),

    /// Environment management
    #[command(subcommand)]
    Env(EnvCommand),

    /// Promote/demote topics between environments
    #[command(subcommand)]
    Promote(PromoteCommand),

    /// Rebuild integration branches
    #[command(subcommand)]
    Rebuild(RebuildCommand),

    /// List conflicts
    #[command(subcommand)]
    Conflicts(ConflictsCommand),

    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        shell: Shell,
    },

    /// Start the UI server
    Ui {
        /// HTTP port
        #[arg(long, short, default_value = "6969")]
        port: u16,
    },
}

fn default_db_path() -> PathBuf {
    // Check env override
    if let Ok(path) = std::env::var("RESTACK_DB_PATH") {
        return PathBuf::from(path);
    }

    // Try to find workspace root
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    match core::workspace::find_workspace_root(&cwd) {
        Ok(root) => core::workspace::resolve_db_path(&root),
        Err(_) => cwd.join(".restack").join("workspace.db"),
    }
}

fn main() {
    let cli = Cli::parse();

    // Completions bypass normal flow
    if let Command::Completions { shell } = &cli.command {
        generate(*shell, &mut Cli::command(), "restack", &mut io::stdout());
        return;
    }

    // UI: spawn Node.js host server
    if let Command::Ui { port } = &cli.command {
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("restack"));
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        // Resolve paths relative to the binary location
        let bin_dir = exe.parent().unwrap_or(Path::new("."));
        // In dev: target/debug/restack -> project root is ../../
        // In prod: host/dist/index.js should be alongside or discoverable
        let project_root = bin_dir
            .join("../../")
            .canonicalize()
            .unwrap_or_else(|_| cwd.clone());
        let host_entry = project_root.join("host/dist/index.js");
        let static_root = project_root.join("ui/dist");

        if !host_entry.exists() {
            eprintln!("Error: Host not built. Run 'npm run build' in the project root first.");
            eprintln!("  Expected: {}", host_entry.display());
            std::process::exit(1);
        }

        let status = std::process::Command::new("node")
            .arg(&host_entry)
            .arg("ui")
            .arg("--cli-path")
            .arg(&exe)
            .arg("--cwd")
            .arg(&cwd)
            .arg("--static-root")
            .arg(&static_root)
            .arg("--port")
            .arg(port.to_string())
            .status();

        match status {
            Ok(s) => std::process::exit(s.code().unwrap_or(1)),
            Err(e) => {
                eprintln!("Error: Failed to start UI server: {}", e);
                eprintln!("  Make sure Node.js is installed and 'npm run build' has been run.");
                std::process::exit(1);
            }
        }
    }

    let db_path = cli.db.unwrap_or_else(default_db_path);

    let result = run(&cli.command, &db_path);

    match result {
        Ok(output) => {
            if cli.json {
                println!("{}", output);
            } else {
                let printer = Printer::new(cli.no_color);
                printer.print_json(&output);
            }
        }
        Err(e) => {
            if cli.json {
                let err = serde_json::json!({ "error": e.to_string() });
                eprintln!("{}", err);
            } else {
                let printer = Printer::new_for_stderr(cli.no_color);
                printer.print_error(&format!("Error: {}", e));
            }
            std::process::exit(1);
        }
    }
}

fn run(command: &Command, db_path: &Path) -> error::Result<String> {
    match command {
        Command::Init => {
            let cwd = std::env::current_dir()?;
            commands::init::handle_init(&cwd)
        }
        Command::Refresh { repo } => {
            let conn = db::open_db(db_path)?;
            let cwd = std::env::current_dir()?;
            commands::refresh::handle_refresh(&conn, repo.as_deref(), &cwd)
        }
        Command::Repo(cmd) => {
            let conn = db::open_db(db_path)?;
            let cwd = std::env::current_dir()?;
            commands::repo::handle(&conn, cmd, &cwd)
        }
        Command::Topic(cmd) => {
            let conn = db::open_db(db_path)?;
            commands::topic::handle(&conn, cmd)
        }
        Command::Env(cmd) => {
            let conn = db::open_db(db_path)?;
            commands::env::handle(&conn, cmd)
        }
        Command::Promote(cmd) => {
            let conn = db::open_db(db_path)?;
            let cwd = std::env::current_dir()?;
            commands::promote::handle(&conn, cmd, &cwd)
        }
        Command::Rebuild(cmd) => {
            let conn = db::open_db(db_path)?;
            let cwd = std::env::current_dir()?;
            commands::rebuild::handle(&conn, cmd, &cwd)
        }
        Command::Conflicts(cmd) => {
            let conn = db::open_db(db_path)?;
            commands::conflicts::handle(&conn, cmd)
        }
        Command::Completions { .. } => unreachable!("completions handled before run()"),
        Command::Ui { .. } => unreachable!("ui handled before run()"),
    }
}
