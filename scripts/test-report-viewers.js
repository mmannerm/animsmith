"use strict";
// Executes the Rust-generated report viewers headlessly against the exact
// documents `just report-browser` produced: both report forms, each in its
// full and evidence-only shape. The DOM and WebGL stubs are deliberately
// thin — everything asserted here is something a reader would see.
const fs = require("fs"), vm = require("vm");
if (process.argv.length !== 8) {
  throw new Error("usage: test-report-viewers.js COMPARISON.html COMPARISON-EVIDENCE.html REPORT.html REPORT-EVIDENCE.html REPORT-MULTI-CLIP.html REPORT-GAIT-GROUP.html");
}
const [, , comparisonPath, comparisonEvidencePath, singlePath, singleEvidencePath, multiPath, groupPath] = process.argv;
const html = fs.readFileSync(comparisonPath, "utf8");
const comparisonEvidenceHtml = fs.readFileSync(comparisonEvidencePath, "utf8");
const singleHtml = fs.readFileSync(singlePath, "utf8");
const singleEvidenceHtml = fs.readFileSync(singleEvidencePath, "utf8");
const multiHtml = fs.readFileSync(multiPath, "utf8");
const groupHtml = fs.readFileSync(groupPath, "utf8");

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
    // Every drawn segment is recorded with the geometry it was drawn from —
    // its two endpoints — beside the paint and the dash in force for it, so a
    // pane that draws two skeletons can be read back as two: which bones,
    // from which grid, in which colour, dashed or not.
    if (!this.context) this.context={arcs:[],strokes:[],segments:[],strokeDashes:[],lineDash:[],pen:null,fillStyle:null,strokeStyle:null,setTransform(){},clearRect(){this.arcs=[];this.strokes=[];this.segments=[];this.strokeDashes=[]},setLineDash(dash){this.lineDash=Array.from(dash)},beginPath(){this.pen=[]},moveTo(...point){this.pen=[point]},lineTo(...point){if(this.pen)this.pen.push(point)},stroke(){this.strokes.push(this.strokeStyle);this.strokeDashes.push(this.lineDash.join(","));this.segments.push({points:(this.pen||[]).map((point)=>point.slice()),strokeStyle:this.strokeStyle,dash:this.lineDash.join(",")})},arc(...args){this.arcs.push({args,fillStyle:this.fillStyle})},fill(){}};
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
// A browser hands `dataset` the decoded attribute value, so the double does
// too: a clip or group name carrying an escapable character must reach the
// viewer as the document's own text and not as its markup spelling.
function decodeEntities(value) {
  return value.replace(/&(amp|lt|gt|quot|#39);/g, (_, entity) =>
    ({amp: "&", lt: "<", gt: ">", quot: '"', "#39": "'"})[entity]);
}

function documentCharts(html) {
  return [...html.matchAll(/<figure class="chart"[\s\S]*?<\/figure>/g)].map(([figure]) => {
    const node = new Node("chart");
    for (const [, name, value] of figure.matchAll(/data-([a-z]+)="([^"]*)"/g)) node.dataset[name] = decodeEntities(value);
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
  const clock = { now: 0, next: 1, pending: new Map() };
  // Every base64 decode the document performs, so a second copy of a pose
  // grid — or an evidence-only document decoding anything at all — is
  // observed rather than merely thought unlikely.
  const decoded = { count: 0 };
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
    requestAnimationFrame: (callback) => { const handle = clock.next++; clock.pending.set(handle, callback); return handle; },
    // `settings.ignoreCancel` is a browser that drops the cancellation on the
    // floor, which is what leaves a viewer with only its run number to stop
    // the chain it retired.
    cancelAnimationFrame: (handle) => { if (!settings.ignoreCancel) clock.pending.delete(handle); },
    atob: value => { decoded.count++; return Buffer.from(value, "base64").toString("binary"); },
    Uint8Array, Float32Array, Buffer, Math, Map, Set, Array, Number, Object, Infinity, JSON, console,
  };
  vm.createContext(context);
  vm.runInContext(`${parts.shared}\n${parts.viewer}`, context);
  return { nodes, root, listeners, context, charts, hash, media, settings, clock, decoded };
}

// One animation frame, `seconds` after the last one. Returns how many
// callbacks that frame actually ran, which is how many redraw chains the
// document is keeping alive.
function stepFrame(state, seconds) {
  state.clock.now += seconds * 1000;
  const pending = [...state.clock.pending.values()];
  state.clock.pending.clear();
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

// The drawn width of an end mark, which is how close two of them may be.
const MARK_WIDTH = 7;
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
const rootCaptionOf = (state) => state["comparison-root-path-caption"].textContent;
for (const [id, phrase] of [
  // Each panel's caption says what its own drawing shows. The gait panel
  // shades stance intervals and says so; the trajectory panels do not and do
  // not claim to; and every panel that draws two ends names them.
  ["comparison-root-path", "the two root paths overlaid so their shapes can be compared"],
  ["comparison-root-path", "before solid, after dashed and translucent"],
  ["comparison-root-path", "fitted to this panel"],
  ["before-path", "what to look for: top-down trails of root, hips and feet over the whole clip; matching trails mean the root, hips and feet moved the same way on both sides; what the trails do not draw is not compared here"],
  ["after-path", "what to look for: top-down trails of root, hips and feet over the whole clip; matching trails mean the root, hips and feet moved the same way on both sides; what the trails do not draw is not compared here"],
  ["before-path", "each trail starts at the hollow circle and ends at the square; the root's trail is the path magnified in the panel above"],
  ["before-path", "what the trails do not draw is not compared here"],
  ["after-path", "each trail starts at the hollow circle and ends at the square; the root's trail is the path magnified in the panel above"],
  ["before-path", "shared scale across both inputs"],
  ["after-path", "shared scale across both inputs"],
  ["before-gait", "shaded runs are sampled foot-slide stance evidence"],
  ["before-gait", "left in the upper band, right in the lower"],
  ["after-gait", "left in the upper band, right in the lower"],
]) {
  if (!panelCaption(id).includes(phrase)) throw new Error(`${id} lost its caption: ${JSON.stringify(phrase)}`);
}
// The "what to look for" sentences are derived on the Rust side from what
// each side's checks declared and judged: a clip that declares no loop and no
// root motion must not be told what its root path should do. The viewer's job
// is to render that sentence, not to carry one of its own.
for (const [id, sentence] of [
  ["comparison-root-path", data.before.guidance.root_path],
  ["before-gait", data.before.guidance.gait],
  ["after-gait", data.after.guidance.gait],
]) {
  if (!sentence) throw new Error(`the payload carries no derived guidance for ${id}`);
  if (!panelCaption(id).includes(sentence)) throw new Error(`${id} does not render the document's own derived guidance: ${JSON.stringify(sentence)} is not in ${JSON.stringify(panelCaption(id))}`);
}
// A viewer holding its own copy of those words would still pass the line
// above, so the sentences are also driven from the payload.
const swapped = JSON.parse(JSON.stringify(data));
swapped.before.guidance = {root_path: "SENTINEL-root-both", gait: "SENTINEL-gait-before"};
swapped.after.guidance = {root_path: "SENTINEL-root-both", gait: "SENTINEL-gait-after"};
const swappedRun = run(generated, "comparison-report-data", html, swapped);
for (const [id, sentinel] of [
  ["comparison-root-path", "SENTINEL-root-both"],
  ["before-gait", "SENTINEL-gait-before"],
  ["after-gait", "SENTINEL-gait-after"],
]) {
  if (!swappedRun.nodes[`${id}-caption`].textContent.includes(sentinel)) throw new Error(`${id} ignores the document's derived guidance and carries a sentence of its own`);
}
// Two sides that declare different contracts are both stated, attributed.
const split = JSON.parse(JSON.stringify(data));
split.before.guidance = {root_path: "SENTINEL-before-only", gait: "g"};
split.after.guidance = {root_path: "SENTINEL-after-only", gait: "g"};
const splitCaption = run(generated, "comparison-report-data", html, split).nodes["comparison-root-path-caption"].textContent;
for (const [side, sentinel] of [["before", "SENTINEL-before-only"], ["after", "SENTINEL-after-only"]]) {
  if (!splitCaption.includes(`${side} — ${sentinel}`)) throw new Error(`the root caption drops the ${side} side's own contract when the two differ: ${splitCaption}`);
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
if (rootDots.length !== 2) throw new Error("both sides' shared-phase dots are drawn, even where the two tracks coincide");
if (new Set(rootDots.map((dot) => dot.attrs.r)).size !== 2) throw new Error("two coincident shared-phase dots are drawn identically");

// A coincident `after` is also thinner and translucent, so the `before` it
// sits on stays visible through it rather than only around its gaps.
const rootPathOf = (state, side) => state.nodes["comparison-root-path"].children.find((child) => child.attrs["data-root-side"] === side);
const beforeStroke = rootPathOf(main, "before"), afterStroke = rootPathOf(main, "after");
if (!(Number(afterStroke.attrs.opacity) < Number(beforeStroke.attrs.opacity))) throw new Error(`the after track is opaque (${afterStroke.attrs.opacity}), so it hides a coincident before track`);
if (!(Number(afterStroke.attrs["stroke-width"]) < Number(beforeStroke.attrs["stroke-width"]))) throw new Error(`the after track is not drawn thinner than the before track: ${afterStroke.attrs["stroke-width"]} against ${beforeStroke.attrs["stroke-width"]}`);

// ---- the shared root panel says how big what it draws is ---------------
// The panel is fitted to its own extent, because a two-centimetre root drawn
// at the trail panels' scale is a five-unit blob under its own end marks.
// What it owes the reader instead is the size of the shape: a scale bar in
// the unit a reader measures in, and the magnification against the panels
// beside it. Both are recomputed here from the payload, so a viewer that
// mislabels either fails on the numbers rather than on a spelling.
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
const rootScale = fittedScale(rootBounds, 720, 180, 28);
if (Math.abs(roleScale - rootScale) < 1e-6) throw new Error("the fixture's root and role extents must differ, or the magnification cannot be told from 1");
// The same projection for any payload, so a mutated pair can be checked
// against its own extents rather than the committed fixture's.
function rootGeometry(payload) {
  const bounds = payloadBounds(payload, ["root"]);
  const scale = fittedScale(bounds, 720, 180, 28);
  return {
    bounds,
    scale,
    map: (point) => [
      720 / 2 + (point[0] - (bounds.x[0] + bounds.x[1]) / 2) * scale,
      180 / 2 - (point[1] - (bounds.z[0] + bounds.z[1]) / 2) * scale,
    ],
  };
}
const rootMap = (point) => [
  720 / 2 + (point[0] - (rootBounds.x[0] + rootBounds.x[1]) / 2) * rootScale,
  180 / 2 - (point[1] - (rootBounds.z[0] + rootBounds.z[1]) / 2) * rootScale,
];
const beforeRootMetres = payloadTrail(data, "before", "root").filter((point) => point.every(Number.isFinite));

// The bar's drawn length is exactly what it says it is. A bar labelled in
// one unit and measured off another scale is worse than no bar: a reader
// takes a length off it and gets a number the panel never drew.
const scaleBar = nodes["comparison-root-path"].children.find((child) => child.attrs["data-scale-bar-m"] != null);
if (!scaleBar) throw new Error("the shared root panel draws no scale bar, so nothing in the picture says how big it is");
const barMetres = Number(scaleBar.attrs["data-scale-bar-m"]);
if (![1, 0.5, 0.1, 0.05, 0.01].includes(barMetres)) throw new Error(`the scale bar is ${barMetres} m, which is not a length a reader measures in`);
const barDrawn = Number(scaleBar.attrs.x2) - Number(scaleBar.attrs.x1);
if (Math.abs(barDrawn - barMetres * rootScale) > 1e-6) throw new Error(`the scale bar is drawn ${barDrawn} plot units for a labelled ${barMetres} m, and this panel draws ${barMetres} m as ${barMetres * rootScale}`);
const barLabel = barMetres >= 0.5 ? `${barMetres} m` : `${Math.round(barMetres * 100)} cm`;
if (!nodes["comparison-root-path"].children.some((child) => child.tag === "text" && child.textContent === barLabel)) throw new Error(`the scale bar is not labelled ${barLabel}`);
if (barDrawn > 664) throw new Error(`the scale bar is ${barDrawn} plot units, wider than the ${664}-unit plot it sits in`);

// And the caption states the magnification against the panels beside it, so
// the two fitted pictures can still be read against each other.
const magnified = rootCaptionOf(nodes).match(/magnified ([0-9.]+)× relative to the trail panels/);
if (!magnified) throw new Error(`the caption does not state the panel's magnification against the trail panels: ${rootCaptionOf(nodes)}`);
if (magnified[1] !== (rootScale / roleScale).toFixed(1)) throw new Error(`the caption claims ${magnified[1]}× where this panel's fitted scale is ${(rootScale / roleScale).toFixed(1)}× the trail panels'`);

// One track's two end marks: the shapes, the track's own colour, their
// placement on the first and last sampled frames, and that neither is hidden
// by the phase dot standing on them where a track closes on itself.
function assertEndMarks(children, attribute, dotAttribute, value, metres, map, what) {
  const found = (kind) => children.find((child) => child.attrs[attribute] === `${value}-${kind}`);
  const circle = found("start"), square = found("end");
  if (!circle || !square) throw new Error(`${what} is not marked at both ends`);
  if (circle.tag !== "circle" || square.tag !== "rect") throw new Error(`${what}: its two ends are not drawn as two different shapes`);
  if (circle.attrs.fill !== "none") throw new Error(`${what}: the start mark is filled, so the phase dot standing on it disappears into it`);
  const dot = children.find((child) => child.attrs[dotAttribute] === value);
  if (!dot) throw new Error(`${what}: no phase dot to size and colour its marks against`);
  // The marks belong to their own track, so a panel of four trails says which
  // end is whose. A shared muted mark would say only that some track ended.
  if (circle.attrs.stroke !== dot.attrs.fill) throw new Error(`${what}: the start mark is stroked ${circle.attrs.stroke}, not the track's own ${dot.attrs.fill}`);
  if (square.attrs.fill !== dot.attrs.fill) throw new Error(`${what}: the end mark is filled ${square.attrs.fill}, not the track's own ${dot.attrs.fill}`);
  // Distinguishable by geometry rather than by paint order: on a track that
  // closes on itself the ring, the square and the phase dot land on one
  // coordinate, and z-order rescues none of them there.
  if (!(Number(circle.attrs.r) >= Number(dot.attrs.r) + 2)) throw new Error(`${what}: the start ring (r ${circle.attrs.r}) does not stand clear of its own phase dot (r ${dot.attrs.r})`);
  if (!square.attrs.stroke || square.attrs.stroke === square.attrs.fill) throw new Error(`${what}: the end mark has no contrasting stroke, so on a line of its own colour it is not a square`);
  const half = Number(square.attrs.width) / 2;
  const squareCentre = [Number(square.attrs.x) + half, Number(square.attrs.y) + half];
  const clearance = Math.hypot(squareCentre[0] - Number(dot.attrs.cx), squareCentre[1] - Number(dot.attrs.cy)) + half;
  if (!(clearance > Number(dot.attrs.r) + 2)) throw new Error(`${what}: the end mark lies inside its own phase dot (reaches ${clearance} against r ${dot.attrs.r})`);
  // No two marks in this panel may share a coordinate: a filled square hides
  // whatever is under it, and a ring hides one of its own radius.
  for (const other of children) {
    const marker = other.attrs[attribute];
    if (!marker || other === square || !marker.endsWith("-end")) continue;
    const otherCentre = [Number(other.attrs.x) + Number(other.attrs.width) / 2, Number(other.attrs.y) + Number(other.attrs.height) / 2];
    if (Math.hypot(otherCentre[0] - squareCentre[0], otherCentre[1] - squareCentre[1]) < Number(square.attrs.width)) throw new Error(`${what}: its end mark shares a coordinate with ${marker}, and a filled square hides what is under it`);
  }
  for (const other of children) {
    const marker = other.attrs[attribute];
    if (!marker || other === circle || !marker.endsWith("-start")) continue;
    if (Number(other.attrs.r) !== Number(circle.attrs.r)) continue;
    if (Math.hypot(Number(other.attrs.cx) - Number(circle.attrs.cx), Number(other.attrs.cy) - Number(circle.attrs.cy)) < Number(square.attrs.width)) throw new Error(`${what}: its start ring shares a coordinate and a radius with ${marker}, so one hides the other`);
  }
  // Each mark sits on the sampled frame it stands for, or steps aside with a
  // leader back to it.
  const anchored = (kind, mark, centre, point) => {
    const leader = children.find((child) => child.attrs[attribute] === `${value}-${kind}-leader`);
    if (!leader) {
      if (Math.hypot(centre[0] - point[0], centre[1] - point[1]) > 1e-6) throw new Error(`${what}: the ${kind} mark is neither on the frame it stands for (${point}) nor offset with a leader, but at ${centre}`);
      return;
    }
    if (Math.abs(leader.attrs.x1 - point[0]) > 1e-6 || Math.abs(leader.attrs.y1 - point[1]) > 1e-6) throw new Error(`${what}: the ${kind} leader does not start at the frame it stands for (${point})`);
    if (Math.abs(leader.attrs.x2 - centre[0]) > 1e-6 || Math.abs(leader.attrs.y2 - centre[1]) > 1e-6) throw new Error(`${what}: the ${kind} leader does not reach the mark it explains`);
  };
  anchored("start", circle, [Number(circle.attrs.cx), Number(circle.attrs.cy)], map(metres[0]));
  anchored("end", square, squareCentre, map(metres[metres.length - 1]));
}
const finiteTrail = (payload, side, role) => payloadTrail(payload, side, role).filter((point) => point.every(Number.isFinite));

// Both sides here carry the same grid, so the after root track is exactly the
// before one: it is drawn as the dashed overlay alone, because a second dot
// and a second pair of marks on the same coordinates say nothing, and the
// caption says the same thing in words.
// Every track carries its own marks, including an after track the repair
// left identical to the before one: its ring is narrower and its square
// takes a second lane, so neither hides under a mark already drawn.
for (const side of ["before", "after"]) {
  assertEndMarks(nodes["comparison-root-path"].children, "data-root-marker", "data-root-dot", side, finiteTrail(data, side, "root"), rootMap, `the ${side} root track`);
}
const rootMarkerOf = (side, kind) => nodes["comparison-root-path"].children.find((child) => child.attrs["data-root-marker"] === `${side}-${kind}`);
const ringRadii = ["before", "after"].map((side) => Number(rootMarkerOf(side, "start").attrs.r));
if (ringRadii[0] === ringRadii[1]) throw new Error(`two coincident start rings are drawn at the same radius (${ringRadii[0]}), so one hides the other`);
const squareCentres = ["before", "after"].map((side) => [Number(rootMarkerOf(side, "end").attrs.x), Number(rootMarkerOf(side, "end").attrs.y)]);
if (Math.hypot(squareCentres[0][0] - squareCentres[1][0], squareCentres[0][1] - squareCentres[1][1]) < 7) throw new Error("two coincident end squares are drawn on top of each other, and a filled square hides what is under it");
if (!rootCaptionOf(nodes).includes("identical, the after path lies under the before path")) throw new Error(`the caption does not say the two coincide: ${rootCaptionOf(nodes)}`);

// The heading is where a reader meets this panel, and it is one of three
// drawings of the same two paths, so it carries the factor too.
const rootTitle = nodes["root-panel-title"].textContent;
const titled = rootTitle.match(/^Root path, before over after \(magnified ([0-9.]+)×\)$/);
if (!titled) throw new Error(`the root panel is not titled as the overlay it is, with its magnification: ${JSON.stringify(rootTitle)}`);
if (titled[1] !== (rootScale / roleScale).toFixed(1)) throw new Error(`the title claims ${titled[1]}× where this panel's fitted scale is ${(rootScale / roleScale).toFixed(1)}× the trail panels'`);

// Every role trail carries the same two marks, in its own colour, at its own
// first and last sampled positions. These panels are where the trails overlap,
// so this is the panel that needs them most.
const trailMap = (point) => [
  360 / 2 + (point[0] - (trailBounds.x[0] + trailBounds.x[1]) / 2) * roleScale,
  180 / 2 - (point[1] - (trailBounds.z[0] + trailBounds.z[1]) / 2) * roleScale,
];
const TRAIL_ROLES = ["root", "hips", "left_foot", "right_foot"];
for (const role of TRAIL_ROLES) {
  if (data.before.clip.trails[role] == null) throw new Error(`the fixture must resolve every role trail; ${role} is absent`);
}
for (const side of ["before", "after"]) {
  for (const role of TRAIL_ROLES) {
    assertEndMarks(nodes[`${side}-path`].children, "data-role-marker", "data-role-dot", role, finiteTrail(data, side, role), trailMap, `the ${side} ${role} trail`);
  }
  // Root and hips run together from above, so one of the two is dashed and
  // translucent: two solid lines on one path leave the panel showing three
  // trails where its legend names four.
  const trailPath = (name) => nodes[`${side}-path`].children.find((child) => child.attrs["data-role"] === name);
  if (!trailPath("hips").attrs["stroke-dasharray"]) throw new Error(`the ${side} hips trail is solid over a solid root, so where the two coincide the panel shows one`);
  if (!(Number(trailPath("hips").attrs.opacity) < Number(trailPath("root").attrs.opacity))) throw new Error(`the ${side} hips trail is opaque over the root trail it covers`);
  // And the marks are named where the trails are named.
  for (const label of ["start", "end"]) {
    if (!nodes[`${side}-path`].children.some((child) => child.tag === "text" && child.textContent === label)) throw new Error(`the ${side} role-trajectory panel does not name its ${label} mark in the legend`);
  }
  const swatch = (tag) => nodes[`${side}-path`].children.find((child) => child.tag === tag && !child.attrs["data-role-marker"] && !child.attrs["data-role-dot"] && !child.attrs["data-role"]);
  if (!swatch("circle") || swatch("circle").attrs.fill !== "none") throw new Error(`the ${side} trail legend names a start mark without drawing the hollow circle it means`);
  if (!swatch("rect") || swatch("rect").attrs.fill === "none") throw new Error(`the ${side} trail legend names an end mark without drawing the filled square it means`);
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
const rootCaption = () => rootCaptionOf(nodes);
if (!rootCaption().includes("identical, the after path lies under the before path")) throw new Error(`two identical root tracks are not declared identical: ${rootCaption()}`);
const beforeGap = Math.hypot(
  beforeRootMetres[beforeRootMetres.length - 1][0] - beforeRootMetres[0][0],
  beforeRootMetres[beforeRootMetres.length - 1][1] - beforeRootMetres[0][1],
);
if (!(beforeGap > 0.001)) throw new Error("the fixture's root track must not already close on itself");
if (!rootCaption().includes(`before ends ${beforeGap.toFixed(3)} m from its start`)) throw new Error(`the caption does not state the end-to-start distance: ${rootCaption()}`);
if (!rootCaption().includes(` m at their widest`)) throw new Error(`the caption does not state the measured extent: ${rootCaption()}`);
if (!rootCaption().includes(`the corner bar is ${barLabel}`)) throw new Error(`the caption does not name the scale bar it draws: ${rootCaption()}`);

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
// It is also the case where all three marks land on one coordinate, so the
// end mark must have stepped aside rather than hidden inside the dot.
const closedGeometry = rootGeometry(closedRoots);
assertEndMarks(closedRun.nodes["comparison-root-path"].children, "data-root-marker", "data-root-dot", "before", finiteTrail(closedRoots, "before", "root"), closedGeometry.map, "the before root track of a closed loop");
if (!closedRun.nodes["comparison-root-path"].children.some((child) => child.attrs["data-root-marker"] === "before-end-leader")) throw new Error("a closed root loop draws its two ends on one coordinate without stepping the end mark aside");

// Two root paths that differ everywhere but begin and end at one point put
// their squares on the same coordinate exactly as two identical paths do, so
// the lane a mark takes is allocated from where it would be plotted rather
// than from whether the paths match.
const sharedEnds = JSON.parse(JSON.stringify(data));
// Two loops through the origin, one arcing each way: every frame between
// the ends differs, and both ends are the same coordinate.
for (const [name, sense] of [["before", 1], ["after", -1]]) {
  const side = sharedEnds[name], bone = side.clip.trails.root;
  const buffer = Buffer.from(side.clip.positions, "base64");
  for (let frame = 0; frame < side.clip.frames; frame++) {
    const base = (frame * sharedEnds.bones.length + bone) * 3;
    const angle = 2 * Math.PI * frame / (side.clip.frames - 1);
    buffer.writeFloatLE(Math.sin(angle) * 0.02, base * 4);
    buffer.writeFloatLE(sense * (1 - Math.cos(angle)) * 0.02, (base + 2) * 4);
  }
  side.clip.positions = buffer.toString("base64");
}
const sharedEndsRun = run(generated, "comparison-report-data", html, sharedEnds);
const sharedEndsCaption = sharedEndsRun.nodes["comparison-root-path-caption"].textContent;
if (!sharedEndsCaption.includes("the before and after paths differ")) throw new Error(`the shared-endpoint fixture must carry two different paths: ${sharedEndsCaption}`);
const sharedEndsGeometry = rootGeometry(sharedEnds);
const endsOf = (side) => {
  const points = finiteTrail(sharedEnds, side, "root");
  return [sharedEndsGeometry.map(points[0]), sharedEndsGeometry.map(points[points.length - 1])];
};
if (Math.hypot(endsOf("before")[1][0] - endsOf("after")[1][0], endsOf("before")[1][1] - endsOf("after")[1][1]) > 1e-6) throw new Error("the shared-endpoint fixture must plot both tracks' last frames on one coordinate");
for (const side of ["before", "after"]) {
  assertEndMarks(sharedEndsRun.nodes["comparison-root-path"].children, "data-root-marker", "data-root-dot", side, finiteTrail(sharedEnds, side, "root"), sharedEndsGeometry.map, `the ${side} root track of two paths that share their endpoints`);
}
const sharedEndsSquare = (side) => {
  const mark = sharedEndsRun.nodes["comparison-root-path"].children.find((child) => child.attrs["data-root-marker"] === `${side}-end`);
  return [Number(mark.attrs.x) + Number(mark.attrs.width) / 2, Number(mark.attrs.y) + Number(mark.attrs.height) / 2];
};
if (Math.hypot(sharedEndsSquare("before")[0] - sharedEndsSquare("after")[0], sharedEndsSquare("before")[1] - sharedEndsSquare("after")[1]) < MARK_WIDTH) throw new Error("two different paths that end on one coordinate drew their end squares in the same lane, and a filled square hides what is under it");

// ---- the scale bar's other branches ------------------------------------
// A payload whose four role trails occupy prescribed X/Z boxes, so the two
// fitted scales — and therefore the bar and the magnification — can be put
// where a committed fixture never reaches.
function shapedPayload(boxes) {
  const shaped = JSON.parse(JSON.stringify(data));
  for (const name of ["before", "after"]) {
    const side = shaped[name], buffer = Buffer.from(side.clip.positions, "base64");
    for (const [role, box] of Object.entries(boxes)) {
      const bone = side.clip.trails[role];
      if (bone == null) continue;
      for (let frame = 0; frame < side.clip.frames; frame++) {
        const base = (frame * shaped.bones.length + bone) * 3;
        const at = side.clip.frames > 1 ? frame / (side.clip.frames - 1) : 0;
        buffer.writeFloatLE(box.x[0] + at * (box.x[1] - box.x[0]), base * 4);
        buffer.writeFloatLE(box.z[0] + at * (box.z[1] - box.z[0]), (base + 2) * 4);
      }
    }
    side.clip.positions = buffer.toString("base64");
  }
  return shaped;
}
const everyRole = (box) => Object.fromEntries(TRAIL_ROLES.map((role) => [role, box]));
const barOf = (state) => state.nodes["comparison-root-path"].children.find((child) => child.attrs["data-scale-bar-m"] != null);

// Under a centimetre there is no bar a reader could measure against, so the
// panel draws none and the caption says which.
const tiny = shapedPayload(everyRole({x: [0, 0.004], z: [0, 0.003]}));
const tinyRun = run(generated, "comparison-report-data", html, tiny);
if (barOf(tinyRun)) throw new Error("a path narrower than the smallest bar still drew one");
if (!tinyRun.nodes["comparison-root-path-caption"].textContent.includes("no scale bar: the paths are narrower than 1 cm")) throw new Error(`a path with no bar does not say so: ${tinyRun.nodes["comparison-root-path-caption"].textContent}`);

// Between a centimetre and about two, a 1 cm bar runs across more than the
// three fifths of the plot the tidy rule allows, and there is no smaller
// step: the fallback keeps the bar rather than dropping it.
const fallback = shapedPayload(everyRole({x: [0, 0.012], z: [0, 0.0005]}));
const fallbackRun = run(generated, "comparison-report-data", html, fallback);
const fallbackBar = barOf(fallbackRun);
if (!fallbackBar) throw new Error("a path wider than a centimetre drew no bar");
if (Number(fallbackBar.attrs["data-scale-bar-m"]) !== 0.01) throw new Error(`the fallback bar is ${fallbackBar.attrs["data-scale-bar-m"]} m, not the smallest step inside the extent`);
const fallbackGeometry = rootGeometry(fallback);
const fallbackLength = Number(fallbackBar.attrs.x2) - Number(fallbackBar.attrs.x1);
if (Math.abs(fallbackLength - 0.01 * fallbackGeometry.scale) > 1e-6) throw new Error(`the fallback bar is drawn ${fallbackLength} plot units for a labelled 1 cm`);
if (!(fallbackLength > 0.6 * 664)) throw new Error("the fallback fixture must not satisfy the tidy rule, or it proves nothing");
if (fallbackLength > 664) throw new Error(`even the fallback bar stays inside the plot: ${fallbackLength}`);

// The root panel is twice as wide as a trail panel but no taller, so a pair
// of extents that both scale off Z draws the root smaller than the trails.
const shrunk = shapedPayload(everyRole({x: [0, 0.2], z: [0, 0.1]}));
const shrunkRun = run(generated, "comparison-report-data", html, shrunk);
const shrunkGeometry = rootGeometry(shrunk);
const shrunkRole = fittedScale(payloadBounds(shrunk, TRAIL_ROLES), 360, 180, 24);
const factor = shrunkGeometry.scale / shrunkRole;
if (!(factor < 1)) throw new Error(`the sub-1 fixture magnifies ${factor}×, so it exercises the wrong branch`);
const shrunkCaption = shrunkRun.nodes["comparison-root-path-caption"].textContent;
if (!shrunkCaption.includes(`drawn at ${factor.toFixed(1)}× the trail panels' scale below`)) throw new Error(`a panel smaller than the trail panels claims a magnification: ${shrunkCaption}`);
if (shrunkCaption.includes("magnified")) throw new Error(`a panel smaller than the trail panels says it is magnified: ${shrunkCaption}`);
if (shrunkRun.nodes["root-panel-title"].textContent !== `Root path, before over after (${factor.toFixed(1)}× the trail panels' scale)`) throw new Error(`the title of a shrunk panel is ${JSON.stringify(shrunkRun.nodes["root-panel-title"].textContent)}`);

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
// The shared phase runs at the *before* clip's duration, so the two sides
// must not share one: with equal durations a viewer reading either would
// pass. The after side is given a duration two and a half times the before
// one, and the rate is then pinned to within half a frame.
const timed = JSON.parse(JSON.stringify(data));
timed.after.clip.duration = timed.before.clip.duration * 2.5;
const playback = run(generated, "comparison-report-data", html, timed);
const playMax = Number(playback.nodes.scrub.max);
const poseArcs = (state, side) => JSON.stringify(state.nodes[`${side}-gl`].context.arcs);
if (!/<button id="play"[^>]*>▶<\/button>/.test(html)) throw new Error("the comparison does not open paused, with a play control the document itself labels");
const paused = {before: poseArcs(playback, "before"), after: poseArcs(playback, "after")};
playback.nodes.play.listeners.click();
if (playback.nodes.play.textContent !== "⏸") throw new Error("pressing play did not start the shared phase");
if (!stepFrame(playback, timed.before.clip.duration / 4)) throw new Error("playing scheduled no animation frame");
const quarter = Number(playback.nodes.scrub.value);
if (Math.abs(quarter - playMax / 4) > 0.51) throw new Error(`the shared phase does not advance at the before clip's ${timed.before.clip.duration}s duration: frame ${quarter} after a quarter of it, against ${playMax / 4} (the after clip's ${timed.after.clip.duration}s would give ${playMax / 10})`);
for (const side of ["before", "after"]) {
  if (poseArcs(playback, side) === paused[side]) throw new Error(`playing the shared phase did not redraw the ${side} pose pane`);
}
// Past the end it loops rather than stopping there.
stepFrame(playback, timed.before.clip.duration);
const looped = Number(playback.nodes.scrub.value);
if (!(looped >= 0 && looped <= quarter + 1)) throw new Error(`the shared phase did not loop at the end of the clip: frame ${looped} of ${playMax}`);
// Scrubbing takes the phase back, the way the single-clip viewer does.
playback.nodes.scrub.value = "10";
playback.nodes.scrub.listeners.input();
if (playback.nodes.play.textContent !== "▶") throw new Error("scrubbing did not pause the shared phase");
stepFrame(playback, timed.before.clip.duration / 4);
if (Number(playback.nodes.scrub.value) !== 10) throw new Error("a paused comparison kept advancing the shared phase");
// And so does selecting a finding, which would otherwise be overwritten by
// the next frame.
playback.nodes.play.listeners.click();
playback.nodes["before-findings"].children[0].listeners.click();
if (playback.nodes.play.textContent !== "▶") throw new Error("selecting a finding did not pause the shared phase");
assertNoHashWrites(playback, "playing the shared phase");

// A `#frame=` navigation is a reader asking for one position. Playback that
// kept running would overwrite it on the next frame, so it pauses the way a
// scrub does.
const fragmentPause = run(generated, "comparison-report-data", html, data);
fragmentPause.nodes.play.listeners.click();
if (fragmentPause.nodes.play.textContent !== "⏸") throw new Error("the fragment-pause fixture never started playing");
fragmentPause.hash.value = "#frame=42";
fragmentPause.listeners.hashchange();
if (fragmentPause.nodes.play.textContent !== "▶") throw new Error("a #frame= navigation did not pause playback");
stepFrame(fragmentPause, data.before.clip.duration / 4);
if (Number(fragmentPause.nodes.scrub.value) !== 42) throw new Error(`playback ran on past the deep-linked frame, leaving ${fragmentPause.nodes.scrub.value}`);
assertNoHashWrites(fragmentPause, "pausing on a deep-linked frame");

// One loop owner. A pause immediately followed by a play must not leave the
// old chain's pending callback alive beside the new one: two chains advance
// the shared phase twice per frame for as long as the document is open.
const restarted = run(generated, "comparison-report-data", html, data);
restarted.nodes.play.listeners.click();
stepFrame(restarted, 0.01);
// Pausing cancels the frame it scheduled, so nothing is left in flight.
restarted.nodes.play.listeners.click();
if (restarted.clock.pending.size !== 0) throw new Error(`pausing left ${restarted.clock.pending.size} animation frame(s) scheduled`);
restarted.nodes.play.listeners.click();
for (const cycle of [1, 2, 3, 4]) {
  const ran = stepFrame(restarted, 0.01);
  if (ran !== 1) throw new Error(`after ${cycle} restart cycle(s) one frame ran ${ran} callbacks, so the shared phase advances more than once per frame`);
  if (restarted.clock.pending.size !== 1) throw new Error(`after ${cycle} restart cycle(s) ${restarted.clock.pending.size} frame loops are scheduled`);
  restarted.nodes.play.listeners.click();
  restarted.nodes.play.listeners.click();
}
restarted.nodes.play.listeners.click();
assertNoHashWrites(restarted, "restarting playback");
// The control is named, not only drawn: a glyph is no accessible name.
if (!/<button id="play"[^>]*aria-label="[^"]+"/.test(html)) throw new Error("the comparison's play control has no accessible name");
if (restarted.nodes.play.attrs["aria-label"] !== "Play the shared phase") throw new Error(`a paused control is named ${JSON.stringify(restarted.nodes.play.attrs["aria-label"])}`);
restarted.nodes.play.listeners.click();
if (restarted.nodes.play.attrs["aria-label"] !== "Pause the shared phase") throw new Error("a playing control still calls itself Play");

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
if (scaledCaption.includes("identical")) throw new Error(`two different root tracks are declared identical: ${scaledCaption}`);
// A pair that really differs is drawn in full on both sides, and the two
// shared-phase dots stay distinguishable where they cross.
const scaledDots = oneCamera.nodes["comparison-root-path"].children.filter((child) => child.attrs["data-root-dot"]);
if (scaledDots.length !== 2) throw new Error("a differing pair must draw both sides' shared-phase dots");
if (new Set(scaledDots.map((dot) => dot.attrs.r)).size !== 2) throw new Error("two shared-phase dots are drawn identically");
const scaledGeometry = rootGeometry(scaledSides);
for (const side of ["before", "after"]) {
  assertEndMarks(oneCamera.nodes["comparison-root-path"].children, "data-root-marker", "data-root-dot", side, finiteTrail(scaledSides, side, "root"), scaledGeometry.map, `the ${side} root track of a differing pair`);
}
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
// What a pose pane actually drew: its recorded arcs, strokes and dash state.
const poseContext = (state, side) => {
  const canvas = state.nodes[`${side}-gl`];
  if (!canvas || !canvas.context) throw new Error(`the ${side} pose pane was never drawn`);
  return canvas.context;
};
const canvasFills = (state, side) => poseContext(state, side).arcs.map((arc) => arc.fillStyle);
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

// ---- the overlay: both skeletons in one pane ---------------------------
// The two panes already share one camera and one shared phase, so the overlay
// is those same two drawings put in one box: the before skeleton solid, the
// after one dashed over it, at exactly the frames the panes drew. Everything
// below is read off the recorded canvas calls, which is what a reader sees.
const SEGMENTS = data.bones.filter((bone) => bone.parent >= 0).length;
if (SEGMENTS < 2) throw new Error("the fixture must carry a skeleton of more than one segment");
const AFTER_DASH = "7,5";
const OVERLAY_LEGEND = "overlay · before solid blue, after dashed orange";
const OVERLAY_POINTER = "judged pose drawn over the before pane, as the dashed skeleton";
const jointPositions = (context) => context.arcs.map((arc) => `${arc.args[0]},${arc.args[1]}`).join(";");
const toggleOverlay = (state, on) => { state.nodes.overlay.checked = on; state.nodes.overlay.listeners.change(); };
// Everything one pane drew, so two runs can be required to have drawn the
// same picture: paint, dash, joint radii and every coordinate.
const drawingOf = (state, side) => {
  const context = poseContext(state, side);
  return JSON.stringify({ arcs: context.arcs, strokes: context.strokes, dashes: context.strokeDashes, segments: context.segments });
};
// The pose panes' own projection, rebuilt from the payload: one camera over
// both sides' finite X/Y, fitted to the canvas this harness gives every node.
// It lets an overlaid joint be checked against a coordinate the fixture
// defines — x = frame/1000 + bone, y = bone — rather than only against what
// the viewer itself drew.
const POSE_CANVAS = { width: 360, height: 270 };
function poseProjection(payload) {
  let minX = Infinity, maxX = -Infinity, minY = Infinity, maxY = -Infinity;
  for (const name of ["before", "after"]) {
    const floats = decodeFloats(payload[name].clip.positions);
    for (let index = 0; index < floats.length; index += 3) {
      if (Number.isFinite(floats[index])) { minX = Math.min(minX, floats[index]); maxX = Math.max(maxX, floats[index]); }
      if (Number.isFinite(floats[index + 1])) { minY = Math.min(minY, floats[index + 1]); maxY = Math.max(maxY, floats[index + 1]); }
    }
  }
  const scale = Math.min(POSE_CANVAS.width / Math.max(.1, maxX - minX), POSE_CANVAS.height / Math.max(.1, maxY - minY)) * .72;
  return (point) => [POSE_CANVAS.width / 2 + (point[0] - (minX + maxX) / 2) * scale, POSE_CANVAS.height / 2 - (point[1] - (minY + maxY) / 2) * scale];
}
const posePointOf = (payload, name, frame, bone) => {
  const floats = decodeFloats(payload[name].clip.positions), base = (frame * bones + bone) * 3;
  return [floats[base], floats[base + 1]];
};
const near = (left, right) => Math.abs(left - right) < 1e-9;
const samePoint = (drawn, want) => drawn && near(drawn[0], want[0]) && near(drawn[1], want[1]);

const overlayRun = run(generated, "comparison-report-data", html, data);
const twoPaneBefore = jointPositions(poseContext(overlayRun, "before"));
const twoPaneAfter = jointPositions(poseContext(overlayRun, "after"));
if (poseContext(overlayRun, "before").arcs.length !== bones) throw new Error(`a pane draws its skeleton's ${bones} joints`);
if (poseContext(overlayRun, "before").strokes.length !== SEGMENTS) throw new Error(`a pane draws its skeleton's ${SEGMENTS} segments`);
if (overlayRun.nodes["after-gl"].hidden) throw new Error("the after pane is hidden before the overlay is ever switched on");
// Both sides of this fixture carry the same grid, which is what a repair that
// left the poses alone produces: the two skeletons coincide, and the overlay
// still has to read as two.
if (twoPaneBefore !== twoPaneAfter) throw new Error("this fixture's two sides must carry the same grid, or a coincident overlay proves nothing");

toggleOverlay(overlayRun, true);
const overlaid = poseContext(overlayRun, "before");
// The two panes' own skeletons, at their own projected coordinates: one
// camera, the frames each pane used, no refit and no offset.
if (jointPositions(overlaid) !== `${twoPaneBefore};${twoPaneAfter}`) throw new Error("the overlay does not draw the two panes' own skeletons at their own projected coordinates");
if (overlaid.strokes.length !== 2 * SEGMENTS) throw new Error(`the overlay drew ${overlaid.strokes.length} segments, not two skeletons' ${2 * SEGMENTS}`);
const beforePass = { strokes: overlaid.strokes.slice(0, SEGMENTS), dashes: overlaid.strokeDashes.slice(0, SEGMENTS) };
const afterPass = { strokes: overlaid.strokes.slice(SEGMENTS), dashes: overlaid.strokeDashes.slice(SEGMENTS) };
if (beforePass.strokes.some((paint) => paint !== palette.accent)) throw new Error(`the before skeleton is not drawn in the accent token: ${beforePass.strokes}`);
if (afterPass.strokes.some((paint) => paint !== palette.warning)) throw new Error(`the after skeleton is not drawn in the warning token: ${afterPass.strokes}`);
if (beforePass.dashes.some((dash) => dash !== "")) throw new Error("the before skeleton is dashed, so two coincident skeletons read as one");
if (afterPass.dashes.some((dash) => dash !== AFTER_DASH)) throw new Error(`the after skeleton is not dashed ${AFTER_DASH}, so where the two coincide the pane shows one: ${afterPass.dashes}`);
// The after skeleton's joints are finer as well as dashed, so a joint the
// repair did not move still reads as two dots rather than one.
const beforeRadii = [...new Set(overlaid.arcs.slice(0, bones).map((arc) => arc.args[2]))];
const afterRadii = [...new Set(overlaid.arcs.slice(bones).map((arc) => arc.args[2]))];
if (beforeRadii.length !== 1 || afterRadii.length !== 1) throw new Error(`each pass draws its joints at one radius: ${beforeRadii} against ${afterRadii}`);
if (!(afterRadii[0] < beforeRadii[0])) throw new Error(`the after skeleton's joints (r ${afterRadii[0]}) are not finer than the before one's (r ${beforeRadii[0]})`);
// One grid per side, decoded once for the document: the overlay is a second
// reading of what the panes already decoded, not a second copy of it.
if (overlayRun.decoded.count !== 2) throw new Error(`the document decoded ${overlayRun.decoded.count} pose grids, not one per side`);
// The after pane keeps a visible label saying where its pose went; hiding it
// with the canvas would leave its heading over an empty box.
if (overlayRun.nodes["after-pose-context"].textContent !== OVERLAY_POINTER) throw new Error(`the hidden after pane's label is ${JSON.stringify(overlayRun.nodes["after-pose-context"].textContent)}`);
if (overlayRun.nodes["after-pose-context"].hidden) throw new Error("the after pane's label is hidden with its canvas, so nothing says where its pose went");
if (overlayRun.nodes["after-gl"].hidden !== true) throw new Error("the after pane still draws its own canvas while its skeleton is drawn over the before one");
const overlaidLabel = overlayRun.nodes["before-pose-context"].textContent;
// The legend names both line styles and both token colours, in the words the
// loop-seam label in this same element already uses for the same two tokens.
if (overlaidLabel !== `${OVERLAY_LEGEND} · exact judged pose-grid frames`) throw new Error(`the overlaid pane's label is ${JSON.stringify(overlaidLabel)}`);

// Switching it off restores the two panes and both of their labels.
toggleOverlay(overlayRun, false);
if (overlayRun.nodes["after-gl"].hidden !== false) throw new Error("switching the overlay off left the after canvas hidden");
if (jointPositions(poseContext(overlayRun, "before")) !== twoPaneBefore) throw new Error("switching the overlay off did not restore the before pane's own skeleton");
if (jointPositions(poseContext(overlayRun, "after")) !== twoPaneAfter) throw new Error("switching the overlay off did not redraw the after pane");
if (poseContext(overlayRun, "before").strokeDashes.some((dash) => dash !== "")) throw new Error("the restored two-pane drawing is dashed");
for (const side of ["before", "after"]) {
  const label = overlayRun.nodes[`${side}-pose-context`].textContent;
  if (label !== "exact judged pose-grid frame") throw new Error(`the ${side} pane's label did not return to the two-pane one: ${label}`);
}
if (overlayRun.decoded.count !== 2) throw new Error(`switching the overlay decoded ${overlayRun.decoded.count - 2} further pose grid(s)`);
assertNoHashWrites(overlayRun, "switching the overlay on and off");

// Unequal judged frame counts: the after clip carries half the before clip's
// frames, so an interior shared phase lands on two different frame indices.
// Its grid is halved as well as truncated, so no frame of it holds the same
// positions as any frame of the before grid. The overlay must therefore draw
// this side's own grid, at this side's own frame, and no other.
const unequal = JSON.parse(JSON.stringify(data));
const unequalFrames = 1001;
const unequalAfterBytes = Buffer.from(data.after.clip.positions, "base64").slice(0, unequalFrames * bones * 3 * 4);
for (let offset = 0; offset < unequalAfterBytes.length; offset += 4) unequalAfterBytes.writeFloatLE(unequalAfterBytes.readFloatLE(offset) * 0.5, offset);
unequal.after.clip.frames = unequalFrames;
unequal.after.clip.positions = unequalAfterBytes.toString("base64");
unequal.after.clip.times = data.after.clip.times.slice(0, unequalFrames);
const unequalRun = run(generated, "comparison-report-data", html, unequal);
unequalRun.nodes.scrub.value = "1000";
unequalRun.nodes.scrub.listeners.input();
// The frames the shared phase selects, recomputed here rather than read back
// from the drawing: phase is the scrub against the longer clip, and each side
// takes that phase of its own last frame.
const unequalPhase = 1000 / (Math.max(unequal.before.clip.frames, unequal.after.clip.frames) - 1);
const unequalFrame = (name) => Math.round(unequalPhase * (unequal[name].clip.frames - 1));
if (unequalFrame("before") === unequalFrame("after")) throw new Error("the unequal fixture must select two different frames");
const unequalTimes = unequalRun.nodes.times.textContent;
for (const name of ["before", "after"]) {
  const stamp = `${name} ${unequal[name].clip.times[unequalFrame(name)].toFixed(3)}s`;
  if (!unequalTimes.includes(stamp)) throw new Error(`the panes do not stand at the frames the shared phase selects: ${unequalTimes} lacks ${stamp}`);
}
const unequalBefore = jointPositions(poseContext(unequalRun, "before"));
const unequalAfter = jointPositions(poseContext(unequalRun, "after"));
if (unequalBefore === unequalAfter) throw new Error("two different frames of this fixture must draw two different poses");
toggleOverlay(unequalRun, true);
if (jointPositions(poseContext(unequalRun, "before")) !== `${unequalBefore};${unequalAfter}`) throw new Error("the overlay drew frames of its own rather than the two the panes selected");
if (unequalRun.nodes.times.textContent !== unequalTimes) throw new Error("the overlay changed the source times the shared phase labels");

// And those coordinates are the ones this fixture defines, joint by joint and
// segment by segment, through the panes' own camera — so a pass that drew the
// after skeleton's bones out of the before grid, or at the before frame,
// lands somewhere this says it should not.
const unequalProject = poseProjection(unequal);
const analyticJoints = ["before", "after"].flatMap((name) =>
  Array.from({ length: bones }, (_, bone) => unequalProject(posePointOf(unequal, name, unequalFrame(name), bone))));
const drawnJoints = poseContext(unequalRun, "before").arcs.map((arc) => [arc.args[0], arc.args[1]]);
if (drawnJoints.length !== analyticJoints.length) throw new Error(`the overlay drew ${drawnJoints.length} joints, not the ${analyticJoints.length} the two grids hold`);
drawnJoints.forEach((point, index) => {
  if (!samePoint(point, analyticJoints[index])) throw new Error(`overlaid joint ${index} is at ${point}, not at the ${index < bones ? "before" : "after"} grid's own ${analyticJoints[index]}`);
});
const analyticSegments = (name) => data.bones
  .map((row, bone) => ({ parent: row.parent, bone }))
  .filter((row) => row.parent >= 0)
  .map((row) => [
    unequalProject(posePointOf(unequal, name, unequalFrame(name), row.parent)),
    unequalProject(posePointOf(unequal, name, unequalFrame(name), row.bone)),
  ]);
const wantSegments = [...analyticSegments("before"), ...analyticSegments("after")];
const drawnSegments = poseContext(unequalRun, "before").segments;
if (drawnSegments.length !== wantSegments.length) throw new Error(`the overlay drew ${drawnSegments.length} segments, not the ${wantSegments.length} the two skeletons hold`);
drawnSegments.forEach((segment, index) => {
  const want = wantSegments[index], side = index < SEGMENTS ? "before" : "after";
  if (segment.points.length !== 2 || !samePoint(segment.points[0], want[0]) || !samePoint(segment.points[1], want[1])) {
    throw new Error(`overlaid segment ${index} runs ${JSON.stringify(segment.points)}, not the ${side} grid's own ${JSON.stringify(want)} at frame ${unequalFrame(side)}`);
  }
  if (segment.dash !== (side === "after" ? AFTER_DASH : "")) throw new Error(`overlaid segment ${index} of the ${side} skeleton is drawn ${JSON.stringify(segment.dash)}`);
});
assertNoHashWrites(unequalRun, "overlaying two clips of unequal length");

// A selected finding marks its subject bone on its own side's skeleton alone.
// These two skeletons are drawn apart, so the marked joint's coordinate — not
// its place in the draw order — says which one carries it, and nothing here
// assumes every joint is finite or drawn in bone order.
for (const [side, index] of [["after", afterIndex], ["before", structuralIndex]]) {
  const panes = run(generated, "comparison-report-data", html, unequal);
  panes.nodes[`${side}-findings`].children[index].listeners.click();
  const own = new Set(jointPositions(poseContext(panes, side)).split(";"));
  const other = new Set(jointPositions(poseContext(panes, side === "before" ? "after" : "before")).split(";"));
  if ([...own].some((at) => other.has(at))) throw new Error(`this fixture must draw its two skeletons apart, or a coordinate cannot say which one is marked`);
  const overlaidRun = run(generated, "comparison-report-data", html, unequal);
  toggleOverlay(overlaidRun, true);
  overlaidRun.nodes[`${side}-findings`].children[index].listeners.click();
  const marks = poseContext(overlaidRun, "before").arcs.filter((arc) => arc.args[2] === 6);
  if (marks.length !== 1) throw new Error(`a ${side}-side finding marked ${marks.length} joints in the overlaid pane, not one`);
  const at = `${marks[0].args[0]},${marks[0].args[1]}`;
  if (!own.has(at)) throw new Error(`a ${side}-side finding marked a joint at ${at}, which is not one the ${side} skeleton drew`);
  if (other.has(at)) throw new Error(`a ${side}-side finding marked a joint of the other skeleton`);
  if (marks[0].fillStyle !== palette.error) throw new Error("the subject bone is not marked with the error token");
}

// A structural context composes with the overlay on either side. The after
// side is the case that matters: its pane is hidden, so its disclosure has to
// travel with it, and the before pane must not adopt it as its own.
const afterStructuralPayload = JSON.parse(JSON.stringify(data));
const afterStructuralContext = {
  check: structural.check, evidence_kind: "structural", finding_anchor: "finding-2222222222222222",
  label: structural.label, source: structural.source, subject_bone_name: structural.subject_bone_name,
};
afterStructuralPayload.after.contexts.structural.push(afterStructuralContext);
afterStructuralPayload.after.findings.push(Object.assign({}, structuralFinding, { anchor: afterStructuralContext.finding_anchor, time: 0.777 }));
const afterStructuralRun = run(generated, "comparison-report-data", html, afterStructuralPayload);
toggleOverlay(afterStructuralRun, true);
afterStructuralRun.nodes["after-findings"].children[afterStructuralPayload.after.findings.length - 1].listeners.click();
if (afterStructuralRun.nodes["after-gl"].hidden !== true) throw new Error("a structural selection on the after side suspended the overlay");
const hiddenPaneLabel = afterStructuralRun.nodes["after-pose-context"].textContent;
if (!hiddenPaneLabel.includes(structural.label)) throw new Error(`the after side's structural disclosure is dropped while its pane is hidden: ${JSON.stringify(hiddenPaneLabel)}`);
if (!hiddenPaneLabel.includes(OVERLAY_POINTER)) throw new Error(`the hidden after pane stopped saying where its pose went: ${JSON.stringify(hiddenPaneLabel)}`);
const overlaidUnderAfterStructural = afterStructuralRun.nodes["before-pose-context"].textContent;
if (!overlaidUnderAfterStructural.includes(OVERLAY_LEGEND)) throw new Error(`the overlaid pane lost its legend: ${overlaidUnderAfterStructural}`);
if (overlaidUnderAfterStructural.includes(structural.label)) throw new Error(`the before pane claimed the after side's structural context as its own: ${overlaidUnderAfterStructural}`);
// The before side's own structural context still composes, and a structural
// label already speaking for the pane still suppresses the exact-frame note.
const beforeStructuralRun = run(generated, "comparison-report-data", html, data);
toggleOverlay(beforeStructuralRun, true);
beforeStructuralRun.nodes["before-findings"].children[structuralIndex].listeners.click();
const beforeStructuralLabel = beforeStructuralRun.nodes["before-pose-context"].textContent;
if (beforeStructuralLabel !== `${structural.label} · ${OVERLAY_LEGEND}`) throw new Error(`a before-side structural context and the overlay do not compose: ${JSON.stringify(beforeStructuralLabel)}`);
if (beforeStructuralRun.nodes["after-pose-context"].textContent !== OVERLAY_POINTER) throw new Error("the after pane adopted the before side's structural context");
assertNoHashWrites(afterStructuralRun, "overlaying an after-side structural context");

// A loop-seam context is one side's own two endpoint frames rather than one
// shared phase, so the overlay stands down for it: both panes come back and
// draw exactly the picture they draw with the overlay switched off, and the
// before label says the overlay is suspended. Any of the three ways a reader
// leaves a selection — the scrub, a `#frame=` navigation, the next played
// frame — resumes it without the box being touched.
const seamOff = run(generated, "comparison-report-data", html, data);
seamOff.nodes["before-findings"].children[seamIndex].listeners.click();
for (const resume of ["scrub", "fragment", "play"]) {
  const seamRun = run(generated, "comparison-report-data", html, data);
  toggleOverlay(seamRun, true);
  if (seamRun.nodes["after-gl"].hidden !== true) throw new Error("the overlay never took effect");
  seamRun.nodes["before-findings"].children[seamIndex].listeners.click();
  if (seamRun.nodes["after-gl"].hidden !== false) throw new Error("a selected loop-seam context left the after pane hidden, so its own judged pose could not be seen");
  for (const side of ["before", "after"]) {
    if (drawingOf(seamRun, side) !== drawingOf(seamOff, side)) throw new Error(`a suspended overlay draws the ${side} pane differently from the same selection with the overlay off`);
  }
  const seamLabel = seamRun.nodes["before-pose-context"].textContent;
  if (!seamLabel.includes("loop seam exact endpoint poses")) throw new Error(`a suspended overlay stopped the seam pane drawing its endpoint poses: ${seamLabel}`);
  if (!seamLabel.includes("overlay suspended while a loop-seam context is selected")) throw new Error(`the pane does not say the overlay stood down: ${seamLabel}`);
  if (seamOff.nodes["before-pose-context"].textContent.includes("suspended")) throw new Error("a pane says the overlay is suspended when the box was never ticked");
  if (resume === "scrub") {
    seamRun.nodes.scrub.value = "700";
    seamRun.nodes.scrub.listeners.input();
  } else if (resume === "fragment") {
    seamRun.hash.value = "#frame=700";
    seamRun.listeners.hashchange();
  } else {
    seamRun.nodes.play.listeners.click();
    if (!stepFrame(seamRun, data.before.clip.duration / 8)) throw new Error("playing scheduled no animation frame");
  }
  if (seamRun.nodes["after-gl"].hidden !== true) throw new Error(`leaving the seam context by ${resume} did not resume the overlay`);
  if (poseContext(seamRun, "before").strokeDashes.slice(SEGMENTS).some((dash) => dash !== AFTER_DASH)) throw new Error(`the overlay resumed by ${resume} does not draw the after skeleton dashed`);
  if (seamRun.nodes["before-pose-context"].textContent.includes("suspended")) throw new Error(`the overlay resumed by ${resume} still calls itself suspended`);
  assertNoHashWrites(seamRun, `suspending and resuming the overlay by ${resume}`);
}

// A side with no finite sampled position has no skeleton to draw, so the
// overlay stands down for that too, from whichever side the hole is on. Both
// panes are then drawn exactly as they are with the overlay off, each keeping
// its own availability disclosure — the side that can still be drawn is not
// taken away with the one that cannot, and neither pane restates in words
// what the other one already says.
for (const blank of ["before", "after"]) {
  const payload = JSON.parse(JSON.stringify(data));
  const buffer = Buffer.from(data[blank].clip.positions, "base64");
  for (let offset = 0; offset < buffer.length; offset += 4) buffer.writeFloatLE(Number.NaN, offset);
  payload[blank].clip.positions = buffer.toString("base64");
  const drawn = blank === "before" ? "after" : "before";
  const off = run(generated, "comparison-report-data", html, payload);
  const state = run(generated, "comparison-report-data", html, payload);
  toggleOverlay(state, true);
  if (state.nodes["after-gl"].hidden !== false) throw new Error(`an all-non-finite ${blank} side still hid the after pane behind an overlay it cannot draw`);
  if (!poseContext(state, drawn).arcs.length) throw new Error(`the ${drawn} side lost its own drawing when the ${blank} side had none`);
  if (drawingOf(state, drawn) !== drawingOf(off, drawn)) throw new Error(`a suspended overlay draws the ${drawn} pane differently from the overlay-off run`);
  if (!state.nodes[`${blank}-pose-context`].textContent.includes("pose drawing unavailable")) throw new Error(`the ${blank} pane stopped saying its sampled positions are non-finite`);
  for (const side of ["before", "after"]) {
    if (state.nodes[`${side}-pose-context`].textContent !== off.nodes[`${side}-pose-context`].textContent) throw new Error(`a suspended overlay changed the ${side} pane's label: ${JSON.stringify(state.nodes[`${side}-pose-context`].textContent)} against ${JSON.stringify(off.nodes[`${side}-pose-context`].textContent)}`);
  }
  assertNoHashWrites(state, `overlaying an all-non-finite ${blank} side`);
}

// One non-finite bone in either selected frame is a drawing to caveat rather
// than to call exact, and the overlaid pane says it for the pair whichever
// side the hole is on.
for (const holed of ["before", "after"]) {
  const payload = JSON.parse(JSON.stringify(data));
  const buffer = Buffer.from(data[holed].clip.positions, "base64");
  buffer.writeFloatLE(Number.NaN, (1501 * bones * 3 + 1 * 3) * 4);
  payload[holed].clip.positions = buffer.toString("base64");
  const state = run(generated, "comparison-report-data", html, payload);
  toggleOverlay(state, true);
  state.nodes.scrub.value = "1501";
  state.nodes.scrub.listeners.input();
  const label = state.nodes["before-pose-context"].textContent;
  if (!label.includes("selected frame contains non-finite positions")) throw new Error(`the overlay called a pair with a non-finite ${holed} frame exact: ${label}`);
  if (label.includes("exact judged pose-grid frames")) throw new Error(`the overlay claims two exact frames where the ${holed} one is not: ${label}`);
  // The other side's frame is finite, and says so again once the two panes
  // are drawn separately.
  const whole = holed === "before" ? "after" : "before";
  toggleOverlay(state, false);
  if (state.nodes[`${whole}-pose-context`].textContent !== "exact judged pose-grid frame") throw new Error(`the ${whole} pane's own exact frame is mislabelled: ${state.nodes[`${whole}-pose-context`].textContent}`);
  if (!state.nodes[`${holed}-pose-context`].textContent.includes("selected frame contains non-finite positions")) throw new Error(`the ${holed} pane stopped disclosing its own non-finite frame`);
  assertNoHashWrites(state, `overlaying a pair whose ${holed} frame is not finite`);
}

// The overlay is a canvas drawing like the panes it replaces, so a system
// theme change repaints it through the same palette resolution.
const overlayTokens = (accent, warning) => tokenStyles({
  ground: "#101010", surface: "#1e1e2a", raised: "#232331", ink: "#123456",
  muted: "#9099b2", line: "#3a3a4e", accent, error: "#202122",
  warning, pass: "#445566", note: "#6b7390",
});
const overlayTheme = run(generated, "comparison-report-data", html, data, { styles: overlayTokens("#0a0b0c", "#101112") });
toggleOverlay(overlayTheme, true);
const overlayPaint = () => new Set(poseContext(overlayTheme, "before").strokes);
if (!overlayPaint().has("#0a0b0c") || !overlayPaint().has("#101112")) throw new Error("the overlay did not paint its two skeletons from the accent and warning tokens");
overlayTheme.settings.styles = overlayTokens("#aabbcc", "#ddeeff");
overlayTheme.media.change();
const overlayRepainted = overlayPaint();
if (!overlayRepainted.has("#aabbcc") || !overlayRepainted.has("#ddeeff")) throw new Error("a system theme change did not repaint the overlay with the new tokens");
if (overlayRepainted.has("#0a0b0c") || overlayRepainted.has("#101112")) throw new Error("a system theme change left the overlay painted in the old tokens");
assertNoHashWrites(overlayTheme, "an overlaid comparison theme change");

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
// The overlay draws the same grid the panes do, so a document that carries
// none disables it too, and using it anyway decodes and draws nothing.
if (!/<input type="checkbox" id="overlay"[^>]*\sdisabled/.test(comparisonEvidenceHtml)) throw new Error("the evidence-only comparison leaves the overlay enabled in its markup");
if (!evidenceNodes.overlay.disabled) throw new Error("the overlay stayed available with no pose grid to overlay");
evidenceNodes.overlay.checked = true;
evidenceNodes.overlay.listeners.change();
for (const side of ["before", "after"]) {
  if (evidenceNodes[`${side}-pose-context`].textContent !== "") throw new Error(`an evidence-only comparison labelled its ${side} pose pane from a grid it does not carry`);
}
if (evidenceRun.decoded.count !== 0) throw new Error(`an evidence-only comparison decoded ${evidenceRun.decoded.count} grid(s) it does not carry`);
if (evidenceNodes["before-findings"].children.length !== evidencePayload.before.findings.length) throw new Error("switching on the overlay of an evidence-only comparison dropped its findings");
evidenceNodes.play.listeners.click();
if (evidenceNodes.play.textContent === "⏸") throw new Error("an evidence-only comparison started playing a grid it does not carry");
if (evidenceRun.clock.pending.size) throw new Error("an evidence-only comparison scheduled an animation frame");
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

// Selecting another clip through the fragment is the same ask as a frame or
// a finding: a running report would put the first clip's next frame over it.
const clipWhilePlaying = runMulti();
clipWhilePlaying.nodes.play.listeners.click();
if (!clipWhilePlaying.clock.pending.size) throw new Error("the multi-clip playback fixture never started playing");
navigate(clipWhilePlaying, `#clip=${encodeURIComponent(secondClip.name)}`);
if (clipWhilePlaying.nodes.play.textContent !== "▶") throw new Error("navigating to another clip did not pause the multi-clip report");
if (clipWhilePlaying.nodes["clip-select"].value !== secondClip.name) throw new Error("navigating to another clip did not select it");
const selectedFrame = Number(clipWhilePlaying.nodes.scrub.value);
stepFrame(clipWhilePlaying, 1);
stepFrame(clipWhilePlaying, 1);
if (clipWhilePlaying.nodes["clip-select"].value !== secondClip.name) throw new Error("a frame callback moved off the deep-linked clip");
if (Number(clipWhilePlaying.nodes.scrub.value) !== selectedFrame) throw new Error(`a frame callback overwrote the frame the deep-linked clip opened on: ${clipWhilePlaying.nodes.scrub.value} against ${selectedFrame}`);
assertNoHashWrites(clipWhilePlaying, "deep-linking a clip while playing");

assertNoHashWrites(selectionNav, "navigating the single-clip viewer");
assertNoHashWrites(clipNav, "navigating clips");
assertNoHashWrites(evidenceSingle, "scrubbing an evidence-only report");
assertNoHashWrites(evidenceRun, "an evidence-only comparison");

// ---- gait-group figures ------------------------------------------------
// A declared gait group draws every member on one figure, so that figure is
// evidence about the document rather than about one clip: it stays visible
// whichever member the reader selects, while a member's own gait chart is
// shown only on its own clip. Its playhead follows the phase being scrubbed,
// because the axis every member is drawn on is that same normalized phase.
const group = singleReportParts(groupHtml);
const groupPayload = JSON.parse(group.payload);
if (groupPayload.clips.length < 2) throw new Error("the gait-group fixture must embed more than one member");
const groupFigures = documentCharts(groupHtml).filter((figure) => figure.dataset.kind === "gait-group");
if (groupFigures.length !== 1) throw new Error(`the gait-group fixture must render exactly one group figure, saw ${groupFigures.length}`);
if (groupFigures[0].dataset.clip !== undefined) throw new Error("a group figure must carry no data-clip, or it would be hidden with its clip");
if (groupFigures[0].dataset.group !== "run-ring") throw new Error(`the group figure names its group: ${groupFigures[0].dataset.group}`);
if (groupFigures[0].dataset.members !== undefined) throw new Error("membership is payload data, not a delimited attribute a clip name can break");
const declaredGroup = (groupPayload.groups || []).find((g) => g.name === groupFigures[0].dataset.group);
if (!declaredGroup) throw new Error(`the payload declares the figure's group: ${groupFigures[0].dataset.group}`);
const groupMembers = declaredGroup.members;
if (groupMembers.length < 2) throw new Error(`the group names its members: ${JSON.stringify(groupMembers)}`);
const groupPad = Number(groupFigures[0].dataset.pad), groupPlotW = Number(groupFigures[0].dataset.plotw);
if (!(groupPad > 0) || !(groupPlotW > 0)) throw new Error("the group figure does not publish its plot rectangle");

const figureOf = (state, select) => {
  const figure = state.charts.find(select);
  if (!figure) throw new Error("the run does not carry the figure under test");
  return figure;
};
const groupFigureOf = (state) => figureOf(state, (figure) => figure.dataset.kind === "gait-group");
const memberGaitOf = (state, name) => figureOf(state, (figure) => figure.dataset.kind === "gait" && figure.dataset.clip === name);
const groupPlayheadOf = (state) => {
  // An unplaced playhead reads as `undefined`, and every comparison against
  // NaN is false: a viewer that stopped moving this figure's playhead would
  // otherwise satisfy every assertion below.
  const at = groupFigureOf(state).query[".playhead"].attrs.x1;
  if (!Number.isFinite(Number(at))) throw new Error(`the group figure's playhead was never placed: ${at}`);
  return Number(at);
};
const groupRun = runSingle(group, groupHtml, groupPayload);
const [firstMember, secondMember] = groupPayload.clips;
if (firstMember.name === secondMember.name) throw new Error("the gait-group fixture must embed distinguishable members");
if (groupFigureOf(groupRun).style.display === "none") throw new Error("the group figure is hidden on the clip the report opens with");
if (memberGaitOf(groupRun, secondMember.name).style.display !== "none") throw new Error("an unselected member's own gait chart stayed visible");
groupRun.nodes["clip-select"].value = secondMember.name;
groupRun.nodes["clip-select"].listeners.change();
if (groupFigureOf(groupRun).style.display === "none") throw new Error("selecting another member hid the group figure");
if (memberGaitOf(groupRun, firstMember.name).style.display !== "none") throw new Error("the previously selected member's own gait chart stayed visible");
if (memberGaitOf(groupRun, secondMember.name).style.display === "none") throw new Error("the selected member's own gait chart is hidden");
if (Math.abs(groupPlayheadOf(groupRun) - groupPad) > 1e-6) throw new Error(`the group playhead did not return to phase 0 with the new member: ${groupPlayheadOf(groupRun)}`);
const groupLastFrame = secondMember.frames - 1;
groupRun.nodes.scrub.value = String(groupLastFrame);
groupRun.nodes.scrub.listeners.input();
if (Math.abs(groupPlayheadOf(groupRun) - (groupPad + groupPlotW)) > 1e-6) throw new Error(`scrubbing to the last frame left the group playhead at ${groupPlayheadOf(groupRun)}`);
if (groupFigureOf(groupRun).style.display === "none") throw new Error("scrubbing hid the group figure");
const groupMidFrame = Math.round(groupLastFrame / 2);
groupRun.nodes.scrub.value = String(groupMidFrame);
groupRun.nodes.scrub.listeners.input();
const groupExpected = groupPad + groupPlotW * (groupMidFrame / groupLastFrame);
if (Math.abs(groupPlayheadOf(groupRun) - groupExpected) > 1e-6) throw new Error(`the group playhead follows the scrubbed phase: ${groupPlayheadOf(groupRun)} against ${groupExpected}`);

// A group figure's axis is the stride cycle its members were measured on,
// which is not the clip's frame count when the grid has no duplicate wrap
// sample to drop. The committed fixture's cycle is `frames - 1`, where the
// two agree, so the short-grid case is introduced here in the payload.
const shortCycle = JSON.parse(JSON.stringify(groupPayload));
shortCycle.clips[0].cycle = shortCycle.clips[0].frames;
const shortRun = runSingle(group, groupHtml, shortCycle);
const shortLast = shortCycle.clips[0].frames - 1;
shortRun.nodes.scrub.value = String(shortLast);
shortRun.nodes.scrub.listeners.input();
const onCycle = groupPad + groupPlotW * (shortLast / shortCycle.clips[0].cycle);
const onFrames = groupPad + groupPlotW;
if (Math.abs(onCycle - onFrames) < 1) throw new Error("the harness failed to make the two axes differ");
if (Math.abs(groupPlayheadOf(shortRun) - onCycle) > 1e-6) throw new Error(`the group playhead follows the members' cycle, not the clip's frame count: ${groupPlayheadOf(shortRun)} against ${onCycle}`);

// A declared member whose own name carries the separator a delimited
// attribute would have used is still its group's member. Encoded as
// "a,b" in an attribute it would split into two names, neither of them
// itself, and its group's figure would vanish while it is selected.
const awkward = JSON.parse(JSON.stringify(groupPayload));
const comma = "run,left & \"quoted\"";
const renamed = awkward.clips[1].name;
awkward.clips[1].name = comma;
for (const group of awkward.groups) group.members = group.members.map((m) => (m === renamed ? comma : m));
const awkwardRun = runSingle(group, groupHtml, awkward);
awkwardRun.nodes["clip-select"].value = comma;
awkwardRun.nodes["clip-select"].listeners.change();
if (awkwardRun.nodes["clip-select"].value !== comma) throw new Error("the harness failed to select the awkwardly named member");
if (groupFigureOf(awkwardRun).style.display === "none") throw new Error("a member whose name carries the list separator lost its own group figure");

// A clip outside the group is not what the figure draws, and its phase is
// not the axis the caption describes. Selecting it hides the figure rather
// than leaving it on screen driven by a clip it does not plot. The fixture
// is all members, so the non-member is introduced here, in the payload the
// viewer reads.
const outsider = JSON.parse(JSON.stringify(groupPayload));
// A name carrying the separator a delimited attribute would have used, so a
// membership encoded that way would split it and lose its own figure.
const stranger = "clip,outside,the-ring";
if (groupMembers.includes(stranger)) throw new Error("the harness picked a name the group declares");
outsider.clips.push(Object.assign({}, outsider.clips[0], {name: stranger}));
const outsiderRun = runSingle(group, groupHtml, outsider);
outsiderRun.nodes["clip-select"].value = stranger;
outsiderRun.nodes["clip-select"].listeners.change();
if (outsiderRun.nodes["clip-select"].value !== stranger) throw new Error("the harness failed to select the non-member");
if (groupFigureOf(outsiderRun).style.display !== "none") throw new Error("a clip outside the group left its figure visible, with a playhead driven by a phase the figure does not plot");
outsiderRun.nodes["clip-select"].value = firstMember.name;
outsiderRun.nodes["clip-select"].listeners.change();
if (groupFigureOf(outsiderRun).style.display === "none") throw new Error("selecting a member again did not restore the group figure");

assertNoHashWrites(groupRun, "selecting a member of a gait group");

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

// The single-clip viewer's frame loop has one owner too. Its transport is
// older than the comparison's and carried the same latent chain: every
// pause-then-play left the previous loop scheduled, so the clip advanced once
// more per frame for as long as the report stayed open.
const singlePlay = runSingle(single, singleHtml, singlePayload);
singlePlay.nodes.play.listeners.click();
stepFrame(singlePlay, 0.01);
singlePlay.nodes.play.listeners.click();
if (singlePlay.clock.pending.size !== 0) throw new Error(`pausing the single-clip viewer left ${singlePlay.clock.pending.size} animation frame(s) scheduled`);
singlePlay.nodes.play.listeners.click();
for (const cycle of [1, 2, 3, 4]) {
  const ran = stepFrame(singlePlay, 0.01);
  if (ran !== 1) throw new Error(`after ${cycle} restart cycle(s) the single-clip viewer ran ${ran} callbacks in one frame`);
  if (singlePlay.clock.pending.size !== 1) throw new Error(`after ${cycle} restart cycle(s) the single-clip viewer has ${singlePlay.clock.pending.size} frame loops scheduled`);
  singlePlay.nodes.play.listeners.click();
  singlePlay.nodes.play.listeners.click();
}
singlePlay.nodes.play.listeners.click();
// Scrubbing pauses it, and pausing cancels the frame it had scheduled.
singlePlay.nodes.play.listeners.click();
stepFrame(singlePlay, 0.01);
singlePlay.nodes.scrub.value = "1";
singlePlay.nodes.scrub.listeners.input();
if (singlePlay.nodes.play.textContent !== "▶" || singlePlay.clock.pending.size !== 0) throw new Error("scrubbing the single-clip viewer did not stop and cancel its frame loop");
assertNoHashWrites(singlePlay, "restarting single-clip playback");

// A deep link is a reader asking for one position, and the single-clip
// viewer's transport would have overwritten it on the very next frame. Each
// selector pauses, and the frame it asked for survives a frame callback.
for (const [what, fragment, expected] of [
  ["a frame", `#frame=${Math.min(3, lastFrame)}`, Math.min(3, lastFrame)],
  ["a finding", `#finding=${findingIndex}`, null],
]) {
  const deepLinked = runSingle(single, singleHtml, singlePayload);
  deepLinked.nodes.play.listeners.click();
  if (!deepLinked.clock.pending.size) throw new Error(`the ${what} fixture never started playing`);
  navigate(deepLinked, fragment);
  if (deepLinked.nodes.play.textContent !== "▶") throw new Error(`navigating to ${what} did not pause the single-clip viewer`);
  const landed = Number(deepLinked.nodes.scrub.value);
  if (expected !== null && landed !== expected) throw new Error(`navigating to ${what} landed on frame ${landed}, not ${expected}`);
  stepFrame(deepLinked, 1);
  stepFrame(deepLinked, 1);
  if (Number(deepLinked.nodes.scrub.value) !== landed) throw new Error(`a frame callback overwrote the frame ${what} selected: ${deepLinked.nodes.scrub.value} against ${landed}`);
  if (expected === null && !deepLinked.nodes.findings.children[findingIndex].classes.has("selected")) throw new Error("a frame callback cleared the deep-linked finding");
  assertNoHashWrites(deepLinked, `deep-linking ${what} while playing`);
}
// Clicking a finding in a running report is the same ask.
const clickedWhilePlaying = runSingle(single, singleHtml, singlePayload);
clickedWhilePlaying.nodes.play.listeners.click();
clickedWhilePlaying.nodes.findings.children[findingIndex].listeners.click();
if (clickedWhilePlaying.nodes.play.textContent !== "▶") throw new Error("clicking a finding did not pause the single-clip viewer");
const clickedFrame = Number(clickedWhilePlaying.nodes.scrub.value);
stepFrame(clickedWhilePlaying, 1);
if (Number(clickedWhilePlaying.nodes.scrub.value) !== clickedFrame) throw new Error("a frame callback overwrote the frame a clicked finding selected");
assertNoHashWrites(clickedWhilePlaying, "clicking a finding while playing");
if (!/<button id="play"[^>]*aria-label="[^"]+"/.test(singleHtml)) throw new Error("the single-clip play control has no accessible name");
if (singlePlay.nodes.play.attrs["aria-label"] !== "Play the clip") throw new Error(`a paused single-clip control is named ${JSON.stringify(singlePlay.nodes.play.attrs["aria-label"])}`);

// A cancellation the browser ignores must not accumulate chains either: the
// run number retires the old loop on its own.
for (const [what, parts, id, document_, payload, options] of [
  ["comparison", generated, "comparison-report-data", html, data, {ignoreCancel: true}],
  ["single-clip", single, "report-data", singleHtml, singlePayload, {ignoreCancel: true}],
]) {
  // A pause the browser did not act on still stops the phase: the callback
  // it kept comes back, sees the loop stopped, and ends there.
  const ignored = run(parts, id, document_, payload, options);
  ignored.nodes.play.listeners.click();
  stepFrame(ignored, 0.01);
  ignored.nodes.play.listeners.click();
  const parked = Number(ignored.nodes.scrub.value);
  stepFrame(ignored, 1);
  stepFrame(ignored, 1);
  if (Number(ignored.nodes.scrub.value) !== parked) throw new Error(`the ${what} viewer kept advancing after a pause its browser ignored: ${ignored.nodes.scrub.value} against ${parked}`);
  if (ignored.clock.pending.size) throw new Error(`a stopped ${what} loop scheduled ${ignored.clock.pending.size} more frame(s)`);

  const uncancelled = run(parts, id, document_, payload, options);
  uncancelled.nodes.play.listeners.click();
  for (const cycle of [1, 2, 3, 4]) {
    // A retired callback still runs once — the browser kept it — but it must
    // not schedule a successor, so exactly one loop survives each frame
    // however many pause/play cycles preceded it.
    stepFrame(uncancelled, 0.01);
    if (uncancelled.clock.pending.size !== 1) throw new Error(`after ${cycle} restart cycle(s) with cancellation ignored, the ${what} viewer has ${uncancelled.clock.pending.size} frame loops scheduled: its run number does not retire the old one`);
    uncancelled.nodes.play.listeners.click();
    uncancelled.nodes.play.listeners.click();
  }
}

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
