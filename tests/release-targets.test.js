const test = require("node:test");
const assert = require("node:assert/strict");

const {
  RELEASE_TARGETS,
  releaseTargetForPlatform,
  expectedBinaryNames,
} = require("../scripts/release-targets.js");

test("releaseTargetForPlatform resolves unix targets", () => {
  assert.deepEqual(releaseTargetForPlatform("darwin", "arm64"), {
    key: "darwin-arm64",
    archive: "ports-rs-darwin-arm64.tar.gz",
    archiveType: "tar.gz",
    binaries: ["ports", "whoisonport"],
  });
});

test("releaseTargetForPlatform resolves windows targets", () => {
  assert.deepEqual(releaseTargetForPlatform("win32", "x64"), {
    key: "win32-x64",
    archive: "ports-rs-windows-x64.zip",
    archiveType: "zip",
    binaries: ["ports.exe", "whoisonport.exe"],
  });
});

test("expectedBinaryNames matches platform rules", () => {
  assert.deepEqual(expectedBinaryNames("darwin"), ["ports", "whoisonport"]);
  assert.deepEqual(expectedBinaryNames("win32"), ["ports.exe", "whoisonport.exe"]);
});

test("release targets export every supported archive name", () => {
  assert.deepEqual(
    Object.keys(RELEASE_TARGETS).sort(),
    ["darwin-arm64", "darwin-x64", "linux-x64", "win32-x64"]
  );
});
