#!/usr/bin/env node
/**
 * Restack Host - Entry point for UI server
 *
 * Usage:
 *   restack-host ui --cli-path /path/to/restack --cwd /path/to/repo --static-root /path/to/dist --port 6969
 */
import { configureCli } from "./cli.js";
import { startMcp } from "./mcp.js";
import { startUiServer } from "./ui.js";

interface Args {
  mode: "ui" | "mcp";
  cliPath: string;
  cwd: string;
  staticRoot: string;
  port: number;
  contextRepo?: string;
}

function parseArgs(argv: readonly string[]): Args {
  const args = argv.slice(2);

  const mode = args[0];
  if (mode !== "ui" && mode !== "mcp") {
    printUsage();
    process.exit(1);
  }

  const result: Args = {
    mode,
    cliPath: "restack",
    cwd: process.cwd(),
    staticRoot: "./dist",
    port: 6969,
  };

  for (let i = 1; i < args.length; i++) {
    const arg = args[i];
    const next = args[i + 1];

    switch (arg) {
      case "--cli-path":
        if (!next) {
          console.error("--cli-path requires a value");
          process.exit(1);
        }
        result.cliPath = next;
        i++;
        break;
      case "--cwd":
        if (!next) {
          console.error("--cwd requires a value");
          process.exit(1);
        }
        result.cwd = next;
        i++;
        break;
      case "--static-root":
        if (!next) {
          console.error("--static-root requires a value");
          process.exit(1);
        }
        result.staticRoot = next;
        i++;
        break;
      case "--port":
        if (!next) {
          console.error("--port requires a value");
          process.exit(1);
        }
        result.port = parseInt(next, 10);
        if (isNaN(result.port)) {
          console.error(`Invalid port: ${next}`);
          process.exit(1);
        }
        i++;
        break;
      case "--context-repo":
        if (!next) {
          console.error("--context-repo requires a value");
          process.exit(1);
        }
        result.contextRepo = next;
        i++;
        break;
      case "--help":
      case "-h":
        printUsage();
        process.exit(0);
        break;
      default:
        console.error(`Unknown argument: ${arg}`);
        printUsage();
        process.exit(1);
    }
  }

  return result;
}

function printUsage(): void {
  console.log(`
Restack Host - UI and MCP server for Restack

Usage:
  restack-host <ui|mcp> [options]

Modes:
  ui     Start HTTP server with web UI
  mcp    Start MCP server over stdio (for AI agent integration)

Options:
  --cli-path <path>      Path to restack binary (default: "restack" in PATH)
  --cwd <path>           Working directory for CLI commands (default: current dir)
  --static-root <path>   Path to static files (default: ./dist) [ui mode only]
  --port <number>        HTTP port (default: 6969) [ui mode only]

Examples:
  restack-host ui --cli-path ./target/debug/restack --static-root ../ui/dist --port 8080
  restack-host mcp --cli-path ./target/debug/restack --cwd /path/to/repo
`.trim());
}

async function main(): Promise<void> {
  const args = parseArgs(process.argv);

  configureCli({
    cliPath: args.cliPath,
    cwd: args.cwd,
  });

  switch (args.mode) {
    case "mcp":
      await startMcp();
      break;
    case "ui":
      await startUiServer({
        port: args.port,
        staticRoot: args.staticRoot,
        contextRepo: args.contextRepo,
      });
      break;
  }
}

main().catch((err) => {
  console.error("Fatal error:", err);
  process.exit(1);
});
