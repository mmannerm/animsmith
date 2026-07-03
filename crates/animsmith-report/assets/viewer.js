// animsmith report viewer — hand-written WebGL2 skeleton renderer.
// Renders exactly the pose-grid frames the checks judged; no animation
// sampling happens here.
"use strict";

const data = JSON.parse(document.getElementById("report-data").textContent);
document.getElementById("file").textContent =
  (data.file || "") + "  ·  rig profile: " + (data.profile || "none");

// ---- decode positions ------------------------------------------------
function decodePositions(b64) {
  const raw = atob(b64);
  const bytes = new Uint8Array(raw.length);
  for (let i = 0; i < raw.length; i++) bytes[i] = raw.charCodeAt(i);
  return new Float32Array(bytes.buffer);
}
for (const clip of data.clips) clip.pos = decodePositions(clip.positions);

const boneCount = data.bones.length;
const parents = data.bones.map((b) => b.parent);

// ---- tiny mat4 -------------------------------------------------------
function perspective(fovy, aspect, near, far) {
  const f = 1 / Math.tan(fovy / 2), nf = 1 / (near - far);
  return [f / aspect, 0, 0, 0, 0, f, 0, 0, 0, 0, (far + near) * nf, -1,
          0, 0, 2 * far * near * nf, 0];
}
function lookAt(eye, at, up) {
  const z = norm3(sub3(eye, at));
  const x = norm3(cross3(up, z));
  const y = cross3(z, x);
  return [x[0], y[0], z[0], 0, x[1], y[1], z[1], 0, x[2], y[2], z[2], 0,
          -dot3(x, eye), -dot3(y, eye), -dot3(z, eye), 1];
}
function mul4(a, b) {
  const o = new Array(16).fill(0);
  for (let c = 0; c < 4; c++)
    for (let r = 0; r < 4; r++)
      for (let k = 0; k < 4; k++) o[c * 4 + r] += a[k * 4 + r] * b[c * 4 + k];
  return o;
}
const sub3 = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const dot3 = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross3 = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
function norm3(v) { const l = Math.hypot(...v) || 1; return [v[0] / l, v[1] / l, v[2] / l]; }

// ---- WebGL setup ------------------------------------------------------
const canvas = document.getElementById("gl");
const gl = canvas.getContext("webgl2", { antialias: true });
const VS = `#version 300 es
layout(location=0) in vec3 pos;
layout(location=1) in vec3 color;
uniform mat4 mvp;
uniform float pointSize;
out vec3 vColor;
void main() { gl_Position = mvp * vec4(pos, 1.0); gl_PointSize = pointSize; vColor = color; }`;
const FS = `#version 300 es
precision mediump float;
in vec3 vColor;
out vec4 frag;
void main() { frag = vec4(vColor, 1.0); }`;
function shader(type, src) {
  const s = gl.createShader(type);
  gl.shaderSource(s, src);
  gl.compileShader(s);
  if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw gl.getShaderInfoLog(s);
  return s;
}
const prog = gl.createProgram();
gl.attachShader(prog, shader(gl.VERTEX_SHADER, VS));
gl.attachShader(prog, shader(gl.FRAGMENT_SHADER, FS));
gl.linkProgram(prog);
gl.useProgram(prog);
const uMvp = gl.getUniformLocation(prog, "mvp");
const uPointSize = gl.getUniformLocation(prog, "pointSize");
const vbo = gl.createBuffer();
gl.bindBuffer(gl.ARRAY_BUFFER, vbo);
gl.enableVertexAttribArray(0);
gl.enableVertexAttribArray(1);
gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 24, 0);
gl.vertexAttribPointer(1, 3, gl.FLOAT, false, 24, 12);
gl.enable(gl.DEPTH_TEST);

// ---- state ------------------------------------------------------------
let clip = data.clips[0] || null;
let frame = 0;
let playing = false;
let yaw = 0.7, pitch = 0.35, dist = 0;
let center = [0, 1, 0];

function fitCamera() {
  if (!clip) return;
  let min = [1e9, 1e9, 1e9], max = [-1e9, -1e9, -1e9];
  for (let i = 0; i < clip.pos.length; i += 3)
    for (let c = 0; c < 3; c++) {
      min[c] = Math.min(min[c], clip.pos[i + c]);
      max[c] = Math.max(max[c], clip.pos[i + c]);
    }
  center = [(min[0] + max[0]) / 2, (min[1] + max[1]) / 2, (min[2] + max[2]) / 2];
  dist = Math.max(max[0] - min[0], max[1] - min[1], max[2] - min[2], 0.5) * 1.8;
}

const TRAIL_COLORS = { root: [0.61, 0.81, 0.42], hips: [0.48, 0.64, 0.97],
                       left_foot: [0.88, 0.69, 0.41], right_foot: [0.96, 0.46, 0.56] };

function buildVertices() {
  // interleaved pos+color: bone lines, joint points appended after.
  const verts = [];
  const f = Math.round(frame);
  const base = f * boneCount * 3;
  const p = (b) => [clip.pos[base + b * 3], clip.pos[base + b * 3 + 1], clip.pos[base + b * 3 + 2]];
  for (let b = 0; b < boneCount; b++) {
    if (parents[b] < 0) continue;
    verts.push(...p(parents[b]), 0.55, 0.6, 0.75, ...p(b), 0.55, 0.6, 0.75);
  }
  const lineVerts = verts.length / 6;
  for (let b = 0; b < boneCount; b++) verts.push(...p(b), 0.85, 0.87, 0.95);
  const pointVerts = boneCount;
  // trails: full path of tracked bones up to the current frame.
  let trailStart = lineVerts + pointVerts;
  const trailRanges = [];
  for (const [name, bone] of Object.entries(clip.trails || {})) {
    const color = TRAIL_COLORS[name] || [0.6, 0.6, 0.6];
    const start = verts.length / 6;
    for (let tf = 0; tf <= f; tf++) {
      const tb = tf * boneCount * 3 + bone * 3;
      verts.push(clip.pos[tb], clip.pos[tb + 1], clip.pos[tb + 2], ...color);
      if (tf > 0 && tf <= f - 0) {
        // line strip emulation: duplicate interior points for GL_LINES
        if (tf < f) {
          verts.push(clip.pos[tb], clip.pos[tb + 1], clip.pos[tb + 2], ...color);
        }
      }
    }
    trailRanges.push([start, verts.length / 6 - start]);
  }
  return { verts: new Float32Array(verts), lineVerts, pointVerts, trailStart, trailRanges };
}

function draw() {
  if (!clip) return;
  const dpr = window.devicePixelRatio || 1;
  const w = canvas.clientWidth * dpr, h = canvas.clientHeight * dpr;
  if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; }
  gl.viewport(0, 0, w, h);
  gl.clearColor(0.09, 0.09, 0.13, 1);
  gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);

  const eye = [
    center[0] + dist * Math.cos(pitch) * Math.sin(yaw),
    center[1] + dist * Math.sin(pitch),
    center[2] + dist * Math.cos(pitch) * Math.cos(yaw),
  ];
  const mvp = mul4(perspective(0.9, w / h, 0.01, 100), lookAt(eye, center, [0, 1, 0]));
  gl.uniformMatrix4fv(uMvp, false, new Float32Array(mvp));

  const { verts, lineVerts, pointVerts, trailRanges } = buildVertices();
  gl.bindBuffer(gl.ARRAY_BUFFER, vbo);
  gl.bufferData(gl.ARRAY_BUFFER, verts, gl.DYNAMIC_DRAW);
  gl.uniform1f(uPointSize, 5 * dpr);
  gl.drawArrays(gl.LINES, 0, lineVerts);
  gl.drawArrays(gl.POINTS, lineVerts, pointVerts);
  for (const [start, count] of trailRanges) gl.drawArrays(gl.LINE_STRIP, start, count);
}

// ---- UI ----------------------------------------------------------------
const clipSelect = document.getElementById("clip-select");
const scrub = document.getElementById("scrub");
const playBtn = document.getElementById("play");
const timeLabel = document.getElementById("time");

for (const c of data.clips) {
  const opt = document.createElement("option");
  opt.value = c.name;
  opt.textContent = c.name;
  clipSelect.appendChild(opt);
}

function selectClip(name) {
  clip = data.clips.find((c) => c.name === name) || data.clips[0];
  if (!clip) return;
  clipSelect.value = clip.name;
  scrub.max = clip.frames - 1;
  frame = Math.min(frame, clip.frames - 1);
  fitCamera();
  updateCharts();
  draw();
}

function setFrame(f) {
  if (!clip) return;
  frame = Math.max(0, Math.min(clip.frames - 1, f));
  scrub.value = Math.round(frame);
  const t = (frame / (clip.frames - 1)) * clip.duration;
  timeLabel.textContent = t.toFixed(3) + "s / " + clip.duration.toFixed(3) + "s (frame " +
    Math.round(frame) + ")";
  updateCharts();
  draw();
}

function updateCharts() {
  if (!clip) return;
  const u = clip.frames > 1 ? frame / (clip.frames - 1) : 0;
  for (const fig of document.querySelectorAll(".chart")) {
    const active = fig.dataset.clip === clip.name;
    fig.style.display = active ? "" : "none";
    if (!active) continue;
    const playhead = fig.querySelector(".playhead");
    if (playhead) {
      const pad = parseFloat(fig.dataset.pad);
      const x = pad + parseFloat(fig.dataset.plotw) * u;
      playhead.setAttribute("x1", x);
      playhead.setAttribute("x2", x);
    }
    const dot = fig.querySelector(".pathdot");
    const points = fig.querySelector(".pathpoints");
    if (dot && points) {
      const pts = points.innerHTML.split(";");
      const i = Math.min(pts.length - 1, Math.round(u * (pts.length - 1)));
      const [cx, cy] = pts[i].split(",");
      dot.setAttribute("cx", cx);
      dot.setAttribute("cy", cy);
    }
  }
}

clipSelect.addEventListener("change", () => { frame = 0; selectClip(clipSelect.value); });
scrub.addEventListener("input", () => { playing = false; playBtn.textContent = "▶"; setFrame(+scrub.value); });
playBtn.addEventListener("click", () => {
  playing = !playing;
  playBtn.textContent = playing ? "⏸" : "▶";
  if (playing) { last = performance.now(); requestAnimationFrame(tick); }
});

let last = 0;
function tick(now) {
  if (!playing || !clip) return;
  const dt = (now - last) / 1000;
  last = now;
  const fps = (clip.frames - 1) / clip.duration;
  let f = frame + dt * fps;
  if (f > clip.frames - 1) f = 0; // wrap like the runtime does
  setFrame(f);
  requestAnimationFrame(tick);
}

// orbit controls
let dragging = false, lastX = 0, lastY = 0;
canvas.addEventListener("mousedown", (e) => { dragging = true; lastX = e.clientX; lastY = e.clientY; });
window.addEventListener("mouseup", () => (dragging = false));
window.addEventListener("mousemove", (e) => {
  if (!dragging) return;
  yaw -= (e.clientX - lastX) * 0.01;
  pitch = Math.max(-1.4, Math.min(1.4, pitch + (e.clientY - lastY) * 0.01));
  lastX = e.clientX; lastY = e.clientY;
  draw();
});
canvas.addEventListener("wheel", (e) => {
  e.preventDefault();
  dist *= Math.exp(e.deltaY * 0.001);
  draw();
}, { passive: false });

// findings panel
const list = document.getElementById("findings");
if (!data.findings.length) {
  const li = document.createElement("li");
  li.textContent = "clean — no findings";
  list.appendChild(li);
}
for (const f of data.findings) {
  // Built with textContent throughout: clip/bone names and messages
  // come from the linted asset, i.e. untrusted input.
  const li = document.createElement("li");
  li.className = "finding " + f.severity;
  const add = (tag, cls, text) => {
    const el = document.createElement(tag);
    if (cls) el.className = cls;
    el.textContent = text;
    li.appendChild(el);
    return el;
  };
  add("span", "sev", f.severity);
  add("code", "", f.check);
  li.appendChild(document.createTextNode(" "));
  add("b", "", [f.clip, f.bone].filter(Boolean).join(" · "));
  if (f.time != null) li.appendChild(document.createTextNode(` @${f.time.toFixed(3)}s`));
  li.appendChild(document.createElement("br"));
  li.appendChild(document.createTextNode(f.message));
  if (f.clip) {
    li.addEventListener("click", () => {
      selectClip(f.clip);
      if (f.time != null && clip && clip.duration > 0)
        setFrame((f.time / clip.duration) * (clip.frames - 1));
    });
  }
  list.appendChild(li);
}

window.addEventListener("resize", draw);
if (clip) { selectClip(clip.name); setFrame(0); }
