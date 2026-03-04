#!/usr/bin/env node
const { chmodSync } = require("node:fs");
const { join } = require("node:path");

try {
  chmodSync(join(__dirname, "restack"), 0o755);
} catch {
  // Ignore errors (e.g., Windows)
}
