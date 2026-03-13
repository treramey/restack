use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::db::{env_repo, rebuild_repo, topic_env_repo};
use crate::error::Result;
use crate::id::{EnvId, RepoId};
use crate::types::{Environment, Rebuild, Topic};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvStatus {
    pub env: Environment,
    pub topics: Vec<Topic>,
    pub last_rebuild: Option<Rebuild>,
}

pub fn add_env(
    conn: &Connection,
    repo_id: &RepoId,
    name: &str,
    branch: &str,
    ordinal: i32,
) -> Result<Environment> {
    env_repo::create_env(conn, repo_id, name, branch, ordinal)
}

pub fn list_envs(conn: &Connection, repo_id: Option<&RepoId>) -> Result<Vec<Environment>> {
    env_repo::list_envs(conn, repo_id)
}

pub fn get_env_status(conn: &Connection, env_id: &EnvId) -> Result<EnvStatus> {
    let env = env_repo::get_env(conn, env_id)?;
    let topics = topic_env_repo::get_topics_in_env(conn, env_id)?;
    let last_rebuild = rebuild_repo::get_last_rebuild(conn, env_id)?;

    Ok(EnvStatus {
        env,
        topics,
        last_rebuild,
    })
}
