"use strict";
// Executes the documentation site's script (docs/site/animsmith.js) against a
// synthetic page, because both of its contracts are behaviour no source-text
// check can prove.
//
// The way home: the sidebar gains exactly one Home entry, at the very top,
// and the title in the top bar becomes a link — both resolving through
// `path_to_root`, so a page two directories down reaches the site root rather
// than its own directory, and neither is written twice when the script loads
// again.
//
// The theme bridge: only the `theme` pair may change, every other pair keeps
// its authored order, a stale `theme` is replaced rather than repeated, and a
// frame that is not a report is never touched.
//
// The DOM is deliberately thin — an <html> element with a class, a sidebar
// and title bar shaped like mdBook's, the frames a published page carries,
// and a MutationObserver that records what it was asked to watch — so
// everything asserted here is something a reader would see in the book.
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

// The page this runs against sits two directories down, exactly like
// docs/symptoms/loop-pops.html, so a link that forgets the prefix resolves
// somewhere else and the assertions below see it.
const PATH_TO_ROOT = "../../";
const MENU_TITLE = "AnimSmith documentation";

// A DOM thin enough to read. Elements know their tag, classes, attributes and
// children; `textContent` reads through the tree and replaces it when set,
// the way a browser's does; and the selector engine understands exactly the
// two shapes the script uses — a descendant pair and a tag with a class.
function element(tag, className) {
  return {
    tagName: tag.toUpperCase(),
    className: className || "",
    attrs: {},
    children: [],
    text: "",
    getAttribute(name) { return this.attrs[name]; },
    setAttribute(name, value) { this.attrs[name] = value; },
    appendChild(child) { this.children.push(child); return child; },
    insertBefore(child, before) {
      const at = this.children.indexOf(before);
      this.children.splice(at === -1 ? this.children.length : at, 0, child);
      return child;
    },
    querySelector(selector) { return select(this, selector); },
    get firstChild() { return this.children.length ? this.children[0] : null; },
    get textContent() {
      return this.children.length
        ? this.children.map((child) => child.textContent).join("")
        : this.text;
    },
    set textContent(value) { this.text = value; this.children = []; },
  };
}

function matches(node, compound) {
  const parts = compound.split(".");
  const tag = parts.shift();
  if (tag && node.tagName !== tag.toUpperCase()) return false;
  const classes = String(node.className || "").split(/\s+/);
  return parts.every((name) => classes.indexOf(name) !== -1);
}

function select(node, selector) {
  const compounds = selector.trim().split(/\s+/);
  const [head, ...rest] = compounds;
  for (const child of node.children) {
    if (matches(child, head)) {
      const hit = rest.length ? select(child, rest.join(" ")) : child;
      if (hit) return hit;
    }
    const deeper = select(child, selector);
    if (deeper) return deeper;
  }
  return null;
}

// mdBook's own chrome, in the shape toc.js and the page template leave it:
// a sidebar whose chapter list is already populated and whose hrefs already
// carry the prefix, and a title bar holding a plain heading.
function chrome() {
  const chapters = element("ol", "chapter");
  const first = element("li", "chapter-item");
  const link = element("a");
  link.setAttribute("href", PATH_TO_ROOT + "docs/why-animsmith.html");
  link.textContent = "Why animsmith";
  first.appendChild(link);
  chapters.appendChild(first);
  const scrollbox = element("mdbook-sidebar-scrollbox", "sidebar-scrollbox");
  scrollbox.appendChild(chapters);
  const sidebar = element("nav", "sidebar");
  sidebar.appendChild(scrollbox);
  const title = element("h1", "menu-title");
  title.textContent = MENU_TITLE;
  const bar = element("div", "menu-bar");
  bar.appendChild(title);
  const body = element("body");
  body.appendChild(sidebar);
  body.appendChild(bar);
  return { body, chapters, title };
}

function fragmentPairs(src) {
  const split = src.indexOf("#");
  return split === -1 ? [] : src.slice(split + 1).split("&").filter(Boolean);
}

function load(code, pathToRoot) {
  const root = { className: "" };
  const page = chrome();
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
      createElement: (tag) => element(tag),
      querySelector: (selector) => select(page.body, selector),
      addEventListener() { throw new Error("a complete document must not wait for DOMContentLoaded"); },
    },
    MutationObserver,
    String, RegExp, Object, Array, Math, console,
  };
  if (pathToRoot !== null) context.path_to_root = pathToRoot;
  vm.createContext(context);
  vm.runInContext(code, context);
  return { root, frames, observed, reached, page, code, context };
}

// Drive the bridge the way a browser does: change the class mdBook writes on
// <html>, then deliver the mutation the observer registered for.
function switchTheme(page, name) {
  page.root.className = `sidebar-visible ${name} js`;
  for (const watch of page.observed) watch.callback();
}

// Every anchor with the script's own class, anywhere under `node`.
function homeLinks(node) {
  let found = matches(node, "a.as-home") ? [node] : [];
  for (const child of node.children || []) found = found.concat(homeLinks(child));
  return found;
}

function assertHome(loaded) {
  const { chapters, title } = loaded.page;
  const expected = PATH_TO_ROOT + "index.html";

  const entries = homeLinks(chapters);
  if (entries.length !== 1) {
    throw new Error(`the sidebar must gain exactly one Home link, got ${entries.length}`);
  }
  const [entry] = entries;
  const item = chapters.firstChild;
  if (item.querySelector("a.as-home") !== entry) {
    throw new Error("the Home entry must be the sidebar's very first item");
  }
  // The stylesheet hangs the scrollbox's top padding and the sticky pin on
  // this class, so an item that loses it is an unpadded, scrolling list.
  if (!matches(item, "li.chapter-item.as-home-item")) {
    throw new Error(`the Home item must carry chapter-item as-home-item, got "${item.className}"`);
  }
  if (entry.getAttribute("href") !== expected) {
    throw new Error(`the Home entry must resolve to ${expected}, got ${entry.getAttribute("href")}`);
  }
  if (entry.textContent !== "Home") {
    throw new Error(`the Home entry must read Home, got ${entry.textContent}`);
  }

  const titles = homeLinks(title);
  if (titles.length !== 1) {
    throw new Error(`the title bar must gain exactly one link home, got ${titles.length}`);
  }
  if (titles[0].getAttribute("href") !== expected) {
    throw new Error(`the title link must resolve to ${expected}, got ${titles[0].getAttribute("href")}`);
  }
  if (title.textContent !== MENU_TITLE) {
    throw new Error(`the title must keep its own text, got ${title.textContent}`);
  }
}

function exercise(code) {
  const page = load(code, PATH_TO_ROOT);
  assertHome(page);
  // A second load must find its own work and leave it alone, rather than
  // stacking a second Home entry on the reader's sidebar.
  const reloaded = load(code, PATH_TO_ROOT);
  vm.runInContext(code, reloaded.context);
  assertHome(reloaded);

  // Without mdBook's prefix there is nothing to resolve against, so the
  // script writes no link at all rather than one pointing at the current
  // directory.
  const rootless = load(code, null);
  if (homeLinks(rootless.page.body).length !== 0) {
    throw new Error("a page without path_to_root must gain no guessed link home");
  }

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

// Every rule above is load-bearing: each mutation below breaks one of them,
// and the harness must refuse it.
const MUTATIONS = [
  // A link home that forgets mdBook's prefix reaches the current directory,
  // which on a symptom page is not the site root.
  [
    'return typeof path_to_root === "string" ? path_to_root + "index.html" : null;',
    'return "index.html";',
  ],
  // Appended instead of prepended, Home lands under the last part rather
  // than above the first.
  ["chapters.insertBefore(item, chapters.firstChild);", "chapters.appendChild(item);"],
  // Without the guard a second load stacks a second Home entry.
  ['if (chapters && !chapters.querySelector("a.as-home")) {', "if (chapters) {"],
  // The stylesheet finds the row by this class; without it the sidebar loses
  // its top padding and Home scrolls away with the list.
  ['item.className = "chapter-item as-home-item";', 'item.className = "chapter-item";'],
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

console.log("site script contract passed");
