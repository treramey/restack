use crate::error::Result;
use crate::provider::{self, PipelineRunResult, TriggerPipelineParams};
use crate::types::Repo;

pub fn trigger_pipeline(
    repo: &Repo,
    branch: &str,
    pipeline_name: Option<&str>,
) -> Result<PipelineRunResult> {
    let adapter = provider::create_adapter(repo.provider, repo.remote_url.as_deref());
    let params = TriggerPipelineParams {
        branch: branch.to_string(),
        pipeline_name: pipeline_name.map(String::from),
    };
    adapter.trigger_pipeline(&params)
}
