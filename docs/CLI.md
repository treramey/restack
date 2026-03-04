# CLI Reference

Complete reference for the `restack` command-line tool.

## Global Options

```bash
restack --version              # Show version
restack --help                 # Show help
restack --json <command>       # JSON output mode
restack --db <path> <command>  # Custom database path
restack --no-color <command>   # Disable colored output
restack --dry-run <command>    # Preview without side-effects
```

## Workspace Setup

### `restack init`

Initialize a restack workspace in the current directory.

```bash
restack init
```

Creates `.restack/workspace.db` (SQLite) and `.restack/config.toml`.

## Repository Management

### `restack repo add`

Add a git repository to the workspace.

```bash
restack repo add <PATH> [--name NAME]
```

**Arguments:**
- `PATH` (required): Path to the git repository
- `--name`: Override display name (defaults to directory name)

Detects the default branch from the remote HEAD and creates default environments (staging, dev).

**Example:**
```bash
restack repo add ./api --name backend-api
```

### `restack repo remove`

Remove a repository from the workspace.

```bash
restack repo remove <ID>
```

**Arguments:**
- `ID` (required): Repo ID or name

### `restack repo list`

List all tracked repositories.

```bash
restack repo list
```

### `restack repo detect`

Auto-discover git repos in workspace subdirectories (1-2 levels deep).

```bash
restack repo detect
```

Walks subdirectories, filters already-tracked repos, detects provider from remote URL, and adds new repos with default environments.

## Topic Branch Tracking

### `restack topic track`

Start tracking a branch as a topic.

```bash
restack topic track <BRANCH> --repo <REPO_ID>
```

**Arguments:**
- `BRANCH` (required): Git branch name
- `--repo` (required): Repo ID

**Example:**
```bash
restack topic track feature/login --repo repo_01JQAZ...
```

### `restack topic untrack`

Stop tracking a topic.

```bash
restack topic untrack <ID> --repo <REPO_ID>
```

**Arguments:**
- `ID` (required): Topic ID or branch name
- `--repo` (required): Repo ID

### `restack topic list`

List tracked topics.

```bash
restack topic list [--repo REPO_ID] [--all-repos]
```

**Arguments:**
- `--repo`: Filter by repo ID
- `--all-repos`: List topics across all tracked repos (grouped by repo)

### `restack topic status`

Show topic details and environment membership.

```bash
restack topic status <ID> --repo <REPO_ID>
```

**Arguments:**
- `ID` (required): Topic ID or branch name
- `--repo` (required): Repo ID

### `restack topic sync`

Import topics from provider pull requests.

```bash
restack topic sync --repo <REPO_ID>
```

**Arguments:**
- `--repo` (required): Repo ID

Requires provider configuration (GitHub or Azure DevOps).

## Environment Management

### `restack env add`

Create an environment.

```bash
restack env add <NAME> \
  --branch <BRANCH> \
  --repo <REPO_ID> \
  [--ordinal N] \
  [--auto-promote]
```

**Arguments:**
- `NAME` (required): Environment name (e.g., "staging", "dev")
- `--branch` (required): Git branch name for this environment
- `--repo` (required): Repo ID
- `--ordinal`: Sort order (lower = rebuilt first, default: 0)
- `--auto-promote`: Auto-promote topics when CI passes

**Example:**
```bash
restack env add production --branch master --repo repo_01JQAZ... --ordinal 2
```

### `restack env list`

List environments.

```bash
restack env list [--repo REPO_ID] [--all-repos]
```

**Arguments:**
- `--repo`: Filter by repo ID
- `--all-repos`: List environments across all tracked repos (grouped by repo)

### `restack env status`

Show environment details (topics, last rebuild).

```bash
restack env status <ENV_ID>
```

**Arguments:**
- `ENV_ID` (required): Environment ID

## Promotion

### `restack promote to`

Add a topic to an environment.

```bash
restack promote to <TOPIC> <ENV> --repo <REPO_ID> [--dry-run]
```

**Arguments:**
- `TOPIC` (required): Topic ID or branch name
- `ENV` (required): Target environment name
- `--repo` (required): Repo ID
- `--dry-run`: Preview without making changes

**Example:**
```bash
restack promote to feature/login dev --repo repo_01JQAZ...
```

### `restack promote from`

Remove a topic from an environment.

```bash
restack promote from <TOPIC> <ENV> --repo <REPO_ID> [--dry-run]
```

**Arguments:**
- `TOPIC` (required): Topic ID or branch name
- `ENV` (required): Environment name to remove from
- `--repo` (required): Repo ID
- `--dry-run`: Preview without making changes

### `restack promote auto`

Auto-promote CI-passed topics to environments with `auto_promote` enabled.

```bash
restack promote auto
```

Checks CI status for all topics across all repos. Promotes topics with `ci_status == passed` into any `auto_promote` environment they're not already in. Triggers rebuilds for changed environments.

**Output:**
```json
{
  "refreshedTopics": 12,
  "promoted": [
    { "topic": "feature/login", "env": "dev", "repo": "api" }
  ],
  "envsChanged": ["dev"]
}
```

## Rebuild

### `restack rebuild env`

Rebuild a single environment.

```bash
restack rebuild env <ENV_ID> [--dry-run] [--interactive]
```

**Arguments:**
- `ENV_ID` (required): Environment ID
- `--dry-run`: Preview without pushing
- `-i, --interactive`: Prompt on conflicts instead of auto-skipping

**Rebuild algorithm:**
1. Fetch latest from remote
2. Reset environment branch to `origin/<base_branch>`
3. Phase 1: merge topics also in the upstream environment (e.g., staging topics into dev)
4. Insert marker commit (`### Match '<upstream_env>'`)
5. Phase 2: merge topics only in this environment
6. Force-push with lease safety

**Interactive mode** (`-i`): On conflict, prompts with four choices:
- Skip topic (remove from environment)
- Open in `$EDITOR`
- Retry merge (after manual resolution)
- Abort rebuild

### `restack rebuild all`

Rebuild all environments for a repo.

```bash
restack rebuild all [REPO_ID] [--dry-run] [--interactive] [--all-repos]
```

**Arguments:**
- `REPO_ID`: Repo ID (required unless `--all-repos`)
- `--dry-run`: Preview without pushing
- `-i, --interactive`: Prompt on conflicts
- `--all-repos`: Rebuild all environments across all tracked repos

### `restack rebuild watch`

Polling mode: periodically run `promote auto` and rebuild changed environments.

```bash
restack rebuild watch [--interval SECONDS]
```

**Arguments:**
- `--interval`: Poll interval in seconds (default: 60)

Runs until Ctrl+C (graceful shutdown). Returns summary on exit:
```json
{
  "cycles": 15,
  "totalPromoted": 3,
  "stopped": "graceful"
}
```

## Release Management

### `restack release prepare`

Preview the next release (version bump + changelog from conventional commits).

```bash
restack release prepare [--bump TYPE]
```

**Arguments:**
- `--bump`: Override bump type (`major`, `minor`, `patch`). Auto-detected from commits if omitted.

### `restack release cut`

Cut a release: tag, push, preserve maint branch, graduate merged topics.

```bash
restack release cut [--bump TYPE]
```

**Arguments:**
- `--bump`: Override bump type

Sequence: prepare → tag → push tag → update `maint` → preserve as `maint-X.Y` → graduate topics → rebuild environments.

### `restack release hotfix`

Create a hotfix branch from maint.

```bash
restack release hotfix [--base BRANCH]
```

**Arguments:**
- `--base`: Maint branch to hotfix from (default: `maint`)

### `restack release hotfix-release`

Release a hotfix: patch bump, tag, push, merge maint to master.

```bash
restack release hotfix-release [--base BRANCH]
```

**Arguments:**
- `--base`: Maint branch (default: `maint`)

## CI / Pipeline

### `restack ci status`

Show CI status for all topics in a repo.

```bash
restack ci status --repo <REPO_ID>
```

### `restack ci generate`

Generate CI workflow files for the repo's provider.

```bash
restack ci generate --repo <REPO_ID> [--stdout] [-o DIR]
```

**Arguments:**
- `--repo` (required): Repo ID
- `--stdout`: Print to stdout instead of writing files
- `-o, --output`: Output directory (default: current directory)

### `restack pipeline trigger`

Trigger a CI pipeline.

```bash
restack pipeline trigger --repo <REPO_ID> --branch <BRANCH> [--name WORKFLOW]
```

**Arguments:**
- `--repo` (required): Repo ID
- `--branch` (required): Branch to build
- `--name`: Pipeline/workflow name (optional)

## Pull Requests

### `restack pr create`

Create a pull request via the provider.

```bash
restack pr create \
  --repo <REPO_ID> \
  --head <BRANCH> \
  --base <BRANCH> \
  --title "Title" \
  [--body "Description"] \
  [--draft]
```

**Arguments:**
- `--repo` (required): Repo ID
- `--head` (required): Source branch
- `--base` (required): Target branch
- `--title` (required): PR title
- `--body`: PR description
- `--draft`: Create as draft PR

### `restack pr merge`

Merge a pull request.

```bash
restack pr merge <PR_NUMBER> \
  --repo <REPO_ID> \
  [--strategy merge|squash|rebase] \
  [--delete-branch]
```

**Arguments:**
- `PR_NUMBER` (required): PR number
- `--repo` (required): Repo ID
- `--strategy`: Merge strategy (default: `squash`)
- `--delete-branch`: Delete source branch after merge

## Branch Protection

### `restack protection set`

Set branch protection rules.

```bash
restack protection set \
  --repo <REPO_ID> \
  --branch <BRANCH> \
  [--checks CHECK1,CHECK2] \
  [--require-pr] \
  [--min-approvals N]
```

**Arguments:**
- `--repo` (required): Repo ID
- `--branch` (required): Branch to protect
- `--checks`: Required CI checks (comma-separated)
- `--require-pr`: Require PR reviews
- `--min-approvals`: Minimum approvals (default: 1)

### `restack protection envs`

Apply protection rules to all environment branches for a repo.

```bash
restack protection envs --repo <REPO_ID>
```

## Shell Completions

```bash
restack completions bash > ~/.bash_completions.d/restack
restack completions zsh > ~/.zsh_completions.d/_restack
restack completions fish > ~/.config/fish/completions/restack.fish
```

## UI Server

```bash
restack ui [--port PORT]
```

**Arguments:**
- `-p, --port`: HTTP port (default: 6969)

Spawns the Node.js host server and serves the React UI. Requires `npm run build` first.

## MCP Server

The host layer also supports MCP (Model Context Protocol) for AI agent integration:

```bash
cd host && npx tsx src/index.ts mcp
```

Exposes all restack commands as MCP tools over stdio. See `host/src/mcp.ts` for tool definitions.

## Environment Variables

| Variable | Description |
|----------|-------------|
| `RESTACK_DB_PATH` | Override database location |
| `NO_COLOR` | Disable colored output |
| `EDITOR` | Editor for interactive conflict resolution |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Error (details in stderr or JSON `error` field) |
