#!/usr/bin/env node

const fs = require("node:fs");
const path = require("node:path");
const os = require("node:os");
const { spawnSync } = require("node:child_process");
const { RELEASE_TARGETS } = require("./release-targets.js");

const ROOT = path.resolve(__dirname, "..");
const DIST = path.join(ROOT, "dist");
const TARGET = process.argv[2] || `${process.platform}-${process.arch}`;

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

function buildArchiveCommand(target, archivePath, stageDir) {
  if (target.archiveType === "tar.gz") {
    return {
      command: "tar",
      args: ["-czf", archivePath, "-C", stageDir, ...target.binaries],
    };
  }

  const sources = target.binaries
    .map((binary) => `'${path.join(stageDir, binary).replace(/\\/g, "/")}'`)
    .join(", ");
  const destination = archivePath.replace(/\\/g, "/");
  return {
    command: "powershell",
    args: [
      "-NoProfile",
      "-Command",
      `Compress-Archive -Path ${sources} -DestinationPath '${destination}' -Force`,
    ],
  };
}

function packageRelease({
  root = ROOT,
  targetKey = TARGET,
  makeTempDir = fs.mkdtempSync,
  runCommand = run,
  log = console.log,
} = {}) {
  const target = RELEASE_TARGETS[targetKey];
  if (!target) {
    fail(`unsupported packaging target ${targetKey}`);
  }

  const dist = path.join(root, "dist");
  fs.mkdirSync(dist, { recursive: true });

  const releaseDir = path.join(root, "target", "release");
  const stageDir = makeTempDir(path.join(os.tmpdir(), `ports-rs-${targetKey}-`));

  try {
    for (const binary of target.binaries) {
      const source = path.join(releaseDir, binary);
      ensureExists(source);
      fs.copyFileSync(source, path.join(stageDir, binary));
    }

    const archivePath = path.join(dist, target.archive);
    if (fs.existsSync(archivePath)) {
      fs.rmSync(archivePath, { force: true });
    }

    const archiveCommand = buildArchiveCommand(target, archivePath, stageDir);
    runCommand(archiveCommand.command, archiveCommand.args);

    log(archivePath);
  } finally {
    fs.rmSync(stageDir, { recursive: true, force: true });
  }
}

function main() {
  packageRelease();
}

if (require.main === module) {
  main();
}

module.exports = {
  TARGETS: RELEASE_TARGETS,
  buildArchiveCommand,
  packageRelease,
};
