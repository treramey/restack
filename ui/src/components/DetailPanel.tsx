/**
 * Collapsible detail panel for topic/environment details.
 * Positioned at bottom, resizable via drag handle.
 */

import { useRef, useEffect, useMemo, useState } from "react";
import {
  useUIStore,
  clampPanelHeight,
  usePanelHeightClamp,
  PANEL_HEIGHT_MIN,
  PANEL_HEIGHT_MAX_VH,
} from "../lib/store.js";
import { useTopics, useEnvironments, useTopicEnvironments, useRepos, useConflicts } from "../lib/queries.js";
import { usePromote, useDemote, useCreatePr, useCloseTopic } from "../lib/mutations.js";
import { STATUS_BADGE, CI_BADGE } from "../lib/badges.js";
import { Badge } from "./Badge.js";
import type { TopicStatus, CiStatus } from "../generated/types.js";

/** Map an environment name to its CSS color variable value. */
function getEnvColor(name: string): string {
  const lower = name.toLowerCase();
  if (lower.includes("dev")) return "var(--color-env-dev)";
  if (lower.includes("stag")) return "var(--color-env-staging)";
  if (lower.includes("prod")) return "var(--color-env-production)";
  return "var(--color-accent)";
}

const COLLAPSED_HEIGHT = 40;

/** Derive panel max height from viewport, updating on resize. */
function usePanelMaxPx(): number {
  const [maxPx, setMaxPx] = useState(() => Math.floor(window.innerHeight * PANEL_HEIGHT_MAX_VH));
  useEffect(() => {
    const update = () => setMaxPx(Math.floor(window.innerHeight * PANEL_HEIGHT_MAX_VH));
    window.addEventListener("resize", update);
    return () => window.removeEventListener("resize", update);
  }, []);
  return maxPx;
}

export function DetailPanel() {
  const selectedTopicId = useUIStore((s) => s.selectedTopicId);
  const detailPanelOpen = useUIStore((s) => s.detailPanelOpen);
  const panelHeight = useUIStore((s) => s.panelHeight);
  const toggleDetailPanel = useUIStore((s) => s.toggleDetailPanel);
  const setPanelHeight = useUIStore((s) => s.setPanelHeight);
  const panelMaxPx = usePanelMaxPx();

  const { data: topics } = useTopics();
  const { data: environments } = useEnvironments();
  const { data: topicEnvs } = useTopicEnvironments();
  const { data: repos } = useRepos();
  const { data: conflicts } = useConflicts();

  const promote = usePromote();
  const demote = useDemote();
  const createPr = useCreatePr();
  const closeTopic = useCloseTopic();

  usePanelHeightClamp();

  const [showCloseConfirm, setShowCloseConfirm] = useState(false);
  const [closeInput, setCloseInput] = useState("");

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

  // Environments for this repo (all, sorted by ordinal) for action logic
  const repoEnvironments = useMemo(() => {
    if (!environments || !selectedTopic) return [];
    return environments
      .filter((e) => e.repoId === selectedTopic.repoId)
      .sort((a, b) => a.ordinal - b.ordinal);
  }, [environments, selectedTopic]);

  // Topic's environments sorted by ordinal
  const topicEnvironmentsSorted = useMemo(
    () => [...topicEnvironments].sort((a, b) => a.ordinal - b.ordinal),
    [topicEnvironments],
  );

  const highestEnv = topicEnvironmentsSorted[topicEnvironmentsSorted.length - 1] ?? null;
  const maxOrdinalEnv = repoEnvironments[repoEnvironments.length - 1] ?? null;
  const isInLastEnv = highestEnv !== null && maxOrdinalEnv !== null && highestEnv.id === maxOrdinalEnv.id;
  const nextEnv = highestEnv !== null
    ? repoEnvironments.find((e) => e.ordinal === highestEnv.ordinal + 1) ?? null
    : null;
  const firstEnv = repoEnvironments[0] ?? null;
  const isUnassigned = topicEnvironments.length === 0;
  const isGraduated = selectedTopic?.status === "graduated";
  const isMutating = promote.isPending || demote.isPending || createPr.isPending || closeTopic.isPending;

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
        newHeight = panelMaxPx;
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
          aria-valuemax={panelMaxPx}
          aria-label="Resize panel"
          tabIndex={0}
          className="h-4 cursor-ns-resize bg-transparent hover:bg-accent-subtle active:bg-accent-muted focus-visible:bg-accent-subtle focus-visible:ring-2 focus-visible:ring-border-focus transition-colors shrink-0 touch-none select-none flex items-center justify-center"
          onPointerDown={handlePointerDown}
          onPointerMove={handlePointerMove}
          onPointerUp={handlePointerUp}
          onPointerCancel={handlePointerUp}
          onLostPointerCapture={handlePointerUp}
          onKeyDown={handleResizeKeyDown}
        >
          <div className="w-8 h-0.5 rounded-full bg-border" aria-hidden="true" />
        </div>
      )}

      {/* Toggle bar */}
      <button
        className="h-10 px-4 flex items-center justify-between shrink-0 hover:bg-surface-primary transition-colors duration-150 cursor-pointer w-full text-left focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus focus-visible:ring-inset"
        onClick={toggleDetailPanel}
        aria-expanded={detailPanelOpen}
        aria-controls="detail-panel-content"
        aria-label="Toggle detail panel"
      >
        <div className="flex items-center gap-2">
          <svg
            width="12"
            height="12"
            viewBox="0 0 12 12"
            fill="none"
            aria-hidden="true"
            className={`transition-transform duration-150 text-text-muted ${detailPanelOpen ? "rotate-180" : ""}`}
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
        <kbd aria-hidden="true" className="text-[10px] font-mono px-1.5 py-0.5 rounded bg-surface-primary text-text-dim border border-border">
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
            <dl className="h-full overflow-y-auto p-4 space-y-4 animate-fade-in">
              <div className="space-y-1">
                <dt className="text-[10px] font-mono text-text-dim uppercase tracking-wider">Actions</dt>
                <dd className="flex items-center gap-2 flex-wrap">
                  {/* Promote: in an env and there's a next env */}
                  {highestEnv !== null && nextEnv !== null && !isGraduated && (
                    <DetailActionButton
                      label={`→ ${nextEnv.name}`}
                      title={`Promote to ${nextEnv.name}`}
                      disabled={isMutating}
                      onClick={() => {
                        if (selectedTopic && nextEnv) {
                          promote.mutate({ topicId: selectedTopic.id, envId: nextEnv.id, repoId: selectedTopic.repoId });
                        }
                      }}
                    />
                  )}
                  {/* Graduate: in last env and not graduated */}
                  {isInLastEnv && !isGraduated && selectedTopic && selectedRepo && (
                    <DetailActionButton
                      label="Create PR →"
                      title="Create PR to merge into base branch"
                      disabled={isMutating}
                      onClick={() => {
                        if (selectedTopic && selectedRepo) {
                          createPr.mutate(
                            { repo: selectedTopic.repoId, head: selectedTopic.branch, base: selectedRepo.baseBranch, title: selectedTopic.branch },
                            { onSuccess: (pr) => { if (pr.url) window.open(pr.url, "_blank"); } },
                          );
                        }
                      }}
                    />
                  )}
                  {/* Archive: move back to unassigned (remove from environment) */}
                  {highestEnv !== null && !isGraduated && (
                    <DetailActionButton
                      label="Archive"
                      title="Move back to unassigned"
                      disabled={isMutating}
                      variant="danger"
                      onClick={() => {
                        if (selectedTopic && highestEnv) {
                          demote.mutate({ topicId: selectedTopic.id, envId: highestEnv.id, repoId: selectedTopic.repoId });
                        }
                      }}
                    />
                  )}
                  {/* Promote to first env: unassigned and not graduated */}
                  {isUnassigned && !isGraduated && firstEnv !== null && selectedTopic && (
                    <DetailActionButton
                      label={`→ ${firstEnv.name}`}
                      title={`Promote to ${firstEnv.name}`}
                      disabled={isMutating}
                      onClick={() => {
                        if (selectedTopic && firstEnv) {
                          promote.mutate({ topicId: selectedTopic.id, envId: firstEnv.id, repoId: selectedTopic.repoId });
                        }
                      }}
                    />
                  )}
                  {/* Close: delete branch on local and origin */}
                  {isUnassigned && !isGraduated && selectedTopic && (
                    <DetailActionButton
                      label="Close"
                      title="Delete branch from local and origin"
                      disabled={isMutating}
                      variant="danger"
                      onClick={() => setShowCloseConfirm(true)}
                    />
                  )}
                  {/* Clean up branch: graduated and unassigned */}
                  {isUnassigned && isGraduated && selectedTopic && (
                    <DetailActionButton
                      label="Clean up branch"
                      title="Delete branch (already merged)"
                      disabled={isMutating}
                      onClick={() => {
                        if (selectedTopic) {
                          closeTopic.mutate({ topicId: selectedTopic.id, repoId: selectedTopic.repoId });
                        }
                      }}
                    />
                  )}
                </dd>
              </div>

              {/* Inline close confirmation */}
              {showCloseConfirm && selectedTopic && (
                <CloseConfirmInline
                  branch={selectedTopic.branch}
                  closeInput={closeInput}
                  disabled={isMutating}
                  onInputChange={setCloseInput}
                  onConfirm={() => {
                    if (selectedTopic) {
                      closeTopic.mutate({ topicId: selectedTopic.id, repoId: selectedTopic.repoId });
                      setShowCloseConfirm(false);
                      setCloseInput("");
                    }
                  }}
                  onCancel={() => { setShowCloseConfirm(false); setCloseInput(""); }}
                />
              )}

              <div className="grid grid-cols-2 gap-4">
                <div className="space-y-1">
                  <dt className="text-[10px] font-mono text-text-dim uppercase tracking-wider">Branch</dt>
                  <dd className="text-sm font-mono text-text-primary truncate">{selectedTopic.branch}</dd>
                </div>
                <div className="space-y-1">
                  <dt className="text-[10px] font-mono text-text-dim uppercase tracking-wider">Repo</dt>
                  <dd className="text-sm font-mono text-text-primary">{selectedRepo?.name ?? "—"}</dd>
                </div>
                {selectedTopic.status !== "active" && (
                  <div className="space-y-1">
                    <dt className="text-[10px] font-mono text-text-dim uppercase tracking-wider">Status</dt>
                    <dd><Badge label={selectedTopic.status} colorClass={STATUS_BADGE[selectedTopic.status as TopicStatus] ?? STATUS_BADGE.closed} /></dd>
                  </div>
                )}
                <div className="space-y-1">
                  <dt className="text-[10px] font-mono text-text-dim uppercase tracking-wider">Age</dt>
                  <dd className="text-sm font-mono text-text-dim">{age}</dd>
                </div>
              </div>

              <div className="space-y-1">
                <dt className="text-[10px] font-mono text-text-dim uppercase tracking-wider">Environment</dt>
                <dd>
                  {topicEnvironments.length > 0 ? (
                    <div className="flex flex-wrap gap-2">
                      {topicEnvironments
                        .sort((a, b) => a.ordinal - b.ordinal)
                        .map((env) => (
                          <span
                            key={env.id}
                            className="text-xs font-mono px-2 py-1 rounded border"
                            style={{
                              color: getEnvColor(env.name),
                              borderColor: `color-mix(in srgb, ${getEnvColor(env.name)} 40%, transparent)`,
                              backgroundColor: `color-mix(in srgb, ${getEnvColor(env.name)} 10%, transparent)`,
                            }}
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
                </dd>
              </div>

              {selectedTopic.ciStatus && (
                <div className="space-y-1">
                  <dt className="text-[10px] font-mono text-text-dim uppercase tracking-wider">CI Status</dt>
                  <dd className="flex items-center gap-3">
                    <Badge label={selectedTopic.ciStatus} colorClass={CI_BADGE[selectedTopic.ciStatus as CiStatus] ?? CI_BADGE.pending} />
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
                  </dd>
                </div>
              )}

              {selectedTopic.prUrl && (
                <div className="space-y-1">
                  <dt className="text-[10px] font-mono text-text-dim uppercase tracking-wider">Pull Request</dt>
                  <dd>
                    <a
                      href={selectedTopic.prUrl}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-sm font-mono text-accent hover:underline"
                    >
                      #{selectedTopic.prId} → {selectedTopic.prUrl.split("/").slice(-2).join("/")}
                    </a>
                  </dd>
                </div>
              )}

              {topicConflicts.length > 0 && (
                <div className="space-y-1">
                  <dt className="text-[10px] font-mono text-status-conflict uppercase tracking-wider">Conflicts</dt>
                  <dd className="text-xs font-mono text-status-conflict">
                    {topicConflicts.length} unresolved conflict{topicConflicts.length > 1 ? "s" : ""}
                  </dd>
                </div>
              )}
            </dl>
          ) : (
            <div className="h-full flex flex-col items-center justify-center gap-1 animate-fade-in">
              <span className="text-text-muted text-sm font-mono">No topic selected</span>
              <span className="text-text-dim text-xs font-mono">Click a card or press <kbd aria-hidden="true" className="px-1 py-0.5 rounded bg-surface-primary border border-border text-[10px]">Enter</kbd> to inspect</span>
            </div>
          )}
        </div>
      )}
    </div>
  );
}


function CloseConfirmInline({
  branch,
  closeInput,
  disabled,
  onInputChange,
  onConfirm,
  onCancel,
}: {
  branch: string;
  closeInput: string;
  disabled: boolean;
  onInputChange: (value: string) => void;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") {
        onCancel();
        return;
      }
      if (e.key !== "Tab" || !container) return;

      const focusable = container.querySelectorAll<HTMLElement>(
        'input, button, [tabindex]:not([tabindex="-1"])',
      );
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (!first || !last) return;

      if (e.shiftKey) {
        if (document.activeElement === first) {
          e.preventDefault();
          last.focus();
        }
      } else {
        if (document.activeElement === last) {
          e.preventDefault();
          first.focus();
        }
      }
    }

    container.addEventListener("keydown", handleKeyDown);
    return () => container.removeEventListener("keydown", handleKeyDown);
  }, [onCancel]);

  return (
    <div ref={containerRef} className="space-y-2 p-3 rounded border border-status-conflict/30 bg-status-conflict/5">
      <p className="text-xs font-mono text-text-muted">
        Delete <span className="text-status-conflict font-bold">{branch}</span> from origin and local? This cannot be undone.
      </p>
      <p className="text-[10px] font-mono text-text-dim">Type the branch name to confirm:</p>
      <input
        type="text"
        value={closeInput}
        onChange={(e) => onInputChange(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && closeInput === branch) {
            onConfirm();
          }
        }}
        className="w-full px-2 py-1.5 rounded border border-border bg-bg-primary text-text-primary font-mono text-xs focus:outline-none focus:ring-1 focus:ring-accent"
        placeholder={branch}
        autoFocus
      />
      <div className="flex gap-2">
        <button
          type="button"
          onClick={onCancel}
          className="text-[10px] font-mono px-2.5 py-1.5 min-h-[36px] inline-flex items-center rounded border border-border text-text-muted hover:text-text-primary hover:bg-surface-secondary transition-colors cursor-pointer"
        >
          Cancel
        </button>
        <button
          type="button"
          disabled={closeInput !== branch || disabled}
          onClick={onConfirm}
          className="text-[10px] font-mono px-2.5 py-1.5 min-h-[36px] inline-flex items-center rounded border border-status-conflict/50 text-status-conflict hover:bg-status-conflict/10 transition-colors cursor-pointer disabled:opacity-30 disabled:cursor-not-allowed"
        >
          Delete branch
        </button>
      </div>
    </div>
  );
}

function DetailActionButton({
  label,
  title,
  disabled,
  onClick,
  variant = "default",
}: {
  label: string;
  title: string;
  disabled: boolean;
  onClick: () => void;
  variant?: "default" | "danger";
}) {
  return (
    <button
      type="button"
      title={title}
      disabled={disabled}
      onClick={onClick}
      className={`
        text-[10px] font-mono px-2.5 py-1.5 min-h-[36px] inline-flex items-center rounded border cursor-pointer
        transition-colors disabled:opacity-40 disabled:cursor-not-allowed
        ${
          variant === "danger"
            ? "border-status-conflict/30 text-status-conflict/70 hover:text-status-conflict hover:bg-status-conflict/10"
            : "border-border text-text-muted hover:text-text-primary hover:bg-surface-secondary"
        }
      `}
    >
      {label}
    </button>
  );
}
