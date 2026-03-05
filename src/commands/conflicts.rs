use clap::Subcommand;
use rusqlite::Connection;

use crate::error::Result;

#[derive(Subcommand)]
pub enum ConflictsCommand {
    /// List all conflicts
    List,
}

pub fn handle(conn: &Connection, cmd: &ConflictsCommand) -> Result<String> {
    match cmd {
        ConflictsCommand::List => {
            let conflicts = crate::db::conflict_repo::list_all_conflicts(conn)?;
            Ok(serde_json::to_string_pretty(&conflicts)?)
        }
    }
}
