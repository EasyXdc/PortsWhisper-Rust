function fail(message) {
  throw new Error(`release-metadata: ${message}`);
}

function resolveReleaseMetadata({ gitTag, packageVersion }) {
  if (!gitTag || !gitTag.startsWith("v")) {
    fail(`expected a version tag starting with v, received '${gitTag}'`);
  }

  const version = gitTag.slice(1);
  if (version !== packageVersion) {
    fail(`tag version '${version}' does not match package.json version '${packageVersion}'`);
  }

  const isBeta = version.includes("-beta.");

  return {
    tag: gitTag,
    version,
    npmTag: isBeta ? "next" : "latest",
    isBeta,
  };
}

module.exports = {
  resolveReleaseMetadata,
};
