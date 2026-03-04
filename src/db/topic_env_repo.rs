use chrono::Utc;
use rusqlite::Connection;

use crate::error::Result;
use crate::id::{EnvId, TopicId};
use crate::types::{Topic, TopicEnvironment};

pub fn add_topic_to_env(
    conn: &Connection,
    topic_id: &TopicId,
    env_id: &EnvId,
) -> Result<TopicEnvironment> {
    let now = Utc::now();

    conn.execute(
        "INSERT OR IGNORE INTO topic_environments (topic_id, env_id, added_at) VALUES (?1, ?2, ?3)",
        rusqlite::params![topic_id, env_id, now.to_rfc3339()],
    )?;

    Ok(TopicEnvironment {
        topic_id: topic_id.clone(),
        env_id: env_id.clone(),
        added_at: now,
    })
}

pub fn remove_topic_from_env(
    conn: &Connection,
    topic_id: &TopicId,
    env_id: &EnvId,
) -> Result<()> {
    conn.execute(
        "DELETE FROM topic_environments WHERE topic_id = ?1 AND env_id = ?2",
        rusqlite::params![topic_id, env_id],
    )?;
    Ok(())
}

pub fn remove_topic_from_all_envs(conn: &Connection, topic_id: &TopicId) -> Result<()> {
    conn.execute(
        "DELETE FROM topic_environments WHERE topic_id = ?1",
        [topic_id],
    )?;
    Ok(())
}

pub fn get_topics_in_env(conn: &Connection, env_id: &EnvId) -> Result<Vec<Topic>> {
    let mut stmt = conn.prepare(
        r#"SELECT t.id, t.repo_id, t.branch, t.pr_id, t.pr_url, t.status, t.ci_status, t.created_at
           FROM topics t
           JOIN topic_environments te ON t.id = te.topic_id
           WHERE te.env_id = ?1
           ORDER BY te.added_at"#,
    )?;

    let rows = stmt.query_map([env_id], |row| {
        Ok(TopicRow {
            id: row.get(0)?,
            repo_id: row.get(1)?,
            branch: row.get(2)?,
            pr_id: row.get(3)?,
            pr_url: row.get(4)?,
            status: row.get::<_, String>(5)?,
            ci_status: row.get::<_, Option<String>>(6)?,
            created_at: row.get::<_, String>(7)?,
        })
    })?;

    let mut topics = Vec::new();
    for row in rows {
        topics.push(row?.into_topic());
    }
    Ok(topics)
}

pub fn get_envs_for_topic(conn: &Connection, topic_id: &TopicId) -> Result<Vec<EnvId>> {
    let mut stmt = conn.prepare(
        "SELECT env_id FROM topic_environments WHERE topic_id = ?1",
    )?;

    let rows = stmt.query_map([topic_id], |row| row.get::<_, EnvId>(0))?;

    let mut env_ids = Vec::new();
    for row in rows {
        env_ids.push(row?);
    }
    Ok(env_ids)
}

struct TopicRow {
    id: crate::id::TopicId,
    repo_id: crate::id::RepoId,
    branch: String,
    pr_id: Option<String>,
    pr_url: Option<String>,
    status: String,
    ci_status: Option<String>,
    created_at: String,
}

impl TopicRow {
    fn into_topic(self) -> Topic {
        use crate::types::{CiStatus, TopicStatus};

        let status = match self.status.as_str() {
            "active" => TopicStatus::Active,
            "conflict" => TopicStatus::Conflict,
            "graduated" => TopicStatus::Graduated,
            "closed" => TopicStatus::Closed,
            _ => TopicStatus::Active,
        };
        let ci_status = self.ci_status.as_deref().map(|s| match s {
            "passed" => CiStatus::Passed,
            "failed" => CiStatus::Failed,
            _ => CiStatus::Pending,
        });
        let created_at = chrono::DateTime::parse_from_rfc3339(&self.created_at)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        Topic {
            id: self.id,
            repo_id: self.repo_id,
            branch: self.branch,
            pr_id: self.pr_id,
            pr_url: self.pr_url,
            status,
            ci_status,
            ci_url: None,
            last_ci_check: None,
            created_at,
        }
    }
}
