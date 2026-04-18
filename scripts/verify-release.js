#!/usr/bin/env node

const fs = require("node:fs");
const path = require("node:path");
const { execFileSync } = require("node:child_process");

const { RELEASE_TARGETS } = require("./release-targets.js");
const { resolveReleaseMetadata } = require("./release-metadata.js");

function createVerifier({
  gitTag,
  packageVersion,
  distDir = path.join(__dirname, "..", "dist"),
}) {
  const metadata = resolveReleaseMetadata({ gitTag, packageVersion });

  return {
    metadata,
    expectedAssets() {
      return Object.values(RELEASE_TARGETS).map((target) => target.archive);
    },
    expectedArchiveEntries(targetKey) {
      const target = RELEASE_TARGETS[targetKey];
      if (!target) {
        throw new Error(`verify-release: unsupported target '${targetKey}'`);
      }

      return [...target.binaries];
    },
    verifyDistArtifacts() {
      for (const target of Object.values(RELEASE_TARGETS)) {
        const archivePath = path.join(distDir, target.archive);
        if (!fs.existsSync(archivePath)) {
          throw new Error(`verify-release: missing asset ${target.archive}`);
        }

        const entries = listArchiveEntries(target, archivePath);
        for (const binary of target.binaries) {
          if (!entries.includes(binary)) {
            throw new Error(`verify-release: ${target.archive} missing ${binary}`);
          }
        }
      }
    },
  };
}

function listArchiveEntries(target, archivePath) {
  if (target.archiveType === "tar.gz") {
    return execFileSync("tar", ["-tzf", archivePath], { encoding: "utf8" })
      .split(/\r?\n/)
      .filter(Boolean)
      .map((entry) => path.basename(entry));
  }

  return execFileSync("unzip", ["-Z1", archivePath], { encoding: "utf8" })
    .split(/\r?\n/)
    .filter(Boolean)
    .map((entry) => path.basename(entry));
}

function main() {
  const gitTag = process.env.GITHUB_REF_NAME;
  const packageVersion = require("../package.json").version;
  const verifier = createVerifier({ gitTag, packageVersion });

  console.log(`release version: ${verifier.metadata.version}`);
  console.log(`npm dist-tag: ${verifier.metadata.npmTag}`);

  if (process.env.VERIFY_RELEASE_DIST === "1") {
    verifier.verifyDistArtifacts();
    console.log("release assets verified");
  }
}

if (require.main === module) {
  try {
    main();
  } catch (error) {
    console.error(error.message);
    process.exit(1);
  }
}

module.exports = {
  createVerifier,
  listArchiveEntries,
};
