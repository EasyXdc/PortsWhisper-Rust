const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const { buildArchiveCommand, packageRelease } = require("../scripts/package-release.js");
const { RELEASE_TARGETS } = require("../scripts/release-targets.js");

test("win32-x64 packaging uses powershell compress-archive", () => {
  const target = RELEASE_TARGETS["win32-x64"];
  const archivePath = "C:/tmp/ports-rs-windows-x64.zip";
  const stageDir = "C:/tmp/stage";

  const command = buildArchiveCommand(target, archivePath, stageDir);

  assert.equal(command.command, "powershell");
  assert.deepEqual(command.args.slice(0, 2), ["-NoProfile", "-Command"]);
  assert.match(command.args[2], /Compress-Archive/);
  assert.match(command.args[2], /ports\.exe/);
  assert.match(command.args[2], /whoisonport\.exe/);
});

test("unix packaging uses tar gz command", () => {
  const target = RELEASE_TARGETS["darwin-arm64"];
  const archivePath = "/tmp/ports-rs-darwin-arm64.tar.gz";
  const stageDir = "/tmp/stage";

  const command = buildArchiveCommand(target, archivePath, stageDir);

  assert.equal(command.command, "tar");
  assert.deepEqual(command.args, [
    "-czf",
    archivePath,
    "-C",
    stageDir,
    "ports",
    "whoisonport",
  ]);
});

test("package release removes temporary staging directory after archiving", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "ports-rs-package-test-"));
  const releaseDir = path.join(root, "target", "release");
  fs.mkdirSync(releaseDir, { recursive: true });
  fs.writeFileSync(path.join(releaseDir, "ports"), "ports");
  fs.writeFileSync(path.join(releaseDir, "whoisonport"), "whoisonport");

  let stagedDirectory;
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), "ports-rs-stage-root-"));

  try {
    packageRelease({
      root,
      targetKey: "darwin-arm64",
      makeTempDir(prefix) {
        stagedDirectory = fs.mkdtempSync(path.join(tempRoot, path.basename(prefix)));
        return stagedDirectory;
      },
      runCommand() {},
      log() {},
    });

    assert.equal(fs.existsSync(stagedDirectory), false);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
    fs.rmSync(tempRoot, { recursive: true, force: true });
  }
});

test("package release removes temporary staging directory after archive failure", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "ports-rs-package-test-"));
  const releaseDir = path.join(root, "target", "release");
  fs.mkdirSync(releaseDir, { recursive: true });
  fs.writeFileSync(path.join(releaseDir, "ports"), "ports");
  fs.writeFileSync(path.join(releaseDir, "whoisonport"), "whoisonport");

  let stagedDirectory;
  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), "ports-rs-stage-root-"));

  try {
    assert.throws(
      () =>
        packageRelease({
          root,
          targetKey: "darwin-arm64",
          makeTempDir(prefix) {
            stagedDirectory = fs.mkdtempSync(path.join(tempRoot, path.basename(prefix)));
            return stagedDirectory;
          },
          runCommand() {
            throw new Error("archive failed");
          },
          log() {},
        }),
      /archive failed/
    );

    assert.equal(fs.existsSync(stagedDirectory), false);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
    fs.rmSync(tempRoot, { recursive: true, force: true });
  }
});
