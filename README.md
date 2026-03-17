# Restack

Topic branch integration manager for multi-environment deployment workflows.

Restack manages feature branches across integration environments (dev, staging, production) using composable CLI commands. State lives in SQLite — no provider API dependency for core operations. Works with any git remote.

## Why Restack?

| Problem | Solution |
|---------|----------|
| PR labels don't exist on Azure DevOps / Bitbucket | CLI owns state in SQLite |
| Rebuilding integration branches is manual and error-prone | Idempotent two-phase rebuild algorithm |
| No visibility into which topics are in which environments | Kanban UI + `topic list --all-repos` |
| GitHub Actions workflows are provider-locked | Provider-agnostic core, optional adapters |

## Quick Start

```bash
# Build from source
cargo build --release
export PATH="$PWD/target/release:$PATH"

# Initialize workspace (auto-tracks current repo + discovers all branches)
cd ~/my-project
restack init

# Refresh: fetch origin, discover new branches, sync CI status
restack refresh

# Promote a topic to dev
restack promote to feature/login dev

# Rebuild dev environment
restack rebuild env dev
```

## Core Concepts

```
Topics (feature branches)
  │
  ├── promote to ──► Environments (dev, staging, production)
  │                       │
  │                       └── rebuild ──► Integration branch
  │                                         │
  └── graduate ◄────────── release cut ◄────┘
```

**Topics** are feature branches tracked by restack.
**Environments** are integration lanes (dev, staging). Each maps to a git branch.
**Rebuild** resets the env branch to master, merges all assigned topics, force-pushes.
**Promotion** moves topics between environments and triggers rebuilds.

### Two-Phase Rebuild

For `dev` environments, restack uses a two-phase merge:

1. **Phase 1**: Merge topics that are also in staging (keeps dev superset of staging)
2. **Marker commit**: `### Match 'staging'` (visual boundary in git log)
3. **Phase 2**: Merge dev-only topics

Conflicts are detected and the topic is removed from the environment. Use `--interactive` for manual resolution.

## Installation

**From source:**
```bash
cargo build --release
```

**From npm** (after publishing):
```bash
npm install -g restack-cli
```

## Commands

| Command | Description |
|---------|-------------|
| `restack init` | Initialize workspace, auto-track repo, discover branches |
| `restack refresh` | Fetch origin, discover new branches, sync CI, cleanup stale |
| `restack repo list/remove` | Repository management |
| `restack topic track/untrack/archive/list/status` | Topic branch tracking |
| `restack env add/list/status` | Environment management |
| `restack promote to/from` | Move topics between environments |
| `restack rebuild env/all` | Rebuild integration branches |
| `restack ui` | Start web UI |

Full reference: [docs/CLI.md](docs/CLI.md)

## Web UI

```bash
npm run build && restack ui
# http://localhost:6969
```

Three views:
- **Kanban**: Environment lanes with topic cards (+ Unassigned lane for conflicts)
- **Canvas**: Multi-repo tree (ReactFlow + dagre)
- **List**: Table view

Features:
- Drag-and-drop promotion between environments
- Toast notifications for merge conflicts
- Refresh button for branch discovery

## MCP Server

For AI agent integration (Claude, etc.):

```bash
cd host && npx tsx src/index.ts mcp
```

Exposes all restack commands as MCP tools over stdio.

## Development

```bash
# Build CLI
cargo build

# Run tests
cargo test

# Dev mode (host + UI with hot reload)
npm run dev

# Typecheck everything
npm run typecheck

# Regenerate TypeScript types from Rust
npm run generate-types
```

## Documentation

| Doc | Description |
|-----|-------------|
| [docs/CLI.md](docs/CLI.md) | Complete command reference |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | System design, state model, rebuild algorithm |
| [MIGRATION.md](MIGRATION.md) | Migrating from GitHub Actions workflows |

## License

MIT
// test
// test2
