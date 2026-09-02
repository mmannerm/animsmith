"use strict";
// Executes the Rust-generated report viewers headlessly against the exact
// documents `just report-browser` produced: both report forms, each in its
// full and evidence-only shape. The DOM and WebGL stubs are deliberately
// thin — everything asserted here is something a reader would see.
const fs = require("fs"), vm = require("vm");
if (process.argv.length !== 7) {
  throw new Error("usage: test-report-viewers.js COMPARISON.html COMPARISON-EVIDENCE.html REPORT.html REPORT-EVIDENCE.html REPORT-MULTI-CLIP.html");
}
const [, , comparisonPath, comparisonEvidencePath, singlePath, singleEvidencePath, multiPath] = process.argv;
const html = fs.readFileSync(comparisonPath, "utf8");
const comparisonEvidenceHtml = fs.readFileSync(comparisonEvidencePath, "utf8");
const singleHtml = fs.readFileSync(singlePath, "utf8");
const singleEvidenceHtml = fs.readFileSync(singleEvidencePath, "utf8");
const multiHtml = fs.readFileSync(multiPath, "utf8");

function generatedReportParts(source) {
  const match = source.match(/<script>([\s\S]*?)<\/script><script type="application\/json" id="comparison-report-data">([\s\S]*?)<\/script><script>([\s\S]*?)<\/script><\/body><\/html>\s*$/);
  if (!match) throw new Error("Rust-generated payload and immediately following inline viewer are absent");
  if (!match[1].startsWith("// animsmith report shared runtime")) throw new Error("wrong inline shared runtime");
  if (!match[3].startsWith("// animsmith comparison viewer:")) throw new Error("wrong inline comparison viewer");
  return { shared: match[1], payload: match[2], viewer: match[3] };
}
const generated = generatedReportParts(html);
for (const mutation of [
  html.replace("</script><script>// animsmith comparison viewer:", "</script><script>// misplaced</script><script>// animsmith comparison viewer:"),
  html.replace("// animsmith comparison viewer:", "// wrong viewer:"),
  html.replace("// animsmith report shared runtime", "// wrong shared runtime"),
  html.replace(/<\/script><\/body><\/html>\s*$/, "</body></html>"),
]) {
  let refused = false;
  try { generatedReportParts(mutation); } catch (_) { refused = true; }
  if (!refused) throw new Error("generated HTML viewer placement/identity mutation was accepted");
}
const data = JSON.parse(generated.payload), viewer = `${generated.shared}\n${generated.viewer}`;
// Every colour resolves through the design tokens. A run with no stylesheet
// exercises the documented dark fallbacks; a run with a stub stylesheet
// proves the viewers really read the tokens.
const noStyles = { getPropertyValue: () => "" };
const tokenStyles = (values) => ({ getPropertyValue: (name) => values[name.replace(/^--/, "")] || "" });
if (data.kind !== "animsmith-comparison-v1") throw new Error("unexpected Rust comparison contract");
const frames = 2002, bones = data.bones.length, positions = Buffer.alloc(frames * bones * 3 * 4);
for (let frame = 0; frame < frames; frame++) for (let bone = 0; bone < bones; bone++) {
  const base = (frame * bones + bone) * 3;
  positions.writeFloatLE(frame / 1000 + bone, base * 4);
  positions.writeFloatLE(bone, (base + 1) * 4);
  positions.writeFloatLE((frame % 11) / 100, (base + 2) * 4);
}

class Node {
  constructor(id) { this.id=id; this.tag=null; this.children=[]; this.style={}; this.attrs={}; this.listeners={}; this.classes=new Set(); this.dataset={}; this.query={}; this.clientWidth=360; this.clientHeight=270; this.value="0"; this.textContent=""; }
  append(x){this.children.push(x)} appendChild(x){this.children.push(x); return x} replaceChildren(){this.children=[]}
  addEventListener(k,f){this.listeners[k]=f}
  setAttribute(k,v){this.attrs[k]=v} removeAttribute(k){delete this.attrs[k]}
  querySelector(selector){return this.query[selector]||null}
  get classList(){const c=this.classes;return {add:x=>c.add(x),remove:x=>c.delete(x),contains:x=>c.has(x)}}
  getContext(kind){
    if (kind === "webgl2") { if (!this.gl) this.gl=webgl(); return this.gl; }
    if (!this.context) this.context={arcs:[],strokes:[],fillStyle:null,strokeStyle:null,setTransform(){},clearRect(){this.arcs=[];this.strokes=[]},beginPath(){},moveTo(){},lineTo(){},stroke(){this.strokes.push(this.strokeStyle)},arc(...args){this.arcs.push({args,fillStyle:this.fillStyle})},fill(){}};
    return this.context;
  }
  scrollIntoView(){this.scrolled=true}
}
// Enough WebGL2 to run the hand-written skeleton renderer headlessly: every
// call is recorded so the harness can assert what was actually drawn.
function webgl() {
  const gl = {VERTEX_SHADER:1,FRAGMENT_SHADER:2,COMPILE_STATUS:3,ARRAY_BUFFER:4,FLOAT:5,DEPTH_TEST:6,COLOR_BUFFER_BIT:7,DEPTH_BUFFER_BIT:8,LINES:9,POINTS:10,LINE_STRIP:11,
    clears:[],draws:[],buffers:[],
    createShader:()=>({}),shaderSource(){},compileShader(){},getShaderParameter:()=>true,getShaderInfoLog:()=>"",
    createProgram:()=>({}),attachShader(){},linkProgram(){},useProgram(){},getUniformLocation:()=>({}),
    createBuffer:()=>({}),bindBuffer(){},enableVertexAttribArray(){},vertexAttribPointer(){},enable(){},
    viewport(){},clear(){},uniformMatrix4fv(){},uniform1f(){},
    clearColor(...args){this.clears.push(args)},
    bufferData(_target,array){this.buffers.push(Array.from(array))},
    drawArrays(mode,first,count){this.draws.push({mode,first,count})}};
  return gl;
}

// Every id the emitted markup carries. The synthetic DOM is built from this
// rather than from a list the harness keeps, so a document that stops
// rendering a surface cannot be executed against a manufactured node.
function documentIds(html) {
  return new Set([...html.matchAll(/\sid="([^"]+)"/g)].map((match) => match[1]));
}

// The charts the single-clip viewer syncs, built from the document's own
// <figure> blocks: real data-* hooks, and the child elements the playhead and
// path dot are moved through.
function documentCharts(html) {
  return [...html.matchAll(/<figure class="chart"[\s\S]*?<\/figure>/g)].map(([figure]) => {
    const node = new Node("chart");
    for (const [, name, value] of figure.matchAll(/data-([a-z]+)="([^"]*)"/g)) node.dataset[name] = value;
    for (const name of ["playhead", "pathdot", "pathpoints"]) {
      if (!figure.includes(`class="${name}"`)) continue;
      const child = new Node(name);
      if (name === "pathpoints") child.innerHTML = figure.split(`class="pathpoints">`)[1].split("<")[0];
      node.query[`.${name}`] = child;
    }
    return node;
  });
}

// One runner for every viewer execution: a fresh document whose elements are
// exactly the ones the generated markup carries, so an absent pose surface is
// absent here too. A query for an id this document deliberately omits — named
// by `omitted`, which is how an evidence-only document drops its pose
// surfaces — returns null, the way a browser does. Any other absent id fails
// the run, so a document that stops rendering a surface cannot be executed
// against a node the harness manufactured for it.
function run(parts, dataId, html, payload, options) {
  const settings = options || {};
  const present = documentIds(html);
  const omitted = new Set(settings.omitted || []);
  const nodes = {};
  for (const id of present) nodes[id] = new Node(id);
  if (!nodes[dataId]) throw new Error(`the generated document carries no #${dataId} payload`);
  nodes[dataId].textContent = JSON.stringify(payload);
  const listeners = {};
  const media = {};
  const root = new Node("documentElement");
  const charts = documentCharts(html);
  // The fragment is read, never written: every assignment to location.hash
  // is counted so a viewer that rewrites the URL cannot pass unnoticed.
  const hash = { value: settings.hash || "", writes: 0 };
  // A frame loop the harness drives: `requestAnimationFrame` records the
  // callback instead of scheduling it, so a viewer that animates does
  // nothing until a test asks for the next frame — and a test that asks can
  // observe exactly what one frame changed.
  const clock = { now: 0, pending: [] };
  const location = {
    get hash() { return hash.value; },
    set hash(next) { hash.writes++; hash.value = next; },
  };
  const context = {
    document: {
      documentElement: root,
      getElementById: (id) => {
        if (nodes[id]) return nodes[id];
        if (omitted.has(id)) return null;
        throw new Error(`the viewer queried #${id}, which this generated document does not define`);
      },
      createElement: () => new Node(),
      createTextNode: text => { const node = new Node(); node.textContent = text; return node; },
      createElementNS: (_ns, tag) => { const node = new Node(); node.tag = tag; return node; },
      querySelectorAll: (selector) => {
        if (selector !== ".chart") throw new Error(`the viewer queried an unexpected selector ${selector}`);
        return charts;
      },
    },
    window: { addEventListener(kind, handler) { listeners[kind] = handler; }, devicePixelRatio: 1 },
    location,
    getComputedStyle: () => settings.styles || noStyles,
    matchMedia: () => ({ matches: false, addEventListener(kind, handler) { media[kind] = handler; }, removeEventListener() {} }),
    performance: { now: () => clock.now },
    requestAnimationFrame: (callback) => clock.pending.push(callback),
    atob: value => Buffer.from(value, "base64").toString("binary"),
    Uint8Array, Float32Array, Buffer, Math, Map, Set, Array, Number, Object, Infinity, JSON, console,
  };
  vm.createContext(context);
  vm.runInContext(`${parts.shared}\n${parts.viewer}`, context);
  return { nodes, root, listeners, context, charts, hash, media, settings, clock };
}

// One animation frame, `seconds` after the last one.
function stepFrame(state, seconds) {
  state.clock.now += seconds * 1000;
  const pending = state.clock.pending.splice(0, state.clock.pending.length);
  for (const callback of pending) callback(state.clock.now);
  return pending.length;
}

// SVG shows no text unless an element carries it, so no panel may explain
// itself by assigning to an <svg>'s own textContent.
function assertNoBareSvgText(nodes, svgIds, why) {
  for (const id of svgIds) {
    const node = nodes[id];
    if (node && node.textContent !== "") throw new Error(`${why}: ${id} carries a bare text node instead of a <text> element`);
  }
}

const comparisonSvgs = ["comparison-root-path","before-path","after-path","before-gait","after-gait"];

for (const side of [data.before, data.after]) {
  side.clip.frames = frames;
  side.clip.duration = 2.001;
  side.clip.times = Array.from({length:frames}, (_, index) => index / 1000);
  side.clip.positions = positions.toString("base64");
}
const seam = data.before.contexts.seams.find(row => row.check === "loop-closure");
const structural = data.before.contexts.structural.find(row => row.check === "constant-track");
if (!seam || !structural) throw new Error("Rust-generated visual matrix contexts are incomplete");
seam.last_frame = 2001; seam.last_s = 2.001;
const stance = data.before.contexts.stances[0];
if (!stance) throw new Error("Rust-generated stance context is absent");
stance.runs = [{start_frame:1000,end_frame:1600,start_s:1,end_s:1.6}];
const seamFinding = data.before.findings.find(row => row.anchor === seam.finding_anchor);
const structuralFinding = data.before.findings.find(row => row.anchor === structural.finding_anchor);
if (!seamFinding || !structuralFinding) throw new Error("Rust-generated context/finding binding is incomplete");
seamFinding.time = 1.501; seamFinding.message = "<img>";
structuralFinding.time = 1.501;
let afterFinding = data.after.findings[0];
if (!afterFinding) {
  afterFinding = {...seamFinding, anchor: "finding-1111111111111111"};
  data.after.findings.push(afterFinding);
}
afterFinding.time = 1.234;

const main = run(generated, "comparison-report-data", html, data);
const nodes = main.nodes, windowListeners = main.listeners, documentElement = main.root, context = main.context;
const palette = context.animsmithPalette();
if(nodes.scrub.max !== 2001 || !nodes["before-findings"].children.some(child=>child.textContent.includes("<img>"))) throw new Error("viewer did not retain exact frames or safe finding text");
if(!nodes["before-identity"].textContent.includes(data.before.dependency_closure_identity.sha256) || !nodes["after-identity"].textContent.includes(data.after.dependency_closure_identity.sha256)) throw new Error("viewer does not disclose complete closure identities");
if(!nodes.times.textContent.includes("before 0.000s") || !nodes.times.textContent.includes("after 0.000s") || !nodes.times.textContent.includes("not a time warp")) throw new Error("shared phase omits source times or no-warp disclosure");
if(!nodes["comparison-root-path"].children.some(child=>child.textContent==="before root path") || !nodes["comparison-root-path"].children.some(child=>child.textContent==="after root path")) throw new Error("shared root chart lacks textual before/after legends");
if(!nodes["before-path"].children.some(child=>child.attrs["data-role"]==="left_foot") || !nodes["before-path"].children.some(child=>child.attrs["data-role"]==="right_foot")) throw new Error("role trail chart omits foot trajectories");
if(!nodes["before-gait"].children.some(child=>child.attrs["data-stance-side"]==="left")) throw new Error("gait chart omits typed stance interval");
assertNoBareSvgText(nodes, comparisonSvgs, "drawn comparison");

// Every panel says what it is. These captions are the only thing telling a
// reader that the trajectory panel covers the whole clip while the dot and
// the pose panes show one shared phase, and which shaded band belongs to
// which foot, so they are part of the document's contract rather than
// decoration.
// Captions live in the HTML paragraph beside each panel, so the browser
// reflows them at whatever width the reader has and this reads them the
// same way a reader does — off the element, not off the drawing.
const panelCaption = (id) => nodes[`${id}-caption`].textContent;
for (const [id, phrase] of [
  // What a reader should look for, in the words of the clip rather than of
  // the check that judged it. Each panel's sentence stays on what its own
  // drawing shows: the gait panel shades stance intervals and says so, the
  // trajectory panels do not and do not claim to.
  ["comparison-root-path", "what to look for: an in-place or looping clip's root path should close on itself and stay near the origin; a travelling clip should trace a straight line ending at the declared distance; the dot is the shared phase, the hollow circle where a track starts and the square where it ends"],
  ["comparison-root-path", "before solid, after dashed and translucent"],
  ["comparison-root-path", "drawn at the same metres scale as the role trajectory panels"],
  ["before-path", "what to look for: top-down trails of root, hips and feet over the whole clip; matching trails on both sides mean the repair changed only what it claims"],
  ["after-path", "what to look for: top-down trails of root, hips and feet over the whole clip; matching trails on both sides mean the repair changed only what it claims"],
  ["before-path", "shared scale across both inputs"],
  ["after-path", "shared scale across both inputs"],
  ["before-gait", "what to look for: the two feet should alternate, one planted flat at contact height while the other swings; the shaded bands are the sampled stance intervals the foot-slide check judged, and a foot that moves horizontally during its band is the slide; for a loop the curves should end where they began"],
  ["after-gait", "what to look for: the two feet should alternate, one planted flat at contact height while the other swings; the shaded bands are the sampled stance intervals the foot-slide check judged, and a foot that moves horizontally during its band is the slide; for a loop the curves should end where they began"],
  ["before-gait", "shaded runs are sampled foot-slide stance evidence"],
  ["before-gait", "left in the upper band, right in the lower"],
  ["after-gait", "left in the upper band, right in the lower"],
]) {
  if (!panelCaption(id).includes(phrase)) throw new Error(`${id} lost its caption: ${JSON.stringify(phrase)}`);
}
// The document presents checked evidence; a caption that told a reader the
// clip was fine would contradict the disclosure two sections above it.
for (const id of comparisonSvgs) {
  for (const word of ["acceptable", "looks good", "approved", "quality"]) {
    if (panelCaption(id).toLowerCase().includes(word)) throw new Error(`${id} promises acceptance with ${JSON.stringify(word)}`);
  }
}
if (!html.includes("An absent finding is not artistic, gameplay, or engine acceptance.")) throw new Error("the comparison dropped its evidence disclaimer");
// Nothing drawn inside a panel may be a caption: SVG does not wrap, so a
// sentence drawn there is cut at the panel edge on a narrow column.
for (const id of comparisonSvgs) {
  for (const child of nodes[id].children) {
    if (child.tag !== "text" || !child.textContent) continue;
    if (child.textContent.split(" ").length > 6) throw new Error(`${id} draws the sentence ${JSON.stringify(child.textContent)} into the picture, where a narrow panel cuts it off; captions belong in the panel's caption element`);
  }
}
// Each pose pane carries its own heading, so a document that labelled only
// one of them fails on the side it left unlabelled.
for (const side of ["before", "after"]) {
  const panel = html.split(`id="${side}-panel"`)[1];
  if (!panel) throw new Error(`the generated document has no ${side} panel`);
  const untilNextPanel = panel.split("<section class=\"side\"")[0];
  if (!untilNextPanel.includes("<h3>Judged pose at the shared phase</h3>")) throw new Error(`the ${side} pose pane is unlabelled: its panel carries no "Judged pose at the shared phase" heading`);
}

// Two root trajectories that coincide stay two visible paths. Both sides
// here carry the same pose grid, so a solid `after` drawn over a solid
// `before` would leave the panel showing one input while its legend named
// two — which is what a repair that does not touch the root produces.
const rootPaths = nodes["comparison-root-path"].children.filter((child) => child.attrs["data-root-side"]);
if (rootPaths.length !== 2) throw new Error("the shared root chart plots both sides");
if (rootPaths.filter((path) => path.attrs["stroke-dasharray"]).length !== 1) throw new Error("two coincident root paths are drawn identically, so one hides the other");
const rootDots = nodes["comparison-root-path"].children.filter((child) => child.attrs["data-root-dot"]);
if (new Set(rootDots.map((dot) => dot.attrs.r)).size !== 2) throw new Error("two coincident shared-phase dots are drawn identically");

// A coincident `after` is also thinner and translucent, so the `before` it
// sits on stays visible through it rather than only around its gaps.
const rootPathOf = (state, side) => state.nodes["comparison-root-path"].children.find((child) => child.attrs["data-root-side"] === side);
const beforeStroke = rootPathOf(main, "before"), afterStroke = rootPathOf(main, "after");
if (!(Number(afterStroke.attrs.opacity) < Number(beforeStroke.attrs.opacity))) throw new Error(`the after track is opaque (${afterStroke.attrs.opacity}), so it hides a coincident before track`);
if (!(Number(afterStroke.attrs["stroke-width"]) < Number(beforeStroke.attrs["stroke-width"]))) throw new Error(`the after track is not drawn thinner than the before track: ${afterStroke.attrs["stroke-width"]} against ${beforeStroke.attrs["stroke-width"]}`);

// ---- the shared root panel is drawn at the role panels' metres scale ----
// It used to be fitted to its own extent, which drew a root that sways two
// centimetres exactly as large as feet that swing half a metre: the least of
// the comparison then read as the most. The expected geometry is recomputed
// here from the payload, so a viewer that goes back to fitting the panel
// fails on the numbers rather than on a spelling.
const decodeFloats = (encoded) => {
  const raw = Buffer.from(encoded, "base64"), out = new Array(raw.byteLength / 4);
  for (let index = 0; index < out.length; index++) out[index] = raw.readFloatLE(index * 4);
  return out;
};
function payloadTrail(payload, name, role) {
  const side = payload[name], bone = side.clip.trails[role];
  if (bone == null) return null;
  const positions = decodeFloats(side.clip.positions);
  return Array.from({length: side.clip.frames}, (_, frame) => {
    const base = (frame * payload.bones.length + bone) * 3;
    return [positions[base], positions[base + 2]];
  });
}
function payloadBounds(payload, roles) {
  const xs = [], zs = [];
  for (const name of ["before", "after"]) for (const role of roles) {
    const points = payloadTrail(payload, name, role);
    if (!points) continue;
    for (const point of points) { xs.push(point[0]); zs.push(point[1]); }
  }
  const range = (values) => {
    let min = Infinity, max = -Infinity;
    for (const value of values) if (Number.isFinite(value)) { min = Math.min(min, value); max = Math.max(max, value); }
    return min === Infinity ? null : [min, max];
  };
  const x = range(xs), z = range(zs);
  return x && z ? {x, z} : null;
}
const fittedScale = (bounds, width, height, pad) => Math.min(
  (width - 2 * pad) / Math.max(.001, bounds.x[1] - bounds.x[0]),
  (height - 2 * pad) / Math.max(.001, bounds.z[1] - bounds.z[0]),
);
const trailBounds = payloadBounds(data, ["root", "hips", "left_foot", "right_foot"]);
const rootBounds = payloadBounds(data, ["root"]);
if (!trailBounds || !rootBounds) throw new Error("the fixture must resolve a root and its role trails");
const roleScale = fittedScale(trailBounds, 360, 180, 24);
if (Math.abs(roleScale - fittedScale(rootBounds, 720, 180, 28)) < 1e-6) throw new Error("the fixture's root and role extents must differ, or the two scalings cannot be told apart");
const rootMap = (point) => [
  720 / 2 + (point[0] - (rootBounds.x[0] + rootBounds.x[1]) / 2) * roleScale,
  180 / 2 - (point[1] - (rootBounds.z[0] + rootBounds.z[1]) / 2) * roleScale,
];
const drawnPoints = (d) => d.split(/[ML]/).filter(Boolean).map((pair) => pair.split(",").map(Number));
const spanOf = (points, axis) => Math.max(...points.map((point) => point[axis])) - Math.min(...points.map((point) => point[axis]));
const beforeRootMetres = payloadTrail(data, "before", "root").filter((point) => point.every(Number.isFinite));
const beforeRootDrawn = drawnPoints(beforeStroke.attrs.d);
if (beforeRootDrawn.length !== beforeRootMetres.length) throw new Error("the root panel drew a different number of points than the clip has finite root frames");
for (const axis of [0, 1]) {
  const expected = spanOf(beforeRootMetres.map(rootMap), axis);
  const drawn = spanOf(beforeRootDrawn, axis);
  if (Math.abs(drawn - expected) > 1e-6) throw new Error(`the shared root panel is not drawn at the role panels' metres scale: ${["X", "Z"][axis]} spans ${drawn} plot units, and the role scale over that metre extent is ${expected}`);
}

// Both ends of both tracks are marked, at the coordinates the first and last
// sampled frames map to, and the start mark is hollow so the phase dot
// sitting on it at frame 0 is still its own thing.
for (const side of ["before", "after"]) {
  const metres = payloadTrail(data, side, "root").filter((point) => point.every(Number.isFinite));
  const marker = (kind) => nodes["comparison-root-path"].children.find((child) => child.attrs["data-root-marker"] === `${side}-${kind}`);
  const circle = marker("start"), square = marker("end");
  if (!circle || !square) throw new Error(`the ${side} root track is not marked at both ends`);
  if (circle.tag !== "circle" || square.tag !== "rect") throw new Error(`the ${side} track's two ends are not drawn as two different shapes`);
  if (circle.attrs.fill !== "none") throw new Error(`the ${side} start mark is filled, so the phase dot standing on it at frame 0 disappears into it`);
  // On a closed loop at frame 0 the start mark, the end mark and the phase
  // dot are one coordinate, so a ring no wider than the dot says nothing.
  const dot = nodes["comparison-root-path"].children.find((child) => child.attrs["data-root-dot"] === side);
  if (!(Number(circle.attrs.r) > Number(dot.attrs.r))) throw new Error(`the ${side} start ring (r ${circle.attrs.r}) does not stand outside its own phase dot (r ${dot.attrs.r})`);
  const drawnAfterDot = nodes["comparison-root-path"].children.indexOf(dot) < nodes["comparison-root-path"].children.indexOf(square);
  if (!drawnAfterDot) throw new Error(`the ${side} end mark is drawn under the phase dot, which hides it wherever a track closes on itself`);
  const start = rootMap(metres[0]), end = rootMap(metres[metres.length - 1]);
  if (Math.abs(circle.attrs.cx - start[0]) > 1e-6 || Math.abs(circle.attrs.cy - start[1]) > 1e-6) throw new Error(`the ${side} start mark is at ${circle.attrs.cx},${circle.attrs.cy} rather than the first sampled frame's ${start}`);
  if (Math.abs(square.attrs.x + square.attrs.width / 2 - end[0]) > 1e-6 || Math.abs(square.attrs.y + square.attrs.height / 2 - end[1]) > 1e-6) throw new Error(`the ${side} end mark is not centred on the last sampled frame's ${end}`);
}
for (const label of ["start", "end"]) {
  if (!nodes["comparison-root-path"].children.some((child) => child.tag === "text" && child.textContent === label)) throw new Error(`the shared root panel does not name its ${label} mark in the legend`);
}
// And draws each name beside a swatch of its own shape, so the words say what
// the two shapes in the picture are rather than only that there are two.
const legendSwatch = (tag) => nodes["comparison-root-path"].children.find((child) => child.tag === tag
  && !child.attrs["data-root-marker"] && !child.attrs["data-root-dot"] && !child.attrs["data-root-side"]);
if (!legendSwatch("circle") || legendSwatch("circle").attrs.fill !== "none") throw new Error("the legend names a start mark without drawing the hollow circle it means");
if (!legendSwatch("rect") || legendSwatch("rect").attrs.fill === "none") throw new Error("the legend names an end mark without drawing the filled square it means");

// The caption says what the reader would otherwise have to notice: that the
// two tracks are the same line, and where each one ends relative to its own
// start.
const rootCaption = () => panelCaption("comparison-root-path");
if (!rootCaption().includes("the before and after paths are identical")) throw new Error(`two identical root tracks are not declared identical: ${rootCaption()}`);
const beforeGap = Math.hypot(
  beforeRootMetres[beforeRootMetres.length - 1][0] - beforeRootMetres[0][0],
  beforeRootMetres[beforeRootMetres.length - 1][1] - beforeRootMetres[0][1],
);
if (!(beforeGap > 0.001)) throw new Error("the fixture's root track must not already close on itself");
if (!rootCaption().includes(`before ends ${beforeGap.toFixed(3)} m from its start`)) throw new Error(`the caption does not state the end-to-start distance: ${rootCaption()}`);
if (!rootCaption().includes(` m at their widest`)) throw new Error(`the caption does not state the measured extent: ${rootCaption()}`);

// A root track that returns to where it began says so instead of naming a
// distance of zero.
const closedRoots = JSON.parse(JSON.stringify(data));
for (const name of ["before", "after"]) {
  const side = closedRoots[name], bone = side.clip.trails.root;
  const buffer = Buffer.from(side.clip.positions, "base64");
  for (let frame = 0; frame < side.clip.frames; frame++) {
    const base = (frame * closedRoots.bones.length + bone) * 3;
    const angle = 2 * Math.PI * frame / (side.clip.frames - 1);
    buffer.writeFloatLE(Math.cos(angle) * 0.01, base * 4);
    buffer.writeFloatLE(Math.sin(angle) * 0.01, (base + 2) * 4);
  }
  side.clip.positions = buffer.toString("base64");
}
const closedRun = run(generated, "comparison-report-data", html, closedRoots);
const closedCaption = closedRun.nodes["comparison-root-path-caption"].textContent;
for (const side of ["before", "after"]) {
  if (!closedCaption.includes(`${side} closes on itself`)) throw new Error(`a closed root loop is not declared closed: ${closedCaption}`);
}
if (closedCaption.includes("from its start")) throw new Error(`a closed root loop still names a distance: ${closedCaption}`);

// ---- panel order: pose, root, trails, gait -----------------------------
// The judged poses are what the comparison is about, so they come first and
// the shared root panel follows them.
const panelAt = (id) => {
  const at = html.indexOf(`id="${id}"`);
  if (at < 0) throw new Error(`the generated comparison renders no #${id}`);
  return at;
};
if (Math.max(panelAt("before-gl"), panelAt("after-gl")) > panelAt("comparison-root-path")) throw new Error("the shared root panel is rendered before a judged pose pane");
if (panelAt("comparison-root-path") > Math.min(panelAt("before-path"), panelAt("after-path"))) throw new Error("the shared root panel is not rendered before the role trajectories");
if (Math.min(panelAt("before-path"), panelAt("after-path")) > Math.min(panelAt("before-gait"), panelAt("after-gait"))) throw new Error("the role trajectories are not rendered before the gait panels");
for (const side of ["before", "after"]) {
  if (panelAt(`${side}-path`) > panelAt(`${side}-gait`)) throw new Error(`${side}: its gait panel is rendered before its role trajectories`);
}

// ---- the shared phase plays --------------------------------------------
// One number drives both sides, so playing it animates the two clips
// together at the before clip's own duration. Nothing about it touches the
// fragment: `#frame=` stays a way in, never something the document writes.
const playback = run(generated, "comparison-report-data", html, data);
const playMax = Number(playback.nodes.scrub.max);
const poseArcs = (state, side) => JSON.stringify(state.nodes[`${side}-gl`].context.arcs);
if (!/<button id="play"[^>]*>▶<\/button>/.test(html)) throw new Error("the comparison does not open paused, with a play control the document itself labels");
const paused = {before: poseArcs(playback, "before"), after: poseArcs(playback, "after")};
playback.nodes.play.listeners.click();
if (playback.nodes.play.textContent !== "⏸") throw new Error("pressing play did not start the shared phase");
if (!stepFrame(playback, data.before.clip.duration / 4)) throw new Error("playing scheduled no animation frame");
const quarter = Number(playback.nodes.scrub.value);
if (Math.abs(quarter - playMax / 4) > 1) throw new Error(`the shared phase does not advance at the before clip's ${data.before.clip.duration}s duration: frame ${quarter} after a quarter of it, against ${playMax / 4}`);
for (const side of ["before", "after"]) {
  if (poseArcs(playback, side) === paused[side]) throw new Error(`playing the shared phase did not redraw the ${side} pose pane`);
}
// Past the end it loops rather than stopping there.
stepFrame(playback, data.before.clip.duration);
const looped = Number(playback.nodes.scrub.value);
if (!(looped >= 0 && looped <= quarter + 1)) throw new Error(`the shared phase did not loop at the end of the clip: frame ${looped} of ${playMax}`);
// Scrubbing takes the phase back, the way the single-clip viewer does.
playback.nodes.scrub.value = "10";
playback.nodes.scrub.listeners.input();
if (playback.nodes.play.textContent !== "▶") throw new Error("scrubbing did not pause the shared phase");
stepFrame(playback, data.before.clip.duration / 4);
if (Number(playback.nodes.scrub.value) !== 10) throw new Error("a paused comparison kept advancing the shared phase");
// And so does selecting a finding, which would otherwise be overwritten by
// the next frame.
playback.nodes.play.listeners.click();
playback.nodes["before-findings"].children[0].listeners.click();
if (playback.nodes.play.textContent !== "▶") throw new Error("selecting a finding did not pause the shared phase");
assertNoHashWrites(playback, "playing the shared phase");

// Two stance windows at the same frames stay two visible bands. The left
// and right shading are semi-transparent, so drawing them over each other
// cancels them into one grey block belonging to neither side — which is
// exactly what a repaired clip whose feet plant together produced.
const coincident = JSON.parse(JSON.stringify(data));
if (coincident.before.contexts.stances.length !== 2) throw new Error("the fixture needs a left and a right stance to coincide");
for (const row of coincident.before.contexts.stances) row.runs = [{start_frame: 200, end_frame: 900, start_s: 0.2, end_s: 0.9}];
const coincidentRun = run(generated, "comparison-report-data", html, coincident);
const bands = coincidentRun.nodes["before-gait"].children.filter((child) => child.attrs["data-stance-side"]);
if (new Set(bands.map((band) => band.attrs["data-stance-side"])).size !== 2) throw new Error("both stance sides must be shaded");
if (new Set(bands.map((band) => `${band.attrs.y}+${band.attrs.height}`)).size !== 2) throw new Error("two coincident stance windows drew one band on top of the other");

// One camera across both pose panes: a skeleton half the size renders half
// the size. Fitting each side to its own extent draws two skeletons the
// repair left identical at the same size whatever their real extents are.
const scaledSides = JSON.parse(JSON.stringify(data));
const halved = Buffer.from(data.after.clip.positions, "base64");
for (let offset = 0; offset < halved.length; offset += 4) halved.writeFloatLE(halved.readFloatLE(offset) * 0.5, offset);
scaledSides.after.clip.positions = halved.toString("base64");
const oneCamera = run(generated, "comparison-report-data", html, scaledSides);
const drawnSpread = (side) => {
  const arcs = oneCamera.nodes[`${side}-gl`].context.arcs.map((arc) => arc.args);
  if (!arcs.length) throw new Error(`the ${side} pose pane drew nothing`);
  const extent = (index) => Math.max(...arcs.map((arc) => arc[index])) - Math.min(...arcs.map((arc) => arc[index]));
  return Math.max(extent(0), extent(1));
};
if (!(drawnSpread("after") < drawnSpread("before") * 0.75)) throw new Error(`the pose panes do not share one camera: a half-size skeleton drew at ${drawnSpread("after")} against ${drawnSpread("before")}`);
// That same payload is the case where the repair did move the root, so the
// caption must not go on claiming the two tracks are the same line.
const scaledCaption = oneCamera.nodes["comparison-root-path-caption"].textContent;
if (!scaledCaption.includes("the before and after paths differ")) throw new Error(`two different root tracks are not declared different: ${scaledCaption}`);
if (scaledCaption.includes("are identical")) throw new Error(`two different root tracks are declared identical: ${scaledCaption}`);
const seamIndex = data.before.findings.indexOf(seamFinding), structuralIndex = data.before.findings.indexOf(structuralFinding), afterIndex = data.after.findings.indexOf(afterFinding);
nodes["before-findings"].children[seamIndex].listeners.click();
if(nodes.scrub.value != 1501 || !nodes["before-pose-context"].textContent.includes("first 0.000s") || !nodes["before-pose-context"].textContent.includes(`affected ${seam.subject_bone_name}`)) throw new Error("seam finding did not select exact frame and endpoint/subject context");
if(!nodes["before-gl"].context.arcs.some(row=>row.args[2]===6 && row.fillStyle===palette.error)) throw new Error("finding did not highlight its Rust-projected bone with the subject token");
nodes["before-findings"].children[structuralIndex].listeners.click();
if(!nodes["before-pose-context"].textContent.includes("structural evidence") || !nodes["before-contexts"].children.some(child=>child.className.includes("structural"))) throw new Error("structural finding was not distinguished from visible pose evidence");
nodes["after-findings"].children[afterIndex].listeners.click(); if(nodes.scrub.value != 1234) throw new Error("after finding did not select exact frame");
main.hash.value=`#time-before-${seamFinding.anchor.replace(/^finding-/, "")}`; windowListeners.hashchange();
if(nodes.scrub.value != 1501) throw new Error("semantic time anchor did not select its finding");
main.hash.value=`#finding-after-${afterFinding.anchor.replace(/^finding-/, "")}`; windowListeners.hashchange();
if(nodes.scrub.value != 1234) throw new Error("cross-side semantic finding anchor did not select its finding");

// Fragment options an embedded comparison honours, their persistence across
// the document's own anchor links, and their removal.
main.hash.value="#embed=1&theme=light&frame=1200&clip=ignored&finding=3"; windowListeners.hashchange();
if(documentElement.attrs["data-embed"] !== "1" || documentElement.attrs["data-theme"] !== "light" || nodes.scrub.value != 1200) throw new Error("comparison viewer ignored embed/theme/frame fragment options");
main.hash.value=`#finding-after-${afterFinding.anchor.replace(/^finding-/, "")}`; windowListeners.hashchange();
if(documentElement.attrs["data-embed"] !== "1" || documentElement.attrs["data-theme"] !== "light") throw new Error("following the document's own finding anchor un-pinned the embedded theme");
if(nodes.scrub.value != 1234) throw new Error("the anchor link stopped selecting its finding once a theme was pinned");
main.hash.value="#frame=999999999"; windowListeners.hashchange();
if(nodes.scrub.value != 2001) throw new Error("an out-of-range frame was not clamped to the shared phase");
if(documentElement.attrs["data-theme"] !== "light") throw new Error("a fragment that never mentions the theme must leave it pinned");
main.hash.value="#theme=neon&embed=0"; windowListeners.hashchange();
if("data-embed" in documentElement.attrs || "data-theme" in documentElement.attrs) throw new Error("an explicitly unusable value must restore the document default");
// The same three states for the shared phase: honoured, unusable (default
// restored), absent (left alone).
main.hash.value="#frame=800"; windowListeners.hashchange();
if(nodes.scrub.value != 800) throw new Error("a valid frame did not move the shared phase");
main.hash.value="#frame=-1"; windowListeners.hashchange();
if(nodes.scrub.value != 0) throw new Error("an unusable frame did not restore the default shared phase");
main.hash.value="#frame=800"; windowListeners.hashchange();
main.hash.value="#theme=dark"; windowListeners.hashchange();
if(nodes.scrub.value != 800) throw new Error("a fragment without a frame moved the shared phase");
assertNoHashWrites(main, "the comparison viewer");

// The comparison's panels are canvas drawings, so nothing but this callback
// repaints them when the reader's system theme changes: the palette has to be
// re-resolved and the panels redrawn with it.
// `pass` is the root trail's token, and the trajectory panels draw that
// trail. A panel whose only muted element was a caption now says nothing
// about the theme, because captions moved to the HTML the stylesheet
// colours; the trail is what the viewer still paints itself.
const comparisonTokens = (ink, pass) => tokenStyles({
  ground: "#101010", surface: "#1e1e2a", raised: "#232331", ink,
  muted: "#9099b2", line: "#3a3a4e", accent: "#0a0b0c", error: "#202122",
  warning: "#101112", pass, note: "#6b7390",
});
const canvasFills = (state, side) => {
  const canvas = state.nodes[`${side}-gl`];
  if (!canvas || !canvas.context) throw new Error(`the ${side} canvas was never drawn`);
  return canvas.context.arcs.map((arc) => arc.fillStyle);
};
const svgPaint = (state, id) => state.nodes[id].children.flatMap((child) => [child.attrs.fill, child.attrs.stroke]).filter(Boolean);
const schemeComparison = run(generated, "comparison-report-data", html, data, {styles: comparisonTokens("#123456", "#445566")});
for (const side of ["before", "after"]) {
  if (!canvasFills(schemeComparison, side).includes("#123456")) throw new Error(`the ${side} canvas did not paint its joints with the ink token`);
}
for (const panel of ["before-path", "after-path"]) {
  if (!svgPaint(schemeComparison, panel).includes("#445566")) throw new Error(`${panel} did not paint the root trail with the pass token`);
}
if (typeof schemeComparison.media.change !== "function") throw new Error("the comparison viewer does not listen for a system theme change");
schemeComparison.settings.styles = comparisonTokens("#654321", "#778899");
schemeComparison.media.change();
for (const side of ["before", "after"]) {
  const fills = canvasFills(schemeComparison, side);
  if (!fills.includes("#654321") || fills.includes("#123456")) throw new Error(`a system theme change did not repaint the ${side} canvas with the new tokens`);
}
for (const panel of ["before-path", "after-path"]) {
  const repainted = svgPaint(schemeComparison, panel);
  if (!repainted.includes("#778899") || repainted.includes("#445566")) throw new Error(`a system theme change did not repaint ${panel} with the new tokens`);
}
assertNoHashWrites(schemeComparison, "a comparison theme change");

// Every colour any comparison surface paints must come from the document's
// tokens: driving eleven distinct values and requiring each surface's paint
// to be a subset of them rejects a literal hard-coded anywhere in the viewer.
const distinctTokens = {
  ground: "#010101", surface: "#020202", raised: "#030303", ink: "#040404",
  muted: "#050505", line: "#060606", accent: "#070707", error: "#080808",
  warning: "#090909", pass: "#0a0a0a", note: "#0b0b0b",
};
const tokenValues = new Set(Object.values(distinctTokens));
const sourcing = run(generated, "comparison-report-data", html, data, {styles: tokenStyles(distinctTokens)});
// A selected finding brings the subject highlight and the stance shading into
// the drawing as well.
sourcing.nodes["before-findings"].children[seamIndex].listeners.click();
for (const surface of ["before-gl", "after-gl"]) {
  const context = sourcing.nodes[surface].context;
  if (!context) throw new Error(`${surface} was never drawn`);
  const painted = [...context.arcs.map((arc) => arc.fillStyle), ...context.strokes].filter(Boolean);
  if (!painted.length) throw new Error(`${surface} painted nothing to check`);
  for (const colour of painted) {
    if (!tokenValues.has(colour)) throw new Error(`${surface} painted ${colour}, which is not one of the document's tokens`);
  }
}
for (const surface of ["comparison-root-path", "before-path", "after-path", "before-gait", "after-gait"]) {
  const painted = svgPaint(sourcing, surface).filter((colour) => colour !== "none");
  if (!painted.length) throw new Error(`${surface} painted nothing to check`);
  for (const colour of painted) {
    if (!tokenValues.has(colour)) throw new Error(`${surface} painted ${colour}, which is not one of the document's tokens`);
  }
}
if (!sourcing.nodes["before-gl"].context.arcs.some((arc) => arc.args[2] === 6 && arc.fillStyle === distinctTokens.error)) {
  throw new Error("the selected finding's subject highlight is not painted from the error token");
}
assertNoHashWrites(sourcing, "painting every comparison surface");

// A comparison finding is reachable two ways, and both must leave the same
// drawing behind — the same frame and the same highlighted subject bone.
const clickedFinding = run(generated, "comparison-report-data", html, data);
clickedFinding.nodes["before-findings"].children[seamIndex].listeners.click();
const clickedArcs = JSON.stringify(clickedFinding.nodes["before-gl"].context.arcs);
const followedAnchor = run(generated, "comparison-report-data", html, data);
followedAnchor.hash.value = `#finding-before-${seamFinding.anchor.replace(/^finding-/, "")}`;
followedAnchor.listeners.hashchange();
if (followedAnchor.nodes.scrub.value !== clickedFinding.nodes.scrub.value) throw new Error("the anchor link and the click disagree on the selected frame");
if (JSON.stringify(followedAnchor.nodes["before-gl"].context.arcs) !== clickedArcs) throw new Error("the anchor link and the click disagree on the drawn pose, subject-bone highlight included");
if (!clickedFinding.nodes["before-gl"].context.arcs.some((arc) => arc.args[2] === 6)) throw new Error("clicking a finding must highlight its subject bone for the comparison to mean anything");
assertNoHashWrites(clickedFinding, "clicking a comparison finding");
assertNoHashWrites(followedAnchor, "following a comparison anchor");

// A non-finite sampled range must degrade the drawing, not abort navigation
// or hide the already-rendered findings and coverage lists.
const cleanBefore = data.before.clip.positions, cleanAfter = data.after.clip.positions;
const execute = payload => run(generated, "comparison-report-data", html, payload).nodes;
const svgText = (node) => node.children.map(child => child.textContent).join(" ");
// A panel's disclosures are its caption, which is the HTML paragraph beside
// it rather than text drawn into the picture.
const captionOf = (state, id) => state[`${id}-caption`].textContent;
const invalid = Buffer.from(data.before.clip.positions, "base64");
for (let offset = 0; offset < invalid.length; offset += 4) invalid.writeFloatLE(Number.NaN, offset);
data.before.clip.positions = invalid.toString("base64");
data.after.clip.positions = cleanAfter;
const isolatedNodes = execute(data);
const rootLabels = isolatedNodes["comparison-root-path"].children.map(child=>child.textContent);
const beforeTrailText = captionOf(isolatedNodes, "before-path");
const afterTrailText = captionOf(isolatedNodes, "after-path");
if (!isolatedNodes["before-pose-context"].textContent.includes("non-finite") || !captionOf(isolatedNodes, "before-gait").includes("non-finite") || !isolatedNodes["after-pose-context"].textContent.includes("exact judged") || !rootLabels.includes("before root unavailable") || !rootLabels.includes("after root path") || !beforeTrailText.includes("unavailable:") || !beforeTrailText.includes("non-finite") || afterTrailText.includes("non-finite") || isolatedNodes["before-findings"].children.length !== data.before.findings.length) throw new Error("before-side non-finite pose/gait/root/trail evidence was mislabeled, hidden, or threw");
assertNoBareSvgText(isolatedNodes, comparisonSvgs, "non-finite before side");

data.before.clip.positions = cleanBefore; data.after.clip.positions = invalid.toString("base64");
const reverseNodes = execute(data);
const reverseTrailText = captionOf(reverseNodes, "after-path");
if (!reverseNodes["before-pose-context"].textContent.includes("exact judged") || !reverseNodes["after-pose-context"].textContent.includes("non-finite") || !reverseTrailText.includes("unavailable:") || !reverseTrailText.includes("non-finite")) throw new Error("after-side non-finite evidence did not remain independent of exact before evidence");

// A selected mixed-finite frame also loses the exact-evidence label while
// other finite frames and the opposite side remain independently available.
const mixed = Buffer.from(cleanAfter, "base64");
mixed.writeFloatLE(Number.NaN, (1501 * bones * 3 + 1 * 3) * 4);
data.before.clip.positions = cleanBefore; data.after.clip.positions = mixed.toString("base64");
const mixedNodes = execute(data);
mixedNodes.scrub.value=1501; mixedNodes.scrub.listeners.input();
const mixedTrailText = captionOf(mixedNodes, "after-path");
if (!mixedNodes["after-pose-context"].textContent.includes("selected frame contains non-finite") || !mixedNodes["before-pose-context"].textContent.includes("exact judged") || !mixedTrailText.includes("incomplete non-finite samples")) throw new Error("mixed per-frame/trail availability was not evaluated independently");

// Structural context must remain visible without overriding the selected
// frame's non-finite availability disclosure.
data.before.clip.positions = mixed.toString("base64"); data.after.clip.positions = cleanAfter;
const structuralNodes = execute(data);
structuralNodes["before-findings"].children[structuralIndex].listeners.click();
if (!structuralNodes["before-pose-context"].textContent.includes("structural evidence") || !structuralNodes["before-pose-context"].textContent.includes("selected frame contains non-finite")) throw new Error("structural selection hid non-finite selected-frame availability");

// ---- evidence-only comparison, as generated ----------------------------
// Every pose surface is replaced by a notice in the document itself, so the
// viewer must find them absent, draw nothing, and keep every evidence list.
const comparisonEvidence = generatedReportParts(comparisonEvidenceHtml);
const evidencePayload = JSON.parse(comparisonEvidence.payload);
if (evidencePayload.evidence_only !== true) throw new Error("the evidence-only comparison is not marked as one");
if (comparisonEvidenceHtml.includes('"positions"')) throw new Error("the evidence-only comparison still embeds a pose grid");
const poseSurfaces = ["before-gl", "after-gl", "comparison-root-path", "before-path", "after-path", "before-gait", "after-gait"];
// Every comparison panel is drawn client-side from the pose grid, so an
// evidence-only comparison replaces all of them with the notice rather than
// leaving blank boxes behind. This is exactly what the documented contract
// now says, and what the follow-up Rust chart producer would change.
for (const surface of poseSurfaces) {
  if (comparisonEvidenceHtml.includes(`id="${surface}"`)) throw new Error(`${surface} is still rendered in an evidence-only comparison`);
  if (!comparisonEvidenceHtml.includes(`id="${surface}-notice"`)) throw new Error(`${surface} lost its omission notice`);
}
if (/<svg\s+id=/.test(comparisonEvidenceHtml)) throw new Error("an evidence-only comparison still carries a chart surface it cannot draw");
if (!/<figure class="chart"/.test(singleEvidenceHtml)) throw new Error("an evidence-only single-clip report lost its Rust-rendered charts");
if (!/<path class="root-path"/.test(singleEvidenceHtml)) throw new Error("an evidence-only single-clip report lost its plotted root path");
// The seven surfaces this form drops when it carries no poses, and nothing
// else, may be missing from the document the viewer runs against.
const evidenceRun = run(comparisonEvidence, "comparison-report-data", comparisonEvidenceHtml, evidencePayload, {omitted: poseSurfaces});
const evidenceNodes = evidenceRun.nodes;
if (!evidenceNodes.scrub.disabled) throw new Error("the shared phase stayed scrubbable with no pose grid behind it");
if (!/<button id="play"[^>]*\sdisabled/.test(comparisonEvidenceHtml)) throw new Error("the evidence-only comparison leaves playback enabled in its markup");
if (!evidenceNodes.play.disabled) throw new Error("the shared phase stayed playable with no pose grid behind it");
evidenceNodes.play.listeners.click();
if (evidenceNodes.play.textContent === "⏸") throw new Error("an evidence-only comparison started playing a grid it does not carry");
if (evidenceRun.clock.pending.length) throw new Error("an evidence-only comparison scheduled an animation frame");
if (evidenceNodes["before-findings"].children.length !== evidencePayload.before.findings.length || !evidenceNodes["before-identity"].textContent.includes(evidencePayload.before.dependency_closure_identity.sha256)) throw new Error("an evidence-only comparison dropped findings or identities");
if (!evidenceNodes.times.textContent.includes("not a time warp") || !evidenceNodes.mapping.textContent) throw new Error("an evidence-only comparison dropped its phase disclosures");

// Navigating an evidence-only comparison stays inert: the theme still
// applies, and no panel starts drawing from a grid the document lacks.
evidenceRun.hash.value = "#frame=5&theme=light";
evidenceRun.listeners.hashchange();
if (evidenceRun.root.attrs["data-theme"] !== "light") throw new Error("an evidence-only comparison stopped honouring the theme option");

// ---- fragment parser ---------------------------------------------------
// One parser serves both documents, so the same matrix runs against each
// document's own embedded copy, with valid, invalid, and hostile fragments.
// Nothing here may throw, and a key that never appears must stay absent so
// navigation cannot silently reset a switch the reader pinned.
function runParserMatrix(parse, document_) {
  const KEYS = ["embed", "theme", "clip", "frame", "finding"];
  function expectOptions(hash, expected, why) {
    let actual;
    try { actual = parse(hash); } catch (error) { throw new Error(`fragment ${JSON.stringify(String(hash).slice(0,40))} threw: ${error}`); }
    for (const key of KEYS) {
      const want = Object.prototype.hasOwnProperty.call(expected, key) ? expected[key] : undefined;
      if (!Object.is(actual[key], want)) throw new Error(`${document_}, ${why}: ${key} was ${JSON.stringify(actual[key])}, expected ${JSON.stringify(want)}`);
    }
  }
  expectOptions("#embed=1&theme=dark&clip=walk&frame=7&finding=2", {embed:true,theme:"dark",clip:"walk",frame:7,finding:2}, "every documented option");
  expectOptions("embed=true&theme=light", {embed:true,theme:"light"}, "a fragment without its leading hash");
  expectOptions("#clip=walk%20cycle%2F01", {clip:"walk cycle/01"}, "percent-encoded clip names");
  expectOptions("#unknown=1&x&=2&clip", {}, "unknown keys and malformed pairs stay absent");
  expectOptions("#finding-before-abcdef0123456789", {}, "the document's own anchor form changes nothing");
  expectOptions("#", {}, "an empty fragment");
  expectOptions("", {}, "no fragment at all");
  // A key that appears with a value this report cannot honour returns that
  // state to its default; a key that never appears stays absent.
  expectOptions("#theme=%3Cscript%3E", {theme:null}, "a hostile theme");
  expectOptions("#theme=LIGHT", {theme:null}, "an upper-case theme");
  expectOptions("#theme=", {theme:null}, "an empty theme");
  expectOptions("#embed=yes", {embed:false}, "an unusable embed value");
  expectOptions("#frame=-1&finding=NaN", {frame:null,finding:null}, "non-integer indices");
  expectOptions("#frame=1.5", {frame:null}, "a fractional frame");
  expectOptions("#clip=%E0%A4%A", {clip:null}, "a malformed percent escape");
  expectOptions("#frame=999999999", {frame:999999999}, "a large but exact index");
  if (parse("#frame=" + "9".repeat(400)).frame !== Infinity) throw new Error("an unbounded index must survive as one the caller clamps");
  for (const wrongType of [null, undefined, 0, {}, [], () => {}]) expectOptions(wrongType, {}, "a non-string fragment");
  // Every pair inside the length bound is read, however many there are.
  expectOptions("#" + "pad=1&".repeat(400) + "theme=dark", {theme:"dark"}, "a fragment with hundreds of pairs");
  const manyPairs = "#" + "p=&".repeat(1300) + "theme=dark";
  if (manyPairs.length > 4096) throw new Error("the many-pair fragment must stay inside the length bound");
  expectOptions(manyPairs, {theme:"dark"}, "a fragment with more than a thousand pairs");
  // The most pairs the length cap can hold, with the meaningful one last: any
  // cap on the number of pairs, however high, drops it.
  const meaningful = "frame=7";
  const fillers = Math.floor((4096 - 1 - meaningful.length) / 3);
  const capPairs = "#" + "p=&".repeat(fillers) + meaningful;
  if (capPairs.length > 4096) throw new Error(`the maximal fragment is ${capPairs.length} characters`);
  if (4096 - capPairs.length >= 3) throw new Error("the maximal fragment must leave no room for another pair");
  expectOptions(capPairs, {frame:7}, `a fragment of ${fillers + 1} pairs, the most the length bound admits`);
  // The length bound itself: the last accepted character and the first
  // rejected one.
  const tail = "&theme=light";
  const filler = "#" + "p=1&".repeat(200);
  const accepted = filler + "x=" + "y".repeat(4096 - filler.length - tail.length - 2) + tail;
  if (accepted.length !== 4096) throw new Error(`harness built a ${accepted.length}-character boundary fragment`);
  expectOptions(accepted, {theme:"light"}, "a fragment at the exact length bound");
  expectOptions("x" + accepted, {}, "a fragment one character past the bound");
}

runParserMatrix(context.animsmithFragmentOptions, "the comparison runtime");

// ---- single-clip viewer ------------------------------------------------
function singleReportParts(source) {
  const match = source.match(/<script>([\s\S]*?)<\/script>\n<script type="application\/json" id="report-data">([\s\S]*?)<\/script>\n<script>([\s\S]*?)<\/script>\n<\/body>\n<\/html>\s*$/);
  if (!match) throw new Error("Rust-generated single-clip payload and its inline viewer are absent");
  if (!match[1].startsWith("// animsmith report shared runtime")) throw new Error("wrong inline shared runtime");
  if (!match[3].startsWith("// animsmith report viewer")) throw new Error("wrong inline single-clip viewer");
  return { shared: match[1], payload: match[2], viewer: match[3] };
}
const single = singleReportParts(singleHtml);
const singlePayload = JSON.parse(single.payload);
if (!singlePayload.clips.length) throw new Error("the single-clip fixture must embed at least one pose grid");
const singleClip = singlePayload.clips[0], lastFrame = singleClip.frames - 1;
// A harness-owned finding proves selection independently of fixture content,
// and its message proves untrusted text stays text.
singlePayload.findings.push({check:"harness-check", severity:"warning", clip:singleClip.name, bone:"hips", node:null, time:singleClip.duration/2, message:"<img src=x>"});
const findingIndex = singlePayload.findings.length - 1;

function runSingle(parts, html, payload, settings) {
  return run(parts, "report-data", html, payload, settings);
}
// The chart the viewer syncs comes from the document, so its plot rectangle
// is read off the figure the report actually rendered — and every assertion
// observes that same figure, selected by kind rather than by position.
const gaitOf = (state) => {
  const chart = state.charts.find((figure) => figure.dataset.kind === "gait");
  if (!chart) throw new Error("the run has no gait chart to observe");
  return chart;
};
const playheadOf = (state) => Number(gaitOf(state).query[".playhead"].attrs.x1);
const gaitChart = documentCharts(singleHtml).find((chart) => chart.dataset.kind === "gait");
if (!gaitChart) throw new Error("the single-clip document carries no gait chart to sync");
const chartPad = Number(gaitChart.dataset.pad), chartPlotW = Number(gaitChart.dataset.plotw);
if (!(chartPad > 0) || !(chartPlotW > 0)) throw new Error("the gait chart does not publish its plot rectangle");

// The root-path dot is placed by frame index, and a frame the clip has no
// sampled position for carries the no-position marker instead of a
// coordinate. Scrubbing to such a frame must hide the dot rather than leave
// it at some other frame's position — the invention this pins is a leading
// run of unavailable frames showing a coordinate the clip only reaches
// later. The document under test is finite throughout, so the hole is
// introduced here, in the template the viewer reads.
const rootChartOf = (state) => {
  const chart = state.charts.find((figure) => figure.dataset.kind === "rootpath");
  if (!chart) throw new Error("the run has no root-path chart to observe");
  return chart;
};
const committedPoints = documentCharts(singleHtml)
  .find((chart) => chart.dataset.kind === "rootpath");
if (!committedPoints) throw new Error("the single-clip document carries no root-path chart");
const originalPoints = committedPoints.query[".pathpoints"].innerHTML.split(";");
if (originalPoints.length !== singleClip.frames) throw new Error(`the root-path template must carry one entry per judged frame: ${originalPoints.length} against ${singleClip.frames}`);
if (originalPoints.some((entry) => entry.split(",").length !== 2)) throw new Error("the committed fixture is finite throughout, so every entry is a coordinate");
// Frames 0 and 1 unavailable, everything after them as generated.
const holed = originalPoints.slice();
holed[0] = "-";
holed[1] = "-";
const holedHtml = singleHtml.replace(
  `<template class="pathpoints">${originalPoints.join(";")}</template>`,
  `<template class="pathpoints">${holed.join(";")}</template>`,
);
if (holedHtml === singleHtml) throw new Error("the harness failed to introduce a hole in the root-path template");
const holedRun = runSingle(single, holedHtml, singlePayload, {hash: "#frame=0"});
const dotOf = (state) => rootChartOf(state).query[".pathdot"];
if (dotOf(holedRun).attrs.display !== "none") throw new Error("scrubbing to a frame with no sampled position left the dot visible, at another frame's coordinate");
holedRun.nodes.scrub.value = "1";
holedRun.nodes.scrub.listeners.input();
if (dotOf(holedRun).attrs.display !== "none") throw new Error("the second unavailable frame left the dot visible");
// The first available frame shows the dot at that frame's own coordinate.
holedRun.nodes.scrub.value = "2";
holedRun.nodes.scrub.listeners.input();
const [availableX, availableY] = holed[2].split(",");
if (dotOf(holedRun).attrs.display === "none") throw new Error("an available frame must show the dot");
if (dotOf(holedRun).attrs.cx !== availableX || dotOf(holedRun).attrs.cy !== availableY) throw new Error(`the dot is placed at frame 2's own coordinate: ${dotOf(holedRun).attrs.cx},${dotOf(holedRun).attrs.cy} against ${holed[2]}`);
// And scrubbing back into the hole hides it again.
holedRun.nodes.scrub.value = "0";
holedRun.nodes.scrub.listeners.input();
if (dotOf(holedRun).attrs.display !== "none") throw new Error("scrubbing back to an unavailable frame did not hide the dot again");
assertNoHashWrites(holedRun, "scrubbing across an unavailable frame");

const plain = runSingle(single, singleHtml, singlePayload);
// The same matrix against this document's own copy of the runtime.
runParserMatrix(plain.context.animsmithFragmentOptions, "the single-clip runtime");
if (!plain.nodes.file.textContent.includes(singlePayload.file || "")) throw new Error("the viewer did not disclose its source file");
if (plain.nodes.findings.children.length !== singlePayload.findings.length) throw new Error("the findings panel dropped rows");
if (!plain.nodes.findings.children.map(row => row.children.map(part => part.textContent).join("|")).some(text => text.includes("<img src=x>"))) throw new Error("untrusted finding text was not carried as text");
if (!plain.nodes.gl.gl.clears.length) throw new Error("the WebGL view never cleared a frame");
if ("data-theme" in plain.root.attrs || "data-embed" in plain.root.attrs) throw new Error("an empty fragment must leave the document defaults alone");

// The 3D view paints from the live tokens: bones, joints, trails, and the
// clear colour all come from the palette the document resolves.
const themedTokens = {ground:"#F4F5F9", muted:"#112233", ink:"#445566", pass:"#010203", accent:"#0A0B0C", warning:"#101112", error:"#202122", raised:"#eef0f6", surface:"#ffffff", line:"#d9deea", note:"#6b7390"};
const rgb = hex => [1,3,5].map(offset => parseInt(hex.slice(offset, offset + 2), 16) / 255);
const themed = runSingle(single, singleHtml, singlePayload, {hash:"#theme=light", styles: tokenStyles(themedTokens)});
if (themed.root.attrs["data-theme"] !== "light") throw new Error("the pinned theme never reached the root element");
const cleared = themed.nodes.gl.gl.clears[0], groundRgb = rgb(themedTokens.ground);
if (cleared.slice(0, 3).some((channel, index) => Math.abs(channel - groundRgb[index]) > 1e-6)) throw new Error("the WebGL view did not clear with the themed ground token");
const uploaded = themed.nodes.gl.gl.buffers[themed.nodes.gl.gl.buffers.length - 1];
if (!uploaded || uploaded.length % 6) throw new Error("the viewer uploaded no interleaved vertices");
const uploadedColours = new Set();
for (let vertex = 0; vertex < uploaded.length; vertex += 6) uploadedColours.add(uploaded.slice(vertex + 3, vertex + 6).map(channel => channel.toFixed(4)).join(","));
const tokenColours = new Map(Object.entries(themedTokens).map(([name, hex]) => [rgb(hex).map(channel => channel.toFixed(4)).join(","), name]));
for (const colour of uploadedColours) {
  if (!tokenColours.has(colour)) throw new Error(`the 3D view uploaded ${colour}, which is not a design token`);
}
for (const required of ["muted", "ink"]) {
  if (![...uploadedColours].some(colour => tokenColours.get(colour) === required)) throw new Error(`bone and joint colours must come from the tokens; --${required} was never uploaded`);
}

// A deep link selects exactly what the equivalent click selects — for a
// finding in the middle of the list as well as the last one, so the
// equivalence cannot be an artefact of the index the harness appended.
function observableSelection(state, index) {
  const rows = state.nodes.findings.children;
  return {
    frame: String(state.nodes.scrub.value),
    time: state.nodes.time.textContent,
    selected: rows[index].classes.has("selected"),
    onlyOneSelected: rows.filter((row) => row.classes.has("selected")).length,
    playhead: gaitOf(state).query[".playhead"].attrs.x1,
    // What the 3D view actually drew, so the two paths agree on the pose and
    // its trail colours as well as on the panel state.
    drawn: JSON.stringify(state.nodes.gl.gl.buffers[state.nodes.gl.gl.buffers.length - 1] || []),
  };
}
function selectionsAgree(index, why) {
  const deep = runSingle(single, singleHtml, singlePayload, {hash: `#finding=${index}`});
  const clicked = runSingle(single, singleHtml, singlePayload);
  clicked.nodes.findings.children[index].listeners.click();
  const viaFragment = observableSelection(deep, index), viaClick = observableSelection(clicked, index);
  for (const key of Object.keys(viaFragment)) {
    if (viaFragment[key] !== viaClick[key]) throw new Error(`${why}: #finding=${index} and clicking row ${index} disagree on ${key}: ${viaFragment[key]} vs ${viaClick[key]}`);
  }
  if (!viaClick.selected || viaClick.onlyOneSelected !== 1) throw new Error(`${why}: selecting a finding must mark its row and only its row`);
  assertNoHashWrites(clicked, `clicking ${why}`);
  assertNoHashWrites(deep, `deep-linking ${why}`);
  return viaFragment;
}
if (singlePayload.findings.length < 3) throw new Error("the equivalence needs a list with a middle to address");
const middleIndex = 1;
const middleSelection = selectionsAgree(middleIndex, "a finding in the middle of the list");
const lastSelection = selectionsAgree(findingIndex, "the last finding");
if (middleSelection.frame === lastSelection.frame && middleSelection.playhead === lastSelection.playhead) {
  throw new Error("the two findings must land somewhere different for the comparison to mean anything");
}
if (middleSelection.drawn === lastSelection.drawn) {
  throw new Error("two findings on different frames must upload different vertices, or the equality proves nothing");
}

// The playhead spans exactly the rectangle the chart publishes.
const atStart = runSingle(single, singleHtml, singlePayload, {hash:"#frame=0"});
if (playheadOf(atStart) !== chartPad) throw new Error("frame 0 does not place the playhead at the plot origin");
const atEnd = runSingle(single, singleHtml, singlePayload, {hash:`#frame=${lastFrame}`});
if (Math.abs(playheadOf(atEnd) - (chartPad + chartPlotW)) > 1e-6) throw new Error("the last judged frame does not place the playhead at the plot's right edge");

// The three ways a frame arrives: honoured, past the end (clamped to the last
// judged frame, because a reader asking for a position wants the nearest one
// the document can show), and unreadable (frame 0).
if (Number(runSingle(single, singleHtml, singlePayload, {hash:`#frame=${Math.min(2, lastFrame)}`}).nodes.scrub.value) !== Math.min(2, lastFrame)) throw new Error("a frame inside the clip was not honoured");
const clampedFrame = runSingle(single, singleHtml, singlePayload, {hash:"#frame=999999999"});
if (Number(clampedFrame.nodes.scrub.value) !== lastFrame) throw new Error("a frame past the end of the clip was not clamped to its last judged frame");
if (Math.abs(playheadOf(clampedFrame) - (chartPad + chartPlotW)) > 1e-6) throw new Error("a clamped frame did not move the chart playhead to the plot's right edge");
if (Number(runSingle(single, singleHtml, singlePayload, {hash:"#frame=1.5"}).nodes.scrub.value) !== 0) throw new Error("an unreadable frame did not restore frame 0");

// Embed and theme reach the document; an unknown clip, an out-of-range
// frame, and a hostile fragment leave a usable report behind.
const embedded = runSingle(single, singleHtml, singlePayload, {hash:`#embed=1&theme=light&clip=${encodeURIComponent(singleClip.name)}&frame=${Math.min(2, lastFrame)}`});
if (embedded.root.attrs["data-theme"] !== "light" || embedded.root.attrs["data-embed"] !== "1") throw new Error("the single-clip viewer ignored embed/theme");
if (Number(embedded.nodes.scrub.value) !== Math.min(2, lastFrame)) throw new Error("a deep-linked frame did not scrub the viewer");
if (embedded.nodes["clip-select"].value !== singleClip.name) throw new Error("a deep-linked clip was not selected");
for (const hostile of [
  "#frame=999999999", "#frame=-1", "#clip=%E0%A4%A", "#clip=no-such-clip", "#finding=999999999",
  "#finding=" + "9".repeat(400), "#theme=%3Cimg%3E", "#embed=maybe", "#unknown=1",
  "#finding-before-abcdef0123456789", "#" + "k=v&".repeat(3000),
]) {
  let hostileRun;
  try { hostileRun = runSingle(single, singleHtml, singlePayload, {hash: hostile}); }
  catch (error) { throw new Error(`fragment ${JSON.stringify(hostile.slice(0,24))} threw in the viewer: ${error}`); }
  const frame = Number(hostileRun.nodes.scrub.value);
  if (!Number.isInteger(frame) || frame < 0 || frame > lastFrame) throw new Error(`fragment ${JSON.stringify(hostile.slice(0,24))} left frame ${frame} outside the judged grid`);
  if (hostileRun.nodes.findings.children.length !== singlePayload.findings.length) throw new Error("a hostile fragment changed the findings panel");
}

// Not just the row count: the rendered evidence is identical to a load with
// no fragment at all.
function serialize(node) {
  if (!node) return "";
  const attrs = Object.keys(node.attrs).sort().map((name) => ` ${name}="${node.attrs[name]}"`).join("");
  const classes = [...node.classes].sort().join(" ");
  return `<${node.tag || "el"}${node.id ? ` id="${node.id}"` : ""}${classes ? ` class="${classes}"` : ""}${attrs}>${node.textContent}${node.children.map(serialize).join("")}</>`;
}
const plainEvidence = runSingle(single, singleHtml, singlePayload);
const hostileEvidence = runSingle(single, singleHtml, singlePayload, {
  hash: "#theme=%3Cimg%3E&clip=%E0%A4%A&finding=999999999&frame=-1&embed=maybe&unknown=1",
});
for (const id of ["gaps", "predictions", "findings"]) {
  if (serialize(hostileEvidence.nodes[id]) !== serialize(plainEvidence.nodes[id])) {
    throw new Error(`a hostile fragment changed the rendered #${id}`);
  }
}
if (hostileEvidence.charts.map(serialize).join("") !== plainEvidence.charts.map(serialize).join("")) {
  throw new Error("a hostile fragment changed the rendered charts");
}

// ---- evidence-only single-clip report, as generated --------------------
const singleEvidence = singleReportParts(singleEvidenceHtml);
const evidenceSinglePayload = JSON.parse(singleEvidence.payload);
if (evidenceSinglePayload.evidence_only !== true) throw new Error("the evidence-only report is not marked as one");
if (singleEvidenceHtml.includes('"positions"')) throw new Error("the evidence-only report still embeds a pose grid");
if (singleEvidenceHtml.includes('id="gl"')) throw new Error("the evidence-only report still renders a canvas");
if (!/<button id="play"[^>]*\sdisabled/.test(singleEvidenceHtml)) throw new Error("the evidence-only report leaves playback enabled");
if (!singleEvidenceHtml.includes('id="gl-notice"')) throw new Error("the evidence-only report drops its omission notice");
evidenceSinglePayload.findings.push({check:"harness-check", severity:"warning", clip:evidenceSinglePayload.clips[0].name, bone:"hips", node:null, time:evidenceSinglePayload.clips[0].duration/2, message:"<img src=x>"});
const evidenceSingle = runSingle(singleEvidence, singleEvidenceHtml, evidenceSinglePayload, {omitted: ["gl"]});
if (evidenceSingle.nodes.findings.children.length !== evidenceSinglePayload.findings.length) throw new Error("an evidence-only report dropped findings");
// The charts survive, so the scrub still moves their playhead.
const evidenceLast = evidenceSinglePayload.clips[0].frames - 1;
evidenceSingle.nodes.scrub.value = String(evidenceLast);
evidenceSingle.nodes.scrub.listeners.input();
const evidencePlayhead = playheadOf(evidenceSingle);
if (Math.abs(evidencePlayhead - (chartPad + chartPlotW)) > 1e-6) throw new Error("scrubbing an evidence-only report does not move the chart playhead");
if (!evidenceSingle.nodes.time.textContent.includes("frame")) throw new Error("an evidence-only report stopped reporting the selected frame");
const evidenceDeep = runSingle(singleEvidence, singleEvidenceHtml, evidenceSinglePayload, {omitted: ["gl"], hash: `#finding=${evidenceSinglePayload.findings.length - 1}&theme=light`});
if (evidenceDeep.root.attrs["data-theme"] !== "light" || !evidenceDeep.nodes.findings.children[evidenceSinglePayload.findings.length - 1].classes.has("selected")) throw new Error("an evidence-only report stopped honouring deep links");

// ---- navigating the single-clip viewer ---------------------------------
// Each option has the same three states on a hash transition as it has on
// load: honoured, present but unusable (default restored), absent (left
// alone).
// Navigation writes the fragment the way a browser does — outside the
// document — so the viewer's own write counter stays meaningful.
function navigate(state, next) { state.hash.value = next; state.listeners.hashchange(); }
function assertNoHashWrites(state, why) {
  if (state.hash.writes !== 0) throw new Error(`${why}: the viewer wrote location.hash ${state.hash.writes} time(s)`);
}
const selectionNav = runSingle(single, singleHtml, singlePayload);
navigate(selectionNav, `#finding=${findingIndex}`);
if (!selectionNav.nodes.findings.children[findingIndex].classes.has("selected")) throw new Error("navigating to a finding did not select it");
navigate(selectionNav, "#finding=999999999");
if (selectionNav.nodes.findings.children.some(row => row.classes.has("selected"))) throw new Error("an unusable finding index left a selection standing");
navigate(selectionNav, `#finding=${findingIndex}`);
navigate(selectionNav, "#theme=dark");
if (!selectionNav.nodes.findings.children[findingIndex].classes.has("selected")) throw new Error("a fragment without a finding cleared the selection");
navigate(selectionNav, "#frame=2");
if (Number(selectionNav.nodes.scrub.value) !== 2) throw new Error("navigating to a frame did not scrub");
navigate(selectionNav, "#frame=999999999");
if (Number(selectionNav.nodes.scrub.value) !== lastFrame) throw new Error("navigating past the end of the clip did not clamp to its last judged frame");
navigate(selectionNav, "#frame=1.5");
if (Number(selectionNav.nodes.scrub.value) !== 0) throw new Error("an unreadable frame did not restore frame 0");
navigate(selectionNav, "#frame=2");
navigate(selectionNav, "#embed=1");
if (Number(selectionNav.nodes.scrub.value) !== 2) throw new Error("a fragment without a frame moved the playhead");

// ---- multi-clip report -------------------------------------------------
// Clip selection needs a document with more than one clip to mean anything.
const multi = singleReportParts(multiHtml);
const multiPayload = JSON.parse(multi.payload);
if (multiPayload.clips.length < 2) throw new Error("the multi-clip fixture must embed at least two clips");
const [firstClip, secondClip] = multiPayload.clips;
if (firstClip.name === secondClip.name) throw new Error("the multi-clip fixture must embed two distinguishable clips");
const runMulti = (hash) => runSingle(multi, multiHtml, multiPayload, hash === undefined ? undefined : {hash});
if (runMulti().nodes["clip-select"].value !== firstClip.name) throw new Error("a report opens on its first clip");
if (runMulti(`#clip=${encodeURIComponent(secondClip.name)}`).nodes["clip-select"].value !== secondClip.name) throw new Error("clip= did not select the named clip");
if (runMulti("#clip=no-such-clip").nodes["clip-select"].value !== firstClip.name) throw new Error("an unknown clip did not restore the first clip");
if (runMulti("#clip=%E0%A4%A").nodes["clip-select"].value !== firstClip.name) throw new Error("a malformed clip did not restore the first clip");
const clipNav = runMulti();
navigate(clipNav, `#clip=${encodeURIComponent(secondClip.name)}`);
if (clipNav.nodes["clip-select"].value !== secondClip.name) throw new Error("navigating to a clip did not select it");
navigate(clipNav, "#clip=no-such-clip");
if (clipNav.nodes["clip-select"].value !== firstClip.name) throw new Error("an unusable clip did not restore the first clip on navigation");
navigate(clipNav, `#clip=${encodeURIComponent(secondClip.name)}`);
navigate(clipNav, "#theme=light");
if (clipNav.nodes["clip-select"].value !== secondClip.name) throw new Error("a fragment without a clip changed the selected clip");

assertNoHashWrites(selectionNav, "navigating the single-clip viewer");
assertNoHashWrites(clipNav, "navigating clips");
assertNoHashWrites(evidenceSingle, "scrubbing an evidence-only report");
assertNoHashWrites(evidenceRun, "an evidence-only comparison");

// ---- system theme changes ----------------------------------------------
// The CSS follows prefers-color-scheme on its own; the canvas views have to
// be repainted, so the viewers listen for the change and re-resolve.
const schemeRun = runSingle(single, singleHtml, singlePayload, {styles: tokenStyles(Object.assign({}, themedTokens, {ground: "#101010"}))});
const firstClear = schemeRun.nodes.gl.gl.clears[schemeRun.nodes.gl.gl.clears.length - 1];
if (Math.abs(firstClear[0] - 0x10 / 255) > 1e-6) throw new Error("the viewer did not paint the first scheme's ground token");
if (typeof schemeRun.media.change !== "function") throw new Error("the viewer does not listen for a system theme change");
// The document's own resolved styles change first; the repaint follows.
schemeRun.settings.styles = tokenStyles(Object.assign({}, themedTokens, {ground: "#fdfdfd"}));
schemeRun.media.change();
const repainted = schemeRun.nodes.gl.gl.clears[schemeRun.nodes.gl.gl.clears.length - 1];
if (Math.abs(repainted[0] - 0xfd / 255) > 1e-6) throw new Error("a system theme change did not repaint the 3D view with the new ground token");
assertNoHashWrites(schemeRun, "a system theme change");

// Selecting a clip and using the transport are viewer actions as much as a
// click on a finding is: none of them may write the fragment either.
const actions = runMulti();
actions.nodes["clip-select"].value = secondClip.name;
actions.nodes["clip-select"].listeners.change();
if (actions.nodes["clip-select"].value !== secondClip.name) throw new Error("changing the clip select did not select that clip");
if (Number(actions.nodes.scrub.value) !== 0) throw new Error("selecting another clip did not return to its first frame");
actions.nodes.play.listeners.click();
if (actions.nodes.play.textContent !== "⏸") throw new Error("the play button did not start playback");
actions.nodes.play.listeners.click();
if (actions.nodes.play.textContent !== "▶") throw new Error("the play button did not pause");
actions.nodes.scrub.value = "1";
actions.nodes.scrub.listeners.input();
if (Number(actions.nodes.scrub.value) !== 1) throw new Error("scrubbing did not move the viewer");
assertNoHashWrites(actions, "selecting a clip, playing, pausing and scrubbing");

// The counters above cover the actions this harness drives; this covers the
// rest, including orbit and zoom: neither viewer nor the shared runtime
// contains an assignment to the fragment at all.
const navigationWrites = [
  [/location\s*\.\s*hash\s*=[^=]/, "location.hash ="],
  [/location\s*\[\s*['"]hash['"]\s*\]/, "location['hash']"],
  [/location\s*\.\s*href/, "location.href"],
  [/location\s*\.\s*(assign|replace)\s*\(/, "location.assign/replace("],
  [/history\s*\.\s*(push|replace)State\s*\(/, "history.pushState/replaceState("],
  [/window\s*\.\s*location\s*=[^=]/, "window.location ="],
  [/\.\s*hash\s*=[^=]/, "a .hash assignment on any alias"],
];
for (const [name, source] of [
  ["single-clip", single.viewer], ["comparison", generated.viewer], ["shared runtime", single.shared],
]) {
  for (const [pattern, spelling] of navigationWrites) {
    if (pattern.test(source)) throw new Error(`the ${name} source contains ${spelling}`);
  }
}

// Orbit and zoom are the actions the counters could not reach through a
// listener the harness drives by name.
const pointer = runSingle(single, singleHtml, singlePayload);
const drawnBefore = pointer.nodes.gl.gl.clears.length;
pointer.nodes.gl.listeners.mousedown({clientX: 10, clientY: 20});
pointer.listeners.mousemove({clientX: 48, clientY: 61});
pointer.listeners.mouseup();
pointer.nodes.gl.listeners.wheel({deltaY: 120, preventDefault() {}});
if (pointer.nodes.gl.gl.clears.length <= drawnBefore) throw new Error("orbiting and zooming did not redraw the 3D view");
assertNoHashWrites(pointer, "orbiting and zooming");

console.log("report viewer harness passed");
