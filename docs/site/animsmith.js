// Theme bridge between the book and the reports it embeds.
//
// A generated AnimSmith report is a whole document in an <iframe>, so it
// cannot see the book's theme: left alone it follows the reader's system
// scheme and a navy page ends up framing a white report. The report viewers
// already take `theme=light|dark` in their URL fragment and re-apply it on
// `hashchange`, so this script only has to keep that one fragment key in step
// with mdBook's theme class on <html>.
//
// It is deliberately small and self-contained: no external resource, no
// inline script in any page, and no read of the frame's document — only the
// parent's own `src` attribute is written, which stays allowed for a
// file:// preview and for the published origin alike.
"use strict";

(function () {
  // mdBook's five built-in themes, as lists rather than lookup objects so a
  // class name can never reach Object.prototype. Anything unknown is left to
  // the report's own system-scheme default rather than guessed at.
  var DARK = ["navy", "coal", "ayu"];
  var LIGHT = ["light", "rust"];

  // Fragment keys the report understands. `theme` is rewritten; the rest are
  // the reader's own deep link and are preserved exactly as written.
  var THEME_KEY = "theme";

  function bookTheme(root) {
    var names = String(root.className || "").split(/\s+/);
    for (var i = 0; i < names.length; i += 1) {
      if (DARK.indexOf(names[i]) !== -1) return "dark";
      if (LIGHT.indexOf(names[i]) !== -1) return "light";
    }
    return null;
  }

  // A report document under docs/visuals/. Staging rewrites every frame
  // source to a site-absolute path (`/animsmith/docs/visuals/…`), which is
  // the only spelling a published page carries; the relative spelling the
  // repository Markdown is written in is accepted too, so the rule holds
  // wherever the page is read. The path is tested before the fragment, so a
  // deep-linked frame is recognised as well.
  function isReport(source) {
    var path = String(source).split("#")[0];
    if (!/\.html$/.test(path)) return false;
    return path.indexOf("/docs/visuals/") !== -1 || /(^|\/)visuals\//.test(path);
  }

  // `path#a=1&b=2` with `theme` replaced by `value` and every other pair kept
  // in its authored order.
  function withTheme(source, value) {
    var split = String(source).indexOf("#");
    var path = split === -1 ? String(source) : String(source).slice(0, split);
    var body = split === -1 ? "" : String(source).slice(split + 1);
    var pairs = [];
    var parts = body ? body.split("&") : [];
    for (var i = 0; i < parts.length; i += 1) {
      var pair = parts[i];
      var equals = pair.indexOf("=");
      var key = equals < 1 ? pair : pair.slice(0, equals);
      if (key !== THEME_KEY && pair) pairs.push(pair);
    }
    pairs.push(THEME_KEY + "=" + value);
    return path + "#" + pairs.join("&");
  }

  function apply() {
    var root = document.documentElement;
    var theme = bookTheme(root);
    if (!theme) return;
    var frames = document.getElementsByTagName("iframe");
    for (var i = 0; i < frames.length; i += 1) {
      var frame = frames[i];
      var source = frame.getAttribute("src");
      if (!source || !isReport(source)) continue;
      var next = withTheme(source, theme);
      // Only a real change is written: assigning the same value would
      // re-navigate the frame on every observed mutation.
      if (next !== source) frame.setAttribute("src", next);
    }
  }

  function start() {
    apply();
    if (typeof MutationObserver !== "function") return;
    new MutationObserver(apply).observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["class"],
    });
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", start);
  } else {
    start();
  }
})();
