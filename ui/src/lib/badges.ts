/**
 * Shared badge color maps for topic status, CI, branch origin, and rebuild.
 *
 * Canonical source — imported by KanbanView, ListView, and DetailPanel.
 * Each map returns Tailwind class strings for consistent badge styling.
 */

import type {
  TopicStatus,
  CiStatus,
  BranchOrigin,
  RebuildStatus,
} from "../generated/types.js";

/** Border color applied to the topic card container based on status. */
export const STATUS_BORDER: Record<TopicStatus, string> = {
  active: "border-border",
  conflict: "border-status-conflict",
  graduated: "border-status-graduated/40",
  closed: "border-status-closed/40",
};

/** Badge background/text/border for topic status. */
export const STATUS_BADGE: Record<TopicStatus, string> = {
  active: "bg-status-active/20 text-status-active border-status-active/40",
  conflict: "bg-status-conflict/20 text-status-conflict border-status-conflict/40",
  graduated: "bg-status-graduated/20 text-status-graduated border-status-graduated/40",
  closed: "bg-status-closed/20 text-status-closed border-status-closed/40",
};

/** Badge background/text/border for CI status. */
export const CI_BADGE: Record<CiStatus, string> = {
  pending: "bg-status-ci-pending/20 text-status-ci-pending border-status-ci-pending/40",
  passed: "bg-status-ci-passed/20 text-status-ci-passed border-status-ci-passed/40",
  failed: "bg-status-ci-failed/20 text-status-ci-failed border-status-ci-failed/40",
};

/** Badge background/text for branch origin. */
export const ORIGIN_BADGE: Record<BranchOrigin, string> = {
  tracked: "bg-status-active/20 text-status-active border-status-active/40",
  "local-only": "bg-surface-secondary text-text-muted border-border/40",
  orphaned: "bg-status-conflict/20 text-status-conflict border-status-conflict/40",
};

/** Text color for rebuild status indicators. */
export const REBUILD_COLOR: Record<RebuildStatus, string> = {
  running: "text-rebuild-running",
  success: "text-rebuild-success",
  partial: "text-rebuild-partial",
  failed: "text-rebuild-failed",
};
