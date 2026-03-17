/**
 * Header: [RESTACK logo] [Kanban|Canvas|List tabs] [repo selector] [spacer] [status]
 */

import { useEffect, useRef, useLayoutEffect } from "react";
import { useUIStore, type ViewMode } from "../lib/store.js";
import { useRepos, useContext } from "../lib/queries.js";
import { useRefresh } from "../lib/mutations.js";
import { useRefreshStatus } from "../lib/websocket.js";
import { isRepoId } from "../generated/types.js";

const TABS: ReadonlyArray<{ mode: ViewMode; label: string; key: string }> = [
  { mode: "kanban", label: "Kanban", key: "1" },
  { mode: "canvas", label: "Canvas", key: "2" },
  { mode: "list", label: "List", key: "3" },
];

export function Header() {
  const viewMode = useUIStore((s) => s.viewMode);
  const setViewMode = useUIStore((s) => s.setViewMode);
  const selectedRepoId = useUIStore((s) => s.selectedRepoId);
  const setSelectedRepoId = useUIStore((s) => s.setSelectedRepoId);
  const { data: repos } = useRepos();
  const { data: context } = useContext();

  // Auto-select repo from context on initial load
  useEffect(() => {
    if (context?.repoName && repos && selectedRepoId === null) {
      const match = repos.find((r) => r.name === context.repoName);
      if (match) {
        setSelectedRepoId(match.id);
      }
    }
  }, [context, repos, selectedRepoId, setSelectedRepoId]);

  // Kanban requires a specific repo — auto-select first if "All repos" is active
  // useLayoutEffect to prevent flash of "no repo selected" state
  useLayoutEffect(() => {
    if (viewMode === "kanban" && selectedRepoId === null && repos && repos.length > 0) {
      setSelectedRepoId(repos[0]!.id);
    }
  }, [viewMode, selectedRepoId, repos, setSelectedRepoId]);

  const showAllReposOption = viewMode !== "kanban";

  const handleRepoChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    const value = e.target.value;
    setSelectedRepoId(value === "" ? null : isRepoId(value) ? value : null);
  };

  return (
    <header className="flex items-center h-12 px-4 gap-3 border-b border-border bg-bg-secondary shrink-0">
      {/* Logo */}
      <span className="text-accent font-mono font-bold text-base tracking-[0.12em] leading-none uppercase select-none">
        RESTACK
      </span>

      {/* Divider */}
      <div className="h-6 w-px bg-border/70" aria-hidden="true" />

      {/* View tabs */}
      <nav aria-label="Views">
        <ViewTabs viewMode={viewMode} onChangeView={setViewMode} />
      </nav>

      {/* Repo selector — always rendered to prevent layout shift */}
      <select
        value={selectedRepoId ?? ""}
        onChange={handleRepoChange}
        disabled={!repos}
        aria-label="Filter by repository"
        className="h-9 px-2 text-[13px] font-mono bg-surface-primary text-text-primary border border-border rounded focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed"
      >
        {showAllReposOption && <option value="">All repos</option>}
        {repos?.map((repo) => (
          <option key={repo.id} value={repo.id}>
            {repo.name}
          </option>
        ))}
      </select>

      {/* Spacer */}
      <div className="flex-1" />

      {/* Refresh button */}
      <RefreshButton />

      {/* Repo count */}
      {repos && (
        <span className="text-xs text-text-dim font-mono">
          {repos.length} repo{repos.length !== 1 ? "s" : ""}
        </span>
      )}
    </header>
  );
}

/** Tabbed view switcher with a sliding active indicator. */
function ViewTabs({ viewMode, onChangeView }: { viewMode: ViewMode; onChangeView: (mode: ViewMode) => void }) {
  const containerRef = useRef<HTMLDivElement>(null);
  const buttonRefs = useRef<Map<ViewMode, HTMLButtonElement>>(new Map());

  // Indicator position: left offset + width of the active button
  const indicatorStyle = useRef<{ left: number; width: number }>({ left: 0, width: 0 });

  function measureIndicator() {
    const btn = buttonRefs.current.get(viewMode);
    const container = containerRef.current;
    if (!btn || !container) return;
    const cRect = container.getBoundingClientRect();
    const bRect = btn.getBoundingClientRect();
    indicatorStyle.current = { left: bRect.left - cRect.left, width: bRect.width };
  }

  // Measure on mount and whenever active tab changes
  useLayoutEffect(() => {
    measureIndicator();
    // Force re-render to apply measured values
    containerRef.current?.style.setProperty("--ind-left", `${String(indicatorStyle.current.left)}px`);
    containerRef.current?.style.setProperty("--ind-width", `${String(indicatorStyle.current.width)}px`);
  }, [viewMode]);

  return (
    <div
      ref={containerRef}
      role="tablist"
      aria-label="View mode"
      className="relative inline-flex h-9 rounded border border-border bg-bg-secondary overflow-hidden divide-x divide-border"
    >
      {/* Sliding indicator */}
      <div
        aria-hidden="true"
        className="absolute bottom-0 h-[2px] bg-accent transition-all duration-150"
        style={{
          left: "var(--ind-left, 0px)",
          width: "var(--ind-width, 0px)",
        }}
      />
      {TABS.map(({ mode, label, key }) => (
        <button
          key={mode}
          ref={(el) => { if (el) buttonRefs.current.set(mode, el); }}
          role="tab"
          aria-selected={viewMode === mode}
          tabIndex={viewMode === mode ? 0 : -1}
          className={`
            h-9 px-3 inline-flex items-center justify-center
            text-[13px] leading-none font-mono border-0
            transition-colors duration-150 cursor-pointer
            focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus
            ${viewMode === mode
              ? "text-accent"
              : "text-text-muted hover:text-text-primary hover:bg-surface-primary/60"}
          `}
          onClick={() => onChangeView(mode)}
          onKeyDown={(e) => {
            const idx = TABS.findIndex((t) => t.mode === mode);
            if (e.key === "ArrowRight" || e.key === "ArrowDown") {
              e.preventDefault();
              const next = TABS[(idx + 1) % TABS.length]!;
              onChangeView(next.mode);
            } else if (e.key === "ArrowLeft" || e.key === "ArrowUp") {
              e.preventDefault();
              const prev = TABS[(idx - 1 + TABS.length) % TABS.length]!;
              onChangeView(prev.mode);
            }
          }}
        >
          <span className="flex items-center gap-1.5">
            {label}
            <kbd aria-hidden="true" className="hidden md:inline-flex text-[10px] font-mono px-1 py-0.5 rounded bg-surface-primary text-text-dim">
              {key}
            </kbd>
          </span>
        </button>
      ))}
    </div>
  );
}

function RefreshButton() {
  const status = useRefreshStatus();
  const refresh = useRefresh();
  const isRunning = status === "running";
  const selectedRepoId = useUIStore((s) => s.selectedRepoId);

  return (
    <button
      type="button"
      title={isRunning ? "Syncing..." : "Refresh branches and CI status"}
      disabled={isRunning}
      onClick={() => refresh.mutate({ repo: selectedRepoId ?? undefined })}
      className={`
        text-[10px] font-mono px-2.5 py-1.5 min-h-[36px] inline-flex items-center gap-1.5
        rounded border transition-colors cursor-pointer
        focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus
        disabled:cursor-not-allowed
        ${isRunning
          ? "border-accent/30 text-accent bg-accent-subtle/20"
          : "border-border text-text-muted hover:text-text-primary hover:bg-surface-secondary"}
      `}
    >
      <svg className={`w-3 h-3 ${isRunning ? "animate-spin" : ""}`} viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true">
        <path d="M2 8a6 6 0 0 1 10.3-4.1" />
        <path d="M14 2v4h-4" />
        <path d="M14 8a6 6 0 0 1-10.3 4.1" />
        <path d="M2 14v-4h4" />
      </svg>
      {isRunning ? "Syncing" : "Sync"}
    </button>
  );
}
