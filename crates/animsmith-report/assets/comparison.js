// animsmith comparison viewer: hand-written, offline, and driven only by
// Rust-sampled metric frames. Shared phase is a display mapping, not a retime.
"use strict";
const data = JSON.parse(document.getElementById("comparison-report-data").textContent);
const q = (id) => document.getElementById(id);
const svgNs = "http:" + "//www.w3.org/2000/svg";
const decode = (encoded) => {
  const raw = atob(encoded), bytes = new Uint8Array(raw.length);
  for (let index = 0; index < raw.length; index++) bytes[index] = raw.charCodeAt(index);
  return new Float32Array(bytes.buffer);
};
for (const side of [data.before, data.after]) side.clip.pos = decode(side.clip.positions);
q("mapping").textContent = data.correspondence.disclosure;
const parents = data.bones.map((bone) => bone.parent);
const namedBone = new Map(data.bones.map((bone, index) => [bone.name, index]));
const sharedFrameMax = Math.max(data.before.clip.frames, data.after.clip.frames) - 1;
q("scrub").max = sharedFrameMax;
let selectedFrames = null;
let selectedContext = null;

function finiteRange(values) {
  let min = Infinity, max = -Infinity;
  for (const value of values) if (Number.isFinite(value)) { min = Math.min(min, value); max = Math.max(max, value); }
  return min === Infinity ? null : [min, max];
}
function posePoint(side, frame, bone) {
  const base = frame * data.bones.length * 3 + bone * 3;
  return [side.clip.pos[base], side.clip.pos[base + 1], side.clip.pos[base + 2]];
}
function trailPoints(side, bone) {
  return Array.from({ length: side.clip.frames }, (_, frame) => {
    const point = posePoint(side, frame, bone);
    return [point[0], point[2]];
  });
}
function finitePoint(point) { return point.every(Number.isFinite); }
function boundsFor(roleNames) {
  const xs = [], zs = [];
  for (const side of [data.before, data.after]) for (const role of roleNames) {
    const bone = side.clip.trails[role];
    if (bone == null) continue;
    for (const point of trailPoints(side, bone)) { xs.push(point[0]); zs.push(point[1]); }
  }
  const x = finiteRange(xs), z = finiteRange(zs);
  return x && z ? { x, z } : null;
}
const sharedPoseBounds = (() => {
  const xs = [], ys = [];
  for (const side of [data.before, data.after]) for (let index = 0; index < side.clip.pos.length; index += 3) {
    xs.push(side.clip.pos[index]); ys.push(side.clip.pos[index + 1]);
  }
  const x = finiteRange(xs), y = finiteRange(ys);
  return x && y ? { x, y } : null;
})();
const sharedRootBounds = boundsFor(["root"]);
const sharedTrailBounds = boundsFor(["root", "hips", "left_foot", "right_foot"]);

function svgElement(tag, attrs, label) {
  const element = document.createElementNS(svgNs, tag);
  for (const [name, value] of Object.entries(attrs || {})) element.setAttribute(name, value);
  if (label != null) element.textContent = label;
  return element;
}
function topDownMap(bounds, width, height, pad) {
  const spanX = Math.max(.001, bounds.x[1] - bounds.x[0]);
  const spanZ = Math.max(.001, bounds.z[1] - bounds.z[0]);
  const scale = Math.min((width - 2 * pad) / spanX, (height - 2 * pad) / spanZ);
  const centerX = (bounds.x[0] + bounds.x[1]) / 2, centerZ = (bounds.z[0] + bounds.z[1]) / 2;
  return (point) => [width / 2 + (point[0] - centerX) * scale, height / 2 - (point[1] - centerZ) * scale];
}
function pathData(points, map) {
  let drawing = false, path = "";
  for (const point of points) {
    if (!finitePoint(point)) { drawing = false; continue; }
    const mapped = map(point);
    if (!finitePoint(mapped)) { drawing = false; continue; }
    path += `${drawing ? "L" : "M"}${mapped.join(",")}`;
    drawing = true;
  }
  return path;
}

function drawSide(name, phase, highlighted) {
  const side = data[name], canvas = q(`${name}-gl`), context = canvas.getContext("2d");
  const dpr = window.devicePixelRatio || 1, width = canvas.clientWidth * dpr, height = canvas.clientHeight * dpr;
  if (canvas.width !== width || canvas.height !== height) { canvas.width = width; canvas.height = height; }
  context.setTransform(dpr, 0, 0, dpr, 0, 0); context.clearRect(0, 0, canvas.clientWidth, canvas.clientHeight);
  const frame = selectedFrames ? selectedFrames[name] : Math.round(phase * Math.max(0, side.clip.frames - 1));
  if (!sharedPoseBounds) {
    q(`${name}-pose-context`).textContent = "pose drawing unavailable: sampled X/Y positions are non-finite; findings and coverage remain listed";
    return frame;
  }
  const [minX, maxX] = sharedPoseBounds.x, [minY, maxY] = sharedPoseBounds.y;
  const scale = Math.min(canvas.clientWidth / Math.max(.1, maxX - minX), canvas.clientHeight / Math.max(.1, maxY - minY)) * .72;
  const project = (point) => [canvas.clientWidth / 2 + (point[0] - (minX + maxX) / 2) * scale, canvas.clientHeight / 2 - (point[1] - (minY + maxY) / 2) * scale];
  const drawFrame = (poseFrame, stroke, fill, subject) => {
    context.lineWidth = 2; context.strokeStyle = stroke;
    for (let bone = 0; bone < parents.length; bone++) if (parents[bone] >= 0) {
      const a = project(posePoint(side, poseFrame, parents[bone])), b = project(posePoint(side, poseFrame, bone));
      if (!finitePoint(a) || !finitePoint(b)) continue;
      context.beginPath(); context.moveTo(...a); context.lineTo(...b); context.stroke();
    }
    for (let bone = 0; bone < data.bones.length; bone++) {
      const point = project(posePoint(side, poseFrame, bone));
      if (!finitePoint(point)) continue;
      context.fillStyle = bone === subject ? "#f0cb83" : fill;
      context.beginPath(); context.arc(...point, bone === subject ? 6 : 3, 0, Math.PI * 2); context.fill();
    }
  };
  const seam = selectedContext && selectedContext.name === name && selectedContext.kind === "seam" ? selectedContext.value : null;
  if (seam) {
    drawFrame(seam.first_frame, "#7aa2f7", "#7aa2f7", seam.subject_bone);
    drawFrame(seam.last_frame, "#e0af68", "#e0af68", seam.subject_bone);
    q(`${name}-pose-context`).textContent = `loop seam endpoint poses — first ${seam.first_s.toFixed(3)}s blue · last ${seam.last_s.toFixed(3)}s orange · affected ${seam.subject_bone_name || "bone unavailable"}`;
  } else {
    drawFrame(frame, "#8e99bc", "#d5d9e5", highlighted);
    const structural = selectedContext && selectedContext.name === name && selectedContext.kind === "structural" ? selectedContext.value : null;
    q(`${name}-pose-context`).textContent = structural ? structural.label : "exact judged pose-grid frame";
  }
  return frame;
}

function drawRootComparison(phase) {
  const svg = q("comparison-root-path"); svg.replaceChildren();
  if (!sharedRootBounds) { svg.textContent = "root trajectories unavailable: a Root role did not resolve"; return; }
  const map = topDownMap(sharedRootBounds, 720, 220, 28);
  const styles = { before: "#7aa2f7", after: "#e0af68" };
  for (const name of ["before", "after"]) {
    const side = data[name], root = side.clip.trails.root;
    if (root == null) continue;
    const points = trailPoints(side, root), color = styles[name];
    svg.append(svgElement("path", { d: pathData(points, map), fill: "none", stroke: color, "stroke-width": 3 }));
    const frame = selectedFrames ? selectedFrames[name] : Math.round(phase * Math.max(0, points.length - 1));
    const selected = map(points[frame]);
    if (finitePoint(selected)) svg.append(svgElement("circle", { cx: selected[0], cy: selected[1], r: 5, fill: color }));
  }
  const beforeLabel = data.before.clip.trails.root == null ? "before root unavailable" : "before root path";
  const afterLabel = data.after.clip.trails.root == null ? "after root unavailable" : "after root path";
  svg.append(svgElement("text", { x: 18, y: 20, fill: data.before.clip.trails.root == null ? "#aab1c5" : "#7aa2f7" }, beforeLabel));
  svg.append(svgElement("text", { x: 170, y: 20, fill: data.after.clip.trails.root == null ? "#aab1c5" : "#e0af68" }, afterLabel));
  svg.append(svgElement("text", { x: 330, y: 20, fill: "#aab1c5" }, "top-down · one shared uniform metres scale"));
  svg.append(svgElement("text", { x: 18, y: 212, fill: "#aab1c5" }, `X ${sharedRootBounds.x[0].toFixed(3)}…${sharedRootBounds.x[1].toFixed(3)} m · Z ${sharedRootBounds.z[0].toFixed(3)}…${sharedRootBounds.z[1].toFixed(3)} m`));
}

const trailStyles = {
  root: ["#9ece6a", "root"], hips: ["#bb9af7", "hips"],
  left_foot: ["#7aa2f7", "left foot"], right_foot: ["#e0af68", "right foot"],
};
function drawTrails(name, phase) {
  const side = data[name], svg = q(`${name}-path`); svg.replaceChildren();
  if (!sharedTrailBounds) { svg.textContent = "role trajectories unavailable"; return; }
  const map = topDownMap(sharedTrailBounds, 360, 180, 24);
  let legendX = 8, unavailable = [];
  for (const role of Object.keys(trailStyles)) {
    const bone = side.clip.trails[role];
    if (bone == null) { unavailable.push(trailStyles[role][1]); continue; }
    const points = trailPoints(side, bone), [color, label] = trailStyles[role];
    svg.append(svgElement("path", { d: pathData(points, map), fill: "none", stroke: color, "stroke-width": 2, "data-role": role }));
    const frame = selectedFrames ? selectedFrames[name] : Math.round(phase * Math.max(0, points.length - 1));
    const selected = map(points[frame]);
    if (finitePoint(selected)) svg.append(svgElement("circle", { cx: selected[0], cy: selected[1], r: 3, fill: color, "data-role-dot": role }));
    svg.append(svgElement("text", { x: legendX, y: 14, fill: color }, label));
    legendX += label.length * 7 + 16;
  }
  const missing = unavailable.length ? ` · unavailable: ${unavailable.join(", ")}` : "";
  svg.append(svgElement("text", { x: 8, y: 174, fill: "#aab1c5" }, `top-down X/Z metres · shared scale across both inputs${missing}`));
}

function drawGait(name, phase) {
  const side = data[name], svg = q(`${name}-gait`), gait = side.contexts.gait; svg.replaceChildren();
  if (!gait) { svg.textContent = "gait unavailable: hips and bilateral foot/toe roles did not all resolve"; return; }
  const series = { left: [], right: [] };
  for (let frame = 0; frame < side.clip.frames; frame++) {
    const hipsY = posePoint(side, frame, gait.hips)[1];
    series.left.push(posePoint(side, frame, gait.left)[1] - hipsY);
    series.right.push(posePoint(side, frame, gait.right)[1] - hipsY);
  }
  const yrange = finiteRange(series.left.concat(series.right));
  if (!yrange) { svg.textContent = "gait drawing unavailable: sampled relative heights are non-finite; stance and coverage evidence remain listed"; return; }
  const span = Math.max(.001, yrange[1] - yrange[0]);
  const x = (frame) => 20 + frame * 320 / Math.max(1, side.clip.frames - 1);
  const y = (value) => 150 - (value - yrange[0]) * 120 / span;
  for (const stance of side.contexts.stances) for (const run of stance.runs) {
    const selected = selectedContext && selectedContext.name === name && selectedContext.kind === "stance" && selectedContext.value.bone === stance.bone;
    svg.append(svgElement("rect", { x: x(run.start_frame), y: 24, width: Math.max(2, x(run.end_frame) - x(run.start_frame)), height: 130, fill: stance.side === "left" ? "#7aa2f7" : "#e0af68", opacity: selected ? .32 : .16, "data-stance-side": stance.side }));
  }
  for (const [label, color] of [["left", "#7aa2f7"], ["right", "#e0af68"]]) {
    const points = series[label].map((value, frame) => [x(frame), y(value)]);
    svg.append(svgElement("path", { d: pathData(points, (point) => point), fill: "none", stroke: color, "stroke-width": 2, "data-gait-side": label }));
  }
  const frame = selectedFrames ? selectedFrames[name] : Math.round(phase * Math.max(0, side.clip.frames - 1));
  svg.append(svgElement("line", { x1: x(frame), x2: x(frame), y1: 20, y2: 154, stroke: "#f0cb83", "stroke-width": 1 }));
  const roleLabel = (role) => role.replace("_", " ");
  svg.append(svgElement("text", { x: 8, y: 14, fill: "#7aa2f7" }, `${roleLabel(gait.left_role)} height rel hips`));
  svg.append(svgElement("text", { x: 170, y: 14, fill: "#e0af68" }, `${roleLabel(gait.right_role)} height rel hips`));
  svg.append(svgElement("text", { x: 8, y: 174, fill: "#aab1c5" }, "height in metres · shaded runs are sampled foot-slide stance evidence"));
}

let highlight = { before: null, after: null };
function update() {
  const phase = Number(q("scrub").value) / Math.max(1, sharedFrameMax);
  const beforeFrame = drawSide("before", phase, highlight.before), afterFrame = drawSide("after", phase, highlight.after);
  drawRootComparison(phase); drawTrails("before", phase); drawTrails("after", phase); drawGait("before", phase); drawGait("after", phase);
  const time = (side, frame) => Number.isFinite(side.clip.times[frame]) ? side.clip.times[frame].toFixed(3) : "unavailable";
  q("times").textContent = `before ${time(data.before, beforeFrame)}s · after ${time(data.after, afterFrame)}s (normalized phase; not a time warp)`;
}
function summary(side) { return `primary ${side.identity.sha256} · ${side.identity.bytes} bytes · complete closure ${side.dependency_closure_identity.sha256} · clip ${side.clip.name}`; }
function subjectBone(row) {
  if (row.bone && namedBone.has(row.bone)) return namedBone.get(row.bone);
  const nodeName = row.node && row.node.match(/\(([^()]*)\)$/);
  return nodeName && namedBone.has(nodeName[1]) ? namedBone.get(nodeName[1]) : null;
}
function nearestFrame(side, time) {
  return side.clip.times.reduce((best, value, index) => Math.abs(value - time) < Math.abs(side.clip.times[best] - time) ? index : best, 0);
}
function selectFinding(name, index) {
  const row = data[name].findings[index], side = data[name]; if (!row) return;
  const frame = nearestFrame(side, row.time == null ? 0 : row.time), phase = frame / Math.max(1, side.clip.frames - 1);
  selectedFrames = { before: Math.round(phase * Math.max(0, data.before.clip.frames - 1)), after: Math.round(phase * Math.max(0, data.after.clip.frames - 1)) };
  selectedFrames[name] = frame; q("scrub").value = Math.round(sharedFrameMax * phase);
  highlight = { before: null, after: null }; highlight[name] = subjectBone(row); selectedContext = null;
  const seam = side.contexts.seams.find((context) => context.finding_anchor === row.anchor);
  const structural = side.contexts.structural.find((context) => context.finding_anchor === row.anchor);
  const stance = row.check === "foot-slide" ? side.contexts.stances.find((context) => context.bone_name === row.bone) : null;
  if (seam) selectedContext = { name, kind: "seam", value: seam };
  else if (structural) selectedContext = { name, kind: "structural", value: structural };
  else if (stance) selectedContext = { name, kind: "stance", value: stance };
  update();
}
function list(name, kind) {
  const side = data[name], target = q(`${name}-${kind}`), rows = side[kind];
  if (!rows.length) { target.textContent = kind === "findings" ? "no findings" : "none"; return; }
  rows.forEach((row, index) => {
    const item = document.createElement("li");
    if (kind === "findings") {
      const token = row.anchor.replace(/^finding-/, "");
      item.id = `finding-${name}-${token}`;
      item.textContent = `${row.severity} · ${row.check} · ${row.bone || row.node || "no mapped subject"}${row.time == null ? "" : ` @${row.time.toFixed(3)}s`} — ${row.message}`;
      const timeAnchor = document.createElement("span"); timeAnchor.id = `time-${name}-${token}`; item.append(timeAnchor);
      item.className = "finding"; item.addEventListener("click", () => { selectFinding(name, index); item.scrollIntoView({ block: "nearest" }); item.focus && item.focus(); });
    } else {
      item.id = `${kind.slice(0, -1)}-${name}-${index}`;
      item.textContent = `${row.check_id} · ${row.code} — ${row.message}`;
    }
    target.append(item);
  });
}
function listContexts(name) {
  const side = data[name], target = q(`${name}-contexts`), rows = [];
  for (const stance of side.contexts.stances) rows.push({ className: "context", text: `${stance.side} sampled stance · selected ${stance.selected_role} ${stance.bone_name} · ${stance.runs.map((run) => `${run.start_s.toFixed(3)}–${run.end_s.toFixed(3)}s`).join(", ") || "no retained adjacent-frame run"} · contact-height ${stance.contact_height_m.toFixed(3)} m` });
  for (const seam of side.contexts.seams) rows.push({ className: "context", text: `${seam.check} endpoint pose evidence · first ${seam.first_s.toFixed(3)}s · last ${seam.last_s.toFixed(3)}s · affected ${seam.subject_bone_name || "bone unavailable"}` });
  for (const structural of side.contexts.structural) rows.push({ className: "context structural", text: `${structural.check} · ${structural.label} · ${structural.subject_bone_name || "subject unavailable"}` });
  if (!rows.length) { target.textContent = "no matrix-specific finding context; see coverage and findings"; return; }
  for (const row of rows) { const item = document.createElement("li"); item.className = row.className; item.textContent = row.text; target.append(item); }
}
for (const name of ["before", "after"]) {
  q(`${name}-identity`).textContent = summary(data[name]);
  q(`clip-${name}`).id = `${name}-${data[name].clip.anchor}`;
  listContexts(name); list(name, "findings"); list(name, "gaps");
  q(`${name}-predictions`).textContent = JSON.stringify({ provenance: data[name].prediction_provenance, predictions: data[name].predictions }, null, 2);
}
function selectHash() {
  const match = location.hash.match(/^#(?:finding|time)-(before|after)-([a-f0-9]{16})$/);
  if (!match) return;
  const index = data[match[1]].findings.findIndex((row) => row.anchor === `finding-${match[2]}`);
  if (index >= 0) selectFinding(match[1], index);
}
q("scrub").addEventListener("input", () => { selectedFrames = null; selectedContext = null; update(); });
window.addEventListener("resize", update); window.addEventListener("hashchange", selectHash); selectHash(); update();
