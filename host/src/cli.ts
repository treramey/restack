/**
 * CLI bridge - persistent `restack serve` child process with NDJSON communication
 */
import { spawn, type ChildProcess } from "node:child_process";
import { createInterface, type Interface } from "node:readline";
import { CliError, CliTimeoutError } from "./types.js";

const CLI_TIMEOUT_MS = 30_000;

export interface CliConfig {
  /** Path to the restack binary */
  cliPath: string;
  /** Working directory for CLI commands */
  cwd: string;
}

// Global config, set by main entry point
let config: CliConfig = {
  cliPath: "restack",
  cwd: process.cwd(),
};

/**
 * Configure the CLI bridge
 */
export function configureCli(newConfig: CliConfig): void {
  config = newConfig;
}

/**
 * Get current CLI config
 */
export function getCliConfig(): CliConfig {
  return config;
}

// --- Persistent process management ---

let daemonProcess: ChildProcess | null = null;
let stdoutReader: Interface | null = null;
let requestId = 0;
const pendingRequests = new Map<
  string,
  {
    resolve: (value: unknown) => void;
    reject: (reason: Error) => void;
    timer: ReturnType<typeof setTimeout>;
  }
>();

function settlePending(
  id: string,
  outcome: { value: unknown } | { error: Error }
): void {
  const pending = pendingRequests.get(id);
  if (!pending) return;
  pendingRequests.delete(id);
  clearTimeout(pending.timer);
  if ("value" in outcome) {
    pending.resolve(outcome.value);
  } else {
    pending.reject(outcome.error);
  }
}

function getOrSpawnDaemon(): ChildProcess {
  if (
    daemonProcess !== null &&
    !daemonProcess.killed &&
    daemonProcess.exitCode === null
  ) {
    return daemonProcess;
  }

  // Spawn persistent `restack serve` process
  const proc = spawn(config.cliPath, ["serve"], {
    cwd: config.cwd,
    stdio: ["pipe", "pipe", "pipe"],
  });

  daemonProcess = proc;

  if (!proc.stdout || !proc.stderr) {
    throw new CliError("restack serve: failed to capture stdio", -1, "");
  }

  // Parse stdout line by line for NDJSON responses
  stdoutReader = createInterface({ input: proc.stdout });
  stdoutReader.on("line", (line: string) => {
    if (!line.trim()) return;
    try {
      const raw: unknown = JSON.parse(line);
      if (
        typeof raw !== "object" ||
        raw === null ||
        !("id" in raw) ||
        typeof (raw as Record<string, unknown>).id !== "string"
      ) {
        return;
      }
      const resp = raw as {
        id: string;
        result?: unknown;
        error?: string;
      };
      if (resp.error !== undefined) {
        settlePending(resp.id, { error: new CliError(resp.error, 1, resp.error) });
      } else {
        settlePending(resp.id, { value: resp.result });
      }
    } catch {
      // Ignore unparseable lines (e.g. stderr leaking to stdout)
    }
  });

  // Handle process exit - reject all pending requests
  proc.on("exit", (code) => {
    stdoutReader?.close();
    for (const id of [...pendingRequests.keys()]) {
      settlePending(id, {
        error: new CliError(
          `restack serve exited with code ${String(code)}`,
          code ?? -1,
          ""
        ),
      });
    }
    daemonProcess = null;
    stdoutReader = null;
  });

  proc.on("error", (err) => {
    stdoutReader?.close();
    for (const id of [...pendingRequests.keys()]) {
      settlePending(id, {
        error: new CliError(`Failed to spawn restack: ${err.message}`, -1, ""),
      });
    }
    daemonProcess = null;
    stdoutReader = null;
  });

  // Forward stderr for debugging
  proc.stderr.on("data", (chunk: Buffer) => {
    process.stderr.write(chunk);
  });

  return proc;
}

/**
 * Execute restack CLI command via persistent serve process (NDJSON protocol)
 */
export async function callCli(args: readonly string[]): Promise<unknown> {
  return new Promise((resolve, reject) => {
    const daemon = getOrSpawnDaemon();

    if (daemon.stdin === null || daemon.stdin.destroyed) {
      reject(new CliError("restack serve stdin not available", -1, ""));
      return;
    }

    const id = `req_${String(++requestId)}`;

    const timer = setTimeout(() => {
      settlePending(id, { error: new CliTimeoutError() });
    }, CLI_TIMEOUT_MS);

    pendingRequests.set(id, { resolve, reject, timer });

    const request = JSON.stringify({ id, args: [...args] }) + "\n";
    daemon.stdin.write(request, (err) => {
      if (err) {
        settlePending(id, {
          error: new CliError(
            `Failed to write to restack serve: ${err.message}`,
            -1,
            ""
          ),
        });
      }
    });
  });
}

/**
 * Gracefully shut down the persistent process
 */
export function shutdownCli(): void {
  const proc = daemonProcess;

  for (const id of [...pendingRequests.keys()]) {
    settlePending(id, { error: new CliError("CLI shutting down", -1, "") });
  }

  stdoutReader?.close();

  if (proc !== null && !proc.killed) {
    proc.stdin?.end();

    const killTimer = setTimeout(() => {
      if (!proc.killed) {
        proc.kill("SIGTERM");
      }
    }, 2000);

    proc.on("exit", () => {
      clearTimeout(killTimer);
    });
  }
}
