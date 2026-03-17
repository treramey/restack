use crate::commands::integration::IntegrationCommand;
use crate::commands::topic::TopicCommand;
use crate::output::Printer;

use super::Command;

pub fn format_human(command: &Command, json_output: &str, printer: &Printer) -> String {
    let val: serde_json::Value = match serde_json::from_str(json_output) {
        Ok(v) => v,
        Err(_) => return json_output.to_string(),
    };

    match command {
        Command::Init => fmt_init(&val, printer),
        Command::List => fmt_repo_list(&val),
        Command::Refresh { .. } => fmt_refresh(&val),
        Command::Add { all, .. } => fmt_add(&val, *all),
        Command::Remove { .. } => fmt_remove(&val),
        Command::Topic(cmd) => fmt_topic(cmd, &val),
        Command::Integration(cmd) => fmt_integration(cmd, &val),
        // These are handled before reaching this point
        Command::Ui { .. } | Command::Serve => json_output.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Init
// ---------------------------------------------------------------------------

fn fmt_init(val: &serde_json::Value, _printer: &Printer) -> String {
    let path = val["path"].as_str().unwrap_or(".");
    let mut out = format!("Workspace initialized at {}", path);
    if let Some(repo) = val["repo"].as_object() {
        let name = repo.get("name").and_then(|v| v.as_str()).unwrap_or("?");
        out.push_str(&format!("\n  Repository added: {}", name));
    }
    out
}

// ---------------------------------------------------------------------------
// List (repos)
// ---------------------------------------------------------------------------

fn fmt_repo_list(val: &serde_json::Value) -> String {
    let repos = match val.as_array() {
        Some(a) => a,
        None => return val.to_string(),
    };
    if repos.is_empty() {
        return "No repositories tracked.".to_string();
    }
    let mut lines = vec!["Repositories:".to_string()];
    for repo in repos {
        let name = repo["name"].as_str().unwrap_or("?");
        let path = repo["path"].as_str().unwrap_or("?");
        let provider = repo["provider"].as_str().unwrap_or("?");
        let base = repo["baseBranch"].as_str().unwrap_or("?");
        lines.push(format!(
            "  {:<20} {:<40} ({}, base: {})",
            name, path, provider, base
        ));
    }
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Refresh
// ---------------------------------------------------------------------------

fn fmt_refresh(val: &serde_json::Value) -> String {
    let results = match val.as_array() {
        Some(a) => a,
        None => return val.to_string(),
    };
    if results.is_empty() {
        return "Nothing to refresh.".to_string();
    }
    let mut lines = Vec::new();
    for r in results {
        let repo = r["repo"].as_str().unwrap_or("?");
        if let Some(err) = r["error"].as_str() {
            lines.push(format!("Refreshed {}: error — {}", repo, err));
        } else {
            let ci = r["ci_refreshed"].as_u64().unwrap_or(0);
            let discovered = r["discovery"]["topics"]
                .as_array()
                .map(|a| a.len())
                .unwrap_or(0);
            lines.push(format!(
                "Refreshed {}: {} topics discovered, {} CI updated",
                repo, discovered, ci
            ));
        }
    }
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Add
// ---------------------------------------------------------------------------

fn fmt_add(val: &serde_json::Value, all: bool) -> String {
    if all {
        let count = val.as_array().map(|a| a.len()).unwrap_or(0);
        return format!("Detected {} repositories", count);
    }
    // Single add: result has a "repo" key
    let name = val["repo"]["name"].as_str().unwrap_or("?");
    let path = val["repo"]["path"].as_str().unwrap_or("?");
    format!("Added repository: {} ({})", name, path)
}

// ---------------------------------------------------------------------------
// Remove
// ---------------------------------------------------------------------------

fn fmt_remove(_val: &serde_json::Value) -> String {
    "Repository removed.".to_string()
}

// ---------------------------------------------------------------------------
// Topic
// ---------------------------------------------------------------------------

fn fmt_topic(cmd: &TopicCommand, val: &serde_json::Value) -> String {
    match cmd {
        TopicCommand::List { all_repos, .. } => fmt_topic_list(val, *all_repos),
        TopicCommand::Close { .. } => fmt_topic_close(val),
        TopicCommand::Promote { all, .. } => {
            if *all {
                fmt_promote_all(val, "Promoted")
            } else {
                fmt_promote_single(val)
            }
        }
        TopicCommand::Demote { all, .. } => {
            if *all {
                fmt_demote_all(val)
            } else {
                fmt_demote_single(val)
            }
        }
        TopicCommand::Status { .. } => fmt_topic_status(val),
        TopicCommand::Envs => fmt_topic_envs(val),
    }
}

fn fmt_topic_list(val: &serde_json::Value, all_repos: bool) -> String {
    if all_repos {
        let repos = match val.as_array() {
            Some(a) => a,
            None => return val.to_string(),
        };
        if repos.is_empty() {
            return "No topics tracked.".to_string();
        }
        let mut lines = Vec::new();
        for repo_entry in repos {
            let repo_name = repo_entry["repoName"].as_str().unwrap_or("?");
            let topics = repo_entry["topics"].as_array();
            lines.push(format!("Topics ({}):", repo_name));
            match topics {
                Some(t) if !t.is_empty() => {
                    for topic in t {
                        lines.push(fmt_topic_row(topic));
                    }
                }
                _ => lines.push("  (none)".to_string()),
            }
        }
        return lines.join("\n");
    }

    let topics = match val.as_array() {
        Some(a) => a,
        None => return val.to_string(),
    };
    if topics.is_empty() {
        return "No topics tracked.".to_string();
    }
    let mut lines = vec!["Topics:".to_string()];
    for topic in topics {
        lines.push(fmt_topic_row(topic));
    }
    lines.join("\n")
}

fn fmt_topic_row(topic: &serde_json::Value) -> String {
    let branch = topic["branch"].as_str().unwrap_or("?");
    let status = topic["status"].as_str().unwrap_or("?");
    let origin = topic["branchOrigin"].as_str().unwrap_or("?");
    format!("  {:<40} {:<12} {}", branch, status, origin)
}

fn fmt_topic_close(val: &serde_json::Value) -> String {
    let branch = val["branch"].as_str().unwrap_or("?");
    format!("Closed topic: {}", branch)
}

fn fmt_promote_single(val: &serde_json::Value) -> String {
    let branch = val["topic"]["branch"].as_str().unwrap_or("?");
    let env = val["env"]["name"].as_str().unwrap_or("?");
    format!("Promoted {} -> {}", branch, env)
}

fn fmt_demote_single(val: &serde_json::Value) -> String {
    let branch = val["topic"]["branch"].as_str().unwrap_or("?");
    let env = val["env"]["name"].as_str().unwrap_or("?");
    format!("Demoted {} from {}", branch, env)
}

fn fmt_promote_all(val: &serde_json::Value, verb: &str) -> String {
    // Check for empty message
    if let Some(msg) = val["message"].as_str() {
        return msg.to_string();
    }
    let source = val["source_env"].as_str().unwrap_or("?");
    let count = val["promoted"].as_u64().unwrap_or(0);
    let results = val["results"].as_array();

    let mut lines = vec![format!("{} {} topics from {}:", verb, count, source)];
    if let Some(results) = results {
        for r in results {
            let topic = r["topic"].as_str().unwrap_or("?");
            if let Some(err) = r["error"].as_str() {
                lines.push(format!("  {}: error — {}", topic, err));
            } else {
                let to_env = r["to_env"].as_str().unwrap_or("?");
                lines.push(format!("  {} -> {}", topic, to_env));
            }
        }
    }
    lines.join("\n")
}

fn fmt_demote_all(val: &serde_json::Value) -> String {
    if let Some(msg) = val["message"].as_str() {
        return msg.to_string();
    }
    let source = val["source_env"].as_str().unwrap_or("?");
    let count = val["demoted"].as_u64().unwrap_or(0);
    let results = val["results"].as_array();

    let mut lines = vec![format!("Demoted {} topics from {}:", count, source)];
    if let Some(results) = results {
        for r in results {
            let topic = r["topic"].as_str().unwrap_or("?");
            if let Some(err) = r["error"].as_str() {
                lines.push(format!("  {}: error — {}", topic, err));
            } else {
                let from_env = r["from_env"].as_str().unwrap_or("?");
                lines.push(format!("  {} from {}", topic, from_env));
            }
        }
    }
    lines.join("\n")
}

fn fmt_topic_status(val: &serde_json::Value) -> String {
    let branch = val["topic"]["branch"].as_str().unwrap_or("?");
    let status = val["topic"]["status"].as_str().unwrap_or("?");
    let ci = val["topic"]["ciStatus"]
        .as_str()
        .unwrap_or("—");
    let mut lines = vec![
        format!("Topic: {}", branch),
        format!("  Status:    {}", status),
        format!("  CI:        {}", ci),
    ];
    if let Some(envs) = val["envs"].as_array() {
        if envs.is_empty() {
            lines.push("  Envs:      (none)".to_string());
        } else {
            let env_names: Vec<&str> = envs
                .iter()
                .filter_map(|e| e["name"].as_str())
                .collect();
            lines.push(format!("  Envs:      {}", env_names.join(", ")));
        }
    }
    lines.join("\n")
}

fn fmt_topic_envs(val: &serde_json::Value) -> String {
    let associations = match val.as_array() {
        Some(a) => a,
        None => return val.to_string(),
    };
    if associations.is_empty() {
        return "No topic-environment associations.".to_string();
    }
    let mut lines = vec![format!(
        "  {:<36}  {:<36}  {}",
        "Topic ID", "Env ID", "Added At"
    )];
    for assoc in associations {
        let topic_id = assoc["topicId"].as_str().unwrap_or("?");
        let env_id = assoc["envId"].as_str().unwrap_or("?");
        let added_at = assoc["addedAt"].as_str().unwrap_or("?");
        lines.push(format!("  {:<36}  {:<36}  {}", topic_id, env_id, added_at));
    }
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Integration
// ---------------------------------------------------------------------------

fn fmt_integration(cmd: &IntegrationCommand, val: &serde_json::Value) -> String {
    match cmd {
        IntegrationCommand::List { all_repos, .. } => fmt_integration_list(val, *all_repos),
        IntegrationCommand::Add { .. } => fmt_integration_add(val),
        IntegrationCommand::Status { .. } => fmt_integration_status(val),
        IntegrationCommand::CiOverride { env_name, .. } => {
            format!("CI override applied for {}", env_name)
        }
        IntegrationCommand::Blame { .. } => fmt_integration_blame(val),
        IntegrationCommand::SpeculativeStatus { .. } => fmt_speculative_status(val),
        IntegrationCommand::Init { .. } => fmt_integration_init(val),
        IntegrationCommand::Rebuild => fmt_integration_rebuild(val),
    }
}

fn fmt_integration_list(val: &serde_json::Value, all_repos: bool) -> String {
    if all_repos {
        let repos = match val.as_array() {
            Some(a) => a,
            None => return val.to_string(),
        };
        if repos.is_empty() {
            return "No integration branches configured.".to_string();
        }
        let mut lines = Vec::new();
        for repo_entry in repos {
            let repo_name = repo_entry["repoName"].as_str().unwrap_or("?");
            let envs = repo_entry["environments"].as_array();
            lines.push(format!("Integration branches ({}):", repo_name));
            match envs {
                Some(e) if !e.is_empty() => {
                    for env in e {
                        lines.push(fmt_env_row(env));
                    }
                }
                _ => lines.push("  (none)".to_string()),
            }
        }
        return lines.join("\n");
    }

    let envs = match val.as_array() {
        Some(a) => a,
        None => return val.to_string(),
    };
    if envs.is_empty() {
        return "No integration branches configured.".to_string();
    }
    let mut lines = vec!["Integration branches:".to_string()];
    for env in envs {
        lines.push(fmt_env_row(env));
    }
    lines.join("\n")
}

fn fmt_env_row(env: &serde_json::Value) -> String {
    let name = env["name"].as_str().unwrap_or("?");
    let branch = env["branch"].as_str().unwrap_or("?");
    let ordinal = env["ordinal"].as_i64().unwrap_or(0);
    let ci = env["ciStatus"].as_str().unwrap_or("—");
    format!(
        "  {:<16} {:<32} ordinal: {:<4} CI: {}",
        name, branch, ordinal, ci
    )
}

fn fmt_integration_add(val: &serde_json::Value) -> String {
    let name = val["name"].as_str().unwrap_or("?");
    let branch = val["branch"].as_str().unwrap_or("?");
    let ordinal = val["ordinal"].as_i64().unwrap_or(0);
    format!(
        "Added integration branch: {} -> {} (ordinal: {})",
        name, branch, ordinal
    )
}

fn fmt_integration_status(val: &serde_json::Value) -> String {
    let name = val["env"]["name"].as_str().unwrap_or("?");
    let branch = val["env"]["branch"].as_str().unwrap_or("?");
    let ci = val["env"]["ciStatus"].as_str().unwrap_or("—");
    let topic_count = val["topics"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    format!(
        "Integration: {} ({})\n  CI: {}\n  Topics: {}",
        name, branch, ci, topic_count
    )
}

fn fmt_integration_blame(val: &serde_json::Value) -> String {
    // Generic JSON fallback for blame — complex nested structure
    if let Some(culprit) = val["culprit"].as_object() {
        let branch = culprit
            .get("branch")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        let reason = val["reason"].as_str().unwrap_or("unknown");
        format!("Likely culprit: {} ({})", branch, reason)
    } else if let Some(msg) = val["message"].as_str() {
        msg.to_string()
    } else {
        val.to_string()
    }
}

fn fmt_speculative_status(val: &serde_json::Value) -> String {
    if let Some(msg) = val["message"].as_str() {
        return msg.to_string();
    }
    // Show per-step statuses if present
    let steps = val["steps"].as_array();
    if let Some(steps) = steps {
        let mut lines = vec!["Speculative CI status:".to_string()];
        for step in steps {
            let topic = step["topic"].as_str().unwrap_or("?");
            let ci = step["ciStatus"].as_str().unwrap_or("pending");
            lines.push(format!("  {:<40} {}", topic, ci));
        }
        return lines.join("\n");
    }
    val.to_string()
}

fn fmt_integration_init(val: &serde_json::Value) -> String {
    if let Some(msg) = val["message"].as_str() {
        return msg.to_string();
    }
    let created = val["created"]
        .as_array()
        .map(|a| a.len())
        .unwrap_or(0);
    let warnings = val["warnings"].as_array();
    let mut lines = vec![format!("Initialized {} integration branch(es).", created)];
    if let Some(ws) = warnings {
        for w in ws {
            if let Some(s) = w.as_str() {
                lines.push(format!("  Warning: {}", s));
            }
        }
    }
    lines.join("\n")
}

fn fmt_integration_rebuild(val: &serde_json::Value) -> String {
    let results = match val.as_array() {
        Some(a) => a,
        None => return val.to_string(),
    };
    if results.is_empty() {
        return "No integration branches rebuilt.".to_string();
    }
    let mut lines = Vec::new();
    for r in results {
        let env_id = r["envId"].as_str().unwrap_or("?");
        let status = r["status"].as_str().unwrap_or("?");
        let merged = r["topicsMerged"].as_i64().unwrap_or(0);
        let conflicted = r["topicsConflicted"].as_i64().unwrap_or(0);
        lines.push(format!(
            "  {} — {} ({} merged, {} conflicted)",
            env_id, status, merged, conflicted
        ));
    }
    lines.join("\n")
}
