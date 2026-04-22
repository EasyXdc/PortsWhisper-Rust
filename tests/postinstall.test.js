const test = require("node:test");
const assert = require("node:assert/strict");
const packageJson = require("../package.json");

const {
  defaultReleaseTag,
  defaultBaseUrl,
  buildZipExtractCommand,
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
