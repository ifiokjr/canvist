{
  pkgs,
  lib,
  config,
  inputs,
  ...
}:

let
  extra = inputs.ifiokjr-nixpkgs.packages.${pkgs.stdenv.system};
in

{
  packages =
    with pkgs;
    [
      cargo-binstall
      cargo-run-bin
      deno
      dprint
      extra.monochange
      extra.monostyle
      mdbook
      nixfmt
      rustup
      shfmt
      wasm-pack
    ]
    ++ lib.optionals stdenv.isDarwin [
      coreutils
      xcodegen
    ];

  enterShell = ''
    set -e
    # Ensure the nightly toolchain is available for rustfmt (used by dprint)
    rustup toolchain install nightly --component rustfmt --no-self-update 2>/dev/null || true
    # Ensure stable is at least 1.86 (required for edition 2024)
    rustup update stable --no-self-update 2>/dev/null || true
  '';

  # disable dotenv since it breaks the variable interpolation supported by `direnv`
  dotenv.disableHint = true;

  scripts = {
    "install:all" = {
      exec = ''
        set -e
        install:cargo:bin
      '';
      description = "Install all packages.";
      binary = "bash";
    };
    "install:cargo:bin" = {
      exec = ''
        set -e
        cargo bin --install
      '';
      description = "Install cargo binaries locally.";
      binary = "bash";
    };
    "update:deps" = {
      exec = ''
        set -e
        cargo update
        devenv update
      '';
      description = "Update dependencies.";
      binary = "bash";
    };
    "build:all" = {
      exec = ''
        set -e
        if [ -z "$CI" ]; then
          echo "Building project locally"
          cargo build --all-features
        else
          echo "Building in CI"
          cargo build --all-features --locked
        fi
      '';
      description = "Build all crates with all features activated.";
      binary = "bash";
    };
    "build:book" = {
      exec = ''
        set -e
        mdbook build docs
      '';
      description = "Build the mdbook documentation.";
      binary = "bash";
    };
    "build:wasm" = {
      exec = ''
        set -e
        wasm-pack build crates/canvist_wasm --target web
      '';
      description = "Build the WASM package for the web.";
      binary = "bash";
    };
    "build:ios" = {
      exec = ''
        set -e
        "$DEVENV_ROOT/platforms/apple/build.sh"
      '';
      description = "Build CanvistFFI.xcframework and the Swift bindings (macOS only).";
      binary = "bash";
    };
    "build:android" = {
      exec = ''
        set -e
        "$DEVENV_ROOT/platforms/android/build.sh"
      '';
      description = "Build libcanvist_ffi.so with cargo-ndk and the Kotlin bindings.";
      binary = "bash";
    };
    "test:ios" = {
      exec = ''
        set -e
        "$DEVENV_ROOT/platforms/apple/test.sh"
      '';
      description = "Run the Swift package and sample app tests on an iOS Simulator (macOS only).";
      binary = "bash";
    };
    "test:android" = {
      exec = ''
        set -e
        "$DEVENV_ROOT/platforms/android/test.sh"
      '';
      description = "Run the Android library and sample app instrumented tests on an emulator.";
      binary = "bash";
    };
    "test:flutter" = {
      exec = ''
        set -e
        "$DEVENV_ROOT/platforms/flutter/test.sh"
      '';
      description = "Run the Flutter integration tests on an iOS Simulator and an Android emulator.";
      binary = "bash";
    };
    "test:dart" = {
      exec = ''
        set -e
        cd "$DEVENV_ROOT/platforms/dart"
        # The Dart SDK alone is enough; fvm's comes with Flutter when present.
        dart=(dart)
        command -v dart >/dev/null || dart=(fvm dart)
        "''${dart[@]}" pub get
        "''${dart[@]}" analyze --fatal-infos
        "''${dart[@]}" run tool/generate.dart --check
        "''${dart[@]}" test
      '';
      description = "Run the pure Dart package's conformance, API, and server tests (no Flutter).";
      binary = "bash";
    };
    "test:headless-js" = {
      exec = ''
        set -e
        cd "$DEVENV_ROOT/packages/canvist-headless"
        npm ci
        npm run build:wasm
        npm run check:generated
        npm run check:types
        npm test
        deno task test
      '';
      description = "Build @canvist/headless and run its tests in Node and Deno.";
      binary = "bash";
    };
    "test:web" = {
      exec = ''
        set -e
        cd "$DEVENV_ROOT/packages/canvist-headless"
        npm ci
        npm run build:wasm
        npm run build
        cd "$DEVENV_ROOT/packages/canvist-web"
        npm ci
        npm run build:wasm
        npx tsc --noEmit
        npm test
        npx playwright test
        node scripts/parity-report.ts
      '';
      description = "Build @canvist/web, type-check it, and run its unit and pixel parity tests (CANVIST_BROWSERS narrows the browsers).";
      binary = "bash";
    };
    "test:all" = {
      exec = ''
        set -e
        test:cargo
        test:docs
      '';
      description = "Run all tests across the crates.";
      binary = "bash";
    };
    "test:cargo" = {
      exec = ''
        set -e
        cargo nextest run --workspace --exclude canvist_wasm --exclude canvist_test
        # The conformance fixtures for collaboration and the SSR renderer only
        # run with canvist_headless's optional features.
        cargo nextest run -p canvist_headless --all-features
        # Scene collaboration lives behind canvist_collab's `scene` feature.
        cargo nextest run -p canvist_collab --features scene
        # Painting backends are opt-in features.
        cargo nextest run -p canvist_paint --features cpu,gpu
      '';
      description = "Run cargo tests with nextest.";
      binary = "bash";
    };
    "test:docs" = {
      exec = ''
        set -e
        cargo test --doc --workspace --exclude canvist_wasm --exclude canvist_test
        cargo test --doc -p canvist_headless --all-features
        cargo test --doc -p canvist_collab --features scene
      '';
      description = "Run documentation tests.";
      binary = "bash";
    };
    "test:perf" = {
      exec = ''
        set -e
        cargo test --release --workspace --exclude canvist_wasm --exclude canvist_test --features canvist_headless/collab,canvist_collab/scene,canvist_paint/cpu,canvist_paint/gpu --test perf_budgets -- --ignored --test-threads=1 --nocapture
      '';
      description = "Fail when a hot path exceeds its performance budget (release mode; CANVIST_PERF_SLACK scales budgets).";
      binary = "bash";
    };
    "test:playwright" = {
      exec = ''
        set -e
        cd packages/canvist
        deno test --allow-all tests/playwright/
      '';
      description = "Run Playwright browser tests for the JS/WASM package.";
      binary = "bash";
    };
    "coverage:all" = {
      exec = ''
        set -e
        cargo llvm-cov nextest --lcov --output-path lcov.info
      '';
      description = "Run coverage across the crates.";
      binary = "bash";
    };
    "fix:all" = {
      exec = ''
        set -e
        fix:clippy
        fix:format
      '';
      description = "Fix all autofixable problems.";
      binary = "bash";
    };
    "fix:format" = {
      exec = ''
        set -e
        dprint fmt --config "$DEVENV_ROOT/dprint.json"
      '';
      description = "Format files with dprint.";
      binary = "bash";
    };
    "fix:clippy" = {
      exec = ''
        set -e
        cargo clippy --workspace --exclude canvist_wasm --exclude canvist_test --fix --allow-dirty --allow-staged --all-features --all-targets
      '';
      description = "Fix clippy lints for rust.";
      binary = "bash";
    };
    "deny:check" = {
      exec = ''
        set -e
        cargo deny check
      '';
      description = "Run cargo-deny checks for security advisories and license compliance.";
      binary = "bash";
    };
    "lint:style" = {
      exec = ''
        set -e
        monostyle check crates/canvist_model --fail-under 90
        monostyle check crates/canvist_layout --fail-under 90
        monostyle check crates/canvist_editor --fail-under 90
        monostyle check crates/canvist_extensions --fail-under 90
        monostyle check crates/canvist_collab --fail-under 90
        monostyle check crates/canvist_scene --fail-under 90
        monostyle check crates/canvist_canvas --fail-under 90
        monostyle check crates/canvist_canvas_wasm --fail-under 90
        monostyle check crates/canvist_paint --fail-under 90
        monostyle check crates/canvist_headless --fail-under 90
        monostyle check crates/canvist_headless_wasm --fail-under 90
        monostyle check packages/canvist-headless --fail-under 90
        monostyle check crates/canvist_web --fail-under 90
        monostyle check packages/canvist-web --fail-under 90
        monostyle check packages/canvist-react --fail-under 90
        monostyle check packages/canvist-vue --fail-under 90
        monostyle check packages/canvist-canvas --fail-under 90
        monostyle check platforms/dart --fail-under 90
        monostyle check services/cloud --fail-under 90
        monostyle check tools/footage --fail-under 90
      '';
      description = "Require readability and complexity scores of at least 90 for the v2 crates.";
      binary = "bash";
    };
    "lint:all" = {
      exec = ''
        set -e
        lint:clippy
        lint:format
        lint:style
        deny:check
      '';
      description = "Run all checks.";
      binary = "bash";
    };
    "lint:format" = {
      exec = ''
        set -e
        dprint check
      '';
      description = "Check that all files are formatted.";
      binary = "bash";
    };
    "lint:clippy" = {
      exec = ''
        set -e
        cargo clippy --workspace --exclude canvist_wasm --exclude canvist_test --all-features --all-targets
      '';
      description = "Check that all rust lints are passing.";
      binary = "bash";
    };
    "snapshot:review" = {
      exec = ''
        set -e
        cargo insta review
      '';
      description = "Review insta snapshots.";
      binary = "bash";
    };
    "snapshot:update" = {
      exec = ''
        set -e
        cargo nextest run
        cargo insta accept
      '';
      description = "Update insta snapshots.";
      binary = "bash";
    };
  };
}
