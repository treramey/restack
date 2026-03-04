#!/usr/bin/env node
/**
 * Postinstall script to ensure platform-specific binary is installed.
 *
 * npm's optionalDependencies may not install when users have `omit=optional`
 * in their npm config. This script detects that and installs the correct
 * platform package.
 */
import { execFileSync } from "node:child_process";
import { createRequire } from "node:module";
import { platform, arch, env } from "node:process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { readFileSync } from "node:fs";

const require = createRequire(import.meta.url);
const pkgRoot = dirname(dirname(fileURLToPath(import.meta.url)));

const PLATFORMS = {
  "darwin-arm64": "@restack-cli/darwin-arm64",
  "darwin-x64": "@restack-cli/darwin-x64",
  "linux-x64": "@restack-cli/linux-x64",
  "win32-x64": "@restack-cli/win32-x64",
};

function ensureInstalled() {
  if (env.RESTACK_SKIP_POSTINSTALL === "1") return;

  const cpuArch = arch === "arm64" ? "arm64" : "x64";
  const key = `${platform}-${cpuArch}`;
  const pkg = PLATFORMS[key];

  if (!pkg) return;

  try {
    require.resolve(`${pkg}/package.json`);
    return;
  } catch {
    // Not installed, try to install
  }

  const pkgJson = JSON.parse(
    readFileSync(join(pkgRoot, "package.json"), "utf8"),
  );
  const version = pkgJson.version;
  const ua = env.npm_config_user_agent || "";
  const isNpm = ua.includes("npm/");

  if (!isNpm) {
    console.error(`\n⚠️  Platform package missing: ${pkg}`);
    console.error(
      `   Your package manager may be omitting optionalDependencies.`,
    );
    console.error(`   Install manually: npm install -g ${pkg}@${version}\n`);
    process.exit(1);
  }

  console.log(`Installing platform package: ${pkg}@${version}`);

  try {
    execFileSync(
      platform === "win32" ? "npm.cmd" : "npm",
      [
        "install",
        "--no-save",
        "--no-package-lock",
        "--silent",
        "--prefix",
        pkgRoot,
        `${pkg}@${version}`,
      ],
      { stdio: "inherit" },
    );
  } catch {
    console.error(`\n⚠️  Failed to install platform package: ${pkg}`);
    console.error(
      `   Try: npm install -g @restack-cli/restack --include=optional\n`,
    );
    process.exit(1);
  }
}

ensureInstalled();
