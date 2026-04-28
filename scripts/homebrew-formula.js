#!/usr/bin/env node

const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");

const { RELEASE_TARGETS } = require("./release-targets.js");

const ROOT = path.resolve(__dirname, "..");
const DEFAULT_DIST = path.join(ROOT, "dist");
const DEFAULT_OUTPUT = path.join(DEFAULT_DIST, "ports-rs.rb");

function fail(message) {
  console.error(`homebrew-formula: ${message}`);
  process.exit(1);
}

function sha256File(filePath) {
  const hash = crypto.createHash("sha256");
  hash.update(fs.readFileSync(filePath));
  return hash.digest("hex");
}

function releaseUrl(version, archive) {
  return `https://github.com/EasyXdc/PortsWhisper-Rust/releases/download/v${version}/${archive}`;
}

function resolveMacSha256s(distDir) {
  const armArchive = RELEASE_TARGETS["darwin-arm64"].archive;
  const x64Archive = RELEASE_TARGETS["darwin-x64"].archive;
  const armPath = path.join(distDir, armArchive);
  const x64Path = path.join(distDir, x64Archive);

  if (!fs.existsSync(armPath)) {
    throw new Error(`missing ${armArchive}`);
  }
  if (!fs.existsSync(x64Path)) {
    throw new Error(`missing ${x64Archive}`);
  }

  return {
    darwinArm64Sha256: sha256File(armPath),
    darwinX64Sha256: sha256File(x64Path),
  };
}

function renderFormula({ version, darwinArm64Sha256, darwinX64Sha256 }) {
  const armArchive = RELEASE_TARGETS["darwin-arm64"].archive;
  const x64Archive = RELEASE_TARGETS["darwin-x64"].archive;

  return `class PortsRs < Formula
  desc "Rust-powered CLI for inspecting and managing listening ports"
  homepage "https://github.com/EasyXdc/PortsWhisper-Rust"
  version "${version}"
  license "MIT"

  depends_on :macos

  on_arm do
    url "${releaseUrl(version, armArchive)}"
    sha256 "${darwinArm64Sha256}"
  end

  on_intel do
    url "${releaseUrl(version, x64Archive)}"
    sha256 "${darwinX64Sha256}"
  end

  def install
    bin.install "ports"
    bin.install "whoisonport"
  end

  test do
    assert_match "Usage:", shell_output("#{bin}/ports --help")
    assert_match "Usage:", shell_output("#{bin}/whoisonport --help")
  end
end
`;
}

function main() {
  const distDir = process.env.DIST_DIR || DEFAULT_DIST;
  const output = process.env.HOMEBREW_FORMULA_OUT || DEFAULT_OUTPUT;
  const version = require("../package.json").version;

  let sha256s;
  try {
    sha256s = resolveMacSha256s(distDir);
  } catch (error) {
    fail(error.message);
  }

  fs.mkdirSync(path.dirname(output), { recursive: true });
  fs.writeFileSync(output, renderFormula({ version, ...sha256s }));
  console.log(output);
}

if (require.main === module) {
  main();
}

module.exports = {
  releaseUrl,
  renderFormula,
  resolveMacSha256s,
  sha256File,
};
