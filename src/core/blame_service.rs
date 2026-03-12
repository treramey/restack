use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::db::{env_repo, rebuild_repo, rebuild_topic_repo, topic_repo};
use crate::error::{RestackError, Result};
use crate::id::{EnvId, RebuildId, TopicId};
use crate::types::Rebuild;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlameResult {
    pub env_id: EnvId,
    pub env_name: String,
    pub red_rebuild_id: RebuildId,
    pub green_rebuild_id: Option<RebuildId>,
    pub suspects: Vec<BlameSuspect>,
    pub confidence: BlameConfidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlameSuspect {
    pub topic_id: TopicId,
    pub branch: String,
    pub phase: i32,
    pub merge_order: i32,
    pub is_new_since_green: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BlameConfidence {
    High,
    Medium,
    Low,
}

pub fn blame_env(conn: &Connection, env_id: &EnvId) -> Result<BlameResult> {
    let env = env_repo::get_env(conn, env_id)?;

    let red_rebuild = rebuild_repo::get_last_rebuild(conn, env_id)?.ok_or_else(|| {
        RestackError::RebuildFailed {
            env: env.name.clone(),
            reason: "no rebuilds found".to_string(),
        }
    })?;

    let green_rebuild = find_last_green_rebuild(conn, env_id)?;

    let red_topics = rebuild_topic_repo::get_topics_for_rebuild(conn, &red_rebuild.id)?;

    let (suspects, confidence, green_rebuild_id) = if let Some(ref green) = green_rebuild {
        let green_topics = rebuild_topic_repo::get_topics_for_rebuild(conn, &green.id)?;
        let green_ids: std::collections::HashSet<&TopicId> =
            green_topics.iter().map(|(id, _, _)| id).collect();

        let new_topics: Vec<&(TopicId, i32, i32)> = red_topics
            .iter()
            .filter(|(id, _, _)| !green_ids.contains(id))
            .collect();

        let (confidence, suspect_entries, is_new) = if new_topics.is_empty() {
            let last: Vec<&(TopicId, i32, i32)> = red_topics
                .iter()
                .max_by_key(|(_, _, mo)| mo)
                .into_iter()
                .collect();
            (BlameConfidence::Low, last, false)
        } else {
            let conf = match new_topics.len() {
                1 => BlameConfidence::High,
                _ => BlameConfidence::Medium,
            };
            let mut sorted = new_topics;
            sorted.sort_by(|a, b| b.2.cmp(&a.2));
            (conf, sorted, true)
        };

        let suspects = build_suspects(conn, &suspect_entries, is_new)?;
        (suspects, confidence, Some(green.id.clone()))
    } else {
        let last: Vec<&(TopicId, i32, i32)> = red_topics
            .iter()
            .max_by_key(|(_, _, mo)| mo)
            .into_iter()
            .collect();
        let suspects = build_suspects(conn, &last, false)?;
        (suspects, BlameConfidence::Low, None)
    };

    Ok(BlameResult {
        env_id: env_id.clone(),
        env_name: env.name,
        red_rebuild_id: red_rebuild.id,
        green_rebuild_id,
        suspects,
        confidence,
    })
}

pub fn find_last_green_rebuild(conn: &Connection, env_id: &EnvId) -> Result<Option<Rebuild>> {
    rebuild_repo::get_last_successful_ci_rebuild(conn, env_id)
}

fn build_suspects(
    conn: &Connection,
    entries: &[&(TopicId, i32, i32)],
    is_new_since_green: bool,
) -> Result<Vec<BlameSuspect>> {
    let mut suspects = Vec::new();
    for (topic_id, phase, merge_order) in entries {
        let topic = topic_repo::get_topic(conn, topic_id)?;
        suspects.push(BlameSuspect {
            topic_id: (*topic_id).clone(),
            branch: topic.branch,
            phase: *phase,
            merge_order: *merge_order,
            is_new_since_green,
        });
    }
    Ok(suspects)
}
