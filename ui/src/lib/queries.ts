/**
 * TanStack Query hooks for restack API data.
 * All server state flows through these hooks — components never call fetch directly.
 * 
 * Sync strategy: No polling. Rely on:
 * - staleTime + refetchOnWindowFocus for user-initiated refresh
 * - Mutation invalidation for data changes
 */

import { useQuery } from "@tanstack/react-query";
import { apiFetch } from "./api.js";
import type {
  Repo,
  Topic,
  Environment,
  TopicEnvironment,
  Rebuild,
  Conflict,
  CiStatusDetail,
  RepoId,
} from "../generated/types.js";

export const queryKeys = {
  repos: {
    all: ["repos"] as const,
    list: () => ["repos", "list"] as const,
  },
  topics: {
    all: ["topics"] as const,
    list: () => ["topics", "list"] as const,
  },
  environments: {
    all: ["environments"] as const,
    list: () => ["environments", "list"] as const,
  },
  topicEnvironments: {
    all: ["topicEnvironments"] as const,
    list: () => ["topicEnvironments", "list"] as const,
  },
  rebuilds: {
    all: ["rebuilds"] as const,
    list: () => ["rebuilds", "list"] as const,
  },
  conflicts: {
    all: ["conflicts"] as const,
    list: () => ["conflicts", "list"] as const,
  },
  ci: {
    all: ["ci"] as const,
    status: (repoId: RepoId) => ["ci", "status", repoId] as const,
  },
} as const;

export function useRepos() {
  return useQuery({
    queryKey: queryKeys.repos.list(),
    queryFn: () => apiFetch<Repo[]>("/api/repos"),
  });
}

export function useTopics() {
  return useQuery({
    queryKey: queryKeys.topics.list(),
    queryFn: () => apiFetch<Topic[]>("/api/topics"),
  });
}

export function useEnvironments() {
  return useQuery({
    queryKey: queryKeys.environments.list(),
    queryFn: () => apiFetch<Environment[]>("/api/envs"),
  });
}

export function useTopicEnvironments() {
  return useQuery({
    queryKey: queryKeys.topicEnvironments.list(),
    queryFn: () => apiFetch<TopicEnvironment[]>("/api/topic-environments"),
  });
}

export function useRebuilds() {
  return useQuery({
    queryKey: queryKeys.rebuilds.list(),
    queryFn: () => apiFetch<Rebuild[]>("/api/rebuilds"),
  });
}

export function useConflicts() {
  return useQuery({
    queryKey: queryKeys.conflicts.list(),
    queryFn: () => apiFetch<Conflict[]>("/api/conflicts"),
  });
}

export function useCiStatus(repoId: RepoId | null) {
  return useQuery({
    queryKey: queryKeys.ci.status(repoId ?? ("" as RepoId)),
    queryFn: () => apiFetch<CiStatusDetail[]>(`/api/ci/status?repo=${repoId}`),
    enabled: !!repoId,
  });
}

export function useRebuildStatus() {
  return useQuery({
    queryKey: ["rebuildStatus"] as const,
    queryFn: () => apiFetch<Rebuild[]>("/api/rebuilds?latest=true"),
  });
}
