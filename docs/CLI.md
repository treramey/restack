# CLI Reference

Complete reference for the `restack` command-line tool.

## Global Options

```bash
restack --version              # Show version
restack --help                 # Show help
restack --json <command>       # JSON output mode
restack --db <path> <command>  # Custom database path
restack --no-color <command>   # Disable colored output
```

## Workspace Setup

### `restack init`

Initialize a restack workspace in the current directory. Automatically:
1. Registers the current git repo
2. Discovers all local and remote branches (excluding environment branches)
3. Creates default environments (staging, dev)

```bash
restack init
```

Creates `.restack/workspace.db` (SQLite) and `.restack/config.toml`.

**Config: Branch Discovery**

Exclude patterns are configurable in `.restack/config.toml`:

```toml
[discovery]
exclude_patterns = ["main", "master", "staging", "dev", "production", "maint", "maint-*"]
```

### `restack refresh`

Fetch origin, discover new branches, sync CI status, and cleanup stale topics.

```bash
restack refresh [--repo REPO_ID]
```

**Arguments:**
- `--repo`: Refresh a specific repo (defaults to all repos)

**Actions:**
1. `git fetch origin` for all repos
2. Discover new branches (respecting exclusion patterns)
3. Archive topics whose branches no longer exist
4. Sync topic CI status from provider (if configured)
5. Sync environment CI status (checks env branch CI, runs blame on failure)
6. Auto-promote CI-passed topics to `auto_promote` environments (gated by env CI status)

## Repository Management

### `restack repo list`

List all tracked repositories.

```bash
restack repo list
```

### `restack repo remove`

Remove a repository from the workspace.

```bash
restack repo remove <ID>
```

**Arguments:**
- `ID` (required): Repo ID or name

## Topic Branch Tracking

### `restack topic track`

Start tracking a branch as a topic. Use this for power-user cases when you need to track a branch that was excluded by discovery patterns.

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

### `restack topic archive`

Archive a topic (hide from board, mark as closed).

```bash
restack topic archive <ID> --repo <REPO_ID>
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
restack env list [--repo REPO_ID]
```

**Arguments:**
- `--repo`: Filter by repo ID

### `restack env status`

Show environment details (topics, last rebuild).

```bash
restack env status <ENV_ID>
```

**Arguments:**
- `ENV_ID` (required): Environment ID

### `restack env init`

Initialize integration environments from config or interactively.

```bash
restack env init [--repo REPO_ID] [--interactive] [--push]
```

**Arguments:**
- `--repo`: Repo ID (auto-resolved if single repo in workspace)
- `-i, --interactive`: Select branches from local/remote interactively
- `--push`: Push newly created branches to remote

**Example:**
```bash
# From config (reads .restack/config.toml [environments] section)
restack env init

# Interactive mode: pick branches, set names/ordinals
restack env init --interactive --push
```

### `restack env ci-override`

Override CI status for an environment. Clears a failed/pending CI state to unblock auto-promotion.

```bash
restack env ci-override <ENV_NAME> [--repo REPO_ID]
```

**Arguments:**
- `ENV_NAME` (required): Environment name
- `--repo`: Repo ID (auto-resolved if single repo in workspace)

**Effect:** Sets `ci_override = passed` on the latest rebuild, resets the environment's `ci_status` to `None`. Auto-promotion resumes immediately.

### `restack env blame`

Identify the topic most likely responsible for a CI failure on an environment.

```bash
restack env blame <ENV_NAME> [--repo REPO_ID]
```

**Arguments:**
- `ENV_NAME` (required): Environment name
- `--repo`: Repo ID (auto-resolved if single repo in workspace)

**Output:** JSON with blame result. Uses speculative blame (exact, from per-step CI) when available, falls back to differential blame (comparing green vs red rebuild topic sets).

**Example output:**
```json
{
  "speculative": {
    "envId": "env_...",
    "envName": "staging",
    "rebuildId": "rebuild_...",
    "breakpointStep": 3,
    "culpritTopicId": "topic_...",
    "culpritBranch": "feature/auth",
    "stepsChecked": 5,
    "stepsPassed": 3,
    "stepsFailed": 2,
    "stepsPending": 0
  }
}
```

### `restack env speculative-status`

Show speculative CI status for each merge step of the latest rebuild.

```bash
restack env speculative-status <ENV_NAME> [--repo REPO_ID]
```

**Arguments:**
- `ENV_NAME` (required): Environment name
- `--repo`: Repo ID (auto-resolved if single repo in workspace)

**Output:** JSON with per-step CI statuses. Each step represents a cumulative merge (base + topics 0..N). A transition from `passed` to `failed` between steps identifies the exact culprit.

## Promotion

### `restack promote to`

Add a topic to an environment and trigger rebuild.

```bash
restack promote to <TOPIC> <ENV> --repo <REPO_ID>
```

**Arguments:**
- `TOPIC` (required): Topic ID or branch name
- `ENV` (required): Target environment name
- `--repo` (required): Repo ID

**Example:**
```bash
restack promote to feature/login dev --repo repo_01JQAZ...
```

**Conflict Handling:**
If the topic causes a merge conflict during rebuild, it's automatically removed from the environment and returned to the "Unassigned" lane in the UI. A toast notification alerts the user.

### `restack promote from`

Remove a topic from an environment and trigger rebuild.

```bash
restack promote from <TOPIC> <ENV> --repo <REPO_ID>
```

**Arguments:**
- `TOPIC` (required): Topic ID or branch name
- `ENV` (required): Environment name to remove from
- `--repo` (required): Repo ID

## Rebuild

### `restack rebuild env`

Rebuild a single environment.

```bash
restack rebuild env <ENV_ID> [--interactive]
```

**Arguments:**
- `ENV_ID` (required): Environment ID
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
restack rebuild all <REPO_ID>
```

**Arguments:**
- `REPO_ID`: Repo ID

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