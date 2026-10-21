---
"canvist_extensions": feat
"canvist_layout": feat
"canvist_headless": feat
"@canvist/headless": feat
"canvist-dart": feat
---

# Add images, YouTube and Twitch videos, audio, and file attachments

Five media blocks, each behind its own Cargo feature (`media` enables them all, and `full` includes them). Each is an atomic block whose attributes say what to show. Layout reserves a slot for it, sized by its `height` attribute, and the host draws the picture, player, or file card there:

- `Image` (`image`): `setImage` with `src`, `alt`, `title`, `width`, and `height`. Typing `![alt](src "title")` inserts one, even with typography's curly quotes.
- `Youtube` (`youtube`) and `Twitch` (`twitch`): `setYoutubeVideo` and `setTwitchVideo`, with TipTap's attributes. As in TipTap, they refuse URLs that are not `http(s)` on YouTube's or Twitch's hosts, so a `javascript:` URL never becomes a frame.
- `Audio` (`audio`): `setAudio` with `controls`, `autoplay`, `loop`, and `muted`.
- `File` (`file`): Remirror's attachment card, with `insertFile`, `url`, `fileName`, `fileType`, and `fileSize`.

All five are data-driven by one implementation. Attributes equal to their default are left out, as everywhere else.

- HTML reads and writes TipTap's markup: `<img>`, `<audio>`, and `<iframe>` inside `<div data-youtube-video>` or `<div data-twitch-video>`, with a `title` so assistive technology can name the frame. File cards use `<div data-type="file">`. An image inside a paragraph splits it, and iframes from other sites are dropped.
- Markdown writes and reads images as `![alt](src "title")`; other media become links, a file without a name linked by its URL.
- The StarterKit theme reserves slots for video, audio, and file blocks.

Media commands type their source as a required string in TypeScript and Dart, as Rust already required; before, `setImage({ src: null })` type-checked and failed at runtime. Server-rendered image slots carry `role="img"` and the image's `alt` text as their accessible name. An empty `alt` marks a decorative image, which gets none.
