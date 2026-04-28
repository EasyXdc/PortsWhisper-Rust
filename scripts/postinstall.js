#!/usr/bin/env node

const fs = require("node:fs");
const path = require("node:path");
const os = require("node:os");
const https = require("node:https");
const crypto = require("node:crypto");
const { spawnSync } = require("node:child_process");
const {
  releaseTargetForPlatform,
  expectedBinaryNames,
} = require("./release-targets.js");

const PACKAGE_ROOT = path.resolve(__dirname, "..");
const VENDOR_DIR = path.join(PACKAGE_ROOT, "vendor");

function packageVersion() {
  return require(path.join(PACKAGE_ROOT, "package.json")).version;
}

function defaultReleaseTag() {
  return process.env.PORTS_RS_RELEASE_TAG || `v${packageVersion()}`;
}

function defaultBaseUrl() {
  return (
    process.env.PORTS_RS_BASE_URL ||
    `https://github.com/EasyXdc/PortsWhisper-Rust/releases/download/${defaultReleaseTag()}`
  );
}

function binaryNames() {
  return expectedBinaryNames(process.platform);
}

function ensureDir(dir) {
  fs.mkdirSync(dir, { recursive: true });
}

function fail(message) {
  console.error(`ports-rs install failed: ${message}`);
  console.error("Fallback options:");
  console.error("1. Download the matching binary from GitHub Releases manually.");
  console.error("2. Run `cargo install --path .` from the repository.");
  process.exit(1);
}

function platformKey() {
  return `${process.platform}-${process.arch}`;
}

function platformTarget() {
  return releaseTargetForPlatform(process.platform, process.arch);
}

function installSkipped() {
  ensureDir(VENDOR_DIR);
  console.log("ports-rs postinstall: download skipped via PORTS_RS_SKIP_DOWNLOAD=1");
}

function download(url, destination) {
  return new Promise((resolve, reject) => {
    const file = fs.createWriteStream(destination);
    https
      .get(url, (response) => {
        if (response.statusCode >= 300 && response.statusCode < 400 && response.headers.location) {
          file.close();
          fs.unlinkSync(destination);
          download(response.headers.location, destination).then(resolve).catch(reject);
          return;
        }
        if (response.statusCode !== 200) {
          file.close();
          fs.unlinkSync(destination);
          reject(new Error(`download returned HTTP ${response.statusCode}`));
          return;
        }
        response.pipe(file);
        file.on("finish", () => file.close(resolve));
      })
      .on("error", (error) => {
        file.close();
        if (fs.existsSync(destination)) fs.unlinkSync(destination);
        reject(error);
      });
  });
}

function expectedSha256FromSums(contents, archiveName) {
  for (const line of contents.split(/\r?\n/)) {
    const match = line.match(/^([a-fA-F0-9]{64})\s+\*?(.+)$/);
    if (match && path.basename(match[2].trim()) === archiveName) {
      return match[1].toLowerCase();
    }
  }
  throw new Error(`SHA256SUMS did not contain ${archiveName}`);
}

function verifyArchiveChecksum(archivePath, expectedSha256) {
  const actualSha256 = crypto.createHash("sha256").update(fs.readFileSync(archivePath)).digest("hex");
  if (actualSha256 !== expectedSha256.toLowerCase()) {
    throw new Error(`checksum mismatch for ${path.basename(archivePath)}`);
  }
}

function extractTarGz(archivePath) {
  const result = spawnSync("tar", ["-xzf", archivePath, "-C", VENDOR_DIR], { stdio: "inherit" });
  if (result.status !== 0) {
    fail(`failed to extract tar archive ${path.basename(archivePath)}`);
  }
}

function buildZipExtractCommand(archivePath, destinationDir) {
  const normalizedArchivePath = archivePath.replace(/\\/g, "/").replace(/'/g, "''");
  const normalizedDestinationDir = destinationDir.replace(/\\/g, "/").replace(/'/g, "''");
  return {
    command: "powershell",
    args: [
      "-NoProfile",
      "-Command",
      `Expand-Archive -Path '${normalizedArchivePath}' -DestinationPath '${normalizedDestinationDir}' -Force`,
    ],
  };
}

function extractZip(archivePath) {
  const command = buildZipExtractCommand(archivePath, VENDOR_DIR);
  const result = spawnSync(command.command, command.args, { stdio: "inherit" });
  if (result.status !== 0) {
    fail(`failed to extract zip archive ${path.basename(archivePath)}`);
  }
}

function ensureExecutablePermissions() {
  if (process.platform === "win32") return;
  for (const file of binaryNames()) {
    const fullPath = path.join(VENDOR_DIR, file);
    if (fs.existsSync(fullPath)) {
      fs.chmodSync(fullPath, 0o755);
    }
  }
}

async function main() {
  if (process.env.PORTS_RS_SKIP_DOWNLOAD === "1") {
    installSkipped();
    return;
  }

  const target = platformTarget();
  if (!target) {
    fail(`unsupported platform ${platformKey()}`);
  }

  ensureDir(VENDOR_DIR);
  const archivePath = path.join(os.tmpdir(), target.archive);
  const checksumsPath = path.join(os.tmpdir(), `ports-rs-${defaultReleaseTag()}-SHA256SUMS`);
  const assetUrl = `${defaultBaseUrl()}/${target.archive}`;
  const checksumsUrl = `${defaultBaseUrl()}/SHA256SUMS`;

  try {
    await download(assetUrl, archivePath);
    await download(checksumsUrl, checksumsPath);
    const expectedSha256 = expectedSha256FromSums(fs.readFileSync(checksumsPath, "utf8"), target.archive);
    verifyArchiveChecksum(archivePath, expectedSha256);
  } catch (error) {
    fail(error.message);
  }

  if (target.archiveType === "tar.gz") {
    extractTarGz(archivePath);
  } else {
    extractZip(archivePath);
  }

  ensureExecutablePermissions();

  for (const file of binaryNames()) {
    if (!fs.existsSync(path.join(VENDOR_DIR, file))) {
      fail(`archive did not contain expected binary ${file}`);
    }
  }

  console.log(`ports-rs postinstall: installed ${target.archive}`);
}

if (require.main === module) {
  main().catch((error) => fail(error.message));
}

module.exports = {
  buildZipExtractCommand,
  defaultBaseUrl,
  defaultReleaseTag,
  expectedSha256FromSums,
  packageVersion,
  verifyArchiveChecksum,
};
