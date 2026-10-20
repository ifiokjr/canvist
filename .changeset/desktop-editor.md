---
"canvist_desktop": feat
---

# Add canvist_desktop, the editor for macOS, Windows, and Linux

`canvist_desktop` (new) is a native rich-text editor component for any winit app. `DesktopEditor` lays a `canvist_headless` document out with `canvist_layout` and parley, paints it through `canvist_paint` (vello on the GPU, `vello_cpu` for PNGs), and takes input as `InputEvent`s, so tests drive it headlessly:

- keys, with `Mod` as Cmd on macOS and Ctrl elsewhere, input rules, paging, and the clipboard shortcuts;
- input methods: preedit text underlined in place, and `ime_area()` for the candidate window;
- the clipboard: HTML and plain text out, HTML preferred on paste;
- mouse and trackpad selection: click, double and triple click, Shift-click, drag, and autoscroll;
- scrolling and high-DPI, with only the visible blocks laid out and painted;
- an AccessKit tree of text runs with character positions, and screen reader actions;
- system fonts, plus `register_font` for fonts loaded from bytes;
- typed commands from the engine's handles.

The `winit` feature (default) adds `winit::EditorWindow`, which embeds the editor in any winit app (vello on wgpu, the system clipboard, IME placement, and the AccessKit adapter), and `winit::run` for a window that holds only the editor. The `collab` feature joins y-websocket and Hocuspocus rooms with `connect` and `receive_sync`.

A keystroke's frame takes about 0.14 ms at 10,000 paragraphs, under a 4 ms budget. `examples/desktop` is a small Markdown editor with a toolbar, open and save, and live collaboration.
