"use strict";
// Executes the documentation site's theme bridge (docs/site/animsmith.js)
// against a synthetic page, because its contract is a rewrite rule no
// source-text check can prove: only the `theme` pair may change, every other
// pair keeps its authored order, a stale `theme` is replaced rather than
// repeated, and a frame that is not a report is never touched.
//
// The DOM is deliberately thin — an <html> element with a class, four
// iframes, and a MutationObserver that records what it was asked to watch —
// so everything asserted here is something a reader would see when they pick
// a theme in the book.
const fs = require("fs"), path = require("path"), vm = require("vm");

const BRIDGE = path.join(__dirname, "..", "docs", "site", "animsmith.js");
const source = fs.readFileSync(BRIDGE, "utf8");

// Every frame shape a published page can carry: the relative spelling the
// repository Markdown is written in, the site-absolute spelling staging
// rewrites it to for the released root (already carrying a stale theme,
// mid-fragment), a report with no fragment at all, the spelling staging
// writes for the `/dev/` subtree, mdBook's own table-of-contents frame, and
// a path that would fool a substring test for `visuals/`.
const FRAMES = [
  { name: "relative report", src: "../visuals/x.report.html#embed=1&finding=0", report: true },
  {
    name: "site-absolute report with a stale theme",
    src: "/animsmith/docs/visuals/x.report.html#finding=3&embed=1&theme=light",
    report: true,
  },
  { name: "report with no fragment", src: "../visuals/y.report.html", report: true },
  {
    name: "site-absolute report in the development subtree",
    src: "/animsmith/dev/docs/visuals/x.report.html#embed=1",
    report: true,
  },
  { name: "the book's own sidebar frame", src: "../../toc.html", report: false },
  // `visuals/` must be a whole path segment: a substring test would take
  // this one for a report and rewrite a page the site does not own.
  {
    name: "a directory merely ending in visuals",
    src: "../../previsuals/x.report.html",
    report: false,
  },
];

// What each of mdBook's five themes must pin, and the order a reader switches
// through in this run.
const THEMES = { light: "light", rust: "light", navy: "dark", coal: "dark", ayu: "dark" };
const SWITCHES = ["light", "navy", "coal"];

function fragmentPairs(src) {
  const split = src.indexOf("#");
  return split === -1 ? [] : src.slice(split + 1).split("&").filter(Boolean);
}

function load(code) {
  const root = { className: "" };
  const frames = FRAMES.map((frame) => ({
    ...frame,
    attrs: { src: frame.src },
    writes: 0,
    getAttribute(name) { return this.attrs[name]; },
    setAttribute(name, value) { this.attrs[name] = value; this.writes++; },
  }));
  const observed = [];
  class MutationObserver {
    constructor(callback) { this.callback = callback; }
    observe(target, options) { observed.push({ target, options, callback: this.callback }); }
  }
  const reached = [];
  const forbidden = (name) => () => {
    reached.push(name);
    throw new Error(`the theme bridge reached ${name}`);
  };
  const context = {
    fetch: forbidden("fetch"),
    XMLHttpRequest: forbidden("XMLHttpRequest"),
    WebSocket: forbidden("WebSocket"),
    navigator: { sendBeacon: forbidden("navigator.sendBeacon") },
    localStorage: {
      getItem: forbidden("localStorage.getItem"),
      setItem: forbidden("localStorage.setItem"),
    },
    document: {
      documentElement: root,
      readyState: "complete",
      getElementsByTagName: (tag) => (tag === "iframe" ? frames : []),
      addEventListener() { throw new Error("a complete document must not wait for DOMContentLoaded"); },
    },
    MutationObserver,
    String, RegExp, Object, Array, Math, console,
  };
  vm.createContext(context);
  vm.runInContext(code, context);
  return { root, frames, observed, reached };
}

// Drive the bridge the way a browser does: change the class mdBook writes on
// <html>, then deliver the mutation the observer registered for.
function switchTheme(page, name) {
  page.root.className = `sidebar-visible ${name} js`;
  for (const watch of page.observed) watch.callback();
}

function exercise(code) {
  const page = load(code);
  const [watch] = page.observed;
  if (page.observed.length !== 1 || watch.target !== page.root) {
    throw new Error("the bridge must watch the <html> element exactly once");
  }
  if (!watch.options || watch.options.attributes !== true
      || String(watch.options.attributeFilter) !== "class") {
    throw new Error("the bridge must observe the class attribute, and only it");
  }

  for (const name of SWITCHES) {
    const expected = THEMES[name];
    switchTheme(page, name);
    for (let index = 0; index < page.frames.length; index += 1) {
      const frame = page.frames[index], original = FRAMES[index];
      const src = frame.getAttribute("src");
      if (!original.report) {
        if (src !== original.src) throw new Error(`${original.name} was rewritten to ${src}`);
        continue;
      }

      const [pathPart, ...rest] = src.split("#");
      if (pathPart !== original.src.split("#")[0]) {
        throw new Error(`${original.name}: the path changed to ${pathPart}`);
      }
      if (rest.length > 1) throw new Error(`${original.name}: more than one fragment in ${src}`);

      const pairs = fragmentPairs(src);
      const themes = pairs.filter((pair) => pair.startsWith("theme="));
      if (themes.length !== 1 || themes[0] !== `theme=${expected}`) {
        throw new Error(`${original.name}: ${name} must pin exactly theme=${expected}, got ${src}`);
      }
      const others = pairs.filter((pair) => !pair.startsWith("theme="));
      const authored = fragmentPairs(original.src).filter((pair) => !pair.startsWith("theme="));
      if (String(others) !== String(authored)) {
        throw new Error(`${original.name}: ${String(authored)} became ${String(others)}`);
      }
      if (authored.length === 0 && src !== `${original.src}#theme=${expected}`) {
        throw new Error(`${original.name}: a frame with no fragment gained more than the theme: ${src}`);
      }
    }
  }

  // The bridge writes one attribute and reads nothing else: no network
  // global, and no per-viewer storage, is reachable from it.
  if (page.reached.length !== 0) {
    throw new Error(`the theme bridge used ${page.reached.join(", ")}`);
  }

  // Re-delivering the same theme must write nothing: assigning an unchanged
  // src would re-navigate every embedded report on every observed mutation.
  const before = page.frames.map((frame) => frame.writes);
  switchTheme(page, SWITCHES[SWITCHES.length - 1]);
  if (String(page.frames.map((frame) => frame.writes)) !== String(before)) {
    throw new Error("an unchanged theme still rewrote a frame source");
  }
}

exercise(source);

// Every rewrite rule above is load-bearing: each mutation below breaks one of
// them, and the harness must refuse it.
const MUTATIONS = [
  // Codex's case: keeping only the theme drops the reader's own deep link.
  ['return path + "#" + pairs.join("&");', 'return path + "#theme=" + value;'],
  // Keeping the old theme pair leaves two of them, and the report reads the first.
  ['if (key !== THEME_KEY && pair) pairs.push(pair);', 'if (pair) pairs.push(pair);'],
  // A swapped mapping pins the opposite of what the page shows.
  ['var DARK = ["navy", "coal", "ayu"];', 'var DARK = ["light", "rust"];'],
  // A selector that matches everything rewrites the book's own frames.
  ['return /\\.html$/.test(path) && /(^|\\/)visuals\\//.test(path);', 'return true;'],
  // A substring test would match a directory merely ending in `visuals`.
  ['/(^|\\/)visuals\\//.test(path)', 'path.indexOf("visuals/") !== -1'],
  // Watching every attribute makes the page rewrite its frames on any change.
  ['attributeFilter: ["class"],', ''],
];
for (const [from, to] of MUTATIONS) {
  if (!source.includes(from)) throw new Error(`the bridge no longer contains ${from}`);
  let refused = false;
  try { exercise(source.replace(from, to)); } catch (_) { refused = true; }
  if (!refused) throw new Error(`the theme-bridge contract accepted the mutation ${from} -> ${to}`);
}

console.log("theme bridge contract passed");
