# canvist brand

The canvist logo is the **Wet Caret**: a text caret whose foot is still wet. canvist paints every glyph onto a canvas itself, so its cursor is a stroke of paint that hasn't dried yet.

![canvist lockup](./vector/lockup.svg)

The other directions explored in the first round are in git history and on the [comparison page](https://claude.ai/artifact/Gr2RqVoGgDZ4pSmgEYWJGU).

## Files

| Folder                    | Contents                                                                                                                                                                                                                    | Source        |
| ------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------- |
| `vector/`                 | Master SVGs: `mark`, `lockup` (each in light, `-dark`, `-mono` and `-mono-white`), `mark-small` for 16 to 63 px, `favicon.svg` (follows the browser's colour scheme), social cards                                          | Hand-authored |
| `print/`                  | PDF and EPS vectors of the mark and lockup in light, dark and mono, at 4 pt per master unit                                                                                                                                 | Generated     |
| `web/`                    | `favicon.ico` (16, 32, 48), `favicon.svg`, `apple-touch-icon.png` (180), `icon-192.png`, `icon-512.png`, `icon-maskable-512.png`, `site.webmanifest`, `og-image.png` (1200 × 630), `github-social-preview.png` (1280 × 640) | Generated     |
| `ios/AppIcon.appiconset/` | Xcode single-size app icon: 1024 light, dark (transparent) and tinted (grayscale) appearances with `Contents.json`                                                                                                          | Generated     |
| `macos/`                  | `canvist.icns` (16 to 1024, including @2x)                                                                                                                                                                                  | Generated     |
| `windows/`                | `canvist.ico` (16, 24, 32, 48, 64, 128, 256)                                                                                                                                                                                | Generated     |
| `android/`                | `res/` drop-in: adaptive icon (foreground, background, monochrome) and legacy square and round launcher icons for mdpi to xxxhdpi, plus `play-store-512.png`                                                                | Generated     |
| `flutter/`                | 1024 px sources and a `flutter_launcher_icons.yaml` block for Android, iOS (including dark and tinted), web, Windows and macOS                                                                                              | Generated     |
| `linux/hicolor/`          | `apps/canvist.png` at 16, 22, 24, 32, 48, 64, 128, 256 and 512, plus `scalable/apps/canvist.svg`                                                                                                                            | Generated     |

The marks are hand-built geometry. The wordmark is outlined from [Bricolage Grotesque](https://fonts.google.com/specimen/Bricolage+Grotesque) ExtraBold (SIL Open Font License), so no font is needed to render it. The social cards use Bricolage Grotesque Medium and Geist Mono, also outlined.

### Regenerating

Everything outside `vector/` (except this readme and the script) is generated from the masters. After changing a master, run this from the repository root:

```sh
deno run --allow-read --allow-write assets/brand/generate.ts
```

The script rasterises with [resvg](https://github.com/linebender/resvg) (WebAssembly build), writes the ICO and ICNS containers and the PDF and EPS vectors itself, and needs no other tools.

## Usage

### Clear space

Leave empty space of at least **half the height of the "c"** on every side of the mark or lockup, measured from the artwork, not from the file edge. Nothing else (text, other logos, the edge of a card) goes inside it.

### Minimum sizes

| Artwork                   | Screen      | Print      |
| ------------------------- | ----------- | ---------- |
| Lockup                    | 72 px wide  | 20 mm wide |
| Mark with drip (`mark`)   | 64 px       | 12 mm      |
| Small mark (`mark-small`) | 16 to 63 px | 6 to 12 mm |

Below 64 px the drip turns into noise, so use `mark-small` (or `favicon.svg`), which drops the drip and snaps the caret to the pixel grid. The generated icons already switch at that size.

### Colours

| Name          | Hex       | Use                                             |
| ------------- | --------- | ----------------------------------------------- |
| Ink           | `#17141F` | The "c" and the wordmark on light backgrounds   |
| Paint         | `#3B2CF5` | The caret on light backgrounds (7.3:1 on white) |
| Paper         | `#F5F3FF` | The "c" and the wordmark on dark backgrounds    |
| Paint on dark | `#8A80FF` | The caret on dark backgrounds (5.7:1 on Ink)    |
| Tile          | `#FFFFFF` | App icon background                             |

When only one colour is available (embossing, single-colour print, system tinting), use the `-mono` files in black or `-mono-white`, or recolour them to any single colour with enough contrast.

### Do

- Use the files in this folder as they are. Pick `-dark` on dark backgrounds and the light version on light ones.
- Keep the caret attached to the wordmark in the lockup. It is part of the name.
- Use the mark alone where space is square or tight: app icons, avatars, favicons.
- Let the caret blink in motion (about 1 s cycle, no fade), or let the drip fall once on first paint.

### Don't

- Retype the wordmark in a live font, or set "canvist" next to the mark in another typeface.
- Recolour the caret to anything other than Paint, Paint on dark, or the single mono colour.
- Stretch, skew, rotate, outline, or add shadows, gradients or glows.
- Put the light lockup on a dark background, or place either version on a busy photo without a solid panel.
- Exaggerate the drip (longer runs, splatter, extra drops). One small drop keeps it playful; more turns it into a horror poster.
- Use the drip version below 64 px.

## Platform notes

**Web.** Copy `web/` to the site root and add:

```html
<link rel="icon" href="/favicon.ico" sizes="32x32">
<link rel="icon" href="/favicon.svg" type="image/svg+xml">
<link rel="apple-touch-icon" href="/apple-touch-icon.png">
<link rel="manifest" href="/site.webmanifest">
<meta property="og:image" content="https://<site>/og-image.png">
```

**GitHub.** Upload `web/github-social-preview.png` under the repository's Settings → General → Social preview.

**iOS.** Replace the app's `Assets.xcassets/AppIcon.appiconset` with `ios/AppIcon.appiconset`. The light and tinted icons have no alpha channel, as App Store Connect requires.

**macOS.** Use `macos/canvist.icns` as the bundle icon (`CFBundleIconFile`).

**Windows.** Use `windows/canvist.ico` as the executable and installer icon.

**Android.** Merge `android/res/` into the app's `src/main/res/`. Upload `android/play-store-512.png` as the Play Store icon.

**Flutter.** Copy `flutter/flutter_launcher_icons.yaml` into the app (or its `pubspec.yaml`), point the paths at the `flutter/` images, and run `dart run flutter_launcher_icons`.

**Linux.** Install `linux/hicolor/` into `/usr/share/icons/hicolor/` (or `~/.local/share/icons/hicolor/`) and reference the icon as `canvist` in the `.desktop` file.
