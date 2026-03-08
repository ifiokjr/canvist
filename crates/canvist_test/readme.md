# canvist_test

> Integration tests for the canvist canvas editor using playwright-rs.

This crate contains end-to-end browser tests that:

1. Build the WASM package
2. Serve it in a local HTTP server
3. Use Playwright to automate a browser
4. Verify rendering, text input, selection, and accessibility
