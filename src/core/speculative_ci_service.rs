use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::db::{env_repo, rebuild_repo, speculative_ref_repo, topic_repo};
use crate::error::{RestackError, Result};
use crate::id::{EnvId, RebuildId, TopicId};
use crate::provider;
use crate::types::{CiStatus, Repo};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeculativeBlameResult {
    pub env_id: EnvId,
    pub env_name: String,
    pub rebuild_id: RebuildId,
    pub breakpoint_step: Option<i32>,
    pub culprit_topic_id: Option<TopicId>,
    pub culprit_branch: Option<String>,
    pub steps_checked: i32,
    pub steps_passed: i32,
    pub steps_failed: i32,
    pub steps_pending: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BlameOutcome {
    Speculative(SpeculativeBlameResult),
    Differential(crate::core::blame_service::BlameResult),
    NoPending,
}

/// Check CI status on all speculative refs for the latest rebuild of an env.
/// Returns the exact breakpoint if found.
pub fn check_speculative_ci(
    conn: &Connection,
    env_id: &EnvId,
    repo: &Repo,
) -> Result<SpeculativeBlameResult> {
    let env = env_repo::get_env(conn, env_id)?;

    let rebuild = rebuild_repo::get_last_rebuild(conn, env_id)?.ok_or_else(|| {
        RestackError::RebuildFailed {
            env: env.name.clone(),
            reason: "no rebuilds found".to_string(),
        }
    })?;

    let spec_refs = speculative_ref_repo::get_refs_for_rebuild(conn, &rebuild.id)?;

    let adapter = provider::create_adapter(repo.provider, repo.remote_url.as_deref());

    let mut steps_checked = 0_i32;

    if adapter.is_available() {
        for spec_ref in &spec_refs {
            let detail = match adapter.get_ci_status(&spec_ref.branch_name) {
                Ok(d) => d,
                Err(_) => continue,
            };

            let ci_url = detail
                .checks
                .iter()
                .find_map(|c| c.url.as_deref())
                .or(detail.sha.as_deref());

            speculative_ref_repo::set_ci_status(conn, &spec_ref.id, detail.overall, ci_url)?;
            steps_checked += 1;
        }
    }

    // Re-fetch to get updated statuses
    let updated_refs = speculative_ref_repo::get_refs_for_rebuild(conn, &rebuild.id)?;

    let mut steps_passed = 0_i32;
    let mut steps_failed = 0_i32;
    let mut steps_pending = 0_i32;

    for spec_ref in &updated_refs {
        match spec_ref.ci_status {
            Some(CiStatus::Passed) => steps_passed += 1,
            Some(CiStatus::Failed) => steps_failed += 1,
            Some(CiStatus::Pending) | None => steps_pending += 1,
        }
    }

    // Find breakpoint: first Failed step where the previous step Passed (or it is step 0)
    let mut breakpoint_step = None;
    let mut culprit_topic_id = None;
    let mut culprit_branch = None;

    for (i, spec_ref) in updated_refs.iter().enumerate() {
        if spec_ref.ci_status == Some(CiStatus::Failed) {
            let prev_passed = i == 0
                || updated_refs[i - 1].ci_status == Some(CiStatus::Passed);

            if prev_passed {
                breakpoint_step = Some(spec_ref.step);
                culprit_topic_id = Some(spec_ref.topic_id.clone());
                if let Ok(topic) = topic_repo::get_topic(conn, &spec_ref.topic_id) {
                    culprit_branch = Some(topic.branch);
                }
                break;
            }
        }
    }

    Ok(SpeculativeBlameResult {
        env_id: env_id.clone(),
        env_name: env.name,
        rebuild_id: rebuild.id,
        breakpoint_step,
        culprit_topic_id,
        culprit_branch,
        steps_checked,
        steps_passed,
        steps_failed,
        steps_pending,
    })
}

/// Try speculative blame first. If speculative refs exist and have CI results,
/// use them for exact blame. Otherwise fall back to differential blame.
pub fn speculative_blame_or_fallback(
    conn: &Connection,
    env_id: &EnvId,
    repo: &Repo,
) -> Result<BlameOutcome> {
    let rebuild = match rebuild_repo::get_last_rebuild(conn, env_id)? {
        Some(r) => r,
        None => return Ok(BlameOutcome::NoPending),
    };

    let spec_refs = speculative_ref_repo::get_refs_for_rebuild(conn, &rebuild.id)?;

    if spec_refs.is_empty() {
        let blame = super::blame_service::blame_env(conn, env_id)?;
        return Ok(BlameOutcome::Differential(blame));
    }

    let result = check_speculative_ci(conn, env_id, repo)?;

    if result.breakpoint_step.is_some() {
        return Ok(BlameOutcome::Speculative(result));
    }

    if result.steps_pending > 0 {
        return Ok(BlameOutcome::NoPending);
    }

    // All steps checked but no clean breakpoint — fall back to differential
    let blame = super::blame_service::blame_env(conn, env_id)?;
    Ok(BlameOutcome::Differential(blame))
}
