#!/usr/bin/env node

const fs = require("node:fs");
const path = require("node:path");
const os = require("node:os");
const { spawnSync } = require("node:child_process");

const ROOT = path.resolve(__dirname, "..");
const DIST = path.join(ROOT, "dist");
const TARGET = process.argv[2] || `${process.platform}-${process.arch}`;

const TARGETS = {
  "darwin-arm64": {
    archive: "ports-rs-darwin-arm64.tar.gz",
    binaries: ["ports", "whoisonport"],
    extension: "tar.gz"
  },
  "darwin-x64": {
    archive: "ports-rs-darwin-x64.tar.gz",
    binaries: ["ports", "whoisonport"],
    extension: "tar.gz"
  },
  "linux-x64": {
    archive: "ports-rs-linux-x64.tar.gz",
    binaries: ["ports", "whoisonport"],
    extension: "tar.gz"
  },
  "win32-x64": {
    archive: "ports-rs-windows-x64.zip",
    binaries: ["ports.exe", "whoisonport.exe"],
    extension: "zip"
  }
};

function fail(message) {
  console.error(`package-release: ${message}`);
  process.exit(1);
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, { stdio: "inherit", ...options });
  if (result.status !== 0) {
    fail(`${command} ${args.join(" ")} failed`);
  }
}

function ensureExists(filePath) {
  if (!fs.existsSync(filePath)) {
    fail(`missing expected file ${filePath}`);
  }
}

function main() {
  const target = TARGETS[TARGET];
  if (!target) {
    fail(`unsupported packaging target ${TARGET}`);
  }

  fs.mkdirSync(DIST, { recursive: true });

  const releaseDir = path.join(ROOT, "target", "release");
  const stageDir = fs.mkdtempSync(path.join(os.tmpdir(), `ports-rs-${TARGET}-`));

  for (const binary of target.binaries) {
    const source = path.join(releaseDir, binary);
    ensureExists(source);
    fs.copyFileSync(source, path.join(stageDir, binary));
  }

  const archivePath = path.join(DIST, target.archive);
  if (fs.existsSync(archivePath)) {
    fs.rmSync(archivePath, { force: true });
  }

  if (target.extension === "tar.gz") {
    run("tar", ["-czf", archivePath, "-C", stageDir, ...target.binaries]);
  } else {
    run("zip", ["-j", archivePath, ...target.binaries.map((binary) => path.join(stageDir, binary))]);
  }

  console.log(archivePath);
}

main();
