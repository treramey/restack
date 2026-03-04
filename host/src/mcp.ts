/**
 * MCP Server - Model Context Protocol server for AI agent integration
 *
 * Exposes restack CLI commands as MCP tools over stdio transport.
 */
import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { z } from "zod";
import { callCli } from "./cli.js";
import { CliError } from "./types.js";

function cliErrorToString(err: unknown): string {
  if (err instanceof CliError) return err.message;
  return err instanceof Error ? err.message : String(err);
}

async function handleCli(args: readonly string[]): Promise<string> {
  try {
    const result = await callCli(args);
    return JSON.stringify(result, null, 2);
  } catch (err) {
    return JSON.stringify({ error: cliErrorToString(err) });
  }
}

export async function startMcp(): Promise<void> {
  const server = new McpServer({
    name: "restack",
    version: "0.1.0",
  });

  // --- Repo tools ---

  server.tool("repo_list", "List all tracked repositories", {}, async () => ({
    content: [{ type: "text", text: await handleCli(["repo", "list"]) }],
  }));

  server.tool(
    "repo_add",
    "Add a repository to restack tracking",
    { path: z.string().describe("Filesystem path to the git repository"), name: z.string().optional().describe("Display name for the repo") },
    async ({ path, name }) => {
      const args = ["repo", "add", path];
      if (name) args.push("--name", name);
      return { content: [{ type: "text", text: await handleCli(args) }] };
    }
  );

  server.tool(
    "repo_remove",
    "Remove a repository from restack tracking",
    { id: z.string().describe("Repository ID or name") },
    async ({ id }) => ({
      content: [{ type: "text", text: await handleCli(["repo", "remove", id]) }],
    })
  );

  // --- Topic tools ---

  server.tool(
    "topic_list",
    "List tracked topic branches",
    { repo: z.string().optional().describe("Filter by repository name") },
    async ({ repo }) => {
      const args = ["topic", "list"];
      if (repo) args.push("--repo", repo);
      return { content: [{ type: "text", text: await handleCli(args) }] };
    }
  );

  server.tool(
    "topic_track",
    "Start tracking a topic branch",
    { repo: z.string().describe("Repository name"), branch: z.string().describe("Branch name to track") },
    async ({ repo, branch }) => ({
      content: [{ type: "text", text: await handleCli(["topic", "track", "--repo", repo, branch]) }],
    })
  );

  server.tool(
    "topic_untrack",
    "Stop tracking a topic branch",
    { repo: z.string().describe("Repository name"), id: z.string().describe("Topic branch ID or name") },
    async ({ repo, id }) => ({
      content: [{ type: "text", text: await handleCli(["topic", "untrack", "--repo", repo, id]) }],
    })
  );

  server.tool(
    "topic_status",
    "Get status of a topic branch across environments",
    { repo: z.string().describe("Repository name"), id: z.string().describe("Topic branch ID or name") },
    async ({ repo, id }) => ({
      content: [{ type: "text", text: await handleCli(["topic", "status", "--repo", repo, id]) }],
    })
  );

  // --- Environment tools ---

  server.tool(
    "env_list",
    "List configured environments",
    { repo: z.string().optional().describe("Filter by repository name") },
    async ({ repo }) => {
      const args = ["env", "list"];
      if (repo) args.push("--repo", repo);
      return { content: [{ type: "text", text: await handleCli(args) }] };
    }
  );

  server.tool(
    "env_add",
    "Add an environment to a repository",
    {
      repo: z.string().describe("Repository name"),
      name: z.string().describe("Environment name (e.g. dev, staging, prod)"),
      branch: z.string().describe("Branch backing this environment"),
      ordinal: z.number().optional().describe("Sort order for promotion chain"),
      autoPromote: z.boolean().optional().describe("Auto-promote on CI pass"),
    },
    async ({ repo, name, branch, ordinal, autoPromote }) => {
      const args = ["env", "add", "--branch", branch, "--repo", repo, name];
      if (ordinal !== undefined) args.push("--ordinal", String(ordinal));
      if (autoPromote) args.push("--auto-promote");
      return { content: [{ type: "text", text: await handleCli(args) }] };
    }
  );

  server.tool(
    "env_status",
    "Get status of an environment",
    { name: z.string().describe("Environment name") },
    async ({ name }) => ({
      content: [{ type: "text", text: await handleCli(["env", "status", name]) }],
    })
  );

  // --- Promote tools ---

  server.tool(
    "promote_to",
    "Promote a topic branch into an environment",
    {
      repo: z.string().describe("Repository name"),
      topic: z.string().describe("Topic branch name"),
      env: z.string().describe("Target environment name"),
    },
    async ({ repo, topic, env }) => ({
      content: [{ type: "text", text: await handleCli(["promote", "to", "--repo", repo, topic, env]) }],
    })
  );

  server.tool(
    "promote_from",
    "Remove a topic branch from an environment",
    {
      repo: z.string().describe("Repository name"),
      topic: z.string().describe("Topic branch name"),
      env: z.string().describe("Environment name"),
    },
    async ({ repo, topic, env }) => ({
      content: [{ type: "text", text: await handleCli(["promote", "from", "--repo", repo, topic, env]) }],
    })
  );

  // --- Rebuild tools ---

  server.tool(
    "rebuild_env",
    "Rebuild a single environment branch",
    { env: z.string().describe("Environment ID to rebuild") },
    async ({ env }) => ({
      content: [{ type: "text", text: await handleCli(["rebuild", "env", env]) }],
    })
  );

  server.tool(
    "rebuild_all",
    "Rebuild all environment branches for a repository",
    { repo: z.string().describe("Repository ID") },
    async ({ repo }) => ({
      content: [{ type: "text", text: await handleCli(["rebuild", "all", repo]) }],
    })
  );

  // --- PR tools ---

  server.tool(
    "pr_create",
    "Create a pull request",
    {
      repo: z.string().describe("Repository name"),
      head: z.string().describe("Head branch"),
      base: z.string().describe("Base branch"),
      title: z.string().describe("PR title"),
      body: z.string().optional().describe("PR description"),
      draft: z.boolean().optional().describe("Create as draft PR"),
    },
    async ({ repo, head, base, title, body, draft }) => {
      const args = ["pr", "create", "--repo", repo, "--head", head, "--base", base, "--title", title];
      if (body) args.push("--body", body);
      if (draft) args.push("--draft");
      return { content: [{ type: "text", text: await handleCli(args) }] };
    }
  );

  server.tool(
    "pr_merge",
    "Merge a pull request",
    {
      repo: z.string().describe("Repository name"),
      prNumber: z.string().describe("PR number"),
      strategy: z.string().optional().describe("Merge strategy (merge, squash, rebase)"),
      deleteBranch: z.boolean().optional().describe("Delete branch after merge"),
    },
    async ({ repo, prNumber, strategy, deleteBranch }) => {
      const args = ["pr", "merge", "--repo", repo, prNumber];
      if (strategy) args.push("--strategy", strategy);
      if (deleteBranch) args.push("--delete-branch");
      return { content: [{ type: "text", text: await handleCli(args) }] };
    }
  );

  // --- Protection tools ---

  server.tool(
    "protection_apply",
    "Set branch protection rules for an environment branch",
    {
      repo: z.string().describe("Repository name"),
      branch: z.string().describe("Branch name to protect"),
      checks: z.array(z.string()).optional().describe("Required status checks"),
      requirePr: z.boolean().optional().describe("Require PR for merges"),
      minApprovals: z.number().optional().describe("Minimum number of approvals"),
    },
    async ({ repo, branch, checks, requirePr, minApprovals }) => {
      const args = ["protection", "set", "--repo", repo, "--branch", branch];
      if (checks && checks.length > 0) args.push("--checks", checks.join(","));
      if (requirePr) args.push("--require-pr");
      if (minApprovals !== undefined) args.push("--min-approvals", String(minApprovals));
      return { content: [{ type: "text", text: await handleCli(args) }] };
    }
  );

  // --- Pipeline tools ---

  server.tool(
    "pipeline_trigger",
    "Trigger a CI/CD pipeline for a branch",
    {
      repo: z.string().describe("Repository name"),
      branch: z.string().describe("Branch to trigger pipeline for"),
      name: z.string().optional().describe("Specific pipeline name"),
    },
    async ({ repo, branch, name }) => {
      const args = ["pipeline", "trigger", "--repo", repo, "--branch", branch];
      if (name) args.push("--name", name);
      return { content: [{ type: "text", text: await handleCli(args) }] };
    }
  );

  // --- CI tools ---

  server.tool(
    "ci_status",
    "Get CI/CD pipeline status for a repository",
    { repo: z.string().describe("Repository name") },
    async ({ repo }) => ({
      content: [{ type: "text", text: await handleCli(["ci", "status", "--repo", repo]) }],
    })
  );

  server.tool(
    "ci_generate",
    "Generate CI configuration for a repository",
    { repo: z.string().describe("Repository name") },
    async ({ repo }) => ({
      content: [{ type: "text", text: await handleCli(["ci", "generate", "--repo", repo]) }],
    })
  );

  // Connect transport
  const transport = new StdioServerTransport();
  await server.connect(transport);
}
