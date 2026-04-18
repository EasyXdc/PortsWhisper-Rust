const RELEASE_TARGETS = {
  "darwin-arm64": {
    key: "darwin-arm64",
    archive: "ports-rs-darwin-arm64.tar.gz",
    archiveType: "tar.gz",
    binaries: ["ports", "whoisonport"],
  },
  "darwin-x64": {
    key: "darwin-x64",
    archive: "ports-rs-darwin-x64.tar.gz",
    archiveType: "tar.gz",
    binaries: ["ports", "whoisonport"],
  },
  "linux-x64": {
    key: "linux-x64",
    archive: "ports-rs-linux-x64.tar.gz",
    archiveType: "tar.gz",
    binaries: ["ports", "whoisonport"],
  },
  "win32-x64": {
    key: "win32-x64",
    archive: "ports-rs-windows-x64.zip",
    archiveType: "zip",
    binaries: ["ports.exe", "whoisonport.exe"],
  },
};

function releaseTargetForPlatform(platform, arch) {
  return RELEASE_TARGETS[`${platform}-${arch}`] || null;
}

function expectedBinaryNames(platform) {
  if (platform === "win32") {
    return ["ports.exe", "whoisonport.exe"];
  }

  return ["ports", "whoisonport"];
}

module.exports = {
  RELEASE_TARGETS,
  releaseTargetForPlatform,
  expectedBinaryNames,
};
