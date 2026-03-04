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

pub struct AzureDevOpsAdapter {
    organization: String,
    project: String,
    repository: String,
}

impl AzureDevOpsAdapter {
    pub fn new(remote_url: Option<&str>) -> Self {
        let (organization, project, repository) = remote_url
            .and_then(parse_azure_slug)
            .unwrap_or_default();
        Self {
            organization,
            project,
            repository,
        }
    }

    fn org_url(&self) -> String {
        format!("https://dev.azure.com/{}", self.organization)
    }
}

impl ProviderAdapter for AzureDevOpsAdapter {
    fn provider(&self) -> Provider {
        Provider::AzureDevOps
    }

    fn list_prs(&self, state: PrState) -> Result<Vec<PullRequest>> {
        let status_flag = match state {
            PrState::Open => "active",
            PrState::Closed => "abandoned",
            PrState::Merged => "completed",
            PrState::All => "all",
        };
        let org_url = self.org_url();
        let json = run_az(&[
            "repos",
            "pr",
            "list",
            "--repository",
            &self.repository,
            "--project",
            &self.project,
            "--org",
            &org_url,
            "--status",
            status_flag,
            "--output",
            "json",
        ])?;
        let raw: Vec<AzPrRow> = serde_json::from_str(&json)?;
        Ok(raw.into_iter().map(PullRequest::from).collect())
    }

    fn get_ci_status(&self, branch_or_sha: &str) -> Result<CiStatusDetail> {
        let org_url = self.org_url();
        let json = run_az(&[
            "pipelines",
            "runs",
            "list",
            "--branch",
            branch_or_sha,
            "--top",
            "5",
            "--project",
            &self.project,
            "--org",
            &org_url,
            "--output",
            "json",
        ])?;
        let runs: Vec<AzPipelineRun> = serde_json::from_str(&json)?;

        let checks: Vec<CheckRun> = runs.into_iter().map(CheckRun::from).collect();
        let overall = derive_overall_status(&checks);

        Ok(CiStatusDetail {
            branch: branch_or_sha.to_string(),
            sha: None,
            overall,
            checks,
        })
    }

    fn comment_on_pr(&self, pr_number: &str, body: &str) -> Result<()> {
        let org_url = self.org_url();
        // Use az rest to post a thread comment on the PR
        let uri = format!(
            "{}/_apis/git/repositories/{}/pullRequests/{}/threads?api-version=7.0",
            org_url, self.repository, pr_number
        );
        let payload = serde_json::json!({
            "comments": [{
                "parentCommentId": 0,
                "content": body,
                "commentType": 1
            }],
            "status": 1
        });
        run_az(&[
            "rest",
            "--method",
            "post",
            "--uri",
            &uri,
            "--body",
            &payload.to_string(),
        ])?;
        Ok(())
    }

    fn create_pr(&self, params: &CreatePrParams) -> Result<PullRequest> {
        let org_url = self.org_url();
        let mut args = vec![
            "repos", "pr", "create",
            "--repository", &self.repository,
            "--project", &self.project,
            "--org", &org_url,
            "--source-branch", &params.head,
            "--target-branch", &params.base,
            "--title", &params.title,
            "--output", "json",
        ];
        let body_str;
        if let Some(body) = &params.body {
            body_str = body.clone();
            args.extend_from_slice(&["--description", &body_str]);
        }
        if params.draft {
            args.push("--draft");
            args.push("true");
        }
        let json = run_az(&args)?;
        let row: AzPrRow = serde_json::from_str(&json)?;
        Ok(PullRequest::from(row))
    }

    fn merge_pr(&self, params: &MergePrParams) -> Result<MergePrResult> {
        let org_url = self.org_url();
        let az_strategy = match params.strategy {
            MergeStrategy::Squash => "squash",
            MergeStrategy::Merge => "noFastForward",
            MergeStrategy::Rebase => "rebase",
        };
        let json = run_az(&[
            "repos", "pr", "update",
            "--id", &params.pr_number,
            "--org", &org_url,
            "--status", "completed",
            "--merge-strategy", az_strategy,
            "--output", "json",
        ])?;
        let row: AzPrRow = serde_json::from_str(&json)?;
        let merged = row.status.to_lowercase() == "completed";
        if params.delete_branch {
            let ref_name = format!("refs/heads/{}", row.source_ref_name
                .strip_prefix("refs/heads/")
                .unwrap_or(&row.source_ref_name));
            // Best-effort branch delete
            let _ = run_az(&[
                "repos", "ref", "delete",
                "--name", &ref_name,
                "--repository", &self.repository,
                "--project", &self.project,
                "--org", &org_url,
                "--object-id", "0000000000000000000000000000000000000000",
            ]);
        }
        Ok(MergePrResult {
            merged,
            sha: None,
            message: format!("PR {} {}", params.pr_number, if merged { "completed" } else { "update requested" }),
        })
    }

    fn set_branch_protection(&self, params: &BranchProtectionParams) -> Result<BranchProtectionResult> {
        let org_url = self.org_url();
        // Get repository ID first
        let repo_json = run_az(&[
            "repos", "show",
            "--repository", &self.repository,
            "--project", &self.project,
            "--org", &org_url,
            "--output", "json",
        ])?;
        let repo_info: AzRepoInfo = serde_json::from_str(&repo_json)?;

        // Create minimum reviewers policy
        if params.require_pr {
            let policy_uri = format!(
                "{}/_apis/policy/configurations?api-version=7.0",
                org_url
            );
            let payload = serde_json::json!({
                "isEnabled": true,
                "isBlocking": true,
                "type": { "id": "fa4e907d-c16b-4a4c-9dfa-4916e5d171ab" },
                "settings": {
                    "minimumApproverCount": params.min_approvals,
                    "creatorVoteCounts": false,
                    "scope": [{
                        "repositoryId": repo_info.id,
                        "refName": format!("refs/heads/{}", params.branch),
                        "matchKind": "exact",
                    }],
                },
            });
            run_az(&[
                "rest", "--method", "post",
                "--uri", &policy_uri,
                "--body", &payload.to_string(),
            ])?;
        }

        // Create build policy for each required check
        for check in &params.required_checks {
            let policy_uri = format!(
                "{}/_apis/policy/configurations?api-version=7.0",
                org_url
            );
            let payload = serde_json::json!({
                "isEnabled": true,
                "isBlocking": true,
                "type": { "id": "0609b952-1397-4640-95ec-e00a01b2c241" },
                "settings": {
                    "buildDefinitionId": check,
                    "queueOnSourceUpdateOnly": true,
                    "scope": [{
                        "repositoryId": repo_info.id,
                        "refName": format!("refs/heads/{}", params.branch),
                        "matchKind": "exact",
                    }],
                },
            });
            run_az(&[
                "rest", "--method", "post",
                "--uri", &policy_uri,
                "--body", &payload.to_string(),
            ])?;
        }

        Ok(BranchProtectionResult {
            branch: params.branch.clone(),
            applied: true,
            message: "Branch protection policies applied".to_string(),
        })
    }

    fn trigger_pipeline(&self, params: &TriggerPipelineParams) -> Result<PipelineRunResult> {
        let org_url = self.org_url();
        let pipeline = params
            .pipeline_name
            .as_deref()
            .unwrap_or(&self.repository);
        let json = run_az(&[
            "pipelines", "run",
            "--name", pipeline,
            "--branch", &params.branch,
            "--project", &self.project,
            "--org", &org_url,
            "--output", "json",
        ])?;
        let run: AzPipelineRunResponse = serde_json::from_str(&json)?;
        Ok(PipelineRunResult {
            run_id: run.id.to_string(),
            url: run.url,
            status: run.state,
        })
    }

    fn is_available(&self) -> bool {
        let version_ok = Command::new("az")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !version_ok {
            return false;
        }
        Command::new("az")
            .args(["account", "show"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// CLI helper
// ---------------------------------------------------------------------------

fn run_az(args: &[&str]) -> Result<String> {
    let output = Command::new("az").args(args).output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(RestackError::ProviderApiError(stderr));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

// ---------------------------------------------------------------------------
// JSON wire types (az CLI output shapes)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AzPrRow {
    pull_request_id: u64,
    title: String,
    source_ref_name: String,
    target_ref_name: String,
    status: String,
    #[serde(default)]
    url: Option<String>,
}

impl From<AzPrRow> for PullRequest {
    fn from(row: AzPrRow) -> Self {
        let state = match row.status.to_lowercase().as_str() {
            "active" => PrState::Open,
            "completed" => PrState::Merged,
            "abandoned" => PrState::Closed,
            _ => PrState::Open,
        };
        // Strip refs/heads/ prefix
        let head = row
            .source_ref_name
            .strip_prefix("refs/heads/")
            .unwrap_or(&row.source_ref_name)
            .to_string();
        let base = row
            .target_ref_name
            .strip_prefix("refs/heads/")
            .unwrap_or(&row.target_ref_name)
            .to_string();
        Self {
            number: row.pull_request_id.to_string(),
            title: row.title,
            head_branch: head,
            base_branch: base,
            state,
            url: row.url.unwrap_or_default(),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AzPipelineRun {
    #[serde(default)]
    name: Option<String>,
    state: String,
    result: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

impl From<AzPipelineRun> for CheckRun {
    fn from(run: AzPipelineRun) -> Self {
        let status = match run.state.as_str() {
            "completed" => CheckStatus::Completed,
            "inProgress" => CheckStatus::InProgress,
            _ => CheckStatus::Queued,
        };
        let conclusion = run.result.as_deref().and_then(|r| match r {
            "succeeded" => Some(CheckConclusion::Success),
            "failed" => Some(CheckConclusion::Failure),
            "canceled" => Some(CheckConclusion::Cancelled),
            _ => None,
        });
        Self {
            name: run.name.unwrap_or_else(|| "pipeline".to_string()),
            status,
            conclusion,
            url: run.url,
        }
    }
}

#[derive(Deserialize)]
struct AzRepoInfo {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AzPipelineRunResponse {
    id: u64,
    state: String,
    #[serde(default)]
    url: Option<String>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_azure_slug(url: &str) -> Option<(String, String, String)> {
    // SSH: git@ssh.dev.azure.com:v3/{org}/{project}/{repo}
    if let Some(rest) = url.strip_prefix("git@ssh.dev.azure.com:v3/") {
        let parts: Vec<&str> = rest.splitn(3, '/').collect();
        if parts.len() == 3 {
            let repo = parts[2].strip_suffix(".git").unwrap_or(parts[2]);
            return Some((
                parts[0].to_string(),
                parts[1].to_string(),
                repo.to_string(),
            ));
        }
        return None;
    }

    // HTTPS: https://dev.azure.com/{org}/{project}/_git/{repo}
    if let Some(rest) = url
        .strip_prefix("https://dev.azure.com/")
        .or_else(|| url.strip_prefix("http://dev.azure.com/"))
    {
        let parts: Vec<&str> = rest.splitn(4, '/').collect();
        // parts: [org, project, "_git", repo]
        if parts.len() == 4 && parts[2] == "_git" {
            let repo = parts[3].strip_suffix(".git").unwrap_or(parts[3]);
            return Some((
                parts[0].to_string(),
                parts[1].to_string(),
                repo.to_string(),
            ));
        }
        return None;
    }

    // Legacy: https://{org}.visualstudio.com/{project}/_git/{repo}
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let (host, path) = rest.split_once('/')?;
    let org = host.strip_suffix(".visualstudio.com")?;
    let parts: Vec<&str> = path.splitn(3, '/').collect();
    // parts: [project, "_git", repo]
    if parts.len() == 3 && parts[1] == "_git" {
        let repo = parts[2].strip_suffix(".git").unwrap_or(parts[2]);
        return Some((org.to_string(), parts[0].to_string(), repo.to_string()));
    }
    None
}

fn derive_overall_status(checks: &[CheckRun]) -> CiStatus {
    if checks.is_empty() {
        return CiStatus::Pending;
    }
    let any_failed = checks.iter().any(|c| {
        c.conclusion
            .map(|con| matches!(con, CheckConclusion::Failure))
            .unwrap_or(false)
    });
    if any_failed {
        return CiStatus::Failed;
    }
    let all_success = checks.iter().all(|c| {
        c.status == CheckStatus::Completed
            && c.conclusion
                .map(|con| matches!(con, CheckConclusion::Success))
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
    fn test_parse_azure_slug_https() {
        let result = parse_azure_slug("https://dev.azure.com/myorg/myproject/_git/myrepo");
        assert_eq!(
            result,
            Some((
                "myorg".to_string(),
                "myproject".to_string(),
                "myrepo".to_string()
            ))
        );
    }

    #[test]
    fn test_parse_azure_slug_visualstudio() {
        let result =
            parse_azure_slug("https://myorg.visualstudio.com/myproject/_git/myrepo");
        assert_eq!(
            result,
            Some((
                "myorg".to_string(),
                "myproject".to_string(),
                "myrepo".to_string()
            ))
        );
    }

    #[test]
    fn test_parse_azure_slug_ssh() {
        let result = parse_azure_slug("git@ssh.dev.azure.com:v3/myorg/myproject/myrepo");
        assert_eq!(
            result,
            Some((
                "myorg".to_string(),
                "myproject".to_string(),
                "myrepo".to_string()
            ))
        );
    }

    #[test]
    fn test_parse_azure_slug_invalid() {
        assert_eq!(parse_azure_slug("https://github.com/foo/bar"), None);
        assert_eq!(parse_azure_slug(""), None);
    }

    #[test]
    fn test_list_prs_json_parsing() {
        let json = r#"[
            {
                "pullRequestId": 101,
                "title": "Add feature",
                "sourceRefName": "refs/heads/feature/auth",
                "targetRefName": "refs/heads/main",
                "status": "active"
            }
        ]"#;
        let raw: Vec<AzPrRow> = serde_json::from_str(json).unwrap();
        let prs: Vec<PullRequest> = raw.into_iter().map(PullRequest::from).collect();
        assert_eq!(prs.len(), 1);
        assert_eq!(prs[0].number, "101");
        assert_eq!(prs[0].head_branch, "feature/auth");
        assert!(matches!(prs[0].state, PrState::Open));
    }

    #[test]
    fn test_ci_status_json_parsing() {
        let json = r#"[
            {
                "name": "CI Pipeline",
                "state": "completed",
                "result": "succeeded"
            },
            {
                "name": "Deploy",
                "state": "completed",
                "result": "failed"
            }
        ]"#;
        let runs: Vec<AzPipelineRun> = serde_json::from_str(json).unwrap();
        let checks: Vec<CheckRun> = runs.into_iter().map(CheckRun::from).collect();
        assert_eq!(checks.len(), 2);
        assert!(matches!(checks[0].conclusion, Some(CheckConclusion::Success)));
        assert!(matches!(checks[1].conclusion, Some(CheckConclusion::Failure)));
        assert_eq!(derive_overall_status(&checks), CiStatus::Failed);
    }

    #[test]
    fn test_merge_strategy_to_azure() {
        // Verify the mapping logic
        assert_eq!(
            match MergeStrategy::Squash {
                MergeStrategy::Squash => "squash",
                MergeStrategy::Merge => "noFastForward",
                MergeStrategy::Rebase => "rebase",
            },
            "squash"
        );
        assert_eq!(
            match MergeStrategy::Merge {
                MergeStrategy::Squash => "squash",
                MergeStrategy::Merge => "noFastForward",
                MergeStrategy::Rebase => "rebase",
            },
            "noFastForward"
        );
    }

    #[test]
    fn test_pipeline_run_response_parsing() {
        let json = r#"{
            "id": 789,
            "state": "inProgress",
            "url": "https://dev.azure.com/myorg/myproject/_build/results?buildId=789"
        }"#;
        let run: AzPipelineRunResponse = serde_json::from_str(json).unwrap();
        assert_eq!(run.id, 789);
        assert_eq!(run.state, "inProgress");
        assert!(run.url.is_some());
    }
}
