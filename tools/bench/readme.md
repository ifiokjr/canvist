# Bench tools

The two required checks that compare a pull request with the commit it merges into: `bench / speed` (this directory's `speed.ts`) and `bench / bundle size` (`size.ts`). `docs/src/guides/performance.md` explains how to read their reports and change a threshold; this readme is about the tools themselves.

Each tool is a `node` script with three commands, run by its workflow (`.github/workflows/bench-speed.yml`, `bench-size.yml`):

- `plan` picks the suites or artifacts a change can affect, so an untouched crate costs nothing,
- `build` compiles them in a checkout — both the pull request's and its base's, sharing one target directory (the base is always checked out after the head's build, or cargo would reuse the head's binaries for it),
- `compare` runs both sides on one runner and fails the check when a benchmark regresses beyond its threshold or an artifact outgrows its budget.

The configuration files, `speed.json` and `size.json`, are validated on load, so a typo in a threshold fails loudly instead of disabling a check.

## Speed's two metrics

`speed.json` lists one suite per bench binary. A suite is wall time by default: its divan binary runs five times per side, alternating base and head, and a time in the report is the median of the runs' divan medians.

A suite with `"metric": "instructions"` counts instead of timing: its gungraun bench binary runs once per side under valgrind's callgrind, and the gate compares exact instruction counts. `canvist_headless`'s `counts` suite, which owns the export benchmarks, is the one — wall time for them measured where the linker happened to place the hot loop, a number that swung by tens of percent between runners on changes that never touched the export path. An instruction count is a property of the revision: the same binary reports the same count on every machine and every run, so the usual rounds, alternation, and confirmation are skipped for it. The trade is that a count cannot see a change that executes the same instructions slower; the absolute budgets in the performance guide still bound wall time, and every other suite still measures it.

Counting suites need valgrind (`sudo apt-get install valgrind`) and gungraun's runner, version-matched to the lockfile (`cargo install gungraun-runner`); the workflow installs both. To run one by hand:

```sh
cargo bench -p canvist_headless --bench counts -- --output-format=json
```

Run any wall-time suite the way CI does, from the repository root:

```sh
node tools/bench/speed.ts build --root . --suites "headless" --out .bench/head
node tools/bench/speed.ts compare --base .bench/base --head .bench/head --suites "headless"
```

## Tests

`node --test 'tools/bench/tests/*.test.ts'` covers the parsers (divan tables, gungraun summaries), the judging rules, and the affected-suite planner. The parser fixtures under `tests/fixtures` are real outputs, trimmed.
