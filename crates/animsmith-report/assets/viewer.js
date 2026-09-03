// animsmith report viewer — hand-written WebGL2 skeleton renderer.
// Renders exactly the pose-grid frames the checks judged; no animation
// sampling happens here.
"use strict";

const data = JSON.parse(document.getElementById("report-data").textContent);
// Theme and embed switches are applied before anything is measured or
// painted, and the palette is resolved once per theme change rather than per
// frame, so the 3D view always paints what the document shows.
animsmithApplyDocument(animsmithFragmentOptions(location.hash));
let palette = animsmithPalette();
document.getElementById("file").textContent =
  (data.file || "") + "  ·  rig profile: " + (data.profile || "none");

// ---- decode positions ------------------------------------------------
function decodePositions(b64) {
  const raw = atob(b64);
  const bytes = new Uint8Array(raw.length);
  for (let i = 0; i < raw.length; i++) bytes[i] = raw.charCodeAt(i);
  return new Float32Array(bytes.buffer);
}
// The document itself says whether poses are available: an evidence-only
// report renders a notice in place of the canvas, so an absent canvas means
// there is nothing to decode, play back, or draw. Findings, coverage gaps,
// charts, and the source path are unchanged either way.
const canvas = document.getElementById("gl");

// The box a clip's whole pose grid occupies, computed once here rather than
// rescanned whenever the selection changes: fitting a camera to two clips is
// then merging two boxes.
function poseBounds(pos) {
  const min = [1e9, 1e9, 1e9], max = [-1e9, -1e9, -1e9];
  for (let i = 0; i < pos.length; i += 3)
    for (let c = 0; c < 3; c++) {
      min[c] = Math.min(min[c], pos[i + c]);
      max[c] = Math.max(max[c], pos[i + c]);
    }
  return { min, max };
}
for (const clip of data.clips) {
  clip.pos = canvas ? decodePositions(clip.positions) : null;
  clip.bounds = clip.pos ? poseBounds(clip.pos) : null;
}

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
let gl = null, uMvp = null, uPointSize = null, vbo = null;
function shader(type, src) {
  const s = gl.createShader(type);
  gl.shaderSource(s, src);
  gl.compileShader(s);
  if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw gl.getShaderInfoLog(s);
  return s;
}
if (canvas) {
  gl = canvas.getContext("webgl2", { antialias: true });
  const prog = gl.createProgram();
  gl.attachShader(prog, shader(gl.VERTEX_SHADER, VS));
  gl.attachShader(prog, shader(gl.FRAGMENT_SHADER, FS));
  gl.linkProgram(prog);
  gl.useProgram(prog);
  uMvp = gl.getUniformLocation(prog, "mvp");
  uPointSize = gl.getUniformLocation(prog, "pointSize");
  vbo = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, vbo);
  gl.enableVertexAttribArray(0);
  gl.enableVertexAttribArray(1);
  gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 24, 0);
  gl.vertexAttribPointer(1, 3, gl.FLOAT, false, 24, 12);
  gl.enable(gl.DEPTH_TEST);
}

// ---- state ------------------------------------------------------------
// `clip` is the clip the transport drives and `withClip` the optional second
// clip of the same document shown beside it; null is the default, the
// selected clip alone. `frame` is the transport's own position, which
// playback advances continuously.
let clip = data.clips[0] || null;
let withClip = null;
let frame = 0;
let yaw = 0.7, pitch = 0.35, dist = 0;
let center = [0, 1, 0];

// The panes on screen, and everything that tells them apart: which clip,
// which of its own grid frames, the tokens its skeleton is painted in, how
// the colour key names it, and what precedes and follows its time in the
// label. The pose view, the key, the time label, the charts and the camera
// all read this one list, so none of them can disagree about which clip is
// shown at which frame.
let shown = [];

function refreshShown() {
  const phase = clip ? animsmithPhaseOf(clip.frames, frame) : 0;
  // Paired, the two halves are told apart by the report's own left/right
  // tokens; alone, the skeleton keeps the muted bones and ink joints it has
  // always had.
  const paint = withClip
    ? [[clip, "accent", "accent"], [withClip, "warning", "warning"]]
    : [[clip, "muted", "ink"]];
  shown = paint.filter(([c]) => c).map(([c, bones, joints], index) => {
    const key = index === 0 ? c.name : "with " + c.name;
    return {
      clip: c,
      // Every pane is at the same phase, in its own frame count.
      at: animsmithFrameAtPhase(c.frames, phase),
      bones,
      joints,
      key,
      lead: index === 0 ? "" : key + " ",
      trail: index === 0 ? "" : " · " + ANIMSMITH_PHASE_DISCLOSURE,
    };
  });
}

// One camera for whatever is on screen, fitted to the shown clips' bounds
// merged, so a pair is drawn at one scale and a skeleton that travels
// further is visibly the one that travels further. Orbit and zoom move that
// one camera, so both halves turn together.
function fitCamera() {
  const boxes = shown.map((pane) => pane.clip.bounds).filter(Boolean);
  if (!boxes.length) return;
  const min = [0, 1, 2].map((c) => Math.min(...boxes.map((box) => box.min[c])));
  const max = [0, 1, 2].map((c) => Math.max(...boxes.map((box) => box.max[c])));
  center = [(min[0] + max[0]) / 2, (min[1] + max[1]) / 2, (min[2] + max[2]) / 2];
  dist = Math.max(max[0] - min[0], max[1] - min[1], max[2] - min[2], 0.5) * 1.8;
}

// Trail, bone, joint, and clear colours are the shared design tokens, so the
// 3D view follows the light/dark theme with the rest of the document.
const TRAIL_TOKENS = { root: "pass", hips: "accent", left_foot: "warning", right_foot: "error" };

// One pane's pose: bone lines, then joint points, then the role trails, as
// one interleaved position+colour buffer. Every colour is a design token the
// pane names, so the same function draws a lone skeleton and either half of
// a pair.
function buildVertices(palette, pane) {
  const clip = pane.clip;
  const boneColor = animsmithRgb(palette[pane.bones]);
  const jointColor = animsmithRgb(palette[pane.joints]);
  const verts = [];
  const f = pane.at;
  const base = f * boneCount * 3;
  const p = (b) => [clip.pos[base + b * 3], clip.pos[base + b * 3 + 1], clip.pos[base + b * 3 + 2]];
  for (let b = 0; b < boneCount; b++) {
    if (parents[b] < 0) continue;
    verts.push(...p(parents[b]), ...boneColor, ...p(b), ...boneColor);
  }
  const lineVerts = verts.length / 6;
  for (let b = 0; b < boneCount; b++) verts.push(...p(b), ...jointColor);
  const pointVerts = boneCount;
  // trails: full path of tracked bones up to the current frame.
  const trailRanges = [];
  for (const [name, bone] of Object.entries(clip.trails || {})) {
    const color = animsmithRgb(palette[TRAIL_TOKENS[name]] || palette.muted);
    const start = verts.length / 6;
    for (let tf = 0; tf <= f; tf++) {
      const tb = tf * boneCount * 3 + bone * 3;
      verts.push(clip.pos[tb], clip.pos[tb + 1], clip.pos[tb + 2], ...color);
      // line strip emulation: duplicate interior points for GL_LINES
      if (tf < f) verts.push(clip.pos[tb], clip.pos[tb + 1], clip.pos[tb + 2], ...color);
    }
    trailRanges.push([start, verts.length / 6 - start]);
  }
  return { verts: new Float32Array(verts), lineVerts, pointVerts, trailRanges };
}

// Each pane gets an equal, scissored slice of the one canvas, left to right,
// so a pair is two viewports rather than two canvases and the document's own
// decision that there is no pose view — an evidence-only report carries no
// #gl at all — keeps working unchanged.
function draw() {
  if (!gl || !shown.length) return;
  const dpr = window.devicePixelRatio || 1;
  const w = canvas.clientWidth * dpr, h = canvas.clientHeight * dpr;
  if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; }
  const ground = animsmithRgb(palette.ground);
  const eye = [
    center[0] + dist * Math.cos(pitch) * Math.sin(yaw),
    center[1] + dist * Math.sin(pitch),
    center[2] + dist * Math.cos(pitch) * Math.cos(yaw),
  ];
  gl.enable(gl.SCISSOR_TEST);
  const wide = Math.floor(w / shown.length);
  shown.forEach((pane, index) => {
    const x = index * wide, pw = index === shown.length - 1 ? w - x : wide;
    // Scissoring keeps each pane's clear inside its own slice, so neither
    // pose can paint over the other's.
    gl.viewport(x, 0, pw, h);
    gl.scissor(x, 0, pw, h);
    gl.clearColor(ground[0], ground[1], ground[2], 1);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    const mvp = mul4(perspective(0.9, pw / h, 0.01, 100), lookAt(eye, center, [0, 1, 0]));
    gl.uniformMatrix4fv(uMvp, false, new Float32Array(mvp));

    const { verts, lineVerts, pointVerts, trailRanges } = buildVertices(palette, pane);
    gl.bindBuffer(gl.ARRAY_BUFFER, vbo);
    gl.bufferData(gl.ARRAY_BUFFER, verts, gl.DYNAMIC_DRAW);
    gl.uniform1f(uPointSize, 5 * dpr);
    gl.drawArrays(gl.LINES, 0, lineVerts);
    gl.drawArrays(gl.POINTS, lineVerts, pointVerts);
    for (const [start, count] of trailRanges) gl.drawArrays(gl.LINE_STRIP, start, count);
  });
}

// ---- UI ----------------------------------------------------------------
const clipSelect = document.getElementById("clip-select");
const withSelect = document.getElementById("with-select");
const scrub = document.getElementById("scrub");
const playBtn = document.getElementById("play");
const timeLabel = document.getElementById("time");
// The colour key naming the two halves of the pose view. An evidence-only
// report has no pose view, so it carries no key either; the pairing itself
// still selects which clips' charts are shown.
const paneLabels = document.getElementById("pane-labels");

const option = (value, text) => {
  const opt = document.createElement("option");
  opt.value = value;
  opt.textContent = text;
  return opt;
};
for (const c of data.clips) clipSelect.appendChild(option(c.name, c.name));

// The chrome that follows the selection rather than the transport: the
// second select's own options, which never offer the clip already selected,
// and the colour key naming each half in the token its skeleton is drawn in.
// Both are rebuilt when the selection changes, not on every frame.
function refreshPairing() {
  const chosen = withClip ? withClip.name : "";
  withSelect.replaceChildren();
  withSelect.appendChild(option("", "alone"));
  for (const c of data.clips) {
    if (clip && c.name === clip.name) continue;
    withSelect.appendChild(option(c.name, c.name));
  }
  withSelect.value = chosen;
  // A document with one clip has nothing to pair it with.
  withSelect.disabled = data.clips.length < 2;
  // An evidence-only document draws no halves, so it carries no key to fill.
  if (paneLabels) {
    paneLabels.replaceChildren();
    for (const pane of shown) {
      // textContent throughout: clip names come from the linted asset. The
      // colour is the pane's own token, so the key cannot name a half in a
      // colour that half is not drawn in.
      const span = document.createElement("span");
      span.textContent = pane.key;
      span.style.color = "var(--" + pane.bones + ")";
      paneLabels.appendChild(span);
    }
  }
}

// One clip's position in its own timeline, as the label states it.
const stamp = (pane) =>
  (animsmithPhaseOf(pane.clip.frames, pane.at) * pane.clip.duration).toFixed(3) + "s / " +
  pane.clip.duration.toFixed(3) + "s (frame " + pane.at + ")";

// The transport moved: rebuild the panes and everything that reads them.
// Every source time the label carries is the time of a frame actually drawn,
// and every chart playhead sits at that same frame.
function refresh() {
  refreshShown();
  if (!shown.length) return;
  scrub.value = shown[0].at;
  timeLabel.textContent = shown.map((pane) => pane.lead + stamp(pane) + pane.trail).join(" · ");
  updateCharts();
  draw();
}

// Which clips are shown changed, so the selection-driven chrome and the
// camera follow before the repaint.
function reselect() {
  refreshShown();
  refreshPairing();
  fitCamera();
  refresh();
}

function selectClip(name) {
  clip = data.clips.find((c) => c.name === name) || data.clips[0];
  if (!clip) return;
  clipSelect.value = clip.name;
  scrub.max = clip.frames - 1;
  // A clip cannot be paired with itself, so selecting the paired clip as the
  // primary one leaves it alone rather than drawing it twice.
  if (withClip && withClip.name === clip.name) withClip = null;
  frame = Math.min(frame, clip.frames - 1);
  reselect();
}

// The empty name, an unknown clip, and the clip already selected all mean
// alone, which is the default this state returns to.
function selectWith(name) {
  const found = name ? data.clips.find((c) => c.name === name) : null;
  withClip = found && clip && found.name !== clip.name ? found : null;
  reselect();
}

function setFrame(f) {
  if (!clip) return;
  frame = Math.max(0, Math.min(clip.frames - 1, f));
  refresh();
}

// Declared gait-group membership, keyed by the name each figure carries as
// `data-group`.
const groupMembers = new Map((data.groups || []).map((g) => [g.name, g.members]));

function updateCharts() {
  if (!clip) return;
  const paneOf = new Map(shown.map((pane) => [pane.clip.name, pane]));
  // A group figure's axis is the stride cycle its members were measured on,
  // which excludes the duplicate wrap sample a longer grid repeats, and it is
  // the selected clip's cycle: pairing a second clip beside it adds a pane,
  // not a second phase for this figure to follow.
  const cycle = clip.cycle > 0
    ? Math.min(1, frame / clip.cycle)
    : animsmithPhaseOf(clip.frames, frame);
  for (const fig of document.querySelectorAll(".chart")) {
    // A group figure draws every member of a declared gait group against the
    // others, so it stays visible while the selected clip is one of them —
    // but only then. Its caption describes its members' own source phase, so
    // a clip outside the group must not drive its playhead, and a figure of a
    // group the reader is not looking at is not evidence about the clip they
    // are. The membership comes from the payload rather than the markup: a
    // clip name is arbitrary text, so no separator can pack several into one
    // attribute without a legitimate name splitting into pieces.
    //
    // A clip figure is its clip's own, so it is visible while that clip is
    // one of the panes on screen — the selected one, or the clip paired
    // beside it — and it is driven by that pane's own frame.
    const members = "group" in fig.dataset ? (groupMembers.get(fig.dataset.group) || []) : null;
    const pane = members ? null : paneOf.get(fig.dataset.clip);
    const active = members ? members.includes(clip.name) : Boolean(pane);
    fig.style.display = active ? "" : "none";
    if (!active) continue;
    const u = members ? cycle : animsmithPhaseOf(pane.clip.frames, pane.at);
    const playhead = fig.querySelector(".playhead");
    if (playhead) {
      const pad = parseFloat(fig.dataset.pad);
      const x = pad + parseFloat(fig.dataset.plotw) * u;
      playhead.setAttribute("x1", x);
      playhead.setAttribute("x2", x);
    }
    const dot = fig.querySelector(".pathdot");
    const points = fig.querySelector(".pathpoints");
    if (pane && dot && points) {
      // One entry per judged frame, so the index is the frame this pane
      // draws — the same frame its pose and its playhead are at. A frame the
      // clip has no sampled position for carries no coordinate, and the dot
      // is hidden for it rather than placed at some other frame's position.
      // The reader would otherwise be shown a place the character is not,
      // and before a leading gap, one it has not reached yet.
      const pts = points.innerHTML.split(";");
      const at = pts[Math.min(pts.length - 1, pane.at)].split(",");
      if (at.length === 2) {
        dot.setAttribute("cx", at[0]);
        dot.setAttribute("cy", at[1]);
        dot.removeAttribute("display");
      } else {
        dot.setAttribute("display", "none");
      }
    }
  }
}

clipSelect.addEventListener("change", () => { frame = 0; selectClip(clipSelect.value); });
// Pairing is not a position, so it neither moves the transport nor stops
// it — a running report keeps playing and the second clip follows from the
// frame the first is already on.
withSelect.addEventListener("change", () => selectWith(withSelect.value));
scrub.addEventListener("input", () => { pausePlayback(); setFrame(+scrub.value); });

// The frame loop's ownership lives in the shared runtime, so both documents
// stop and restart playback the same way.
let last = 0;
const playLoop = animsmithFrameLoop((now) => {
  if (!clip) return;
  const dt = (now - last) / 1000;
  last = now;
  const fps = (clip.frames - 1) / clip.duration;
  let f = frame + dt * fps;
  if (f > clip.frames - 1) f = 0; // wrap like the runtime does
  setFrame(f);
});
function pausePlayback() {
  if (!playLoop.stop()) return;
  playBtn.textContent = "▶";
  playBtn.setAttribute("aria-label", "Play the clip");
}
playBtn.addEventListener("click", () => {
  if (!canvas) return;
  if (playLoop.running) { pausePlayback(); return; }
  last = performance.now();
  playLoop.start();
  playBtn.textContent = "⏸";
  playBtn.setAttribute("aria-label", "Pause the clip");
});

// orbit controls
let dragging = false, lastX = 0, lastY = 0;
if (canvas) {
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
}

// findings panel
const list = document.getElementById("findings");
if (!data.findings.length) {
  const li = document.createElement("li");
  li.textContent = "clean — no findings";
  list.appendChild(li);
}
const findingItems = [];
// One selection path for a click and for a `#finding=` deep link: scrub to
// the judged frame and mark the row.
function selectFinding(index) {
  const f = data.findings[index], item = findingItems[index];
  for (const other of findingItems) other.classList.remove("selected");
  // An index this document cannot honour leaves nothing selected, which is
  // the default state, rather than keeping a previous selection.
  if (!f || !item) return;
  pausePlayback();
  item.classList.add("selected");
  if (item.scrollIntoView) item.scrollIntoView({ block: "nearest" });
  if (!f.clip) return;
  selectClip(f.clip);
  if (f.time != null && clip && clip.duration > 0)
    setFrame((f.time / clip.duration) * (clip.frames - 1));
}
data.findings.forEach((f, index) => {
  // Built with textContent throughout: clip/bone/node names and messages
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
  add("b", "", [f.clip, f.bone, f.node].filter(Boolean).join(" · "));
  if (f.time != null) li.appendChild(document.createTextNode(` @${f.time.toFixed(3)}s`));
  li.appendChild(document.createElement("br"));
  li.appendChild(document.createTextNode(f.message));
  li.addEventListener("click", () => selectFinding(index));
  findingItems.push(li);
  list.appendChild(li);
});

const gapList = document.getElementById("gaps");
if (!data.gaps.length) {
  const li = document.createElement("li");
  li.textContent = "none";
  gapList.appendChild(li);
}
for (const gap of data.gaps) {
  const li = document.createElement("li");
  const subject = gap.scope?.subject == null ? "" : ` · ${gap.scope.subject}`;
  const scope = gap.scope == null ? "" : ` · ${gap.scope.code}${subject}`;
  li.textContent = `${gap.check_id} · ${gap.code}${scope} — ${gap.message}`;
  gapList.appendChild(li);
}

// Prediction facets remain distinct from content findings and ordinary gaps.
const predictionList = document.getElementById("predictions");
if (!data.prediction_provenance) {
  const li = document.createElement("li");
  li.textContent = "engine-neutral — no profile resolved";
  predictionList.appendChild(li);
} else if (!data.predictions.length) {
  const li = document.createElement("li");
  const selection = data.prediction_provenance.profile.selection;
  li.textContent = `profile ${selection.family} revision ${selection.profile_revision} resolved; ` +
    "no engine-backed checks emitted predictions";
  predictionList.appendChild(li);
}
for (const row of data.predictions) {
  for (const facet of row.prediction.facets) {
    const li = document.createElement("li");
    li.className = "prediction " + facet.state;
    const subject = facet.scope.subject == null ? "" : ` · ${facet.scope.subject}`;
    const state = facet.state === "available"
      ? "available"
      : `required prediction unavailable: ${facet.reasons.join(", ")}`;
    li.textContent = `${row.check_id} · ${facet.scope.code}${subject} — ${state}`;
    predictionList.appendChild(li);
  }
}

// Fragment selection runs once the clip list, charts, and findings exist, so
// a deep link lands exactly where the equivalent click would. Each option that
// appears is applied in order — clip, then with, then finding, then frame — as
// far as this document allows: setFrame clamps a frame past the end to the last
// judged frame, and a clip name the document does not carry selects the first
// clip. A value the parser could not read restores that state's default
// instead: frame 0, the first clip, no selected finding. An option that does
// not appear is left alone.
function applyFragment() {
  const options = animsmithApplyDocument(animsmithFragmentOptions(location.hash));
  palette = animsmithPalette();
  // A clip, a finding or a frame is a reader asking for one position, and a
  // running loop overwrites it on the very next frame. Only `theme` and
  // `embed` leave playback alone.
  if (options.clip !== undefined || options.finding !== undefined || options.frame !== undefined) pausePlayback();
  if (options.clip !== undefined && data.clips.length) {
    const known = data.clips.some((c) => c.name === options.clip);
    selectClip(known ? options.clip : data.clips[0].name);
    setFrame(0);
  }
  // After `clip`, because which clip a name may not equal is the clip this
  // fragment just selected. A name the document does not carry, the selected
  // clip's own name, and a value the parser could not read are all alone.
  if (options.with !== undefined) selectWith(options.with == null ? "" : options.with);
  if (options.finding !== undefined) selectFinding(options.finding);
  if (options.frame !== undefined) setFrame(options.frame == null ? 0 : options.frame);
  draw();
}

window.addEventListener("resize", draw);
window.addEventListener("hashchange", applyFragment);
animsmithOnSchemeChange(() => { palette = animsmithPalette(); draw(); });
if (clip) { selectClip(clip.name); setFrame(0); }
applyFragment();
