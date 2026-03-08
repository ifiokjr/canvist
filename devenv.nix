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
      extra.knope
      mdbook
      nixfmt
      rustup
      shfmt
      wasm-pack
    ]
    ++ lib.optionals stdenv.isDarwin [
      coreutils
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
      '';
      description = "Run cargo tests with nextest.";
      binary = "bash";
    };
    "test:docs" = {
      exec = ''
        set -e
        cargo test --doc --workspace --exclude canvist_wasm --exclude canvist_test
      '';
      description = "Run documentation tests.";
      binary = "bash";
    };
    "test:playwright" = {
      exec = ''
        set -e
        cargo nextest run --package canvist_test
      '';
      description = "Run playwright browser integration tests.";
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
        cargo clippy --workspace --fix --allow-dirty --allow-staged --all-features --all-targets
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
    "lint:all" = {
      exec = ''
        set -e
        lint:clippy
        lint:format
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
        cargo clippy --workspace --all-features --all-targets
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
