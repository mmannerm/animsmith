"use strict";
// Executes the Rust-generated report viewers headlessly against the exact
// documents `just report-browser` produced: both report forms, each in its
// full and evidence-only shape. The DOM and WebGL stubs are deliberately
// thin — everything asserted here is something a reader would see.
const fs = require("fs"), vm = require("vm");
if (process.argv.length !== 6) {
  throw new Error("usage: test-report-viewers.js COMPARISON.html COMPARISON-EVIDENCE.html REPORT.html REPORT-EVIDENCE.html");
}
const [, , comparisonPath, comparisonEvidencePath, singlePath, singleEvidencePath] = process.argv;
const html = fs.readFileSync(comparisonPath, "utf8");
const comparisonEvidenceHtml = fs.readFileSync(comparisonEvidencePath, "utf8");
const singleHtml = fs.readFileSync(singlePath, "utf8");
const singleEvidenceHtml = fs.readFileSync(singleEvidencePath, "utf8");

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
    if (!this.context) this.context={arcs:[],fillStyle:null,setTransform(){},clearRect(){this.arcs=[]},beginPath(){},moveTo(){},lineTo(){},stroke(){},arc(...args){this.arcs.push({args,fillStyle:this.fillStyle})},fill(){}};
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

// One runner for every viewer execution: a fresh document whose elements are
// exactly the ids the generated markup carries, so an absent pose surface is
// absent here too.
function run(parts, dataId, ids, payload, options) {
  const settings = options || {};
  const nodes = Object.fromEntries(ids.map(id => [id, new Node(id)]));
  nodes[dataId] = new Node(dataId);
  nodes[dataId].textContent = JSON.stringify(payload);
  const listeners = {};
  const root = new Node("documentElement");
  const charts = settings.charts || [];
  const context = {
    document: {
      documentElement: root,
      getElementById: id => nodes[id] || null,
      createElement: () => new Node(),
      createTextNode: text => { const node = new Node(); node.textContent = text; return node; },
      createElementNS: (_ns, tag) => { const node = new Node(); node.tag = tag; return node; },
      querySelectorAll: () => charts,
    },
    window: { addEventListener(kind, handler) { listeners[kind] = handler; }, devicePixelRatio: 1 },
    location: { hash: settings.hash || "" },
    getComputedStyle: () => settings.styles || noStyles,
    performance: { now: () => 0 }, requestAnimationFrame: () => 0,
    atob: value => Buffer.from(value, "base64").toString("binary"),
    Uint8Array, Float32Array, Buffer, Math, Map, Set, Array, Number, Object, Infinity, JSON, console,
  };
  vm.createContext(context);
  vm.runInContext(`${parts.shared}\n${parts.viewer}`, context);
  return { nodes, root, listeners, context, charts };
}

// SVG shows no text unless an element carries it, so no panel may explain
// itself by assigning to an <svg>'s own textContent.
function assertNoBareSvgText(nodes, svgIds, why) {
  for (const id of svgIds) {
    const node = nodes[id];
    if (node && node.textContent !== "") throw new Error(`${why}: ${id} carries a bare text node instead of a <text> element`);
  }
}

const comparisonIds = ["mapping","scrub","times","comparison-root-path","clip-before","clip-after","before-gl","after-gl","before-pose-context","after-pose-context","before-path","after-path","before-gait","after-gait","before-contexts","after-contexts","before-identity","after-identity","before-findings","after-findings","before-gaps","after-gaps","before-predictions","after-predictions"];
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

const main = run(generated, "comparison-report-data", comparisonIds, data);
const nodes = main.nodes, windowListeners = main.listeners, documentElement = main.root, context = main.context;
const palette = context.animsmithPalette();
if(nodes.scrub.max !== 2001 || !nodes["before-findings"].children.some(child=>child.textContent.includes("<img>"))) throw new Error("viewer did not retain exact frames or safe finding text");
if(!nodes["before-identity"].textContent.includes(data.before.dependency_closure_identity.sha256) || !nodes["after-identity"].textContent.includes(data.after.dependency_closure_identity.sha256)) throw new Error("viewer does not disclose complete closure identities");
if(!nodes.times.textContent.includes("before 0.000s") || !nodes.times.textContent.includes("after 0.000s") || !nodes.times.textContent.includes("not a time warp")) throw new Error("shared phase omits source times or no-warp disclosure");
if(!nodes["comparison-root-path"].children.some(child=>child.textContent==="before root path") || !nodes["comparison-root-path"].children.some(child=>child.textContent==="after root path")) throw new Error("shared root chart lacks textual before/after legends");
if(!nodes["before-path"].children.some(child=>child.attrs["data-role"]==="left_foot") || !nodes["before-path"].children.some(child=>child.attrs["data-role"]==="right_foot")) throw new Error("role trail chart omits foot trajectories");
if(!nodes["before-gait"].children.some(child=>child.attrs["data-stance-side"]==="left")) throw new Error("gait chart omits typed stance interval");
assertNoBareSvgText(nodes, comparisonSvgs, "drawn comparison");
const seamIndex = data.before.findings.indexOf(seamFinding), structuralIndex = data.before.findings.indexOf(structuralFinding), afterIndex = data.after.findings.indexOf(afterFinding);
nodes["before-findings"].children[seamIndex].listeners.click();
if(nodes.scrub.value != 1501 || !nodes["before-pose-context"].textContent.includes("first 0.000s") || !nodes["before-pose-context"].textContent.includes(`affected ${seam.subject_bone_name}`)) throw new Error("seam finding did not select exact frame and endpoint/subject context");
if(!nodes["before-gl"].context.arcs.some(row=>row.args[2]===6 && row.fillStyle===palette.error)) throw new Error("finding did not highlight its Rust-projected bone with the subject token");
nodes["before-findings"].children[structuralIndex].listeners.click();
if(!nodes["before-pose-context"].textContent.includes("structural evidence") || !nodes["before-contexts"].children.some(child=>child.className.includes("structural"))) throw new Error("structural finding was not distinguished from visible pose evidence");
nodes["after-findings"].children[afterIndex].listeners.click(); if(nodes.scrub.value != 1234) throw new Error("after finding did not select exact frame");
context.location.hash=`#time-before-${seamFinding.anchor.replace(/^finding-/, "")}`; windowListeners.hashchange();
if(nodes.scrub.value != 1501) throw new Error("semantic time anchor did not select its finding");
context.location.hash=`#finding-after-${afterFinding.anchor.replace(/^finding-/, "")}`; windowListeners.hashchange();
if(nodes.scrub.value != 1234) throw new Error("cross-side semantic finding anchor did not select its finding");

// Fragment options an embedded comparison honours, their persistence across
// the document's own anchor links, and their removal.
context.location.hash="#embed=1&theme=light&frame=1200&clip=ignored&finding=3"; windowListeners.hashchange();
if(documentElement.attrs["data-embed"] !== "1" || documentElement.attrs["data-theme"] !== "light" || nodes.scrub.value != 1200) throw new Error("comparison viewer ignored embed/theme/frame fragment options");
context.location.hash=`#finding-after-${afterFinding.anchor.replace(/^finding-/, "")}`; windowListeners.hashchange();
if(documentElement.attrs["data-embed"] !== "1" || documentElement.attrs["data-theme"] !== "light") throw new Error("following the document's own finding anchor un-pinned the embedded theme");
if(nodes.scrub.value != 1234) throw new Error("the anchor link stopped selecting its finding once a theme was pinned");
context.location.hash="#frame=999999999"; windowListeners.hashchange();
if(nodes.scrub.value != 2001) throw new Error("an out-of-range frame was not clamped to the shared phase");
if(documentElement.attrs["data-theme"] !== "light") throw new Error("a fragment that never mentions the theme must leave it pinned");
context.location.hash="#theme=neon&embed=0"; windowListeners.hashchange();
if("data-embed" in documentElement.attrs || "data-theme" in documentElement.attrs) throw new Error("an explicitly unusable value must restore the document default");

// A non-finite sampled range must degrade the drawing, not abort navigation
// or hide the already-rendered findings and coverage lists.
const cleanBefore = data.before.clip.positions, cleanAfter = data.after.clip.positions;
const execute = payload => run(generated, "comparison-report-data", comparisonIds, payload).nodes;
const svgText = (node) => node.children.map(child => child.textContent).join(" ");
const invalid = Buffer.from(data.before.clip.positions, "base64");
for (let offset = 0; offset < invalid.length; offset += 4) invalid.writeFloatLE(Number.NaN, offset);
data.before.clip.positions = invalid.toString("base64");
data.after.clip.positions = cleanAfter;
const isolatedNodes = execute(data);
const rootLabels = isolatedNodes["comparison-root-path"].children.map(child=>child.textContent);
const beforeTrailText = svgText(isolatedNodes["before-path"]);
const afterTrailText = svgText(isolatedNodes["after-path"]);
if (!isolatedNodes["before-pose-context"].textContent.includes("non-finite") || !svgText(isolatedNodes["before-gait"]).includes("non-finite") || !isolatedNodes["after-pose-context"].textContent.includes("exact judged") || !rootLabels.includes("before root unavailable") || !rootLabels.includes("after root path") || !beforeTrailText.includes("unavailable:") || !beforeTrailText.includes("non-finite") || afterTrailText.includes("non-finite") || isolatedNodes["before-findings"].children.length !== data.before.findings.length) throw new Error("before-side non-finite pose/gait/root/trail evidence was mislabeled, hidden, or threw");
assertNoBareSvgText(isolatedNodes, comparisonSvgs, "non-finite before side");

data.before.clip.positions = cleanBefore; data.after.clip.positions = invalid.toString("base64");
const reverseNodes = execute(data);
const reverseTrailText = svgText(reverseNodes["after-path"]);
if (!reverseNodes["before-pose-context"].textContent.includes("exact judged") || !reverseNodes["after-pose-context"].textContent.includes("non-finite") || !reverseTrailText.includes("unavailable:") || !reverseTrailText.includes("non-finite")) throw new Error("after-side non-finite evidence did not remain independent of exact before evidence");

// A selected mixed-finite frame also loses the exact-evidence label while
// other finite frames and the opposite side remain independently available.
const mixed = Buffer.from(cleanAfter, "base64");
mixed.writeFloatLE(Number.NaN, (1501 * bones * 3 + 1 * 3) * 4);
data.before.clip.positions = cleanBefore; data.after.clip.positions = mixed.toString("base64");
const mixedNodes = execute(data);
mixedNodes.scrub.value=1501; mixedNodes.scrub.listeners.input();
const mixedTrailText = svgText(mixedNodes["after-path"]);
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
for (const surface of ["before-gl", "after-gl", "comparison-root-path"]) {
  if (comparisonEvidenceHtml.includes(`id="${surface}"`)) throw new Error(`${surface} is still rendered in an evidence-only comparison`);
  if (!comparisonEvidenceHtml.includes(`id="${surface}-notice"`)) throw new Error(`${surface} lost its omission notice`);
}
const evidenceIds = comparisonIds.filter(id => !["before-gl", "after-gl", "comparison-root-path"].includes(id));
const evidenceRun = run(comparisonEvidence, "comparison-report-data", evidenceIds, evidencePayload);
const evidenceNodes = evidenceRun.nodes;
if (!evidenceNodes.scrub.disabled) throw new Error("the shared phase stayed scrubbable with no pose grid behind it");
if (evidenceNodes["before-findings"].children.length !== evidencePayload.before.findings.length || !evidenceNodes["before-identity"].textContent.includes(evidencePayload.before.dependency_closure_identity.sha256)) throw new Error("an evidence-only comparison dropped findings or identities");
if (!evidenceNodes.times.textContent.includes("not a time warp") || !evidenceNodes.mapping.textContent) throw new Error("an evidence-only comparison dropped its phase disclosures");
for (const id of ["before-path","after-path","before-gait","after-gait"]) {
  if (evidenceNodes[id].children.length) throw new Error(`${id} drew from a pose grid the document does not carry`);
}
assertNoBareSvgText(evidenceNodes, ["before-path","after-path","before-gait","after-gait"], "evidence-only comparison");
// Navigating an evidence-only comparison stays inert: the theme still
// applies, and no panel starts drawing from a grid the document lacks.
evidenceRun.context.location.hash = "#frame=5&theme=light";
evidenceRun.listeners.hashchange();
if (evidenceRun.root.attrs["data-theme"] !== "light") throw new Error("an evidence-only comparison stopped honouring the theme option");
for (const id of ["before-path","after-path","before-gait","after-gait"]) {
  if (evidenceNodes[id].children.length) throw new Error(`${id} drew after a navigation without a pose grid`);
}

// ---- fragment parser ---------------------------------------------------
// One parser serves both documents, so it is exercised once, directly, with
// valid, invalid, and hostile fragments. Nothing here may throw, and a key
// that never appears must stay absent so navigation cannot silently reset a
// switch the reader pinned.
const parse = context.animsmithFragmentOptions;
const KEYS = ["embed", "theme", "clip", "frame", "finding"];
function expectOptions(hash, expected, why) {
  let actual;
  try { actual = parse(hash); } catch (error) { throw new Error(`fragment ${JSON.stringify(String(hash).slice(0,40))} threw: ${error}`); }
  for (const key of KEYS) {
    const want = Object.prototype.hasOwnProperty.call(expected, key) ? expected[key] : undefined;
    if (!Object.is(actual[key], want)) throw new Error(`${why}: ${key} was ${JSON.stringify(actual[key])}, expected ${JSON.stringify(want)}`);
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
// The length bound itself: the last accepted character and the first
// rejected one.
const tail = "&theme=light";
const filler = "#" + "p=1&".repeat(200);
const accepted = filler + "x=" + "y".repeat(4096 - filler.length - tail.length - 2) + tail;
if (accepted.length !== 4096) throw new Error(`harness built a ${accepted.length}-character boundary fragment`);
expectOptions(accepted, {theme:"light"}, "a fragment at the exact length bound");
expectOptions("x" + accepted, {}, "a fragment one character past the bound");

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

// The chart the viewer syncs is described by the document itself, so the
// harness reads its hooks rather than restating them.
const figure = singleHtml.match(/<figure class="chart"[^>]*data-kind="gait"[^>]*>/);
if (!figure) throw new Error("the single-clip document carries no gait chart to sync");
const chartAttrs = Object.fromEntries([...figure[0].matchAll(/([a-z-]+)="([^"]*)"/g)].map(m => [m[1], m[2]]));
const chartPad = Number(chartAttrs["data-pad"]), chartPlotW = Number(chartAttrs["data-plotw"]);
if (!(chartPad > 0) || !(chartPlotW > 0)) throw new Error("the gait chart does not publish its plot rectangle");
const singleIds = ["file","clip-select","play","scrub","time","gl","findings","gaps","predictions"];
function runSingle(parts, payload, ids, settings) {
  const chart = new Node("chart");
  chart.dataset = {clip: payload.clips[0].name, kind: "gait", pad: String(chartPad), plotw: String(chartPlotW)};
  chart.query[".playhead"] = new Node("playhead");
  return run(parts, "report-data", ids, payload, Object.assign({charts: [chart]}, settings || {}));
}

const plain = runSingle(single, singlePayload, singleIds);
if (!plain.nodes.file.textContent.includes(singlePayload.file || "")) throw new Error("the viewer did not disclose its source file");
if (plain.nodes.findings.children.length !== singlePayload.findings.length) throw new Error("the findings panel dropped rows");
if (!plain.nodes.findings.children.map(row => row.children.map(part => part.textContent).join("|")).some(text => text.includes("<img src=x>"))) throw new Error("untrusted finding text was not carried as text");
if (!plain.nodes.gl.gl.clears.length) throw new Error("the WebGL view never cleared a frame");
if ("data-theme" in plain.root.attrs || "data-embed" in plain.root.attrs) throw new Error("an empty fragment must leave the document defaults alone");

// The 3D view paints from the live tokens: bones, joints, trails, and the
// clear colour all come from the palette the document resolves.
const themedTokens = {ground:"#F4F5F9", muted:"#112233", ink:"#445566", pass:"#010203", accent:"#0A0B0C", warning:"#101112", error:"#202122", raised:"#eef0f6", surface:"#ffffff", line:"#d9deea", note:"#6b7390"};
const rgb = hex => [1,3,5].map(offset => parseInt(hex.slice(offset, offset + 2), 16) / 255);
const themed = runSingle(single, singlePayload, singleIds, {hash:"#theme=light", styles: tokenStyles(themedTokens)});
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

// A deep link selects exactly what the equivalent click selects.
const deep = runSingle(single, singlePayload, singleIds, {hash:`#finding=${findingIndex}`});
const clicked = runSingle(single, singlePayload, singleIds);
clicked.nodes.findings.children[findingIndex].listeners.click();
const observable = run => ({
  frame: String(run.nodes.scrub.value),
  time: run.nodes.time.textContent,
  selected: run.nodes.findings.children[findingIndex].classes.has("selected"),
  playhead: run.charts[0].query[".playhead"].attrs.x1,
});
const viaFragment = observable(deep), viaClick = observable(clicked);
for (const key of Object.keys(viaFragment)) {
  if (viaFragment[key] !== viaClick[key]) throw new Error(`#finding=${findingIndex} and clicking row ${findingIndex} disagree on ${key}: ${viaFragment[key]} vs ${viaClick[key]}`);
}
if (!viaClick.selected) throw new Error("selecting a finding does not mark its row");

// The playhead spans exactly the rectangle the chart publishes.
const atStart = runSingle(single, singlePayload, singleIds, {hash:"#frame=0"});
if (Number(atStart.charts[0].query[".playhead"].attrs.x1) !== chartPad) throw new Error("frame 0 does not place the playhead at the plot origin");
const atEnd = runSingle(single, singlePayload, singleIds, {hash:`#frame=${lastFrame}`});
if (Math.abs(Number(atEnd.charts[0].query[".playhead"].attrs.x1) - (chartPad + chartPlotW)) > 1e-6) throw new Error("the last judged frame does not place the playhead at the plot's right edge");

// Embed and theme reach the document; an unknown clip, an out-of-range
// frame, and a hostile fragment leave a usable report behind.
const embedded = runSingle(single, singlePayload, singleIds, {hash:`#embed=1&theme=light&clip=${encodeURIComponent(singleClip.name)}&frame=${Math.min(2, lastFrame)}`});
if (embedded.root.attrs["data-theme"] !== "light" || embedded.root.attrs["data-embed"] !== "1") throw new Error("the single-clip viewer ignored embed/theme");
if (Number(embedded.nodes.scrub.value) !== Math.min(2, lastFrame)) throw new Error("a deep-linked frame did not scrub the viewer");
if (embedded.nodes["clip-select"].value !== singleClip.name) throw new Error("a deep-linked clip was not selected");
for (const hostile of [
  "#frame=999999999", "#frame=-1", "#clip=%E0%A4%A", "#clip=no-such-clip", "#finding=999999999",
  "#finding=" + "9".repeat(400), "#theme=%3Cimg%3E", "#embed=maybe", "#unknown=1",
  "#finding-before-abcdef0123456789", "#" + "k=v&".repeat(3000),
]) {
  let hostileRun;
  try { hostileRun = runSingle(single, singlePayload, singleIds, {hash: hostile}); }
  catch (error) { throw new Error(`fragment ${JSON.stringify(hostile.slice(0,24))} threw in the viewer: ${error}`); }
  const frame = Number(hostileRun.nodes.scrub.value);
  if (!Number.isInteger(frame) || frame < 0 || frame > lastFrame) throw new Error(`fragment ${JSON.stringify(hostile.slice(0,24))} left frame ${frame} outside the judged grid`);
  if (hostileRun.nodes.findings.children.length !== singlePayload.findings.length) throw new Error("a hostile fragment changed the findings panel");
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
const evidenceSingleIds = singleIds.filter(id => id !== "gl");
const evidenceSingle = runSingle(singleEvidence, evidenceSinglePayload, evidenceSingleIds);
if (evidenceSingle.nodes.findings.children.length !== evidenceSinglePayload.findings.length) throw new Error("an evidence-only report dropped findings");
// The charts survive, so the scrub still moves their playhead.
const evidenceLast = evidenceSinglePayload.clips[0].frames - 1;
evidenceSingle.nodes.scrub.value = String(evidenceLast);
evidenceSingle.nodes.scrub.listeners.input();
const evidencePlayhead = Number(evidenceSingle.charts[0].query[".playhead"].attrs.x1);
if (Math.abs(evidencePlayhead - (chartPad + chartPlotW)) > 1e-6) throw new Error("scrubbing an evidence-only report does not move the chart playhead");
if (!evidenceSingle.nodes.time.textContent.includes("frame")) throw new Error("an evidence-only report stopped reporting the selected frame");
const evidenceDeep = runSingle(singleEvidence, evidenceSinglePayload, evidenceSingleIds, {hash: `#finding=${evidenceSinglePayload.findings.length - 1}&theme=light`});
if (evidenceDeep.root.attrs["data-theme"] !== "light" || !evidenceDeep.nodes.findings.children[evidenceSinglePayload.findings.length - 1].classes.has("selected")) throw new Error("an evidence-only report stopped honouring deep links");

console.log("report viewer harness passed");
