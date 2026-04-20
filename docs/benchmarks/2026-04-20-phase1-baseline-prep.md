# Phase 1 Baseline Prep

- Date: 2026-04-20
- Branch: `feat/benchmark-baseline`
- Commit: `bd20b769a49f4be9abd936aa27e60191a5443b67`
- Rust compiler: `rustc 1.95.0 (59807616e 2026-04-14)`
- Cargo: `cargo 1.95.0 (f2d3ce0bd 2026-03-21)`
- Hyperfine version: `hyperfine 1.19.0`
- OS: `Darwin EasyMacBook-Pro.local 25.4.0 Darwin Kernel Version 25.4.0: Thu Mar 19 19:30:44 PDT 2026; root:xnu-12377.101.15~1/RELEASE_ARM64_T6000 arm64`
- CPU: `Apple M1 Pro`
- Install method: `brew install hyperfine`
- Before installation in this session, `hyperfine` was unavailable on `PATH`.
- `./target/release/ports`: executed successfully.
- `./target/release/ports ps`: executed successfully.
- `./target/release/ports 3000`: executed successfully.
- Port 3000 state during measurement: unoccupied (`lsof -nP -iTCP:3000 -sTCP:LISTEN` returned no listening process).
- On this machine, `./target/release/ports 3000` found no matching process and emitted `warning: system inspection failed`.

## Raw Task 2 Measurements

### Cold Start

- `command time -lp ./target/release/ports`
  - `real 0.12`
  - `user 0.05`
  - `sys 0.13`
- `command time -lp ./target/release/ports ps`
  - `real 0.11`
  - `user 0.03`
  - `sys 0.06`
- `command time -lp ./target/release/ports 3000`
  - output included `No process found on that port.`
  - output included `warning: system inspection failed`
  - `real 0.05`
  - `user 0.01`
  - `sys 0.03`

### Hyperfine

- Methodology: `hyperfine --warmup 3 ...`

- `hyperfine --warmup 3 './target/release/ports'`
  - `Time (mean ± σ):     116.1 ms ±   5.7 ms    [User: 48.8 ms, System: 133.8 ms]`
  - `Range (min … max):   106.4 ms … 137.2 ms    24 runs`
- `hyperfine --warmup 3 './target/release/ports ps'`
  - `Time (mean ± σ):      96.8 ms ±   7.5 ms    [User: 32.1 ms, System: 58.1 ms]`
  - `Range (min … max):    88.8 ms … 120.1 ms    25 runs`
- `hyperfine --warmup 3 './target/release/ports 3000'`
  - `Time (mean ± σ):      54.4 ms ±   3.6 ms    [User: 18.3 ms, System: 33.4 ms]`
  - `Range (min … max):    48.3 ms …  66.1 ms    45 runs`

## Caveat

- A fresh rerun already showed noticeable noise for at least `./target/release/ports`, so these measurements should be treated as a machine-local sample baseline rather than a stable truth.
