import { useCallback } from "react";
import type { Topic, TopicId } from "../../../generated/types.js";
import { STATUS_BORDER, STATUS_BADGE, CI_BADGE, ORIGIN_BADGE } from "../../../lib/badges.js";
import type { TopicCardProps } from "./types.js";
import { PromotionTrail } from "./PromotionTrail.js";
import { ActionButton } from "./ActionButton.js";

export function TopicCard({
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
  onGraduate,
}: TopicCardProps) {
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

          {/* Forward action: promote to next env, or create PR in last env */}
          {!isGraduated && (nextEnv || isLastEnvLane) ? (
            <div
              className="flex items-center gap-2 pt-1.5 border-t border-border/50"
              onClick={(e) => e.stopPropagation()}
              onKeyDown={(e) => e.stopPropagation()}
            >
              {nextEnv ? (
                <ActionButton
                  label={`→ ${nextEnv.name}`}
                  title={`Promote to ${nextEnv.name}`}
                  disabled={isMutating}
                  onClick={() => onPromote(topic.id, topic.repoId)}
                  variant="primary"
                />
              ) : (
                <ActionButton
                  label="Create PR →"
                  title="Create PR to merge into base branch"
                  disabled={isMutating}
                  onClick={() => onGraduate(topic)}
                  variant="primary"
                />
              )}
            </div>
          ) : null}
        </div>
    </div>
  );
}

/** Compact card for graduated branches — just name + cleanup action. */
export function CompactGraduatedCard({
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
          className="text-[10px] font-mono px-2 py-1 min-h-[36px] rounded border border-border/40 text-text-dim hover:text-text-muted hover:bg-surface-secondary transition-colors cursor-pointer disabled:opacity-40 disabled:cursor-not-allowed shrink-0 inline-flex items-center"
        >
          Clean up
        </button>
      </div>
    </div>
  );
}
