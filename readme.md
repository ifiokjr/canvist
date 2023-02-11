# cavnvist

> a canvas based text editor written in rust.

## Motivation

**Figma** and **Google Docs** manage to do this really well. It would be cool to build a wasm first implementation of rendering to the canvas which is fully open source.

- Convert document events to canvas actions
- Render to the canvas
- Use `yrs` for managing the editor state and supporting first class collaboration
- Support both a json data model and a binary data model
- Full accessibility support similar to Google Docs (via parallel hidden svg dom)
- Potentially render via `skia`
- Support server rendering (print canvas to image)

The canvas renderer should be an implementation detail, so that in the future we can rendering to non-web canvas.

**Non-goals:**

- Currently, size is not a concern as WASM builds tend to be larger.

## Contributing

[`devenv`](https://devenv.sh/) is used to provide a reproducible development environment for this
project. Follow the [getting started instructions](https://devenv.sh/getting-started/).

To automatically load the environment you should
[install direnv](https://devenv.sh/automatic-shell-activation/) and then load the `direnv`.

```bash
# The security mechanism didn't allow to load the `.envrc`.
# Since we trust it, let's allow it execution.
direnv allow .
```

At this point you should see the `nix` commands available in your terminal.

To setup recommended configuration for your favourite editor run the following commands.

```bash
setup:vscode # Setup vscode
setup:helix  # Setup helix configuration
```
