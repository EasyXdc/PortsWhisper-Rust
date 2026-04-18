#!/usr/bin/env node

const { spawn } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

function binaryName(base) {
  return process.platform === "win32" ? `${base}.exe` : base;
}

function fail(message) {
  console.error(`ports-rs: ${message}`);
  console.error("Install the matching GitHub Release binary manually, or run `cargo install --path .`.");
  process.exit(1);
}

const binaryPath = path.resolve(__dirname, "..", "vendor", binaryName("whoisonport"));
if (!fs.existsSync(binaryPath)) {
  fail(`missing bundled binary at ${binaryPath}`);
}

const child = spawn(binaryPath, process.argv.slice(2), { stdio: "inherit" });
child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code ?? 1);
});
child.on("error", (error) => fail(error.message));
