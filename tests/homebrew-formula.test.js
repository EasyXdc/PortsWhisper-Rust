const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");

const {
  releaseUrl,
  renderFormula,
  resolveMacSha256s,
} = require("../scripts/homebrew-formula.js");

test("homebrew formula uses versioned GitHub release URLs for both mac architectures", () => {
  const formula = renderFormula({
    version: "0.2.3",
    darwinArm64Sha256: "a".repeat(64),
    darwinX64Sha256: "b".repeat(64),
  });

  assert.match(formula, /class PortsRs < Formula/);
  assert.match(
    formula,
    /https:\/\/github\.com\/EasyXdc\/PortsWhisper-Rust\/releases\/download\/v0\.2\.3\/ports-rs-darwin-arm64\.tar\.gz/,
  );
  assert.match(
    formula,
    /https:\/\/github\.com\/EasyXdc\/PortsWhisper-Rust\/releases\/download\/v0\.2\.3\/ports-rs-darwin-x64\.tar\.gz/,
  );
  assert.match(formula, /if Hardware::CPU\.arm\?/);
  assert.doesNotMatch(formula, /on_arm do/);
  assert.doesNotMatch(formula, /on_intel do/);
  assert.match(formula, /sha256 "a{64}"/);
  assert.match(formula, /sha256 "b{64}"/);
  assert.match(formula, /bin\.install "ports"/);
  assert.match(formula, /bin\.install "whoisonport"/);
});

test("releaseUrl matches GitHub release asset convention", () => {
  assert.equal(
    releaseUrl("0.2.3", "ports-rs-darwin-arm64.tar.gz"),
    "https://github.com/EasyXdc/PortsWhisper-Rust/releases/download/v0.2.3/ports-rs-darwin-arm64.tar.gz",
  );
});

test("resolveMacSha256s reads mac release archives from dist", () => {
  const dist = fs.mkdtempSync(path.join(os.tmpdir(), "ports-rs-homebrew-"));
  fs.writeFileSync(path.join(dist, "ports-rs-darwin-arm64.tar.gz"), "arm");
  fs.writeFileSync(path.join(dist, "ports-rs-darwin-x64.tar.gz"), "x64");

  assert.deepEqual(resolveMacSha256s(dist), {
    darwinArm64Sha256:
      "ddf7ff5ebd9d66ce161466c1c0262430fa04de32b0e420ee3f489e2e2112e386",
    darwinX64Sha256:
      "5609f728403e197bb255ef50c62aeabb1f93b09f7b7c379903440b65cd4319cb",
  });
});
