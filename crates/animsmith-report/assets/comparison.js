// animsmith comparison viewer: hand-written, offline, and driven only by
// Rust-sampled metric frames. Shared phase is a display mapping, not a retime.
"use strict";
const data = JSON.parse(document.getElementById("comparison-report-data").textContent);
const q = (id) => document.getElementById(id);
const decode = (encoded) => {
  const raw = atob(encoded), bytes = new Uint8Array(raw.length);
  for (let i = 0; i < raw.length; i++) bytes[i] = raw.charCodeAt(i);
  return new Float32Array(bytes.buffer);
};
for (const side of [data.before, data.after]) side.clip.pos = decode(side.clip.positions);
q("mapping").textContent = data.correspondence.disclosure;
const parents = data.bones.map((bone) => bone.parent);
const namedBone = new Map(data.bones.map((bone, index) => [bone.name, index]));
const sharedFrameMax = Math.max(data.before.clip.frames, data.after.clip.frames) - 1;
q("scrub").max = sharedFrameMax;
let selectedFrames = null;
const range = (values) => [Math.min(...values), Math.max(...values)];
const sharedPoseBounds = (() => {
  const xs = [], ys = [];
  for (const side of [data.before, data.after]) for (let i = 0; i < side.clip.pos.length; i += 3) { xs.push(side.clip.pos[i]); ys.push(side.clip.pos[i + 1]); }
  return { x: range(xs), y: range(ys) };
})();
const sharedPathBounds = (() => {
  const xs = [], zs = [];
  for (const side of [data.before, data.after]) { const root = side.clip.trails.root; if (root == null) continue; for (let frame = 0; frame < side.clip.frames; frame++) { const base = frame * data.bones.length * 3 + root * 3; xs.push(side.clip.pos[base]); zs.push(side.clip.pos[base + 2]); } }
  return xs.length ? { x: range(xs), z: range(zs) } : null;
})();

function drawSide(name, phase, highlighted) {
  const side = data[name], canvas = q(`${name}-gl`), context = canvas.getContext("2d");
  const dpr = window.devicePixelRatio || 1, width = canvas.clientWidth * dpr, height = canvas.clientHeight * dpr;
  if (canvas.width !== width || canvas.height !== height) { canvas.width = width; canvas.height = height; }
  context.setTransform(dpr, 0, 0, dpr, 0, 0); context.clearRect(0, 0, canvas.clientWidth, canvas.clientHeight);
  const frame = selectedFrames ? selectedFrames[name] : Math.round(phase * Math.max(0, side.clip.frames - 1)), base = frame * data.bones.length * 3;
  const point = (bone) => [side.clip.pos[base + bone * 3], side.clip.pos[base + bone * 3 + 1], side.clip.pos[base + bone * 3 + 2]];
  const all = data.bones.map((_, bone) => point(bone));
  const [minX, maxX] = sharedPoseBounds.x, [minY, maxY] = sharedPoseBounds.y;
  const scale = Math.min(canvas.clientWidth / Math.max(.1, maxX - minX), canvas.clientHeight / Math.max(.1, maxY - minY)) * .72;
  const project = (p) => [canvas.clientWidth / 2 + (p[0] - (minX + maxX) / 2) * scale, canvas.clientHeight / 2 - (p[1] - (minY + maxY) / 2) * scale];
  context.lineWidth = 2; context.strokeStyle = "#8e99bc";
  for (let bone = 0; bone < parents.length; bone++) if (parents[bone] >= 0) { const a = project(point(parents[bone])), b = project(point(bone)); context.beginPath(); context.moveTo(...a); context.lineTo(...b); context.stroke(); }
  for (let bone = 0; bone < all.length; bone++) { const p = project(all[bone]); context.fillStyle = bone === highlighted ? "#f0cb83" : "#d5d9e5"; context.beginPath(); context.arc(...p, bone === highlighted ? 6 : 3, 0, Math.PI * 2); context.fill(); }
  return frame;
}
function drawPath(name, phase) {
  const side = data[name], svg = q(`${name}-path`), root = side.clip.trails.root;
  svg.replaceChildren(); if (root == null) { svg.textContent = "root path unavailable"; return; }
  const points = Array.from({ length: side.clip.frames }, (_, frame) => { const base = frame * data.bones.length * 3 + root * 3; return [side.clip.pos[base], side.clip.pos[base + 2]]; });
  const [minX, maxX] = sharedPathBounds.x, [minZ, maxZ] = sharedPathBounds.z;
  const map = (point) => [18 + (point[0] - minX) * 324 / Math.max(.001, maxX - minX), 132 - (point[1] - minZ) * 114 / Math.max(.001, maxZ - minZ)];
  const svgNs = "http:" + "//www.w3.org/2000/svg";
  const path = document.createElementNS(svgNs, "path"); path.setAttribute("d", points.map((point, index) => `${index ? "L" : "M"}${map(point).join(",")}`).join("")); path.setAttribute("fill", "none"); path.setAttribute("stroke", "#7aa2f7"); path.setAttribute("stroke-width", "2"); svg.append(path);
  const dotFrame = selectedFrames ? selectedFrames[name] : Math.round(phase * Math.max(0, points.length - 1));
  const dot = document.createElementNS(svgNs, "circle"), selected = map(points[dotFrame]); dot.setAttribute("cx", selected[0]); dot.setAttribute("cy", selected[1]); dot.setAttribute("r", "4"); dot.setAttribute("fill", "#f0cb83"); svg.append(dot);
  const legend = document.createElementNS(svgNs, "text"); legend.setAttribute("x", "8"); legend.setAttribute("y", "14"); legend.setAttribute("fill", "#aab1c5"); legend.textContent = "root path top-down (m) — shared scale"; svg.append(legend);
}
let highlight = { before: null, after: null };
function update() { const phase = Number(q("scrub").value) / Math.max(1, sharedFrameMax); const beforeFrame = drawSide("before", phase, highlight.before), afterFrame = drawSide("after", phase, highlight.after); drawPath("before", phase); drawPath("after", phase); const time = (side, frame) => side.clip.times[frame].toFixed(3); q("times").textContent = `before ${time(data.before, beforeFrame)}s · after ${time(data.after, afterFrame)}s (normalized phase; not a time warp)`; }
function summary(side) { return `${side.identity.sha256} · ${side.identity.bytes} bytes · clip ${side.clip.name}`; }
function subjectBone(row) { if (row.bone && namedBone.has(row.bone)) return namedBone.get(row.bone); const nodeName = row.node && row.node.match(/\(([^()]*)\)$/); return nodeName && namedBone.has(nodeName[1]) ? namedBone.get(nodeName[1]) : null; }
function nearestFrame(side, time) { return side.clip.times.reduce((best, value, index) => Math.abs(value - time) < Math.abs(side.clip.times[best] - time) ? index : best, 0); }
function selectFinding(name, index) { const row = data[name].findings[index], side = data[name]; if (!row) return; const frame = nearestFrame(side, row.time == null ? 0 : row.time); const phase = frame / Math.max(1, side.clip.frames - 1); selectedFrames = { before: Math.round(phase * Math.max(0, data.before.clip.frames - 1)), after: Math.round(phase * Math.max(0, data.after.clip.frames - 1)) }; selectedFrames[name] = frame; q("scrub").value = Math.round(sharedFrameMax * phase); highlight = { before: null, after: null }; highlight[name] = subjectBone(row); update(); }
function list(name, kind) { const side = data[name], target = q(`${name}-${kind}`), rows = side[kind]; if (!rows.length) { target.textContent = kind === "findings" ? "no findings" : "none"; return; } rows.forEach((row, index) => { const item = document.createElement("li"); item.id = kind === "findings" ? `${name}-${row.anchor}` : `${kind.slice(0, -1)}-${name}-${index}`; item.textContent = kind === "findings" ? `${row.severity} · ${row.check} · ${row.bone || row.node || "no mapped subject"}${row.time == null ? "" : ` @${row.time.toFixed(3)}s`} — ${row.message}` : `${row.check_id} · ${row.code} — ${row.message}`; if (kind === "findings") { const timeAnchor = document.createElement("span"); timeAnchor.id = `${name}-time-${row.anchor}`; item.append(timeAnchor); item.className = "finding"; item.addEventListener("click", () => { selectFinding(name, index); item.scrollIntoView({ block: "nearest" }); item.focus && item.focus(); }); } target.append(item); }); }
for (const name of ["before", "after"]) { q(`${name}-identity`).textContent = summary(data[name]); list(name, "findings"); list(name, "gaps"); q(`${name}-predictions`).textContent = JSON.stringify({ provenance: data[name].prediction_provenance, predictions: data[name].predictions }, null, 2); }
function selectHash() { const match = location.hash.match(/^#(?:finding|time)-(before|after)-(\d+)$/); if (match) selectFinding(match[1], Number(match[2])); }
q("scrub").addEventListener("input", () => { selectedFrames = null; update(); }); window.addEventListener("resize", update); window.addEventListener("hashchange", selectHash); selectHash(); update();
