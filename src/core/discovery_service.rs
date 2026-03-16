use std::collections::{HashMap, HashSet};
use std::path::Path;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::config::{DiscoveryMode, WorkspaceConfig};
use crate::db::{env_repo, repo_repo, topic_env_repo, topic_repo};
use crate::error::Result;
use crate::git;
use crate::id::{EnvId, RepoId, TopicId};
use crate::types::{BranchOrigin, Environment, Topic, TopicStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryResult {
    pub discovered: i32,
    pub created: i32,
    pub closed: i32,
    pub skipped: i32,
    pub topics: Vec<Topic>,
}

#[derive(Debug, Clone)]
pub enum RepoMutation {
    CreateTopic { branch: String, origin: BranchOrigin },
    UpdateOrigin { topic_id: TopicId, origin: BranchOrigin },
    DeleteTopic { topic_id: TopicId },
    AssignToEnv { topic_id: TopicId, env_id: EnvId },
    RemoveFromAllEnvs { topic_id: TopicId },
    UpdateStatus { topic_id: TopicId, status: TopicStatus },
}

pub struct RepoSnapshot {
    pub topics: Vec<Topic>,
    pub envs: Vec<Environment>,
    pub topic_envs: HashMap<TopicId, Vec<EnvId>>,
}

/// Git merge data collected during the plan phase for use in the apply phase.
pub struct MergeData {
    pub merged_into_base: HashSet<String>,
    pub env_merged: Vec<(EnvId, HashSet<String>)>,
    pub new_branches: HashSet<String>,
}

/// Pure git I/O + computation phase. No DB writes.
/// Returns (mutations, counts, new_fingerprint, merge_data).
/// `new_fingerprint` is None if the fingerprint matched (cache hit — no work done).
pub fn discover_topics_plan(
    repo: &crate::types::Repo,
    snapshot: &RepoSnapshot,
    config: &WorkspaceConfig,
) -> Result<(Vec<RepoMutation>, DiscoveryResult, Option<String>, Option<MergeData>)> {
    let repo_path = Path::new(&repo.path);

    // Fingerprint gating: skip if refs haven't changed
    let new_fingerprint = git::compute_refs_fingerprint(repo_path)
        .unwrap_or_default();
    if !new_fingerprint.is_empty()
        && repo.refs_fingerprint.as_deref() == Some(new_fingerprint.as_str())
    {
        return Ok((
            Vec::new(),
            DiscoveryResult { discovered: 0, created: 0, closed: 0, skipped: 0, topics: Vec::new() },
            None,
            None,
        ));
    }

    if git::has_remote(repo_path) {
        let _ = git::fetch(repo_path);
    }

    let env_branches: Vec<&str> = snapshot.envs.iter().map(|e| e.branch.as_str()).collect();

    let branches = git::list_branch_presence(repo_path)?;

    let known_branches: HashSet<String> =
        branches.iter().map(|b| b.branch.clone()).collect();
    let remote_branches: HashSet<String> =
        branches.iter().filter(|b| b.has_remote).map(|b| b.branch.clone()).collect();
    let local_branches: HashSet<String> =
        branches.iter().filter(|b| b.has_local).map(|b| b.branch.clone()).collect();

    let mut mutations: Vec<RepoMutation> = Vec::new();
    let mut discovered = 0i32;
    let mut created = 0i32;
    let mut skipped = 0i32;

    // Branch iteration: create or update-origin mutations
    for bp in &branches {
        if !should_track_branch(config.discovery.mode, bp.has_local, bp.has_remote) {
            skipped += 1;
            continue;
        }
        if config.discovery.should_exclude(&bp.branch, &env_branches) {
            skipped += 1;
            continue;
        }

        let existing = snapshot.topics.iter().find(|t| t.branch == bp.branch);
        if let Some(topic) = existing {
            let next_origin =
                classify_branch_origin(topic.branch_origin, bp.has_local, bp.has_remote);
            if next_origin != topic.branch_origin {
                mutations.push(RepoMutation::UpdateOrigin {
                    topic_id: topic.id.clone(),
                    origin: next_origin,
                });
            }
            discovered += 1;
        } else {
            let origin = classify_new_topic_origin(bp.has_local, bp.has_remote);
            mutations.push(RepoMutation::CreateTopic { branch: bp.branch.clone(), origin });
            discovered += 1;
            created += 1;
        }
    }

    // Close deleted topics: active topics whose branch no longer exists
    let mut closed_topic_ids: HashSet<TopicId> = HashSet::new();
    let mut closed = 0i32;
    for topic in &snapshot.topics {
        if topic.status == TopicStatus::Closed || topic.status == TopicStatus::Graduated {
            continue;
        }
        if !known_branches.contains(&topic.branch) {
            mutations.push(RepoMutation::RemoveFromAllEnvs { topic_id: topic.id.clone() });
            mutations.push(RepoMutation::DeleteTopic { topic_id: topic.id.clone() });
            closed_topic_ids.insert(topic.id.clone());
            closed += 1;
        }
    }

    // Collect git merge data for graduation + env assignment.
    let merged_into_base: HashSet<String> =
        preferred_ref(&repo.base_branch, &remote_branches, &local_branches)
            .and_then(|base_ref| git::list_branches_merged_into(repo_path, &base_ref).ok())
            .unwrap_or_default()
            .into_iter()
            .collect();

    let mut env_merged: Vec<(EnvId, HashSet<String>)> = Vec::new();
    for env in &snapshot.envs {
        let env_ref = match preferred_ref(&env.branch, &remote_branches, &local_branches) {
            Some(r) => r,
            None => continue,
        };
        let merged: HashSet<String> =
            git::list_branches_merged_into(repo_path, &env_ref)
                .unwrap_or_default()
                .into_iter()
                .collect();
        env_merged.push((env.id.clone(), merged));
    }

    // Graduation for existing topics (in snapshot)
    for topic in &snapshot.topics {
        if topic.status == TopicStatus::Closed || topic.status == TopicStatus::Graduated {
            continue;
        }
        if closed_topic_ids.contains(&topic.id) {
            continue;
        }
        if merged_into_base.contains(&topic.branch) {
            mutations.push(RepoMutation::RemoveFromAllEnvs { topic_id: topic.id.clone() });
            mutations.push(RepoMutation::UpdateStatus {
                topic_id: topic.id.clone(),
                status: TopicStatus::Graduated,
            });
        }
    }

    // Env assignment for existing topics (in snapshot)
    let active_snapshot_topics: HashMap<&str, &Topic> = snapshot
        .topics
        .iter()
        .filter(|t| t.status != TopicStatus::Closed && t.status != TopicStatus::Graduated)
        .filter(|t| !closed_topic_ids.contains(&t.id))
        .filter(|t| !merged_into_base.contains(&t.branch))
        .map(|t| (t.branch.as_str(), t))
        .collect();

    for (env_id, merged) in &env_merged {
        for branch_name in merged {
            let topic = match active_snapshot_topics.get(branch_name.as_str()) {
                Some(t) => t,
                None => continue,
            };
            let already_in_env = snapshot
                .topic_envs
                .get(&topic.id)
                .map(|envs| envs.contains(env_id))
                .unwrap_or(false);
            if already_in_env {
                continue;
            }
            mutations.push(RepoMutation::AssignToEnv {
                topic_id: topic.id.clone(),
                env_id: env_id.clone(),
            });
        }
    }

    // Track newly created branches so apply phase can graduate/assign them
    let new_branches: HashSet<String> = mutations
        .iter()
        .filter_map(|m| match m {
            RepoMutation::CreateTopic { branch, .. } => Some(branch.clone()),
            _ => None,
        })
        .collect();

    let merge_data = MergeData { merged_into_base, env_merged, new_branches };

    let result = DiscoveryResult {
        discovered,
        created,
        closed,
        skipped,
        topics: Vec::new(), // caller populates after apply
    };
    let fp_result = if new_fingerprint.is_empty() { None } else { Some(new_fingerprint) };
    Ok((mutations, result, fp_result, Some(merge_data)))
}

/// Pure DB write phase. Applies mutations, then handles graduation + env assignment
/// for any newly created topics using the merge data from the plan phase.
pub fn apply_mutations(
    conn: &Connection,
    repo_id: &RepoId,
    mutations: &[RepoMutation],
    merge_data: Option<&MergeData>,
) -> Result<()> {
    for mutation in mutations {
        match mutation {
            RepoMutation::CreateTopic { branch, origin } => {
                topic_repo::create_topic(conn, repo_id, branch, *origin, None, None)?;
            }
            RepoMutation::UpdateOrigin { topic_id, origin } => {
                topic_repo::update_topic_branch_origin(conn, topic_id, *origin)?;
            }
            RepoMutation::DeleteTopic { topic_id } => {
                topic_repo::delete_topic(conn, topic_id)?;
            }
            RepoMutation::AssignToEnv { topic_id, env_id } => {
                if let Err(e) = topic_env_repo::add_topic_to_env(conn, topic_id, env_id) {
                    eprintln!(
                        "Warning: failed to assign topic '{}' to env '{}': {}",
                        topic_id, env_id, e
                    );
                }
            }
            RepoMutation::RemoveFromAllEnvs { topic_id } => {
                topic_env_repo::remove_topic_from_all_envs(conn, topic_id)?;
            }
            RepoMutation::UpdateStatus { topic_id, status } => {
                topic_repo::update_topic_status(conn, topic_id, *status)?;
            }
        }
    }

    // Second pass: handle newly created topics that need graduation or env assignment.
    // These topics didn't exist in the snapshot, so the plan couldn't emit mutations for them.
    if let Some(md) = merge_data {
        if !md.new_branches.is_empty() {
            for branch in &md.new_branches {
                let topic = match topic_repo::get_topic_by_branch(conn, repo_id, branch)? {
                    Some(t) => t,
                    None => continue,
                };

                if md.merged_into_base.contains(branch) {
                    topic_env_repo::remove_topic_from_all_envs(conn, &topic.id)?;
                    topic_repo::update_topic_status(conn, &topic.id, TopicStatus::Graduated)?;
                    continue;
                }

                for (env_id, merged) in &md.env_merged {
                    if merged.contains(branch) {
                        if let Err(e) = topic_env_repo::add_topic_to_env(conn, &topic.id, env_id) {
                            eprintln!(
                                "Warning: failed to assign new topic '{}' to env '{}': {}",
                                branch, env_id, e
                            );
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Thin wrapper for backward compatibility with `repo add --discover` and similar callers.
pub fn discover_topics(
    conn: &Connection,
    repo_id: &RepoId,
    _repo_path: &Path,
    config: &WorkspaceConfig,
) -> Result<DiscoveryResult> {
    let repo = repo_repo::get_repo(conn, repo_id)?;

    // Build per-repo snapshot from DB
    let topics = topic_repo::list_topics(conn, Some(repo_id))?;
    let envs = env_repo::list_envs(conn, Some(repo_id))?;
    let mut topic_envs: HashMap<TopicId, Vec<EnvId>> = HashMap::new();
    for topic in &topics {
        let env_ids = topic_env_repo::get_envs_for_topic(conn, &topic.id)?;
        if !env_ids.is_empty() {
            topic_envs.insert(topic.id.clone(), env_ids);
        }
    }
    let snapshot = RepoSnapshot { topics, envs, topic_envs };

    let (mutations, mut result, _fingerprint, merge_data) =
        discover_topics_plan(&repo, &snapshot, config)?;

    if !mutations.is_empty() || merge_data.is_some() {
        apply_mutations(conn, repo_id, &mutations, merge_data.as_ref())?;
    }

    // Do NOT write fingerprint here — only handle_refresh should cache it,
    // because this wrapper is called during `repo add --discover` and `topic discover`
    // where the initial scan must not prevent the first refresh from running fully.

    result.topics = topic_repo::list_topics(conn, Some(repo_id))?;
    Ok(result)
}

fn should_track_branch(mode: DiscoveryMode, has_local: bool, has_remote: bool) -> bool {
    match mode {
        DiscoveryMode::OriginOnly => has_remote,
        DiscoveryMode::LocalOnly => has_local && !has_remote,
        DiscoveryMode::All => has_local || has_remote,
    }
}

fn classify_new_topic_origin(has_local: bool, has_remote: bool) -> BranchOrigin {
    if has_remote {
        BranchOrigin::Tracked
    } else if has_local {
        BranchOrigin::LocalOnly
    } else {
        BranchOrigin::Tracked
    }
}

fn classify_branch_origin(
    previous: BranchOrigin,
    has_local: bool,
    has_remote: bool,
) -> BranchOrigin {
    if has_remote {
        BranchOrigin::Tracked
    } else if has_local {
        match previous {
            BranchOrigin::Tracked | BranchOrigin::Orphaned => BranchOrigin::Orphaned,
            BranchOrigin::LocalOnly => BranchOrigin::LocalOnly,
        }
    } else {
        previous
    }
}

/// Resolve a single canonical git ref for a branch name, preferring origin/.
fn preferred_ref(
    branch: &str,
    remote_branches: &HashSet<String>,
    local_branches: &HashSet<String>,
) -> Option<String> {
    if remote_branches.contains(branch) {
        Some(format!("refs/remotes/origin/{branch}"))
    } else if local_branches.contains(branch) {
        Some(format!("refs/heads/{branch}"))
    } else {
        None
    }
}

pub fn archive_topic(conn: &Connection, topic_id: &crate::id::TopicId) -> Result<Topic> {
    topic_repo::update_topic_status_return(conn, topic_id, TopicStatus::Closed)
}
