use crate::error::{RestackError, Result};
use crate::types::Provider;

use super::{CiStatusDetail, PrState, ProviderAdapter, PullRequest};

// ---------------------------------------------------------------------------
// Stub adapter (not yet implemented)
// ---------------------------------------------------------------------------

pub struct BitbucketAdapter;

impl BitbucketAdapter {
    pub fn new(_remote_url: Option<&str>) -> Self {
        Self
    }
}

impl ProviderAdapter for BitbucketAdapter {
    fn provider(&self) -> Provider {
        Provider::Bitbucket
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
