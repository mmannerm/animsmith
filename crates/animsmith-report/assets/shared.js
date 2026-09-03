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
// pins light or dark, and `clip`, `with`, `frame`, and `finding` are the
// deep-link selectors. `with` names a second clip of the same document to
// show beside the selected one; an empty or unusable value is the default,
// which is the selected clip alone.
//
// Each option has three states, because a fragment is navigated as well as
// loaded: `undefined` when the key never appeared (leave that state alone),
// `null`/`false` when the value cannot be read at all (return that state to
// its default), and the value otherwise. A value that reads but overshoots is
// the caller's to place: a frame past the end of a clip is clamped to its last
// frame, not discarded, because a reader who asks for a position wants the
// nearest one the document can show.
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
      case "with": options.with = animsmithDecoded(value); break;
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

// Two clips shown together are shown at one normalized phase, and both
// documents place the second one against the first the same way: the grid
// frame nearest that phase, in the frame count the clip actually has.
//
// It selects a sample the checks already judged — it never resamples,
// interpolates, or retimes — which is what the disclosure says wherever a
// document shows two timelines at once. Both viewers label the mapping with
// that one sentence, and both label both source times beside it, so the
// reader is never shown one clock for two clips.
const ANIMSMITH_PHASE_DISCLOSURE = "normalized phase, not a time warp";

// Where a frame sits in its own clip, on [0, 1]. A clip with fewer than two
// frames has no span to sit in, so every frame of it is phase 0 rather than
// a division by zero.
function animsmithPhaseOf(frames, at) {
  return frames > 1 ? at / (frames - 1) : 0;
}

// The inverse: the clip's own grid frame nearest a phase. A phase past the
// end lands on the last frame, and anything unreadable — a negative, a NaN
// from an empty clip — lands on the first, so the result is always a frame
// the clip has.
function animsmithFrameAtPhase(frames, phase) {
  if (!(frames > 1) || !(phase > 0)) return 0;
  return Math.min(frames - 1, Math.round(phase * (frames - 1)));
}

// Repaint hook for a reader who switches their system theme with the report
// open: the CSS follows on its own, the canvas views need the new palette.
function animsmithOnSchemeChange(repaint) {
  if (typeof matchMedia !== "function") return;
  const scheme = matchMedia("(prefers-color-scheme: light)");
  if (!scheme || typeof scheme.addEventListener !== "function") return;
  scheme.addEventListener("change", repaint);
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

// A frame loop with exactly one owner, for the transports both documents
// carry. Playback in either is nothing but advancing one number per frame,
// and both had the same defect: pausing left the callback it had scheduled
// alive, and playing scheduled another, so every pause-then-play added a
// redraw chain that ran for as long as the document stayed open.
//
// `stop` cancels the frame in flight, and the run number retires a callback
// the browser hands back anyway — a browser that ignores the cancellation
// still ends up with one loop, whether it was stopped or restarted.
function animsmithFrameLoop(advance) {
  const loop = { running: false, handle: 0, run: 0 };
  function chain(mine) {
    return function frame(now) {
      // Stopped, or retired by a later start: this chain ends here rather
      // than scheduling a successor.
      if (!loop.running || mine !== loop.run) return;
      advance(now);
      loop.handle = requestAnimationFrame(frame);
    };
  }
  return {
    get running() { return loop.running; },
    start() {
      loop.running = true;
      loop.run += 1;
      loop.handle = requestAnimationFrame(chain(loop.run));
    },
    // Returns false when it was already stopped, so a caller can tell a real
    // pause from a redundant one.
    stop() {
      if (!loop.running) return false;
      loop.running = false;
      if (loop.handle) {
        cancelAnimationFrame(loop.handle);
        loop.handle = 0;
      }
      return true;
    },
  };
}
