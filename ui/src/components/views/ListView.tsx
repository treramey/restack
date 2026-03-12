/**
 * List view — sortable table of all topics across all repos.
 *
 * Columns: Repo, Branch, Status, CI Status, Environments, Created
 * Filterable by selectedRepoId from store.
 */

import { useMemo, useState, useCallback } from "react";
import { useUIStore } from "../../lib/store.js";
import {
  useRepos,
  useTopics,
  useEnvironments,
  useTopicEnvironments,
} from "../../lib/queries.js";
import type {
  Repo,
  Topic,
  Environment,
  RepoId,
  TopicId,
  EnvId,
} from "../../generated/types.js";

// ── Sort config ──────────────────────────────────────────────────────

type SortField = "repo" | "branch" | "status" | "ciStatus" | "environments" | "created";
type SortDir = "asc" | "desc";

interface SortConfig {
  field: SortField;
  dir: SortDir;
}

// ── Row data (pre-joined for sort/display) ───────────────────────────

interface TopicRow {
  topic: Topic;
  repoName: string;
  envNames: string[];
  envCount: number;
}

// ── Status badge helpers ─────────────────────────────────────────────

const STATUS_COLORS: Record<string, string> = {
  active: "bg-status-active/20 text-status-active border-status-active/40",
  conflict: "bg-status-conflict/20 text-status-conflict border-status-conflict/40",
  graduated: "bg-status-graduated/20 text-status-graduated border-status-graduated/40",
  closed: "bg-status-closed/20 text-status-closed border-status-closed/40",
};

const CI_COLORS: Record<string, string> = {
  pending: "bg-status-ci-pending/20 text-status-ci-pending border-status-ci-pending/40",
  passed: "bg-status-ci-passed/20 text-status-ci-passed border-status-ci-passed/40",
  failed: "bg-status-ci-failed/20 text-status-ci-failed border-status-ci-failed/40",
};

const ORIGIN_COLORS: Record<Topic["branchOrigin"], string> = {
  tracked: "bg-status-active/20 text-status-active border-status-active/40",
  "local-only": "bg-surface-secondary text-text-muted border-border/40",
  orphaned: "bg-status-conflict/20 text-status-conflict border-status-conflict/40",
};

function Badge({ label, colorClass }: { label: string; colorClass: string }) {
  return (
    <span className={`px-1.5 py-0.5 rounded border text-[10px] font-mono uppercase tracking-wider ${colorClass}`}>
      {label}
    </span>
  );
}

// ── Sort arrow ───────────────────────────────────────────────────────

function SortIndicator({ active, dir }: { active: boolean; dir: SortDir }) {
  if (!active) return <span className="text-text-dim/40 ml-1">&#8597;</span>;
  return <span className="text-accent ml-1">{dir === "asc" ? "\u25B2" : "\u25BC"}</span>;
}

// ── Relative time ────────────────────────────────────────────────────

function formatRelative(iso: string): string {
  const ms = Date.now() - new Date(iso).getTime();
  const sec = Math.floor(ms / 1000);
  if (sec < 60) return "just now";
  const min = Math.floor(sec / 60);
  if (min < 60) return `${String(min)}m ago`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${String(hr)}h ago`;
  const d = Math.floor(hr / 24);
  return `${String(d)}d ago`;
}

// ── Main component ───────────────────────────────────────────────────

export function ListView() {
  const selectedRepoId = useUIStore((s) => s.selectedRepoId);
  const setSelectedTopicId = useUIStore((s) => s.setSelectedTopicId);
  const selectedTopicId = useUIStore((s) => s.selectedTopicId);

  const { data: repos } = useRepos();
  const { data: topics } = useTopics();
  const { data: environments } = useEnvironments();
  const { data: topicEnvs } = useTopicEnvironments();

  const [sort, setSort] = useState<SortConfig>({ field: "created", dir: "desc" });

  const repoMap = useMemo(() => {
    if (!repos) return new Map<RepoId, Repo>();
    return new Map(repos.map((r) => [r.id, r]));
  }, [repos]);

  const envMap = useMemo(() => {
    if (!environments) return new Map<EnvId, Environment>();
    return new Map(environments.map((e) => [e.id, e]));
  }, [environments]);

  // Build topic → env names lookup
  const topicEnvNames = useMemo(() => {
    if (!topicEnvs) return new Map<TopicId, string[]>();
    const map = new Map<TopicId, string[]>();
    for (const te of topicEnvs) {
      const env = envMap.get(te.envId);
      if (env) {
        const list = map.get(te.topicId) ?? [];
        list.push(env.name);
        map.set(te.topicId, list);
      }
    }
    return map;
  }, [topicEnvs, envMap]);

  // Build rows with joined data
  const rows: TopicRow[] = useMemo(() => {
    if (!topics) return [];
    const filtered = selectedRepoId
      ? topics.filter((t) => t.repoId === selectedRepoId)
      : topics;

    return filtered.map((topic) => {
      const repo = repoMap.get(topic.repoId);
      const envNames = topicEnvNames.get(topic.id) ?? [];
      return {
        topic,
        repoName: repo?.name ?? "unknown",
        envNames,
        envCount: envNames.length,
      };
    });
  }, [topics, selectedRepoId, repoMap, topicEnvNames]);

  // Sort rows
  const sortedRows = useMemo(() => {
    const sorted = [...rows];
    const dir = sort.dir === "asc" ? 1 : -1;

    sorted.sort((a, b) => {
      switch (sort.field) {
        case "repo":
          return dir * a.repoName.localeCompare(b.repoName);
        case "branch":
          return dir * a.topic.branch.localeCompare(b.topic.branch);
        case "status":
          return dir * a.topic.status.localeCompare(b.topic.status);
        case "ciStatus":
          return dir * (a.topic.ciStatus ?? "").localeCompare(b.topic.ciStatus ?? "");
        case "environments":
          return dir * (a.envCount - b.envCount);
        case "created":
          return dir * a.topic.createdAt.localeCompare(b.topic.createdAt);
        default:
          return 0;
      }
    });
    return sorted;
  }, [rows, sort]);

  const toggleSort = useCallback((field: SortField) => {
    setSort((prev) =>
      prev.field === field
        ? { field, dir: prev.dir === "asc" ? "desc" : "asc" }
        : { field, dir: "asc" },
    );
  }, []);

  const handleRowClick = useCallback(
    (topicId: TopicId) => {
      setSelectedTopicId(topicId);
    },
    [setSelectedTopicId],
  );

  if (!topics || topics.length === 0) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-4 text-text-muted p-8">
        <p className="font-mono uppercase tracking-wider text-sm">No topics</p>
        <p className="text-text-dim text-xs font-mono">Run `restack topic add` to begin</p>
      </div>
    );
  }

  const columns: Array<{ field: SortField; label: string; className: string }> = [
    { field: "repo", label: "Repo", className: "w-[140px]" },
    { field: "branch", label: "Branch", className: "flex-1 min-w-[180px]" },
    { field: "status", label: "Status", className: "w-[100px]" },
    { field: "ciStatus", label: "CI", className: "w-[90px]" },
    { field: "environments", label: "Environments", className: "w-[180px]" },
    { field: "created", label: "Created", className: "w-[90px]" },
  ];

  return (
    <div className="flex-1 flex flex-col bg-bg-primary overflow-hidden">
      {/* Table header */}
      <div className="flex items-center px-4 py-2 border-b border-border bg-bg-secondary shrink-0 gap-2">
        {columns.map((col) => (
          <button
            key={col.field}
            className={`${col.className} text-left text-[11px] font-mono uppercase tracking-wider text-text-dim hover:text-text-muted transition-colors cursor-pointer flex items-center`}
            onClick={() => toggleSort(col.field)}
          >
            {col.label}
            <SortIndicator active={sort.field === col.field} dir={sort.dir} />
          </button>
        ))}
      </div>

      {/* Rows */}
      <div className="flex-1 overflow-y-auto">
        {sortedRows.map((row) => {
          const isSelected = selectedTopicId === row.topic.id;
          return (
            <button
              key={row.topic.id}
              type="button"
              className={`
                w-full flex items-center px-4 py-2.5 gap-2 text-left
                border-b border-border/50 transition-colors cursor-pointer
                focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus focus-visible:ring-inset
                ${isSelected
                  ? "bg-accent-subtle border-accent/30"
                  : "hover:bg-surface-primary/50"}
              `}
              onClick={() => handleRowClick(row.topic.id)}
            >
              {/* Repo */}
              <span className="w-[140px] text-xs font-mono text-text-muted truncate">
                {row.repoName}
              </span>

              {/* Branch */}
              <span className="flex-1 min-w-[180px] text-sm font-mono text-text-primary truncate flex items-center gap-2">
                <span className="truncate">{row.topic.branch}</span>
                {row.topic.branchOrigin !== "tracked" && (
                  <Badge
                    label={row.topic.branchOrigin === "local-only" ? "local" : "orphaned"}
                    colorClass={ORIGIN_COLORS[row.topic.branchOrigin]}
                  />
                )}
              </span>

              {/* Status */}
              <span className="w-[100px]">
                <Badge
                  label={row.topic.status}
                  colorClass={STATUS_COLORS[row.topic.status] ?? STATUS_COLORS["closed"]!}
                />
              </span>

              {/* CI */}
              <span className="w-[90px]">
                {row.topic.ciStatus ? (
                  <Badge
                    label={row.topic.ciStatus}
                    colorClass={CI_COLORS[row.topic.ciStatus] ?? CI_COLORS["pending"]!}
                  />
                ) : (
                  <span className="text-[10px] font-mono text-text-dim">--</span>
                )}
              </span>

              {/* Environments */}
              <span className="w-[180px] flex flex-wrap gap-1">
                {row.envNames.length > 0 ? (
                  row.envNames.map((name) => (
                    <span
                      key={name}
                      className="text-[9px] font-mono px-1.5 py-0.5 rounded bg-surface-secondary border border-border text-text-muted"
                    >
                      {name}
                    </span>
                  ))
                ) : (
                  <span className="text-[10px] font-mono text-text-dim">--</span>
                )}
              </span>

              {/* Created */}
              <span className="w-[90px] text-[11px] font-mono text-text-dim">
                {formatRelative(row.topic.createdAt)}
              </span>
            </button>
          );
        })}
      </div>

      {/* Footer */}
      <div className="px-4 py-2 border-t border-border text-xs text-text-dim font-mono shrink-0">
        {sortedRows.length} topic{sortedRows.length !== 1 ? "s" : ""}
        {selectedRepoId ? " (filtered)" : ""}
      </div>
    </div>
  );
}
