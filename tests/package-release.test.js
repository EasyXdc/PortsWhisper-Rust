const test = require("node:test");
const assert = require("node:assert/strict");

const { buildArchiveCommand } = require("../scripts/package-release.js");
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
