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
  Conflict,
} from "../../../generated/types.js";
import {
  useTopics,
  useEnvironments,
  useTopicEnvironments,
  useRebuildStatus,
  useConflicts,
  useRepos,
} from "../../../lib/queries.js";
import { usePromote, useDemote, useRebuild, useCreatePr, useCloseTopic } from "../../../lib/mutations.js";
import { useUIStore } from "../../../lib/store.js";
import type { Lane } from "./types.js";
import { topicsInEnv, highestEnvForTopics, latestRebuild, unassignedTopics } from "./utils.js";
import { LaneColumn } from "./LaneColumn.js";

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

  // Pre-computed map: topicId -> unresolved Conflict[] (avoids O(n*m) per-card filter)
  const topicConflictsMap = useMemo(() => {
    const map = new Map<TopicId, Conflict[]>();
    if (!conflicts) return map;
    for (const c of conflicts) {
      if (c.resolved) continue;
      let list = map.get(c.topicId);
      if (!list) {
        list = [];
        map.set(c.topicId, list);
      }
      list.push(c);
    }
    return map;
  }, [conflicts]);

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
        e.target instanceof HTMLSelectElement ||
        (e.target instanceof HTMLElement && e.target.isContentEditable)
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
  if (!allTopics || !allEnvironments || !topicEnvs || !repos) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <div className="w-16 h-1 rounded-full bg-border animate-skeleton-pulse" />
      </div>
    );
  }

  // No repo selected state (kanban requires a specific repo)
  if (!selectedRepoId) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-2 text-text-muted font-mono text-sm p-8">
        <p>Select a repository</p>
        <p className="text-text-dim text-xs">
          Use the dropdown above to choose a repo
        </p>
      </div>
    );
  }

  // Empty state - no environments for this repo
  if (environments.length === 0) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-2 text-text-muted font-mono text-sm p-8">
        <p>No environments configured</p>
        <p className="text-text-dim text-xs">
          Add a <code className="text-accent">.restack.yml</code> to your
          repo or run{" "}
          <code className="text-accent">restack integration add</code>
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
              topicConflictsMap={topicConflictsMap}
              isCurrentLane={laneIndex === focusedLane}
              focusedCardIndex={laneIndex === focusedLane ? focusedCard : null}
              selectedTopicId={selectedTopicId}
              nextEnv={lane.env ? nextEnv(lane.env.id) : undefined}
              isMutating={isMutating}
              isLastEnvLane={laneIndex === lanes.length - 1 && !isUnassigned}
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
                if (!lane.env) {
                  // Unassigned → promote to first environment
                  const first = environments[0];
                  if (first) promote.mutate({ topicId, envId: first.id, repoId });
                  return;
                }
                const next = nextEnv(lane.env.id);
                if (next) promote.mutate({ topicId, envId: next.id, repoId });
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
