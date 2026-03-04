#!/bin/bash
# Generate TypeScript types from Rust types
#
# Source: restack/src/types.rs, restack/src/id.rs, restack/src/version.rs
#
# Usage: ./scripts/generate-types.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

OUTPUT_FILE="$PROJECT_ROOT/ui/src/generated/types.ts"
mkdir -p "$(dirname "$OUTPUT_FILE")"

cat > "$OUTPUT_FILE" << 'EOF'
/**
 * AUTO-GENERATED TypeScript types from Rust source of truth.
 *
 * Source: restack/src/types.rs, restack/src/id.rs, restack/src/version.rs
 *
 * DO NOT EDIT - regenerate with: ./scripts/generate-types.sh
 */

// ============ Branded ID Types ============

declare const RepoIdBrand: unique symbol;
declare const TopicIdBrand: unique symbol;
declare const EnvIdBrand: unique symbol;
declare const RebuildIdBrand: unique symbol;
declare const ConflictIdBrand: unique symbol;

/** Repo ID: "repo_" prefix + 26-char ULID */
export type RepoId = string & { readonly [RepoIdBrand]: never };

/** Topic ID: "topic_" prefix + 26-char ULID */
export type TopicId = string & { readonly [TopicIdBrand]: never };

/** Environment ID: "env_" prefix + 26-char ULID */
export type EnvId = string & { readonly [EnvIdBrand]: never };

/** Rebuild ID: "rebuild_" prefix + 26-char ULID */
export type RebuildId = string & { readonly [RebuildIdBrand]: never };

/** Conflict ID: "conflict_" prefix + 26-char ULID */
export type ConflictId = string & { readonly [ConflictIdBrand]: never };

// ============ Validation Helpers ============

export function isRepoId(s: string): s is RepoId {
  return s.startsWith("repo_") && s.length === 31;
}

export function isTopicId(s: string): s is TopicId {
  return s.startsWith("topic_") && s.length === 32;
}

export function isEnvId(s: string): s is EnvId {
  return s.startsWith("env_") && s.length === 30;
}

export function isRebuildId(s: string): s is RebuildId {
  return s.startsWith("rebuild_") && s.length === 34;
}

export function isConflictId(s: string): s is ConflictId {
  return s.startsWith("conflict_") && s.length === 35;
}

export function parseRepoId(s: string): RepoId {
  if (!isRepoId(s)) throw new Error(`Invalid RepoId: ${s}`);
  return s;
}

export function parseTopicId(s: string): TopicId {
  if (!isTopicId(s)) throw new Error(`Invalid TopicId: ${s}`);
  return s;
}

export function parseEnvId(s: string): EnvId {
  if (!isEnvId(s)) throw new Error(`Invalid EnvId: ${s}`);
  return s;
}

export function parseRebuildId(s: string): RebuildId {
  if (!isRebuildId(s)) throw new Error(`Invalid RebuildId: ${s}`);
  return s;
}

export function parseConflictId(s: string): ConflictId {
  if (!isConflictId(s)) throw new Error(`Invalid ConflictId: ${s}`);
  return s;
}

// ============ Enums ============

export type Provider = "gitHub" | "azureDevOps" | "bitbucket" | "unknown";

export type TopicStatus = "active" | "conflict" | "graduated" | "closed";

export type CiStatus = "pending" | "passed" | "failed";

export type RebuildStatus = "running" | "success" | "partial" | "failed";

export type ForcePushMode = "lease" | "never" | "force";

export type BumpType = "major" | "minor" | "patch";

// ============ Domain Types ============

export interface Repo {
  id: RepoId;
  name: string;
  path: string;
  remoteUrl: string | null;
  provider: Provider;
  baseBranch: string;
  createdAt: string; // ISO 8601
}

export interface Environment {
  id: EnvId;
  repoId: RepoId;
  name: string;
  branch: string;
  ordinal: number;
  autoPromote: boolean;
}

export interface Topic {
  id: TopicId;
  repoId: RepoId;
  branch: string;
  prId: string | null;
  prUrl: string | null;
  status: TopicStatus;
  ciStatus: CiStatus | null;
  ciUrl: string | null;
  lastCiCheck: string | null; // ISO 8601
  createdAt: string; // ISO 8601
}

export interface TopicEnvironment {
  topicId: TopicId;
  envId: EnvId;
  addedAt: string; // ISO 8601
}

export interface Rebuild {
  id: RebuildId;
  envId: EnvId;
  startedAt: string; // ISO 8601
  completedAt: string | null;
  status: RebuildStatus;
  topicsMerged: number;
  topicsConflicted: number;
  resultSha: string | null;
}

export interface Conflict {
  id: ConflictId;
  rebuildId: RebuildId;
  topicId: TopicId;
  conflictedWith: string | null;
  resolved: boolean;
  createdAt: string; // ISO 8601
}

// ============ Release / Hotfix Types ============

export interface ConventionalCommit {
  type: string;
  scope: string | null;
  breaking: boolean;
  description: string;
  sha: string;
}

export interface ChangelogSection {
  title: string;
  entries: ChangelogEntry[];
}

export interface ChangelogEntry {
  description: string;
  sha: string;
  scope: string | null;
}

export interface ReleaseInfo {
  version: string;
  tag: string;
  bumpType: BumpType;
  changelog: ChangelogSection[];
  previousVersion: string | null;
}

export interface HotfixInfo {
  version: string;
  tag: string;
  maintBranch: string;
  mergedToMaster: boolean;
}

// ============ Provider Integration Types ============

export type PrState = "open" | "closed" | "merged" | "all";

export type CheckStatus = "queued" | "inProgress" | "completed";

export type CheckConclusion =
  | "success"
  | "failure"
  | "neutral"
  | "cancelled"
  | "timedOut"
  | "actionRequired"
  | "skipped"
  | "stale";

export interface PullRequest {
  number: string;
  title: string;
  headBranch: string;
  baseBranch: string;
  state: PrState;
  url: string;
}

export interface CheckRun {
  name: string;
  status: CheckStatus;
  conclusion: CheckConclusion | null;
  url: string | null;
}

export interface CiStatusDetail {
  branch: string;
  sha: string | null;
  overall: CiStatus;
  checks: CheckRun[];
}

export interface SyncResult {
  created: number;
  updated: number;
  totalPrs: number;
}

export interface GeneratedFile {
  path: string;
  content: string;
}

// ============ PR Management Types ============

export interface CreatePrParams {
  head: string;
  base: string;
  title: string;
  body: string | null;
  draft: boolean;
}

export interface MergePrParams {
  prNumber: string;
  strategy: MergeStrategy;
  deleteBranch: boolean;
}

export interface MergePrResult {
  merged: boolean;
  sha: string | null;
  message: string;
}

export type MergeStrategy = "merge" | "squash" | "rebase";

// ============ Branch Protection Types ============

export interface BranchProtectionParams {
  branch: string;
  requiredChecks: string[];
  requirePr: boolean;
  minApprovals: number;
}

export interface BranchProtectionResult {
  branch: string;
  applied: boolean;
  message: string;
}

// ============ Pipeline Types ============

export interface TriggerPipelineParams {
  branch: string;
  pipelineName: string | null;
}

export interface PipelineRunResult {
  runId: string;
  url: string | null;
  status: string;
}

// ============ Error Types ============

export class CliError extends Error {
  constructor(
    message: string,
    public exitCode: number,
    public stderr: string
  ) {
    super(message);
    this.name = "CliError";
  }
}

export class CliTimeoutError extends Error {
  constructor(message = "CLI command timeout (30s)") {
    super(message);
    this.name = "CliTimeoutError";
  }
}
EOF

echo "Generated: $OUTPUT_FILE"
