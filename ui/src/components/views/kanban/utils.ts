import type {
  Topic,
  TopicId,
  Environment,
  EnvId,
  TopicEnvironment,
  Rebuild,
} from "../../../generated/types.js";

export function topicsInEnv(
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
export function highestEnvForTopics(
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

export function latestRebuild(
  envId: EnvId,
  rebuilds: Rebuild[],
): Rebuild | undefined {
  return rebuilds
    .filter((r) => r.envId === envId)
    .sort((a, b) => b.startedAt.localeCompare(a.startedAt))[0];
}

export function unassignedTopics(
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
export function getEnvColor(name: string): string {
  const lower = name.toLowerCase();
  if (lower.includes("dev")) return "var(--color-env-dev)";
  if (lower.includes("stag")) return "var(--color-env-staging)";
  if (lower.includes("prod")) return "var(--color-env-production)";
  return "var(--color-accent)";
}
