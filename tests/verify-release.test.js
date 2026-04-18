const test = require("node:test");
const assert = require("node:assert/strict");

const { createVerifier } = require("../scripts/verify-release.js");

test("createVerifier reports expected asset names", () => {
  const verifier = createVerifier({
    gitTag: "v0.1.0-beta.3",
    packageVersion: "0.1.0-beta.3",
  });

  assert.deepEqual(verifier.expectedAssets(), [
    "ports-rs-darwin-arm64.tar.gz",
    "ports-rs-darwin-x64.tar.gz",
    "ports-rs-linux-x64.tar.gz",
    "ports-rs-windows-x64.zip",
  ]);
});

test("createVerifier reports expected archive entries", () => {
  const verifier = createVerifier({
    gitTag: "v0.1.0-beta.3",
    packageVersion: "0.1.0-beta.3",
  });

  assert.deepEqual(verifier.expectedArchiveEntries("win32-x64"), [
    "ports.exe",
    "whoisonport.exe",
  ]);
  assert.deepEqual(verifier.expectedArchiveEntries("linux-x64"), [
    "ports",
    "whoisonport",
  ]);
});

test("createVerifier exposes resolved npm tag", () => {
  const verifier = createVerifier({
    gitTag: "v0.1.0",
    packageVersion: "0.1.0",
  });

  assert.equal(verifier.metadata.npmTag, "latest");
});

test("createVerifier asset list stays aligned with target definitions", () => {
  const verifier = createVerifier({
    gitTag: "v0.1.0-beta.3",
    packageVersion: "0.1.0-beta.3",
  });

  assert.equal(verifier.expectedAssets().length, 4);
  assert.match(verifier.expectedAssets()[0], /^ports-rs-/);
});
