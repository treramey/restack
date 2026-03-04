#!/usr/bin/env node
/**
 * Restack Host - Entry point for UI server
 *
 * Usage:
 *   restack-host ui --cli-path /path/to/restack --cwd /path/to/repo --static-root /path/to/dist --port 6969
 */
import { configureCli } from "./cli.js";
import { startUiServer } from "./ui.js";

interface Args {
  mode: "ui";
  cliPath: string;
  cwd: string;
  staticRoot: string;
  port: number;
}

function parseArgs(argv: readonly string[]): Args {
  const args = argv.slice(2);

  if (args.length === 0 || args[0] !== "ui") {
    printUsage();
    process.exit(1);
  }

  const result: Args = {
    mode: "ui",
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
Restack Host - UI server for Restack

Usage:
  restack-host ui [options]

Options:
  --cli-path <path>      Path to restack binary (default: "restack" in PATH)
  --cwd <path>           Working directory for CLI commands (default: current dir)
  --static-root <path>   Path to static files (default: ./dist)
  --port <number>        HTTP port (default: 6969)

Examples:
  restack-host ui --cli-path ./target/debug/restack --static-root ../ui/dist --port 8080
`.trim());
}

async function main(): Promise<void> {
  const args = parseArgs(process.argv);

  configureCli({
    cliPath: args.cliPath,
    cwd: args.cwd,
  });

  await startUiServer({
    port: args.port,
    staticRoot: args.staticRoot,
  });
}

main().catch((err) => {
  console.error("Fatal error:", err);
  process.exit(1);
});
