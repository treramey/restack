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
import { STATUS_BORDER, STATUS_BADGE, CI_BADGE, ORIGIN_BADGE, REBUILD_COLOR } from "../../lib/badges.js";

/** Lightweight header for a kanban lane — only the fields LaneColumn reads. */
interface LaneHeader {
  readonly id: string;
  readonly name: string;
  readonly branch: string;
}

interface Lane {
  header: LaneHeader;
  env: Environment | null;
  topics: Topic[];
  totalInEnv: number;
  lastRebuild: Rebuild | undefined;
}

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

/** Return the highest-ordinal env each topic belongs to. */
function highestEnvForTopics(
  environments: Environment[],
  topicEnvs: TopicEnvironment[],
): Map<TopicId, EnvId> {
  const envOrdinal = new Map(environments.map((e) => [e.id, e.ordinal]));
  const best = new Map<TopicId, { envId: EnvId; ordinal: number }>();

  for (const te of topicEnvs) {
    const ord = envOrdinal.get(te.envId);
    if (ord === undefined) continue;
    const cur = best.get(te.topicId);
    if (!cur || ord > cur.ordinal) {
      best.set(te.topicId, { envId: te.envId, ordinal: ord });
    }
  }

  const result = new Map<TopicId, EnvId>();
  for (const [topicId, { envId }] of best) {
    result.set(topicId, envId);
  }
  return result;
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
      t.status !== "closed" &&
      (!assignedIds.has(t.id) || t.status === "graduated"),
  );
}

/** Map an environment name to its CSS color variable value. */
function getEnvColor(name: string): string {
  const lower = name.toLowerCase();
  if (lower.includes("dev")) return "var(--color-env-dev)";
  if (lower.includes("stag")) return "var(--color-env-staging)";
  if (lower.includes("prod")) return "var(--color-env-production)";
  return "var(--color-accent)";
}

// ============ Promotion Trail ============

interface PromotionTrailProps {
  allEnvs: Environment[];
  topicEnvIds: Set<EnvId>;
  highestEnvId: EnvId | null;
}

function PromotionTrail({ allEnvs, topicEnvIds, highestEnvId }: PromotionTrailProps) {
  if (allEnvs.length <= 1) return null;

  return (
    <div className="flex items-center mb-2 text-[10px]">
      {allEnvs.map((env, idx) => {
        const present = topicEnvIds.has(env.id);
        const isHighest = env.id === highestEnvId;
        const color = present ? getEnvColor(env.name) : "var(--color-text-dim)";

        return (
          <div key={env.id} className="flex items-center">
            {idx > 0 && (
              <div className="w-3 h-px bg-border shrink-0" />
            )}
            <div className="flex items-center gap-1">
              <div
                title={env.name}
                className={`rounded-full shrink-0 ${isHighest ? "w-2 h-2" : "w-1.5 h-1.5"}`}
                style={{
                  backgroundColor: present ? color : "transparent",
                  border: `1.5px solid ${color}`,
                  ...(isHighest ? { boxShadow: `0 0 0 2px color-mix(in srgb, ${color} 25%, transparent)` } : {}),
                }}
              />
              <span
                className={`font-mono text-[10px] leading-none ${isHighest ? "font-semibold" : ""}`}
                style={{ color }}
              >
                {env.name}
              </span>
            </div>
          </div>
        );
      })}
    </div>
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

  // Synced scroll for environment lanes (excludes unassigned)
  const envScrollRefs = useRef<Map<string, HTMLDivElement>>(new Map());
  const isSyncingScroll = useRef(false);

  const handleEnvScroll = useCallback((envId: string) => {
    if (isSyncingScroll.current) return;
    const source = envScrollRefs.current.get(envId);
    if (!source) return;
    isSyncingScroll.current = true;
    const top = source.scrollTop;
    for (const [id, el] of envScrollRefs.current) {
      if (id !== envId && id !== "unassigned") {
        el.scrollTop = top;
      }
    }
    isSyncingScroll.current = false;
  }, []);

  const handleEnvScrollRef = useCallback(
    (envId: string, el: HTMLDivElement | null) => {
      if (el) {
        envScrollRefs.current.set(envId, el);
      } else {
        envScrollRefs.current.delete(envId);
      }
    },
    [],
  );

  // Map of topicId -> highest env it belongs to (for dedup)
  const topicHighestEnv = useMemo(() => {
    if (!topicEnvs) return new Map<TopicId, EnvId>();
    return highestEnvForTopics(environments, topicEnvs);
  }, [environments, topicEnvs]);

  // Pre-computed map: topicId -> Set<EnvId> (avoids O(n*m) per-card filter)
  const topicEnvMap = useMemo(() => {
    const map = new Map<TopicId, Set<EnvId>>();
    if (!topicEnvs) return map;
    for (const te of topicEnvs) {
      let set = map.get(te.topicId);
      if (!set) {
        set = new Set<EnvId>();
        map.set(te.topicId, set);
      }
      set.add(te.envId);
    }
    return map;
  }, [topicEnvs]);

  // Env memberships for the selected topic (for ghost cards)
  const selectedTopicEnvIds = useMemo(() => {
    if (!selectedTopicId) return new Set<EnvId>();
    return topicEnvMap.get(selectedTopicId) ?? new Set<EnvId>();
  }, [selectedTopicId, topicEnvMap]);

  // The selected topic object (for ghost cards)
  const selectedTopic = useMemo(() => {
    if (!selectedTopicId) return undefined;
    return topics.find((t) => t.id === selectedTopicId);
  }, [selectedTopicId, topics]);

  // Build lane data — each topic appears only in its highest env
  const lanes: Lane[] = useMemo(() => {
    if (!topicEnvs) return [];
    const envLanes: Lane[] = environments.map((env) => {
      const allInEnv = topicsInEnv(env.id, topicEnvs, topics);
      const deduped = allInEnv.filter(
        (t) => topicHighestEnv.get(t.id) === env.id,
      );
      return {
        header: { id: env.id, name: env.name, branch: env.branch },
        env,
        topics: deduped,
        totalInEnv: allInEnv.length,
        lastRebuild: rebuilds ? latestRebuild(env.id, rebuilds) : undefined,
      };
    });

    const unassigned = unassignedTopics(topics, topicEnvs);
    // Active feature branches first, graduated (merged) last; newest first within each group
    unassigned.sort((a, b) => {
      const aGrad = a.status === "graduated" ? 1 : 0;
      const bGrad = b.status === "graduated" ? 1 : 0;
      if (aGrad !== bGrad) return aGrad - bGrad;
      return b.createdAt.localeCompare(a.createdAt);
    });
    if (unassigned.length > 0) {
      return [
        {
          header: { id: "unassigned", name: "Unassigned", branch: "" },
          env: null,
          topics: unassigned,
          totalInEnv: unassigned.length,
          lastRebuild: undefined,
        },
        ...envLanes,
      ];
    }
    return envLanes;
  }, [environments, topics, topicEnvs, rebuilds, topicHighestEnv]);

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
        case "O": {
          // Shift+O only — prevent accidental PR creation
          if (!e.shiftKey) break;
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
      <div className="flex-1 flex items-center justify-center">
        <div className="w-16 h-1 rounded-full bg-border animate-skeleton-pulse" />
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
        {lanes.map((lane, laneIndex) => {
          const isUnassigned = lane.env === null;
          return (
            <LaneColumn
              key={lane.header.id}
              header={lane.header}
              isUnassigned={isUnassigned}
              topics={lane.topics}
              totalInEnv={lane.totalInEnv}
              lastRebuild={lane.lastRebuild}
              conflicts={conflicts ?? []}
              isCurrentLane={laneIndex === focusedLane}
              focusedCardIndex={laneIndex === focusedLane ? focusedCard : null}
              selectedTopicId={selectedTopicId}
              nextEnv={lane.env ? nextEnv(lane.env.id) : undefined}
              isMutating={isMutating}
              isLastEnvLane={laneIndex === lanes.length - 1 && !isUnassigned}
              ghostTopic={
                selectedTopic &&
                lane.env &&
                selectedTopicEnvIds.has(lane.env.id) &&
                topicHighestEnv.get(selectedTopic.id) !== lane.env.id
                  ? selectedTopic
                  : undefined
              }
              allEnvs={environments}
              topicEnvMap={topicEnvMap}
              topicHighestEnv={topicHighestEnv}
              onCardRef={handleCardRef}
              onScrollRef={handleEnvScrollRef}
              onScroll={handleEnvScroll}
              onSelect={(topic, cardIndex) => {
                setFocusedLane(laneIndex);
                setFocusedCard(cardIndex);
                setSelectedTopicId(topic.id);
              }}
              onPromote={(topicId, repoId) => {
                if (!lane.env) return;
                const next = nextEnv(lane.env.id);
                if (next) promote.mutate({ topicId, envId: next.id, repoId });
              }}
              onDemote={(topicId, repoId) => {
                if (lane.env) {
                  demote.mutate({ topicId, envId: lane.env.id, repoId });
                }
              }}
              onRebuild={() => {
                if (lane.env) rebuild.mutate({ envId: lane.env.id });
              }}
              onGraduate={handleGraduate}
              onClose={(topicId, repoId) => closeTopic.mutate({ topicId, repoId })}
            />
          );
        })}
      </div>

      {/* Navigation hint */}
      <div className="px-4 py-2 border-t border-border text-xs text-text-dim font-mono flex-shrink-0">
        <span className="text-text-muted">h/l</span> lanes{" "}
        <span className="text-text-muted">j/k</span> navigate{" "}
        <span className="text-text-muted">Enter</span> select{" "}
        <span className="text-text-muted">Shift+O</span> create PR
      </div>
    </div>
  );
}

// ============ Lane Column ============

interface LaneColumnProps {
  header: LaneHeader;
  isUnassigned: boolean;
  topics: Topic[];
  totalInEnv: number;
  lastRebuild: Rebuild | undefined;
  conflicts: Conflict[];
  isCurrentLane: boolean;
  focusedCardIndex: number | null;
  selectedTopicId: TopicId | null;
  nextEnv: Environment | undefined;
  isMutating: boolean;
  isLastEnvLane: boolean;
  ghostTopic: Topic | undefined;
  allEnvs: Environment[];
  topicEnvMap: Map<TopicId, Set<EnvId>>;
  topicHighestEnv: Map<TopicId, EnvId>;
  onCardRef: (id: TopicId, el: HTMLDivElement | null) => void;
  onScrollRef: (envId: string, el: HTMLDivElement | null) => void;
  onScroll: (envId: string) => void;
  onSelect: (topic: Topic, cardIndex: number) => void;
  onPromote: (topicId: TopicId, repoId: string) => void;
  onDemote: (topicId: TopicId, repoId: string) => void;
  onRebuild: () => void;
  onGraduate: (topic: Topic) => void;
  onClose: (topicId: TopicId, repoId: string) => void;
}

function LaneColumn({
  header,
  isUnassigned,
  topics,
  totalInEnv,
  lastRebuild,
  conflicts,
  isCurrentLane,
  focusedCardIndex,
  selectedTopicId,
  nextEnv,
  isMutating,
  isLastEnvLane,
  ghostTopic,
  allEnvs,
  topicEnvMap,
  topicHighestEnv,
  onCardRef,
  onScrollRef,
  onScroll,
  onSelect,
  onPromote,
  onDemote,
  onRebuild,
  onGraduate,
  onClose,
}: LaneColumnProps) {

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
            {header.name}
          </span>
          <div className="flex items-center gap-2">
            <span
              className={`
                text-xs font-mono px-2 py-0.5 rounded
                ${isCurrentLane ? "bg-accent/20 text-accent" : "bg-surface-secondary text-text-dim"}
              `}
              title={
                !isUnassigned && topics.length !== totalInEnv
                  ? `${topics.length} unique here, ${totalInEnv} total including lower envs`
                  : isUnassigned
                    ? `${topics.filter((t) => t.status !== "graduated").length} active, ${topics.filter((t) => t.status === "graduated").length} merged`
                    : undefined
              }
            >
              {isUnassigned
                ? topics.filter((t) => t.status !== "graduated").length
                : topics.length}
            </span>
            {!isUnassigned && (
              <button
                type="button"
                disabled={isMutating}
                onClick={onRebuild}
                className="text-[10px] font-mono px-2 py-1 min-h-[28px] inline-flex items-center rounded border border-border text-text-muted hover:text-text-primary hover:bg-surface-secondary transition-colors disabled:opacity-40 disabled:cursor-not-allowed cursor-pointer"
                title={`Rebuild ${header.name}`}
              >
                Rebuild
              </button>
            )}
          </div>
        </div>

        <div className="flex items-center gap-2 text-[10px] font-mono text-text-dim h-4 mt-1">
          {!isUnassigned && (
            <>
              <span>{header.branch}</span>
              {lastRebuild && (
                <span className={REBUILD_COLOR[lastRebuild.status]}>
                  {lastRebuild.status}
                </span>
              )}
            </>
          )}
        </div>
      </div>

      {/* Card list */}
      <div className="flex-1 relative min-h-0">
        <div
          ref={(el) => onScrollRef(header.id, el)}
          onScroll={() => onScroll(header.id)}
          className="absolute inset-0 overflow-y-auto"
        >
          {isUnassigned ? (
            <UnassignedLaneContent
              topics={topics}
              conflicts={conflicts}
              isCurrentLane={isCurrentLane}
              focusedCardIndex={focusedCardIndex}
              selectedTopicId={selectedTopicId}
              isMutating={isMutating}
              allEnvs={allEnvs}
              topicEnvMap={topicEnvMap}
              topicHighestEnv={topicHighestEnv}
              onCardRef={onCardRef}
              onSelect={onSelect}
              onClose={onClose}
            />
          ) : (
            <div
              className="space-y-2 px-3 pb-3"
              role="list"
              aria-label={`${header.name} topics`}
            >
              {topics.length === 0 && !ghostTopic ? (
                <div className="text-text-dim text-xs font-mono text-center py-8 opacity-50">
                  No topics promoted here yet
                </div>
              ) : (
                <>
                  {ghostTopic && (
                    <GhostCard branch={ghostTopic.branch} />
                  )}
                  {topics.map((topic, cardIndex) => (
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
                      isUnassignedLane={false}
                      isLastEnvLane={isLastEnvLane}
                      isMutating={isMutating}
                      allEnvs={allEnvs}
                      topicEnvIds={topicEnvMap.get(topic.id) ?? new Set<EnvId>()}
                      highestEnvId={topicHighestEnv.get(topic.id) ?? null}
                      onRef={onCardRef}
                      onSelect={onSelect}
                      onPromote={onPromote}
                      onDemote={onDemote}
                      onGraduate={onGraduate}
                      onClose={onClose}
                    />
                  ))}
                </>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

// ============ Unassigned Lane Content ============

interface UnassignedLaneContentProps {
  topics: Topic[];
  conflicts: Conflict[];
  isCurrentLane: boolean;
  focusedCardIndex: number | null;
  selectedTopicId: TopicId | null;
  isMutating: boolean;
  allEnvs: Environment[];
  topicEnvMap: Map<TopicId, Set<EnvId>>;
  topicHighestEnv: Map<TopicId, EnvId>;
  onCardRef: (id: TopicId, el: HTMLDivElement | null) => void;
  onSelect: (topic: Topic, cardIndex: number) => void;
  onClose: (topicId: TopicId, repoId: string) => void;
}

function UnassignedLaneContent({
  topics,
  conflicts,
  isCurrentLane,
  focusedCardIndex,
  selectedTopicId,
  isMutating,
  allEnvs,
  topicEnvMap,
  topicHighestEnv,
  onCardRef,
  onSelect,
  onClose,
}: UnassignedLaneContentProps) {
  const [graduatedExpanded, setGraduatedExpanded] = useState(false);
  const active = topics.filter((t) => t.status !== "graduated");
  const graduated = topics.filter((t) => t.status === "graduated");

  // Map card indices back to the original array for keyboard focus
  const activeIndices = useMemo(() => {
    let idx = 0;
    return active.map(() => idx++);
  }, [active]);

  return (
    <div className="px-3 pb-3">
      {/* Active branches — full cards */}
      {active.length > 0 && (
        <div className="space-y-2 mb-3" role="list" aria-label="Active unassigned topics">
          {active.map((topic, i) => (
            <TopicCard
              key={topic.id}
              topic={topic}
              conflicts={conflictsForTopic(topic.id, conflicts)}
              cardIndex={activeIndices[i]!}
              isFocused={isCurrentLane && focusedCardIndex === activeIndices[i]}
              isSelected={selectedTopicId === topic.id}
              nextEnv={undefined}
              isUnassignedLane={true}
              isLastEnvLane={false}
              isMutating={isMutating}
              allEnvs={allEnvs}
              topicEnvIds={topicEnvMap.get(topic.id) ?? new Set<EnvId>()}
              highestEnvId={topicHighestEnv.get(topic.id) ?? null}
              onRef={onCardRef}
              onSelect={onSelect}
              onPromote={() => {}}
              onDemote={() => {}}
              onGraduate={() => {}}
              onClose={onClose}
            />
          ))}
        </div>
      )}

      {active.length === 0 && graduated.length === 0 && (
        <div className="text-text-dim text-xs font-mono text-center py-8 opacity-50">
          No topics
        </div>
      )}

      {/* Graduated branches — collapsed summary with expand */}
      {graduated.length > 0 && (
        <div>
          <button
            type="button"
            onClick={() => setGraduatedExpanded((prev) => !prev)}
            className="w-full flex items-center gap-2 px-2 py-1.5 rounded border border-border/50 bg-surface-primary/50 text-text-dim hover:text-text-muted hover:bg-surface-primary transition-colors cursor-pointer text-[11px] font-mono"
          >
            <span className={`transition-transform ${graduatedExpanded ? "rotate-90" : ""}`}>
              ▸
            </span>
            <span className="px-1.5 py-0.5 rounded bg-status-graduated/20 text-status-graduated text-[10px] uppercase">
              merged
            </span>
            <span>{graduated.length} branch{graduated.length !== 1 ? "es" : ""} to clean up</span>
          </button>

          {graduatedExpanded && (
            <div className="mt-1.5 space-y-1" role="list" aria-label="Graduated branches">
              {graduated.map((topic, i) => {
                const cardIndex = active.length + i;
                return (
                  <CompactGraduatedCard
                    key={topic.id}
                    topic={topic}
                    cardIndex={cardIndex}
                    isFocused={isCurrentLane && focusedCardIndex === cardIndex}
                    isSelected={selectedTopicId === topic.id}
                    isMutating={isMutating}
                    onRef={onCardRef}
                    onSelect={onSelect}
                    onClose={onClose}
                  />
                );
              })}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

/** Compact card for graduated branches — just name + cleanup action. */
function CompactGraduatedCard({
  topic,
  cardIndex,
  isFocused,
  isSelected,
  isMutating,
  onRef,
  onSelect,
  onClose,
}: {
  topic: Topic;
  cardIndex: number;
  isFocused: boolean;
  isSelected: boolean;
  isMutating: boolean;
  onRef: (id: TopicId, el: HTMLDivElement | null) => void;
  onSelect: (topic: Topic, cardIndex: number) => void;
  onClose: (topicId: TopicId, repoId: string) => void;
}) {
  const handleRef = useCallback(
    (el: HTMLDivElement | null) => { onRef(topic.id, el); },
    [topic.id, onRef],
  );

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
          flex items-center justify-between gap-2 px-2 py-1.5 rounded border transition-colors cursor-pointer opacity-60
          focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent
          ${isSelected ? "border-accent bg-accent-subtle/30 opacity-100" : "border-border/40 bg-surface-primary/50 hover:bg-surface-secondary"}
        `}
      >
        <span className="text-xs font-mono text-text-muted truncate">{topic.branch}</span>
        <button
          type="button"
          title="Delete branch (already merged)"
          disabled={isMutating}
          onClick={(e) => { e.stopPropagation(); onClose(topic.id, topic.repoId); }}
          className="text-[10px] font-mono px-1.5 py-0.5 rounded border border-border/40 text-text-dim hover:text-text-muted hover:bg-surface-secondary transition-colors cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed shrink-0"
        >
          Clean up
        </button>
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
  allEnvs: Environment[];
  topicEnvIds: Set<EnvId>;
  highestEnvId: EnvId | null;
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
  allEnvs,
  topicEnvIds,
  highestEnvId,
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
            {isGraduated && isUnassignedLane ? (
              <span
                className="px-1.5 py-0.5 rounded border text-[10px] font-mono uppercase tracking-wider bg-status-graduated/20 text-status-graduated border-status-graduated/40"
              >
                ✓ merged
              </span>
            ) : topic.status !== "active" && topic.status !== "closed" ? (
              <span
                className={`px-1.5 py-0.5 rounded border text-[10px] font-mono uppercase tracking-wider ${STATUS_BADGE[topic.status]}`}
              >
                {topic.status}
              </span>
            ) : null}
            {topic.branchOrigin !== "tracked" && (
              <span
                className={`px-1.5 py-0.5 rounded text-[10px] font-mono uppercase ${ORIGIN_BADGE[topic.branchOrigin]}`}
              >
                {topic.branchOrigin === "local-only" ? "local" : "orphaned"}
              </span>
            )}
            {topic.ciStatus &&
              (topic.ciUrl ? (
                <a
                  href={topic.ciUrl}
                  target="_blank"
                  rel="noopener noreferrer"
                  className={`px-1.5 py-0.5 rounded text-[10px] font-mono uppercase hover:underline ${CI_BADGE[topic.ciStatus]}`}
                  onClick={(e) => e.stopPropagation()}
                >
                  CI: {topic.ciStatus}
                </a>
              ) : (
                <span
                  className={`px-1.5 py-0.5 rounded text-[10px] font-mono uppercase ${CI_BADGE[topic.ciStatus]}`}
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

          {/* Promotion trail */}
          {!isUnassignedLane && allEnvs.length > 1 && (
            <PromotionTrail
              allEnvs={allEnvs}
              topicEnvIds={topicEnvIds}
              highestEnvId={highestEnvId}
            />
          )}

          {/* Action buttons */}
          <div
            className="flex items-center gap-2 pt-1.5 border-t border-border/50"
            onClick={(e) => e.stopPropagation()}
            onKeyDown={(e) => e.stopPropagation()}
          >
            {nextEnv && !isGraduated && !isUnassignedLane && (
              <ActionButton
                label={`→ ${nextEnv.name}`}
                title={`Promote to ${nextEnv.name}`}
                disabled={isMutating}
                onClick={() => onPromote(topic.id, topic.repoId)}
                variant="primary"
              />
            )}
            {isLastEnvLane && !isGraduated && (
              <ActionButton
                label="Graduate →"
                title="Create PR to merge into master"
                disabled={isMutating}
                onClick={() => onGraduate(topic)}
                variant="primary"
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
            {isUnassignedLane && isGraduated && (
              <ActionButton
                label="Clean up branch"
                title="Delete branch (already merged into master)"
                disabled={isMutating}
                onClick={() => onClose(topic.id, topic.repoId)}
              />
            )}
          </div>

          {/* Close confirmation modal */}
          {showCloseConfirm && (
            <CloseConfirmDialog
              branch={topic.branch}
              closeInput={closeInput}
              onInputChange={setCloseInput}
              onConfirm={() => {
                onClose(topic.id, topic.repoId);
                setShowCloseConfirm(false);
                setCloseInput("");
              }}
              onCancel={() => { setShowCloseConfirm(false); setCloseInput(""); }}
            />
          )}
        </div>
    </div>
  );
}

// ============ Ghost Card ============

function GhostCard({ branch }: { branch: string }) {
  return (
    <div role="listitem">
      <div
        aria-label={`${branch} (present in lower environment)`}
        className="p-3 rounded border border-accent/40 bg-accent-subtle/10 border-dashed animate-ghost-pulse"
      >
        <div className="text-sm font-mono leading-tight truncate text-accent/70">
          {branch}
        </div>
        <div className="flex items-center gap-1.5 mt-1">
          <span className="px-1.5 py-0.5 rounded text-[10px] font-mono uppercase text-accent/50">
            ✓ present
          </span>
        </div>
      </div>
    </div>
  );
}

// ============ Close Confirm Dialog ============

function CloseConfirmDialog({
  branch,
  closeInput,
  onInputChange,
  onConfirm,
  onCancel,
}: {
  branch: string;
  closeInput: string;
  onInputChange: (value: string) => void;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const dialogRef = useRef<HTMLDivElement>(null);

  // Focus trap: cycle through focusable elements within the dialog
  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;

    function handleKeyDown(e: KeyboardEvent) {
      if (e.key === "Escape") {
        onCancel();
        return;
      }
      if (e.key !== "Tab") return;

      const focusable = dialog!.querySelectorAll<HTMLElement>(
        'input, button, [tabindex]:not([tabindex="-1"])',
      );
      if (focusable.length === 0) return;

      const first = focusable[0]!;
      const last = focusable[focusable.length - 1]!;

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

    dialog.addEventListener("keydown", handleKeyDown);
    return () => dialog.removeEventListener("keydown", handleKeyDown);
  }, [onCancel]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 animate-backdrop"
      onClick={(e) => { e.stopPropagation(); onCancel(); }}
      onKeyDown={(e) => e.stopPropagation()}
    >
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="close-dialog-title"
        aria-describedby="close-dialog-desc"
        className="bg-surface-primary border border-border rounded-lg p-4 w-[400px] shadow-xl animate-scale-in"
        onClick={(e) => e.stopPropagation()}
      >
        <h3 id="close-dialog-title" className="text-sm font-mono font-bold text-text-primary mb-2">
          Delete branch?
        </h3>
        <p id="close-dialog-desc" className="text-xs font-mono text-text-muted mb-3">
          This will delete <span className="text-status-conflict font-bold">{branch}</span> from
          both origin and local. This cannot be undone.
        </p>
        <p id="close-dialog-input-hint" className="text-xs font-mono text-text-dim mb-2">
          Type the branch name to confirm:
        </p>
        <input
          type="text"
          value={closeInput}
          onChange={(e) => onInputChange(e.target.value)}
          onKeyDown={(e) => {
            e.stopPropagation();
            if (e.key === "Enter" && closeInput === branch) {
              onConfirm();
            }
          }}
          aria-labelledby="close-dialog-input-hint"
          className="w-full px-2 py-1.5 rounded border border-border bg-bg-primary text-text-primary font-mono text-xs focus:outline-none focus:ring-1 focus:ring-accent mb-3"
          placeholder={branch}
          autoFocus
        />
        <div className="flex justify-end gap-2">
          <button
            type="button"
            onClick={onCancel}
            className="text-xs font-mono px-3 py-1.5 min-h-[32px] inline-flex items-center rounded border border-border text-text-muted hover:text-text-primary hover:bg-surface-secondary transition-colors cursor-pointer focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus"
          >
            Cancel
          </button>
          <button
            type="button"
            disabled={closeInput !== branch}
            onClick={onConfirm}
            className="text-xs font-mono px-3 py-1.5 min-h-[32px] inline-flex items-center rounded border border-status-conflict/50 text-status-conflict hover:bg-status-conflict/10 transition-colors cursor-pointer disabled:opacity-30 disabled:cursor-not-allowed focus:outline-none focus-visible:ring-2 focus-visible:ring-border-focus"
          >
            Delete branch
          </button>
        </div>
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
  variant = "secondary",
}: {
  label: string;
  title: string;
  disabled: boolean;
  onClick: () => void;
  variant?: "primary" | "secondary" | "danger";
}) {
  const styles = {
    primary:
      "border-accent/40 text-accent bg-accent-subtle/20 hover:bg-accent-subtle/40 hover:text-accent",
    secondary:
      "border-border text-text-muted hover:text-text-primary hover:bg-surface-secondary",
    danger:
      "border-status-conflict/30 text-status-conflict/70 hover:text-status-conflict hover:bg-status-conflict/10",
  };

  return (
    <button
      type="button"
      title={title}
      disabled={disabled}
      onClick={onClick}
      className={`
        text-[10px] font-mono px-2 py-1 min-h-[28px] inline-flex items-center rounded border cursor-pointer
        transition-colors disabled:opacity-40 disabled:cursor-not-allowed
        ${styles[variant]}
      `}
    >
      {label}
    </button>
  );
}
