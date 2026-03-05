use std::process::Command;

use serde::Deserialize;

use crate::error::{RestackError, Result};
use crate::types::{CiStatus, Provider};

use super::{
    BranchProtectionParams, BranchProtectionResult, CheckConclusion, CheckRun, CheckStatus,
    CiStatusDetail, CreatePrParams, MergePrParams, MergePrResult, MergeStrategy, PipelineRunResult,
    PrState, ProviderAdapter, PullRequest, TriggerPipelineParams,
};

// ---------------------------------------------------------------------------
// Adapter
// ---------------------------------------------------------------------------

pub struct GitHubAdapter {
    owner: String,
    repo: String,
}

impl GitHubAdapter {
    pub fn new(remote_url: Option<&str>) -> Self {
        let (owner, repo) = remote_url.and_then(parse_github_slug).unwrap_or_default();
        Self { owner, repo }
    }
}

impl ProviderAdapter for GitHubAdapter {
    fn provider(&self) -> Provider {
        Provider::GitHub
    }

    fn list_prs(&self, state: PrState) -> Result<Vec<PullRequest>> {
        let state_flag = match state {
            PrState::Open => "open",
            PrState::Closed => "closed",
            PrState::Merged => "merged",
            PrState::All => "all",
        };
        let repo_flag = format!("{}/{}", self.owner, self.repo);
        let json = run_gh(&[
            "pr",
            "list",
            "--repo",
            &repo_flag,
            "--json",
            "number,title,headRefName,baseRefName,state,url",
            "--state",
            state_flag,
        ])?;
        let raw: Vec<GhPrRow> = serde_json::from_str(&json)?;
        Ok(raw.into_iter().map(PullRequest::from).collect())
    }

    fn get_ci_status(&self, branch_or_sha: &str) -> Result<CiStatusDetail> {
        let api_path = format!(
            "repos/{}/{}/commits/{}/check-runs",
            self.owner, self.repo, branch_or_sha
        );
        let json = run_gh(&["api", &api_path])?;
        let resp: GhCheckRunsResponse = serde_json::from_str(&json)?;

        let checks: Vec<CheckRun> = resp.check_runs.into_iter().map(CheckRun::from).collect();
        let overall = derive_overall_status(&checks);

        Ok(CiStatusDetail {
            branch: branch_or_sha.to_string(),
            sha: None,
            overall,
            checks,
        })
    }

    fn comment_on_pr(&self, pr_number: &str, body: &str) -> Result<()> {
        let repo_flag = format!("{}/{}", self.owner, self.repo);
        run_gh(&[
            "pr", "comment", pr_number, "--repo", &repo_flag, "--body", body,
        ])?;
        Ok(())
    }

    fn create_pr(&self, params: &CreatePrParams) -> Result<PullRequest> {
        let repo_flag = format!("{}/{}", self.owner, self.repo);
        let mut args = vec![
            "pr",
            "create",
            "--head",
            &params.head,
            "--base",
            &params.base,
            "--title",
            &params.title,
            "--repo",
            &repo_flag,
            "--json",
            "number,title,headRefName,baseRefName,state,url",
        ];
        let body_str;
        if let Some(body) = &params.body {
            body_str = body.clone();
            args.extend_from_slice(&["--body", &body_str]);
        }
        if params.draft {
            args.push("--draft");
        }
        let json = run_gh(&args)?;
        let row: GhPrRow = serde_json::from_str(&json)?;
        Ok(PullRequest::from(row))
    }

    fn merge_pr(&self, params: &MergePrParams) -> Result<MergePrResult> {
        let repo_flag = format!("{}/{}", self.owner, self.repo);
        let strategy_flag = match params.strategy {
            MergeStrategy::Merge => "--merge",
            MergeStrategy::Squash => "--squash",
            MergeStrategy::Rebase => "--rebase",
        };
        let mut args = vec![
            "pr",
            "merge",
            &params.pr_number,
            "--repo",
            &repo_flag,
            strategy_flag,
        ];
        if params.delete_branch {
            args.push("--delete-branch");
        }
        let output = run_gh(&args)?;
        Ok(MergePrResult {
            merged: true,
            sha: None,
            message: output,
        })
    }

    fn set_branch_protection(
        &self,
        params: &BranchProtectionParams,
    ) -> Result<BranchProtectionResult> {
        let api_path = format!(
            "repos/{}/{}/branches/{}/protection",
            self.owner, self.repo, params.branch
        );
        let checks: Vec<serde_json::Value> = params
            .required_checks
            .iter()
            .map(|c| serde_json::json!({ "context": c }))
            .collect();
        let payload = serde_json::json!({
            "required_status_checks": if checks.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::json!({
                    "strict": true,
                    "checks": checks,
                })
            },
            "enforce_admins": true,
            "required_pull_request_reviews": if params.require_pr {
                serde_json::json!({
                    "required_approving_review_count": params.min_approvals,
                })
            } else {
                serde_json::Value::Null
            },
            "restrictions": serde_json::Value::Null,
        });
        run_gh_with_stdin(
            &["api", "-X", "PUT", &api_path, "--input", "-"],
            &payload.to_string(),
        )?;
        Ok(BranchProtectionResult {
            branch: params.branch.clone(),
            applied: true,
            message: "Branch protection applied".to_string(),
        })
    }

    fn trigger_pipeline(&self, params: &TriggerPipelineParams) -> Result<PipelineRunResult> {
        let repo_flag = format!("{}/{}", self.owner, self.repo);
        let pipeline = params.pipeline_name.as_deref().unwrap_or("ci.yml");
        run_gh(&[
            "workflow",
            "run",
            pipeline,
            "--ref",
            &params.branch,
            "--repo",
            &repo_flag,
        ])?;
        // Fetch the latest run for this branch
        let json = run_gh(&[
            "run",
            "list",
            "--branch",
            &params.branch,
            "--limit",
            "1",
            "--repo",
            &repo_flag,
            "--json",
            "databaseId,url,status",
        ])?;
        let runs: Vec<GhWorkflowRun> = serde_json::from_str(&json)?;
        match runs.into_iter().next() {
            Some(run) => Ok(PipelineRunResult {
                run_id: run.database_id.to_string(),
                url: run.url,
                status: run.status,
            }),
            None => Ok(PipelineRunResult {
                run_id: String::new(),
                url: None,
                status: "queued".to_string(),
            }),
        }
    }

    fn is_available(&self) -> bool {
        let version_ok = Command::new("gh")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !version_ok {
            return false;
        }
        Command::new("gh")
            .args(["auth", "status"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// CLI helper
// ---------------------------------------------------------------------------

fn run_gh(args: &[&str]) -> Result<String> {
    let output = Command::new("gh").args(args).output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(RestackError::ProviderApiError(stderr));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn run_gh_with_stdin(args: &[&str], stdin_data: &str) -> Result<String> {
    use std::io::Write;
    let mut child = Command::new("gh")
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(stdin_data.as_bytes())?;
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(RestackError::ProviderApiError(stderr));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

// ---------------------------------------------------------------------------
// JSON wire types (gh CLI output shapes)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhPrRow {
    number: u64,
    title: String,
    head_ref_name: String,
    base_ref_name: String,
    state: String,
    url: String,
}

impl From<GhPrRow> for PullRequest {
    fn from(row: GhPrRow) -> Self {
        let state = match row.state.to_lowercase().as_str() {
            "open" => PrState::Open,
            "closed" => PrState::Closed,
            "merged" => PrState::Merged,
            _ => PrState::Open,
        };
        Self {
            number: row.number.to_string(),
            title: row.title,
            head_branch: row.head_ref_name,
            base_branch: row.base_ref_name,
            state,
            url: row.url,
        }
    }
}

#[derive(Deserialize)]
struct GhCheckRunsResponse {
    check_runs: Vec<GhCheckRun>,
}

#[derive(Deserialize)]
struct GhCheckRun {
    name: String,
    status: String,
    conclusion: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
}

impl From<GhCheckRun> for CheckRun {
    fn from(cr: GhCheckRun) -> Self {
        let status = match cr.status.as_str() {
            "queued" => CheckStatus::Queued,
            "in_progress" => CheckStatus::InProgress,
            "completed" => CheckStatus::Completed,
            _ => CheckStatus::Queued,
        };
        let conclusion = cr.conclusion.as_deref().and_then(parse_conclusion);
        Self {
            name: cr.name,
            status,
            conclusion,
            url: cr.html_url,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhWorkflowRun {
    database_id: u64,
    #[serde(default)]
    url: Option<String>,
    status: String,
}

fn parse_conclusion(s: &str) -> Option<CheckConclusion> {
    match s {
        "success" => Some(CheckConclusion::Success),
        "failure" => Some(CheckConclusion::Failure),
        "neutral" => Some(CheckConclusion::Neutral),
        "cancelled" => Some(CheckConclusion::Cancelled),
        "timed_out" => Some(CheckConclusion::TimedOut),
        "action_required" => Some(CheckConclusion::ActionRequired),
        "skipped" => Some(CheckConclusion::Skipped),
        "stale" => Some(CheckConclusion::Stale),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_github_slug(url: &str) -> Option<(String, String)> {
    // SSH: git@github.com:owner/repo.git
    if let Some(rest) = url.strip_prefix("git@github.com:") {
        let rest = rest.strip_suffix(".git").unwrap_or(rest);
        let (owner, repo) = rest.split_once('/')?;
        return Some((owner.to_string(), repo.to_string()));
    }
    // HTTPS: https://github.com/owner/repo.git
    let rest = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))?;
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let (owner, repo) = rest.split_once('/')?;
    Some((owner.to_string(), repo.to_string()))
}

fn derive_overall_status(checks: &[CheckRun]) -> CiStatus {
    if checks.is_empty() {
        return CiStatus::Pending;
    }
    let any_failed = checks.iter().any(|c| {
        c.conclusion
            .map(|con| matches!(con, CheckConclusion::Failure | CheckConclusion::TimedOut))
            .unwrap_or(false)
    });
    if any_failed {
        return CiStatus::Failed;
    }
    let all_success = checks.iter().all(|c| {
        c.status == CheckStatus::Completed
            && c.conclusion
                .map(|con| {
                    matches!(
                        con,
                        CheckConclusion::Success
                            | CheckConclusion::Neutral
                            | CheckConclusion::Skipped
                    )
                })
                .unwrap_or(false)
    });
    if all_success {
        CiStatus::Passed
    } else {
        CiStatus::Pending
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_github_slug_ssh() {
        let result = parse_github_slug("git@github.com:octocat/hello-world.git");
        assert_eq!(
            result,
            Some(("octocat".to_string(), "hello-world".to_string()))
        );
    }

    #[test]
    fn test_parse_github_slug_https() {
        let result = parse_github_slug("https://github.com/octocat/hello-world.git");
        assert_eq!(
            result,
            Some(("octocat".to_string(), "hello-world".to_string()))
        );
    }

    #[test]
    fn test_parse_github_slug_no_dotgit() {
        let result = parse_github_slug("https://github.com/octocat/hello-world");
        assert_eq!(
            result,
            Some(("octocat".to_string(), "hello-world".to_string()))
        );
    }

    #[test]
    fn test_parse_github_slug_invalid() {
        assert_eq!(parse_github_slug("https://gitlab.com/foo/bar"), None);
        assert_eq!(parse_github_slug(""), None);
    }

    #[test]
    fn test_list_prs_json_parsing() {
        let json = r#"[
            {
                "number": 42,
                "title": "Add feature",
                "headRefName": "feature/auth",
                "baseRefName": "main",
                "state": "OPEN",
                "url": "https://github.com/octocat/hello/pull/42"
            }
        ]"#;
        let raw: Vec<GhPrRow> = serde_json::from_str(json).unwrap();
        let prs: Vec<PullRequest> = raw.into_iter().map(PullRequest::from).collect();
        assert_eq!(prs.len(), 1);
        assert_eq!(prs[0].number, "42");
        assert_eq!(prs[0].head_branch, "feature/auth");
        assert!(matches!(prs[0].state, PrState::Open));
    }

    #[test]
    fn test_ci_status_json_parsing() {
        let json = r#"{
            "total_count": 2,
            "check_runs": [
                {
                    "name": "build",
                    "status": "completed",
                    "conclusion": "success",
                    "html_url": "https://github.com/octocat/hello/runs/1"
                },
                {
                    "name": "test",
                    "status": "completed",
                    "conclusion": "failure",
                    "html_url": "https://github.com/octocat/hello/runs/2"
                }
            ]
        }"#;
        let resp: GhCheckRunsResponse = serde_json::from_str(json).unwrap();
        let checks: Vec<CheckRun> = resp.check_runs.into_iter().map(CheckRun::from).collect();
        assert_eq!(checks.len(), 2);
        assert_eq!(checks[0].name, "build");
        assert!(matches!(
            checks[0].conclusion,
            Some(CheckConclusion::Success)
        ));
        assert!(matches!(
            checks[1].conclusion,
            Some(CheckConclusion::Failure)
        ));
    }

    #[test]
    fn test_derive_overall_status_all_pass() {
        let checks = vec![CheckRun {
            name: "build".to_string(),
            status: CheckStatus::Completed,
            conclusion: Some(CheckConclusion::Success),
            url: None,
        }];
        assert_eq!(derive_overall_status(&checks), CiStatus::Passed);
    }

    #[test]
    fn test_derive_overall_status_any_fail() {
        let checks = vec![
            CheckRun {
                name: "build".to_string(),
                status: CheckStatus::Completed,
                conclusion: Some(CheckConclusion::Success),
                url: None,
            },
            CheckRun {
                name: "test".to_string(),
                status: CheckStatus::Completed,
                conclusion: Some(CheckConclusion::Failure),
                url: None,
            },
        ];
        assert_eq!(derive_overall_status(&checks), CiStatus::Failed);
    }

    #[test]
    fn test_derive_overall_status_pending() {
        let checks = vec![CheckRun {
            name: "build".to_string(),
            status: CheckStatus::InProgress,
            conclusion: None,
            url: None,
        }];
        assert_eq!(derive_overall_status(&checks), CiStatus::Pending);
    }

    #[test]
    fn test_derive_overall_status_empty() {
        assert_eq!(derive_overall_status(&[]), CiStatus::Pending);
    }

    #[test]
    fn test_merge_strategy_serde_roundtrip() {
        let strategies = vec![
            MergeStrategy::Merge,
            MergeStrategy::Squash,
            MergeStrategy::Rebase,
        ];
        for s in strategies {
            let json = serde_json::to_string(&s).unwrap();
            let back: MergeStrategy = serde_json::from_str(&json).unwrap();
            assert_eq!(s, back);
        }
    }

    #[test]
    fn test_create_pr_json_parsing() {
        let json = r#"{
            "number": 55,
            "title": "Add auth",
            "headRefName": "feature/auth",
            "baseRefName": "main",
            "state": "OPEN",
            "url": "https://github.com/octocat/hello/pull/55"
        }"#;
        let row: GhPrRow = serde_json::from_str(json).unwrap();
        let pr = PullRequest::from(row);
        assert_eq!(pr.number, "55");
        assert_eq!(pr.head_branch, "feature/auth");
    }

    #[test]
    fn test_workflow_run_json_parsing() {
        let json = r#"[{
            "databaseId": 12345,
            "url": "https://github.com/octocat/hello/actions/runs/12345",
            "status": "queued"
        }]"#;
        let runs: Vec<GhWorkflowRun> = serde_json::from_str(json).unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].database_id, 12345);
        assert_eq!(runs[0].status, "queued");
    }
}
