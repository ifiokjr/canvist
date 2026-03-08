# Getting Started

## Prerequisites

canvist uses [devenv](https://devenv.sh/) for a reproducible development environment. Follow the [getting started instructions](https://devenv.sh/getting-started/) to install it.

## Setup

```bash
# Clone the repository.
git clone https://github.com/ifiokjr/canvist.git
cd canvist

# Allow direnv to load the environment.
direnv allow .

# Install cargo binary tools.
install:all
```

## Development commands

| Command | Description |
|---------|-------------|
| `build:all` | Build all crates |
| `build:wasm` | Build the WASM package |
| `build:book` | Build the mdbook documentation |
| `test:all` | Run all tests |
| `test:cargo` | Run cargo tests with nextest |
| `test:docs` | Run documentation tests |
| `test:playwright` | Run Playwright browser tests |
| `fix:all` | Auto-fix lints and formatting |
| `fix:format` | Format with dprint |
| `fix:clippy` | Fix clippy lints |
| `lint:all` | Run all linters |
| `lint:format` | Check formatting |
| `lint:clippy` | Check clippy lints |
| `deny:check` | Run cargo-deny security checks |
| `coverage:all` | Generate code coverage |
| `snapshot:review` | Review insta snapshots |

## Building for the web

```bash
# Build the WASM package.
build:wasm

# The output is in crates/canvist_wasm/pkg/
```
