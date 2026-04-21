# Phase 1 Benchmark Baseline

## Purpose

This document records the initial performance baseline for the current Rust CLI binary so later optimization work can be compared against a fixed reference point. It captures the commands used, the machine and toolchain context, and the measured results for both one-off invocations and repeated steady-state runs.

## Environment

- Date: `2026-04-20`
- Branch: `feat/benchmark-baseline`
- Commit: `bd20b769a49f4be9abd936aa27e60191a5443b67`
- Binary under test: `./target/release/ports`
- Rust compiler: `rustc 1.95.0 (59807616e 2026-04-14)`
- Cargo: `cargo 1.95.0 (f2d3ce0bd 2026-03-21)`
- Hyperfine version: `hyperfine 1.19.0`
- OS: `Darwin EasyMacBook-Pro.local 25.4.0 Darwin Kernel Version 25.4.0: Thu Mar 19 19:30:44 PDT 2026; root:xnu-12377.101.15~1/RELEASE_ARM64_T6000 arm64`
- CPU: `Apple M1 Pro`
- Hyperfine install method: `brew install hyperfine`

## Commands

Cold-start timing was captured with `command time -lp`:

```sh
command time -lp ./target/release/ports
command time -lp ./target/release/ports ps
command time -lp ./target/release/ports 3000
```

Steady-state timing was captured with `hyperfine --warmup 3`:

```sh
hyperfine --warmup 3 './target/release/ports'
hyperfine --warmup 3 './target/release/ports ps'
hyperfine --warmup 3 './target/release/ports 3000'
```

## Cold-Start Results

| Command | Scenario | Real (s) | User (s) | Sys (s) | Notes |
| --- | --- | ---: | ---: | ---: | --- |
| `./target/release/ports` | top-level invocation | 0.12 | 0.05 | 0.13 | Executed successfully. |
| `./target/release/ports ps` | process-list subcommand | 0.11 | 0.03 | 0.06 | Executed successfully. |
| `./target/release/ports 3000` | single-port lookup | 0.05 | 0.01 | 0.03 | Port 3000 was unoccupied; output included `No process found on that port.` and `warning: system inspection failed`. |

## Steady-State Results

| Command | Mean | Std. Dev. | Min | Max | Runs | User | System |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `./target/release/ports` | 116.1 ms | 5.7 ms | 106.4 ms | 137.2 ms | 24 | 48.8 ms | 133.8 ms |
| `./target/release/ports ps` | 96.8 ms | 7.5 ms | 88.8 ms | 120.1 ms | 25 | 32.1 ms | 58.1 ms |
| `./target/release/ports 3000` | 54.4 ms | 3.6 ms | 48.3 ms | 66.1 ms | 45 | 18.3 ms | 33.4 ms |

## Notes

- `ports 3000` was the fastest measured path in this sample, but that measurement reflects an unoccupied port and should not be generalized to active-port lookups.
- During measurement, `./target/release/ports 3000` emitted `warning: system inspection failed` even though the command completed and reported no process for port 3000. That caveat should be preserved when comparing future runs.
- The top-level command and `ps` subcommand were both near the 100 ms range in steady-state runs, with the `ps` path slightly faster in this sample.
- A fresh rerun already showed noticeable noise for at least `./target/release/ports`, so these results should be treated as a machine-local sample baseline rather than a stable truth.

## Rerun Instructions

1. Ensure the release binary exists: `cargo build --release`.
2. Confirm the comparison target matches this baseline's commit and branch context, or record the new commit if rerunning elsewhere.
3. Record whether port 3000 is occupied before running the port lookup benchmark:
   `lsof -nP -iTCP:3000 -sTCP:LISTEN`
4. Re-run the cold-start commands listed above.
5. Re-run the `hyperfine --warmup 3` commands listed above.
6. Preserve any command output caveats, especially if `ports 3000` still emits `warning: system inspection failed` or if port 3000 is occupied in the rerun environment.
