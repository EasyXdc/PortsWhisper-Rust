# Phase 2 Clap Build Footprint

## Purpose

This document records the post-`clap` release build time and release binary size for Phase 2, using the checked-in Phase 1 build baseline prep note as the comparison source.

## Comparison Source

- Baseline document: `docs/benchmarks/2026-04-20-phase1-baseline-prep.md`
- Phase 1 branch: `feat/benchmark-baseline`
- Phase 1 commit: `bd20b769a49f4be9abd936aa27e60191a5443b67`
- Phase 1 release build time: `7.12s`
- Phase 1 release binary size: `1,101,872` bytes

## Phase 2 Measurement Provenance

- Date: `2026-04-20`
- Branch: `feat/phase2-clap-impl`
- Measured state: current `feat/phase2-clap-impl` worktree snapshot with uncommitted Phase 2 implementation changes
- Binary under test: `./target/release/ports`
- Rust compiler: `rustc 1.95.0 (59807616e 2026-04-14)`
- Cargo: `cargo 1.95.0 (f2d3ce0bd 2026-03-21)`
- OS: `Darwin EasyMacBook-Pro.local 25.4.0 Darwin Kernel Version 25.4.0: Thu Mar 19 19:30:44 PDT 2026; root:xnu-12377.101.15~1/RELEASE_ARM64_T6000 arm64`

## Commands

Release build time was taken from a fresh `cargo build --release` run:

```sh
cargo build --release
```

Release binary size was measured with:

```sh
stat -f '%z' ./target/release/ports
```

## Results

| Metric | Phase 1 Baseline | Phase 2 Post-clap | Delta |
| --- | ---: | ---: | ---: |
| Release build time | 7.12s | 10.90s | +3.78s |
| Release binary size | 1,101,872 bytes | 1,773,664 bytes | +671,792 bytes |

## Relative Change

| Metric | Change vs. Phase 1 |
| --- | ---: |
| Release build time | +53.1% |
| Release binary size | +61.0% |

## Notes

- These Phase 2 values were measured from the current `feat/phase2-clap-impl` worktree snapshot rather than a committed revision identifier.
- This note records only the measured build-time and binary-size differences against the checked-in Phase 1 build baseline prep note.
