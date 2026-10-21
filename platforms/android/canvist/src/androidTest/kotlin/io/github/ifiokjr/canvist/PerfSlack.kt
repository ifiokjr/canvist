package io.github.ifiokjr.canvist

import androidx.test.platform.app.InstrumentationRegistry

/**
 * How many times its budget a timing may take here: the `perfSlack`
 * instrumentation argument, which `test.sh` sets from `CANVIST_PERF_SLACK` as
 * the Rust, TypeScript, and Dart budgets read it. Shared CI runners set it;
 * elsewhere it is 1.
 */
internal val perfSlack: Double =
    InstrumentationRegistry.getArguments().getString("perfSlack")?.toDoubleOrNull() ?: 1.0
