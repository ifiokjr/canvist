# Contributing

Thank you for considering contributing to canvist!

## Development setup

See [Getting Started](./getting-started.md) for environment setup.

## Making changes

1. Create a feature branch from `main`.
2. Make your changes with clear, focused commits.
3. Add a changeset documenting your changes:

   ```bash
   knope document-change
   ```

4. Push your branch and open a pull request.

## Code style

- All Rust code uses **hard tabs** and edition 2024.
- Run `fix:all` before committing to auto-fix formatting and lints.
- Every public item must have a documentation comment.
- Use `thiserror` for error types.

## Testing

- Run `test:all` to execute the full test suite.
- Add tests for new functionality.
- Use `insta` for snapshot testing where appropriate.

## Architecture

See [Architecture](./architecture.md) for an overview of the crate structure.
