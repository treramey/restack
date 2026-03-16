use std::path::Path;
use std::process::{Command, Output};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::types::Provider;

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Error, Debug)]
pub enum GitError {
    #[error("Git command failed (exit {code}): {stderr}")]
    CommandFailed { code: i32, stderr: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Failed to parse git output: {0}")]
    Parse(String),
}

pub type GitResult<T> = Result<T, GitError>;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum MergeResult {
    Success { sha: String },
    Conflict,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeLogEntry {
    pub sha: String,
    pub subject: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchPresence {
    pub branch: String,
    pub has_local: bool,
    pub has_remote: bool,
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Run a git command in `repo_path`, returning trimmed stdout on success.
fn run_git(repo_path: &Path, args: &[&str]) -> GitResult<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(GitError::CommandFailed {
            code: output.status.code().unwrap_or(-1),
            stderr,
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Run a git command returning the raw `Output` for cases where exit code
/// carries semantic meaning (e.g. `merge-base --is-ancestor`).
fn run_git_raw(repo_path: &Path, args: &[&str]) -> GitResult<Output> {
    Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .map_err(GitError::from)
}

// ---------------------------------------------------------------------------
// Branch operations
// ---------------------------------------------------------------------------

pub fn branch_create(repo: &Path, name: &str, from_ref: &str) -> GitResult<()> {
    run_git(repo, &["checkout", "-b", name, from_ref])?;
    Ok(())
}

pub fn branch_create_at(repo: &Path, name: &str, from_ref: &str) -> GitResult<()> {
    run_git(repo, &["branch", name, from_ref])?;
    Ok(())
}

pub fn branch_delete(repo: &Path, name: &str, remote: bool) -> GitResult<()> {
    if remote {
        run_git(repo, &["push", "--delete", "origin", name])?;
    } else {
        run_git(repo, &["branch", "-d", name])?;
    }
    Ok(())
}

pub fn branch_reset(repo: &Path, name: &str, to_ref: &str) -> GitResult<()> {
    run_git(repo, &["checkout", "-B", name, to_ref])?;
    Ok(())
}

pub fn branch_exists(repo: &Path, name: &str) -> GitResult<bool> {
    let ref_name = format!("refs/heads/{name}");
    let output = run_git_raw(repo, &["rev-parse", "--verify", &ref_name])?;
    Ok(output.status.success())
}

/// Check if a branch exists locally OR remotely in a repo.
/// Returns true if the branch exists anywhere (refs/heads/ or refs/remotes/origin/).
pub fn branch_exists_anywhere(repo: &Path, name: &str) -> GitResult<bool> {
    let local_ref = format!("refs/heads/{name}");
    let remote_ref = format!("refs/remotes/origin/{name}");

    let local_output = run_git_raw(repo, &["rev-parse", "--verify", &local_ref])?;
    if local_output.status.success() {
        return Ok(true);
    }

    let remote_output = run_git_raw(repo, &["rev-parse", "--verify", &remote_ref])?;
    Ok(remote_output.status.success())
}

/// Check if a branch exists on the remote (origin/<name>).
pub fn remote_branch_exists(repo: &Path, name: &str) -> GitResult<bool> {
    let ref_name = format!("refs/remotes/origin/{name}");
    let output = run_git_raw(repo, &["rev-parse", "--verify", &ref_name])?;
    Ok(output.status.success())
}

/// List all local branch names.
pub fn list_local_branches(repo: &Path) -> GitResult<Vec<String>> {
    let output = run_git(repo, &["branch", "--format=%(refname:short)"])?;
    if output.is_empty() {
        return Ok(Vec::new());
    }
    Ok(output.lines().map(|l| l.trim().to_string()).collect())
}

/// List all branch names (local + remote, without origin/ prefix) merged into `target_ref`.
/// Uses a single `git branch -a --merged` call for efficiency.
pub fn list_branches_merged_into(repo: &Path, target_ref: &str) -> GitResult<Vec<String>> {
    let output = run_git(
        repo,
        &["branch", "-a", "--merged", target_ref, "--format=%(refname:short)"],
    )?;
    if output.is_empty() {
        return Ok(Vec::new());
    }
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for line in output.lines() {
        let name = line.trim();
        if name.is_empty() || name.contains("HEAD") {
            continue;
        }
        let stripped = name.strip_prefix("origin/").unwrap_or(name);
        if seen.insert(stripped.to_string()) {
            result.push(stripped.to_string());
        }
    }
    Ok(result)
}

/// List all remote branch names (without the origin/ prefix).
/// Filters out HEAD and other symbolic refs.
pub fn list_remote_branches(repo: &Path) -> GitResult<Vec<String>> {
    let output = run_git(repo, &["branch", "-r", "--format=%(refname:short)"])?;
    if output.is_empty() {
        return Ok(Vec::new());
    }
    Ok(output
        .lines()
        .map(|l| l.trim())
        .filter(|l| l.starts_with("origin/"))
        .filter(|l| !l.contains("HEAD")) // Skip origin/HEAD
        .filter_map(|l| l.strip_prefix("origin/"))
        .map(|l| l.to_string())
        .collect())
}

/// List all branches (local + remote), with a flag indicating if local.
/// Returns (branch_name, is_local) tuples. Deduplicates by preferring local.
pub fn list_all_branches(repo: &Path) -> GitResult<Vec<(String, bool)>> {
    let local = list_local_branches(repo)?;
    let remote = list_remote_branches(repo)?;

    let local_set: std::collections::HashSet<_> = local.iter().cloned().collect();
    let mut result: Vec<(String, bool)> = local.into_iter().map(|b| (b, true)).collect();

    for branch in remote {
        if !local_set.contains(&branch) {
            result.push((branch, false));
        }
    }

    Ok(result)
}

pub fn list_branch_presence(repo: &Path) -> GitResult<Vec<BranchPresence>> {
    let local = list_local_branches(repo)?;
    let remote = list_remote_branches(repo)?;

    let mut merged: std::collections::BTreeMap<String, BranchPresence> =
        std::collections::BTreeMap::new();

    for branch in local {
        let key = branch.clone();
        merged
            .entry(key)
            .and_modify(|entry| entry.has_local = true)
            .or_insert(BranchPresence {
                branch,
                has_local: true,
                has_remote: false,
            });
    }

    for branch in remote {
        let key = branch.clone();
        merged
            .entry(key)
            .and_modify(|entry| entry.has_remote = true)
            .or_insert(BranchPresence {
                branch,
                has_local: false,
                has_remote: true,
            });
    }

    Ok(merged.into_values().collect())
}

// ---------------------------------------------------------------------------
// Merge operations
// ---------------------------------------------------------------------------

pub fn merge_topic(repo: &Path, topic_branch: &str) -> GitResult<MergeResult> {
    let output = run_git_raw(repo, &["merge", "--no-ff", "--no-edit", topic_branch])?;

    if output.status.success() {
        let sha = run_git(repo, &["rev-parse", "HEAD"])?;
        return Ok(MergeResult::Success { sha });
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("CONFLICT") || stderr.contains("Automatic merge failed") {
        merge_abort(repo)?;
        return Ok(MergeResult::Conflict);
    }

    Err(GitError::CommandFailed {
        code: output.status.code().unwrap_or(-1),
        stderr: stderr.trim().to_string(),
    })
}

pub fn merge_abort(repo: &Path) -> GitResult<()> {
    run_git(repo, &["merge", "--abort"])?;
    Ok(())
}

pub fn is_ancestor(repo: &Path, maybe_ancestor: &str, descendant: &str) -> GitResult<bool> {
    let output = run_git_raw(
        repo,
        &["merge-base", "--is-ancestor", maybe_ancestor, descendant],
    )?;
    Ok(output.status.success())
}

// ---------------------------------------------------------------------------
// Remote operations
// ---------------------------------------------------------------------------

pub fn force_push(repo: &Path, branch: &str) -> GitResult<()> {
    run_git(repo, &["push", "--force-with-lease", "origin", branch])?;
    Ok(())
}

pub fn push(repo: &Path, branch: &str) -> GitResult<()> {
    run_git(repo, &["push", "origin", branch])?;
    Ok(())
}

pub fn fetch(repo: &Path) -> GitResult<()> {
    run_git(repo, &["fetch", "origin"])?;
    Ok(())
}

/// Check if origin remote exists.
pub fn has_remote(repo: &Path) -> bool {
    run_git(repo, &["remote", "get-url", "origin"]).is_ok()
}

// ---------------------------------------------------------------------------
// Log / history
// ---------------------------------------------------------------------------

pub fn log_merges(repo: &Path, branch: &str, limit: usize) -> GitResult<Vec<MergeLogEntry>> {
    let limit_str = limit.to_string();
    let output = run_git(
        repo,
        &[
            "log",
            "--merges",
            "--first-parent",
            "--format=%H|||%s",
            "-n",
            &limit_str,
            branch,
        ],
    )?;

    if output.is_empty() {
        return Ok(Vec::new());
    }

    output
        .lines()
        .map(|line| {
            let (sha, subject) = line
                .split_once("|||")
                .ok_or_else(|| GitError::Parse(format!("unexpected merge log format: {line}")))?;
            Ok(MergeLogEntry {
                sha: sha.to_string(),
                subject: subject.to_string(),
            })
        })
        .collect()
}

/// Extract topic branch names from merge commit subjects.
///
/// Parses standard git merge messages of the form `Merge branch 'feature/foo'`
/// and `Merge branch 'feature/foo' into dev`.
pub fn extract_topics_from_merges(merge_subjects: &[String]) -> Vec<String> {
    const PREFIX: &str = "Merge branch '";

    merge_subjects
        .iter()
        .filter_map(|subject| {
            let rest = subject.strip_prefix(PREFIX)?;
            let end = rest.find('\'')?;
            Some(rest[..end].to_string())
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tag / commit
// ---------------------------------------------------------------------------

pub fn tag_create(repo: &Path, name: &str, message: &str) -> GitResult<()> {
    run_git(repo, &["tag", "-a", name, "-m", message])?;
    Ok(())
}

pub fn commit_empty(repo: &Path, message: &str) -> GitResult<String> {
    run_git(repo, &["commit", "--allow-empty", "-m", message])?;
    run_git(repo, &["rev-parse", "HEAD"])
}

// ---------------------------------------------------------------------------
// Repository introspection
// ---------------------------------------------------------------------------

pub fn detect_provider(repo: &Path) -> GitResult<Provider> {
    let url = match get_remote_url(repo) {
        Some(url) => url,
        None => return Ok(Provider::Unknown),
    };
    Ok(parse_provider_url(&url))
}

pub fn get_remote_url(repo: &Path) -> Option<String> {
    run_git(repo, &["remote", "get-url", "origin"]).ok()
}

fn parse_provider_url(url: &str) -> Provider {
    if url.contains("github.com") {
        Provider::GitHub
    } else if url.contains("dev.azure.com") || url.contains("visualstudio.com") {
        Provider::AzureDevOps
    } else if url.contains("bitbucket.org") {
        Provider::Bitbucket
    } else {
        Provider::Unknown
    }
}

pub fn rerere_enable(repo: &Path) -> GitResult<()> {
    run_git(repo, &["config", "rerere.enabled", "true"])?;
    Ok(())
}

pub fn current_branch(repo: &Path) -> GitResult<String> {
    run_git(repo, &["rev-parse", "--abbrev-ref", "HEAD"])
}

pub fn current_sha(repo: &Path) -> GitResult<String> {
    run_git(repo, &["rev-parse", "HEAD"])
}

pub fn checkout(repo: &Path, branch: &str) -> GitResult<()> {
    run_git(repo, &["checkout", branch])?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Merge-tree plumbing (object-level merge, no working-tree mutation)
// Requires Git >= 2.38.
// ---------------------------------------------------------------------------

/// Result of a `git merge-tree --write-tree` operation.
#[derive(Debug, Clone)]
pub enum MergeTreeResult {
    /// Clean merge. `tree_oid` is the resulting tree object.
    Success { tree_oid: String },
    /// Merge has conflicts. `info` contains conflict details from stdout.
    Conflict { info: String },
}

/// Perform an object-level 3-way merge without touching the working tree.
pub fn merge_tree(repo: &Path, ours: &str, theirs: &str) -> GitResult<MergeTreeResult> {
    let output = run_git_raw(repo, &["merge-tree", "--write-tree", ours, theirs])?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if output.status.success() {
        let tree_oid = stdout.lines().next().unwrap_or("").to_string();
        Ok(MergeTreeResult::Success { tree_oid })
    } else if output.status.code() == Some(1) {
        // Exit code 1 = conflicts
        Ok(MergeTreeResult::Conflict { info: stdout })
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(GitError::CommandFailed {
            code: output.status.code().unwrap_or(-1),
            stderr,
        })
    }
}

/// Create a commit object from a tree OID and parent commits.
/// Returns the new commit SHA.
pub fn commit_tree(
    repo: &Path,
    tree_oid: &str,
    parents: &[&str],
    message: &str,
) -> GitResult<String> {
    let mut args = vec!["commit-tree", tree_oid];
    for parent in parents {
        args.push("-p");
        args.push(parent);
    }
    args.push("-m");
    args.push(message);
    run_git(repo, &args)
}

/// Resolve a commitish to its tree OID.
pub fn rev_parse_tree(repo: &Path, commitish: &str) -> GitResult<String> {
    let tree_ref = format!("{commitish}^{{tree}}");
    run_git(repo, &["rev-parse", &tree_ref])
}

/// Update a branch ref to point at a new commit SHA.
pub fn update_ref(repo: &Path, branch: &str, sha: &str) -> GitResult<()> {
    let ref_name = format!("refs/heads/{branch}");
    run_git(repo, &["update-ref", &ref_name, sha])?;
    Ok(())
}

/// Resolve a ref/commitish to its commit SHA.
pub fn resolve_ref(repo: &Path, refspec: &str) -> GitResult<String> {
    run_git(repo, &["rev-parse", refspec])
}

// ---------------------------------------------------------------------------
// Release / tag primitives
// ---------------------------------------------------------------------------

/// Get the latest tag reachable from HEAD via `git describe --tags --abbrev=0`.
/// Returns `None` if no tags exist.
pub fn describe_latest_tag(repo: &Path) -> GitResult<Option<String>> {
    match run_git(repo, &["describe", "--tags", "--abbrev=0"]) {
        Ok(tag) => Ok(Some(tag)),
        Err(GitError::CommandFailed { stderr, .. })
            if stderr.contains("No names found") || stderr.contains("No tags can describe") =>
        {
            Ok(None)
        }
        Err(e) => Err(e),
    }
}

/// Get the latest tag reachable from a specific commitish.
/// Returns `None` if no tags exist.
pub fn describe_latest_tag_from(repo: &Path, commitish: &str) -> GitResult<Option<String>> {
    match run_git(repo, &["describe", "--tags", "--abbrev=0", commitish]) {
        Ok(tag) => Ok(Some(tag)),
        Err(GitError::CommandFailed { stderr, .. })
            if stderr.contains("No names found") || stderr.contains("No tags can describe") =>
        {
            Ok(None)
        }
        Err(e) => Err(e),
    }
}

/// Get commit subjects (one-line) since `since_ref`. If `since_ref` is None,
/// returns all commits on HEAD.
pub fn log_since(repo: &Path, since_ref: Option<&str>, format: &str) -> GitResult<Vec<String>> {
    let format_arg = format!("--format={format}");
    let mut args = vec!["log", &format_arg];
    let range;
    if let Some(r) = since_ref {
        range = format!("{r}..HEAD");
        args.push(&range);
    }
    let output = run_git(repo, &args)?;
    if output.is_empty() {
        return Ok(Vec::new());
    }
    Ok(output.lines().map(|l| l.to_string()).collect())
}

/// Parse conventional commit subjects ("type(scope): description").
/// Breaking = trailing `!` before colon OR `BREAKING CHANGE` in description.
pub fn parse_conventional_commits(subjects: &[String]) -> Vec<crate::types::ConventionalCommit> {
    subjects
        .iter()
        .filter_map(|line| {
            // Format: "sha|||type(scope)!: description" or "sha|||type: description"
            let (sha, rest) = line.split_once("|||")?;
            let colon_pos = rest.find(':')?;
            let prefix = &rest[..colon_pos];
            let description = rest[colon_pos + 1..].trim().to_string();

            let breaking_bang = prefix.ends_with('!');
            let prefix_clean = prefix.trim_end_matches('!');

            let (commit_type, scope) = if let Some(paren_start) = prefix_clean.find('(') {
                let paren_end = prefix_clean.find(')')?;
                let t = prefix_clean[..paren_start].to_string();
                let s = prefix_clean[paren_start + 1..paren_end].to_string();
                (t, Some(s))
            } else {
                (prefix_clean.to_string(), None)
            };

            let breaking = breaking_bang || description.contains("BREAKING CHANGE");

            Some(crate::types::ConventionalCommit {
                commit_type,
                scope,
                breaking,
                description,
                sha: sha.to_string(),
            })
        })
        .collect()
}

/// Get `git diff --stat` between two refs.
pub fn diff_stat(repo: &Path, from_ref: &str, to_ref: &str) -> GitResult<String> {
    run_git(repo, &["diff", "--stat", from_ref, to_ref])
}

/// Update a remote branch to point at a target ref using `--force-with-lease`.
pub fn branch_update_to(repo: &Path, branch: &str, target_ref: &str) -> GitResult<()> {
    let refspec = format!("{target_ref}:refs/heads/{branch}");
    run_git(repo, &["push", "origin", &refspec, "--force-with-lease"])?;
    Ok(())
}

/// Check if there are commits between `base` and `head`.
pub fn has_commits_between(repo: &Path, base: &str, head: &str) -> GitResult<bool> {
    let range = format!("{base}..{head}");
    let output = run_git(repo, &["log", "--oneline", &range])?;
    Ok(!output.is_empty())
}

/// Push a tag to origin.
/// Detect the default branch from the remote HEAD (e.g. "main" or "master").
/// Falls back to "main" if the remote HEAD cannot be resolved.
pub fn detect_default_branch(repo: &Path) -> String {
    // `git symbolic-ref refs/remotes/origin/HEAD` -> "refs/remotes/origin/main"
    match run_git(repo, &["symbolic-ref", "refs/remotes/origin/HEAD"]) {
        Ok(full_ref) => full_ref
            .trim()
            .rsplit('/')
            .next()
            .unwrap_or("main")
            .to_string(),
        Err(_) => {
            // Try `git remote show origin` as fallback (slower, needs network)
            match run_git(repo, &["remote", "show", "origin"]) {
                Ok(output) => output
                    .lines()
                    .find(|l| l.contains("HEAD branch:"))
                    .and_then(|l| l.split(':').nth(1))
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "main".to_string()),
                Err(_) => "main".to_string(),
            }
        }
    }
}

pub fn push_tag(repo: &Path, tag: &str) -> GitResult<()> {
    run_git(repo, &["push", "origin", tag])?;
    Ok(())
}

/// Push multiple branch refs to origin in a single command with force-with-lease.
pub fn push_refs(repo: &Path, branches: &[&str]) -> GitResult<()> {
    if branches.is_empty() {
        return Ok(());
    }
    let mut args = vec!["push", "--force-with-lease", "origin"];
    for branch in branches {
        args.push(branch);
    }
    run_git(repo, &args)?;
    Ok(())
}

/// Delete multiple remote branches in a single push command.
/// Uses the `:refs/heads/branch` refspec to delete.
pub fn delete_remote_refs(repo: &Path, branches: &[&str]) -> GitResult<()> {
    if branches.is_empty() {
        return Ok(());
    }
    let refspecs: Vec<String> = branches
        .iter()
        .map(|b| format!(":refs/heads/{b}"))
        .collect();
    let refspec_strs: Vec<&str> = refspecs.iter().map(|s| s.as_str()).collect();
    let mut args = vec!["push", "origin"];
    args.extend(refspec_strs.iter());
    run_git(repo, &args)?;
    Ok(())
}

/// Create a local branch at a specific commit SHA (no checkout).
/// Uses `--force` to overwrite if the branch already exists.
pub fn create_branch_at_sha(repo: &Path, branch: &str, sha: &str) -> GitResult<()> {
    run_git(repo, &["branch", "--force", branch, sha])?;
    Ok(())
}

pub fn compute_refs_fingerprint(repo: &Path) -> GitResult<String> {
    let output = run_git(repo, &["for-each-ref", "--format=%(objectname)", "--sort=refname", "refs/remotes/origin/"])?;
    use sha2::{Sha256, Digest};
    let hash = Sha256::digest(output.as_bytes());
    Ok(format!("{hash:x}"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_provider_url_github() {
        assert_eq!(
            parse_provider_url("git@github.com:user/repo.git"),
            Provider::GitHub
        );
        assert_eq!(
            parse_provider_url("https://github.com/user/repo.git"),
            Provider::GitHub
        );
    }

    #[test]
    fn test_parse_provider_url_azure() {
        assert_eq!(
            parse_provider_url("https://dev.azure.com/org/project/_git/repo"),
            Provider::AzureDevOps
        );
        assert_eq!(
            parse_provider_url("https://org.visualstudio.com/project/_git/repo"),
            Provider::AzureDevOps
        );
    }

    #[test]
    fn test_parse_provider_url_bitbucket() {
        assert_eq!(
            parse_provider_url("git@bitbucket.org:team/repo.git"),
            Provider::Bitbucket
        );
    }

    #[test]
    fn test_parse_provider_url_unknown() {
        assert_eq!(
            parse_provider_url("https://gitlab.com/user/repo.git"),
            Provider::Unknown
        );
        assert_eq!(parse_provider_url(""), Provider::Unknown);
    }

    #[test]
    fn test_extract_topics_standard() {
        let subjects = vec![
            "Merge branch 'feature/auth'".to_string(),
            "Merge branch 'fix/login' into dev".to_string(),
            "Regular commit message".to_string(),
        ];
        let topics = extract_topics_from_merges(&subjects);
        assert_eq!(topics, vec!["feature/auth", "fix/login"]);
    }

    #[test]
    fn test_extract_topics_empty() {
        let topics = extract_topics_from_merges(&[]);
        assert!(topics.is_empty());
    }

    #[test]
    fn test_extract_topics_no_matches() {
        let subjects = vec![
            "Initial commit".to_string(),
            "Fix typo in README".to_string(),
        ];
        let topics = extract_topics_from_merges(&subjects);
        assert!(topics.is_empty());
    }

    #[test]
    fn test_provider_display() {
        assert_eq!(Provider::GitHub.to_string(), "github");
        assert_eq!(Provider::AzureDevOps.to_string(), "azure");
        assert_eq!(Provider::Bitbucket.to_string(), "bitbucket");
        assert_eq!(Provider::Unknown.to_string(), "unknown");
    }

    #[test]
    fn test_provider_serde_roundtrip() {
        // types::Provider uses rename_all = "camelCase"
        let json = serde_json::to_string(&Provider::GitHub).unwrap();
        assert_eq!(json, "\"gitHub\"");

        let parsed: Provider = serde_json::from_str("\"azureDevOps\"").unwrap();
        assert_eq!(parsed, Provider::AzureDevOps);
    }

    #[test]
    fn test_merge_result_serde() {
        let success = MergeResult::Success {
            sha: "abc123".to_string(),
        };
        let json = serde_json::to_string(&success).unwrap();
        assert!(json.contains("\"status\":\"success\""));
        assert!(json.contains("\"sha\":\"abc123\""));

        let conflict = MergeResult::Conflict;
        let json = serde_json::to_string(&conflict).unwrap();
        assert!(json.contains("\"status\":\"conflict\""));
    }

    #[test]
    fn test_parse_conventional_simple() {
        let subjects = vec!["abc123|||feat: add login".to_string()];
        let commits = parse_conventional_commits(&subjects);
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].commit_type, "feat");
        assert_eq!(commits[0].description, "add login");
        assert_eq!(commits[0].sha, "abc123");
        assert!(commits[0].scope.is_none());
        assert!(!commits[0].breaking);
    }

    #[test]
    fn test_parse_conventional_with_scope() {
        let subjects = vec!["def456|||fix(auth): handle expired tokens".to_string()];
        let commits = parse_conventional_commits(&subjects);
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].commit_type, "fix");
        assert_eq!(commits[0].scope.as_deref(), Some("auth"));
        assert!(!commits[0].breaking);
    }

    #[test]
    fn test_parse_conventional_breaking_bang() {
        let subjects = vec!["aaa111|||feat!: remove old API".to_string()];
        let commits = parse_conventional_commits(&subjects);
        assert_eq!(commits.len(), 1);
        assert!(commits[0].breaking);
    }

    #[test]
    fn test_parse_conventional_breaking_keyword() {
        let subjects = vec!["bbb222|||feat: BREAKING CHANGE drop v1 support".to_string()];
        let commits = parse_conventional_commits(&subjects);
        assert_eq!(commits.len(), 1);
        assert!(commits[0].breaking);
    }

    #[test]
    fn test_parse_conventional_skips_non_conventional() {
        let subjects = vec![
            "ccc333|||feat: valid commit".to_string(),
            "ddd444|||not a conventional commit".to_string(),
        ];
        let commits = parse_conventional_commits(&subjects);
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].commit_type, "feat");
    }

    #[test]
    fn test_log_since_percent_format() {
        use std::process::Command;

        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let repo_path = temp_dir.path();

        Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .status()
            .expect("git init");

        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(repo_path)
            .status()
            .expect("git config email");

        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(repo_path)
            .status()
            .expect("git config name");

        std::fs::write(repo_path.join("test.txt"), "hello").expect("create test file");

        Command::new("git")
            .args(["add", "test.txt"])
            .current_dir(repo_path)
            .status()
            .expect("git add");

        Command::new("git")
            .args(["commit", "-m", "test commit"])
            .current_dir(repo_path)
            .status()
            .expect("git commit");

        let result = log_since(repo_path, None, "%H|||%s").expect("log_since should succeed");

        assert!(!result.is_empty(), "should have at least one commit");
        for line in &result {
            assert!(
                line.contains("|||"),
                "each line should contain '|||' separator, got: {line}"
            );
            let parts: Vec<_> = line.split("|||").collect();
            assert_eq!(parts.len(), 2, "should have sha and subject: {line}");
            assert_eq!(
                parts[1], "test commit",
                "subject should match commit message"
            );
        }
    }
}
