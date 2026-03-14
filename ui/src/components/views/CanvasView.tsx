/**
 * Canvas view — multi-repo tree using @xyflow/react + @dagrejs/dagre.
 *
 * Layout: Workspace (root) → Repos → Topics
 * Color-coded by environment membership.
 */

import { useMemo, useCallback, memo } from "react";
import {
  ReactFlow,
  Controls,
  Background,
  type Node,
  type Edge,
  type NodeProps,
  type NodeMouseHandler,
  Handle,
  Position,
  BackgroundVariant,
} from "@xyflow/react";
import dagre from "@dagrejs/dagre";
import { useUIStore } from "../../lib/store.js";
import {
  useRepos,
  useTopics,
  useEnvironments,
  useTopicEnvironments,
} from "../../lib/queries.js";
import type {
  Repo,
  Topic,
  Environment,
  TopicEnvironment,
  RepoId,
  TopicId,
  EnvId,
} from "../../generated/types.js";

import "@xyflow/react/dist/style.css";

// Layout constants
const REPO_NODE_WIDTH = 220;
const REPO_NODE_HEIGHT = 72;
const TOPIC_NODE_WIDTH = 240;
const TOPIC_NODE_HEIGHT = 64;
const WORKSPACE_NODE_WIDTH = 180;
const WORKSPACE_NODE_HEIGHT = 48;

// ── Node data types ──────────────────────────────────────────────────

interface WorkspaceNodeData extends Record<string, unknown> {
  label: string;
}

interface RepoNodeData extends Record<string, unknown> {
  repo: Repo;
}

interface TopicNodeData extends Record<string, unknown> {
  topic: Topic;
  envColor: string;
  envLabel: string;
}

type WorkspaceNode = Node<WorkspaceNodeData, "workspace">;
type RepoNode = Node<RepoNodeData, "repo">;
type TopicNode = Node<TopicNodeData, "topic">;
type CanvasNode = WorkspaceNode | RepoNode | TopicNode;
type CanvasEdge = Edge;

// ── Environment color logic ──────────────────────────────────────────

function resolveTopicEnvColor(
  topicId: TopicId,
  topicStatus: string,
  topicEnvs: TopicEnvironment[],
  envMap: Map<EnvId, Environment>,
): { color: string; label: string } {
  if (topicStatus === "conflict") {
    return { color: "var(--color-env-conflict)", label: "conflict" };
  }

  const envIds = topicEnvs.filter((te) => te.topicId === topicId).map((te) => te.envId);
  if (envIds.length === 0) {
    return { color: "var(--color-env-unassigned)", label: "unassigned" };
  }

  // Pick highest-ordinal environment (most promoted)
  let best: Environment | undefined;
  for (const eid of envIds) {
    const env = envMap.get(eid);
    if (env && (!best || env.ordinal > best.ordinal)) {
      best = env;
    }
  }

  if (!best) return { color: "var(--color-env-unassigned)", label: "unassigned" };

  const name = best.name.toLowerCase();
  if (name.includes("prod")) return { color: "var(--color-env-production)", label: best.name };
  if (name.includes("stag")) return { color: "var(--color-env-staging)", label: best.name };
  if (name.includes("dev")) return { color: "var(--color-env-dev)", label: best.name };
  return { color: "var(--color-env-dev)", label: best.name };
}

// ── Provider badge ───────────────────────────────────────────────────

const PROVIDER_LABELS: Record<string, string> = {
  gitHub: "GH",
  azureDevOps: "ADO",
  bitbucket: "BB",
  unknown: "",
};

// ── Dagre layout ─────────────────────────────────────────────────────

function layoutElements(
  nodes: CanvasNode[],
  edges: CanvasEdge[],
): { nodes: CanvasNode[]; edges: CanvasEdge[] } {
  if (nodes.length === 0) return { nodes: [], edges: [] };

  const g = new dagre.graphlib.Graph().setDefaultEdgeLabel(() => ({}));
  g.setGraph({ rankdir: "TB", nodesep: 32, ranksep: 56, marginx: 24, marginy: 24 });

  for (const node of nodes) {
    const w =
      node.type === "workspace" ? WORKSPACE_NODE_WIDTH :
      node.type === "repo" ? REPO_NODE_WIDTH :
      TOPIC_NODE_WIDTH;
    const h =
      node.type === "workspace" ? WORKSPACE_NODE_HEIGHT :
      node.type === "repo" ? REPO_NODE_HEIGHT :
      TOPIC_NODE_HEIGHT;
    g.setNode(node.id, { width: w, height: h });
  }

  for (const edge of edges) {
    g.setEdge(edge.source, edge.target);
  }

  dagre.layout(g);

  const laid = nodes.map((node) => {
    const pos = g.node(node.id);
    const w =
      node.type === "workspace" ? WORKSPACE_NODE_WIDTH :
      node.type === "repo" ? REPO_NODE_WIDTH :
      TOPIC_NODE_WIDTH;
    const h =
      node.type === "workspace" ? WORKSPACE_NODE_HEIGHT :
      node.type === "repo" ? REPO_NODE_HEIGHT :
      TOPIC_NODE_HEIGHT;
    return {
      ...node,
      targetPosition: Position.Top,
      sourcePosition: Position.Bottom,
      position: { x: pos.x - w / 2, y: pos.y - h / 2 },
    };
  });

  return { nodes: laid, edges };
}

// ── Custom node components ───────────────────────────────────────────

const WorkspaceNodeComponent = memo(function WorkspaceNodeComponent({
  data,
}: NodeProps<WorkspaceNode>) {
  return (
    <>
      <Handle type="source" position={Position.Bottom} style={{ opacity: 0 }} />
      <div
        className="flex items-center justify-center border border-accent bg-accent-subtle text-accent font-mono font-bold text-xs uppercase tracking-widest rounded"
        style={{ width: WORKSPACE_NODE_WIDTH, height: WORKSPACE_NODE_HEIGHT }}
      >
        {data.label}
      </div>
    </>
  );
});

const RepoNodeComponent = memo(function RepoNodeComponent({
  data,
}: NodeProps<RepoNode>) {
  const { repo } = data;
  const badge = PROVIDER_LABELS[repo.provider] ?? "";

  return (
    <>
      <Handle type="target" position={Position.Top} style={{ opacity: 0 }} />
      <div
        role="button"
        tabIndex={0}
        aria-label={repo.name}
        onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.currentTarget.click(); } }}
        className="flex items-center gap-2 border border-border bg-surface-primary px-3 py-2 rounded hover:border-border-hover transition-colors cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-border-focus"
        style={{ width: REPO_NODE_WIDTH, height: REPO_NODE_HEIGHT }}
      >
        {/* Folder icon */}
        <svg width="16" height="16" viewBox="0 0 16 16" fill="none" className="shrink-0 text-text-muted" aria-hidden="true">
          <path d="M2 4a1 1 0 011-1h3.586a1 1 0 01.707.293L8 4h5a1 1 0 011 1v7a1 1 0 01-1 1H3a1 1 0 01-1-1V4z" stroke="currentColor" strokeWidth="1.2" />
        </svg>
        <div className="flex flex-col min-w-0 flex-1">
          <span className="text-sm font-mono text-text-primary truncate">{repo.name}</span>
          <span className="text-[10px] font-mono text-text-dim truncate">{repo.baseBranch}</span>
        </div>
        {badge && (
          <span className="text-[9px] font-mono px-1.5 py-0.5 rounded bg-surface-secondary text-text-dim border border-border shrink-0">
            {badge}
          </span>
        )}
      </div>
      <Handle type="source" position={Position.Bottom} style={{ opacity: 0 }} />
    </>
  );
});

const TopicNodeComponent = memo(function TopicNodeComponent({
  data,
}: NodeProps<TopicNode>) {
  const { topic, envColor, envLabel } = data;

  return (
    <>
      <Handle type="target" position={Position.Top} style={{ opacity: 0 }} />
      <div
        role="button"
        tabIndex={0}
        aria-label={topic.branch}
        onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.currentTarget.click(); } }}
        className="flex items-center gap-2 border bg-surface-primary px-3 py-2 rounded cursor-pointer hover:border-border-hover transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-border-focus"
        style={{
          width: TOPIC_NODE_WIDTH,
          height: TOPIC_NODE_HEIGHT,
          borderColor: envColor,
          borderLeftWidth: 3,
        }}
      >
        <div className="flex flex-col min-w-0 flex-1 gap-0.5">
          <span className="text-xs font-mono text-text-primary truncate">{topic.branch}</span>
          <div className="flex items-center gap-1.5">
            <span
              className="text-[9px] font-mono px-1 py-0.5 rounded"
              style={{ backgroundColor: `color-mix(in oklch, ${envColor} 20%, transparent)`, color: envColor }}
            >
              {envLabel}
            </span>
            {topic.status === "conflict" && (
              <span className="text-[9px] font-mono text-text-dim">conflict</span>
            )}
          </div>
        </div>
      </div>
      <Handle type="source" position={Position.Bottom} style={{ opacity: 0 }} />
    </>
  );
});

const nodeTypes = {
  workspace: WorkspaceNodeComponent,
  repo: RepoNodeComponent,
  topic: TopicNodeComponent,
};

// ── Main component ───────────────────────────────────────────────────

export function CanvasView() {
  const setSelectedRepoId = useUIStore((s) => s.setSelectedRepoId);
  const setSelectedTopicId = useUIStore((s) => s.setSelectedTopicId);

  const { data: repos } = useRepos();
  const { data: topics } = useTopics();
  const { data: environments } = useEnvironments();
  const { data: topicEnvs } = useTopicEnvironments();

  const envMap = useMemo(() => {
    if (!environments) return new Map<EnvId, Environment>();
    return new Map(environments.map((e) => [e.id, e]));
  }, [environments]);

  const { nodes, edges } = useMemo(() => {
    if (!repos || !topics) return { nodes: [] as CanvasNode[], edges: [] as CanvasEdge[] };

    const allNodes: CanvasNode[] = [];
    const allEdges: CanvasEdge[] = [];

    // Workspace root
    const wsId = "__workspace__";
    allNodes.push({
      id: wsId,
      type: "workspace",
      position: { x: 0, y: 0 },
      data: { label: "WORKSPACE" },
    });

    // Group topics by repo
    const topicsByRepo = new Map<RepoId, Topic[]>();
    for (const t of topics) {
      const list = topicsByRepo.get(t.repoId) ?? [];
      list.push(t);
      topicsByRepo.set(t.repoId, list);
    }

    for (const repo of repos) {
      allNodes.push({
        id: repo.id,
        type: "repo",
        position: { x: 0, y: 0 },
        data: { repo },
      });
      allEdges.push({
        id: `ws-${repo.id}`,
        source: wsId,
        target: repo.id,
        type: "smoothstep",
        style: { stroke: "var(--color-text-dim)", strokeWidth: 1.5 },
      });

      const repoTopics = (topicsByRepo.get(repo.id) ?? []).filter(
        (t) => t.status !== "closed" && t.status !== "graduated",
      );
      for (const topic of repoTopics) {
        const { color, label } = resolveTopicEnvColor(
          topic.id,
          topic.status,
          topicEnvs ?? [],
          envMap,
        );
        allNodes.push({
          id: topic.id,
          type: "topic",
          position: { x: 0, y: 0 },
          data: { topic, envColor: color, envLabel: label },
        });
        allEdges.push({
          id: `repo-${repo.id}-${topic.id}`,
          source: repo.id,
          target: topic.id,
          type: "smoothstep",
          style: { stroke: color, strokeWidth: 1.5 },
        });
      }
    }

    return layoutElements(allNodes, allEdges);
  }, [repos, topics, topicEnvs, envMap]);

  const handleNodeClick: NodeMouseHandler<CanvasNode> = useCallback(
    (_, node) => {
      if (node.type === "repo" && "repo" in node.data) {
        setSelectedRepoId(node.data.repo.id);
      } else if (node.type === "topic" && "topic" in node.data) {
        setSelectedTopicId(node.data.topic.id);
      }
    },
    [setSelectedRepoId, setSelectedTopicId],
  );

  if (!repos || repos.length === 0) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-4 text-text-muted p-8">
        <p className="font-mono uppercase tracking-wider text-sm">No repos tracked</p>
        <p className="text-text-dim text-xs font-mono">Run `restack repo add` to begin</p>
      </div>
    );
  }

  return (
    <div className="w-full h-full min-h-0">
      <ReactFlow
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        onNodeClick={handleNodeClick}
        nodesDraggable={false}
        nodesConnectable={false}
        edgesReconnectable={false}
        elementsSelectable={false}
        nodesFocusable={true}
        edgesFocusable={false}
        fitView
        fitViewOptions={{ padding: 0.3, minZoom: 0.2 }}
        minZoom={0.1}
        maxZoom={2}
        proOptions={{ hideAttribution: true }}
      >
        <Background variant={BackgroundVariant.Dots} gap={20} size={1} color="var(--color-border)" />
        <Controls showInteractive={false} />
      </ReactFlow>
    </div>
  );
}
