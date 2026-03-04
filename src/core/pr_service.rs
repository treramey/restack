use crate::error::Result;
use crate::provider::{self, CreatePrParams, MergePrParams, MergePrResult, MergeStrategy, PullRequest};
use crate::types::Repo;

pub fn create_pr(
    repo: &Repo,
    head: &str,
    base: &str,
    title: &str,
    body: Option<&str>,
    draft: bool,
) -> Result<PullRequest> {
    let adapter = provider::create_adapter(repo.provider, repo.remote_url.as_deref());
    let params = CreatePrParams {
        head: head.to_string(),
        base: base.to_string(),
        title: title.to_string(),
        body: body.map(String::from),
        draft,
    };
    adapter.create_pr(&params)
}

pub fn merge_pr(
    repo: &Repo,
    pr_number: &str,
    strategy: MergeStrategy,
    delete_branch: bool,
) -> Result<MergePrResult> {
    let adapter = provider::create_adapter(repo.provider, repo.remote_url.as_deref());
    let params = MergePrParams {
        pr_number: pr_number.to_string(),
        strategy,
        delete_branch,
    };
    adapter.merge_pr(&params)
}
