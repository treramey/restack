use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::db::{env_repo, rebuild_repo, topic_repo};
use crate::error::Result;
use crate::id::{EnvId, TopicId};
use crate::provider;
use crate::types::{CiStatus, RebuildStatus, Repo, TopicStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvCiRefreshEntry {
    pub env_id: EnvId,
    pub env_name: String,
    pub old_status: Option<CiStatus>,
    pub new_status: CiStatus,
    pub blamed_topic: Option<TopicId>,
}

pub fn refresh_env_ci_statuses(conn: &Connection, repo: &Repo) -> Result<Vec<EnvCiRefreshEntry>> {
    let adapter = provider::create_adapter(repo.provider, repo.remote_url.as_deref());
    if !adapter.is_available() {
        return Ok(Vec::new());
    }

    let envs = env_repo::list_envs(conn, Some(&repo.id))?;
    let mut entries = Vec::new();

    for env in &envs {
        let rebuild = match rebuild_repo::get_last_rebuild(conn, &env.id)? {
            Some(r) => r,
            None => continue,
        };

        // Skip if the merge itself failed
        if rebuild.status != RebuildStatus::Success {
            continue;
        }

        // Skip if CI already resolved
        match rebuild.ci_status {
            Some(CiStatus::Passed) | Some(CiStatus::Failed) => continue,
            _ => {}
        }

        // Skip if CI override is set
        if rebuild.ci_override.is_some() {
            continue;
        }

        let detail = match adapter.get_ci_status(&env.branch) {
            Ok(d) => d,
            Err(_) => continue,
        };

        let old_status = env.ci_status;
        let new_status = detail.overall;
        let ci_url = detail
            .checks
            .iter()
            .find_map(|c| c.url.as_deref())
            .or(detail.sha.as_deref());

        rebuild_repo::set_rebuild_ci_status(conn, &rebuild.id, Some(new_status), ci_url)?;
        env_repo::set_env_ci_status(conn, &env.id, Some(new_status), ci_url)?;

        let mut blamed_topic = None;

        if new_status == CiStatus::Failed {
            let max_retries = 1u32;
            if (rebuild.ci_retry_count as u32) < max_retries {
                // Retry: increment count, reset to pending
                rebuild_repo::increment_rebuild_ci_retry(conn, &rebuild.id)?;
                rebuild_repo::set_rebuild_ci_status(
                    conn,
                    &rebuild.id,
                    Some(CiStatus::Pending),
                    ci_url,
                )?;
                env_repo::set_env_ci_status(conn, &env.id, Some(CiStatus::Pending), ci_url)?;
            } else {
                let blame_outcome = super::speculative_ci_service::speculative_blame_or_fallback(
                    conn, &env.id, repo,
                )?;
                match blame_outcome {
                    super::speculative_ci_service::BlameOutcome::NoPending => {
                        // Speculative CI still pending — reset to Pending and retry next cycle
                        rebuild_repo::set_rebuild_ci_status(
                            conn,
                            &rebuild.id,
                            Some(CiStatus::Pending),
                            ci_url,
                        )?;
                        env_repo::set_env_ci_status(
                            conn,
                            &env.id,
                            Some(CiStatus::Pending),
                            ci_url,
                        )?;
                    }
                    super::speculative_ci_service::BlameOutcome::Speculative(spec) => {
                        if let Some(ref culprit_id) = spec.culprit_topic_id {
                            topic_repo::update_topic_status(
                                conn,
                                culprit_id,
                                TopicStatus::CiQuarantined,
                            )?;
                            blamed_topic = Some(culprit_id.clone());

                            if let Ok(topic) = topic_repo::get_topic(conn, culprit_id) {
                                if let Some(pr_number) = topic.pr_id.as_deref() {
                                    let confidence_str = format!(
                                        "exact speculative blame (step {})",
                                        spec.breakpoint_step.unwrap_or(-1)
                                    );
                                    let body = format!(
                                        "**Restack env CI failure** ({})\n\n\
                                         Branch `{}` is the likely cause of CI failure on environment `{}`.\n\
                                         {}\n\
                                         The topic has been quarantined.",
                                        confidence_str, topic.branch, env.name,
                                        ci_url.map(|u| format!("CI run: {u}\n")).unwrap_or_default(),
                                    );
                                    let _ = adapter.comment_on_pr(pr_number, &body);
                                }
                            }
                        }
                    }
                    super::speculative_ci_service::BlameOutcome::Differential(diff) => {
                        if let Some(suspect) = diff.suspects.first() {
                            topic_repo::update_topic_status(
                                conn,
                                &suspect.topic_id,
                                TopicStatus::CiQuarantined,
                            )?;
                            blamed_topic = Some(suspect.topic_id.clone());

                            if let Ok(topic) = topic_repo::get_topic(conn, &suspect.topic_id) {
                                if let Some(pr_number) = topic.pr_id.as_deref() {
                                    let confidence_str = match diff.confidence {
                                        super::blame_service::BlameConfidence::High => {
                                            "high confidence"
                                        }
                                        super::blame_service::BlameConfidence::Medium => {
                                            "medium confidence (multiple new topics)"
                                        }
                                        super::blame_service::BlameConfidence::Low => {
                                            "low confidence (heuristic)"
                                        }
                                    };
                                    let other_suspects = if diff.suspects.len() > 1 {
                                        format!(
                                            "\n\nOther suspects: {}",
                                            diff.suspects[1..]
                                                .iter()
                                                .map(|s| format!("`{}`", s.branch))
                                                .collect::<Vec<_>>()
                                                .join(", ")
                                        )
                                    } else {
                                        String::new()
                                    };
                                    let body = format!(
                                        "**Restack env CI failure** ({})\n\n\
                                         Branch `{}` is the likely cause of CI failure on environment `{}`.\n\
                                         {}{}\n\
                                         The topic has been quarantined.",
                                        confidence_str, topic.branch, env.name,
                                        ci_url.map(|u| format!("CI run: {u}\n")).unwrap_or_default(),
                                        other_suspects,
                                    );
                                    let _ = adapter.comment_on_pr(pr_number, &body);
                                }
                            }
                        }
                    }
                }
            }
        }

        entries.push(EnvCiRefreshEntry {
            env_id: env.id.clone(),
            env_name: env.name.clone(),
            old_status,
            new_status,
            blamed_topic,
        });
    }

    Ok(entries)
}
