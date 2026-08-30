"use strict";
const fs = require("fs"), vm = require("vm");
if (process.argv.length !== 3) throw new Error("usage: test-comparison-viewer.js GENERATED_REPORT.html");
const html = fs.readFileSync(process.argv[2], "utf8");
const payload = html.match(/<script type="application\/json" id="comparison-report-data">([\s\S]*?)<\/script>/);
if (!payload) throw new Error("Rust-generated comparison payload is absent");
const data = JSON.parse(payload[1]);
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
  setAttribute(k,v){this.attrs[k]=v} getContext(){return {setTransform(){},clearRect(){},beginPath(){},moveTo(){},lineTo(){},stroke(){},arc(){},fill(){}}}
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
let afterFinding = data.after.findings[0];
if (!afterFinding) {
  afterFinding = {...seamFinding, anchor: "finding-1111111111111111"};
  data.after.findings.push(afterFinding);
}
afterFinding.time = 1.234;
nodes["comparison-report-data"].textContent=JSON.stringify(data);
const windowListeners={};
const context={document:{getElementById:id=>nodes[id],createElement:()=>new Node(),createElementNS:()=>new Node()},window:{addEventListener(k,f){windowListeners[k]=f}},location:{hash:""},atob:s=>Buffer.from(s,"base64").toString("binary"),Uint8Array,Float32Array,Math,Map,Array,Number,Object,Infinity,JSON,console};
vm.createContext(context); vm.runInContext(fs.readFileSync("crates/animsmith-report/assets/comparison.js","utf8"),context);
const viewer = fs.readFileSync("crates/animsmith-report/assets/comparison.js", "utf8");
if(nodes.scrub.max !== 2001 || !nodes["before-findings"].children.some(child=>child.textContent.includes("<img>")) || viewer.includes("innerHTML")) throw new Error("viewer did not retain exact frames or safe textContent");
if(!nodes["before-identity"].textContent.includes(data.before.dependency_closure_identity.sha256) || !nodes["after-identity"].textContent.includes(data.after.dependency_closure_identity.sha256)) throw new Error("viewer does not disclose complete closure identities");
if(!nodes["comparison-root-path"].children.some(child=>child.attrs.stroke==="#7aa2f7") || !nodes["comparison-root-path"].children.some(child=>child.attrs.stroke==="#e0af68")) throw new Error("shared root chart lacks unambiguous before/after paths");
if(!nodes["before-path"].children.some(child=>child.attrs["data-role"]==="left_foot") || !nodes["before-path"].children.some(child=>child.attrs["data-role"]==="right_foot")) throw new Error("role trail chart omits foot trajectories");
if(!nodes["before-gait"].children.some(child=>child.attrs["data-stance-side"]==="left")) throw new Error("gait chart omits typed stance interval");
const seamIndex = data.before.findings.indexOf(seamFinding), structuralIndex = data.before.findings.indexOf(structuralFinding), afterIndex = data.after.findings.indexOf(afterFinding);
nodes["before-findings"].children[seamIndex].listeners.click();
if(nodes.scrub.value != 1501 || !nodes["before-pose-context"].textContent.includes("first 0.000s") || !nodes["before-pose-context"].textContent.includes(`affected ${seam.subject_bone_name}`)) throw new Error("seam finding did not select exact frame and endpoint/subject context");
nodes["before-findings"].children[structuralIndex].listeners.click();
if(!nodes["before-pose-context"].textContent.includes("structural evidence") || !nodes["before-contexts"].children.some(child=>child.className.includes("structural"))) throw new Error("structural finding was not distinguished from visible pose evidence");
nodes["after-findings"].children[afterIndex].listeners.click(); if(nodes.scrub.value != 1234) throw new Error("after finding did not select exact frame");
context.location.hash=`#time-before-${seamFinding.anchor.replace(/^finding-/, "")}`; windowListeners.hashchange();
if(nodes.scrub.value != 1501) throw new Error("semantic time anchor did not select its finding");
context.location.hash=`#finding-after-${afterFinding.anchor.replace(/^finding-/, "")}`; windowListeners.hashchange();
if(nodes.scrub.value != 1234) throw new Error("cross-side semantic finding anchor did not select its finding");

// A non-finite sampled range must degrade the drawing, not abort navigation
// or hide the already-rendered findings and coverage lists.
const invalid = Buffer.from(data.before.clip.positions, "base64");
for (let offset = 0; offset < invalid.length; offset += 4) invalid.writeFloatLE(Number.NaN, offset);
data.before.clip.positions = invalid.toString("base64"); data.after.clip.positions = invalid.toString("base64");
nodes["comparison-report-data"].textContent=JSON.stringify(data);
const isolatedNodes = Object.fromEntries(ids.map(id=>[id,new Node(id)]));
isolatedNodes["comparison-report-data"].textContent=JSON.stringify(data);
const isolatedContext={document:{getElementById:id=>isolatedNodes[id],createElement:()=>new Node(),createElementNS:()=>new Node()},window:{addEventListener(){}},location:{hash:""},atob:s=>Buffer.from(s,"base64").toString("binary"),Uint8Array,Float32Array,Math,Map,Array,Number,Object,Infinity,JSON,console};
vm.createContext(isolatedContext); vm.runInContext(viewer, isolatedContext);
if (!isolatedNodes["before-pose-context"].textContent.includes("non-finite") || !isolatedNodes["before-gait"].textContent.includes("non-finite") || isolatedNodes["before-findings"].children.length !== data.before.findings.length) throw new Error("non-finite pose/gait range hid diagnostics or threw");
console.log("comparison viewer harness passed");
