# Pages site theme

This directory is the visual theme for the GitHub Pages documentation
site. It is not published as documentation itself.

`scripts/build-docs-site.py` stages the tracked files here as mdBook's
theme-override directory (`<stage>/theme/`) and registers
[`animsmith.css`](animsmith.css) as `additional-css`, so it loads after
mdBook's own `variables.css`, `general.css` and `chrome.css`. mdBook
copies `fonts/*` to `book/fonts/` and links `fonts/fonts.css` on every
page; it copies `favicon.svg` and `favicon.png`, but it does **not**
copy files it does not recognise, which is why `logo.svg` is also
embedded in the stylesheet as a data URI. The staging and preview
commands live in
[DEVELOPMENT.md](../../DEVELOPMENT.md#github-pages-preview).

| File | What it is |
| --- | --- |
| [`animsmith.css`](animsmith.css) | The theme: tokens, mdBook variable mapping, typography, chrome and content styling |
| [`fonts/fonts.css`](fonts/fonts.css) | `@font-face` rules for the self-hosted subsets |
| [`logo.svg`](logo.svg) | Mark plus "AnimSmith" wordmark, wordmark converted to outlines |
| [`favicon.svg`](favicon.svg) | Mark on a light chip, so it reads in light and dark tab bars |
| [`favicon.png`](favicon.png) | 32x32 raster fallback of the same artwork |
| [`redirects.toml`](redirects.toml) | Compatibility routes (owned by the site build, not the theme) |

The mark is the letter A drawn the way the report viewer draws a rig:
two bone strokes from the apex to the base, an accent crossbar, hollow
joints at the base and crossbar, and a filled accent keyframe diamond at
the apex.

## Design tokens

`animsmith.css` defines these on `.light` and on `.navy` as `--as-*`
custom properties, then maps them onto the mdBook theme variables. The
dark values are the palette
[`crates/animsmith-report/assets/viewer.css`](../../crates/animsmith-report/assets/viewer.css)
already uses, so an embedded report reads as part of the page. The other
three built-in themes (`coal`, `ayu`, `rust`) keep their stock palettes:
rules outside the two theme blocks read tokens through
`var(--as-x, <mdBook fallback>)`.

| Token | Light | Dark | Use |
| --- | --- | --- | --- |
| `--as-ground` | `#f4f5f9` | `#17171f` | The flat surface: page chrome in light, the reading column in dark |
| `--as-surface` | `#ffffff` | `#1e1e2a` | The raised surface: the reading column in light, sidebar and panels in dark |
| `--as-page` | `--as-surface` | `--as-ground` | Content background (`--bg`) |
| `--as-panel` | `--as-ground` | `--as-surface` | Sidebar, blockquotes, `details` (`--sidebar-bg`, `--quote-bg`) |
| `--as-ink` | `#1a1e2c` | `#d5d9e5` | Body and heading text (`--fg`, `--sidebar-fg`, `--icons-hover`) |
| `--as-muted` | `#5b6382` | `#9099b2` | Secondary text, part titles, table headers, icons (`--icons`) |
| `--as-line` | `#d9deea` | `#2c2c3c` | Hairlines: rules, cell borders, panel borders (`--table-border-color`) |
| `--as-line-strong` | `#b9c1d6` | `#3d3d52` | Emphasised borders: header underline, popup border, scrollbar |
| `--as-accent` | `#3b67d6` | `#7aa2f7` | Active sidebar item, focus ring, logo diamond (`--sidebar-active`) |
| `--as-accent-ink` | `#2f56b8` | `#9ab8ff` | Text links at body contrast (`--links`) |
| `--as-accent-soft` | `#e7edfb` | `#252c48` | Active sidebar background, search highlight |
| `--as-code-bg` | `#eef0f6` | `#23232f` | Code blocks, inline code chips, table header row |
| `--as-error` | `#cf3f5b` | `#f7768e` | Reserved for error severity |
| `--as-warning` | `#946414` | `#e0af68` | Warning severity; drives mdBook's `--warning-border` |
| `--as-pass` | `#287a3b` | `#9ece6a` | Reserved for passing severity |
| `--as-note` | `#6b7390` | `#9099b2` | Note severity; also unbuilt sidebar entries |

Severity colours carry meaning. They are not used for decoration.

## Typography

Headings are Barlow Semi Condensed (600/700), body copy is Source Sans 3
(400/600 plus 400 italic), and code, table headers and small labels are
JetBrains Mono (400/500). Every face is self-hosted; the site makes no
external font requests.

## Font provenance

All three families are licensed under the SIL Open Font License 1.1.
The licence text for each sits next to the files:
[`fonts/OFL-BarlowSemiCondensed.txt`](fonts/OFL-BarlowSemiCondensed.txt),
[`fonts/OFL-SourceSans3.txt`](fonts/OFL-SourceSans3.txt),
[`fonts/OFL-JetBrainsMono.txt`](fonts/OFL-JetBrainsMono.txt).

| Family | Upstream | Version | Upstream file | Committed file | sha256 of committed file |
| --- | --- | --- | --- | --- | --- |
| Barlow Semi Condensed | `google/fonts`, `ofl/barlowsemicondensed/` | v1.408 (upstream `jpt/barlow`) | `BarlowSemiCondensed-SemiBold.ttf` | `fonts/barlow-semi-condensed-600.woff2` | `69c0436c449acc147b89df7c0dd2c107a045812819cc0d1f5616b457fe4921a1` |
| Barlow Semi Condensed | `google/fonts`, `ofl/barlowsemicondensed/` | v1.408 (upstream `jpt/barlow`) | `BarlowSemiCondensed-Bold.ttf` | `fonts/barlow-semi-condensed-700.woff2` | `a08ccfbcaf4d28f552de452d67c6b4a51bb30843d75be1f939c221ae5ee3c137` |
| Source Sans 3 | `adobe-fonts/source-sans` release `3.052R` | 3.052 | `WOFF2/OTF/SourceSans3-Regular.otf.woff2` | `fonts/source-sans-3-400.woff2` | `9403730f0fea20eb2ede0407f443680ed17b392460bb0bd4e1cbf638e67ff0c0` |
| Source Sans 3 | `adobe-fonts/source-sans` release `3.052R` | 3.052 | `WOFF2/OTF/SourceSans3-It.otf.woff2` | `fonts/source-sans-3-400italic.woff2` | `873b6eb789dba775039af53b22a9c16f04ad068611b493b4a9362f4fb1b43cfc` |
| Source Sans 3 | `adobe-fonts/source-sans` release `3.052R` | 3.052 | `WOFF2/OTF/SourceSans3-Semibold.otf.woff2` | `fonts/source-sans-3-600.woff2` | `9e78df86600484f3c6d3759cc7f8c2ef26153b85de41539b4bfe0478060e812e` |
| JetBrains Mono | `JetBrains/JetBrainsMono` release `v2.304` | 2.304 | `fonts/webfonts/JetBrainsMono-Regular.woff2` | `fonts/jetbrains-mono-400.woff2` | `aa1ad10917786937cfb9077dc3e245815762e090f7a06b9f24951cbfa0221acd` |
| JetBrains Mono | `JetBrains/JetBrainsMono` release `v2.304` | 2.304 | `fonts/webfonts/JetBrainsMono-Medium.woff2` | `fonts/jetbrains-mono-500.woff2` | `ed065cf493ae85d69067bdf0cb270558c9ce62f048d71d57e740d46ef70225f5` |

Download URLs:

- `https://raw.githubusercontent.com/google/fonts/main/ofl/barlowsemicondensed/BarlowSemiCondensed-{SemiBold,Bold}.ttf`
  (sha256 `bd299f4bc5b44d30be8d42e9bad3a5df7d66af1cd55d0ed72a8b8916360a1424`
  and `5bd6757e459a110da81b44532f0f43058e2dab2401688abca84f2fdbde193cbb`)
- `https://github.com/adobe-fonts/source-sans/releases/download/3.052R/WOFF2-source-sans-3.052R.zip`
  (sha256 `d7f6724027fe5ca0bab44d6121284f1fa8f66dec5f2864aa969aed347041ac95`)
- `https://github.com/JetBrains/JetBrainsMono/releases/download/v2.304/JetBrainsMono-2.304.zip`
  (sha256 `6f6376c6ed2960ea8a963cd7387ec9d76e3f629125bc33d1fdcd7eb7012f7bbf`)

### Subsetting

Each face was reduced to the characters the documentation uses, with
`fonttools` 4.64.0 and `brotli`:

```console
$ python3 -m pip install fonttools brotli
$ TEXT=U+0000-00FF,U+0100-017F,U+0131,U+0152-0153,U+0192,U+02BB-02BC,U+02C6,U+02DA,U+02DC,U+0370-03FF,U+2000-206F,U+2074,U+20AC,U+2122,U+2190-21FF,U+2200-22FF,U+2713-271F,U+FEFF,U+FFFD
$ MONO=$TEXT,U+2500-257F
$ pyftsubset <upstream file> --output-file=<committed file> --flavor=woff2 \
      --unicodes="$TEXT" --layout-features+=tnum,lnum,zero \
      --name-IDs='*' --notdef-outline
```

The mono faces use `$MONO` instead of `$TEXT`: box-drawing characters
appear only in CLI transcripts, which are always set in code.
`--layout-features+=tnum` keeps tabular figures available for tables
(Source Sans 3 and JetBrains Mono are tabular by default; Barlow Semi
Condensed needs the feature). `--name-IDs='*'` keeps the OFL copyright
and licence strings inside each file. Total committed font weight is
about 260 KB.
