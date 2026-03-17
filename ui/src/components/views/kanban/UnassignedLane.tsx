import { useMemo, useState } from "react";
import type { EnvId } from "../../../generated/types.js";
import type { UnassignedLaneContentProps } from "./types.js";
import { TopicCard, CompactGraduatedCard } from "./TopicCard.js";

export function UnassignedLaneContent({
  topics,
  topicConflictsMap,
  isCurrentLane,
  focusedCardIndex,
  selectedTopicId,
  isMutating,
  allEnvs,
  topicEnvMap,
  firstEnv,
  topicHighestEnv,
  onCardRef,
  onSelect,
  onPromote,
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
              conflicts={topicConflictsMap.get(topic.id) ?? []}
              cardIndex={activeIndices[i]!}
              isFocused={isCurrentLane && focusedCardIndex === activeIndices[i]}
              isSelected={selectedTopicId === topic.id}
              nextEnv={firstEnv}
              isUnassignedLane={true}
              isLastEnvLane={false}
              isMutating={isMutating}
              allEnvs={allEnvs}
              topicEnvIds={topicEnvMap.get(topic.id) ?? new Set<EnvId>()}
              highestEnvId={topicHighestEnv.get(topic.id) ?? null}
              onRef={onCardRef}
              onSelect={onSelect}
              onPromote={onPromote}
              onGraduate={() => {}}
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
