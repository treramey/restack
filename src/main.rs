#![allow(dead_code)]

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};

mod commands;
mod config;
mod core;
mod db;
mod error;
mod format;
mod git;
mod id;
mod output;
mod provider;
mod types;
mod version;

use commands::{
    // conflicts::ConflictsCommand,
    integration::IntegrationCommand,
    // pr::ConflictsCommand,
    topic::TopicCommand,
};
use output::Printer;

#[derive(Parser)]
#[command(name = "restack")]
#[command(version)]
#[command(disable_help_subcommand = true)]
#[command(
    about = "Restack - Topic branch integration manager",
    long_about = r#"
Restack - Manage topic branches across integration branches.

Features:
  • Track topic branches (PRs) across dev/staging/production
  • Two-phase rebuild: staging topics merged first, then dev-only
  • Conflict detection with automatic topic removal
  • Integration branch promotion/demotion workflow

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

    /// Skip auto-reconciliation with .restack.yml
    #[arg(long, global = true)]
    no_reconcile: bool,
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

    /// List all repositories
    List,

    /// Add a repository to the workspace
    Add {
        /// Path to the repository
        #[arg(required_unless_present = "all")]
        path: Option<String>,
        /// Optional name for the repository (defaults to directory name)
        #[arg(short, long)]
        name: Option<String>,
        /// Optional repo ID override
        #[arg(short, long)]
        id: Option<String>,
        /// Add all repositories found in the workspace
        #[arg(long)]
        all: bool,
    },

    /// Remove a repository from the workspace
    Remove {
        /// Repo ID or name
        id: String,
    },

    /// Topic branch tracking
    #[command(subcommand)]
    Topic(TopicCommand),

    /// Integration branch management
    #[command(subcommand, name = "integration")]
    Integration(IntegrationCommand),

    // /// List conflicts
    // #[command(subcommand)]
    // Conflicts(ConflictsCommand),

    // /// Pull request management
    // #[command(subcommand)]
    // Pr(PrCommand),

    // /// Generate shell completions
    // Completions {
    //     /// Shell to generate completions for
    //     shell: Shell,
    // },
    /// Start the UI server
    Ui {
        /// HTTP port
        #[arg(long, short, default_value = "6969")]
        port: u16,
    },

    /// Internal: persistent NDJSON server over stdin/stdout (used by UI host)
    #[command(hide = true)]
    Serve,
}

#[derive(serde::Deserialize)]
struct ServeRequest {
    id: String,
    args: Vec<String>,
}

/// Every response sets exactly one of `result` or `error`.
#[derive(serde::Serialize)]
struct ServeResponse {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
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
    // if let Command::Completions { shell } = &cli.command {
    //     generate(*shell, &mut Cli::command(), "restack", &mut io::stdout());
    //     return;
    // }

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

        // Try to detect current repo from cwd
        let context_repo_name = {
            let db_path_for_context = core::workspace::find_workspace_root(&cwd)
                .map(|root| core::workspace::resolve_db_path(&root))
                .unwrap_or_else(|_| cwd.join(".restack").join("workspace.db"));
            if let Ok(conn) = db::open_db(&db_path_for_context) {
                core::repo_service::find_repo_from_cwd(&conn, &cwd)
                    .ok()
                    .flatten()
                    .map(|r| r.name)
            } else {
                None
            }
        };

        let mut cmd = std::process::Command::new("node");
        cmd.arg(&host_entry)
            .arg("ui")
            .arg("--cli-path")
            .arg(&exe)
            .arg("--cwd")
            .arg(&cwd)
            .arg("--static-root")
            .arg(&static_root)
            .arg("--port")
            .arg(port.to_string());

        if let Some(ref name) = context_repo_name {
            cmd.arg("--context-repo").arg(name);
        }

        let status = cmd.status();

        match status {
            Ok(s) => std::process::exit(s.code().unwrap_or(1)),
            Err(e) => {
                eprintln!("Error: Failed to start UI server: {}", e);
                eprintln!("  Make sure Node.js is installed and 'npm run build' has been run.");
                std::process::exit(1);
            }
        }
    }

    // Serve: persistent stdin/stdout JSON-RPC loop
    if let Command::Serve = &cli.command {
        let db_path = cli.db.unwrap_or_else(default_db_path);
        let conn = match db::open_db(&db_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("restack serve: failed to open DB: {}", e);
                std::process::exit(1);
            }
        };

        let stdin = std::io::stdin();
        let reader = std::io::BufReader::new(stdin.lock());
        let stdout = std::io::stdout();
        let mut writer = stdout.lock();

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };
            if line.trim().is_empty() {
                continue;
            }

            let req: ServeRequest = match serde_json::from_str(&line) {
                Ok(r) => r,
                Err(e) => {
                    let resp = ServeResponse {
                        id: "unknown".into(),
                        result: None,
                        error: Some(e.to_string()),
                    };
                    if let Err(e) = serde_json::to_writer(&mut writer, &resp) {
                        if e.io_error_kind() == Some(std::io::ErrorKind::BrokenPipe) {
                            break;
                        }
                        eprintln!("restack serve: write error: {}", e);
                    }
                    if let Err(e) = writeln!(writer) {
                        if e.kind() == std::io::ErrorKind::BrokenPipe {
                            break;
                        }
                        eprintln!("restack serve: write error: {}", e);
                    }
                    if let Err(e) = writer.flush() {
                        if e.kind() == std::io::ErrorKind::BrokenPipe {
                            break;
                        }
                        eprintln!("restack serve: write error: {}", e);
                    }
                    continue;
                }
            };

            let mut full_args = vec!["restack".to_string()];
            full_args.extend(req.args.clone());
            // Only append --json if not already present.
            if !req.args.iter().any(|a| a == "--json") {
                full_args.push("--json".to_string());
            }

            // NOTE: Commands that call std::env::current_dir() (Init, Refresh, Add, Topic, Integration)
            // resolve relative to the serve process's cwd, not the client's.
            // A future protocol extension could add an optional `cwd` field to ServeRequest.
            let resp = match Cli::try_parse_from(&full_args) {
                Ok(parsed_cli) => {
                    match &parsed_cli.command {
                        Command::Serve | Command::Ui { .. } => ServeResponse {
                            id: req.id,
                            result: None,
                            error: Some("command not available via serve".into()),
                        },
                        _ => {
                            let no_reconcile = parsed_cli.no_reconcile;
                            match run_with_conn(&parsed_cli.command, &conn, no_reconcile) {
                                Ok(json_str) => {
                                    let value: serde_json::Value =
                                        serde_json::from_str(&json_str).unwrap_or(serde_json::Value::Null);
                                    ServeResponse {
                                        id: req.id,
                                        result: Some(value),
                                        error: None,
                                    }
                                }
                                Err(e) => ServeResponse {
                                    id: req.id,
                                    result: None,
                                    error: Some(e.to_string()),
                                },
                            }
                        }
                    }
                }
                Err(e) => ServeResponse {
                    id: req.id,
                    result: None,
                    error: Some(e.to_string()),
                },
            };

            if let Err(e) = serde_json::to_writer(&mut writer, &resp) {
                if e.io_error_kind() == Some(std::io::ErrorKind::BrokenPipe) {
                    break;
                }
                eprintln!("restack serve: write error: {}", e);
            }
            if let Err(e) = writeln!(writer) {
                if e.kind() == std::io::ErrorKind::BrokenPipe {
                    break;
                }
                eprintln!("restack serve: write error: {}", e);
            }
            if let Err(e) = writer.flush() {
                if e.kind() == std::io::ErrorKind::BrokenPipe {
                    break;
                }
                eprintln!("restack serve: write error: {}", e);
            }
        }

        return;
    }

    let db_path = cli.db.unwrap_or_else(default_db_path);

    let result = run(&cli.command, &db_path, cli.no_reconcile);

    match result {
        Ok(output) => {
            if cli.json {
                println!("{}", output);
            } else {
                let printer = Printer::new(cli.no_color);
                let formatted = format::format_human(&cli.command, &output, &printer);
                println!("{}", formatted);
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

fn run(command: &Command, db_path: &Path, no_reconcile: bool) -> error::Result<String> {
    let conn = db::open_db(db_path)?;
    run_with_conn(command, &conn, no_reconcile)
}

fn run_with_conn(
    command: &Command,
    conn: &rusqlite::Connection,
    no_reconcile: bool,
) -> error::Result<String> {
    match command {
        Command::Init => {
            let cwd = std::env::current_dir()?;
            commands::init::handle_init(&cwd)
        }
        Command::Refresh { repo } => {
            let cwd = std::env::current_dir()?;
            commands::refresh::handle_refresh(conn, repo.as_deref(), &cwd)
        }
        Command::List => {
            let repos = core::repo_service::list_repos(conn)?;
            Ok(serde_json::to_string_pretty(&repos)?)
        }
        Command::Add {
            path,
            name,
            id: _,
            all,
        } => {
            let cwd = std::env::current_dir()?;
            let workspace_root = core::workspace::find_workspace_root(&cwd)?;

            if *all {
                let result = core::repo_service::detect_repos(conn, &workspace_root)?;
                return Ok(serde_json::to_string_pretty(&result)?);
            }

            let path = path.as_deref().expect("path required unless --all");
            let result =
                core::repo_service::add_repo(conn, &workspace_root, path, name.as_deref(), true)?;
            Ok(serde_json::to_string_pretty(&result)?)
        }
        Command::Remove { id } => {
            core::repo_service::remove_repo(conn, id)?;
            Ok(serde_json::json!({ "deleted": true }).to_string())
        }
        Command::Topic(cmd) => {
            let cwd = std::env::current_dir()?;
            commands::topic::handle(conn, cmd, &cwd, no_reconcile)
        }
        Command::Integration(cmd) => {
            let cwd = std::env::current_dir()?;
            commands::integration::handle(conn, cmd, &cwd, no_reconcile)
        }
        // Command::Conflicts(cmd) => commands::conflicts::handle(conn, cmd),
        // Command::Pr(cmd) => commands::pr::handle(conn, cmd),
        // Command::Completions { .. } => unreachable!("completions handled before run()"),
        Command::Ui { .. } => unreachable!("ui handled before run()"),
        Command::Serve => unreachable!("serve handled before run()"),
    }
}

