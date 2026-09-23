// The additions the published book needs that mdBook's own templates do
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
// 3. Readable report cards and visible, keyboard-accessible table overflow.
//
// It is self-contained: no external resource, no
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

  // -- report tables -------------------------------------------------

  var ISSUE_HEADERS = [
    "ID", "Severity", "Problem and impact", "Primary owner", "Current action",
    "Future AnimSmith potential", "Evidence/status",
  ];
  var SET_HEADERS = ["Set", "Controller use", "Adoption decision", "Exact members"];

  function copyContents(from, to) {
    var copy = from.cloneNode(true);
    while (copy.firstChild) to.appendChild(copy.firstChild);
  }

  function hasHeaders(table, expected) {
    if (!table || !table.tHead || !table.tBodies.length) return false;
    var headers = table.tHead.rows[0] && table.tHead.rows[0].cells;
    if (!headers || headers.length !== expected.length) return false;
    for (var i = 0; i < headers.length; i += 1) {
      if (headers[i].textContent.trim() !== expected[i]) return false;
    }
    var rows = table.tBodies[0].rows;
    if (!rows.length) return false;
    for (var j = 0; j < rows.length; j += 1) {
      if (rows[j].cells.length !== expected.length) return false;
    }
    return true;
  }

  function renderCards(table, headers, kind, label) {
    var cards = document.createElement("div");
    cards.className = "as-" + kind + "-cards";
    cards.setAttribute("role", "group");
    cards.setAttribute("aria-label", label);
    var rows = table.tBodies[0].rows;
    for (var r = 0; r < rows.length; r += 1) {
      var card = document.createElement("article");
      card.className = "as-" + kind + "-card";
      var title = document.createElement("h3");
      copyContents(rows[r].cells[0], title);
      card.appendChild(title);
      var details = document.createElement("dl");
      for (var c = 1; c < headers.length; c += 1) {
        var term = document.createElement("dt");
        term.textContent = headers[c];
        var value = document.createElement("dd");
        copyContents(rows[r].cells[c], value);
        details.appendChild(term);
        details.appendChild(value);
      }
      card.appendChild(details);
      cards.appendChild(card);
    }
    return cards;
  }

  function issueCards() {
    var heading = document.getElementById && document.getElementById("technical-issue-register");
    if (!heading || heading.tagName !== "H2") return;
    for (var node = heading.nextElementSibling; node && node.tagName !== "H2"; node = node.nextElementSibling) {
      if (!node.querySelector) continue;
      var table = node.classList && node.classList.contains("table-wrapper")
        ? node.querySelector("table") : null;
      if (!hasHeaders(table, ISSUE_HEADERS) || table.getAttribute("data-as-cards")) continue;
      var cards = renderCards(table, ISSUE_HEADERS, "issue", "Technical issue register");
      node.parentNode.insertBefore(cards, node.nextSibling);
      table.setAttribute("data-as-cards", "true");
      node.hidden = true;
    }
  }

  function setSummaryCards() {
    var heading = document.getElementById && document.getElementById("runtime-sets-and-authored-motion");
    if (!heading || heading.tagName !== "H2") return;
    for (var node = heading.nextElementSibling; node && node.tagName !== "H2"; node = node.nextElementSibling) {
      var table = node.classList && node.classList.contains("table-wrapper")
        ? node.querySelector("table") : null;
      if (!hasHeaders(table, SET_HEADERS) || table.getAttribute("data-as-set-cards")) continue;
      var cards = renderCards(table, SET_HEADERS, "set", "Runtime set decisions");
      node.parentNode.insertBefore(cards, node.nextSibling);
      node.className += " as-set-summary-source";
      table.setAttribute("data-as-set-cards", "true");
    }
  }

  function tableLabel(wrapper) {
    var previous = wrapper.previousElementSibling;
    while (previous && !/^H[1-6]$/.test(previous.tagName)) previous = previous.previousElementSibling;
    return (previous ? previous.textContent.trim() : "Data") + " table";
  }

  function updateOverflow(wrapper, hint) {
    var overflow = wrapper.scrollWidth > wrapper.clientWidth + 1;
    hint.hidden = !overflow;
    if (overflow) {
      wrapper.setAttribute("tabindex", "0");
      wrapper.setAttribute("role", "region");
      wrapper.setAttribute("aria-label", tableLabel(wrapper) + ", scroll horizontally");
    } else {
      wrapper.removeAttribute("tabindex");
      wrapper.removeAttribute("role");
      wrapper.removeAttribute("aria-label");
    }
  }

  function enhanceTables() {
    issueCards();
    setSummaryCards();
    if (!document.querySelectorAll) return;
    var wrappers = document.querySelectorAll(".content .table-wrapper");
    for (var i = 0; i < wrappers.length; i += 1) {
      var wrapper = wrappers[i];
      if (wrapper.hidden || wrapper.getAttribute("data-as-overflow")) continue;
      var hint = document.createElement("p");
      hint.className = "as-table-hint";
      if (wrapper.classList.contains("as-set-summary-source")) hint.className += " as-set-table-hint";
      hint.textContent = "Scroll horizontally to see the rest of this table. Focus the table and use the arrow keys.";
      wrapper.parentNode.insertBefore(hint, wrapper);
      wrapper.setAttribute("data-as-overflow", "true");
      wrapper.addEventListener("keydown", function (event) {
        if (event.target !== this || (event.key !== "ArrowRight" && event.key !== "ArrowLeft")
            || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey
            || this.scrollWidth <= this.clientWidth + 1) return;
        // mdBook uses these keys for chapter navigation at document level.
        // Handle them here so a focused table can move without leaving it.
        event.preventDefault();
        event.stopPropagation();
        var step = Math.max(40, Math.floor(this.clientWidth / 4));
        this.scrollLeft += event.key === "ArrowRight" ? step : -step;
      });
      updateOverflow(wrapper, hint);
      if (typeof ResizeObserver === "function") {
        new ResizeObserver((function (region, cue) {
          return function () { updateOverflow(region, cue); };
        })(wrapper, hint)).observe(wrapper);
      } else if (typeof window !== "undefined") {
        window.addEventListener("resize", (function (region, cue) {
          return function () { updateOverflow(region, cue); };
        })(wrapper, hint));
      }
    }
  }

  function start() {
    addHome();
    apply();
    enhanceTables();
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
