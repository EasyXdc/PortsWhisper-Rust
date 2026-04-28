const test = require("node:test");
const assert = require("node:assert/strict");
const crypto = require("node:crypto");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const packageJson = require("../package.json");

const {
  defaultReleaseTag,
  defaultBaseUrl,
  buildZipExtractCommand,
  expectedSha256FromSums,
  verifyArchiveChecksum,
} = require("../scripts/postinstall.js");

test("postinstall defaults to the current package version tag", () => {
  assert.equal(defaultReleaseTag(), `v${packageJson.version}`);
});

test("postinstall default download URL uses the current package version tag", () => {
  assert.equal(
    defaultBaseUrl(),
    `https://github.com/EasyXdc/PortsWhisper-Rust/releases/download/v${packageJson.version}`
  );
});

test("windows zip extraction uses powershell expand-archive instead of unzip", () => {
  const command = buildZipExtractCommand("C:/tmp/ports-rs-windows-x64.zip", "C:/tmp/vendor");

  assert.equal(command.command, "powershell");
  assert.deepEqual(command.args.slice(0, 2), ["-NoProfile", "-Command"]);
  assert.match(command.args[2], /Expand-Archive/);
  assert.doesNotMatch(command.args[2], /unzip/);
});

test("windows zip extraction escapes apostrophes in paths for powershell", () => {
  const command = buildZipExtractCommand(
    "C:/Users/O'Brien/tmp/ports-rs-windows-x64.zip",
    "C:/Users/O'Brien/vendor"
  );

  assert.match(command.args[2], /O''Brien/);
});

test("postinstall extracts expected sha256 from release checksum file", () => {
  const sums = [
    "1111111111111111111111111111111111111111111111111111111111111111  ports-rs-darwin-x64.tar.gz",
    "2222222222222222222222222222222222222222222222222222222222222222  ports-rs-linux-x64.tar.gz",
  ].join("\n");

  assert.equal(
    expectedSha256FromSums(sums, "ports-rs-linux-x64.tar.gz"),
    "2222222222222222222222222222222222222222222222222222222222222222"
  );
});

test("postinstall reports missing archive entries in checksum file", () => {
  const sums = "1111111111111111111111111111111111111111111111111111111111111111  ports-rs-darwin-x64.tar.gz";

  assert.throws(
    () => expectedSha256FromSums(sums, "ports-rs-linux-x64.tar.gz"),
    /SHA256SUMS did not contain ports-rs-linux-x64\.tar\.gz/
  );
});

test("postinstall rejects checksum mismatches", () => {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "ports-rs-checksum-test-"));
  const archivePath = path.join(tempDir, "ports-rs-linux-x64.tar.gz");
  fs.writeFileSync(archivePath, "archive contents");

  try {
    const actual = crypto.createHash("sha256").update("archive contents").digest("hex");
    assert.doesNotThrow(() => verifyArchiveChecksum(archivePath, actual));
    assert.throws(
      () => verifyArchiveChecksum(archivePath, "0".repeat(64)),
      /checksum mismatch/
    );
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
});
