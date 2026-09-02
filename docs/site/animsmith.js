// The two things the published book needs that mdBook's own templates do
// not do, kept in one tracked script so no page ever carries an inline one.
//
// 1. A way back to the front door. The site's home is a hand-authored page
//    at the artifact root, and no chapter links it: mdBook's sidebar lists
//    chapters only, and its title bar is a plain heading. Both gain the
//    same link here, built from `path_to_root` — the site-root prefix
//    mdBook writes into every page — so home is one click from anywhere.
//
// 2. A theme bridge between the book and the reports it embeds.
//
// A generated AnimSmith report is a whole document in an <iframe>, so it
// cannot see the book's theme: left alone it follows the reader's system
// scheme and a navy page ends up framing a white report. The report viewers
// already take `theme=light|dark` in their URL fragment and re-apply it on
// `hashchange`, so this script only has to keep that one fragment key in step
// with mdBook's theme class on <html>.
//
// It is deliberately small and self-contained: no external resource, no
// storage, and no read of an embedded frame's document — it writes one link
// into the page's own chrome and the parent's own `src` attribute, which
// stays allowed for a file:// preview and for the published origin alike.
"use strict";

(function () {
  // -- a way home ---------------------------------------------------

  // mdBook writes the current page's site-root prefix into every page as
  // `path_to_root`: "" at the root, "../../" two directories down. The front
  // door is that root's index.html, so one prefix answers every page. A page
  // built without the binding (a harness, a future template) gets no link
  // rather than a guessed one.
  function homeHref() {
    return typeof path_to_root === "string" ? path_to_root + "index.html" : null;
  }

  function homeLink(href, label) {
    var link = document.createElement("a");
    link.setAttribute("href", href);
    link.className = "as-home";
    link.textContent = label;
    return link;
  }

  // Two affordances, one destination: the first sidebar entry, above every
  // part, and the title in the top bar, which the stylesheet draws as the
  // logo. Both are written once — mdBook's own toc.js has already built the
  // sidebar by the time this runs — and both check for themselves first, so
  // loading the script twice cannot produce two of either.
  function addHome() {
    var href = homeHref();
    if (href === null) return;
    var chapters = document.querySelector(".sidebar .chapter");
    if (chapters && !chapters.querySelector("a.as-home")) {
      var item = document.createElement("li");
      item.className = "chapter-item as-home-item";
      item.appendChild(homeLink(href, "Home"));
      chapters.insertBefore(item, chapters.firstChild);
    }
    var title = document.querySelector(".menu-title");
    if (title && !title.querySelector("a.as-home")) {
      // The heading keeps its own text: it is what a screen reader announces
      // and what a browser without mask support still shows.
      var label = title.textContent;
      title.textContent = "";
      title.appendChild(homeLink(href, label));
    }
  }

  // -- theme bridge -------------------------------------------------

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

  // A report document under docs/visuals/, in every spelling a frame source
  // is written in: the relative one the repository Markdown carries
  // (`../visuals/x.report.html`), the site-absolute one staging writes for
  // the released root (`/animsmith/docs/visuals/…`), and the one it writes
  // for the development subtree (`/animsmith/dev/docs/visuals/…`). All three
  // have `visuals/` as a whole path segment, so one segment test covers them
  // — a directory merely ending in `visuals` does not match — and the path is
  // tested before the fragment, so a deep-linked frame is recognised too.
  function isReport(source) {
    var path = String(source).split("#")[0];
    return /\.html$/.test(path) && /(^|\/)visuals\//.test(path);
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
    addHome();
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
