use crate::types::Provider;

use super::types::GeneratedFile;

/// Generate CI workflow files for the given provider.
pub fn generate_workflow_files(provider: Provider) -> Vec<GeneratedFile> {
    match provider {
        Provider::GitHub => generate_github_workflows(),
        Provider::AzureDevOps => generate_azure_pipelines(),
        Provider::Bitbucket | Provider::Unknown => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// GitHub Actions
// ---------------------------------------------------------------------------

fn generate_github_workflows() -> Vec<GeneratedFile> {
    vec![
        GeneratedFile {
            path: ".github/workflows/restack-rebuild.yml".to_string(),
            content: GITHUB_REBUILD_WORKFLOW.to_string(),
        },
        GeneratedFile {
            path: ".github/workflows/restack-ci-status.yml".to_string(),
            content: GITHUB_CI_STATUS_WORKFLOW.to_string(),
        },
    ]
}

const GITHUB_REBUILD_WORKFLOW: &str = r#"name: Restack Rebuild
on:
  push:
    branches: [staging, dev]
  workflow_dispatch:
    inputs:
      environment:
        description: 'Environment to rebuild'
        required: false
        type: string
concurrency:
  group: restack-rebuild-${{ github.ref }}
  cancel-in-progress: true
jobs:
  rebuild:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - name: Install restack
        run: cargo install --path .
      - name: Rebuild
        run: |
          if [ -n "${{ github.event.inputs.environment }}" ]; then
            restack rebuild env "${{ github.event.inputs.environment }}" --json
          else
            restack rebuild all "$(restack repo list --json | jq -r '.[0].id')" --json
          fi
"#;

const GITHUB_CI_STATUS_WORKFLOW: &str = r#"name: Restack CI Status
on:
  check_suite:
    types: [completed]
  workflow_run:
    types: [completed]
concurrency:
  group: restack-ci-status-${{ github.ref }}
  cancel-in-progress: true
jobs:
  update-status:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - name: Install restack
        run: cargo install --path .
      - name: Update CI statuses
        run: restack ci status --repo "$(restack repo list --json | jq -r '.[0].id')" --json
"#;

// ---------------------------------------------------------------------------
// Azure Pipelines
// ---------------------------------------------------------------------------

fn generate_azure_pipelines() -> Vec<GeneratedFile> {
    vec![
        GeneratedFile {
            path: "azure-pipelines-rebuild.yml".to_string(),
            content: AZURE_REBUILD_PIPELINE.to_string(),
        },
        GeneratedFile {
            path: "azure-pipelines-ci-status.yml".to_string(),
            content: AZURE_CI_STATUS_PIPELINE.to_string(),
        },
    ]
}

const AZURE_REBUILD_PIPELINE: &str = r#"trigger:
  branches:
    include:
      - staging
      - dev
pool:
  vmImage: 'ubuntu-latest'
steps:
  - checkout: self
    fetchDepth: 0
  - script: cargo install --path .
    displayName: 'Install restack'
  - script: restack rebuild all "$(restack repo list --json | jq -r '.[0].id')" --json
    displayName: 'Rebuild environments'
"#;

const AZURE_CI_STATUS_PIPELINE: &str = r#"trigger: none
schedules:
  - cron: '*/15 * * * *'
    displayName: 'Poll CI status'
    branches:
      include:
        - staging
        - dev
    always: true
pool:
  vmImage: 'ubuntu-latest'
steps:
  - checkout: self
    fetchDepth: 0
  - script: cargo install --path .
    displayName: 'Install restack'
  - script: restack ci status --repo "$(restack repo list --json | jq -r '.[0].id')" --json
    displayName: 'Update CI statuses'
"#;
