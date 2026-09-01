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
