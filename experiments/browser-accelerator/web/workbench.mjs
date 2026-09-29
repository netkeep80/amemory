import {
  closeScenarioLiveSession,
  openScenarioLiveSession,
  readScenarioTransportLimits,
  refreshScenarioLiveHistoryStatus,
  runScenarioLiveSession,
  setScenarioLiveRetentionPolicy,
} from "./scenario-transport.mjs";
import {
  loadScenarioPresetManifestByIndex,
  refreshScenarioPresetRegistry,
} from "./scenario-presets.mjs";

const BACKENDS = [
  ["optimized-cpu", "CPU / optimized"],
  ["webgpu", "WebGPU"],
  ["linksdb", "LinksDB"],
];

const clone = (value) => JSON.parse(JSON.stringify(value));
const sameInputs = (a, b) => JSON.stringify(a || {}) === JSON.stringify(b || {});

function parseUnsigned(raw, max, label) {
  const text = String(raw).trim();
  const value = /^0x[0-9a-f]+$/i.test(text)
    ? Number.parseInt(text.slice(2), 16)
    : Number.parseInt(text, 10);
  if (!Number.isSafeInteger(value) || value < 0 || value > max) {
    throw new Error(label + " must be 0.." + max);
  }
  return value;
}

export function normalizeWorkbenchInputs(schema, raw) {
  const out = {};
  for (const field of schema || []) {
    const type = String(field.type || "").toUpperCase();
    const value = raw ? raw[field.key] : undefined;
    if (type === "BIT") {
      const bit = Number(value);
      if (bit !== 0 && bit !== 1) throw new Error(field.key + " must be 0 or 1");
      out[field.key] = bit;
    } else if (type === "COUNT8" || type === "U8") {
      out[field.key] = parseUnsigned(value, 0xff, field.key);
    } else if (type === "WORD32" || type === "U32" || type === "UINT32") {
      out[field.key] = parseUnsigned(value, 0xffff_ffff, field.key) >>> 0;
    } else if (/^-?\d+$/.test(String(value).trim())) {
      out[field.key] = Number.parseInt(String(value).trim(), 10);
    } else {
      out[field.key] = value;
    }
  }
  return out;
}

export function createWorkbenchRun(manifest, presetIndex, rawInputs, ordinal, mode = "preset") {
  const runs = Array.isArray(manifest && manifest.runSequence) ? manifest.runSequence : [];
  const base = clone(runs[presetIndex] || runs[0] || {
    runId: "workbench-template",
    executionMode: "TO_QUIESCENCE",
    assertions: [],
  });
  const inputs = normalizeWorkbenchInputs(
    (manifest && manifest.inputSchema) || [],
    rawInputs || base.inputs || (manifest && manifest.initialInputs) || {},
  );
  base.runId = "workbench-live-" + ordinal;
  base.inputs = inputs;
  if (mode === "manual") {
    const match = runs.find((run) => sameInputs(run.inputs, inputs));
    base.assertions = match ? clone(match.assertions || []) : [];
  }
  return base;
}

export function deriveWorkbenchPipeline(status, run) {
  const open = Boolean(status);
  const stages = run && run.pipelineProfile && run.pipelineProfile.stages;
  const done = Boolean(run && stages);
  return [
    ["PREPARE", open && status.prepareCount === 1, open ? "prepareCount=" + status.prepareCount : "Session not open"],
    ["LOAD", open && status.loadCount === 1, open ? "base Links=" + status.baseLinkCount : "Session not open"],
    ["CONFIGURE", done, done ? "Links " + run.linksBeforeConfigure + " → " + run.linksAfterConfigure : "Run not executed"],
    ["EXECUTE", done, done ? "reactions=" + (run.observed && run.observed.activeReactionCount) : "Run not executed"],
    ["RESULT", done, done ? "runtime result projected" : "Run not executed"],
    ["EVIDENCE", done, done ? "events=" + ((run.observed && run.observed.events || []).length) : "Run not executed"],
  ].map(([id, complete, detail]) => ({ id, state: complete ? "done" : "waiting", detail }));
}

const esc = (value) => String(value == null ? "" : value)
  .replaceAll("&", "&amp;").replaceAll("<", "&lt;")
  .replaceAll(">", "&gt;").replaceAll('"', "&quot;")
  .replaceAll("'", "&#39;");
const json = (value) => esc(JSON.stringify(value, null, 2));
const compact = (value) => {
  const text = String(value == null ? "—" : value);
  return text.length <= 22 ? text : text.slice(0, 10) + "…" + text.slice(-9);
};

function timing(stages, key) {
  if (!stages || stages.timingAvailable === false) return "timing unavailable";
  return Number.isFinite(stages[key]) ? stages[key] + " ns" : "n/a";
}

function resultText(run) {
  if (!run) return "No result yet";
  const result = run.result;
  if (result == null || typeof result !== "object") return String(result);
  if ("valueHi" in result && "value" in result) return "hi=" + result.valueHi + " · lo=" + result.value;
  return "value" in result ? String(result.value) : JSON.stringify(result);
}

function assertionText(run) {
  const items = Array.isArray(run && run.assertionResults) ? run.assertionResults : [];
  if (!items.length) return "no preset assertion for this manual input";
  return items.filter((item) => item.passed === true).length + "/" + items.length + " assertions passed";
}

function styles() {
  if (document.querySelector("#amemory-workbench-style")) return;
  const style = document.createElement("style");
  style.id = "amemory-workbench-style";
  style.textContent = [
    ".wb{margin:18px 0 30px;display:grid;gap:12px}",
    ".wb-card{background:var(--surface);border:1px solid var(--line);border-radius:16px;box-shadow:var(--shadow);min-width:0}",
    ".wb-top{padding:14px 16px;display:flex;gap:8px;flex-wrap:wrap;align-items:center}.wb-top strong{margin-right:auto}",
    ".wb-chip{padding:5px 8px;border:1px solid var(--line);border-radius:999px;background:var(--surface-2);font-size:.76rem}.wb-chip.good{border-color:var(--good);color:var(--good)}.wb-chip.bad{border-color:var(--bad);color:var(--bad)}",
    ".wb-pipe{display:grid;grid-template-columns:repeat(6,minmax(0,1fr));gap:8px}.wb-stage{padding:10px;border:1px solid var(--line);border-radius:12px;background:var(--surface)}.wb-stage.done{border-color:var(--good)}.wb-stage strong{display:block;font-size:.76rem}.wb-stage small{display:block;color:var(--muted);margin-top:4px;overflow-wrap:anywhere}",
    ".wb-grid{display:grid;grid-template-columns:minmax(300px,360px) minmax(0,1fr);gap:12px;align-items:start}.wb-controls,.wb-main{padding:16px}.wb-controls h3,.wb-main h3{margin:0 0 12px}",
    ".wb-field{display:grid;gap:5px;margin:10px 0}.wb-field label{color:var(--muted);font-size:.78rem;font-weight:700}.wb-field input,.wb-field select,.wb-actions button,.wb-mode button{min-height:38px;border:1px solid var(--line);border-radius:9px;background:var(--surface-2);color:var(--text);padding:7px 9px}.wb-field input,.wb-field select{width:100%}",
    ".wb-mode,.wb-actions{display:flex;gap:7px;flex-wrap:wrap}.wb-mode button,.wb-actions button{cursor:pointer}.wb-mode button[aria-pressed=true]{outline:2px solid var(--accent);font-weight:800}.wb-actions .primary{background:var(--accent);color:#fff;border-color:var(--accent);font-weight:800}.wb-actions button:disabled{opacity:.45}",
    ".wb-help{color:var(--muted);font-size:.78rem;margin-top:6px}.wb-memory{display:grid;grid-template-columns:repeat(4,minmax(0,1fr));gap:8px}.wb-metric{padding:10px;border:1px solid var(--line);border-radius:10px;background:var(--surface-2);min-width:0}.wb-metric small{display:block;color:var(--muted)}.wb-metric strong,.wb-metric code{display:block;margin-top:4px;overflow-wrap:anywhere}",
    ".wb-result{margin-top:12px;padding:14px;border:2px solid var(--line);border-radius:12px}.wb-result.ready{border-color:var(--good)}.wb-result h4{margin:0 0 7px}.wb-result-value{font-size:1.35rem;font-weight:900;overflow-wrap:anywhere}",
    ".wb-tabs{padding:0 16px 16px}.wb-tabbar{display:flex;gap:6px;flex-wrap:wrap;border-bottom:1px solid var(--line);padding-bottom:9px}.wb-tabbar button{border:0;background:transparent;color:var(--muted);padding:7px 9px;cursor:pointer}.wb-tabbar button[aria-selected=true]{color:var(--text);font-weight:800;border-bottom:2px solid var(--accent)}.wb-tab{padding-top:12px}",
    ".wb-log{display:grid;gap:6px;max-height:360px;overflow:auto}.wb-log-row{padding:8px 10px;border:1px solid var(--line);border-radius:9px;background:var(--surface-2);font-size:.78rem}.wb-table{width:100%;border-collapse:collapse;font-size:.8rem}.wb-table th,.wb-table td{text-align:left;padding:7px 8px;border-bottom:1px solid var(--line)}.wb-table th{color:var(--muted)}",
    ".wb-raw{margin-top:10px;border:1px solid var(--line);border-radius:10px;overflow:hidden}.wb-raw summary{cursor:pointer;padding:9px 11px;font-weight:700}.wb-raw pre{border-radius:0;max-height:360px;font-size:.75rem}.wb-error{color:var(--bad);font-weight:800}",
    "@media(max-width:1000px){.wb-grid{grid-template-columns:1fr}.wb-memory{grid-template-columns:1fr 1fr}.wb-pipe{grid-template-columns:repeat(3,1fr)}}@media(max-width:620px){.wb-memory,.wb-pipe{grid-template-columns:1fr}}",
  ].join("\n");
  document.head.append(style);
}

async function wasm() {
  const response = await fetch("./amemory_a_circuit.wasm", { cache: "no-store" });
  if (!response.ok) throw new Error("A-Circuit WASM HTTP " + response.status);
  const bytes = await response.arrayBuffer();
  const loaded = await WebAssembly.instantiate(bytes, {});
  const w = loaded.instance.exports;
  for (const name of [
    "amemory_scenario_preset_registry_refresh",
    "amemory_scenario_live_open_json",
    "amemory_scenario_live_execute_json",
    "amemory_scenario_live_close",
  ]) {
    if (typeof w[name] !== "function") throw new Error("Workbench WASM ABI missing " + name);
  }
  return w;
}

async function buildInfo() {
  try {
    const response = await fetch("./build-info.json", { cache: "no-store" });
    const info = response.ok ? await response.json() : {};
    return { version: info.version || "unknown", sha: info.mainSha || "unknown" };
  } catch {
    return { version: "unknown", sha: "unknown" };
  }
}

function presetInputs(state) {
  const run = state.manifest.runSequence[state.presetIndex];
  state.inputs = clone((run && run.inputs) || state.manifest.initialInputs || {});
}

function loadManifest(state, index) {
  if (state.session) closeScenarioLiveSession(state.wasm);
  const source = loadScenarioPresetManifestByIndex(state.wasm, index);
  if (source == null) throw new Error("Scenario manifest missing at index " + index);
  state.scenarioIndex = index;
  state.manifest = JSON.parse(source);
  state.presetIndex = 0;
  state.mode = "preset";
  state.backend = (state.manifest.supportedBackends || [])[0] || "optimized-cpu";
  state.session = null;
  state.run = null;
  state.history = null;
  state.error = null;
  presetInputs(state);
}

async function history(state) {
  const refreshed = refreshScenarioLiveHistoryStatus(state.wasm);
  state.history = refreshed.ok ? refreshed.status : null;
}

async function open(state) {
  state.loading = true; state.error = null; render(state);
  try {
    const opened = openScenarioLiveSession(state.wasm, state.manifest, state.backend);
    if (!opened.ok) throw new Error(opened.error && opened.error.message || JSON.stringify(opened.error));
    state.session = opened.status;
    const cap = readScenarioTransportLimits(state.wasm).liveObserver;
    const policy = setScenarioLiveRetentionPolicy(state.wasm, {
      retentionMode: "RING",
      maxRetainedRuns: Math.min(8, cap.maxRetainedRuns),
      maxRetainedEvents: Math.min(8192, cap.maxRetainedEvents),
      maxRetainedBytes: Math.min(8 * 1024 * 1024, cap.maxRetainedBytes),
    });
    if (!policy.ok) throw new Error(policy.error && policy.error.message || JSON.stringify(policy.error));
    await history(state);
  } catch (error) {
    state.session = null; state.run = null; state.history = null;
    state.error = error && error.message || String(error);
  }
  state.loading = false; render(state);
}

async function execute(state) {
  state.loading = true; state.error = null; render(state);
  try {
    const request = createWorkbenchRun(
      state.manifest, state.presetIndex, state.inputs,
      (state.session.completedRuns || 0) + 1, state.mode,
    );
    const result = runScenarioLiveSession(state.wasm, request);
    if (!result.ok) throw new Error(result.error && result.error.message || JSON.stringify(result.error));
    state.session = result.payload.status;
    state.run = result.payload.run;
    await history(state);
  } catch (error) {
    state.error = error && error.message || String(error);
  }
  state.loading = false; render(state);
}

function close(state) {
  if (state.session) closeScenarioLiveSession(state.wasm);
  state.session = null; state.run = null; state.history = null; state.error = null;
  render(state);
}

function tab(state) {
  const run = state.run;
  if (state.tab === "log") {
    const events = run && run.observed && Array.isArray(run.observed.events) ? run.observed.events : [];
    return events.length
      ? '<div class="wb-log">' + events.map((event, i) =>
          '<div class="wb-log-row"><strong>#' + i + " " + esc(event.kind || "EVENT") +
          '</strong><br><code>' + esc(JSON.stringify(event)) + '</code></div>').join("") + '</div>'
      : '<div class="wb-help">No runtime events yet.</div>';
  }
  if (state.tab === "profile") {
    const stages = run && run.pipelineProfile && run.pipelineProfile.stages;
    const structural = run && run.observed && run.observed.profile && run.observed.profile.structural;
    if (!stages) return '<div class="wb-help">Run once to populate the real profile.</div>';
    return '<table class="wb-table"><tbody>' +
      '<tr><th>CONFIGURE</th><td>' + timing(stages, "configureNs") + '</td></tr>' +
      '<tr><th>EXECUTE</th><td>' + timing(stages, "executeNs") + '</td></tr>' +
      '<tr><th>RESULT</th><td>' + timing(stages, "resultNs") + '</td></tr>' +
      '<tr><th>EVIDENCE</th><td>' + timing(stages, "evidenceNs") + '</td></tr>' +
      '<tr><th>trigger candidates</th><td>' + esc(structural && structural.triggerIncidenceCandidates || "n/a") + '</td></tr>' +
      '<tr><th>unification attempts</th><td>' + esc(structural && structural.unificationAttempts || "n/a") + '</td></tr>' +
      '<tr><th>published outputs</th><td>' + esc(structural && structural.publicationOutputs || "n/a") + '</td></tr></tbody></table>';
  }
  if (state.tab === "history") {
    const h = state.history;
    return h
      ? '<div class="wb-help">Observer-only ' + esc(h.retentionMode) +
        " · runs=" + h.retainedRuns + " · events=" + h.retainedEvents +
        " · bytes=" + h.retainedBytes + " · profile points=" + h.profilePoints + '</div>' +
        '<details class="wb-raw"><summary>Profile trend</summary><pre>' + json(h.profileTrend || []) + '</pre></details>'
      : '<div class="wb-help">Observer history appears after Session open.</div>';
  }
  return '<div class="wb-help">Proof/raw structures are collapsed by default.</div>' +
    '<details class="wb-raw"><summary>Current run report</summary><pre>' + json(run || {}) + '</pre></details>' +
    '<details class="wb-raw"><summary>Scenario manifest</summary><pre>' + json(state.manifest || {}) + '</pre></details>';
}

function syncInputs(state) {
  for (const node of state.root.querySelectorAll("[data-input-key]")) {
    state.inputs[node.dataset.inputKey] = node.value;
  }
}

function render(state) {
  const status = state.session;
  const run = state.run;
  const pipeline = deriveWorkbenchPipeline(status, run);
  const openSession = Boolean(status);
  const currentLinks = status
    ? (status.currentLinkCount == null ? status.baseLinkCount : status.currentLinkCount)
    : "—";

  state.root.innerHTML =
    '<div class="wb">' +
    '<section class="wb-card wb-top"><strong>A-memory Workbench</strong>' +
    '<span class="wb-chip">v' + esc(state.build.version) + '</span>' +
    '<span class="wb-chip" title="' + esc(state.build.sha) + '">SHA ' + esc(compact(state.build.sha)) + '</span>' +
    '<span class="wb-chip">backend ' + esc(state.backend) + '</span>' +
    '<span class="wb-chip">session ' + esc(compact(status && status.sessionId)) + '</span>' +
    '<span class="wb-chip">run ' + esc(run && run.sessionRunId || 0) + '</span>' +
    '<span class="wb-chip ' + (state.error ? "bad" : openSession ? "good" : "") + '">' +
    esc(state.error ? "ERROR" : openSession ? "SESSION OPEN" : state.loading ? "LOADING" : "CLOSED") +
    '</span></section>' +

    '<section class="wb-pipe">' + pipeline.map((stage) =>
      '<div class="wb-stage ' + stage.state + '"><strong>' + esc(stage.id) +
      '</strong><small>' + esc(stage.detail) + '</small></div>').join("") + '</section>' +

    '<section class="wb-grid"><div class="wb-card wb-controls"><h3>Scenario / constructor</h3>' +
    '<div class="wb-field"><label>Scenario preset</label><select id="wb-scenario"' +
    (openSession ? " disabled" : "") + '>' +
    (state.registry.entries || []).map((entry, i) =>
      '<option value="' + i + '"' + (i === state.scenarioIndex ? " selected" : "") + '>' +
      esc((entry.title || entry.scenarioId || "Scenario") +
      (entry.scenarioVersion ? " @" + entry.scenarioVersion : "")) + '</option>').join("") + '</select></div>' +

    '<div class="wb-mode"><button data-mode="preset" aria-pressed="' + (state.mode === "preset") +
    '">Preset</button><button data-mode="manual" aria-pressed="' + (state.mode === "manual") +
    '">Manual</button></div>' +

    '<div class="wb-field"><label>Preset run</label><select id="wb-preset-run">' +
    (state.manifest.runSequence || []).map((item, i) =>
      '<option value="' + i + '"' + (i === state.presetIndex ? " selected" : "") + '>' +
      esc(item.runId || "run-" + (i + 1)) + '</option>').join("") + '</select></div>' +

    (state.manifest.inputSchema || []).map((field) => {
      const value = state.inputs[field.key];
      const disabled = state.mode === "preset" ? " disabled" : "";
      const control = String(field.type || "").toUpperCase() === "BIT"
        ? '<select data-input-key="' + esc(field.key) + '"' + disabled + '><option value="0"' +
          (Number(value) === 0 ? " selected" : "") + '>0</option><option value="1"' +
          (Number(value) === 1 ? " selected" : "") + '>1</option></select>'
        : '<input data-input-key="' + esc(field.key) + '" value="' + esc(value) + '"' + disabled + '>';
      return '<div class="wb-field"><label>' + esc(field.key) + " · " + esc(field.type) +
        '</label>' + control + '<div class="wb-help">' + esc(field.description || "") + '</div></div>';
    }).join("") +

    '<div class="wb-field"><label>Backend</label><select id="wb-backend"' + (openSession ? " disabled" : "") + '>' +
    BACKENDS.map(([id, label]) => {
      const supported = (state.manifest.supportedBackends || []).includes(id);
      return '<option value="' + id + '"' + (id === state.backend ? " selected" : "") +
        (supported ? "" : " disabled") + '>' + esc(label + (supported ? "" : " — unsupported")) + '</option>';
    }).join("") + '</select><div class="wb-help">Unsupported backends stay explicit; there is no silent fallback.</div></div>' +

    '<div class="wb-actions"><button id="wb-open"' + (openSession || state.loading ? " disabled" : "") +
    '>Open A-memory</button><button id="wb-run" class="primary"' + (!openSession || state.loading ? " disabled" : "") +
    '>Run</button><button id="wb-close"' + (!openSession || state.loading ? " disabled" : "") + '>Close</button></div>' +
    '<div class="wb-help">Preset and Manual use the same Scenario manifest. Editing inputs keeps this Session and loaded A-memory.</div>' +
    (state.error ? '<p class="wb-error">' + esc(state.error) + '</p>' : '') + '</div>' +

    '<div class="wb-card wb-main"><h3>One persistent A-memory</h3><div class="wb-memory">' +
    '<div class="wb-metric"><small>Session</small><code>' + esc(status && status.sessionId || "closed") + '</code></div>' +
    '<div class="wb-metric"><small>Store / engine</small><code>' + esc(compact(status && status.storeInstanceId)) +
    " / " + esc(compact(status && status.engineInstanceId)) + '</code></div>' +
    '<div class="wb-metric"><small>Links base / current</small><strong>' + esc(status && status.baseLinkCount || "—") +
    " / " + esc(currentLinks) + '</strong></div>' +
    '<div class="wb-metric"><small>Prepare / load / runs</small><strong>' + esc(status && status.prepareCount || 0) +
    " / " + esc(status && status.loadCount || 0) + " / " + esc(status && status.completedRuns || 0) + '</strong></div>' +
    '</div><div class="wb-result ' + (run ? "ready" : "") + '"><h4>Result</h4><div class="wb-result-value">' +
    esc(resultText(run)) + '</div><div class="wb-help">' +
    esc(run ? (run.configurationReused ? "canonical configuration reused" : "configuration Links published") +
      " · " + assertionText(run) + " · quiescent=" + String(run.observed && run.observed.finalQuiescent)
      : "Press Run. The UI only renders the retained runtime report; it does not recompute the answer.") +
    '</div>' + (run ? '<details class="wb-raw"><summary>Normalized result</summary><pre>' + json(run.result) +
    '</pre></details>' : '') + '</div></div></section>' +

    '<section class="wb-card wb-tabs"><div class="wb-tabbar">' +
    ["history", "log", "profile", "proof"].map((name) =>
      '<button data-tab="' + name + '" aria-selected="' + (state.tab === name) + '">' +
      esc(name[0].toUpperCase() + name.slice(1)) + '</button>').join("") +
    '</div><div class="wb-tab">' + tab(state) + '</div></section></div>';

  const root = state.root;
  root.querySelector("#wb-scenario")?.addEventListener("change", (event) => {
    try { loadManifest(state, Number(event.target.value)); } catch (error) {
      state.error = error && error.message || String(error);
    }
    render(state);
  });
  root.querySelector("#wb-preset-run")?.addEventListener("change", (event) => {
    state.presetIndex = Number(event.target.value); presetInputs(state); render(state);
  });
  for (const button of root.querySelectorAll("[data-mode]")) {
    button.addEventListener("click", () => {
      syncInputs(state); state.mode = button.dataset.mode;
      if (state.mode === "preset") presetInputs(state);
      render(state);
    });
  }
  root.querySelector("#wb-backend")?.addEventListener("change", (event) => {
    state.backend = event.target.value; render(state);
  });
  for (const input of root.querySelectorAll("[data-input-key]")) {
    input.addEventListener("change", () => { state.inputs[input.dataset.inputKey] = input.value; });
  }
  root.querySelector("#wb-open")?.addEventListener("click", () => void open(state));
  root.querySelector("#wb-run")?.addEventListener("click", () => { syncInputs(state); void execute(state); });
  root.querySelector("#wb-close")?.addEventListener("click", () => close(state));
  for (const button of root.querySelectorAll("[data-tab]")) {
    button.addEventListener("click", () => { state.tab = button.dataset.tab; render(state); });
  }
}

export async function mountWorkbench(root) {
  styles();
  const state = {
    root, wasm: null, registry: { entries: [] }, manifest: { runSequence: [], inputSchema: [] },
    scenarioIndex: 0, presetIndex: 0, inputs: {}, mode: "preset", backend: "optimized-cpu",
    session: null, run: null, history: null, tab: "history",
    build: { version: "loading", sha: "loading" }, loading: true, error: null,
  };
  root.innerHTML = '<div class="notice">Loading real A-memory Workbench…</div>';
  try {
    const loaded = await Promise.all([wasm(), buildInfo()]);
    state.wasm = loaded[0]; state.build = loaded[1];
    state.registry = refreshScenarioPresetRegistry(state.wasm);
    let index = state.registry.entries.findIndex((entry) => entry.scenarioId === "mux1-lifecycle");
    if (index < 0) index = 0;
    loadManifest(state, index);
    state.loading = false; render(state);
    await open(state);
  } catch (error) {
    state.loading = false; state.error = error && error.message || String(error); render(state);
  }
  return state;
}

if (typeof document !== "undefined") {
  const boot = () => {
    const root = document.querySelector("#workbench-root");
    if (root) void mountWorkbench(root);
  };
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", boot, { once: true });
  else boot();
}
