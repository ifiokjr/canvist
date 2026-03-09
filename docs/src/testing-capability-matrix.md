# Testing Capability Matrix

This matrix tracks where core capabilities are tested today, what is only documented as intent, and what remains TODO.

## Legend

- ✅ Covered: capability is validated by executable tests in CI.
- ⚠️ Partial: capability has some executable coverage, but not full matrix/platform parity.
- 📝 Intent only: capability is described in docs/helpers but not currently executed by tests.
- ❌ Missing: no known automated coverage yet.

## Browser / Playwright Capability Matrix

| Capability                                                    | `crates/canvist_test` docs/helpers intent                                                                                             | `crates/canvist_test` current executable tests                                                                                  | Active CI-backed Playwright coverage (`packages/canvist`)                                                                            | Status         |
| ------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ | -------------- |
| Browser automation harness exists                             | `crates/canvist_test/readme.md` and `crates/canvist_test/src/lib.rs` describe Playwright-driven browser automation goals and helpers. | `crates/canvist_test/tests/smoke.rs` does not launch browsers; it is smoke-only and explicitly notes browser tests are pending. | `.github/workflows/playwright.yml` runs `deno task test:playwright` in `packages/canvist` using Playwright.                          | ⚠️ Partial      |
| Cross-browser runtime checks (Chromium, Firefox, WebKit)      | Crate docs imply browser execution intent but do not define an executed browser matrix in crate tests.                                | No crate-level Playwright browser matrix is executed today.                                                                     | Workflow executes Linux jobs for Chromium + Firefox and macOS jobs for WebKit via Playwright test runs in `packages/canvist`.        | ✅ Covered     |
| CI execution of browser tests                                 | Crate docs discuss local/test helper flows but do not provide crate CI browser test jobs.                                             | Crate smoke tests run as non-browser checks only.                                                                               | Dedicated workflow `.github/workflows/playwright.yml` provides CI-backed browser execution and dependency setup for Playwright runs. | ✅ Covered     |
| Wasm build + serve + browser verification flow at crate level | Documented as intended end-to-end shape in crate docs/helpers.                                                                        | Not implemented as executable browser automation in crate tests; smoke tests are currently non-browser.                         | Equivalent browser verification is actively exercised via Deno Playwright tests under `packages/canvist` in CI.                      | ⚠️ Partial      |
| Crate-native Playwright browser assertions                    | Mentioned directionally in crate docs/helper descriptions.                                                                            | No crate-native Playwright assertions currently execute.                                                                        | Browser assertions currently live in `packages/canvist` Playwright tests, not in `crates/canvist_test`.                              | 📝 Intent only |

## Source Pointers

- Crate intent/docs:
  - `crates/canvist_test/readme.md`
  - `crates/canvist_test/src/lib.rs`
- Crate current smoke reality:
  - `crates/canvist_test/tests/smoke.rs`
- Active CI-backed browser coverage:
  - `.github/workflows/playwright.yml`
  - `packages/canvist` Playwright tests executed via `deno task test:playwright`

## Backfill Notes

- The repository currently achieves practical browser Playwright coverage through `packages/canvist` + CI workflow execution.
- `crates/canvist_test` should be treated as **intent/helper-oriented** for browser automation until crate-native Playwright tests are implemented.
- Keep this matrix updated when crate-level browser tests are added or when CI browser matrix changes.
