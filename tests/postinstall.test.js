const test = require("node:test");
const assert = require("node:assert/strict");
const packageJson = require("../package.json");

const { defaultReleaseTag, defaultBaseUrl } = require("../scripts/postinstall.js");

test("postinstall defaults to the current package version tag", () => {
  assert.equal(defaultReleaseTag(), `v${packageJson.version}`);
});

test("postinstall default download URL uses the current package version tag", () => {
  assert.equal(
    defaultBaseUrl(),
    `https://github.com/EasyXdc/PortsWhisper-Rust/releases/download/v${packageJson.version}`
  );
});
