"use strict";
const fs = require("fs"), vm = require("vm");
const positions = Buffer.alloc(2 * 2 * 3 * 4); for (let i = 0; i < positions.length / 4; i++) positions.writeFloatLE(i, i * 4);
class Node { constructor(id) { this.id=id; this.children=[]; this.style={}; this.attrs={}; this.listeners={}; this.clientWidth=360; this.clientHeight=270; this.value="0"; this.textContent=""; } append(x){this.children.push(x)} replaceChildren(){this.children=[]} addEventListener(k,f){this.listeners[k]=f} setAttribute(k,v){this.attrs[k]=v} getContext(){return {setTransform(){},clearRect(){},beginPath(){},moveTo(){},lineTo(){},stroke(){},arc(){},fill(){}}} scrollIntoView(){this.scrolled=true} }
const ids = ["comparison-report-data","mapping","scrub","times","before-gl","after-gl","before-path","after-path","before-identity","after-identity","before-findings","after-findings","before-gaps","after-gaps","before-predictions","after-predictions"];
const nodes = Object.fromEntries(ids.map(id=>[id,new Node(id)]));
const clip = (name, offset) => ({name,duration:2,frames:2002,times:Array.from({length:2002},(_,i)=>i/1000),positions:positions.toString("base64"),trails:{root:0,hips:1,left_foot:0,right_foot:1}});
const data={bones:[{name:"root",parent:-1},{name:"hips",parent:0}],correspondence:{disclosure:"normalized phase"},before:{identity:{sha256:"a",bytes:1},clip:clip("before"),findings:[{check:"x",severity:"warning",bone:"hips",time:1.501,message:"<img>"}],gaps:[],prediction_provenance:null,predictions:[]},after:{identity:{sha256:"b",bytes:2},clip:clip("after"),findings:[{check:"x",severity:"warning",node:"#0(root)/#1(hips)",time:1.234,message:"safe"}],gaps:[],prediction_provenance:null,predictions:[]}};
nodes["comparison-report-data"].textContent=JSON.stringify(data);
const context={document:{getElementById:id=>nodes[id],createElement:()=>new Node(),createElementNS:()=>new Node()},window:{addEventListener(){}},location:{hash:""},atob:s=>Buffer.from(s,"base64").toString("binary"),Uint8Array,Float32Array,Math,Map,Array,Number,console}; vm.createContext(context); vm.runInContext(fs.readFileSync("crates/animsmith-report/assets/comparison.js","utf8"),context);
if(nodes.scrub.max !== 2001 || !nodes["before-findings"].children[0].textContent.includes("<img>") || fs.readFileSync("crates/animsmith-report/assets/comparison.js", "utf8").includes("innerHTML")) throw new Error("viewer did not retain exact frames or safe textContent");
nodes["before-findings"].children[0].listeners.click(); if(nodes.scrub.value != 1501) throw new Error("finding did not select exact frame");
nodes["after-findings"].children[0].listeners.click(); if(nodes.scrub.value != 1234) throw new Error("node finding did not select exact frame");
console.log("comparison viewer harness passed");
