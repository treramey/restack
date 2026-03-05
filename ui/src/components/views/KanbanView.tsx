/**
 * Kanban view — environment lanes with topic cards.
 *
 * Layout: lanes ordered by environment ordinal (dev left → production right).
 * Cards: topic branches within each environment.
 * Interactions: click to select, h/l between lanes, j/k within lanes,
 *   Enter to view details, promote/demote/rebuild buttons.
 */

import { useMemo, useCallback, useState, useRef, useEffect } from "react";
import type {
  Topic,
  TopicId,
  Environment,
  EnvId,
  TopicEnvironment,
  Rebuild,
  Conflict,
  TopicStatus,
  CiStatus,
  RebuildStatus,
} from "../../generated/types.js";
import {
  useTopics,
  useEnvironments,
  useTopicEnvironments,
  useRebuildStatus,
  useConflicts,
  useRepos,
} from "../../lib/queries.js";
import { usePromote, useDemote, useRebuild, useCreatePr, useCloseTopic } from "../../lib/mutations.js";
import { useUIStore } from "../../lib/store.js";

// ============ Helpers ============

const STATUS_BORDER: Record<TopicStatus, string> = {
  active: "border-border",
  conflict: "border-status-conflict",
  graduated: "border-status-graduated/40",
  closed: "border-status-closed/40",
};

const STATUS_BADGE_COLORS: Record<TopicStatus, string> = {
  active: "bg-status-active/20 text-status-active border-status-active/40",
  conflict: "bg-status-conflict/20 text-status-conflict border-status-conflict/40",
  graduated: "bg-status-graduated/20 text-status-graduated border-status-graduated/40",
  closed: "bg-status-closed/20 text-status-closed border-status-closed/40",
};

const CI_BADGE_COLORS: Record<CiStatus, string> = {
  pending: "bg-status-ci-pending/20 text-status-ci-pending",
  passed: "bg-status-ci-passed/20 text-status-ci-passed",
  failed: "bg-status-ci-failed/20 text-status-ci-failed",
};

const REBUILD_COLORS: Record<RebuildStatus, string> = {
  running: "text-rebuild-running",
  success: "text-rebuild-success",
  partial: "text-rebuild-partial",
  failed: "text-rebuild-failed",
};

function topicsInEnv(
  envId: EnvId,
  topicEnvs: TopicEnvironment[],
  topics: Topic[],
): Topic[] {
  const topicIds = new Set(
    topicEnvs.filter((te) => te.envId === envId).map((te) => te.topicId),
  );
  return topics.filter((t) => topicIds.has(t.id));
}

function latestRebuild(
  envId: EnvId,
  rebuilds: Rebuild[],
): Rebuild | undefined {
  return rebuilds
    .filter((r) => r.envId === envId)
    .sort((a, b) => b.startedAt.localeCompare(a.startedAt))[0];
}

function conflictsForTopic(
  topicId: TopicId,
  conflicts: Conflict[],
): Conflict[] {
  return conflicts.filter((c) => c.topicId === topicId && !c.resolved);
}

function unassignedTopics(
  topics: Topic[],
  topicEnvs: TopicEnvironment[],
): Topic[] {
  const assignedIds = new Set(topicEnvs.map((te) => te.topicId));
  return topics.filter(
    (t) =>
      !assignedIds.has(t.id) &&
      t.status !== "closed" &&
      t.status !== "graduated",
  );
}

// ============ Main Component ============

export function KanbanView() {
  const selectedRepoId = useUIStore((s) => s.selectedRepoId);
  const selectedTopicId = useUIStore((s) => s.selectedTopicId);
  const setSelectedTopicId = useUIStore((s) => s.setSelectedTopicId);

  const { data: allTopics } = useTopics();
  const { data: allEnvironments } = useEnvironments();
  const { data: topicEnvs } = useTopicEnvironments();
  const { data: rebuilds } = useRebuildStatus();
  const { data: conflicts } = useConflicts();
  const { data: repos } = useRepos();

  const promote = usePromote();
  const demote = useDemote();
  const rebuild = useRebuild();
  const createPr = useCreatePr();
  const closeTopic = useCloseTopic();

  // Filter by selected repo
  const environments = useMemo(() => {
    if (!allEnvironments) return [];
    const filtered = selectedRepoId
      ? allEnvironments.filter((e) => e.repoId === selectedRepoId)
      : allEnvironments;
    return filtered.sort((a, b) => a.ordinal - b.ordinal);
  }, [allEnvironments, selectedRepoId]);

  const topics = useMemo(() => {
    if (!allTopics) return [];
    return selectedRepoId
      ? allTopics.filter((t) => t.repoId === selectedRepoId)
      : allTopics;
  }, [allTopics, selectedRepoId]);

  // Find next environment for promote
  const nextEnv = useCallback(
    (currentEnvId: EnvId): Environment | undefined => {
      const idx = environments.findIndex((e) => e.id === currentEnvId);
      return idx >= 0 && idx < environments.length - 1
        ? environments[idx + 1]
        : undefined;
    },
    [environments],
  );

  // Keyboard focus: [laneIndex, cardIndex]
  const [focusedLane, setFocusedLane] = useState(0);
  const [focusedCard, setFocusedCard] = useState(0);
  const cardRefs = useRef<Map<string, HTMLDivElement>>(new Map());

  // Build lane data
  const lanes = useMemo(() => {
    if (!topicEnvs) return [];
    const envLanes = environments.map((env) => ({
      env,
      topics: topicsInEnv(env.id, topicEnvs, topics),
      lastRebuild: rebuilds ? latestRebuild(env.id, rebuilds) : undefined,
    }));

    const unassigned = unassignedTopics(topics, topicEnvs);
    if (unassigned.length > 0) {
      return [
        {
          env: {
            id: "unassigned" as EnvId,
            repoId: (selectedRepoId ?? "") as any,
            name: "Unassigned",
            branch: "",
            ordinal: -1,
            autoPromote: false,
          },
          topics: unassigned,
          lastRebuild: undefined,
        },
        ...envLanes,
      ];
    }
    return envLanes;
  }, [environments, topics, topicEnvs, rebuilds, selectedRepoId]);

  // Clamp focus when lane contents change
  useEffect(() => {
    const lane = lanes[focusedLane];
    if (!lane) return;
    if (focusedCard >= lane.topics.length && lane.topics.length > 0) {
      setFocusedCard(lane.topics.length - 1);
    } else if (lane.topics.length === 0) {
      setFocusedCard(0);
    }
  }, [lanes, focusedLane, focusedCard]);

  // Focus the active card element
  useEffect(() => {
    const lane = lanes[focusedLane];
    if (!lane) return;
    const topic = lane.topics[focusedCard];
    if (topic) {
      const el = cardRefs.current.get(topic.id);
      if (el) {
        el.focus({ preventScroll: true });
        el.scrollIntoView({ block: "nearest", behavior: "auto" });
      }
    }
  }, [focusedLane, focusedCard, lanes]);

  // Keyboard handler
  useEffect(() => {
    function handleKey(e: KeyboardEvent) {
      // Don't capture if user is in an input
      if (
        e.target instanceof HTMLInputElement ||
        e.target instanceof HTMLTextAreaElement ||
        e.target instanceof HTMLSelectElement
      ) {
        return;
      }

      switch (e.key) {
        case "h":
        case "ArrowLeft":
          e.preventDefault();
          setFocusedLane((prev) => Math.max(0, prev - 1));
          break;
        case "l":
        case "ArrowRight":
          e.preventDefault();
          setFocusedLane((prev) => Math.min(lanes.length - 1, prev + 1));
          break;
        case "k":
        case "ArrowUp": {
          e.preventDefault();
          setFocusedCard((prev) => Math.max(0, prev - 1));
          break;
        }
        case "j":
        case "ArrowDown": {
          e.preventDefault();
          const lane = lanes[focusedLane];
          if (lane) {
            setFocusedCard((prev) =>
              Math.min(lane.topics.length - 1, prev + 1),
            );
          }
          break;
        }
        case "Enter": {
          e.preventDefault();
          const lane = lanes[focusedLane];
          const topic = lane?.topics[focusedCard];
          if (topic) {
            setSelectedTopicId(topic.id);
          }
          break;
        }
        case "O":
        case "o": {
          e.preventDefault();
          const lane = lanes[focusedLane];
          const topic = lane?.topics[focusedCard];
          if (topic && repos) {
            const repo = repos.find((r) => r.id === topic.repoId);
            if (repo) {
              createPr.mutate(
                { repo: topic.repoId, head: topic.branch, base: repo.baseBranch, title: topic.branch },
                { onSuccess: (pr) => { if (pr.url) window.open(pr.url, "_blank"); } },
              );
            }
          }
          break;
        }
      }
    }

    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [lanes, focusedLane, focusedCard, setSelectedTopicId, repos, createPr]);

  const handleCardRef = useCallback(
    (id: TopicId, el: HTMLDivElement | null) => {
      if (el) {
        cardRefs.current.set(id, el);
      } else {
        cardRefs.current.delete(id);
      }
    },
    [],
  );

  const handleGraduate = useCallback(
    (topic: Topic) => {
      const repo = repos?.find((r) => r.id === topic.repoId);
      if (!repo) return;
      createPr.mutate(
        { repo: topic.repoId, head: topic.branch, base: repo.baseBranch, title: topic.branch },
        {
          onSuccess: (pr) => {
            if (pr.url) window.open(pr.url, "_blank");
          },
        },
      );
    },
    [repos, createPr],
  );

  // Loading state
  if (!allTopics || !allEnvironments || !topicEnvs) {
    return (
      <div className="flex-1 flex items-center justify-center text-text-muted font-mono text-sm">
        Loading...
      </div>
    );
  }

  // Empty state
  if (environments.length === 0) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-2 text-text-muted font-mono text-sm p-8">
        <p>No environments configured</p>
        <p className="text-text-dim text-xs">
          Run <code className="text-accent">restack env add</code> to create
          one
        </p>
      </div>
    );
  }

  const isMutating =
    promote.isPending || demote.isPending || rebuild.isPending || createPr.isPending || closeTopic.isPending;

  return (
    <div className="flex-1 flex flex-col bg-bg-primary overflow-hidden">
      <div className="flex-1 flex gap-3 p-4 overflow-x-auto min-h-0">
        {lanes.map((lane, laneIndex) => (
          <LaneColumn
            key={lane.env.id}
            env={lane.env}
            topics={lane.topics}
            lastRebuild={lane.lastRebuild}
            conflicts={conflicts ?? []}
            isCurrentLane={laneIndex === focusedLane}
            focusedCardIndex={laneIndex === focusedLane ? focusedCard : null}
            selectedTopicId={selectedTopicId}
            nextEnv={nextEnv(lane.env.id)}
            isMutating={isMutating}
            isLastEnvLane={laneIndex === lanes.length - 1 && lane.env.id !== "unassigned"}
            onCardRef={handleCardRef}
            onSelect={(topic, cardIndex) => {
              setFocusedLane(laneIndex);
              setFocusedCard(cardIndex);
              setSelectedTopicId(topic.id);
            }}
            onPromote={(topicId, repoId) => {
              const next = nextEnv(lane.env.id);
              if (next) promote.mutate({ topicId, envId: next.id, repoId });
            }}
            onDemote={(topicId, repoId) => {
              if (lane.env.id !== "unassigned") {
                demote.mutate({ topicId, envId: lane.env.id, repoId });
              }
            }}
            onRebuild={() => {
              rebuild.mutate({ envId: lane.env.id });
            }}
            onGraduate={handleGraduate}
            onClose={(topicId, repoId) => closeTopic.mutate({ topicId, repoId })}
          />
        ))}
      </div>

      {/* Navigation hint */}
      <div className="px-4 py-2 border-t border-border text-xs text-text-dim font-mono flex-shrink-0">
        <span className="opacity-70">h/l</span> lanes{" "}
        <span className="opacity-70">j/k</span> navigate{" "}
        <span className="opacity-70">Enter</span> select{" "}
        <span className="opacity-70">O</span> create PR
      </div>
    </div>
  );
}

// ============ Lane Column ============

interface LaneColumnProps {
  env: Environment;
  topics: Topic[];
  lastRebuild: Rebuild | undefined;
  conflicts: Conflict[];
  isCurrentLane: boolean;
  focusedCardIndex: number | null;
  selectedTopicId: TopicId | null;
  nextEnv: Environment | undefined;
  isMutating: boolean;
  isLastEnvLane: boolean;
  onCardRef: (id: TopicId, el: HTMLDivElement | null) => void;
  onSelect: (topic: Topic, cardIndex: number) => void;
  onPromote: (topicId: TopicId, repoId: string) => void;
  onDemote: (topicId: TopicId, repoId: string) => void;
  onRebuild: () => void;
  onGraduate: (topic: Topic) => void;
  onClose: (topicId: TopicId, repoId: string) => void;
}

function LaneColumn({
  env,
  topics,
  lastRebuild,
  conflicts,
  isCurrentLane,
  focusedCardIndex,
  selectedTopicId,
  nextEnv,
  isMutating,
  isLastEnvLane,
  onCardRef,
  onSelect,
  onPromote,
  onDemote,
  onRebuild,
  onGraduate,
  onClose,
}: LaneColumnProps) {
  const isUnassigned = env.id === "unassigned";

  return (
    <div className="flex-1 min-w-[280px] max-w-[400px] flex flex-col min-h-0">
      <div
        className={`
          px-3 py-2 mb-2 rounded border flex-shrink-0
          ${isCurrentLane ? "border-accent bg-accent-subtle/30" : "border-border bg-surface-primary"}
        `}
      >
        <div className="flex items-center justify-between">
          <span
            className={`
              text-xs font-mono uppercase tracking-wider font-bold
              ${isCurrentLane ? "text-accent" : "text-text-primary"}
            `}
          >
            {env.name}
          </span>
          <div className="flex items-center gap-2">
            <span
              className={`
                text-xs font-mono px-2 py-0.5 rounded
                ${isCurrentLane ? "bg-accent/20 text-accent" : "bg-surface-secondary text-text-dim"}
              `}
            >
              {topics.length}
            </span>
            {!isUnassigned && (
              <button
                type="button"
                disabled={isMutating}
                onClick={onRebuild}
                className="text-[10px] font-mono px-1.5 py-0.5 rounded border border-border text-text-muted hover:text-text-primary hover:bg-surface-secondary transition-colors disabled:opacity-40 disabled:cursor-not-allowed cursor-pointer"
                title={`Rebuild ${env.name}`}
              >
                Rebuild
              </button>
            )}
          </div>
        </div>

        <div className="flex items-center gap-2 text-[10px] font-mono text-text-dim h-4 mt-1">
          {!isUnassigned && (
            <>
              <span>{env.branch}</span>
              {lastRebuild && (
                <span className={REBUILD_COLORS[lastRebuild.status]}>
                  {lastRebuild.status}
                </span>
              )}
            </>
          )}
        </div>
      </div>

      {/* Card list */}
      <div className="flex-1 relative min-h-0">
        <div className="absolute inset-0 overflow-y-auto">
          <div
            className="space-y-2 px-3 pb-3"
            role="list"
            aria-label={`${env.name} topics`}
          >
            {topics.length === 0 ? (
              <div className="text-text-dim text-xs font-mono text-center py-8 opacity-50">
                No topics
              </div>
            ) : (
topics.map((topic, cardIndex) => (
                 <TopicCard
                   key={topic.id}
                   topic={topic}
                   conflicts={conflictsForTopic(topic.id, conflicts)}
                   cardIndex={cardIndex}
                   isFocused={
                     isCurrentLane && focusedCardIndex === cardIndex
                   }
                   isSelected={selectedTopicId === topic.id}
                   nextEnv={nextEnv}
                   isUnassignedLane={isUnassigned}
                   isLastEnvLane={isLastEnvLane}
                   isMutating={isMutating}
                   onRef={onCardRef}
                   onSelect={onSelect}
                   onPromote={onPromote}
                   onDemote={onDemote}
                   onGraduate={onGraduate}
                   onClose={onClose}
                 />
               ))
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

// ============ Topic Card ============

interface TopicCardProps {
  topic: Topic;
  conflicts: Conflict[];
  cardIndex: number;
  isFocused: boolean;
  isSelected: boolean;
  nextEnv: Environment | undefined;
  isUnassignedLane: boolean;
  isLastEnvLane: boolean;
  isMutating: boolean;
  onRef: (id: TopicId, el: HTMLDivElement | null) => void;
  onSelect: (topic: Topic, cardIndex: number) => void;
  onPromote: (topicId: TopicId, repoId: string) => void;
  onDemote: (topicId: TopicId, repoId: string) => void;
  onGraduate: (topic: Topic) => void;
  onClose: (topicId: TopicId, repoId: string) => void;
}

function TopicCard({
  topic,
  conflicts,
  cardIndex,
  isFocused,
  isSelected,
  nextEnv,
  isUnassignedLane,
  isLastEnvLane,
  isMutating,
  onRef,
  onSelect,
  onPromote,
  onDemote,
  onGraduate,
  onClose,
}: TopicCardProps) {
  const [showCloseConfirm, setShowCloseConfirm] = useState(false);
  const [closeInput, setCloseInput] = useState("");

  const handleRef = useCallback(
    (el: HTMLDivElement | null) => {
      onRef(topic.id, el);
    },
    [topic.id, onRef],
  );

  const isGraduated = topic.status === "graduated";
  const isConflict = topic.status === "conflict";

  return (
    <div role="listitem">
      <div
        ref={handleRef}
        role="button"
        tabIndex={isFocused ? 0 : -1}
        aria-label={topic.branch}
        aria-current={isSelected ? "true" : undefined}
        onClick={() => onSelect(topic, cardIndex)}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            onSelect(topic, cardIndex);
          }
        }}
        className={`
          p-3 rounded border transition-colors cursor-pointer
          focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent focus-visible:ring-offset-2 focus-visible:ring-offset-bg-primary
          ${isSelected ? "border-accent bg-accent-subtle/30" : `border-border ${STATUS_BORDER[topic.status]} bg-surface-primary hover:bg-surface-secondary`}
          ${isConflict ? "border-status-conflict" : ""}
          ${isGraduated ? "opacity-60" : ""}
        `}
      >
        <div
          className={`
            text-sm font-mono leading-tight mb-2 truncate
            ${isGraduated ? "text-text-muted" : "text-text-primary"}
          `}
        >
          {topic.branch}
        </div>

          {/* Status badges row */}
          <div className="flex items-center gap-1.5 mb-2 flex-wrap">
            {topic.status !== "active" && topic.status !== "closed" && (
              <span
                className={`px-1.5 py-0.5 rounded border text-[10px] font-mono uppercase tracking-wider ${STATUS_BADGE_COLORS[topic.status]}`}
              >
                {topic.status}
              </span>
            )}
            {topic.ciStatus &&
              (topic.ciUrl ? (
                <a
                  href={topic.ciUrl}
                  target="_blank"
                  rel="noopener noreferrer"
                  className={`px-1.5 py-0.5 rounded text-[10px] font-mono uppercase hover:underline ${CI_BADGE_COLORS[topic.ciStatus]}`}
                  onClick={(e) => e.stopPropagation()}
                >
                  CI: {topic.ciStatus}
                </a>
              ) : (
                <span
                  className={`px-1.5 py-0.5 rounded text-[10px] font-mono uppercase ${CI_BADGE_COLORS[topic.ciStatus]}`}
                >
                  CI: {topic.ciStatus}
                </span>
              ))}
          </div>

          {/* Conflict details */}
          {conflicts.length > 0 && (
            <div className="text-[10px] font-mono text-status-conflict mb-2">
              {conflicts.length} conflict{conflicts.length > 1 ? "s" : ""}
              {conflicts[0]?.conflictedWith
                ? ` (with ${conflicts[0].conflictedWith})`
                : ""}
            </div>
          )}

          {/* PR link */}
          {topic.prUrl && (
            <div className="mb-2">
              <a
                href={topic.prUrl}
                target="_blank"
                rel="noopener noreferrer"
                className="text-[10px] font-mono text-accent hover:underline"
                onClick={(e) => e.stopPropagation()}
              >
                PR #{topic.prId}
              </a>
            </div>
          )}

          {/* Action buttons */}
          <div
            className="flex items-center gap-1.5 pt-1 border-t border-border/50"
            onClick={(e) => e.stopPropagation()}
            onKeyDown={(e) => e.stopPropagation()}
          >
            {nextEnv && !isGraduated && !isUnassignedLane && (
              <ActionButton
                label={`→ ${nextEnv.name}`}
                title={`Promote to ${nextEnv.name}`}
                disabled={isMutating}
                onClick={() => onPromote(topic.id, topic.repoId)}
              />
            )}
            {isLastEnvLane && !isGraduated && (
              <ActionButton
                label="Graduate →"
                title="Create PR to merge into master"
                disabled={isMutating}
                onClick={() => onGraduate(topic)}
              />
            )}
            {!isGraduated && !isUnassignedLane && (
              <ActionButton
                label="Archive"
                title="Remove from environment"
                disabled={isMutating}
                onClick={() => onDemote(topic.id, topic.repoId)}
                variant="danger"
              />
            )}
            {isUnassignedLane && !isGraduated && (
              <ActionButton
                label="Close"
                title="Delete branch from origin and local"
                disabled={isMutating}
                onClick={() => setShowCloseConfirm(true)}
                variant="danger"
              />
            )}
          </div>

          {/* Close confirmation modal */}
          {showCloseConfirm && (
            <div
              className="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
              onClick={(e) => { e.stopPropagation(); setShowCloseConfirm(false); setCloseInput(""); }}
              onKeyDown={(e) => e.stopPropagation()}
            >
              <div
                className="bg-surface-primary border border-border rounded-lg p-4 w-[400px] shadow-xl"
                onClick={(e) => e.stopPropagation()}
              >
                <h3 className="text-sm font-mono font-bold text-text-primary mb-2">
                  Delete branch?
                </h3>
                <p className="text-xs font-mono text-text-muted mb-3">
                  This will delete <span className="text-status-conflict font-bold">{topic.branch}</span> from
                  both origin and local. This cannot be undone.
                </p>
                <p className="text-xs font-mono text-text-dim mb-2">
                  Type the branch name to confirm:
                </p>
                <input
                  type="text"
                  value={closeInput}
                  onChange={(e) => setCloseInput(e.target.value)}
                  onKeyDown={(e) => {
                    e.stopPropagation();
                    if (e.key === "Escape") { setShowCloseConfirm(false); setCloseInput(""); }
                    if (e.key === "Enter" && closeInput === topic.branch) {
                      onClose(topic.id, topic.repoId);
                      setShowCloseConfirm(false);
                      setCloseInput("");
                    }
                  }}
                  className="w-full px-2 py-1.5 rounded border border-border bg-bg-primary text-text-primary font-mono text-xs focus:outline-none focus:ring-1 focus:ring-accent mb-3"
                  placeholder={topic.branch}
                  autoFocus
                />
                <div className="flex justify-end gap-2">
                  <button
                    type="button"
                    onClick={() => { setShowCloseConfirm(false); setCloseInput(""); }}
                    className="text-xs font-mono px-3 py-1 rounded border border-border text-text-muted hover:text-text-primary hover:bg-surface-secondary cursor-pointer"
                  >
                    Cancel
                  </button>
                  <button
                    type="button"
                    disabled={closeInput !== topic.branch}
                    onClick={() => {
                      onClose(topic.id, topic.repoId);
                      setShowCloseConfirm(false);
                      setCloseInput("");
                    }}
                    className="text-xs font-mono px-3 py-1 rounded border border-status-conflict/50 text-status-conflict hover:bg-status-conflict/10 cursor-pointer disabled:opacity-30 disabled:cursor-not-allowed"
                  >
                    Delete branch
                  </button>
                </div>
              </div>
            </div>
          )}
        </div>
    </div>
  );
}

// ============ Action Button ============

function ActionButton({
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
        text-[10px] font-mono px-1.5 py-0.5 rounded border cursor-pointer
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
