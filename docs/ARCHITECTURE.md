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
    ci_status TEXT,                     -- 'pending' | 'passed' | 'failed'
    ci_url TEXT,
    last_ci_check TEXT,
    UNIQUE(repo_id, name)
);

-- Topic branches (feature branches / PRs)
CREATE TABLE topics (
    id TEXT PRIMARY KEY,                -- ULID, prefixed "topic_"
    repo_id TEXT NOT NULL,
    branch TEXT NOT NULL,               -- branch name
    pr_id TEXT,                         -- provider PR ID if exists
    pr_url TEXT,
    status TEXT NOT NULL,               -- 'active' | 'conflict' | 'graduated' | 'closed' | 'ci_quarantined'
    ci_status TEXT,                     -- 'pending' | 'passed' | 'failed'
    ci_url TEXT,
    last_ci_check TEXT,
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
    result_sha TEXT,
    ci_status TEXT,                     -- 'pending' | 'passed' | 'failed'
    ci_url TEXT,
    ci_checked_at TEXT,
    ci_retry_count INTEGER DEFAULT 0,
    ci_override TEXT                    -- manual override of CI status
);

-- Which topics were in each rebuild (for blame tracking)
CREATE TABLE rebuild_topics (
    rebuild_id TEXT NOT NULL,
    topic_id TEXT NOT NULL,
    phase INTEGER DEFAULT 0,            -- 0=phase1/single, 1=phase2
    merge_order INTEGER NOT NULL,       -- order within rebuild
    PRIMARY KEY (rebuild_id, topic_id)
);

-- Speculative refs for parallel CI blame detection
CREATE TABLE speculative_refs (
    id TEXT PRIMARY KEY,                -- ULID, prefixed "specref_"
    rebuild_id TEXT NOT NULL,
    env_id TEXT NOT NULL,
    step INTEGER NOT NULL,              -- 0-indexed cumulative merge step
    topic_id TEXT NOT NULL,             -- topic merged at this step
    sha TEXT NOT NULL,                  -- cumulative commit SHA
    branch_name TEXT NOT NULL,          -- e.g. "restack/spec/staging/xxx/step-0"
    ci_status TEXT,
    ci_url TEXT,
    created_at TEXT NOT NULL,
    UNIQUE(rebuild_id, step)
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

## CI Feedback Loop

Restack provides environment-level CI tracking with automatic blame detection. When a rebuild force-pushes an environment branch, CI runs on the result. Restack monitors CI status and, on failure, identifies the culprit topic.

### Architecture Overview

```
  rebuild_env()
       │
       ├─ Merge topics (object-level, no working tree)
       ├─ Record rebuild_topics (topic_id, phase, merge_order)
       ├─ Tree-OID dedup: skip push if tree unchanged from last green build
       ├─ Force-push env branch → triggers CI
       ├─ Create speculative refs (step-0, step-1, ...) → triggers parallel CI
       └─ Set env.ci_status = Pending

  refresh_env_ci_statuses()  (called periodically)
       │
       ├─ For each env: check CI via provider adapter
       ├─ On Passed: update env + rebuild ci_status
       ├─ On Failed:
       │    ├─ Retry if ci_retry_count < max_ci_retries
       │    └─ Else: run blame → quarantine culprit → PR comment
       └─ Blame strategy:
            ├─ Speculative (exact): check per-step CI results
            └─ Differential (fallback): compare green vs red topic sets
```

### CI Status Flow

```
Rebuild completes → ci_status = Pending
                         │
            ┌────────────┼────────────┐
            ▼            ▼            ▼
         Passed       Pending      Failed
       (promote OK)  (wait)     ┌────┴────┐
                                ▼         ▼
                           retry < max?  blame
                              │           │
                              ▼           ▼
                          reset to    quarantine
                          Pending     culprit topic
```

### Speculative Parallel Execution

The "endgame" optimization for blame detection. Instead of waiting for the full env CI to fail and then guessing which topic broke it, restack creates **cumulative speculative refs** during rebuild:

```
base ──→ base+topic1 ──→ base+topic1+topic2 ──→ ... ──→ base+all (env branch)
              │                   │                              │
         step-0 ref          step-1 ref                    env branch
         (push, CI)          (push, CI)                    (push, CI)
```

Each speculative ref triggers CI in parallel. When CI results arrive:
- If step-0 passes but step-1 fails → **topic2 is the exact culprit**
- All steps checked in parallel → **O(1) wall-clock time** for blame

**Branch naming**: `restack/spec/{env_name}/{rebuild_id}/step-{N}`

**Lifecycle**:
1. Created during `rebuild_env()` after force-push
2. Pushed in batch via single `git push`
3. CI checked via `check_speculative_ci()`
4. Cleaned up at start of next rebuild (old refs deleted from remote + local + DB)

**Fallback**: When no speculative refs exist (e.g., old rebuilds before this feature), the differential blame algorithm is used instead.

### Differential Blame (Fallback)

Compares the topic set of the last green rebuild against the current red rebuild:

```
Green rebuild topics: {A, B, C}
Red rebuild topics:   {A, B, C, D, E}
                                ↑ ↑
                          New since green → suspects
```

**Confidence levels**:
- **High**: Single new topic since green → almost certainly the culprit
- **Medium**: Multiple new topics since green → one of them
- **Low**: No green rebuild exists → fallback to last-in heuristic (highest merge_order)

### Tree-OID Deduplication

Before force-pushing, restack compares the rebuilt tree OID against the last successful CI-passed rebuild. If identical (e.g., a topic was promoted then immediately demoted), the push is skipped and CI status is carried forward as `Passed`. This prevents redundant CI runs.

### Quarantine and Retry

1. **Retry-before-blame**: On first CI failure, `ci_retry_count` is incremented and status reset to `Pending`. Only after `max_ci_retries` (default: 1) exhausted does blame run.
2. **CiQuarantined status**: The blamed topic is set to `CiQuarantined`, preventing it from being auto-promoted to other environments.
3. **PR notification**: A comment is posted on the culprit topic's PR with confidence level and CI run URL.

### Per-Environment CI Configuration

```toml
[environments.staging]
branch = "staging"
ci_strategy = "full"       # "full" | "buildOnly" | "none"
ci_pipeline = "build-test" # optional: specific pipeline name
max_ci_retries = 1         # retries before blame (default: 1)
auto_demote = false        # auto-remove blamed topics (default: false)
```

### Auto-Promote Gating

`promote_auto()` skips environments with `ci_status == Failed` or `Pending`. Only environments with `ci_status == None` (permissive default) or `Passed` receive auto-promoted topics. This prevents cascading failures into already-broken environments.

### CI Override

When CI is flaky or a known failure is acceptable, use `restack env ci-override` to clear the CI status. This sets `ci_override` on the rebuild record and resets the environment's `ci_status` to `None`, unblocking auto-promotion.

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
        │ active  │──────────────────┐
        └────┬────┘                  │
             │               conflict in rebuild
             │                       │
             │       ┌───────────────┤
             │       │               │
             │       ▼               ▼
             │  ┌─────────┐   ┌──────────────┐
             │  │conflict │   │ci_quarantined│
             │  └────┬────┘   └──────┬───────┘
             │       │ resolve        │ fix CI & re-promote
             │       └───────┬────────┘
             │               │
             │ merge to master
             │               ▼
             │         ┌────────────┐
             └─────────│ graduated  │
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

## Provider Adapters

Restack is designed to work with any provider. Core rebuild is pure git. Providers add CI status polling, PR comments, pipeline triggering, and branch protection.

### ProviderAdapter Trait

```rust
trait ProviderAdapter: Send + Sync {
    fn provider(&self) -> Provider;
    fn list_prs(&self, state: PrState) -> Result<Vec<PullRequest>>;
    fn get_ci_status(&self, branch_or_sha: &str) -> Result<CiStatusDetail>;
    fn comment_on_pr(&self, pr_number: &str, body: &str) -> Result<()>;
    fn is_available(&self) -> bool;
    fn create_pr(&self, params: &CreatePrParams) -> Result<PullRequest>;
    fn merge_pr(&self, params: &MergePrParams) -> Result<MergePrResult>;
    fn set_branch_protection(&self, params: &BranchProtectionParams) -> Result<BranchProtectionResult>;
    fn trigger_pipeline(&self, params: &TriggerPipelineParams) -> Result<PipelineRunResult>;
}
```

### Supported Providers

- **GitHub**: `gh` CLI for PR listing, CI status, comments, branch protection
- **Azure DevOps**: `az repos` / `az pipelines` CLI for full lifecycle
- **Bitbucket**: REST API for PR and pipeline operations
- **NullAdapter**: Fallback when provider is unknown or unconfigured

### CI Integration

The `get_ci_status(branch_or_sha)` method works for both topic branches and environment branches. It returns `CiStatusDetail` with:
- Overall status (`Pending` / `Passed` / `Failed`)
- Individual check runs with names, statuses, conclusions, and URLs
- Commit SHA for staleness detection

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

[environments.staging]
branch = "staging"
ordinal = 0
auto_promote = false
ci_strategy = "full"         # "full" | "buildOnly" | "none"
ci_pipeline = "build-test"   # optional: target specific pipeline
max_ci_retries = 1           # retries before blame (default: 1)
auto_demote = false          # auto-remove blamed topics

[environments.dev]
branch = "dev"
ordinal = 1
auto_promote = true
ci_strategy = "none"

[provider]
auto_ci_refresh = true       # refresh CI on restack refresh
conflict_notifications = false

[provider.github]
repo_slug = "owner/repo"    # optional: explicit repo slug

[rebuild]
force_push = "lease"         # "lease" | "never" | "force"
marker_commits = true
rebuild_debounce_secs = 0    # skip rebuild if last completed within N seconds

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
- **Speculative ref rate limiting**: Guard against CI provider rate limits for large topic sets
- **Speculative ref TTL**: Garbage-collect orphaned speculative branches
- **Binary bisection**: For extremely large topic sets, use binary search instead of linear speculative refs
