import type { PromotionTrailProps } from "./types.js";
import { getEnvColor } from "./utils.js";

export function PromotionTrail({ allEnvs, topicEnvIds, highestEnvId }: PromotionTrailProps) {
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
                title={`${env.name}${present ? " (promoted)" : ""}`}
                aria-label={`${env.name}: ${present ? "promoted" : "not promoted"}`}
                className={`rounded-full shrink-0 ${isHighest ? "w-2 h-2" : "w-1.5 h-1.5"}`}
                style={{
                  backgroundColor: present ? color : "transparent",
                  border: `1.5px solid ${color}`,
                  ...(isHighest ? { boxShadow: `0 0 0 2px color-mix(in srgb, ${color} 25%, transparent)` } : {}),
                  ...(present ? {} : { borderStyle: "dashed" }),
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
