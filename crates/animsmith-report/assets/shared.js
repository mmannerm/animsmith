// animsmith report shared runtime — the pure helpers both generated viewers
// use. Kept in one asset so the single-clip and comparison documents cannot
// drift apart on theming or on how a URL fragment is read.
"use strict";

// The ten design tokens of tokens.css. These dark values are also the
// fallback whenever a token cannot be resolved.
const ANIMSMITH_DEFAULT_PALETTE = {
  ground: "#17171f", surface: "#1e1e2a", ink: "#d5d9e5", muted: "#9099b2",
  line: "#3a3a4e", accent: "#7aa2f7", error: "#f7768e", warning: "#e0af68",
  pass: "#9ece6a", note: "#bb9af7",
};

// Live token values from the root element, so the WebGL and canvas pose views
// paint with the same palette as the CSS. Anything that is not an exact
// `#rrggbb` string — an old browser, a stripped stylesheet, an injected
// value — falls back to that token's dark default.
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
// selectors. Unknown keys, pairs without a key, malformed percent escapes,
// and out-of-range values are ignored rather than thrown, and the fragment
// is read but never written, so no input here can raise an exception or
// drive a loop. Whether a clip exists, how many frames it has, and how many
// findings there are is knowledge the caller holds, so the caller does that
// last bound check against its own embedded data.
const ANIMSMITH_MAX_FRAGMENT_CHARS = 4096;
const ANIMSMITH_MAX_FRAGMENT_PAIRS = 32;

function animsmithFragmentOptions(hash) {
  const options = { embed: false, theme: null, clip: null, frame: null, finding: null };
  if (typeof hash !== "string" || hash.length > ANIMSMITH_MAX_FRAGMENT_CHARS) return options;
  const body = hash.charAt(0) === "#" ? hash.slice(1) : hash;
  for (const pair of body.split("&", ANIMSMITH_MAX_FRAGMENT_PAIRS)) {
    const split = pair.indexOf("=");
    if (split < 1) continue;
    const value = pair.slice(split + 1);
    switch (pair.slice(0, split)) {
      case "embed": if (value === "1" || value === "true") options.embed = true; break;
      case "theme": if (value === "light" || value === "dark") options.theme = value; break;
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

// A non-negative decimal index of at most nine digits: every frame or
// finding a report can address, and small enough that a caller clamping
// against its own data stays in bounds.
function animsmithIndex(value) {
  return /^[0-9]{1,9}$/.test(value) ? Number(value) : null;
}

// The two document-wide switches, applied to the root element so every
// visual consequence stays in the stylesheets. Re-applying a fragment
// without them restores the document default.
function animsmithApplyDocument(options) {
  const root = typeof document === "undefined" ? null : document.documentElement;
  if (!root || typeof root.setAttribute !== "function") return options;
  if (options.theme) root.setAttribute("data-theme", options.theme);
  else root.removeAttribute("data-theme");
  if (options.embed) root.setAttribute("data-embed", "1");
  else root.removeAttribute("data-embed");
  return options;
}
