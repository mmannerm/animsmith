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
// The document says whether poses are available: an evidence-only comparison
// renders a notice in place of every pose view, so an absent pose surface
// means there is nothing to decode and nothing to draw. The notice is
// rendered once per surface by the report itself and never repeated here.
// Findings, coverage, contexts, and identities are unchanged either way.
const posesOmitted = q("before-gl") === null;
for (const side of [data.before, data.after]) side.clip.pos = posesOmitted ? new Float32Array(0) : decode(side.clip.positions);
q("mapping").textContent = data.correspondence.disclosure;
const parents = data.bones.map((bone) => bone.parent);
const sharedFrameMax = Math.max(data.before.clip.frames, data.after.clip.frames) - 1;
q("scrub").max = sharedFrameMax;
if (posesOmitted) q("scrub").disabled = true;
let selectedFrames = null;
let selectedContext = null;
// Canvas and SVG paint comes from the shared design tokens, so both panels
// follow the document theme. `--error` marks the subject of a selected
// finding and the gait playhead: the most salient token in either theme.
// Resolved once per theme change and handed to the drawing functions.
let documentPalette = animsmithPalette();

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
const poseBounds = (side) => {
  const xs = [], ys = [];
  for (let index = 0; index < side.clip.pos.length; index += 3) {
    xs.push(side.clip.pos[index]); ys.push(side.clip.pos[index + 1]);
  }
  const x = finiteRange(xs), y = finiteRange(ys);
  return x && y ? { x, y } : null;
};
const sidePoseBounds = { before: poseBounds(data.before), after: poseBounds(data.after) };
// One camera across both pose panes. Fitting each side to its own extent
// draws two skeletons the repair left identical at two different sizes, and
// a reader comparing them sees a change the clips do not contain. A side
// whose own samples are all non-finite still says so on its own.
const sharedPoseBounds = [sidePoseBounds.before, sidePoseBounds.after].reduce((merged, bounds) => {
  if (!bounds) return merged;
  if (!merged) return bounds;
  return {
    x: [Math.min(merged.x[0], bounds.x[0]), Math.max(merged.x[1], bounds.x[1])],
    y: [Math.min(merged.y[0], bounds.y[0]), Math.max(merged.y[1], bounds.y[1])],
  };
}, null);
const frameFinite = (side, frame) => {
  if (!Number.isInteger(frame) || frame < 0 || frame >= side.clip.frames) return false;
  for (let bone = 0; bone < data.bones.length; bone++) if (!finitePoint(posePoint(side, frame, bone))) return false;
  return true;
};
const sharedRootBounds = boundsFor(["root"]);
const sharedTrailBounds = boundsFor(["root", "hips", "left_foot", "right_foot"]);

function svgElement(tag, attrs, label) {
  const element = document.createElementNS(svgNs, tag);
  for (const [name, value] of Object.entries(attrs || {})) element.setAttribute(name, value);
  if (label != null) element.textContent = label;
  return element;
}
// SVG shows no text unless an element carries it, so a panel that cannot be
// drawn marks the empty box with a <text> child. Assigning to an <svg>'s
// textContent would leave a blank box in a real browser.
//
// The marker is the headline only; the sentence explaining it goes in the
// panel's caption, which the browser wraps. A whole sentence drawn into the
// box would be cut at its edge exactly like a caption would.
function svgMessage(id, svg, palette, message) {
  svg.replaceChildren();
  const headline = message.split(":")[0];
  svg.append(svgElement("text", { x: 8, y: 20, fill: palette.muted }, headline));
  panelCaption(id, message);
}
// A panel's caption is the HTML paragraph the document emits beside it,
// not text drawn into the picture. SVG does not wrap, so a caption drawn
// inside a panel is cut at its edge on a narrow column — and guessing
// where to break it from a character count ignores the width the reader
// actually has. The browser reflows a <p> at any width for free.
function panelCaption(id, message) {
  const caption = q(`${id}-caption`);
  if (caption) caption.textContent = message;
}
// The legend metrics for the type scale `comparison.css` sets on panel
// text through `--chart-type`: `advance` is the average glyph width at
// that size and `gap` the space between entries. Both only lay entries out
// left to right, so an approximation is enough — the size itself lives in
// the stylesheet, which is what actually draws the glyphs.
const CHART_TYPE = { advance: 4.6, gap: 16 };
const legendAdvance = (label) => label.length * CHART_TYPE.advance + CHART_TYPE.gap;
// Plot units per metre for a set of bounds fitted to a padded box.
function topDownScale(bounds, width, height, pad) {
  const spanX = Math.max(.001, bounds.x[1] - bounds.x[0]);
  const spanZ = Math.max(.001, bounds.z[1] - bounds.z[0]);
  return Math.min((width - 2 * pad) / spanX, (height - 2 * pad) / spanZ);
}
// The same projection at a scale the caller chooses, so two panels can draw
// their own bounds at one shared metres scale.
function topDownMapAt(bounds, width, height, scale) {
  const centerX = (bounds.x[0] + bounds.x[1]) / 2, centerZ = (bounds.z[0] + bounds.z[1]) / 2;
  return (point) => [width / 2 + (point[0] - centerX) * scale, height / 2 - (point[1] - centerZ) * scale];
}
function topDownMap(bounds, width, height, pad) {
  return topDownMapAt(bounds, width, height, topDownScale(bounds, width, height, pad));
}
const TRAIL_PANEL = { width: 360, height: 180, pad: 24 };
// The shared root panel is fitted to its own extent, because a root that
// sways two centimetres drawn at the trail panels' scale is a five-unit blob
// under its own end marks — unreadable rather than merely out of proportion.
// What the old panel actually lacked was the size of what it drew, so the
// panel states it twice: a scale bar in its corner, in the unit a reader
// measures in, and the magnification against the trail panels in its
// caption.
const ROOT_PANEL = { width: 720, height: 180, pad: 28 };
const ROOT_PLOT_W = ROOT_PANEL.width - 2 * ROOT_PANEL.pad;
// Bar lengths, largest first. A reader reads a picture against a round
// number, not against whatever fraction of the extent would have filled some
// share of the panel.
const SCALE_BAR_STEPS_M = [1, 0.5, 0.1, 0.05, 0.01];
// The share of the plot a bar may take before the next step down reads
// better. A step at or under the extent always fits the plot, so this only
// chooses between the ones that do.
const SCALE_BAR_MAX_SHARE = 0.6;
// The longest bar the track is at least as wide as, preferring one that does
// not run across the panel. `null` when the path is narrower than the
// smallest bar, which the caption then says.
function scaleBarStep(extent, scale) {
  const within = SCALE_BAR_STEPS_M.filter((step) => step <= extent);
  if (!within.length) return null;
  return within.find((step) => step * scale <= SCALE_BAR_MAX_SHARE * ROOT_PLOT_W) || within[within.length - 1];
}
const scaleBarLabel = (metres) => metres >= 0.5 ? `${metres} m` : `${Math.round(metres * 100)} cm`;
// How close a track's two ends must be to count as closing on itself: the
// same millimetre the single-clip chart calls stationary, and finer than any
// clip an engine distinguishes from a closed loop.
const CLOSED_PATH_M = 0.001;
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

function drawSide(name, palette, phase, highlighted) {
  const side = data[name];
  const frame = selectedFrames ? selectedFrames[name] : Math.round(phase * Math.max(0, side.clip.frames - 1));
  const canvas = q(`${name}-gl`);
  if (!canvas) return frame;
  const context = canvas.getContext("2d");
  const dpr = window.devicePixelRatio || 1, width = canvas.clientWidth * dpr, height = canvas.clientHeight * dpr;
  if (canvas.width !== width || canvas.height !== height) { canvas.width = width; canvas.height = height; }
  context.setTransform(dpr, 0, 0, dpr, 0, 0); context.clearRect(0, 0, canvas.clientWidth, canvas.clientHeight);
  const structural = selectedContext && selectedContext.name === name && selectedContext.kind === "structural" ? selectedContext.value : null;
  const bounds = sidePoseBounds[name];
  if (!bounds) {
    const unavailable = "pose drawing unavailable: sampled X/Y positions are non-finite; findings and coverage remain listed";
    q(`${name}-pose-context`).textContent = structural ? `${structural.label} · ${unavailable}` : unavailable;
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
      context.fillStyle = bone === subject ? palette.error : fill;
      context.beginPath(); context.arc(...point, bone === subject ? 6 : 3, 0, Math.PI * 2); context.fill();
    }
  };
  const seam = selectedContext && selectedContext.name === name && selectedContext.kind === "seam" ? selectedContext.value : null;
  if (seam) {
    drawFrame(seam.first_frame, palette.accent, palette.accent, seam.subject_bone);
    drawFrame(seam.last_frame, palette.warning, palette.warning, seam.subject_bone);
    const exact = frameFinite(side, seam.first_frame) && frameFinite(side, seam.last_frame);
    q(`${name}-pose-context`).textContent = exact
      ? `loop seam exact endpoint poses — first ${seam.first_s.toFixed(3)}s blue · last ${seam.last_s.toFixed(3)}s orange · affected ${seam.subject_bone_name || "bone unavailable"}`
      : `loop seam pose drawing incomplete: an endpoint contains non-finite positions; finding and coverage evidence remain listed · affected ${seam.subject_bone_name || "bone unavailable"}`;
  } else {
    drawFrame(frame, palette.muted, palette.ink, highlighted);
    q(`${name}-pose-context`).textContent = structural
      ? frameFinite(side, frame)
        ? structural.label
        : `${structural.label} · pose drawing incomplete: selected frame contains non-finite positions; findings and coverage remain listed`
      : frameFinite(side, frame)
        ? "exact judged pose-grid frame"
        : "pose drawing incomplete: selected frame contains non-finite positions; findings and coverage remain listed";
  }
  return frame;
}

// The two root tracks a comparison draws, and the facts about them the
// caption states: a track the reader can see the direction of, whether each
// one comes back to where it started, and whether the repair moved the root
// at all.
function rootTrack(side) {
  const bone = side.clip.trails.root;
  return bone == null ? null : trailPoints(side, bone);
}
// Identical means every sampled coordinate is the same number, `Object.is`
// so two equally non-finite tracks still count as unchanged. A repair that
// did not touch the root produces exactly this, and the panel then draws one
// line where the legend names two — which is the thing to say in words.
function rootTracksIdentical() {
  const before = rootTrack(data.before), after = rootTrack(data.after);
  if (!before || !after || before.length !== after.length) return null;
  return before.every((point, index) => Object.is(point[0], after[index][0]) && Object.is(point[1], after[index][1]));
}
function trackEnds(points) {
  const finite = points.filter(finitePoint);
  return finite.length ? [finite[0], finite[finite.length - 1]] : null;
}
function trackClosure(name) {
  const points = rootTrack(data[name]);
  const ends = points && trackEnds(points);
  if (!ends) return null;
  const gap = Math.hypot(ends[1][0] - ends[0][0], ends[1][1] - ends[0][1]);
  return gap < CLOSED_PATH_M ? `${name} closes on itself` : `${name} ends ${gap.toFixed(3)} m from its start`;
}

function drawRootComparison(palette, phase) {
  const svg = q("comparison-root-path");
  if (!svg) return;
  svg.replaceChildren();
  if (!sharedRootBounds) { svgMessage("comparison-root-path", svg, palette, "root trajectories unavailable: no input has finite resolved Root samples"); return; }
  const scale = topDownScale(sharedRootBounds, ROOT_PANEL.width, ROOT_PANEL.height, ROOT_PANEL.pad);
  const map = topDownMapAt(sharedRootBounds, ROOT_PANEL.width, ROOT_PANEL.height, scale);
  // A repair that leaves the root alone draws the two trajectories on top of
  // each other, and a solid `after` over a solid `before` hides the before
  // entirely — the panel then reads as one input rather than as two that
  // agree. The after path is dashed, thinner and translucent, and its dot
  // smaller, so a coincident pair still reads as two.
  const styles = {
    before: { color: palette.accent, dash: null, width: 3, opacity: 1, radius: 6 },
    after: { color: palette.warning, dash: "7 5", width: 1.6, opacity: .7, radius: 3.5 },
  };
  const identical = rootTracksIdentical();
  const place = markLanes(ROOT_PANEL);
  for (const name of ["before", "after"]) {
    const points = rootTrack(data[name]), style = styles[name];
    if (!points) continue;
    const path = { d: pathData(points, map), fill: "none", stroke: style.color, "stroke-width": style.width, opacity: style.opacity, "data-root-side": name };
    if (style.dash) path["stroke-dasharray"] = style.dash;
    svg.append(svgElement("path", path));
    const frame = selectedFrames ? selectedFrames[name] : Math.round(phase * Math.max(0, points.length - 1));
    const selected = map(points[frame]);
    if (finitePoint(selected)) svg.append(svgElement("circle", { cx: selected[0], cy: selected[1], r: style.radius, fill: style.color, "data-root-dot": name }));
    // Every track carries its own marks, including an after track the repair
    // left identical: its ring is narrower than the before one, and a square
    // that would land on one already drawn takes the next lane.
    const ends = trackEnds(points);
    if (ends) appendTrackEnds(svg, [map(ends[0]), map(ends[1])], style, place, palette, "data-root-marker", name);
  }
  const rootState = (side) => {
    const points = rootTrack(side);
    if (!points) return "unavailable";
    const finite = points.filter(finitePoint).length;
    return finite === 0 ? "unavailable" : finite === points.length ? "path" : "path incomplete";
  };
  const beforeState = rootState(data.before), afterState = rootState(data.after);
  const beforeLabel = `before root ${beforeState}`;
  const afterLabel = `after root ${afterState}`;
  // Entries are laid out from their own widths rather than from fixed
  // stops. At the chart type scale the old stops left the legend strung
  // across the panel with a hole in the middle.
  let legendX = 18;
  for (const [label, state, token] of [[beforeLabel, beforeState, "accent"], [afterLabel, afterState, "warning"]]) {
    svg.append(svgElement("text", { x: legendX, y: 20, fill: state === "unavailable" ? palette.muted : palette[token] }, label));
    legendX += legendAdvance(label);
  }
  appendMarksLegend(svg, palette, legendX, 20);
  // The bar in the corner: how long a round number is in this panel. It is
  // the one thing a fitted plot cannot say by its shape, and a reader
  // measures the drawing against it directly rather than against a ratio.
  const extent = Math.max(sharedRootBounds.x[1] - sharedRootBounds.x[0], sharedRootBounds.z[1] - sharedRootBounds.z[0]);
  const step = scaleBarStep(extent, scale);
  const barY = ROOT_PANEL.height - 12, barX = ROOT_PANEL.pad;
  if (step) {
    const barLength = step * scale;
    svg.append(svgElement("line", { x1: barX, x2: barX + barLength, y1: barY, y2: barY, stroke: palette.muted, "stroke-width": 1.5, "data-scale-bar-m": step }));
    for (const at of [barX, barX + barLength]) {
      svg.append(svgElement("line", { x1: at, x2: at, y1: barY - 4, y2: barY + 4, stroke: palette.muted, "stroke-width": 1.5 }));
    }
    svg.append(svgElement("text", { x: barX + barLength + 6, y: barY + 3, fill: palette.muted }, scaleBarLabel(step)));
  }
  // And the magnification against the panels beside it, so the two pictures
  // can be read against each other even though neither is drawn at the
  // other's scale.
  const trailScale = topDownScale(sharedTrailBounds || sharedRootBounds, TRAIL_PANEL.width, TRAIL_PANEL.height, TRAIL_PANEL.pad);
  const magnification = scale / trailScale;
  const relation = magnification >= 1
    ? `magnified ${magnification.toFixed(1)}× relative to the trail panels below, where the same paths appear at the trail scale`
    : `drawn at ${magnification.toFixed(1)}× the trail panels' scale below, where the same paths appear at that scale`;
  // The heading carries the factor too: the panel is one of three drawings of
  // the same two root paths, and a reader meeting the big one first has to be
  // told that the small ones below are it.
  const title = q("root-panel-title");
  if (title) title.textContent = magnification >= 1
    ? `Root path, before over after (magnified ${magnification.toFixed(1)}×)`
    : `Root path, before over after (${magnification.toFixed(1)}× the trail panels' scale)`;
  const sameness = identical === null
    ? []
    : [identical
      ? "identical, the after path lies under the before path: its dot is the smaller one and its end mark steps out beside the before one"
      : "the before and after paths differ"];
  // One clip's contract, or both when the two sides declare different ones.
  const guidance = data.before.guidance.root_path === data.after.guidance.root_path
    ? [data.before.guidance.root_path]
    : [`before — ${data.before.guidance.root_path}`, `after — ${data.after.guidance.root_path}`];
  const closures = ["before", "after"].map(trackClosure).filter(Boolean);
  panelCaption("comparison-root-path", [
    `the two root paths overlaid so their shapes can be compared, ${relation}`,
    ...guidance,
    "before solid, after dashed and translucent",
    ...sameness,
    ...closures,
    `X ${sharedRootBounds.x[0].toFixed(3)}…${sharedRootBounds.x[1].toFixed(3)} m`,
    `Z ${sharedRootBounds.z[0].toFixed(3)}…${sharedRootBounds.z[1].toFixed(3)} m`,
    `the paths span ${extent.toFixed(3)} m at their widest`,
    "fitted to this panel",
    step ? `the corner bar is ${scaleBarLabel(step)}` : "no scale bar: the paths are narrower than 1 cm",
  ].join(" · "));
}

// Role → its token, its legend label, and how its line is drawn. Seen from
// above, the root and the hips run within a few centimetres of each other for
// most of a clip, so the hips trail is dashed and translucent over a solid
// root — the same device the root panel uses for a coincident after track, so
// one reading serves both panels.
const TRAIL_TOKENS = {
  root: { token: "pass", label: "root", dash: null, opacity: 1, width: 2, radius: 3 },
  hips: { token: "note", label: "hips", dash: "7 5", opacity: .7, width: 1.6, radius: 3 },
  left_foot: { token: "accent", label: "left foot", dash: null, opacity: 1, width: 2, radius: 3 },
  right_foot: { token: "warning", label: "right foot", dash: null, opacity: 1, width: 2, radius: 3 },
};
// How far apart a track's two drawn ends have to be before both fit where
// they belong, and how far the end mark steps aside when they do not.
const MARK_CLEAR = 3, MARK_OFFSET = 11, MARK_SIDE = 7;
// Where one panel's end marks may be drawn, given the ones already on it.
//
// A mark that lands on an indistinguishable neighbour is not a mark: it is
// the neighbour. Which marks hide which is a property of the shapes, not of
// the tracks — two root paths that differ everywhere but start and finish at
// one point put their squares on the same coordinate exactly as two
// identical paths do — so a lane is allocated from where a mark would be
// plotted rather than from what its track is. A filled square hides
// everything beneath it; a hollow ring hides only a ring of its own radius,
// so two tracks whose phase dots differ still read as concentric circles at
// a shared point. A mark that has to move steps toward the middle of the
// panel, where it cannot leave the picture, and carries a leader back to the
// coordinate it belongs to.
function markLanes(panel) {
  const placed = [];
  const stepped = (point, lane) => [
    point[0] + (point[0] > panel.width / 2 ? -MARK_OFFSET : MARK_OFFSET) * lane,
    point[1] + (point[1] > panel.height / 2 ? -MARK_OFFSET : MARK_OFFSET) * lane,
  ];
  return function place(point, mark) {
    let lane = 0, at = point;
    while (placed.some((prior) => mark.hides(prior) && Math.hypot(prior.at[0] - at[0], prior.at[1] - at[1]) < prior.clear)) {
      at = stepped(point, ++lane);
    }
    placed.push({ at, kind: mark.kind, size: mark.size, clear: mark.clear });
    return { at, lane };
  };
}
// The two ends of one drawn track: a hollow ring where it starts and a filled
// square where it ends, in the track's own colour so a panel of four trails
// says which end belongs to which. The square also carries the panel's own
// ground as a stroke, so it reads as a square wherever it lands on a line of
// its own colour.
function appendTrackEnds(svg, ends, style, place, palette, attribute, value) {
  const [first, last] = ends;
  const ring = style.radius + 3;
  const leader = (from, to, kind) => svg.append(svgElement("line", { x1: from[0], y1: from[1], x2: to[0], y2: to[1], stroke: style.color, "stroke-width": 1, opacity: style.opacity, [attribute]: `${value}-${kind}-leader` }));
  if (finitePoint(first)) {
    // A ring clears its own width plus a margin, so a square drawn inside it
    // later has to step out of it.
    const at = place(first, { kind: "start", size: ring, clear: ring + MARK_CLEAR, hides: (prior) => prior.kind === "start" && prior.size === ring });
    if (at.lane) leader(first, at.at, "start");
    svg.append(svgElement("circle", { cx: at.at[0], cy: at.at[1], r: ring, fill: "none", stroke: style.color, "stroke-width": 1.5, opacity: style.opacity, [attribute]: `${value}-start` }));
  }
  if (!finitePoint(last)) return;
  const at = place(last, { kind: "end", size: MARK_SIDE, clear: MARK_SIDE, hides: () => true });
  if (at.lane) leader(last, at.at, "end");
  svg.append(svgElement("rect", { x: at.at[0] - MARK_SIDE / 2, y: at.at[1] - MARK_SIDE / 2, width: MARK_SIDE, height: MARK_SIDE, fill: style.color, stroke: palette.ground, "stroke-width": 1, opacity: style.opacity, [attribute]: `${value}-end` }));
}
// The shared marks legend: one hollow ring and one filled square in the muted
// token, because the shapes mean the same thing on every track and each mark
// is already drawn in its own track's colour. Returns the x it ends at.
function appendMarksLegend(svg, palette, legendX, y) {
  svg.append(svgElement("circle", { cx: legendX + 4, cy: y - 4, r: 4, fill: "none", stroke: palette.muted, "stroke-width": 1.5 }));
  svg.append(svgElement("text", { x: legendX + 12, y, fill: palette.muted }, "start"));
  legendX += legendAdvance("start") + 8;
  svg.append(svgElement("rect", { x: legendX, y: y - 8, width: 7, height: 7, fill: palette.muted }));
  svg.append(svgElement("text", { x: legendX + 12, y, fill: palette.muted }, "end"));
  return legendX + legendAdvance("end");
}
function drawTrails(name, palette, phase) {
  const side = data[name], svg = q(`${name}-path`);
  if (!svg) return;
  svg.replaceChildren();
  if (!sharedTrailBounds) { svgMessage(`${name}-path`, svg, palette, "role trajectories unavailable"); return; }
  const map = topDownMap(sharedTrailBounds, TRAIL_PANEL.width, TRAIL_PANEL.height, TRAIL_PANEL.pad);
  const place = markLanes(TRAIL_PANEL);
  let legendX = 8, unavailable = [], incomplete = [];
  for (const role of Object.keys(TRAIL_TOKENS)) {
    const bone = side.clip.trails[role];
    if (bone == null) { unavailable.push(TRAIL_TOKENS[role].label); continue; }
    const points = trailPoints(side, bone), spec = TRAIL_TOKENS[role];
    const style = Object.assign({}, spec, { color: palette[spec.token] });
    const finite = points.filter(finitePoint).length;
    if (finite === 0) { unavailable.push(`${spec.label} (non-finite)`); continue; }
    if (finite !== points.length) incomplete.push(spec.label);
    const path = { d: pathData(points, map), fill: "none", stroke: style.color, "stroke-width": style.width, opacity: style.opacity, "data-role": role };
    if (spec.dash) path["stroke-dasharray"] = spec.dash;
    svg.append(svgElement("path", path));
    const frame = selectedFrames ? selectedFrames[name] : Math.round(phase * Math.max(0, points.length - 1));
    const selected = map(points[frame]);
    if (finitePoint(selected)) svg.append(svgElement("circle", { cx: selected[0], cy: selected[1], r: style.radius, fill: style.color, "data-role-dot": role }));
    const ends = trackEnds(points);
    if (ends) appendTrackEnds(svg, [map(ends[0]), map(ends[1])], style, place, palette, "data-role-marker", role);
    const legend = finite === points.length ? spec.label : `${spec.label} incomplete`;
    svg.append(svgElement("text", { x: legendX, y: 14, fill: style.color }, legend));
    legendX += legendAdvance(spec.label);
  }
  appendMarksLegend(svg, palette, legendX, 14);
  const missing = unavailable.length ? ` · unavailable: ${unavailable.join(", ")}` : "";
  const partial = incomplete.length ? ` · incomplete non-finite samples: ${incomplete.join(", ")}` : "";
  panelCaption(`${name}-path`, "what to look for: top-down trails of root, hips and feet over the whole clip; "
    + "matching trails mean the root, hips and feet moved the same way on both sides; "
    + "what the trails do not draw is not compared here"
    + " · each trail starts at the hollow circle and ends at the square; the root's trail is the path magnified in the panel above"
    + " · hips dashed and translucent over root, where the two run together"
    + ` · top-down X/Z metres · shared scale across both inputs${missing}${partial}`);
}

function drawGait(name, palette, phase) {
  const side = data[name], svg = q(`${name}-gait`), gait = side.contexts.gait;
  if (!svg) return;
  svg.replaceChildren();
  if (!gait) { svgMessage(`${name}-gait`, svg, palette, "gait unavailable: hips and bilateral foot/toe roles did not all resolve"); return; }
  const series = { left: [], right: [] };
  for (let frame = 0; frame < side.clip.frames; frame++) {
    const hipsY = posePoint(side, frame, gait.hips)[1];
    series.left.push(posePoint(side, frame, gait.left)[1] - hipsY);
    series.right.push(posePoint(side, frame, gait.right)[1] - hipsY);
  }
  const gaitFinite = series.left.every(Number.isFinite) && series.right.every(Number.isFinite);
  const yrange = finiteRange(series.left.concat(series.right));
  if (!yrange) { svgMessage(`${name}-gait`, svg, palette, "gait drawing unavailable: sampled relative heights are non-finite; stance and coverage evidence remain listed"); return; }
  const span = Math.max(.001, yrange[1] - yrange[0]);
  const x = (frame) => 20 + frame * 320 / Math.max(1, side.clip.frames - 1);
  const y = (value) => 150 - (value - yrange[0]) * 120 / span;
  // Left shading takes the upper half of the plot and right the lower. A
  // clean walk plants one foot while the other swings, but a defective clip
  // — or a repair that moved both feet the same way — can put the two
  // windows at the same frames, and two 16%-opacity bands drawn over each
  // other cancel into one grey block that belongs to neither side.
  for (const stance of side.contexts.stances) for (const run of stance.runs) {
    const selected = selectedContext && selectedContext.name === name && selectedContext.kind === "stance" && selectedContext.value.bone === stance.bone;
    const left = stance.side === "left";
    svg.append(svgElement("rect", { x: x(run.start_frame), y: left ? 24 : 89, width: Math.max(2, x(run.end_frame) - x(run.start_frame)), height: 65, fill: left ? palette.accent : palette.warning, opacity: selected ? .32 : .16, "data-stance-side": stance.side }));
  }
  for (const [label, color] of [["left", palette.accent], ["right", palette.warning]]) {
    const points = series[label].map((value, frame) => [x(frame), y(value)]);
    svg.append(svgElement("path", { d: pathData(points, (point) => point), fill: "none", stroke: color, "stroke-width": 2, "data-gait-side": label }));
  }
  const frame = selectedFrames ? selectedFrames[name] : Math.round(phase * Math.max(0, side.clip.frames - 1));
  svg.append(svgElement("line", { x1: x(frame), x2: x(frame), y1: 20, y2: 154, stroke: palette.error, "stroke-width": 1 }));
  const roleLabel = (role) => role.replace("_", " ");
  let legendX = 8;
  for (const [role, token] of [[gait.left_role, "accent"], [gait.right_role, "warning"]]) {
    const label = `${roleLabel(role)} height rel hips`;
    svg.append(svgElement("text", { x: legendX, y: 14, fill: palette[token] }, label));
    legendX += legendAdvance(label);
  }
  // The guidance sentence is derived on the Rust side from what this side's
  // checks declared and judged, so the same clip reads the same way in a
  // single-clip report and here.
  panelCaption(`${name}-gait`, gaitFinite
    ? `${side.guidance.gait}`
      + " · exact sampled height in metres · shaded runs are sampled foot-slide stance evidence"
      + " · left in the upper band, right in the lower"
    : "gait drawing incomplete: non-finite sampled heights; stance and coverage evidence remain listed");
}

let highlight = { before: null, after: null };
function update() {
  const palette = documentPalette;
  const phase = Number(q("scrub").value) / Math.max(1, sharedFrameMax);
  const beforeFrame = drawSide("before", palette, phase, highlight.before), afterFrame = drawSide("after", palette, phase, highlight.after);
  drawRootComparison(palette, phase); drawTrails("before", palette, phase); drawTrails("after", palette, phase); drawGait("before", palette, phase); drawGait("after", palette, phase);
  const time = (side, frame) => Number.isFinite(side.clip.times[frame]) ? side.clip.times[frame].toFixed(3) : "unavailable";
  q("times").textContent = `before ${time(data.before, beforeFrame)}s · after ${time(data.after, afterFrame)}s (normalized phase; not a time warp)`;
}
function summary(side) { return `primary ${side.identity.sha256} · ${side.identity.bytes} bytes · complete closure ${side.dependency_closure_identity.sha256} · clip ${side.clip.name}`; }
function subjectBone(row) {
  return Number.isInteger(row.subject_bone) && row.subject_bone >= 0 && row.subject_bone < data.bones.length ? row.subject_bone : null;
}
function nearestFrame(side, time) {
  return side.clip.times.reduce((best, value, index) => Math.abs(value - time) < Math.abs(side.clip.times[best] - time) ? index : best, 0);
}
function selectFinding(name, index) {
  const row = data[name].findings[index], side = data[name]; if (!row) return;
  pausePlayback();
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
  listContexts(name); list(name, "findings"); list(name, "gaps");
  q(`${name}-predictions`).textContent = JSON.stringify({ provenance: data[name].prediction_provenance, predictions: data[name].predictions }, null, 2);
}
function selectHash() {
  const match = location.hash.match(/^#(?:finding|time)-(before|after)-([a-f0-9]{16})$/);
  if (!match) return;
  const index = data[match[1]].findings.findIndex((row) => row.anchor === `finding-${match[2]}`);
  if (index >= 0) selectFinding(match[1], index);
}
// Fragment options this document can honour: embed and theme are document
// switches, and frame is the shared phase. Clip correspondence is declared
// by the two inputs, and a finding is addressed by its stable side anchor
// through selectHash, so neither is read from a key=value pair here.
function applyFragment() {
  const options = animsmithApplyDocument(animsmithFragmentOptions(location.hash));
  documentPalette = animsmithPalette();
  if (options.frame === undefined) return;
  // A frame past the end of the shared phase clamps to its last frame; one
  // the parser could not read restores frame 0. Either way the previous
  // selection does not stand.
  selectedFrames = null; selectedContext = null;
  pausePlayback();
  q("scrub").value = options.frame == null ? 0 : Math.min(options.frame, sharedFrameMax);
}

// ---- shared-phase playback ---------------------------------------------
// Both panes already draw whatever the shared phase says, so playing is
// nothing more than advancing that one number. It advances at the before
// clip's own duration — the phase is a display mapping, not a retime, so
// there is no combined timeline to run at — and wraps at the end the way a
// looping clip does. Nothing here touches the fragment: the `#frame=` deep
// link stays a way in, never a thing the document writes back.
const playButton = q("play");
// Frames of shared phase per second. A clip with no positive duration, a
// single judged frame, or no embedded poses has nothing to advance through,
// and the button stays disabled rather than becoming a control that does
// nothing when pressed.
const playRate = (() => {
  const duration = data.before.clip.duration;
  if (posesOmitted || !(duration > 0) || !Number.isFinite(duration) || sharedFrameMax < 1) return 0;
  return sharedFrameMax / duration;
})();
if (!playRate) playButton.disabled = true;
let playFrame = 0, playedAt = 0;
const playLoop = animsmithFrameLoop((now) => {
  const elapsed = Number.isFinite(now - playedAt) ? Math.max(0, (now - playedAt) / 1000) : 0;
  playedAt = now;
  playFrame += elapsed * playRate;
  if (playFrame > sharedFrameMax) playFrame = 0;
  selectedFrames = null; selectedContext = null;
  q("scrub").value = Math.round(playFrame);
  update();
});
function pausePlayback() {
  if (!playLoop.stop()) return;
  playButton.textContent = "▶";
  playButton.setAttribute("aria-label", "Play the shared phase");
}
playButton.addEventListener("click", () => {
  if (!playRate) return;
  if (playLoop.running) { pausePlayback(); return; }
  // Resume from wherever the reader left the phase, not from where playback
  // last stopped: a scrub between two presses is the position they chose.
  playFrame = Number(q("scrub").value) || 0;
  playedAt = performance.now();
  playLoop.start();
  playButton.textContent = "⏸";
  playButton.setAttribute("aria-label", "Pause the shared phase");
});
q("scrub").addEventListener("input", () => { pausePlayback(); selectedFrames = null; selectedContext = null; update(); });
window.addEventListener("resize", update);
animsmithOnSchemeChange(() => { documentPalette = animsmithPalette(); update(); });
window.addEventListener("hashchange", () => { applyFragment(); selectHash(); update(); });
applyFragment(); selectHash(); update();
