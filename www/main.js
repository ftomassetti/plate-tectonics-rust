// Browser demo for the Rust/WASM port of plate-tectonics.
//
// The simulation is stepped one iteration at a time and re-rendered after each
// step, so the world can be watched as it forms.

import init, { Simulation } from './pkg/platec_wasm.js';

const $ = (id) => document.getElementById(id);

const els = {
  seed: $('seed'), width: $('width'), height: $('height'), seaLevel: $('seaLevel'),
  numPlates: $('numPlates'), erosionPeriod: $('erosionPeriod'), foldingRatio: $('foldingRatio'),
  aggrAbs: $('aggrAbs'), aggrRel: $('aggrRel'), cycleCount: $('cycleCount'),
  create: $('create'), randomSeed: $('randomSeed'),
  run: $('run'), step: $('step'), step10: $('step10'), stepsPerFrame: $('stepsPerFrame'),
  statIter: $('statIter'), statCycle: $('statCycle'), statPlates: $('statPlates'), statMs: $('statMs'),
  mode: $('mode'), overlay: $('overlay'), arrows: $('arrows'),
  canvas: $('canvas'), status: $('status'),
};

const ctx = els.canvas.getContext('2d');

let wasm = null;      // the wasm module exports (for `memory`)
let sim = null;       // the live Simulation
let running = false;
let imageData = null;
let lastStepMs = 0;

// --- Rendering ------------------------------------------------------------

// The colour ramp of the C++ `examples/map_drawing.cpp`, keyed off quantiles of
// the (normalised) height map.
const TERRAIN_STOPS = [
  { q: 0.15, from: [0, 0, 255],     to: [0, 20, 200] },
  { q: 0.70, from: [0, 20, 200],    to: [50, 80, 225] },
  { q: 0.75, from: [50, 80, 225],   to: [135, 237, 235] },
  { q: 0.90, from: [88, 173, 49],   to: [218, 226, 58] },
  { q: 0.95, from: [218, 226, 58],  to: [251, 252, 42] },
  { q: 0.99, from: [251, 252, 42],  to: [91, 28, 13] },
  { q: 1.00, from: [91, 28, 13],    to: [51, 0, 4] },
];

// A stable palette for plate indices.
const PLATE_COLORS = [];
for (let i = 0; i < 64; i++) {
  const h = (i * 137.508) % 360;              // golden-angle hue spacing
  const s = 0.55 + 0.2 * ((i >> 2) % 2);
  const l = 0.45 + 0.12 * (i % 3);
  PLATE_COLORS.push(hslToRgb(h / 360, s, l));
}

function hslToRgb(h, s, l) {
  const f = (n) => {
    const k = (n + h * 12) % 12;
    const a = s * Math.min(l, 1 - l);
    return Math.round(255 * (l - a * Math.max(-1, Math.min(k - 3, Math.min(9 - k, 1)))));
  };
  return [f(0), f(8), f(4)];
}

/// Quantile thresholds via a 2048-bin histogram — a single extra pass, instead
/// of the binary search the C++ uses per quantile.
function quantileThresholds(values, min, max, qs) {
  const BINS = 2048;
  const hist = new Uint32Array(BINS);
  const span = max - min;
  const scale = span > 0 ? (BINS - 1) / span : 0;
  for (let i = 0; i < values.length; i++) {
    hist[((values[i] - min) * scale) | 0]++;
  }
  const out = new Float32Array(qs.length);
  let cum = 0, qi = 0;
  for (let b = 0; b < BINS && qi < qs.length; b++) {
    cum += hist[b];
    const frac = cum / values.length;
    while (qi < qs.length && frac >= qs[qi]) {
      out[qi++] = min + (b / (BINS - 1)) * span;
    }
  }
  while (qi < qs.length) out[qi++] = max;
  return out;
}

function lerpColor(px, o, from, to, t) {
  const a = 1 - t;
  px[o]     = a * from[0] + t * to[0];
  px[o + 1] = a * from[1] + t * to[1];
  px[o + 2] = a * from[2] + t * to[2];
  px[o + 3] = 255;
}

function renderTerrain(px, h, min, max) {
  const qs = quantileThresholds(h, min, max, TERRAIN_STOPS.map((s) => s.q));
  const span = max - min || 1;
  for (let i = 0; i < h.length; i++) {
    const v = h[i];
    let lo = min, s = TERRAIN_STOPS.length - 1;
    for (let k = 0; k < TERRAIN_STOPS.length; k++) {
      if (v < qs[k] || k === TERRAIN_STOPS.length - 1) { s = k; lo = k === 0 ? min : qs[k - 1]; break; }
    }
    const hi = qs[s];
    const d = hi - lo;
    const t = d > 0 ? Math.min(1, Math.max(0, (v - lo) / d)) : 0;
    lerpColor(px, i * 4, TERRAIN_STOPS[s].from, TERRAIN_STOPS[s].to, t);
    // `span` keeps the value used even for degenerate maps.
    if (d <= 0 && span === 0) px[i * 4 + 3] = 255;
  }
}

function renderGray(px, h, min, max) {
  const span = max - min || 1;
  for (let i = 0; i < h.length; i++) {
    const g = Math.round(255 * ((h[i] - min) / span));
    px[i * 4] = px[i * 4 + 1] = px[i * 4 + 2] = g;
    px[i * 4 + 3] = 255;
  }
}

function renderPlates(px, imap, plateCount) {
  for (let i = 0; i < imap.length; i++) {
    const id = imap[i];
    const c = id < plateCount ? PLATE_COLORS[id % PLATE_COLORS.length] : [24, 24, 24];
    px[i * 4] = c[0]; px[i * 4 + 1] = c[1]; px[i * 4 + 2] = c[2]; px[i * 4 + 3] = 255;
  }
}

function renderAge(px, amap, iterCount) {
  let maxAge = 1;
  for (let i = 0; i < amap.length; i++) if (amap[i] <= iterCount && amap[i] > maxAge) maxAge = amap[i];
  for (let i = 0; i < amap.length; i++) {
    const t = Math.min(1, Math.max(0, amap[i] / maxAge));
    // Young crust hot (red), old crust cool (blue).
    px[i * 4] = Math.round(255 * (1 - t) + 30 * t);
    px[i * 4 + 1] = Math.round(60 * (1 - t) + 90 * t);
    px[i * 4 + 2] = Math.round(30 * (1 - t) + 200 * t);
    px[i * 4 + 3] = 255;
  }
}

/// Darken pixels that sit on a plate boundary.
function drawBoundaries(px, imap, w, h) {
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const i = y * w + x;
      const id = imap[i];
      const right = imap[y * w + ((x + 1) % w)];
      const down = imap[((y + 1) % h) * w + x];
      if (id !== right || id !== down) {
        px[i * 4] = px[i * 4] * 0.25;
        px[i * 4 + 1] = px[i * 4 + 1] * 0.25;
        px[i * 4 + 2] = px[i * 4 + 2] * 0.25;
      }
    }
  }
}

/// Per-plate centroids of the index map, used to anchor velocity arrows.
function plateCentroids(imap, w, h, plateCount) {
  const sx = new Float64Array(plateCount);
  const sy = new Float64Array(plateCount);
  const n = new Float64Array(plateCount);
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const id = imap[y * w + x];
      if (id < plateCount) { sx[id] += x; sy[id] += y; n[id]++; }
    }
  }
  const out = [];
  for (let i = 0; i < plateCount; i++) {
    out.push(n[i] > 0 ? { x: sx[i] / n[i], y: sy[i] / n[i] } : null);
  }
  return out;
}

function drawArrows(imap, w, h, plateCount) {
  const centroids = plateCentroids(imap, w, h, plateCount);
  const len = Math.max(12, Math.min(w, h) * 0.06);
  ctx.save();
  ctx.lineWidth = Math.max(1.5, Math.min(w, h) / 250);
  ctx.strokeStyle = 'rgba(255,255,255,0.9)';
  ctx.fillStyle = 'rgba(255,255,255,0.9)';
  for (let i = 0; i < plateCount; i++) {
    const c = centroids[i];
    if (!c) continue;
    const vx = sim.plateVelocityX(i);
    const vy = sim.plateVelocityY(i);
    const m = Math.hypot(vx, vy) || 1;
    const dx = (vx / m) * len;
    const dy = (vy / m) * len;
    ctx.beginPath();
    ctx.moveTo(c.x, c.y);
    ctx.lineTo(c.x + dx, c.y + dy);
    ctx.stroke();
    // Arrow head.
    const ang = Math.atan2(dy, dx);
    const head = len * 0.35;
    ctx.beginPath();
    ctx.moveTo(c.x + dx, c.y + dy);
    ctx.lineTo(c.x + dx - head * Math.cos(ang - 0.4), c.y + dy - head * Math.sin(ang - 0.4));
    ctx.lineTo(c.x + dx - head * Math.cos(ang + 0.4), c.y + dy - head * Math.sin(ang + 0.4));
    ctx.closePath();
    ctx.fill();
  }
  ctx.restore();
}

function render() {
  if (!sim) return;
  const w = sim.width();
  const h = sim.height();
  const n = w * h;

  // These views alias wasm linear memory, which `step()` can grow — so they are
  // rebuilt from scratch on every render rather than cached.
  const heights = new Float32Array(wasm.memory.buffer, sim.heightmapPtr(), n);
  const imap = new Uint32Array(wasm.memory.buffer, sim.platesmapPtr(), n);

  const px = imageData.data;
  const mode = els.mode.value;

  if (mode === 'plates') {
    renderPlates(px, imap, sim.plateCount());
  } else if (mode === 'age') {
    const amap = new Uint32Array(wasm.memory.buffer, sim.agemapPtr(), n);
    renderAge(px, amap, sim.iterationCount());
  } else {
    let min = Infinity, max = -Infinity;
    for (let i = 0; i < n; i++) {
      const v = heights[i];
      if (v < min) min = v;
      if (v > max) max = v;
    }
    if (mode === 'gray') renderGray(px, heights, min, max);
    else renderTerrain(px, heights, min, max);
  }

  if (els.overlay.checked) drawBoundaries(px, imap, w, h);

  ctx.putImageData(imageData, 0, 0);
  if (els.arrows.checked) drawArrows(imap, w, h, sim.plateCount());
}

// --- Simulation control ---------------------------------------------------

function updateStats() {
  if (!sim) return;
  els.statIter.textContent = sim.iterationCount();
  els.statCycle.textContent = sim.cycleCount();
  els.statPlates.textContent = sim.plateCount();
  els.statMs.textContent = lastStepMs ? `${lastStepMs.toFixed(1)} ms` : '–';
}

function setStatus(text, isError = false) {
  els.status.textContent = text;
  els.status.classList.toggle('error', isError);
}

function doSteps(count) {
  if (!sim || sim.isFinished()) return;
  const t0 = performance.now();
  let done = 0;
  for (let i = 0; i < count && !sim.isFinished(); i++) {
    sim.step();
    done++;
  }
  lastStepMs = done ? (performance.now() - t0) / done : lastStepMs;
  render();
  updateStats();
  if (sim.isFinished()) {
    running = false;
    els.run.textContent = 'Run';
    setStatus(`Simulation finished after ${sim.iterationCount()} iterations (${sim.cycleCount()} cycles).`);
    els.run.disabled = true;
    els.step.disabled = true;
    els.step10.disabled = true;
  }
}

function frame() {
  if (!running) return;
  doSteps(Math.max(1, Number(els.stepsPerFrame.value) || 1));
  if (running) requestAnimationFrame(frame);
}

function createSimulation() {
  running = false;
  els.run.textContent = 'Run';
  const w = Math.max(5, Number(els.width.value) | 0);
  const h = Math.max(5, Number(els.height.value) | 0);

  try {
    sim = new Simulation(
      Number(els.seed.value) >>> 0,
      w, h,
      Number(els.seaLevel.value),
      Number(els.erosionPeriod.value) | 0,
      Number(els.foldingRatio.value),
      Number(els.aggrAbs.value) | 0,
      Number(els.aggrRel.value),
      Number(els.cycleCount.value) | 0,
      Number(els.numPlates.value) | 0,
    );
  } catch (e) {
    setStatus(`Could not create the simulation: ${e.message ?? e}`, true);
    return;
  }

  els.canvas.width = w;
  els.canvas.height = h;
  imageData = ctx.createImageData(w, h);

  els.run.disabled = false;
  els.step.disabled = false;
  els.step10.disabled = false;
  lastStepMs = 0;

  render();
  updateStats();
  setStatus(`World generated (${w}×${h}, ${sim.plateCount()} plates). Press Run to watch the plates move.`);
}

// --- Wiring ---------------------------------------------------------------

els.create.addEventListener('click', createSimulation);
els.randomSeed.addEventListener('click', () => {
  els.seed.value = Math.floor(Math.random() * 2 ** 31);
});
els.run.addEventListener('click', () => {
  running = !running;
  els.run.textContent = running ? 'Pause' : 'Run';
  if (running) {
    setStatus('Running…');
    requestAnimationFrame(frame);
  } else {
    setStatus('Paused.');
  }
});
els.step.addEventListener('click', () => doSteps(1));
els.step10.addEventListener('click', () => doSteps(10));
els.mode.addEventListener('change', render);
els.overlay.addEventListener('change', render);
els.arrows.addEventListener('change', render);

init().then((module) => {
  wasm = module;
  setStatus('Ready. Adjust the parameters and press "Generate world".');
  createSimulation();
}).catch((e) => {
  setStatus(`Failed to load the WebAssembly module: ${e.message ?? e}`, true);
});
