/**
 * Header: [RESTACK logo] [Kanban|Canvas|List tabs] [repo selector] [spacer] [status]
 */

import { useUIStore, type ViewMode } from "../lib/store.js";
import { useRepos } from "../lib/queries.js";
import { isRepoId } from "../generated/types.js";

const VIEW_TABS: ReadonlyArray<{ mode: ViewMode; label: string; key: string }> = [
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
        <div className="inline-flex h-9 rounded border border-border bg-bg-secondary overflow-hidden divide-x divide-border">
          {VIEW_TABS.map(({ mode, label, key }) => (
            <button
              key={mode}
              aria-pressed={viewMode === mode}
              className={`
                h-9 px-3 inline-flex items-center justify-center
                text-[13px] leading-none font-mono border-0
                transition-colors duration-150 cursor-pointer
                focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus
                ${viewMode === mode
                  ? "bg-accent-subtle text-accent"
                  : "text-text-muted hover:text-text-primary hover:bg-surface-primary/60"}
              `}
              onClick={() => setViewMode(mode)}
            >
              <span className="flex items-center gap-1.5">
                {label}
                <kbd className="hidden md:inline-flex text-[10px] font-mono px-1 py-0.5 rounded bg-surface-primary text-text-dim">
                  {key}
                </kbd>
              </span>
            </button>
          ))}
        </div>
      </nav>

      {/* Repo selector */}
      {repos && repos.length > 0 && (
        <select
          value={selectedRepoId ?? ""}
          onChange={handleRepoChange}
          className="h-9 px-2 text-[13px] font-mono bg-surface-primary text-text-primary border border-border rounded focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus cursor-pointer"
        >
          <option value="">All repos</option>
          {repos.map((repo) => (
            <option key={repo.id} value={repo.id}>
              {repo.name}
            </option>
          ))}
        </select>
      )}

      {/* Spacer */}
      <div className="flex-1" />

      {/* Repo count */}
      {repos && (
        <span className="text-xs text-text-dim font-mono">
          {repos.length} repo{repos.length !== 1 ? "s" : ""}
        </span>
      )}
    </header>
  );
}
