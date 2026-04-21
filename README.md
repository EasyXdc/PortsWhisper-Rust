# Port Whisperer

<p align="center">
  <img src="./assets/hero.svg" alt="Port Whisperer Hero" width="100%" />
</p>

<p align="center">
  <strong>🔎 Discover which process owns a port, inspect runtime details, tail logs, and watch local port activity from one CLI.</strong>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/CLI-Rust-0f172a?style=for-the-badge&logo=rust" alt="Rust CLI" />
  <img src="https://img.shields.io/badge/npm-ports--rs-CB3837?style=for-the-badge&logo=npm" alt="npm package" />
  <img src="https://img.shields.io/badge/platform-macOS%20%7C%20Linux%20%7C%20Windows-2563eb?style=for-the-badge" alt="Platform support" />
</p>

<p align="center">
  <a href="./README.zh-CN.md">🇨🇳 中文文档: README.zh-CN.md</a>
</p>

## ✨ What It Does

Port Whisperer helps you answer questions like:

- Which process is listening on this port?
- Is this a dev server, database, Docker container, or orphaned process?
- Which project folder and framework does it belong to?
- Can I inspect logs or kill it right away?

Typical workflows look like this:

- `ports` when your dev environment feels "busy" and you want a quick overview
- `ports 3000` when a frontend or backend refuses to start and you need the owner immediately
- `ports check 3000 5173 8080` before starting multiple local services
- `ports logs 3000 --grep error` when you want signal from noisy logs
- `ports open 3000` when you know the service is up and just want the browser tab now
- `ports kill --signal SIGINT 3000` when you want a graceful stop instead of a force kill

It provides two commands:

- `ports`
- `whoisonport`

`whoisonport <port>` is simply a friendly alias for `ports <port>`.

## 🚀 Installation

### npm

```bash
npm i -g ports-rs
```

That exposes:

```bash
ports
whoisonport
```

If you want to test a future prerelease build later, use the `next` dist-tag explicitly.

### Cargo

If you already use Rust and want to install from source:

```bash
git clone https://github.com/LarsenCundric/port-whisperer-rust.git
cd port-whisperer-rust
cargo install --path .
```

### GitHub Releases

You can also download prebuilt binaries directly.

Current asset naming:

- `ports-rs-darwin-arm64.tar.gz`
- `ports-rs-darwin-x64.tar.gz`
- `ports-rs-linux-x64.tar.gz`
- `ports-rs-windows-x64.zip`

Each archive contains:

- `ports`
- `whoisonport`

Windows archives contain:

- `ports.exe`
- `whoisonport.exe`

## ⚡ Quick Start

### Show development ports

```bash
ports
```

By default, Port Whisperer focuses on development-related processes such as Node.js, Python, Java, Rust, Docker, frontend dev servers, and common local services.

### Show all listening ports

```bash
ports --all
ports -a
```

### Inspect a specific port

```bash
ports 3000
whoisonport 3000
```

Detail view includes:

- port
- process name
- PID
- health status
- framework
- memory usage
- uptime
- start time
- working directory
- project name
- git branch
- process tree

If a listener exists, the detail view can prompt to terminate it:

```text
Kill process on :3000? [y/N]
```

Only `y` or `Y` confirms the kill.

### Check whether a port is free before you start something

```bash
ports check 3000
ports check 3000 5173 8080
ports --json check 3000 5173 8080
```

This is useful before starting a frontend, backend, proxy, or local database.

Exit codes:

- `0` when all requested ports are available
- `1` when any requested port is already occupied

### Open a local service in the browser

```bash
ports open 3000
```

This opens `http://localhost:3000` in your default browser.

## 🧰 Commands

### Process list

```bash
ports ps
ports ps --all
ports ps -a
```

`ports ps` shows a developer-focused process table with:

- PID
- process name
- CPU%
- memory
- project
- framework
- uptime
- summarized command

### Kill listeners or processes

```bash
ports kill 3000
ports kill 3000 5173 8080
ports kill 3000-3010
ports kill 42872
ports kill --force 3000
ports kill --signal SIGINT 3000
```

Rules:

- numbers in `1..=65535` are interpreted as ports first
- if no listener is found, they fall back to PID resolution
- values above `65535` are treated as PID only
- ranges expand into multiple ports
- empty ports inside a range are summarized, not treated as hard errors
- `--signal <name>` lets you request a specific signal such as `SIGINT`, `SIGTERM`, or `SIGKILL`
- `--force` still means `SIGKILL`

### Logs

```bash
ports logs 3000
ports logs 3000 -f
ports logs 3000 --follow
ports logs 3000 --lines 10
ports logs 3000 --lines=10
ports logs 3000 --err
ports logs 3000 --grep error
ports logs 3000 --since 10m
ports logs 3000 -f --grep error
```

Port Whisperer tries to discover:

- redirected stdout/stderr files
- `.log` / `logs/` / `nohup.out` style paths
- system log fallbacks on macOS, Linux, and Windows

Use cases:

- `--grep <pattern>` keeps only matching lines
- `--since <value>` narrows system-log fallback queries such as `journalctl` or macOS unified logs
- `--err` prefers stderr when it is redirected separately

### Clean orphaned or zombie processes

```bash
ports clean
```

### Watch port changes

```bash
ports watch
```

Press `Ctrl+C` to stop.

## 🧭 Global Options

These flags work across the CLI where supported:

- `--json`: emit structured JSON for scriptable commands such as `ports`, `ports ps`, `ports 3000`, and `ports check`
- `--quiet`: reduce decorative output for cleaner terminal or redirected use
- `--ascii`: force ASCII-safe symbols for limited terminals
- `--all` / `-a`: show all listeners or processes instead of the development-focused filtered view

Examples:

```bash
ports --json
ports ps --json --all
ports --quiet
ports --ascii
TERM=dumb ports
```

## 🧱 Tech Stack

| Layer | Tech |
| --- | --- |
| Core CLI | Rust |
| Packaging | npm + Node.js install scripts |
| macOS process discovery | `lsof` + `ps` + `log` |
| Linux process discovery | `/proc` + `ss` / `netstat` + `ps` + `journalctl` |
| Windows process discovery | PowerShell + `Get-NetTCPConnection` / `Get-Process` + `taskkill` |

## 📊 Performance

Fresh release-mode local measurements (`hyperfine --warmup 3`):

| Command | Mean |
| --- | ---: |
| `./target/release/ports` | `99.8 ms` |
| `./target/release/ports ps` | `96.2 ms` |
| `./target/release/ports 3000` | `43.9 ms` |

Compared with the Phase 1 baseline on the same development machine:

- `ports` improved from `116.1 ms` to `99.8 ms`
- `ports ps` stayed effectively flat (`96.8 ms` -> `96.2 ms`)
- `ports 3000` improved from `54.4 ms` to `43.9 ms`

These figures are local samples, not a universal guarantee. Real timings vary by OS, hardware, Docker state, and background load.

## 🖥️ Platform Support

Current target release platforms:

- macOS arm64
- macOS x64
- Linux x64
- Windows x64

## 🔗 Reference

This Rust rewrite is based on the original project:

- [LarsenCundric/port-whisperer](https://github.com/LarsenCundric/port-whisperer)

The goal of this repository is to preserve the core CLI workflow and user-facing behavior while rebuilding the implementation in Rust.

## ❓ FAQ

### What is `whoisonport`?

It is just an alias for:

```bash
ports <port>
```

### npm install failed. What now?

You can:

1. download the matching binary from GitHub Releases manually
2. install directly from source:

```bash
cargo install --path .
```

### Why didn't `ports logs` show the same output I see in another terminal?

If a process is only writing to a live terminal session and not to a redirected file or system log, Port Whisperer may not be able to recover that output afterward. In that case:

- rerun the process with stdout/stderr redirected to a file
- or use a framework/service logger that writes to discoverable logs

### When should I use `ports check` instead of `ports 3000`?

Use:

- `ports 3000` when you want details about the owner of a port
- `ports check 3000` when you only care whether the port is free before starting something

### Why are some fields empty?

Directory, project, framework, and git branch information depend on system visibility, permissions, cwd discovery, and project-root detection. Some system or sandboxed processes cannot expose all metadata.

### Why doesn't the default view show every port?

The default `ports` command is intentionally filtered for development-related processes. Use:

```bash
ports --all
```

to see everything.

## 🔧 Development

```bash
cargo build
cargo test
```

Run locally:

```bash
cargo run --bin ports
cargo run --bin whoisonport -- 3000
```

## 📄 License

[MIT](LICENSE)
