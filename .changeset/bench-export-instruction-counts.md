---
"canvist_headless": minor
---

# Export speed, counted instead of timed

The `bench / speed` check's headless export benchmarks now compare callgrind instruction counts instead of wall time. Their wall time on the Linux x86_64 runner measured where the linker placed the export hot loop: because the workspace links with LTO and one codegen unit, any code change could shift that loop to an address where the same instructions run slower, and which side won depended on the runner's CPU. Pull requests that never touched the export path moved the number between −26% and +35%, which is why its threshold had climbed to 40%.

Measured, not guessed: on one runner the divan binaries showed a revision 7% faster while a second build of the same sources showed it 13% slower, and callgrind counted the same instructions (within 0.2%) either way. The new `counts` bench suite, run under valgrind through gungraun (iai-callgrind's new name), reports exact counts that the same binary reproduces on every machine, so the `headless/export/*` threshold is gone and the 10% default applies. Wall time still gates every other suite, and the export budgets in the performance guide still bound wall time absolutely.
