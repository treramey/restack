# Restack Architecture

High-level overview of restack's design, state model, and rebuild algorithm.

## Layer Model

```
┌─────────────────────────────────────────────────────┐
│                  UI Layer (React SPA)                │
│  Kanban (env lanes) │ Canvas (multi-repo) │ List    │
├─────────────────────────────────────────────────────┤
│              Host Layer (Node.js + Hono)             │
│  REST API routes │ CLI bridge │ Static file serving  │
├─────────────────────────────────────────────────────┤
│            CLI Layer (Rust binary)                   │
│  Commands │ Git operations │ State management        │
├─────────────────────────────────────────────────────┤
│          Storage Layer (SQLite)                      │
│  Workspace.db │ TOML config │ Git repository         │
└─────────────────────────────────────────────────────┘
```

## State Model

Restack tracks all state in **SQLite**. No dependency on provider APIs or PR labels.

### Database Schema

```sql
-- Repositories in workspace
CREATE TABLE repos (
    id TEXT PRIMARY KEY,                -- ULID, prefixed "repo_"
    name TEXT NOT NULL,
    path TEXT NOT NULL UNIQUE,          -- relative from workspace root
    remote_url TEXT,
    provider TEXT,                      -- 'github' | 'azure' | 'bitbucket'
    base_branch TEXT DEFAULT 'master',
    created_at TEXT NOT NULL
);

-- Environments per repo (dev, staging, production)
CREATE TABLE environments (
    id TEXT PRIMARY KEY,                -- ULID, prefixed "env_"
    repo_id TEXT NOT NULL,
    name TEXT NOT NULL,                 -- 'dev', 'staging', 'production'
    branch TEXT NOT NULL,               -- git branch name
    ordinal INTEGER NOT NULL,           -- 0=dev, 1=staging, 2=production
    auto_promote BOOLEAN DEFAULT false,
    UNIQUE(repo_id, name)
);

-- Topic branches (feature branches / PRs)
CREATE TABLE topics (
    id TEXT PRIMARY KEY,                -- ULID, prefixed "topic_"
    repo_id TEXT NOT NULL,
    branch TEXT NOT NULL,               -- branch name
    pr_id TEXT,                         -- provider PR ID if exists
    pr_url TEXT,
    status TEXT NOT NULL,               -- 'active' | 'conflict' | 'graduated' | 'closed'
    ci_status TEXT,                     -- 'pending' | 'passed' | 'failed'
    created_at TEXT NOT NULL,
    UNIQUE(repo_id, branch)
);

-- Topic environment membership (which topics are in which envs)
CREATE TABLE topic_environments (
    topic_id TEXT NOT NULL,
    env_id TEXT NOT NULL,
    added_at TEXT NOT NULL,
    PRIMARY KEY (topic_id, env_id)
);

-- Rebuild history
CREATE TABLE rebuilds (
    id TEXT PRIMARY KEY,
    env_id TEXT NOT NULL,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    status TEXT NOT NULL,               -- 'running' | 'success' | 'partial' | 'failed'
    topics_merged INTEGER,
    topics_conflicted INTEGER,
    result_sha TEXT
);

-- Conflict log
CREATE TABLE conflicts (
    id TEXT PRIMARY KEY,
    rebuild_id TEXT NOT NULL,
    topic_id TEXT NOT NULL,
    conflicted_with TEXT,               -- topic name that caused conflict
    resolved BOOLEAN DEFAULT false,
    created_at TEXT NOT NULL
);
```

### Key Properties

1. **Workspace-scoped**: One database per workspace, tracks all repos
2. **Topic-centric**: Topics (feature branches) are first-class entities
3. **Environment membership**: Many-to-many: topics can be in multiple environments
4. **Immutable rebuilds**: Each rebuild is logged with outcomes
5. **No provider dependency**: Pure git operations, optional provider sync

## Rebuild Algorithm

The rebuild is restack's core operation. It's **idempotent** and **conflict-safe**.

### Two-Phase Merge Strategy

```
dev branch rebuild:
  Reset dev to master
  │
  ├─ Phase 1 (Staging topics)
  │  ├─ Merge topic-A from staging
  │  ├─ Merge topic-B from staging
  │  └─ [skip already-merged topics]
  │
  ├─ Marker commit: "### Match 'staging'"
  │  (Separates staging topics from dev-only)
  │
  └─ Phase 2 (Dev-only topics)
     ├─ Merge topic-C (dev-only)
     ├─ Merge topic-D (dev-only)
     └─ [skip conflicting topics]
  │
  Force-push origin/dev
```

### Algorithm Details

**Input:**
- Environment to rebuild (e.g., `dev`)
- Topics assigned to that environment
- Topics in next-upstream environment (e.g., `staging` topics)

**Process:**

1. **Reset**: `git checkout -B dev origin/master`
   - Start fresh, discard old dev history

2. **Phase 1 - Merge upstream topics**:
   ```bash
   for topic in staging_topics:
     if topic exists in remote:
       if not ancestor(topic, master):
         git merge --no-ff --no-edit origin/topic
   ```
   - Ensures dev has all staging topics
   - Skips topics already merged to master (graduated)
   - Records conflicts, continues on next topic

3. **Marker commit**:
   ```bash
   git commit --allow-empty -m "### Match 'staging'"
   ```
   - Visual boundary in git log
   - Helps developers identify phase boundary

4. **Phase 2 - Merge dev-only topics**:
   ```bash
   for topic in dev_topics:
     if not in staging_topics:
       if topic exists in remote:
         if not ancestor(topic, master):
           if not ancestor(topic, HEAD):  # Not already merged in Phase 1
             git merge --no-ff --no-edit origin/topic
   ```
   - Merge topics exclusive to dev (not in staging)
   - Skips topics already merged in Phase 1
   - Conflict topics are removed from environment state

5. **Force-push**:
   ```bash
   git push origin dev --force-with-lease
   ```
   - Lease safety: fails if remote changed
   - Overwrites old dev with new integration branch

**Conflict Handling:**

When merge conflict detected:
1. `git merge --abort` (clean up merge state)
2. Remove topic from environment (state update)
3. Mark topic with `conflict` status
4. Log conflict: which topic conflicted with which
5. Continue with next topic (resilient)

**Idempotency:**

Rebuilds are idempotent:
- Running rebuild twice produces same result
- Ancestor checks prevent duplicate merges
- Marker commit is idempotent (already-empty commit)

---

## State Transitions

### Topic State Machine

```
        ┌─────────┐
        │ tracked │
        └────┬────┘
             │ add to env
             ▼
        ┌─────────┐
        │ active  │──┐
        └────┬────┘  │ conflict in rebuild
             │       │
             │       ▼
             │    ┌─────────┐
             │    │conflict │
             │    └────┬────┘
             │         │ resolve & re-promote
             │         └──────────┬──────────┘
             │                    │
             │ merge to master    ▼
             │              ┌────────────┐
             └──────────────│ graduated  │
                            └────────────┘
```

### Environment Membership

```
Topic added:    topic_environments += (topic_id, env_id)
Rebuild merge:  Topic stays in env
Conflict:       topic_environments -= (topic_id, env_id)  [removed]
Promotion:      topic_environments += (topic_id, env_id)  [re-add]
Merge to master: topic_environments = []                  [graduated]
```

---

## Provider Adapters (Future)

Restack is designed to work with any provider. Core rebuild is pure git. Providers add:

### ProviderAdapter Trait

```rust
trait ProviderAdapter {
    // Topic discovery from PRs
    fn list_prs(&self, filter: PrFilter) -> Result<Vec<PullRequest>>;

    // PR operations
    fn comment_on_pr(&self, pr_id: &str, body: &str) -> Result<()>;
    fn update_pr_labels(&self, pr_id: &str, labels: Vec<String>) -> Result<()>;

    // Branch protection
    fn get_branch_rules(&self, branch: &str) -> Result<Vec<BranchRule>>;
    fn set_branch_rules(&self, branch: &str, rules: Vec<BranchRule>) -> Result<()>;

    // CI operations
    fn get_pipeline_status(&self, run_id: &str) -> Result<PipelineStatus>;

    // Notifications
    fn notify_conflict(&self, topic: &str, conflict_info: ConflictInfo) -> Result<()>;
}
```

### Current Scope (MVP)

Pure git operations only:
- Topic tracking (manual `restack topic track`)
- Environment promotion (`restack promote`)
- Rebuild algorithm
- State management

### Future Providers

- **GitHub**: `gh` CLI wrapper for PR listing, comments, labels
- **Azure DevOps**: `az repos` CLI + REST API
- **Bitbucket**: REST API (no CLI available)

---

## Type System

### Rust Source of Truth

Rust structs are the canonical type definitions. TypeScript types are **generated** from Rust.

```
src/types.rs (Rust)
       ↓
./scripts/generate-types.sh
       ↓
ui/src/generated/types.ts (TypeScript)
```

Generation handles:
- Struct → interface (with optional fields)
- Enum → union types
- Vec → Array
- HashMap → Record
- DateTime → ISO 8601 string

Benefits:
- Single source of truth (Rust)
- No manual type synchronization
- Rust as system API contract

---

## Configuration

### Config File: `.restack/config.toml`

```toml
[workspace]
name = "my-workspace"

[defaults]
base_branch = "master"
environments = ["dev", "staging"]
auto_promote_on_ci_pass = false
force_push_mode = "lease"  # "lease" | "never" | "force"

[environments.dev]
branch = "dev"
ordinal = 0
auto_promote = false

[environments.staging]
branch = "staging"
ordinal = 1
auto_promote = false

[provider.github]
cli = "gh"
auth_method = "cli"

[rebuild]
max_topics = 50
rerere = true
conflict_strategy = "skip_and_notify"

[release]
versioning = "conventional"
changelog = true
maint_branches = true
```

### Environment Variables

```bash
RESTACK_DB_PATH         Override database location
NO_COLOR                Disable colored output
```

---

## API Layers

### Host → CLI Bridge

The Node.js host spawns the Rust CLI to execute operations.

```typescript
// host/src/cli.ts
async function runCli(args: string[]): Promise<string> {
  const child = spawn('restack', args);
  const stdout = await collect(child.stdout);
  // Parse JSON output
  return JSON.parse(stdout);
}
```

Commands return JSON:
```bash
restack --json rebuild dev
# →
{
  "env": "dev",
  "status": "success",
  "merged_topics": ["feature/login", "feature/auth"],
  "conflicted_topics": [],
  "result_sha": "abc123..."
}
```

### REST API Routes

Host exposes HTTP routes for UI:

```
GET  /api/repos                        # List repos
POST /api/repos                        # Add repo
GET  /api/repos/:id                    # Get repo details
DELETE /api/repos/:id                  # Remove repo

GET  /api/topics                       # List topics
POST /api/topics/track                 # Track topic
DELETE /api/topics/:id                 # Untrack
GET  /api/topics/:id/status            # Topic details

GET  /api/envs                         # List environments
POST /api/envs                         # Add environment
GET  /api/envs/:name/status            # Environment details

POST /api/promote/to                   # Promote topic
POST /api/promote/from                 # Demote topic

POST /api/rebuild/:env                 # Rebuild environment
GET  /api/rebuild/:env/status          # Last rebuild status

GET  /health                           # Health check
```

Each route calls CLI via bridge, returns JSON response.

---

## Performance Characteristics

### Rebuild Complexity

- **Git operations**: O(n) merges where n = topics in environment
- **Conflict detection**: O(n) merge attempts
- **Database queries**: O(log n) with indexes on repo_id, env_id

### Typical Times

- Small rebuild (5 topics): ~2 seconds
- Medium rebuild (20 topics): ~5 seconds
- Large rebuild (50 topics): ~15 seconds

(Dominated by network latency to git remote)

### Scaling Limits

- **Max topics per environment**: 50 (configurable, soft limit)
- **Max repos per workspace**: 100+ (no hard limit)
- **Database size**: <10MB for 1 year of history

---

## Error Handling

### Git Errors

All git operations are wrapped with error context:

```rust
fn git_merge(topic: &str) -> Result<(), RestackError> {
  match Command::new("git")
    .args(&["merge", "--no-ff", "--no-edit", &format!("origin/{}", topic)])
    .output() {
    Ok(output) if !output.status.success() => {
      Err(RestackError::MergeFailed {
        topic: topic.to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
      })
    }
    Ok(_) => Ok(()),
    Err(e) => Err(RestackError::GitCommandFailed(e)),
  }
}
```

### Database Errors

SQLite errors are caught and contextual:

```rust
fn add_topic_to_env(topic_id: &str, env_id: &str) -> Result<()> {
  conn.execute(
    "INSERT INTO topic_environments (topic_id, env_id, added_at) VALUES (?1, ?2, ?3)",
    [topic_id, env_id, &now()],
  )
  .map_err(|e| RestackError::DatabaseError {
    operation: "add_topic_to_env",
    cause: e.to_string(),
  })?;
  Ok(())
}
```

---

## Design Principles

1. **Stateless CLI**: Commands are composable, no session state
2. **Idempotent operations**: Running same command twice = same result
3. **Conflict-resilient**: One conflict doesn't fail entire rebuild
4. **Provider-agnostic**: Core algorithm doesn't depend on GitHub/Azure/etc
5. **Fast-path**: Most commands complete in <1 second
6. **Safe defaults**: `--force-with-lease` by default, `--dry-run` available
7. **Observable**: JSON output for automation, human-readable for CLI

---

## Future Improvements

- **Parallel rebuilds**: Rebuild multiple environments concurrently
- **Workspace federation**: Link multiple workspaces (e.g., frontend + backend)
- **Metrics**: Track rebuild times, conflict rates, topic lifetime
- **Auto-cleanup**: Delete graduated topic branches
