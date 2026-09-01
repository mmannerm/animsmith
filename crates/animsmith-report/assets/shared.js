// animsmith report shared runtime — the pure helpers both generated viewers
// use. Kept in one asset so the single-clip and comparison documents cannot
// drift apart on theming or on how a URL fragment is read.
"use strict";

// The fallback palette used whenever a token cannot be resolved from the
// document. The report crate substitutes the dark values of tokens.css for
// this placeholder as it emits the runtime, so a token value is written in
// exactly one place; this asset is a template rather than standalone JS.
const ANIMSMITH_DEFAULT_PALETTE = "__ANIMSMITH_DARK_TOKENS__";

// Live token values from the root element, so the WebGL and canvas pose views
// paint with the same palette as the CSS. Anything that is not an exact
// `#rrggbb` string — an old browser, a stripped stylesheet, an injected
// value — falls back to that token's dark default. Callers resolve this once
// per theme change rather than per frame.
function animsmithPalette() {
  const palette = Object.assign({}, ANIMSMITH_DEFAULT_PALETTE);
  const root = typeof document === "undefined" ? null : document.documentElement;
  const styles = root && typeof getComputedStyle === "function" ? getComputedStyle(root) : null;
  if (!styles) return palette;
  for (const name of Object.keys(palette)) {
    const value = String(styles.getPropertyValue("--" + name) || "").trim();
    if (/^#[0-9a-f]{6}$/i.test(value)) palette[name] = value.toLowerCase();
  }
  return palette;
}

// `#rrggbb` to the 0..1 RGB triple a WebGL vertex buffer carries.
function animsmithRgb(hex) {
  if (!/^#[0-9a-f]{6}$/i.test(String(hex))) return [0.5, 0.5, 0.5];
  return [1, 3, 5].map((offset) => parseInt(String(hex).slice(offset, offset + 2), 16) / 255);
}

// Bounded, total parser for a report's URL fragment: `&`-separated
// `key=value` pairs. `embed` strips the page chrome for an iframe, `theme`
// pins light or dark, and `clip`, `frame`, and `finding` are the deep-link
// selectors.
//
// Each option has three states, because a fragment is navigated as well as
// loaded: `undefined` when the key never appeared (leave that state alone),
// `null`/`false` when the key appeared with a value this report will not
// honour (return that state to its default), and the value otherwise.
// Unknown keys, pairs without a key, and malformed percent escapes are
// ignored rather than thrown, and the fragment is read but never written, so
// no fragment can raise an exception or drive a loop. The work is bounded by
// the length cap alone; every pair inside it is read. Whether a clip exists,
// how many frames it has, and how many findings there are is knowledge the
// caller holds, so the caller clamps against its own embedded data.
const ANIMSMITH_MAX_FRAGMENT_CHARS = 4096;

function animsmithFragmentOptions(hash) {
  const options = {};
  if (typeof hash !== "string" || hash.length > ANIMSMITH_MAX_FRAGMENT_CHARS) return options;
  const body = hash.charAt(0) === "#" ? hash.slice(1) : hash;
  for (const pair of body.split("&")) {
    const split = pair.indexOf("=");
    if (split < 1) continue;
    const value = pair.slice(split + 1);
    switch (pair.slice(0, split)) {
      case "embed": options.embed = value === "1" || value === "true"; break;
      case "theme": options.theme = value === "light" || value === "dark" ? value : null; break;
      case "clip": options.clip = animsmithDecoded(value); break;
      case "frame": options.frame = animsmithIndex(value); break;
      case "finding": options.finding = animsmithIndex(value); break;
      default: break;
    }
  }
  return options;
}

// A malformed percent escape is a discarded value, never a thrown URIError.
function animsmithDecoded(value) {
  try { return decodeURIComponent(value); } catch (error) { return null; }
}

// A non-negative decimal index. Digits beyond the float range become
// Infinity, which every caller clamps against its own frame or finding
// count, so no separate length rule is needed here.
function animsmithIndex(value) {
  return /^[0-9]+$/.test(value) ? Number(value) : null;
}

// The two document-wide switches, applied to the root element so every visual
// consequence stays in the stylesheets. A fragment that does not mention a
// switch leaves it as it is — following a `#finding-…` anchor inside an
// embedded, theme-pinned report must not un-pin it — while an explicitly
// unusable value restores that switch's default.
function animsmithApplyDocument(options) {
  const root = typeof document === "undefined" ? null : document.documentElement;
  if (!root || typeof root.setAttribute !== "function") return options;
  if (options.theme !== undefined) {
    if (options.theme) root.setAttribute("data-theme", options.theme);
    else root.removeAttribute("data-theme");
  }
  if (options.embed !== undefined) {
    if (options.embed) root.setAttribute("data-embed", "1");
    else root.removeAttribute("data-embed");
  }
  return options;
}
