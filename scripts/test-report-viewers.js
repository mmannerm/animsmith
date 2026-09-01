"use strict";
const fs = require("fs"), vm = require("vm");
if (process.argv.length !== 4) throw new Error("usage: test-report-viewers.js GENERATED_COMPARISON.html GENERATED_REPORT.html");
const html = fs.readFileSync(process.argv[2], "utf8");
const singleHtml = fs.readFileSync(process.argv[3], "utf8");
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
// The generated documents resolve every colour through the design tokens; a
// harness with no stylesheet exercises the documented dark fallbacks.
const noStyles = { getPropertyValue: () => "" };
if (data.kind !== "animsmith-comparison-v1") throw new Error("unexpected Rust comparison contract");
const frames = 2002, bones = data.bones.length, positions = Buffer.alloc(frames * bones * 3 * 4);
for (let frame = 0; frame < frames; frame++) for (let bone = 0; bone < bones; bone++) {
  const base = (frame * bones + bone) * 3;
  positions.writeFloatLE(frame / 1000 + bone, base * 4);
  positions.writeFloatLE(bone, (base + 1) * 4);
  positions.writeFloatLE((frame % 11) / 100, (base + 2) * 4);
}
class Node {
  constructor(id) { this.id=id; this.children=[]; this.style={}; this.attrs={}; this.listeners={}; this.classes=new Set(); this.dataset={}; this.query={}; this.clientWidth=360; this.clientHeight=270; this.value="0"; this.textContent=""; }
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
    clears:[],draws:[],uploads:0,
    createShader:()=>({}),shaderSource(){},compileShader(){},getShaderParameter:()=>true,getShaderInfoLog:()=>"",
    createProgram:()=>({}),attachShader(){},linkProgram(){},useProgram(){},getUniformLocation:()=>({}),
    createBuffer:()=>({}),bindBuffer(){},enableVertexAttribArray(){},vertexAttribPointer(){},enable(){},
    viewport(){},clear(){},uniformMatrix4fv(){},uniform1f(){},
    clearColor(...args){this.clears.push(args)},
    bufferData(_target,data){this.uploads=data.length},
    drawArrays(mode,first,count){this.draws.push({mode,first,count})}};
  return gl;
}
const ids = ["comparison-report-data","mapping","scrub","times","comparison-root-path","clip-before","clip-after","before-gl","after-gl","before-pose-context","after-pose-context","before-path","after-path","before-gait","after-gait","before-contexts","after-contexts","before-identity","after-identity","before-findings","after-findings","before-gaps","after-gaps","before-predictions","after-predictions"];
const nodes = Object.fromEntries(ids.map(id=>[id,new Node(id)]));
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
nodes["comparison-report-data"].textContent=JSON.stringify(data);
const windowListeners={};
const documentElement=new Node("documentElement");
const context={document:{documentElement,getElementById:id=>nodes[id],createElement:()=>new Node(),createElementNS:()=>new Node()},window:{addEventListener(k,f){windowListeners[k]=f}},location:{hash:""},getComputedStyle:()=>noStyles,atob:s=>Buffer.from(s,"base64").toString("binary"),Uint8Array,Float32Array,Math,Map,Array,Number,Object,Infinity,JSON,console};
vm.createContext(context); vm.runInContext(viewer,context);
if(nodes.scrub.max !== 2001 || !nodes["before-findings"].children.some(child=>child.textContent.includes("<img>"))) throw new Error("viewer did not retain exact frames or safe finding text");
if(!nodes["before-identity"].textContent.includes(data.before.dependency_closure_identity.sha256) || !nodes["after-identity"].textContent.includes(data.after.dependency_closure_identity.sha256)) throw new Error("viewer does not disclose complete closure identities");
if(!nodes.times.textContent.includes("before 0.000s") || !nodes.times.textContent.includes("after 0.000s") || !nodes.times.textContent.includes("not a time warp")) throw new Error("shared phase omits source times or no-warp disclosure");
if(!nodes["comparison-root-path"].children.some(child=>child.textContent==="before root path") || !nodes["comparison-root-path"].children.some(child=>child.textContent==="after root path")) throw new Error("shared root chart lacks textual before/after legends");
if(!nodes["before-path"].children.some(child=>child.attrs["data-role"]==="left_foot") || !nodes["before-path"].children.some(child=>child.attrs["data-role"]==="right_foot")) throw new Error("role trail chart omits foot trajectories");
if(!nodes["before-gait"].children.some(child=>child.attrs["data-stance-side"]==="left")) throw new Error("gait chart omits typed stance interval");
const seamIndex = data.before.findings.indexOf(seamFinding), structuralIndex = data.before.findings.indexOf(structuralFinding), afterIndex = data.after.findings.indexOf(afterFinding);
nodes["before-findings"].children[seamIndex].listeners.click();
if(nodes.scrub.value != 1501 || !nodes["before-pose-context"].textContent.includes("first 0.000s") || !nodes["before-pose-context"].textContent.includes(`affected ${seam.subject_bone_name}`)) throw new Error("seam finding did not select exact frame and endpoint/subject context");
if(!nodes["before-gl"].context.arcs.some(row=>row.args[2]===6 && row.fillStyle==="#f7768e")) throw new Error("finding did not highlight its Rust-projected bone on canvas");
nodes["before-findings"].children[structuralIndex].listeners.click();
if(!nodes["before-pose-context"].textContent.includes("structural evidence") || !nodes["before-contexts"].children.some(child=>child.className.includes("structural"))) throw new Error("structural finding was not distinguished from visible pose evidence");
nodes["after-findings"].children[afterIndex].listeners.click(); if(nodes.scrub.value != 1234) throw new Error("after finding did not select exact frame");
context.location.hash=`#time-before-${seamFinding.anchor.replace(/^finding-/, "")}`; windowListeners.hashchange();
if(nodes.scrub.value != 1501) throw new Error("semantic time anchor did not select its finding");
context.location.hash=`#finding-after-${afterFinding.anchor.replace(/^finding-/, "")}`; windowListeners.hashchange();
if(nodes.scrub.value != 1234) throw new Error("cross-side semantic finding anchor did not select its finding");

// Fragment options an embedded comparison honours, and their removal.
context.location.hash="#embed=1&theme=light&frame=1200&clip=ignored&finding=3"; windowListeners.hashchange();
if(documentElement.attrs["data-embed"] !== "1" || documentElement.attrs["data-theme"] !== "light" || nodes.scrub.value != 1200) throw new Error("comparison viewer ignored embed/theme/frame fragment options");
context.location.hash="#frame=999999999"; windowListeners.hashchange();
if(nodes.scrub.value != 2001) throw new Error("an out-of-range frame was not clamped to the shared phase");
context.location.hash="#theme=neon&embed=0&frame=-4"; windowListeners.hashchange();
if("data-embed" in documentElement.attrs || "data-theme" in documentElement.attrs || nodes.scrub.value != 2001) throw new Error("invalid fragment values were not ignored, or document switches were not restored");

// A non-finite sampled range must degrade the drawing, not abort navigation
// or hide the already-rendered findings and coverage lists.
const cleanBefore = data.before.clip.positions, cleanAfter = data.after.clip.positions;
function execute(payload) {
  const testNodes = Object.fromEntries(ids.map(id=>[id,new Node(id)]));
  testNodes["comparison-report-data"].textContent=JSON.stringify(payload);
  const testContext={document:{documentElement:new Node("documentElement"),getElementById:id=>testNodes[id],createElement:()=>new Node(),createElementNS:()=>new Node()},window:{addEventListener(){}},location:{hash:""},getComputedStyle:()=>noStyles,atob:s=>Buffer.from(s,"base64").toString("binary"),Uint8Array,Float32Array,Math,Map,Array,Number,Object,Infinity,JSON,console};
  vm.createContext(testContext); vm.runInContext(viewer, testContext);
  return testNodes;
}
const invalid = Buffer.from(data.before.clip.positions, "base64");
for (let offset = 0; offset < invalid.length; offset += 4) invalid.writeFloatLE(Number.NaN, offset);
data.before.clip.positions = invalid.toString("base64");
data.after.clip.positions = cleanAfter;
const isolatedNodes = execute(data);
const rootLabels = isolatedNodes["comparison-root-path"].children.map(child=>child.textContent);
const beforeTrailText = isolatedNodes["before-path"].children.map(child=>child.textContent).join(" ");
const afterTrailText = isolatedNodes["after-path"].children.map(child=>child.textContent).join(" ");
if (!isolatedNodes["before-pose-context"].textContent.includes("non-finite") || !isolatedNodes["before-gait"].textContent.includes("non-finite") || !isolatedNodes["after-pose-context"].textContent.includes("exact judged") || !rootLabels.includes("before root unavailable") || !rootLabels.includes("after root path") || !beforeTrailText.includes("unavailable:") || !beforeTrailText.includes("non-finite") || afterTrailText.includes("non-finite") || isolatedNodes["before-findings"].children.length !== data.before.findings.length) throw new Error("before-side non-finite pose/gait/root/trail evidence was mislabeled, hidden, or threw");

data.before.clip.positions = cleanBefore; data.after.clip.positions = invalid.toString("base64");
const reverseNodes = execute(data);
const reverseTrailText = reverseNodes["after-path"].children.map(child=>child.textContent).join(" ");
if (!reverseNodes["before-pose-context"].textContent.includes("exact judged") || !reverseNodes["after-pose-context"].textContent.includes("non-finite") || !reverseTrailText.includes("unavailable:") || !reverseTrailText.includes("non-finite")) throw new Error("after-side non-finite evidence did not remain independent of exact before evidence");

// A selected mixed-finite frame also loses the exact-evidence label while
// other finite frames and the opposite side remain independently available.
const mixed = Buffer.from(cleanAfter, "base64");
mixed.writeFloatLE(Number.NaN, (1501 * bones * 3 + 1 * 3) * 4);
data.before.clip.positions = cleanBefore; data.after.clip.positions = mixed.toString("base64");
const mixedNodes = execute(data);
mixedNodes.scrub.value=1501; mixedNodes.scrub.listeners.input();
const mixedTrailText = mixedNodes["after-path"].children.map(child=>child.textContent).join(" ");
if (!mixedNodes["after-pose-context"].textContent.includes("selected frame contains non-finite") || !mixedNodes["before-pose-context"].textContent.includes("exact judged") || !mixedTrailText.includes("incomplete non-finite samples")) throw new Error("mixed per-frame/trail availability was not evaluated independently");

// Structural context must remain visible without overriding the selected
// frame's non-finite availability disclosure.
data.before.clip.positions = mixed.toString("base64"); data.after.clip.positions = cleanAfter;
const structuralNodes = execute(data);
structuralNodes["before-findings"].children[structuralIndex].listeners.click();
if (!structuralNodes["before-pose-context"].textContent.includes("structural evidence") || !structuralNodes["before-pose-context"].textContent.includes("selected frame contains non-finite")) throw new Error("structural selection hid non-finite selected-frame availability");
// ---- fragment parser ---------------------------------------------------
// One parser serves both documents, so it is exercised once, directly, with
// valid, invalid, and hostile fragments. Nothing here may throw.
const parse = context.animsmithFragmentOptions;
const NONE = {embed:false,theme:null,clip:null,frame:null,finding:null};
function expectOptions(hash, expected, why) {
  let actual;
  try { actual = parse(hash); } catch (error) { throw new Error(`fragment ${JSON.stringify(String(hash).slice(0,40))} threw: ${error}`); }
  const merged = Object.assign({}, NONE, expected);
  for (const key of Object.keys(NONE)) if (actual[key] !== merged[key]) throw new Error(`${why}: ${key} was ${JSON.stringify(actual[key])}, expected ${JSON.stringify(merged[key])}`);
}
expectOptions("#embed=1&theme=dark&clip=walk&frame=7&finding=2", {embed:true,theme:"dark",clip:"walk",frame:7,finding:2}, "every documented option");
expectOptions("embed=true&theme=light", {embed:true,theme:"light"}, "a fragment without its leading hash");
expectOptions("#clip=walk%20cycle%2F01", {clip:"walk cycle/01"}, "percent-encoded clip names");
expectOptions("#unknown=1&x&=2&clip", {}, "unknown keys and malformed pairs");
for (const hostile of [
  "#theme=%3Cscript%3E", "#theme=LIGHT", "#theme=light%00", "#embed=yes", "#embed=2",
  "#frame=-1", "#frame=1e3", "#frame=0x10", "#frame=1.5", "#frame=9999999999999999999999",
  "#frame=" + "9".repeat(4000), "#finding=-2", "#finding=NaN", "#finding=Infinity",
  "#clip=%E0%A4%A", "#clip=%", "#" + "&".repeat(5000), "#theme=light" + "&pad=1".repeat(2000),
  "#finding-before-abcdef0123456789", "#", "", "#=light", "#theme", "#theme=",
]) expectOptions(hostile, {}, `hostile fragment ${JSON.stringify(hostile.slice(0, 24))}`);
for (const wrongType of [null, undefined, 0, {}, [], () => {}]) expectOptions(wrongType, {}, "a non-string fragment");
if (parse("#frame=999999999").frame !== 999999999) throw new Error("the largest addressable index must survive parsing for the caller to clamp");

// ---- single-clip viewer ------------------------------------------------
function singleReportParts(source) {
  const match = source.match(/<script>([\s\S]*?)<\/script>\n<script type="application\/json" id="report-data">([\s\S]*?)<\/script>\n<script>([\s\S]*?)<\/script>\n<\/body>\n<\/html>\s*$/);
  if (!match) throw new Error("Rust-generated single-clip payload and its inline viewer are absent");
  if (!match[1].startsWith("// animsmith report shared runtime")) throw new Error("wrong inline shared runtime");
  if (!match[3].startsWith("// animsmith report viewer")) throw new Error("wrong inline single-clip viewer");
  return { shared: match[1], payload: match[2], viewer: match[3] };
}
const single = singleReportParts(singleHtml);
const singleIds = ["report-data","file","clip-select","play","scrub","time","gl","findings","gaps","predictions"];
function runSingle(payload, hash) {
  const testNodes = Object.fromEntries(singleIds.map(id=>[id,new Node(id)]));
  testNodes["report-data"].textContent = JSON.stringify(payload);
  const chart = new Node("chart"); chart.dataset = {clip: payload.clips[0].name, kind: "gait", pad: "34", plotw: "318"};
  chart.query[".playhead"] = new Node("playhead");
  const root = new Node("documentElement"), listeners = {};
  const testContext = {
    document: {documentElement: root, getElementById: id=>testNodes[id], createElement: ()=>new Node(), createTextNode: text=>{const n=new Node(); n.textContent=text; return n}, querySelectorAll: ()=>[chart]},
    window: {addEventListener(k,f){listeners[k]=f}, devicePixelRatio: 1},
    location: {hash: hash || ""}, getComputedStyle: ()=>noStyles,
    performance: {now: ()=>0}, requestAnimationFrame: ()=>0,
    atob: value=>Buffer.from(value, "base64").toString("binary"),
    Uint8Array, Float32Array, Math, Map, Set, Array, Number, Object, Infinity, JSON, console,
  };
  vm.createContext(testContext);
  vm.runInContext(`${single.shared}\n${single.viewer}`, testContext);
  return {nodes: testNodes, root, chart, listeners, context: testContext};
}
const singlePayload = JSON.parse(single.payload);
if (!singlePayload.clips.length) throw new Error("the single-clip fixture must embed at least one pose grid");
const singleClip = singlePayload.clips[0], lastFrame = singleClip.frames - 1;
// A harness-owned finding proves selection independently of fixture content,
// and its message proves untrusted text stays text.
singlePayload.findings.push({check:"harness-check", severity:"warning", clip:singleClip.name, bone:"hips", node:null, time:singleClip.duration/2, message:"<img src=x>"});
const findingIndex = singlePayload.findings.length - 1;
const judgedFrame = Math.round((singlePayload.findings[findingIndex].time / singleClip.duration) * lastFrame);

const plain = runSingle(singlePayload, "");
if (!plain.nodes.file.textContent.includes(singlePayload.file || "")) throw new Error("the viewer did not disclose its source file");
if (plain.nodes.findings.children.length !== singlePayload.findings.length) throw new Error("the findings panel dropped rows");
if (!plain.nodes.findings.children.some(row=>row.children.some(part=>part.textContent === "<img src=x>") || row.children.some(part=>part.textContent && part.textContent.includes("<img")))) {
  const texts = plain.nodes.findings.children.map(row=>row.children.map(part=>part.textContent).join("|"));
  if (!texts.some(text=>text.includes("<img src=x>"))) throw new Error("untrusted finding text was not carried as text");
}
if (!plain.nodes.gl.gl.clears.length) throw new Error("the WebGL view never cleared a frame");
const cleared = plain.nodes.gl.gl.clears[0];
if (Math.abs(cleared[0] - 0x17/255) > 1e-6 || Math.abs(cleared[2] - 0x1f/255) > 1e-6) throw new Error("the WebGL clear colour is not the ground token");
if ("data-theme" in plain.root.attrs || "data-embed" in plain.root.attrs) throw new Error("an empty fragment must leave the document defaults alone");

const deep = runSingle(singlePayload, `#embed=1&theme=light&clip=${encodeURIComponent(singleClip.name)}&frame=${Math.min(2, lastFrame)}`);
if (deep.root.attrs["data-theme"] !== "light" || deep.root.attrs["data-embed"] !== "1") throw new Error("the single-clip viewer ignored embed/theme");
if (Number(deep.nodes.scrub.value) !== Math.min(2, lastFrame)) throw new Error("a deep-linked frame did not scrub the viewer");
if (deep.nodes["clip-select"].value !== singleClip.name) throw new Error("a deep-linked clip was not selected");
if (deep.chart.query[".playhead"].attrs.x1 === undefined) throw new Error("the chart playhead did not follow a deep-linked frame");

const selected = runSingle(singlePayload, `#finding=${findingIndex}`);
if (Number(selected.nodes.scrub.value) !== judgedFrame) throw new Error("a deep-linked finding did not scrub to its judged frame");
if (!selected.nodes.findings.children[findingIndex].classes.has("selected")) throw new Error("a deep-linked finding was not highlighted");

for (const hostile of [
  "#frame=999999999", "#frame=-1", "#clip=%E0%A4%A", "#clip=no-such-clip", "#finding=999999999",
  "#finding=" + "9".repeat(400), "#theme=%3Cimg%3E", "#embed=maybe", "#unknown=1",
  "#finding-before-abcdef0123456789", "#" + "k=v&".repeat(3000),
]) {
  let run;
  try { run = runSingle(singlePayload, hostile); } catch (error) { throw new Error(`fragment ${JSON.stringify(hostile.slice(0,24))} threw in the viewer: ${error}`); }
  const frame = Number(run.nodes.scrub.value);
  if (!Number.isInteger(frame) || frame < 0 || frame > lastFrame) throw new Error(`fragment ${JSON.stringify(hostile.slice(0,24))} left frame ${frame} outside the judged grid`);
  if (run.nodes.findings.children.length !== singlePayload.findings.length) throw new Error("a hostile fragment changed the findings panel");
}

console.log("report viewer harness passed");
