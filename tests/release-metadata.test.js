const test = require("node:test");
const assert = require("node:assert/strict");

const { resolveReleaseMetadata } = require("../scripts/release-metadata.js");

test("beta tags resolve to next", () => {
  assert.deepEqual(
    resolveReleaseMetadata({
      gitTag: "v0.1.0-beta.3",
      packageVersion: "0.1.0-beta.3",
    }),
    {
      tag: "v0.1.0-beta.3",
      version: "0.1.0-beta.3",
      npmTag: "next",
      isBeta: true,
    }
  );
});

test("stable tags resolve to latest", () => {
  assert.deepEqual(
    resolveReleaseMetadata({
      gitTag: "v0.1.0",
      packageVersion: "0.1.0",
    }),
    {
      tag: "v0.1.0",
      version: "0.1.0",
      npmTag: "latest",
      isBeta: false,
    }
  );
});

test("version mismatch throws", () => {
  assert.throws(
    () =>
      resolveReleaseMetadata({
        gitTag: "v0.1.0-beta.3",
        packageVersion: "0.1.0-beta.2",
      }),
    /does not match/
  );
});
