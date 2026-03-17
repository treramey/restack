import type { EnvId } from "../../../generated/types.js";
import { REBUILD_COLOR } from "../../../lib/badges.js";
import type { LaneColumnProps } from "./types.js";
import { TopicCard } from "./TopicCard.js";
import { UnassignedLaneContent } from "./UnassignedLane.js";

export function LaneColumn({
  header,
  isUnassigned,
  topics,
  totalInEnv,
  lastRebuild,
  topicConflictsMap,
  isCurrentLane,
  focusedCardIndex,
  selectedTopicId,
  nextEnv,
  isMutating,
  isLastEnvLane,
  allEnvs,
  topicEnvMap,
  topicHighestEnv,
  onCardRef,
  onScrollRef,
  onScroll,
  onSelect,
  onPromote,
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
                className="text-[10px] font-mono px-2.5 py-1.5 min-h-[36px] inline-flex items-center rounded border border-border text-text-muted hover:text-text-primary hover:bg-surface-secondary transition-colors disabled:opacity-40 disabled:cursor-not-allowed cursor-pointer"
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
              topicConflictsMap={topicConflictsMap}
              isCurrentLane={isCurrentLane}
              focusedCardIndex={focusedCardIndex}
              selectedTopicId={selectedTopicId}
              isMutating={isMutating}
              allEnvs={allEnvs}
              topicEnvMap={topicEnvMap}
              topicHighestEnv={topicHighestEnv}
              firstEnv={allEnvs[0]}
              onCardRef={onCardRef}
              onSelect={onSelect}
              onPromote={onPromote}
              onClose={onClose}
            />
          ) : (
            <div
              className="space-y-2 px-3 pb-3"
              role="list"
              aria-label={`${header.name} topics`}
            >
              {topics.length === 0 ? (
                <div className="text-text-dim text-xs font-mono text-center py-8 opacity-50">
                  No topics promoted here yet
                </div>
              ) : (
                <>
                  {topics.map((topic, cardIndex) => (
                    <TopicCard
                      key={topic.id}
                      topic={topic}
                      conflicts={topicConflictsMap.get(topic.id) ?? []}
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
                      onGraduate={onGraduate}
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
