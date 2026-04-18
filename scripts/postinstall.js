#!/usr/bin/env node

const fs = require("node:fs");
const path = require("node:path");
const os = require("node:os");
const https = require("node:https");
const { spawnSync } = require("node:child_process");

const PACKAGE_ROOT = path.resolve(__dirname, "..");
const VENDOR_DIR = path.join(PACKAGE_ROOT, "vendor");
const DEFAULT_TAG = process.env.PORTS_RS_RELEASE_TAG || "v0.1.0-beta.1";
const DEFAULT_BASE_URL =
  process.env.PORTS_RS_BASE_URL ||
  `https://github.com/easyxdc/port-whisperer-rust/releases/download/${DEFAULT_TAG}`;

const PLATFORM_MAP = {
  "darwin-arm64": { asset: "ports-rs-darwin-arm64.tar.gz", archiveType: "tar.gz" },
  "darwin-x64": { asset: "ports-rs-darwin-x64.tar.gz", archiveType: "tar.gz" },
  "linux-x64": { asset: "ports-rs-linux-x64.tar.gz", archiveType: "tar.gz" },
  "win32-x64": { asset: "ports-rs-windows-x64.zip", archiveType: "zip" }
};

function binaryNames() {
  if (process.platform === "win32") {
    return ["ports.exe", "whoisonport.exe"];
  }
  return ["ports", "whoisonport"];
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

function extractTarGz(archivePath) {
  const result = spawnSync("tar", ["-xzf", archivePath, "-C", VENDOR_DIR], { stdio: "inherit" });
  if (result.status !== 0) {
    fail(`failed to extract tar archive ${path.basename(archivePath)}`);
  }
}

function extractZip(archivePath) {
  const result = spawnSync("unzip", ["-o", archivePath, "-d", VENDOR_DIR], { stdio: "inherit" });
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

  const target = PLATFORM_MAP[platformKey()];
  if (!target) {
    fail(`unsupported platform ${platformKey()}`);
  }

  ensureDir(VENDOR_DIR);
  const archivePath = path.join(os.tmpdir(), `${target.asset}`);
  const assetUrl = `${DEFAULT_BASE_URL}/${target.asset}`;

  try {
    await download(assetUrl, archivePath);
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

  console.log(`ports-rs postinstall: installed ${target.asset}`);
}

main().catch((error) => fail(error.message));
