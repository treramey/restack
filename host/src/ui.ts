/**
 * UI Server - Hono HTTP server for Restack with WebSocket support
 */
import { serve } from "@hono/node-server";
import { createNodeWebSocket } from "@hono/node-ws";
import { Hono, type Context } from "hono";
import { serveStatic } from "@hono/node-server/serve-static";
import type { StatusCode } from "hono/utils/http-status";
import { callCli } from "./cli.js";
import { CliError } from "./types.js";

type WsClient = { send: (data: string) => void; readyState: number };
const wsClients = new Set<WsClient>();

export type WsEvent = {
  type: "invalidate";
  queryKeys: string[][];
};

function broadcast(event: WsEvent) {
  const data = JSON.stringify(event);
  for (const client of wsClients) {
    if (client.readyState === 1) {
      client.send(data);
    }
  }
}

function broadcastInvalidate(...queryKeys: string[][]) {
  broadcast({ type: "invalidate", queryKeys });
}

interface ApiError {
  error: string;
  code?: string;
}

/**
 * Handle CLI errors and return appropriate HTTP status
 */
function handleCliError(
  c: Context,
  err: unknown
): Response & { _data: ApiError; _status: StatusCode } {
  if (err instanceof CliError) {
    const message = err.message.toLowerCase();
    if (message.includes("not found") || message.includes("no such")) {
      return c.json({ error: err.message }, 404);
    }
    if (
      message.includes("invalid") ||
      message.includes("already") ||
      message.includes("conflict")
    ) {
      return c.json({ error: err.message }, 400);
    }
    if (message.includes("not a git") || message.includes("not initialized")) {
      return c.json({ error: err.message, code: "NOT_INITIALIZED" }, 400);
    }
    return c.json({ error: err.message }, 500);
  }
  const message = err instanceof Error ? err.message : String(err);
  return c.json({ error: message }, 500);
}

// Validation helpers
function isObject(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

function requireString(
  body: Record<string, unknown>,
  key: string
): string | undefined {
  const val = body[key];
  if (val === undefined) return undefined;
  if (typeof val !== "string") return undefined;
  return val;
}

function requireBool(
  body: Record<string, unknown>,
  key: string
): boolean | undefined {
  const val = body[key];
  if (val === undefined) return undefined;
  if (typeof val !== "boolean") return undefined;
  return val;
}

function requireNumber(
  body: Record<string, unknown>,
  key: string
): number | undefined {
  const val = body[key];
  if (val === undefined) return undefined;
  if (typeof val !== "number") return undefined;
  return val;
}

/**
 * Repo routes
 */
function createRepoRoutes() {
  return new Hono()
    .get("/", async (c) => {
      try {
        const result = await callCli(["repo", "list"]);
        return c.json(result);
      } catch (err) {
        return handleCliError(c, err);
      }
    })
    .delete("/:id", async (c) => {
      const id = c.req.param("id");

      try {
        const result = await callCli(["repo", "remove", id]);
        return c.json(result);
      } catch (err) {
        return handleCliError(c, err);
      }
    });
}

/**
 * Topic routes
 */
function createTopicRoutes() {
  return new Hono()
    .get("/", async (c) => {
      const repo = c.req.query("repo");
      const args = ["topic", "list"];
      if (repo) args.push("--repo", repo);

      try {
        const result = await callCli(args);
        return c.json(result);
      } catch (err) {
        return handleCliError(c, err);
      }
    })

    .post("/track", async (c) => {
      let body: Record<string, unknown>;
      try {
        const raw: unknown = await c.req.json();
        if (!isObject(raw)) return c.json({ error: "Invalid JSON body" }, 400);
        body = raw;
      } catch {
        return c.json({ error: "Invalid JSON body" }, 400);
      }

      const branch = requireString(body, "branch");
      const repo = requireString(body, "repo");
      if (!branch) return c.json({ error: "branch is required" }, 400);
      if (!repo) return c.json({ error: "repo is required" }, 400);

      try {
        const result = await callCli(["topic", "track", "--repo", repo, branch]);
        return c.json(result, 201);
      } catch (err) {
        return handleCliError(c, err);
      }
    })

    .delete("/:id", async (c) => {
      const id = c.req.param("id");
      const repo = c.req.query("repo");
      if (!repo) return c.json({ error: "repo query param is required" }, 400);

      try {
        const result = await callCli(["topic", "untrack", "--repo", repo, id]);
        return c.json(result);
      } catch (err) {
        return handleCliError(c, err);
      }
    })

    .get("/:id/status", async (c) => {
      const id = c.req.param("id");
      const repo = c.req.query("repo");
      if (!repo) return c.json({ error: "repo query param is required" }, 400);

      try {
        const result = await callCli(["topic", "status", "--repo", repo, id]);
        return c.json(result);
      } catch (err) {
        return handleCliError(c, err);
      }
    });
}

/**
 * Environment routes
 */
function createEnvRoutes() {
  return new Hono()
    .get("/", async (c) => {
      const repo = c.req.query("repo");
      const args = ["env", "list"];
      if (repo) args.push("--repo", repo);

      try {
        const result = await callCli(args);
        return c.json(result);
      } catch (err) {
        return handleCliError(c, err);
      }
    })

    .post("/", async (c) => {
      let body: Record<string, unknown>;
      try {
        const raw: unknown = await c.req.json();
        if (!isObject(raw)) return c.json({ error: "Invalid JSON body" }, 400);
        body = raw;
      } catch {
        return c.json({ error: "Invalid JSON body" }, 400);
      }

      const name = requireString(body, "name");
      const branch = requireString(body, "branch");
      const repo = requireString(body, "repo");
      if (!name) return c.json({ error: "name is required" }, 400);
      if (!branch) return c.json({ error: "branch is required" }, 400);
      if (!repo) return c.json({ error: "repo is required" }, 400);

      const args = ["env", "add", "--branch", branch, "--repo", repo, name];
      const ordinal = requireNumber(body, "ordinal");
      if (ordinal !== undefined) args.push("--ordinal", String(ordinal));
      const autoPromote = requireBool(body, "autoPromote");
      if (autoPromote) args.push("--auto-promote");

      try {
        const result = await callCli(args);
        return c.json(result, 201);
      } catch (err) {
        return handleCliError(c, err);
      }
    })

    .get("/:name/status", async (c) => {
      const name = c.req.param("name");

      try {
        const result = await callCli(["env", "status", name]);
        return c.json(result);
      } catch (err) {
        return handleCliError(c, err);
      }
    });
}

/**
 * Promote routes
 */
function createPromoteRoutes() {
  return new Hono()
    .post("/to", async (c) => {
      let body: Record<string, unknown>;
      try {
        const raw: unknown = await c.req.json();
        if (!isObject(raw)) return c.json({ error: "Invalid JSON body" }, 400);
        body = raw;
      } catch {
        return c.json({ error: "Invalid JSON body" }, 400);
      }

      const topicId = requireString(body, "topicId");
      const envId = requireString(body, "envId");
      if (!topicId) return c.json({ error: "topicId is required" }, 400);
      if (!envId) return c.json({ error: "envId is required" }, 400);

      try {
        const reposResult = await callCli(["repo", "list"]);
        const repos = reposResult as Array<{ id: string }>;
        let repoId: string | undefined;
        let topicBranch: string | undefined;

        for (const repo of repos) {
          const topicsResult = await callCli(["topic", "list", "--repo", repo.id]);
          const topics = topicsResult as Array<{ id: string; branch: string }>;
          const topic = topics.find((t) => t.id === topicId);
          if (topic) {
            repoId = repo.id;
            topicBranch = topic.branch;
            break;
          }
        }

        if (!repoId) return c.json({ error: "Topic not found" }, 404);

        const envResult = await callCli(["env", "list", "--repo", repoId]);
        const envs = envResult as Array<{ id: string; name: string }>;
        const env = envs.find((e) => e.id === envId);
        if (!env) return c.json({ error: "Environment not found" }, 404);

        const result = await callCli([
          "promote", "to", "--repo", repoId, topicId, env.name,
        ]);
        broadcastInvalidate(["topics"], ["topicEnvironments"], ["rebuilds"], ["conflicts"]);
        return c.json(result);
      } catch (err) {
        return handleCliError(c, err);
      }
    })

    .post("/from", async (c) => {
      let body: Record<string, unknown>;
      try {
        const raw: unknown = await c.req.json();
        if (!isObject(raw)) return c.json({ error: "Invalid JSON body" }, 400);
        body = raw;
      } catch {
        return c.json({ error: "Invalid JSON body" }, 400);
      }

      const topicId = requireString(body, "topicId");
      const envId = requireString(body, "envId");
      if (!topicId) return c.json({ error: "topicId is required" }, 400);
      if (!envId) return c.json({ error: "envId is required" }, 400);

      try {
        const reposResult = await callCli(["repo", "list"]);
        const repos = reposResult as Array<{ id: string }>;
        let repoId: string | undefined;

        for (const repo of repos) {
          const topicsResult = await callCli(["topic", "list", "--repo", repo.id]);
          const topics = topicsResult as Array<{ id: string }>;
          if (topics.find((t) => t.id === topicId)) {
            repoId = repo.id;
            break;
          }
        }

        if (!repoId) return c.json({ error: "Topic not found" }, 404);

        const envResult = await callCli(["env", "list", "--repo", repoId]);
        const envs = envResult as Array<{ id: string; name: string }>;
        const env = envs.find((e) => e.id === envId);
        if (!env) return c.json({ error: "Environment not found" }, 404);

        const result = await callCli([
          "promote", "from", "--repo", repoId, topicId, env.name,
        ]);
        broadcastInvalidate(["topics"], ["topicEnvironments"], ["rebuilds"]);
        return c.json(result);
      } catch (err) {
        return handleCliError(c, err);
      }
    });
}

/**
 * Rebuild routes
 */
function createRebuildRoutes() {
  return new Hono()
    .post("/all", async (c) => {
      let body: Record<string, unknown>;
      try {
        const raw: unknown = await c.req.json();
        if (!isObject(raw)) return c.json({ error: "Invalid JSON body" }, 400);
        body = raw;
      } catch {
        return c.json({ error: "Invalid JSON body" }, 400);
      }

      const repo = requireString(body, "repo");
      if (!repo) return c.json({ error: "repo is required" }, 400);

      try {
        const result = await callCli(["rebuild", "all", repo]);
        broadcastInvalidate(["rebuilds"], ["topics"], ["conflicts"]);
        return c.json(result);
      } catch (err) {
        return handleCliError(c, err);
      }
    })

    .post("/:env", async (c) => {
      const env = c.req.param("env");

      try {
        const result = await callCli(["rebuild", "env", env]);
        broadcastInvalidate(["rebuilds"], ["topics"], ["conflicts"]);
        return c.json(result);
      } catch (err) {
        return handleCliError(c, err);
      }
    });
}

function createRefreshRoutes() {
  return new Hono().post("/", async (c) => {
    let body: Record<string, unknown>;
    try {
      const raw: unknown = await c.req.json();
      if (!isObject(raw)) return c.json({ error: "Invalid JSON body" }, 400);
      body = raw;
    } catch {
      return c.json({ error: "Invalid JSON body" }, 400);
    }

    const args = ["refresh"];
    const repo = requireString(body, "repo");
    if (repo) {
      args.push("--repo", repo);
    }

    try {
      const result = await callCli(args);
      broadcastInvalidate(["topics"], ["topicEnvironments"], ["rebuilds"], ["conflicts"]);
      return c.json(result);
    } catch (err) {
      return handleCliError(c, err);
    }
  });
}

function createTopicEnvironmentRoutes() {
  return new Hono().get("/", async (c) => {
    try {
      const result = await callCli(["topic", "envs"]);
      return c.json(result);
    } catch (err) {
      return handleCliError(c, err);
    }
  });
}

function createRebuildsRoutes() {
  return new Hono().get("/", async (c) => {
    try {
      const result = await callCli(["rebuild", "list"]);
      return c.json(result);
    } catch (err) {
      return handleCliError(c, err);
    }
  });
}

function createConflictsRoutes() {
  return new Hono().get("/", async (c) => {
    try {
      const result = await callCli(["conflicts", "list"]);
      return c.json(result);
    } catch (err) {
      return handleCliError(c, err);
    }
  });
}

export interface UiServerConfig {
  port: number;
  staticRoot: string;
}

export function createUiApp(staticRoot: string) {
  const app = new Hono();
  const { injectWebSocket, upgradeWebSocket } = createNodeWebSocket({ app });

  app.get(
    "/ws",
    upgradeWebSocket(() => ({
      onOpen(_e, ws) {
        wsClients.add(ws as unknown as WsClient);
      },
      onClose(_e, ws) {
        wsClients.delete(ws as unknown as WsClient);
      },
    }))
  );

  app
    .get("/health", (c) => c.json({ status: "ok" }))
    .post("/api/cli/notify", async (c) => {
      let body: Record<string, unknown>;
      try {
        const raw: unknown = await c.req.json();
        if (!isObject(raw)) return c.json({ error: "Invalid JSON body" }, 400);
        body = raw;
      } catch {
        return c.json({ error: "Invalid JSON body" }, 400);
      }

      const queryKeys = body.queryKeys as string[][] | undefined;
      if (queryKeys && Array.isArray(queryKeys)) {
        broadcastInvalidate(...queryKeys);
      }
      return c.json({ broadcasted: true });
    })
    .route("/api/repos", createRepoRoutes())
    .route("/api/topics", createTopicRoutes())
    .route("/api/envs", createEnvRoutes())
    .route("/api/environments", createEnvRoutes())
    .route("/api/promote", createPromoteRoutes())
    .route("/api/rebuild", createRebuildRoutes())
    .route("/api/rebuilds", createRebuildsRoutes())
    .route("/api/topic-environments", createTopicEnvironmentRoutes())
    .route("/api/conflicts", createConflictsRoutes())
    .route("/api/refresh", createRefreshRoutes())
    .all("/api/*", (c) => c.json({ error: "Not found" }, 404))
    .use("/*", serveStatic({ root: staticRoot }))
    .get("/*", serveStatic({ root: staticRoot, path: "index.html" }));

  return { app, injectWebSocket };
}

export async function startUiServer(config: UiServerConfig): Promise<void> {
  const { app, injectWebSocket } = createUiApp(config.staticRoot);

  const server = serve(
    {
      fetch: app.fetch,
      port: config.port,
    },
    (info) => {
      console.log(`Restack UI: http://localhost:${info.port}`);
    }
  );

  injectWebSocket(server);
}
