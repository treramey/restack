pub mod azure;
pub mod bitbucket;
pub mod github;
pub mod types;
pub mod workflow;

pub use types::*;

use crate::error::{RestackError, Result};
use crate::types::Provider;

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

pub trait ProviderAdapter: Send + Sync {
    fn provider(&self) -> Provider;
    fn list_prs(&self, state: PrState) -> Result<Vec<PullRequest>>;
    fn get_ci_status(&self, branch_or_sha: &str) -> Result<CiStatusDetail>;
    fn comment_on_pr(&self, pr_number: &str, body: &str) -> Result<()>;
    fn is_available(&self) -> bool;

    fn create_pr(&self, _params: &CreatePrParams) -> Result<PullRequest> {
        Err(RestackError::ProviderNotConfigured)
    }

    fn merge_pr(&self, _params: &MergePrParams) -> Result<MergePrResult> {
        Err(RestackError::ProviderNotConfigured)
    }

    fn set_branch_protection(
        &self,
        _params: &BranchProtectionParams,
    ) -> Result<BranchProtectionResult> {
        Err(RestackError::ProviderNotConfigured)
    }

    fn trigger_pipeline(&self, _params: &TriggerPipelineParams) -> Result<PipelineRunResult> {
        Err(RestackError::ProviderNotConfigured)
    }
}

// ---------------------------------------------------------------------------
// Null adapter (unsupported / unconfigured providers)
// ---------------------------------------------------------------------------

pub struct NullAdapter;

impl ProviderAdapter for NullAdapter {
    fn provider(&self) -> Provider {
        Provider::Unknown
    }

    fn list_prs(&self, _state: PrState) -> Result<Vec<PullRequest>> {
        Err(RestackError::ProviderNotConfigured)
    }

    fn get_ci_status(&self, _branch_or_sha: &str) -> Result<CiStatusDetail> {
        Err(RestackError::ProviderNotConfigured)
    }

    fn comment_on_pr(&self, _pr_number: &str, _body: &str) -> Result<()> {
        Err(RestackError::ProviderNotConfigured)
    }

    fn is_available(&self) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------
// Factory
// ---------------------------------------------------------------------------

pub fn create_adapter(provider: Provider, remote_url: Option<&str>) -> Box<dyn ProviderAdapter> {
    match provider {
        Provider::GitHub => Box::new(github::GitHubAdapter::new(remote_url)),
        Provider::AzureDevOps => Box::new(azure::AzureDevOpsAdapter::new(remote_url)),
        Provider::Bitbucket => Box::new(bitbucket::BitbucketAdapter::new(remote_url)),
        Provider::Unknown => Box::new(NullAdapter),
    }
}
