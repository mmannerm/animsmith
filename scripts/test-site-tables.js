"use strict";
// Behavioral contract for the report issue cards and accessible table overflow.
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");

const source = fs.readFileSync(path.join(__dirname, "..", "docs/site/animsmith.js"), "utf8");

class Element {
  constructor(tag) {
    this.tagName = tag.toUpperCase();
    this.children = [];
    this.attributes = {};
    this.className = "";
    this.hidden = false;
    this.ownText = "";
    this.listeners = {};
  }
  get firstChild() { return this.children[0] || null; }
  get nextSibling() {
    if (!this.parentNode) return null;
    return this.parentNode.children[this.parentNode.children.indexOf(this) + 1] || null;
  }
  get nextElementSibling() { return this.nextSibling; }
  get previousElementSibling() {
    if (!this.parentNode) return null;
    return this.parentNode.children[this.parentNode.children.indexOf(this) - 1] || null;
  }
  get textContent() { return this.ownText + this.children.map((child) => child.textContent).join(""); }
  set textContent(value) {
    this.ownText = this.tagName === "#TEXT" ? value : "";
    this.children = [];
    if (value && this.tagName !== "#TEXT") {
      const text = new Element("#text"); text.textContent = value; this.appendChild(text);
    }
  }
  appendChild(child) {
    if (child.parentNode) child.parentNode.removeChild(child);
    child.parentNode = this;
    this.children.push(child);
    return child;
  }
  removeChild(child) { this.children.splice(this.children.indexOf(child), 1); child.parentNode = null; }
  insertBefore(child, before) {
    if (child.parentNode) child.parentNode.removeChild(child);
    child.parentNode = this;
    const index = this.children.indexOf(before);
    this.children.splice(index < 0 ? this.children.length : index, 0, child);
    return child;
  }
  setAttribute(key, value) { this.attributes[key] = String(value); }
  getAttribute(key) { return this.attributes[key] || null; }
  removeAttribute(key) { delete this.attributes[key]; }
  addEventListener(key, callback) { (this.listeners[key] ||= []).push(callback); }
  get classList() { return { contains: (name) => this.className.split(/\s+/).includes(name) }; }
  querySelector(tag) { return walk(this).find((node) => node.tagName === tag.toUpperCase()) || null; }
  cloneNode(deep) {
    const clone = new Element(this.tagName);
    clone.ownText = this.ownText;
    clone.className = this.className;
    clone.attributes = { ...this.attributes };
    if (deep) this.children.forEach((child) => clone.appendChild(child.cloneNode(true)));
    return clone;
  }
}
function walk(node) { return [node, ...node.children.flatMap(walk)]; }
function el(tag, text = "") { const node = new Element(tag); node.textContent = text; return node; }
function table(headers, rows) {
  const wrapper = el("div"); wrapper.className = "table-wrapper";
  const node = el("table");
  const head = el("thead"), body = el("tbody"), headerRow = el("tr");
  headerRow.cells = headers.map((name) => headerRow.appendChild(el("th", name)));
  const bodyRows = rows.map((values) => {
    const row = el("tr");
    row.cells = values.map((value) => {
      const cell = el("td");
      if (Array.isArray(value)) value.forEach((child) => cell.appendChild(child));
      else cell.textContent = value;
      return row.appendChild(cell);
    });
    return row;
  });
  head.rows = [headerRow]; body.rows = bodyRows;
  head.appendChild(headerRow); bodyRows.forEach((row) => body.appendChild(row));
  node.tHead = head; node.tBodies = [body];
  node.appendChild(head); node.appendChild(body); wrapper.appendChild(node);
  wrapper.scrollWidth = 1200; wrapper.clientWidth = 500;
  return { wrapper, node };
}
const headers = ["ID", "Severity", "Problem and impact", "Primary owner", "Current action", "Future AnimSmith potential", "Evidence/status"];
const root = el("div");
const setHeading = el("h2", "Runtime sets and authored motion");
setHeading.setAttribute("id", "runtime-sets-and-authored-motion"); root.appendChild(setHeading);
const setLink = el("a", "Exact files"); setLink.setAttribute("href", "set-evidence.md#exact-runtime-members");
const secondSetLink = el("a", "Second set files"); secondSetLink.setAttribute("href", "other-evidence.md#exact-runtime-members");
const setHeaders = ["Set", "Controller use", "Adoption decision", "Exact members"];
const set = table(setHeaders, [
  ["run-8-way", "Full-body locomotion", "Test foot contacts", [setLink]],
  ["idle-to-run", "Starting movement", "Check the transition", [secondSetLink]],
]);
root.appendChild(set.wrapper);
const heading = el("h2", "Technical issue register");
heading.setAttribute("id", "technical-issue-register"); root.appendChild(heading);
const guidance = el("a", "Readiness guidance"); guidance.setAttribute("href", "../guide.html#readiness");
const secondGuidance = el("a", "Contact procedure"); secondGuidance.setAttribute("href", "../guide.html#contact");
const issue = table(headers, [
  ["ISSUE-1", "major", [el("span", "Reproduce with current config. "), guidance], "artist-author", "Fix and retest", "Possible tool work", "observed; unresolved"],
  ["ISSUE-2", "minor", [el("span", "Inspect contact. "), secondGuidance], "engine-config", "Test the target rig", "No generic fix", "untested; open"],
]);
root.appendChild(issue.wrapper);
const nextHeading = el("h2", "Engine status"); root.appendChild(nextHeading);
const unrelated = table(headers, [["OTHER", "minor", "Unrelated", "unknown", "none", "none", "none"]]);
root.appendChild(unrelated.wrapper);
const evidence = table(["File", "Measurement"], [["clip.fbx", "42"]]);
root.appendChild(evidence.wrapper);
const observed = [];
class ResizeObserver {
  constructor(callback) { this.callback = callback; }
  observe(node) { observed.push({ node, callback: this.callback }); }
}
const document = {
  readyState: "complete", documentElement: { className: "" },
  getElementById: (id) => walk(root).find((node) => node.getAttribute("id") === id) || null,
  querySelectorAll: (selector) => selector === ".content .table-wrapper"
    ? walk(root).filter((node) => node.classList.contains("table-wrapper")) : [],
  querySelector: () => null, createElement: (tag) => el(tag), getElementsByTagName: () => [],
};
const context = vm.createContext({ document, ResizeObserver, MutationObserver: undefined });
vm.runInContext(source, context);
function assertCardRows(tableNode, cardContainer, labels) {
  const rows = tableNode.tBodies[0].rows;
  assert.equal(cardContainer.children.length, rows.length);
  rows.forEach((row, rowIndex) => {
    const card = cardContainer.children[rowIndex];
    assert.equal(card.tagName, "ARTICLE");
    assert.equal(card.children[0].textContent, row.cells[0].textContent);
    const details = card.children[1].children;
    assert.equal(details.length, (labels.length - 1) * 2);
    labels.slice(1).forEach((label, columnIndex) => {
      assert.equal(details[columnIndex * 2].textContent, label);
      assert.equal(details[columnIndex * 2 + 1].textContent, row.cells[columnIndex + 1].textContent);
    });
    const links = (node) => walk(node).filter((item) => item.tagName === "A")
      .map((item) => [item.textContent, item.getAttribute("href")]);
    assert.deepEqual(links(card), links(row), "each row keeps its links in order");
  });
}
const cards = walk(root).filter((node) => node.className === "as-issue-cards");
assert.equal(cards.length, 1);
assert.equal(issue.wrapper.hidden, true);
assert.equal(issue.node.getAttribute("data-as-cards"), "true");
assertCardRows(issue.node, cards[0], headers);
const setCards = walk(root).filter((node) => node.className === "as-set-cards");
assert.equal(setCards.length, 1);
assert.equal(set.wrapper.hidden, false, "the semantic table remains the desktop and no-JS source");
assert.equal(set.node.getAttribute("data-as-set-cards"), "true");
assertCardRows(set.node, setCards[0], setHeaders);
assert.equal(unrelated.wrapper.hidden, false);
assert.equal(evidence.wrapper.hidden, false);
assert.equal(evidence.wrapper.getAttribute("tabindex"), "0");
assert.equal(evidence.wrapper.getAttribute("role"), "region");
assert.match(evidence.wrapper.getAttribute("aria-label"), /Engine status table, scroll horizontally/);
const hints = walk(root).filter((node) => node.className === "as-table-hint");
assert.equal(hints.length, 2, "only the unrelated and evidence tables need scroll hints");
for (const hint of hints) {
  assert.equal(hint.hidden, false, "overflow hints are visible");
  assert.match(hint.textContent, /scroll horizontally/i);
  assert.match(hint.textContent, /arrow keys/i);
}
assert.equal(observed.length, 3);
const setHint = walk(root).find((node) => node.className === "as-table-hint as-set-table-hint");
assert.ok(setHint, "the desktop table retains a cue when narrow");
function keyOn(wrapper, key, extras = {}) {
  const event = {
    key, target: wrapper, altKey: false, ctrlKey: false, metaKey: false, shiftKey: false,
    prevented: false, stopped: false,
    preventDefault() { this.prevented = true; },
    stopPropagation() { this.stopped = true; },
    ...extras,
  };
  for (const listener of wrapper.listeners.keydown || []) listener.call(wrapper, event);
  return event;
}
evidence.wrapper.scrollLeft = 0;
let key = keyOn(evidence.wrapper, "ArrowRight");
assert(key.prevented && key.stopped, "the focused region keeps mdBook from navigating away");
assert(evidence.wrapper.scrollLeft > 0, "Right moves the table");
const position = evidence.wrapper.scrollLeft;
key = keyOn(evidence.wrapper, "ArrowLeft");
assert(key.prevented && key.stopped);
assert(evidence.wrapper.scrollLeft < position, "Left moves the table back");
key = keyOn(evidence.wrapper, "ArrowRight", { ctrlKey: true });
assert(!key.prevented && !key.stopped, "modified shortcuts remain available");
key = keyOn(evidence.wrapper, "ArrowRight", { target: setLink });
assert(!key.prevented && !key.stopped, "focused descendants keep their own keyboard behavior");
// A narrower table loses both cue and extra keyboard stop; growing restores them.
evidence.wrapper.scrollWidth = 500;
observed.find((item) => item.node === evidence.wrapper).callback();
assert.equal(evidence.wrapper.getAttribute("tabindex"), null);
assert.equal(hints[1].hidden, true);
key = keyOn(evidence.wrapper, "ArrowRight");
assert(!key.prevented && !key.stopped, "a fitted table does not take arrow keys");
evidence.wrapper.scrollWidth = 900;
observed.find((item) => item.node === evidence.wrapper).callback();
assert.equal(evidence.wrapper.getAttribute("tabindex"), "0");
assert.equal(hints[1].hidden, false);
vm.runInContext(source, context);
assert.equal(walk(root).filter((node) => node.className === "as-issue-cards").length, 1);
assert.equal(walk(root).filter((node) => node.className === "as-set-cards").length, 1);
assert.equal(walk(root).filter((node) => node.className === "as-table-hint").length, 2);
assert.equal(observed.length, 3);
assert.equal(evidence.wrapper.listeners.keydown.length, 1);
console.log("site table contract passed");
