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
import { STATUS_BADGE, CI_BADGE, ORIGIN_BADGE, getEnvColor } from "../../lib/badges.js";
import { Badge } from "../Badge.js";

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

  // Loading state — data not yet fetched
  if (!topics || !repos) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="w-16 h-1 rounded-full bg-border animate-skeleton-pulse" />
      </div>
    );
  }

  if (topics.length === 0) {
    const isNoRepos = repos.length === 0;
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-6 sm:gap-8 p-4 sm:p-8 font-mono animate-fade-in">
        {/* Ghost table header */}
        <div className="w-full max-w-2xl opacity-[0.07] pointer-events-none select-none animate-stagger-in" aria-hidden="true">
          <div className="flex gap-4 px-3 py-2 border-b border-text-primary">
            {["Repo", "Branch", "Status", "CI", "Envs", "Created"].map((col, i) => (
              <div key={col} className={`text-[11px] uppercase tracking-wider text-text-primary flex-1 ${i >= 3 ? "hidden sm:block" : ""}`}>{col}</div>
            ))}
          </div>
          {Array.from({ length: 3 }, (_, i) => (
            <div key={i} className="flex gap-4 px-3 py-3 border-b border-dashed border-text-primary">
              {Array.from({ length: 6 }, (_, j) => (
                <div key={j} className={`flex-1 h-3 rounded bg-text-primary/50 ${j >= 3 ? "hidden sm:block" : ""}`} style={{ width: `${55 + ((i * 3 + j) % 5) * 10}%` }} />
              ))}
            </div>
          ))}
        </div>
        {/* Message */}
        <div className="flex flex-col items-center gap-1.5 text-center">
          {isNoRepos ? (
            <>
              <p className="text-text-muted text-sm uppercase tracking-wider">No repositories tracked</p>
              <p className="text-text-muted text-xs">
                Click <span className="text-accent">Sync</span> above or run{" "}
                <code className="text-accent">restack sync</code> to get started
              </p>
            </>
          ) : (
            <>
              <p className="text-text-muted text-sm uppercase tracking-wider">No topics</p>
              <p className="text-text-muted text-xs">
                Run <code className="text-accent">restack topic add</code> to track a branch
              </p>
            </>
          )}
        </div>
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
      <div className="flex-1 overflow-y-auto">
        <table className="w-full border-collapse font-mono" aria-label="Topics">
          <thead className="sticky top-0 z-10 bg-bg-secondary">
            <tr className="border-b border-border">
              {columns.map((col) => (
                <th
                  key={col.field}
                  scope="col"
                  className={`${col.className} px-4 py-2 text-left text-[11px] uppercase tracking-wider text-text-dim hover:text-text-muted transition-colors cursor-pointer font-normal focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus focus-visible:ring-inset`}
                  tabIndex={0}
                  onClick={() => toggleSort(col.field)}
                  onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); toggleSort(col.field); } }}
                  aria-sort={sort.field === col.field ? (sort.dir === "asc" ? "ascending" : "descending") : undefined}
                >
                  {col.label}
                  <SortIndicator active={sort.field === col.field} dir={sort.dir} />
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {sortedRows.map((row) => {
              const isSelected = selectedTopicId === row.topic.id;
              return (
                <tr
                  key={row.topic.id}
                  tabIndex={0}
                  aria-selected={isSelected}
                  className={`
                    border-b border-border/50 transition-colors cursor-pointer
                    focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus focus-visible:ring-inset
                    ${isSelected
                      ? "bg-accent-subtle border-accent/30"
                      : "hover:bg-surface-primary/50"}
                  `}
                  onClick={() => handleRowClick(row.topic.id)}
                  onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); handleRowClick(row.topic.id); } }}
                >
                  {/* Repo */}
                  <td className="w-[140px] px-4 py-2.5 text-xs text-text-muted truncate max-w-[140px]">
                    {row.repoName}
                  </td>

                  {/* Branch */}
                  <td className="px-4 py-2.5 text-sm text-text-primary truncate max-w-0">
                    <span className="flex items-center gap-2">
                      <span className="truncate">{row.topic.branch}</span>
                      {row.topic.branchOrigin !== "tracked" && (
                        <Badge
                          label={row.topic.branchOrigin === "local-only" ? "local" : "orphaned"}
                          colorClass={ORIGIN_BADGE[row.topic.branchOrigin]}
                        />
                      )}
                    </span>
                  </td>

                  {/* Status */}
                  <td className="w-[100px] px-4 py-2.5">
                    <Badge
                      label={row.topic.status}
                      colorClass={STATUS_BADGE[row.topic.status] ?? STATUS_BADGE.closed}
                    />
                  </td>

                  {/* CI */}
                  <td className="w-[90px] px-4 py-2.5">
                    {row.topic.ciStatus ? (
                      <Badge
                        label={row.topic.ciStatus}
                        colorClass={CI_BADGE[row.topic.ciStatus] ?? CI_BADGE.pending}
                      />
                    ) : (
                      <span className="text-[10px] text-text-dim">--</span>
                    )}
                  </td>

                  {/* Environments */}
                  <td className="w-[180px] px-4 py-2.5">
                    <span className="flex flex-wrap gap-1">
                      {row.envNames.length > 0 ? (
                        row.envNames.map((name) => {
                          const color = getEnvColor(name);
                          return (
                            <span
                              key={`${row.topic.id}-${name}`}
                              className="text-[10px] px-1.5 py-0.5 rounded border font-mono"
                              style={{
                                color,
                                borderColor: `color-mix(in srgb, ${color} 40%, transparent)`,
                                backgroundColor: `color-mix(in srgb, ${color} 10%, transparent)`,
                              }}
                            >
                              {name}
                            </span>
                          );
                        })
                      ) : (
                        <span className="text-[10px] text-text-dim">--</span>
                      )}
                    </span>
                  </td>

                  {/* Created */}
                  <td className="w-[90px] px-4 py-2.5 text-[11px] text-text-dim">
                    {formatRelative(row.topic.createdAt)}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>

      {/* Footer */}
      <div className="px-4 py-2 border-t border-border text-xs text-text-dim font-mono shrink-0">
        {sortedRows.length} topic{sortedRows.length !== 1 ? "s" : ""}
        {selectedRepoId ? " (filtered)" : ""}
      </div>
    </div>
  );
}
