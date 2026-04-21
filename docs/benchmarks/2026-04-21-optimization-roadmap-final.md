# Optimization Roadmap Final Benchmark

## Purpose

This document records a fresh release-mode benchmark after completing the roadmap's Phase 3 and Phase 4 work on `feat/optimization-plan`. It exists to show whether the completed functionality and terminal UX changes introduced a measurable regression relative to the Phase 1 baseline.

## Measurement Context

- Date: `2026-04-21`
- Branch: `feat/optimization-plan`
- Commit: `8b680e2`
- Binary under test: `./target/release/ports`
- Tool: `hyperfine 1.19.0`
- Warmup: `3 runs`
- Machine: same local development machine used for prior benchmark capture

## Command

```sh
hyperfine --warmup 3 './target/release/ports' './target/release/ports ps' './target/release/ports 3000'
```

## Current Results

| Command | Mean | Std. Dev. | Min | Max | Runs | User | System |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `./target/release/ports` | 99.8 ms | 2.0 ms | 96.3 ms | 104.1 ms | 27 | 44.5 ms | 98.9 ms |
| `./target/release/ports ps` | 96.2 ms | 1.7 ms | 93.6 ms | 100.2 ms | 29 | 32.5 ms | 61.4 ms |
| `./target/release/ports 3000` | 43.9 ms | 1.9 ms | 42.0 ms | 52.5 ms | 60 | 16.9 ms | 26.7 ms |

## Comparison Against Phase 1 Baseline

Reference baseline: `docs/benchmarks/2026-04-20-phase1-baseline.md`

| Command | Phase 1 Baseline | Current | Delta |
| --- | ---: | ---: | ---: |
| `./target/release/ports` | 116.1 ms | 99.8 ms | -16.3 ms |
| `./target/release/ports ps` | 96.8 ms | 96.2 ms | -0.6 ms |
| `./target/release/ports 3000` | 54.4 ms | 43.9 ms | -10.5 ms |

## Interpretation

- The top-level `ports` command is faster than the Phase 1 baseline by roughly `14.0%`.
- The `ports ps` command is effectively flat relative to baseline, with a small improvement of roughly `0.6%`.
- The single-port lookup path is faster than the Phase 1 baseline by roughly `19.3%`.
- Based on this local sample, the completed Phase 3 and Phase 4 roadmap work did not introduce a measurable steady-state performance regression.

## Notes

- `hyperfine` reported statistical outliers and recommended rerunning on a quieter system. Treat these values as a local benchmark sample rather than a universal truth.
- This run captures steady-state timing only. If a future release decision depends on cold-start behavior, rerun the `command time -lp` sequence from the Phase 1 baseline doc.
- The machine and background load were not strictly controlled beyond the standard local development environment, so small deltas should not be over-interpreted.
