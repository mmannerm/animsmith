"use strict";
const fs = require("fs"), vm = require("vm");
if (process.argv.length !== 3) throw new Error("usage: test-comparison-viewer.js GENERATED_REPORT.html");
const html = fs.readFileSync(process.argv[2], "utf8");
function generatedReportParts(source) {
  const match = source.match(/<script type="application\/json" id="comparison-report-data">([\s\S]*?)<\/script><script>([\s\S]*?)<\/script><\/body><\/html>\s*$/);
  if (!match) throw new Error("Rust-generated payload and immediately following inline viewer are absent");
  if (!match[2].startsWith("// animsmith comparison viewer:")) throw new Error("wrong inline comparison viewer");
  return { payload: match[1], viewer: match[2] };
}
const generated = generatedReportParts(html);
for (const mutation of [
  html.replace("</script><script>// animsmith comparison viewer:", "</script><script>// misplaced</script><script>// animsmith comparison viewer:"),
  html.replace("// animsmith comparison viewer:", "// wrong viewer:"),
  html.replace(/<\/script><\/body><\/html>\s*$/, "</body></html>"),
]) {
  let refused = false;
  try { generatedReportParts(mutation); } catch (_) { refused = true; }
  if (!refused) throw new Error("generated HTML viewer placement/identity mutation was accepted");
}
const data = JSON.parse(generated.payload), viewer = generated.viewer;
if (data.kind !== "animsmith-comparison-v1") throw new Error("unexpected Rust comparison contract");
const frames = 2002, bones = data.bones.length, positions = Buffer.alloc(frames * bones * 3 * 4);
for (let frame = 0; frame < frames; frame++) for (let bone = 0; bone < bones; bone++) {
  const base = (frame * bones + bone) * 3;
  positions.writeFloatLE(frame / 1000 + bone, base * 4);
  positions.writeFloatLE(bone, (base + 1) * 4);
  positions.writeFloatLE((frame % 11) / 100, (base + 2) * 4);
}
class Node {
  constructor(id) { this.id=id; this.children=[]; this.style={}; this.attrs={}; this.listeners={}; this.clientWidth=360; this.clientHeight=270; this.value="0"; this.textContent=""; }
  append(x){this.children.push(x)} replaceChildren(){this.children=[]} addEventListener(k,f){this.listeners[k]=f}
  setAttribute(k,v){this.attrs[k]=v} getContext(){
    if (!this.context) this.context={arcs:[],fillStyle:null,setTransform(){},clearRect(){this.arcs=[]},beginPath(){},moveTo(){},lineTo(){},stroke(){},arc(...args){this.arcs.push({args,fillStyle:this.fillStyle})},fill(){}};
    return this.context;
  }
  scrollIntoView(){this.scrolled=true}
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
const context={document:{getElementById:id=>nodes[id],createElement:()=>new Node(),createElementNS:()=>new Node()},window:{addEventListener(k,f){windowListeners[k]=f}},location:{hash:""},atob:s=>Buffer.from(s,"base64").toString("binary"),Uint8Array,Float32Array,Math,Map,Array,Number,Object,Infinity,JSON,console};
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
if(!nodes["before-gl"].context.arcs.some(row=>row.args[2]===6 && row.fillStyle==="#f0cb83")) throw new Error("finding did not highlight its Rust-projected bone on canvas");
nodes["before-findings"].children[structuralIndex].listeners.click();
if(!nodes["before-pose-context"].textContent.includes("structural evidence") || !nodes["before-contexts"].children.some(child=>child.className.includes("structural"))) throw new Error("structural finding was not distinguished from visible pose evidence");
nodes["after-findings"].children[afterIndex].listeners.click(); if(nodes.scrub.value != 1234) throw new Error("after finding did not select exact frame");
context.location.hash=`#time-before-${seamFinding.anchor.replace(/^finding-/, "")}`; windowListeners.hashchange();
if(nodes.scrub.value != 1501) throw new Error("semantic time anchor did not select its finding");
context.location.hash=`#finding-after-${afterFinding.anchor.replace(/^finding-/, "")}`; windowListeners.hashchange();
if(nodes.scrub.value != 1234) throw new Error("cross-side semantic finding anchor did not select its finding");

// A non-finite sampled range must degrade the drawing, not abort navigation
// or hide the already-rendered findings and coverage lists.
const cleanBefore = data.before.clip.positions, cleanAfter = data.after.clip.positions;
function execute(payload) {
  const testNodes = Object.fromEntries(ids.map(id=>[id,new Node(id)]));
  testNodes["comparison-report-data"].textContent=JSON.stringify(payload);
  const testContext={document:{getElementById:id=>testNodes[id],createElement:()=>new Node(),createElementNS:()=>new Node()},window:{addEventListener(){}},location:{hash:""},atob:s=>Buffer.from(s,"base64").toString("binary"),Uint8Array,Float32Array,Math,Map,Array,Number,Object,Infinity,JSON,console};
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
console.log("comparison viewer harness passed");
