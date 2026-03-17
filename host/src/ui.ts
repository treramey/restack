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

const WS_OPEN = 1;

type WsClient = { send: (data: string) => void; readyState: number };
const wsClients = new Set<WsClient>();

export type WsEvent =
  | { type: "invalidate"; queryKeys: string[][] }
  | { type: "refreshStatus"; status: "running" | "done" | "error"; error?: string };

function broadcast(event: WsEvent) {
  const data = JSON.stringify(event);
  for (const client of wsClients) {
    if (client.readyState === WS_OPEN) {
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
        const result = await callCli(["list"]);
        return c.json(result);
      } catch (err) {
        return handleCliError(c, err);
      }
    })
    .delete("/:id", async (c) => {
      const id = c.req.param("id");

      try {
        const result = await callCli(["remove", id]);
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

    .post("/:id/close", async (c) => {
      const id = c.req.param("id");
      const repo = c.req.query("repo");
      if (!repo) return c.json({ error: "repo query param is required" }, 400);

      try {
        const result = await callCli(["topic", "close", "--repo", repo, id]);
        broadcastInvalidate(["topics", "topicEnvironments"]);
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

      try {
        if (repo) {
          const result = await callCli(["integration", "list", "--repo", repo]);
          return c.json(result);
        }
        // No repo param: aggregate envs across all repos
        const repos = await callCli(["list"]);
        if (!Array.isArray(repos)) return c.json([]);
        const all = await Promise.all(
          repos.map(async (r: { id: string }) => {
            try {
              const envs = await callCli(["integration", "list", "--repo", r.id]);
              return Array.isArray(envs) ? envs : [];
            } catch {
              return [];
            }
          }),
        );
        return c.json(all.flat());
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

      const args = ["integration", "add", "--branch", branch, "--repo", repo, name];
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
        const result = await callCli(["integration", "status", name]);
        return c.json(result);
      } catch (err) {
        return handleCliError(c, err);
      }
    });
}

/**
 * Promote routes
 * 
 * Requires repoId from client - eliminates TOCTOU race condition from
 * sequential CLI calls during topic/repo lookup.
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
      const repoId = requireString(body, "repoId");
      if (!topicId) return c.json({ error: "topicId is required" }, 400);
      if (!envId) return c.json({ error: "envId is required" }, 400);
      if (!repoId) return c.json({ error: "repoId is required" }, 400);

      try {
        const envResult = await callCli(["integration", "list", "--repo", repoId]);
        if (!Array.isArray(envResult)) {
          return c.json({ error: "Invalid environment list response" }, 500);
        }
        const env = envResult.find((e): e is { id: string; name: string } => 
          typeof e === "object" && e !== null && "id" in e && "name" in e && e.id === envId
        );
        if (!env) return c.json({ error: "Environment not found" }, 404);

        const result = await callCli([
          "topic", "promote", "--repo", repoId, topicId, env.name,
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
      const repoId = requireString(body, "repoId");
      if (!topicId) return c.json({ error: "topicId is required" }, 400);
      if (!envId) return c.json({ error: "envId is required" }, 400);
      if (!repoId) return c.json({ error: "repoId is required" }, 400);

      try {
        const envResult = await callCli(["integration", "list", "--repo", repoId]);
        if (!Array.isArray(envResult)) {
          return c.json({ error: "Invalid environment list response" }, 500);
        }
        const env = envResult.find((e): e is { id: string; name: string } => 
          typeof e === "object" && e !== null && "id" in e && "name" in e && e.id === envId
        );
        if (!env) return c.json({ error: "Environment not found" }, 404);

        const result = await callCli([
          "topic", "demote", "--repo", repoId, topicId, env.name,
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

/** In-flight refresh guard: prevents concurrent refreshes from stacking up. */
let refreshInFlight = false;

function runBackgroundRefresh(repo?: string) {
  if (refreshInFlight) return;
  refreshInFlight = true;

  const args = ["refresh"];
  if (repo) args.push("--repo", repo);

  broadcast({ type: "refreshStatus", status: "running" });

  callCli(args)
    .then(() => {
      broadcastInvalidate(["repos"], ["topics"], ["environments"], ["topicEnvironments"], ["rebuilds"], ["conflicts"]);
      broadcast({ type: "refreshStatus", status: "done" });
    })
    .catch((err: unknown) => {
      const message = err instanceof Error ? err.message : String(err);
      broadcast({ type: "refreshStatus", status: "error", error: message });
    })
    .finally(() => {
      refreshInFlight = false;
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

    const repo = requireString(body, "repo");

    if (refreshInFlight) {
      return c.json({ status: "already_running" }, 202);
    }

    runBackgroundRefresh(repo);
    return c.json({ status: "started" }, 202);
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
  return new Hono().get("/", (c) => {
    // rebuild CLI commands are not yet available in the flattened CLI
    return c.json([]);
  });
}

function createConflictsRoutes() {
  return new Hono().get("/", (c) => {
    // conflicts CLI commands are not yet available in the flattened CLI
    return c.json([]);
  });
}

/**
 * PR routes
 */
function createPrRoutes() {
  return new Hono()
    .post("/create", async (c) => {
      let body: Record<string, unknown>;
      try {
        const raw: unknown = await c.req.json();
        if (!isObject(raw)) return c.json({ error: "Invalid JSON body" }, 400);
        body = raw;
      } catch {
        return c.json({ error: "Invalid JSON body" }, 400);
      }

      const repo = requireString(body, "repo");
      const head = requireString(body, "head");
      const base = requireString(body, "base");
      const title = requireString(body, "title");
      if (!repo) return c.json({ error: "repo is required" }, 400);
      if (!head) return c.json({ error: "head is required" }, 400);
      if (!base) return c.json({ error: "base is required" }, 400);
      if (!title) return c.json({ error: "title is required" }, 400);

      const args = ["pr", "create", "--repo", repo, "--head", head, "--base", base, "--title", title];
      const prBody = requireString(body, "body");
      if (prBody) args.push("--body", prBody);
      const draft = requireBool(body, "draft");
      if (draft) args.push("--draft");

      try {
        const result = await callCli(args);
        broadcastInvalidate(["topics"]);
        return c.json(result);
      } catch (err) {
        return handleCliError(c, err);
      }
    })

    .post("/merge", async (c) => {
      let body: Record<string, unknown>;
      try {
        const raw: unknown = await c.req.json();
        if (!isObject(raw)) return c.json({ error: "Invalid JSON body" }, 400);
        body = raw;
      } catch {
        return c.json({ error: "Invalid JSON body" }, 400);
      }

      const repo = requireString(body, "repo");
      const prNumber = requireNumber(body, "prNumber");
      if (!repo) return c.json({ error: "repo is required" }, 400);
      if (prNumber === undefined) return c.json({ error: "prNumber is required" }, 400);

      const args = ["pr", "merge", "--repo", repo, String(prNumber)];
      const strategy = requireString(body, "strategy");
      if (strategy) args.push("--strategy", strategy);
      const deleteBranch = requireBool(body, "deleteBranch");
      if (deleteBranch) args.push("--delete-branch");

      try {
        const result = await callCli(args);
        broadcastInvalidate(["topics"]);
        return c.json(result);
      } catch (err) {
        return handleCliError(c, err);
      }
    });
}

export interface UiServerConfig {
  port: number;
  staticRoot: string;
  contextRepo?: string;
}

export function createUiApp(staticRoot: string, contextRepo?: string) {
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
    .get("/api/context", (c) => {
      return c.json({ repoName: contextRepo ?? null });
    })
    .post("/api/cli/notify", async (c) => {
      let body: Record<string, unknown>;
      try {
        const raw: unknown = await c.req.json();
        if (!isObject(raw)) return c.json({ error: "Invalid JSON body" }, 400);
        body = raw;
      } catch {
        return c.json({ error: "Invalid JSON body" }, 400);
      }

      const rawQueryKeys = body.queryKeys;
      if (
        Array.isArray(rawQueryKeys) &&
        rawQueryKeys.every(
          (k) => Array.isArray(k) && k.every((s) => typeof s === "string")
        )
      ) {
        broadcastInvalidate(...(rawQueryKeys as string[][]));
      }
      return c.json({ broadcasted: true });
    })
    .route("/api/repos", createRepoRoutes())
    .route("/api/topics", createTopicRoutes())
    .route("/api/envs", createEnvRoutes())
    .route("/api/promote", createPromoteRoutes())
    .route("/api/rebuild", createRebuildRoutes())
    .route("/api/rebuilds", createRebuildsRoutes())
    .route("/api/topic-environments", createTopicEnvironmentRoutes())
    .route("/api/conflicts", createConflictsRoutes())
    .route("/api/refresh", createRefreshRoutes())
    .route("/api/pr", createPrRoutes())
    .all("/api/*", (c) => c.json({ error: "Not found" }, 404))
    .use("/*", serveStatic({ root: staticRoot }))
    .get("/*", serveStatic({ root: staticRoot, path: "index.html" }));

  return { app, injectWebSocket };
}

export async function startUiServer(config: UiServerConfig): Promise<void> {
  const { app, injectWebSocket } = createUiApp(config.staticRoot, config.contextRepo);

  const server = serve(
    {
      fetch: app.fetch,
      port: config.port,
    },
    (info) => {
      console.log(`Restack UI: http://localhost:${info.port}`);

      // Auto-refresh in background: UI loads cached DB data immediately,
      // then updates stream in via WebSocket when refresh completes.
      runBackgroundRefresh();
    }
  );

  injectWebSocket(server);
}
