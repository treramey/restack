/**
 * TanStack Query mutations for restack API actions.
 * Promote, demote, and rebuild operations with query invalidation.
 */

import { useMutation, useQueryClient } from "@tanstack/react-query";
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
} from "../generated/types.js";

/** Promote a topic into a target environment. */
export function usePromote() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ topicId, envId }: { topicId: TopicId; envId: EnvId }) =>
      apiFetch<void>("/api/promote/to", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ topicId, envId }),
      }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: queryKeys.topicEnvironments.all });
      void qc.invalidateQueries({ queryKey: queryKeys.topics.all });
      void qc.invalidateQueries({ queryKey: queryKeys.rebuilds.all });
    },
  });
}

/** Demote (remove) a topic from an environment. */
export function useDemote() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ topicId, envId }: { topicId: TopicId; envId: EnvId }) =>
      apiFetch<void>("/api/promote/from", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ topicId, envId }),
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
