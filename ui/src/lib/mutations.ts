/**
 * TanStack Query mutations for restack API actions.
 * Promote, demote, and rebuild operations with query invalidation.
 */

import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { apiFetch } from "./api.js";
import { queryKeys } from "./queries.js";
import type {
  EnvId,
  TopicId,
  SyncResult,
  GeneratedFile,
  PullRequest,
  MergePrResult,
  MergeStrategy,
  BranchProtectionResult,
  PipelineRunResult,
  Conflict,
} from "../generated/types.js";

/** Promote a topic into a target environment. */
export function usePromote() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ topicId, envId, repoId }: { topicId: TopicId; envId: EnvId; repoId: string }) =>
      apiFetch<{ conflicts: Conflict[] }>("/api/promote/to", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ topicId, envId, repoId }),
      }),
    onSuccess: (result) => {
      void qc.invalidateQueries({ queryKey: queryKeys.topicEnvironments.all });
      void qc.invalidateQueries({ queryKey: queryKeys.topics.all });
      void qc.invalidateQueries({ queryKey: queryKeys.rebuilds.all });
      void qc.invalidateQueries({ queryKey: queryKeys.conflicts.all });

      if (result.conflicts && result.conflicts.length > 0) {
        toast.error("Merge conflict detected", {
          description: `${result.conflicts.length} topic${result.conflicts.length > 1 ? "s" : ""} could not be merged and were removed from the environment.`,
        });
      }
    },
  });
}

/** Demote (remove) a topic from an environment. */
export function useDemote() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ topicId, envId, repoId }: { topicId: TopicId; envId: EnvId; repoId: string }) =>
      apiFetch<void>("/api/promote/from", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ topicId, envId, repoId }),
      }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: queryKeys.topicEnvironments.all });
      void qc.invalidateQueries({ queryKey: queryKeys.topics.all });
      void qc.invalidateQueries({ queryKey: queryKeys.rebuilds.all });
    },
  });
}

/** Sync topic PRs from provider. */
export function useTopicSync() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ repo }: { repo: string }) =>
      apiFetch<SyncResult>("/api/topics/sync", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ repo }),
      }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: queryKeys.topics.all });
    },
  });
}

export interface RefreshResult {
  repo: string;
  discovery: {
    discovered: number;
    created: number;
    closed: number;
    skipped: number;
  };
  ciRefreshed: number;
}

/** Refresh: fetch origin, discover branches, sync CI, cleanup stale topics. */
export function useRefresh() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ repo }: { repo?: string }) =>
      apiFetch<RefreshResult[]>("/api/refresh", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ repo }),
      }),
    onSuccess: (results) => {
      void qc.invalidateQueries({ queryKey: queryKeys.topics.all });
      void qc.invalidateQueries({ queryKey: queryKeys.topicEnvironments.all });
      void qc.invalidateQueries({ queryKey: queryKeys.rebuilds.all });
      void qc.invalidateQueries({ queryKey: queryKeys.conflicts.all });

      let totalCreated = 0;
      let totalClosed = 0;
      for (const r of results) {
        totalCreated += r.discovery.created;
        totalClosed += r.discovery.closed;
      }
      if (totalCreated > 0 || totalClosed > 0) {
        toast.success("Refresh complete", {
          description: `${totalCreated} new topics discovered, ${totalClosed} closed`,
        });
      }
    },
  });
}

/** Generate CI configuration files. */
export function useCiGenerate() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ repo }: { repo: string }) =>
      apiFetch<GeneratedFile[]>("/api/ci/generate", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ repo }),
      }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: queryKeys.ci.all });
    },
  });
}

/** Trigger a rebuild for an environment. */
export function useRebuild() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ envId }: { envId: EnvId }) =>
      apiFetch<void>(`/api/rebuild/${envId}`, { method: "POST" }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: queryKeys.rebuilds.all });
      void qc.invalidateQueries({ queryKey: queryKeys.topics.all });
      void qc.invalidateQueries({ queryKey: queryKeys.conflicts.all });
    },
  });
}

/** Close a topic: delete branch on origin + local, remove from DB. */
export function useCloseTopic() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ topicId, repoId }: { topicId: TopicId; repoId: string }) =>
      apiFetch<{ deleted: boolean; branch: string }>(`/api/topics/${topicId}/close?repo=${encodeURIComponent(repoId)}`, {
        method: "POST",
      }),
    onSuccess: (result) => {
      void qc.invalidateQueries({ queryKey: queryKeys.topics.all });
      void qc.invalidateQueries({ queryKey: queryKeys.topicEnvironments.all });
      toast.success("Topic closed", {
        description: `Branch ${result.branch} deleted from origin and local`,
      });
    },
  });
}

/** Create a pull request. */
export function useCreatePr() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (params: {
      repo: string;
      head: string;
      base: string;
      title: string;
      body?: string;
      draft?: boolean;
    }) =>
      apiFetch<PullRequest>("/api/pr/create", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(params),
      }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: queryKeys.topics.all });
    },
  });
}

/** Merge a pull request. */
export function useMergePr() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (params: {
      repo: string;
      prNumber: string;
      strategy?: MergeStrategy;
      deleteBranch?: boolean;
    }) =>
      apiFetch<MergePrResult>("/api/pr/merge", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(params),
      }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: queryKeys.topics.all });
    },
  });
}

/** Set branch protection rules. */
export function useSetBranchProtection() {
  return useMutation({
    mutationFn: (params: {
      repo: string;
      branch: string;
      checks?: string[];
      requirePr?: boolean;
      minApprovals?: number;
    }) =>
      apiFetch<BranchProtectionResult>("/api/protection/set", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(params),
      }),
  });
}

/** Protect all environment branches. */
export function useProtectEnvBranches() {
  return useMutation({
    mutationFn: (params: { repo: string }) =>
      apiFetch<BranchProtectionResult[]>("/api/protection/envs", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(params),
      }),
  });
}

/** Trigger a CI pipeline. */
export function useTriggerPipeline() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (params: {
      repo: string;
      branch: string;
      name?: string;
    }) =>
      apiFetch<PipelineRunResult>("/api/pipeline/trigger", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(params),
      }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: queryKeys.rebuilds.all });
    },
  });
}
