/**
 * Collapsible detail panel for topic/environment details.
 * Positioned at bottom, resizable via drag handle.
 */

import { useRef, useEffect, useMemo } from "react";
import {
  useUIStore,
  clampPanelHeight,
  PANEL_HEIGHT_MIN,
  PANEL_HEIGHT_MAX_VH,
} from "../lib/store.js";
import { useTopics, useEnvironments, useTopicEnvironments, useRepos, useConflicts } from "../lib/queries.js";

const COLLAPSED_HEIGHT = 40;

export function DetailPanel() {
  const selectedTopicId = useUIStore((s) => s.selectedTopicId);
  const detailPanelOpen = useUIStore((s) => s.detailPanelOpen);
  const panelHeight = useUIStore((s) => s.panelHeight);
  const toggleDetailPanel = useUIStore((s) => s.toggleDetailPanel);
  const setPanelHeight = useUIStore((s) => s.setPanelHeight);

  const { data: topics } = useTopics();
  const { data: environments } = useEnvironments();
  const { data: topicEnvs } = useTopicEnvironments();
  const { data: repos } = useRepos();
  const { data: conflicts } = useConflicts();

  const selectedTopic = selectedTopicId
    ? topics?.find((t) => t.id === selectedTopicId) ?? null
    : null;

  const selectedRepo = selectedTopic
    ? repos?.find((r) => r.id === selectedTopic.repoId) ?? null
    : null;

  const topicEnvIds = selectedTopicId
    ? (topicEnvs?.filter((te) => te.topicId === selectedTopicId).map((te) => te.envId) ?? [])
    : [];

  const topicEnvironments = environments?.filter((e) => topicEnvIds.includes(e.id)) ?? [];

  const topicConflicts = useMemo(() => {
    if (!selectedTopicId || !conflicts) return [];
    return conflicts.filter((c) => c.topicId === selectedTopicId && !c.resolved);
  }, [selectedTopicId, conflicts]);

  const age = useMemo(() => {
    if (!selectedTopic) return null;
    const created = new Date(selectedTopic.createdAt);
    const now = new Date();
    const diffMs = now.getTime() - created.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    const diffHours = Math.floor(diffMins / 60);
    const diffDays = Math.floor(diffHours / 24);
    
    if (diffDays > 0) return `${diffDays}d ${diffHours % 24}h`;
    if (diffHours > 0) return `${diffHours}h ${diffMins % 60}m`;
    if (diffMins > 0) return `${diffMins}m`;
    return "just now";
  }, [selectedTopic]);

  const panelRef = useRef<HTMLDivElement>(null);
  const isDragging = useRef(false);
  const dragStartY = useRef(0);
  const dragStartHeight = useRef(0);

  useEffect(() => {
    if (!detailPanelOpen && isDragging.current) {
      isDragging.current = false;
      if (panelRef.current) {
        panelRef.current.style.transition = "";
      }
    }
  }, [detailPanelOpen]);

  const handlePointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!detailPanelOpen || !panelRef.current) return;
    isDragging.current = true;
    dragStartY.current = e.clientY;
    dragStartHeight.current = panelHeight;
    panelRef.current.style.transition = "none";
    e.currentTarget.setPointerCapture(e.pointerId);
  };

  const handlePointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!isDragging.current || !panelRef.current) return;
    const delta = dragStartY.current - e.clientY;
    const newHeight = clampPanelHeight(dragStartHeight.current + delta);
    panelRef.current.style.height = `${String(newHeight)}px`;
  };

  const handlePointerUp = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!isDragging.current || !panelRef.current) return;
    panelRef.current.style.transition = "";
    setPanelHeight(panelRef.current.offsetHeight);
    isDragging.current = false;
    if (e.currentTarget.hasPointerCapture(e.pointerId)) {
      e.currentTarget.releasePointerCapture(e.pointerId);
    }
  };

  const handleResizeKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    const step = e.shiftKey ? 50 : 10;
    let newHeight = panelHeight;

    switch (e.key) {
      case "ArrowUp":
        newHeight = panelHeight + step;
        break;
      case "ArrowDown":
        newHeight = panelHeight - step;
        break;
      case "Home":
        newHeight = Math.floor(window.innerHeight * PANEL_HEIGHT_MAX_VH);
        break;
      case "End":
        newHeight = PANEL_HEIGHT_MIN;
        break;
      default:
        return;
    }

    e.preventDefault();
    setPanelHeight(clampPanelHeight(newHeight));
  };

  const height = detailPanelOpen ? panelHeight : COLLAPSED_HEIGHT;

  return (
    <div
      ref={panelRef}
      className="border-t border-border bg-bg-secondary transition-[height] duration-150 flex flex-col"
      style={{ height }}
    >
      {/* Resize handle */}
      {detailPanelOpen && (
        <div
          role="separator"
          aria-orientation="horizontal"
          aria-valuenow={panelHeight}
          aria-valuemin={PANEL_HEIGHT_MIN}
          aria-valuemax={Math.floor(window.innerHeight * PANEL_HEIGHT_MAX_VH)}
          aria-label="Resize panel"
          tabIndex={0}
          className="h-2 cursor-ns-resize bg-transparent hover:bg-accent-subtle active:bg-accent-muted focus-visible:bg-accent-subtle focus-visible:ring-2 focus-visible:ring-border-focus transition-colors shrink-0 touch-none select-none"
          onPointerDown={handlePointerDown}
          onPointerMove={handlePointerMove}
          onPointerUp={handlePointerUp}
          onPointerCancel={handlePointerUp}
          onLostPointerCapture={handlePointerUp}
          onKeyDown={handleResizeKeyDown}
        />
      )}

      {/* Toggle bar */}
      <button
        className="h-10 px-4 flex items-center justify-between shrink-0 hover:bg-surface-primary transition-colors duration-150 cursor-pointer w-full text-left focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus focus-visible:ring-inset"
        onClick={toggleDetailPanel}
        aria-expanded={detailPanelOpen}
        aria-controls="detail-panel-content"
      >
        <div className="flex items-center gap-2">
          <svg
            width="12"
            height="12"
            viewBox="0 0 12 12"
            fill="none"
            aria-hidden="true"
            className={`transition-transform text-text-muted ${detailPanelOpen ? "rotate-180" : ""}`}
          >
            <path
              d="M2 8L6 4L10 8"
              stroke="currentColor"
              strokeWidth="1.5"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
          <span className="text-sm font-mono text-text-muted">
            {selectedTopic ? selectedTopic.branch : "No topic selected"}
          </span>
        </div>
        <kbd className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-surface-primary text-text-dim border border-border">
          D
        </kbd>
      </button>

      {/* Content */}
      {detailPanelOpen && (
        <div
          id="detail-panel-content"
          className="flex-1 overflow-hidden border-t border-border"
        >
          {selectedTopic ? (
            <div className="h-full overflow-y-auto p-4 space-y-4">
              <div className="grid grid-cols-2 gap-4">
                <div className="space-y-1">
                  <label className="text-[10px] font-mono text-text-dim uppercase tracking-wider">Branch</label>
                  <div className="text-sm font-mono text-text-primary truncate">{selectedTopic.branch}</div>
                </div>
                <div className="space-y-1">
                  <label className="text-[10px] font-mono text-text-dim uppercase tracking-wider">Repo</label>
                  <div className="text-sm font-mono text-text-primary">{selectedRepo?.name ?? "—"}</div>
                </div>
                {selectedTopic.status !== "active" && (
                  <div className="space-y-1">
                    <label className="text-[10px] font-mono text-text-dim uppercase tracking-wider">Status</label>
                    <StatusBadge label={selectedTopic.status} variant={topicStatusVariant(selectedTopic.status)} />
                  </div>
                )}
                <div className="space-y-1">
                  <label className="text-[10px] font-mono text-text-dim uppercase tracking-wider">Age</label>
                  <div className="text-sm font-mono text-text-dim">{age}</div>
                </div>
              </div>

              <div className="space-y-1">
                <label className="text-[10px] font-mono text-text-dim uppercase tracking-wider">Environment</label>
                {topicEnvironments.length > 0 ? (
                  <div className="flex flex-wrap gap-2">
                    {topicEnvironments
                      .sort((a, b) => a.ordinal - b.ordinal)
                      .map((env) => (
                        <span
                          key={env.id}
                          className="text-xs font-mono px-2 py-1 rounded bg-accent-subtle text-accent border border-accent/40"
                        >
                          {env.name}
                          {env.branch !== env.name && (
                            <span className="text-text-dim ml-1">({env.branch})</span>
                          )}
                        </span>
                      ))}
                  </div>
                ) : (
                  <div className="text-xs font-mono text-text-dim italic">
                    Not promoted to any environment
                  </div>
                )}
              </div>

              {selectedTopic.ciStatus && (
                <div className="space-y-1">
                  <label className="text-[10px] font-mono text-text-dim uppercase tracking-wider">CI Status</label>
                  <div className="flex items-center gap-3">
                    <StatusBadge label={selectedTopic.ciStatus} variant={ciStatusVariant(selectedTopic.ciStatus)} />
                    {selectedTopic.ciUrl && (
                      <a
                        href={selectedTopic.ciUrl}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="text-xs font-mono text-accent hover:underline"
                      >
                        View CI →
                      </a>
                    )}
                  </div>
                </div>
              )}

              {selectedTopic.prUrl && (
                <div className="space-y-1">
                  <label className="text-[10px] font-mono text-text-dim uppercase tracking-wider">Pull Request</label>
                  <a
                    href={selectedTopic.prUrl}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-sm font-mono text-accent hover:underline"
                  >
                    #{selectedTopic.prId} → {selectedTopic.prUrl.split("/").slice(-2).join("/")}
                  </a>
                </div>
              )}

              {topicConflicts.length > 0 && (
                <div className="space-y-1">
                  <label className="text-[10px] font-mono text-status-conflict uppercase tracking-wider">Conflicts</label>
                  <div className="text-xs font-mono text-status-conflict">
                    {topicConflicts.length} unresolved conflict{topicConflicts.length > 1 ? "s" : ""}
                  </div>
                </div>
              )}
            </div>
          ) : (
            <div className="h-full flex items-center justify-center text-text-muted text-sm font-mono">
              Select a topic to view details
            </div>
          )}
        </div>
      )}
    </div>
  );
}

type BadgeVariant = "active" | "conflict" | "graduated" | "closed" | "pending" | "passed" | "failed";

function topicStatusVariant(status: string): BadgeVariant {
  switch (status) {
    case "active": return "active";
    case "conflict": return "conflict";
    case "graduated": return "graduated";
    case "closed": return "closed";
    default: return "closed";
  }
}

function ciStatusVariant(status: string): BadgeVariant {
  switch (status) {
    case "pending": return "pending";
    case "passed": return "passed";
    case "failed": return "failed";
    default: return "pending";
  }
}

const BADGE_COLORS: Record<BadgeVariant, string> = {
  active: "bg-status-active/20 text-status-active border-status-active/40",
  conflict: "bg-status-conflict/20 text-status-conflict border-status-conflict/40",
  graduated: "bg-status-graduated/20 text-status-graduated border-status-graduated/40",
  closed: "bg-status-closed/20 text-status-closed border-status-closed/40",
  pending: "bg-status-ci-pending/20 text-status-ci-pending border-status-ci-pending/40",
  passed: "bg-status-ci-passed/20 text-status-ci-passed border-status-ci-passed/40",
  failed: "bg-status-ci-failed/20 text-status-ci-failed border-status-ci-failed/40",
};

function StatusBadge({ label, variant }: { label: string; variant: BadgeVariant }) {
  return (
    <span className={`px-2 py-0.5 rounded border text-[10px] font-mono uppercase tracking-wider ${BADGE_COLORS[variant]}`}>
      {label}
    </span>
  );
}
