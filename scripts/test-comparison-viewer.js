"use strict";
const fs = require("fs"), vm = require("vm");
const frames = 2002, bones = 2, positions = Buffer.alloc(frames * bones * 3 * 4);
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
const clip = (name) => ({anchor:`clip-${name}`,name,duration:2,frames,times:Array.from({length:frames},(_,i)=>i/1000),positions:positions.toString("base64"),trails:{root:0,hips:1,left_foot:0,right_foot:1}});
const contexts = (seam, structural) => ({
  gait:{source:"pose grid",hips:1,left:0,left_role:"left_foot",right:1,right_role:"right_foot"},
  stances:[{source:"typed scope",scope:"left_foot_stance",side:"left",selected_role:"left_foot",bone:0,bone_name:"root",contact_height_m:.03,runs:[{start_frame:1000,end_frame:1600,start_s:1,end_s:1.6}]}],
  seams:seam?[{source:"typed finding",finding_anchor:"finding-0123456789abcdef",check:"loop-closure",first_frame:0,last_frame:2001,first_s:0,last_s:2.001,subject_bone:1,subject_bone_name:"hips"}]:[],
  structural:structural?[{source:"typed finding",finding_anchor:"finding-fedcba9876543210",check:"constant-track",evidence_kind:"structural",subject_bone_name:"hips",label:"structural evidence — poses may look unchanged"}]:[],
});
const data={bones:[{name:"root",parent:-1},{name:"hips",parent:0}],correspondence:{disclosure:"normalized phase"},before:{identity:{sha256:"a",bytes:1},clip:clip("before"),contexts:contexts(true,true),findings:[{anchor:"finding-0123456789abcdef",check:"loop-closure",severity:"error",bone:"hips",time:1.501,message:"<img>"},{anchor:"finding-fedcba9876543210",check:"constant-track",severity:"note",bone:"hips",time:null,message:"structural"}],gaps:[],prediction_provenance:null,predictions:[]},after:{identity:{sha256:"b",bytes:2},clip:clip("after"),contexts:contexts(false,false),findings:[{anchor:"finding-1111111111111111",check:"x",severity:"warning",node:"#0(root)/#1(hips)",time:1.234,message:"safe"}],gaps:[],prediction_provenance:null,predictions:[]}};
nodes["comparison-report-data"].textContent=JSON.stringify(data);
const windowListeners={};
const context={document:{getElementById:id=>nodes[id],createElement:()=>new Node(),createElementNS:()=>new Node()},window:{addEventListener(k,f){windowListeners[k]=f}},location:{hash:""},atob:s=>Buffer.from(s,"base64").toString("binary"),Uint8Array,Float32Array,Math,Map,Array,Number,Object,Infinity,JSON,console};
vm.createContext(context); vm.runInContext(fs.readFileSync("crates/animsmith-report/assets/comparison.js","utf8"),context);
const viewer = fs.readFileSync("crates/animsmith-report/assets/comparison.js", "utf8");
if(nodes.scrub.max !== 2001 || !nodes["before-findings"].children[0].textContent.includes("<img>") || viewer.includes("innerHTML")) throw new Error("viewer did not retain exact frames or safe textContent");
if(!nodes["comparison-root-path"].children.some(child=>child.attrs.stroke==="#7aa2f7") || !nodes["comparison-root-path"].children.some(child=>child.attrs.stroke==="#e0af68")) throw new Error("shared root chart lacks unambiguous before/after paths");
if(!nodes["before-path"].children.some(child=>child.attrs["data-role"]==="left_foot") || !nodes["before-path"].children.some(child=>child.attrs["data-role"]==="right_foot")) throw new Error("role trail chart omits foot trajectories");
if(!nodes["before-gait"].children.some(child=>child.attrs["data-stance-side"]==="left")) throw new Error("gait chart omits typed stance interval");
nodes["before-findings"].children[0].listeners.click();
if(nodes.scrub.value != 1501 || !nodes["before-pose-context"].textContent.includes("first 0.000s") || !nodes["before-pose-context"].textContent.includes("affected hips")) throw new Error("seam finding did not select exact frame and endpoint/subject context");
nodes["before-findings"].children[1].listeners.click();
if(!nodes["before-pose-context"].textContent.includes("structural evidence") || !nodes["before-contexts"].children.some(child=>child.className.includes("structural"))) throw new Error("structural finding was not distinguished from visible pose evidence");
nodes["after-findings"].children[0].listeners.click(); if(nodes.scrub.value != 1234) throw new Error("node finding did not select exact frame");
context.location.hash="#time-before-0123456789abcdef"; windowListeners.hashchange();
if(nodes.scrub.value != 1501) throw new Error("semantic time anchor did not select its finding");
console.log("comparison viewer harness passed");
