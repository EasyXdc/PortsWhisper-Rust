# Clap Decision for Phase 2

## Context

Phase 2 needs a single decision for the CLI parameter layer before implementing filters, range queries, and shell completion. The roadmap explicitly requires that choice to be made before the phase begins, and it names two routes: adopt `clap` or keep a custom parser and build a parameter registry around it.

This note uses only the currently checked-in evidence across four categories: current parser code, dependency baseline, benchmark baseline, and roadmap requirements.

- `src/cli.rs`
- `docs/benchmarks/2026-04-20-phase1-baseline.md`
- `docs/benchmarks/2026-04-20-phase1-baseline-prep.md`
- `Cargo.toml`
- `docs/optimization-roadmap.md`

Because the benchmark data is machine-local and the repo does not yet contain a measured `clap` prototype, this decision is based on a mix of direct code evidence and bounded risk judgment rather than a full A/B benchmark.

## Evidence Summary

### Current custom parser already shows coupling and drift

`src/cli.rs` currently parses flags by scanning raw strings, dropping recognized global flags, and then matching the remaining positional tokens manually in `parse_args`. That works for the current small surface area, but it already has command-specific leakage.

The strongest direct example is `Some("logs") => CliCommand::Logs(filtered_args)`, which passes the subcommand token itself into the `Logs` payload instead of only the subcommand arguments. The roadmap calls this out explicitly as issue `S6`: `CliCommand::Logs(filtered_args)` includes `"logs"`, and `logs.rs` compensates with `.skip(1)` rather than receiving a clean argument shape from the parser layer.

That matters for Phase 2 because the roadmap adds more parse-heavy work immediately after this decision:

- `--framework`
- `--pid`
- `--project`
- `--port-range`
- `ports 3000-3010`
- `ports completion bash|zsh|fish`

The current parser is adequate for a handful of booleans plus a few subcommands, but the evidence in `src/cli.rs` and roadmap item `S6` shows it is already encoding command quirks in ad hoc ways before those new combinations arrive.

### The current dependency baseline is intentionally small

`Cargo.toml` currently lists only five runtime dependencies: `serde`, `serde_json`, `ctrlc`, `unicode-width`, and `wait-timeout`. The roadmap also describes the project snapshot as having "almost zero runtime dependencies". That is a real reason to hesitate before adding `clap`, because `clap` would be a visible shift in dependency footprint for a small CLI.

### The available benchmark baseline is useful, but incomplete for a direct `clap` cost comparison

The benchmark documents provide a measured Phase 1 baseline for the current binary:

- Release build time: `7.12s`
- Release binary size: `1,101,872` bytes
- Steady-state `./target/release/ports`: `116.1 ms ± 5.7 ms`
- Steady-state `./target/release/ports ps`: `96.8 ms ± 7.5 ms`
- Steady-state `./target/release/ports 3000`: `54.4 ms ± 3.6 ms`

Those numbers are useful because Phase 2 must preserve awareness of binary-size and compile-time cost. But both benchmark docs also state that the results are machine-local samples with noticeable noise. More importantly, they measure only the current hand-written parser. There is no checked-in `clap` branch or measured prototype here, so the existing evidence cannot prove the actual cost of adopting `clap` in this repo.

### The roadmap favors stable CLI contracts over preserving the current parser shape

The roadmap makes the Phase 2 objective clear: "first put the CLI contract in order, to build a stable parameter layer for filters, completion, and later expansion." It also names `clap` directly in `S6` as the recommended direction for the current manual parsing problem, while still leaving room for a custom parameter registry if the cost is not justified.

That framing matters: the main Phase 2 risk is not that the current parser fails today's commands, but that the CLI contract becomes harder to extend correctly just as the project starts adding more combinatorial options.

## Option Comparison

### Option A: Adopt `clap`

Pros supported by current evidence:

- Directly addresses the roadmap's identified `S6` problem: manual string slicing in `src/cli.rs`.
- Gives Phase 2 a cleaner base for filters, ranged queries, and completion, all of which are specifically in scope.
- Reduces the need for command-specific cleanup like the current `logs` argument workaround.
- Makes the parser contract more declarative at the same moment the CLI surface is expanding.

Cons supported by current evidence:

- Adds a meaningful new dependency to a project that currently keeps dependencies sparse.
- Likely increases compile time and binary size, but the current repo evidence does not quantify that increase yet.
- Requires careful backward-compatibility verification because the roadmap requires current CLI syntax to continue working.

### Option B: Keep a custom parser and build a parameter registry

Pros supported by current evidence:

- Preserves the current low-dependency profile from `Cargo.toml`.
- Avoids taking an unmeasured dependency-size and build-time hit.
- Could be kept narrowly tailored to the existing CLI style.

Cons supported by current evidence:

- Starts Phase 2 by investing more design work into a parser layer that is already showing command leakage in `src/cli.rs`.
- Requires separate work for completion generation, which the roadmap explicitly notes as extra work on the custom path.
- Increases the chance that each new Phase 2 flag and command combination needs more one-off parsing rules.
- Keeps the project responsible for maintaining edge cases that a mature CLI parser already handles.

## Decision

Choose `clap` for the Phase 2 CLI parameter layer, and do not take the custom-parser registry route for Phase 2.

This is the better tradeoff given the evidence currently in the repo. The strongest direct evidence is not a benchmark but the state of `src/cli.rs`: the parser is already leaking command-specific behavior before Phase 2's additional filters and completion work have even landed. The roadmap's Phase 2 scope is exactly the kind of expansion that increases the maintenance cost of a custom parser faster than it increases the value of keeping one.

The cost side is real and should not be minimized. The current build baseline is `7.12s`, the current release binary is `1,101,872` bytes, and the project deliberately keeps dependencies light. But the benchmark docs are explicit that they are machine-local samples, and they do not include a measured `clap` prototype. So the honest conclusion is not "`clap` is free"; it is that, based on the code and roadmap evidence available today, the parser-structure benefits are strong enough to justify adopting `clap` and then measuring the cost during implementation.

## Immediate Next Step

Implement a minimal `clap` migration that preserves the current CLI contract before adding any new Phase 2 filters.

Concretely, the first implementation step is:

1. Add `clap` using the derive API.
2. Model only the existing commands and global flags from `src/cli.rs`: top-level list behavior, `help`, `ps`, `clean`, `kill`, `logs`, `watch`, port detail, `--all`, `--verbose`, and `--json`.
3. Verify backward compatibility for the current syntax before introducing `--framework`, `--pid`, `--project`, `--port-range`, range queries, or completion generation.
4. Record the new build-time and binary-size numbers against the Phase 1 baseline so the cost of the choice is documented rather than assumed.

That ordering keeps the next step small, makes the decision testable, and respects the roadmap requirement that any `clap` cost be measured against the Phase 1 baseline.

## What This Decision Does Not Claim

- It does not claim that `clap` is already measured to be acceptable in this repo.
- It does not claim the benchmark sample is broadly representative beyond the machine where it was collected.
- It does not claim a custom parser could not be made to work.

It claims only that, given the checked-in evidence today, `clap` is the clearer Phase 2 choice.
