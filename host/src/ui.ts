/**
 * UI Server - Hono HTTP server for Restack
 */
import { serve } from "@hono/node-server";
import { Hono, type Context } from "hono";
import { serveStatic } from "@hono/node-server/serve-static";
import type { StatusCode } from "hono/utils/http-status";
import { callCli } from "./cli.js";
import { CliError } from "./types.js";

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

    .post("/", async (c) => {
      let body: Record<string, unknown>;
      try {
        const raw: unknown = await c.req.json();
        if (!isObject(raw)) return c.json({ error: "Invalid JSON body" }, 400);
        body = raw;
      } catch {
        return c.json({ error: "Invalid JSON body" }, 400);
      }

      const path = requireString(body, "path");
      if (!path) return c.json({ error: "path is required" }, 400);

      const args = ["repo", "add", path];
      const name = requireString(body, "name");
      if (name) args.push("--name", name);

      try {
        const result = await callCli(args);
        return c.json(result, 201);
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

    .post("/sync", async (c) => {
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
        const result = await callCli(["topic", "sync", "--repo", repo]);
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

      const topic = requireString(body, "topic");
      const env = requireString(body, "env");
      const repo = requireString(body, "repo");
      if (!topic) return c.json({ error: "topic is required" }, 400);
      if (!env) return c.json({ error: "env is required" }, 400);
      if (!repo) return c.json({ error: "repo is required" }, 400);

      try {
        const result = await callCli([
          "promote", "to", "--repo", repo, topic, env,
        ]);
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

      const topic = requireString(body, "topic");
      const env = requireString(body, "env");
      const repo = requireString(body, "repo");
      if (!topic) return c.json({ error: "topic is required" }, 400);
      if (!env) return c.json({ error: "env is required" }, 400);
      if (!repo) return c.json({ error: "repo is required" }, 400);

      try {
        const result = await callCli([
          "promote", "from", "--repo", repo, topic, env,
        ]);
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
        return c.json(result);
      } catch (err) {
        return handleCliError(c, err);
      }
    })

    .post("/:env", async (c) => {
      const env = c.req.param("env");

      try {
        const result = await callCli(["rebuild", "env", env]);
        return c.json(result);
      } catch (err) {
        return handleCliError(c, err);
      }
    });
}

/**
 * CI routes
 */
function createCiRoutes() {
  return new Hono()
    .get("/status", async (c) => {
      const repo = c.req.query("repo");
      if (!repo) return c.json({ error: "repo query param required" }, 400);
      try {
        const result = await callCli(["ci", "status", "--repo", repo]);
        return c.json(result);
      } catch (err) {
        return handleCliError(c, err);
      }
    })
    .post("/generate", async (c) => {
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
        const result = await callCli(["ci", "generate", "--repo", repo]);
        return c.json(result);
      } catch (err) {
        return handleCliError(c, err);
      }
    });
}

/**
 * PR management routes
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
      const bodyText = requireString(body, "body");
      if (bodyText) args.push("--body", bodyText);
      const draft = requireBool(body, "draft");
      if (draft) args.push("--draft");

      try {
        const result = await callCli(args);
        return c.json(result, 201);
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
      const prNumber = requireString(body, "prNumber");
      if (!repo) return c.json({ error: "repo is required" }, 400);
      if (!prNumber) return c.json({ error: "prNumber is required" }, 400);

      const args = ["pr", "merge", "--repo", repo, prNumber];
      const strategy = requireString(body, "strategy");
      if (strategy) args.push("--strategy", strategy);
      const deleteBranch = requireBool(body, "deleteBranch");
      if (deleteBranch) args.push("--delete-branch");

      try {
        const result = await callCli(args);
        return c.json(result);
      } catch (err) {
        return handleCliError(c, err);
      }
    });
}

/**
 * Branch protection routes
 */
function createProtectionRoutes() {
  return new Hono()
    .post("/set", async (c) => {
      let body: Record<string, unknown>;
      try {
        const raw: unknown = await c.req.json();
        if (!isObject(raw)) return c.json({ error: "Invalid JSON body" }, 400);
        body = raw;
      } catch {
        return c.json({ error: "Invalid JSON body" }, 400);
      }

      const repo = requireString(body, "repo");
      const branch = requireString(body, "branch");
      if (!repo) return c.json({ error: "repo is required" }, 400);
      if (!branch) return c.json({ error: "branch is required" }, 400);

      const args = ["protection", "set", "--repo", repo, "--branch", branch];
      const checks = body["checks"];
      if (Array.isArray(checks)) {
        const checkStrs = checks.filter((c): c is string => typeof c === "string");
        if (checkStrs.length > 0) args.push("--checks", checkStrs.join(","));
      }
      const requirePr = requireBool(body, "requirePr");
      if (requirePr) args.push("--require-pr");
      const minApprovals = requireNumber(body, "minApprovals");
      if (minApprovals !== undefined) args.push("--min-approvals", String(minApprovals));

      try {
        const result = await callCli(args);
        return c.json(result);
      } catch (err) {
        return handleCliError(c, err);
      }
    })

    .post("/envs", async (c) => {
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
        const result = await callCli(["protection", "envs", "--repo", repo]);
        return c.json(result);
      } catch (err) {
        return handleCliError(c, err);
      }
    });
}

/**
 * Pipeline routes
 */
function createPipelineRoutes() {
  return new Hono()
    .post("/trigger", async (c) => {
      let body: Record<string, unknown>;
      try {
        const raw: unknown = await c.req.json();
        if (!isObject(raw)) return c.json({ error: "Invalid JSON body" }, 400);
        body = raw;
      } catch {
        return c.json({ error: "Invalid JSON body" }, 400);
      }

      const repo = requireString(body, "repo");
      const branch = requireString(body, "branch");
      if (!repo) return c.json({ error: "repo is required" }, 400);
      if (!branch) return c.json({ error: "branch is required" }, 400);

      const args = ["pipeline", "trigger", "--repo", repo, "--branch", branch];
      const name = requireString(body, "name");
      if (name) args.push("--name", name);

      try {
        const result = await callCli(args);
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

/**
 * Create UI app with API routes and static file serving
 */
export function createUiApp(staticRoot: string) {
  const api = new Hono()
    .get("/health", (c) => c.json({ status: "ok" }))
    .route("/api/repos", createRepoRoutes())
    .route("/api/topics", createTopicRoutes())
    .route("/api/envs", createEnvRoutes())
    .route("/api/promote", createPromoteRoutes())
    .route("/api/rebuild", createRebuildRoutes())
    .route("/api/ci", createCiRoutes())
    .route("/api/pr", createPrRoutes())
    .route("/api/protection", createProtectionRoutes())
    .route("/api/pipeline", createPipelineRoutes())
    .all("/api/*", (c) => c.json({ error: "Not found" }, 404));

  return new Hono()
    .route("/", api)
    .use("/*", serveStatic({ root: staticRoot }))
    .get("/*", serveStatic({ root: staticRoot, path: "index.html" }));
}

/**
 * Start UI server
 */
export async function startUiServer(config: UiServerConfig): Promise<void> {
  const app = createUiApp(config.staticRoot);

  serve(
    {
      fetch: app.fetch,
      port: config.port,
    },
    (info) => {
      console.log(`Restack UI: http://localhost:${info.port}`);
    }
  );
}
