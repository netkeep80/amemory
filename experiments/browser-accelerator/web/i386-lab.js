import {
  FLAGS,
  blockDefinitionHtml,
  catalogCardHtml,
  escapeHtml,
  evidenceHtml,
  flagState,
  hex32,
  inputPreviewHtml,
  outputHtml,
  outputValue,
} from "./i386-lab-view.mjs";
import { renderProofPipeline } from "./i386-proof-view.mjs";
import { collectBrowserProof } from "./i386-proof-transport.mjs";

function parseWord(text) {
  const value = String(text).trim();
  if (!value) throw new Error("empty value");
  let n;
  if (/^0x[0-9a-f]+$/i.test(value)) n = Number.parseInt(value.slice(2), 16);
  else if (/^[0-9]+$/.test(value)) n = Number.parseInt(value, 10);
  else throw new Error(`invalid 32-bit value: ${text}`);
  if (!Number.isFinite(n) || n < 0 || n > 0xffff_ffff) {
    throw new Error(`outside uint32: ${text}`);
  }
  return n >>> 0;
}

function parseTyped(type, value) {
  if (type === "word32") return parseWord(value);
  if (type === "bit") {
    const n = Number(value);
    if (n !== 0 && n !== 1) throw new Error(`bit must be 0 or 1: ${value}`);
    return n;
  }
  if (type === "count8") {
    const n = Number(value);
    if (!Number.isInteger(n) || n < 0 || n > 255) {
      throw new Error(`count8 must be 0..255: ${value}`);
    }
    return n;
  }
  throw new Error(`unsupported input type: ${type}`);
}

async function loadRegistry() {
  const response = await fetch("./i386-blocks.json", { cache: "no-store" });
  if (!response.ok) throw new Error(`block registry HTTP ${response.status}`);
  const registry = await response.json();
  if (registry.schemaVersion !== 1 || !Array.isArray(registry.blocks)) {
    throw new Error("unsupported block registry");
  }
  return registry;
}

async function loadLabWasm() {
  const response = await fetch("./amemory_a_circuit.wasm", { cache: "no-store" });
  if (!response.ok) throw new Error(`A-Circuit WASM HTTP ${response.status}`);
  const bytes = await response.arrayBuffer();
  const { instance } = await WebAssembly.instantiate(bytes, {});
  const wasm = instance.exports;
  if (wasm.amemory_i386_lab_probe?.() !== 0x386) {
    throw new Error("unexpected A-Circuit WASM probe");
  }
  return wasm;
}

function styleLab() {
  const style = document.createElement("style");
  style.textContent = `
    .i386-lab { margin: 34px 0 44px; }
    .i386-lab > h2 { margin-bottom: 6px; }
    .i386-lab > p { max-width: 920px; margin-top: 0; }
    .lab-state-shell { margin:16px 0; padding:16px 18px; background:var(--surface); border:2px solid var(--line); border-radius:16px; box-shadow:var(--shadow); }
    .lab-state-head { display:flex; justify-content:space-between; gap:18px; align-items:flex-start; }
    .lab-state-head h3 { margin:0 0 5px; }
    .lab-state-head p { margin:0; color:var(--muted); max-width:920px; }
    .lab-state-grid { display:grid; grid-template-columns:1fr 1fr; gap:12px; margin-top:12px; }
    .lab-state-snapshot { border:1px solid var(--line); border-radius:12px; background:var(--surface-2); padding:12px; }
    .lab-state-snapshot h4 { margin:0 0 8px; }
    .lab-state-registers { display:grid; gap:5px; }
    .lab-state-registers code { overflow-wrap:anywhere; }
    .lab-state-note { margin-top:10px; color:var(--muted); font-size:.82rem; }
    .lab-layout { display:grid; grid-template-columns:minmax(250px,300px) minmax(0,1fr); gap:16px; align-items:start; }
    .lab-catalog-shell,.lab-shell { background:var(--surface); border:1px solid var(--line); border-radius:16px; box-shadow:var(--shadow); }
    .lab-catalog-shell { padding:14px; position:sticky; top:12px; max-height:calc(100vh - 24px); overflow:auto; }
    .lab-shell { padding:18px; min-width:0; }
    .lab-catalog-head { display:flex; align-items:baseline; justify-content:space-between; gap:10px; margin-bottom:10px; }
    .lab-catalog-head h3 { margin:0; font-size:1rem; }
    .lab-catalog-head small { color:var(--muted); }
    .lab-search { width:100%; min-height:40px; border-radius:10px; border:1px solid var(--line); background:var(--surface-2); color:var(--text); padding:8px 10px; }
    .lab-categorybar { display:flex; gap:6px; flex-wrap:wrap; margin:10px 0; }
    .lab-category,.lab-tab,.lab-run,.lab-small {
      min-height:38px; border-radius:10px; border:1px solid var(--line);
      background:var(--surface-2); color:var(--text); padding:7px 10px; cursor:pointer;
    }
    .lab-category[aria-pressed="true"],.lab-tab[aria-selected="true"] { font-weight:800; outline:2px solid var(--accent); }
    .lab-catalog { display:grid; gap:8px; }
    .lab-block-card { width:100%; text-align:left; display:grid; gap:5px; border:1px solid var(--line); border-radius:12px; background:var(--surface-2); color:var(--text); padding:10px 11px; cursor:pointer; }
    .lab-block-card:hover { border-color:var(--accent); }
    .lab-block-card[aria-current="true"] { outline:2px solid var(--accent); background:var(--surface); }
    .lab-card-top { display:flex; justify-content:space-between; gap:8px; align-items:baseline; }
    .lab-card-category { color:var(--muted); font-size:.72rem; text-transform:uppercase; letter-spacing:.06em; }
    .lab-block-card code { color:var(--muted); font-size:.75rem; overflow-wrap:anywhere; }
    .lab-block-card small { color:var(--muted); }
    .lab-definition { display:grid; gap:12px; margin-bottom:12px; }
    .lab-definition-head { display:flex; flex-wrap:wrap; justify-content:space-between; gap:12px; align-items:start; }
    .lab-definition-head h3 { margin:0; font-size:1.35rem; }
    .lab-definition-head code { display:block; margin-top:5px; }
    .lab-badges { display:flex; flex-wrap:wrap; gap:6px; }
    .lab-pill { padding:5px 8px; border:1px solid var(--line); border-radius:999px; background:var(--surface-2); color:var(--muted); font-size:.76rem; }
    .lab-spec-grid { display:grid; grid-template-columns:1fr 1fr; gap:10px; }
    .lab-spec { border:1px solid var(--line); border-radius:10px; padding:10px 12px; background:var(--surface-2); }
    .lab-spec strong { display:block; margin-bottom:6px; }
    .lab-spec code { display:block; overflow-wrap:anywhere; margin:3px 0; }
    .lab-modebar { display:flex; justify-content:space-between; gap:12px; align-items:center; border-top:1px solid var(--line); padding-top:12px; }
    .lab-modebar > span { color:var(--muted); font-size:.82rem; }
    .lab-tabs { display:flex; flex-wrap:wrap; gap:7px; }
    .lab-constructor { display:grid; grid-template-columns:minmax(0,1fr) minmax(190px,230px) minmax(0,1fr); gap:18px; align-items:stretch; margin-top:16px; }
    .lab-stage-label { color:var(--muted); font-size:.72rem; font-weight:800; letter-spacing:.08em; text-transform:uppercase; margin-bottom:8px; }
    .lab-port-stack { display:grid; gap:10px; }
    .lab-port-card { position:relative; border:1px solid var(--line); border-radius:12px; background:var(--surface-2); padding:11px 12px; display:grid; gap:7px; min-width:0; }
    .lab-input-port::after,.lab-output-port::before { content:""; width:10px; height:10px; border-radius:50%; background:var(--accent); position:absolute; top:20px; }
    .lab-input-port::after { right:-6px; }
    .lab-output-port::before { left:-6px; }
    .lab-port-head { display:flex; justify-content:space-between; gap:8px; align-items:baseline; }
    .lab-port-head small { color:var(--muted); }
    .lab-port-card input,.lab-port-card select {
      width:100%; min-height:40px; border-radius:9px; border:1px solid var(--line);
      background:var(--surface); color:var(--text); padding:8px 10px;
    }
    .lab-input-preview { display:grid; gap:2px; color:var(--muted); font-size:.76rem; }
    .lab-input-preview code { color:var(--text); overflow-wrap:anywhere; }
    .lab-chip { position:relative; align-self:center; min-height:210px; border:2px solid var(--text); border-radius:16px; padding:18px 14px; display:flex; flex-direction:column; justify-content:center; text-align:center; background:var(--surface); }
    .lab-chip::before,.lab-chip::after { content:""; position:absolute; top:50%; width:18px; border-top:2px solid var(--text); }
    .lab-chip::before { left:-19px; } .lab-chip::after { right:-19px; }
    .lab-chip h3 { margin:4px 0; font-size:1.3rem; }
    .lab-chip code { overflow-wrap:anywhere; }
    .lab-chip small { color:var(--muted); }
    .lab-chip .lab-run { margin-top:14px; background:var(--accent); color:#fff; border-color:var(--accent); font-weight:800; }
    .lab-word { display:grid; gap:4px; }
    .lab-word span { color:var(--muted); font-size:.82rem; }
    .lab-binary { overflow-wrap:anywhere; font-size:.76rem; }
    .lab-placeholder { color:var(--muted); font-size:.82rem; }
    .lab-evidence { margin-top:14px; }
    .lab-evidence-io { display:grid; grid-template-columns:1fr 1fr; gap:12px; margin-top:12px; }
    .lab-flags { display:flex; flex-wrap:wrap; gap:7px; margin-top:12px; }
    .lab-flag { padding:6px 8px; border-radius:8px; background:var(--surface-2); font-family:ui-monospace,SFMono-Regular,Consolas,monospace; font-size:.82rem; }
    .lab-metrics { display:grid; grid-template-columns:repeat(5,minmax(0,1fr)); gap:10px; margin-top:12px; }
    .lab-metric { padding:10px 12px; border:1px solid var(--line); border-radius:10px; }
    .lab-metric small { display:block; color:var(--muted); }
    .lab-error { color:var(--bad); font-weight:700; }
    .lab-ok { color:var(--good); font-weight:700; }
    .lab-table-wrap { overflow:auto; margin-top:12px; border:1px solid var(--line); border-radius:12px; }
    .lab-table { width:100%; border-collapse:collapse; min-width:760px; }
    .lab-table th,.lab-table td { padding:9px 10px; border-bottom:1px solid var(--line); text-align:left; }
    .lab-table th { color:var(--muted); font-size:.78rem; text-transform:uppercase; }
    .lab-table input,.lab-table select { width:100%; min-width:95px; box-sizing:border-box; background:var(--surface-2); color:var(--text); border:1px solid var(--line); border-radius:7px; padding:7px; }
    .lab-vector-actions { display:flex; gap:8px; flex-wrap:wrap; margin-top:10px; }
    .lab-trace { margin-top:12px; padding:10px 12px; border:1px solid var(--line); border-radius:10px; }
    .proof-pipeline { margin-top:18px; border-top:2px solid var(--text); padding-top:18px; }
    .proof-title { display:flex; justify-content:space-between; gap:16px; align-items:flex-start; }
    .proof-title h3 { margin:0 0 4px; font-size:1.15rem; }
    .proof-title p { margin:0; color:var(--muted); }
    .proof-memory { min-width:250px; display:grid; gap:3px; padding:10px 12px; border-radius:12px; border:2px solid var(--line); }
    .proof-memory small,.proof-memory span { color:var(--muted); }
    .proof-memory strong { font-family:ui-monospace,SFMono-Regular,Consolas,monospace; }
    .proof-pass { border-color:var(--good); }
    .proof-fail { border-color:var(--bad); }
    .proof-stages { display:grid; gap:8px; margin-top:14px; }
    .proof-stage { display:grid; grid-template-columns:44px minmax(0,1fr); gap:12px; border:1px solid var(--line); border-radius:14px; padding:14px; background:var(--surface-2); }
    .proof-stage-number { width:36px; height:36px; display:grid; place-items:center; border-radius:50%; background:var(--accent); color:#fff; font-weight:900; font-size:1.05rem; }
    .proof-stage h4 { margin:3px 0 6px; }
    .proof-stage h5 { margin:14px 0 7px; }
    .proof-stage p { margin:4px 0 10px; }
    .proof-arrow { text-align:center; color:var(--muted); font-weight:800; }
    .proof-kpis { display:flex; flex-wrap:wrap; gap:8px; margin:10px 0; }
    .proof-kpis > span { display:grid; gap:2px; min-width:120px; padding:8px 10px; border:1px solid var(--line); border-radius:9px; background:var(--surface); }
    .proof-kpis small { color:var(--muted); }
    .proof-root-list { display:grid; gap:6px; }
    .proof-root { display:grid; grid-template-columns:minmax(130px,.6fr) minmax(0,2fr); gap:6px 10px; padding:8px 10px; border:1px solid var(--line); border-radius:9px; background:var(--surface); }
    .proof-root > span { color:var(--muted); font-size:.78rem; }
    .proof-root code { grid-column:2; overflow-wrap:anywhere; }
    .proof-aset { max-height:340px; overflow:auto; white-space:pre-wrap; overflow-wrap:anywhere; padding:10px; background:var(--surface); border:1px solid var(--line); border-radius:9px; font-size:.72rem; }
    .proof-reaction-table code { max-width:360px; display:inline-block; overflow-wrap:anywhere; }
    .proof-result-anums { display:grid; gap:8px; margin:10px 0; }
    .proof-result-anums > div { display:grid; gap:5px; padding:9px 10px; border:1px solid var(--line); border-radius:9px; background:var(--surface); }
    .proof-result-anums code { overflow-wrap:anywhere; }
    .proof-visual-toolbar { display:flex; justify-content:space-between; gap:10px; flex-wrap:wrap; margin:14px 0 8px; align-items:center; }
    .proof-visual-toolbar > div:first-child { display:grid; gap:3px; }
    .proof-visual-toolbar span { color:var(--muted); }
    .proof-visual-tabs { display:flex; flex-wrap:wrap; gap:6px; }
    .proof-visual-tabs button { min-height:36px; border:1px solid var(--line); border-radius:9px; padding:6px 9px; background:var(--surface-2); color:var(--text); cursor:pointer; }
    .proof-visual-tabs button[aria-pressed="true"] { outline:2px solid var(--accent); font-weight:800; }
    .proof-visual-note { margin:6px 0 8px; color:var(--muted); font-size:.8rem; }
    .proof-visual-note code { color:var(--text); }
    .proof-visual { min-height:280px; overflow:auto; border:1px solid var(--line); border-radius:12px; background:var(--surface); padding:8px; }
    .proof-visual svg { width:100%; min-width:720px; min-height:520px; }
    .proof-visual-three { height:620px; min-height:620px; overflow:hidden; padding:0; position:relative; }
    .proof-visual-three canvas { display:block; width:100% !important; height:100% !important; }
    .lab-hidden { display:none !important; }
    @media(max-width:1050px){ .lab-layout{grid-template-columns:1fr;} .lab-catalog-shell{position:static;max-height:none;} .lab-catalog{grid-template-columns:repeat(3,minmax(0,1fr));} }
    @media(max-width:820px){ .lab-state-head{flex-direction:column;} .lab-state-grid{grid-template-columns:1fr;} .lab-catalog{grid-template-columns:repeat(2,minmax(0,1fr));} .lab-constructor{grid-template-columns:1fr;} .lab-chip::before,.lab-chip::after,.lab-input-port::after,.lab-output-port::before{display:none;} .lab-evidence-io,.lab-spec-grid,.lab-metrics{grid-template-columns:1fr 1fr;} .proof-title{flex-direction:column;} .proof-memory{min-width:0;width:100%;box-sizing:border-box;} .proof-root{grid-template-columns:1fr;} .proof-root code{grid-column:1;} }
    @media(max-width:560px){ .lab-catalog{grid-template-columns:1fr;} .lab-evidence-io,.lab-spec-grid,.lab-metrics{grid-template-columns:1fr;} .lab-modebar{align-items:flex-start;flex-direction:column;} }
  `;
  document.head.append(style);
}

function inputControl(def, value, rowMode = false) {
  const label = rowMode ? ` aria-label="${escapeHtml(def.key)}"` : "";
  if (def.type === "bit") {
    return `<select data-input="${escapeHtml(def.key)}" data-type="bit"${label}>
      <option value="0"${Number(value) === 0 ? " selected" : ""}>0</option>
      <option value="1"${Number(value) === 1 ? " selected" : ""}>1</option>
    </select>`;
  }
  if (def.type === "count8") {
    return `<input type="number" min="0" max="255" step="1" data-input="${escapeHtml(def.key)}" data-type="count8" value="${escapeHtml(value)}"${label}>`;
  }
  return `<input data-input="${escapeHtml(def.key)}" data-type="${escapeHtml(def.type)}" value="${escapeHtml(value)}"${label}>`;
}

function inputPortCard(def, value) {
  let preview;
  try {
    preview = inputPreviewHtml(def, parseTyped(def.type, value));
  } catch {
    preview = '<div class="lab-input-preview lab-error">invalid input</div>';
  }
  return `
    <div class="lab-port-card lab-input-port" data-port="${escapeHtml(def.key)}">
      <div class="lab-port-head">
        <strong>${escapeHtml(def.key)}</strong>
        <small>${escapeHtml(def.type)} · ABI ${escapeHtml(def.slot)}</small>
      </div>
      ${inputControl(def, value)}
      <div data-input-preview>${preview}</div>
    </div>`;
}

function outputPlaceholder(block) {
  return (block.outputs || [{ key: "Result", type: "word32" }]).map((def) => `
    <div class="lab-port-card lab-output-port">
      <div class="lab-port-head">
        <strong>${escapeHtml(def.key)}</strong>
        <small>${escapeHtml(def.type)} · ABI ${escapeHtml(def.slot || "value")}</small>
      </div>
      <span class="lab-placeholder">Run the real A-Circuit block to observe this port.</span>
    </div>`).join("");
}

function buildLab(registry) {
  const main = document.querySelector("main");
  if (!main) return null;
  styleLab();

  const section = document.createElement("section");
  section.className = "i386-lab";
  section.id = "i386-lab";
  section.innerHTML = `
    <h2>80386 A-memory logic-block constructor</h2>
    <p>Select a real block, edit its input ports and execute the structural A-Circuit implementation compiled to WASM. The constructor shows the block ABI, outputs, x86 flag patch and structural execution evidence; JavaScript only drives UI and formatting.</p>
    <div class="lab-state-shell" id="lab-state-witness">
      <div class="lab-state-head">
        <div>
          <h3>M5 architectural state witness</h3>
          <p>This is not another opcode. Real structural ALU effects return into temporary state Contexts; the same A-memory atomically publishes a Full GPR + EIP + EFLAGS successor.</p>
        </div>
        <div class="lab-vector-actions">
          <button class="lab-run" id="lab-run-state-add" type="button">Run ADD → EAX State′</button>
          <button class="lab-run" id="lab-run-state-mul" type="button">Run MUL → EDX:EAX State′</button>
          <button class="lab-run" id="lab-run-state-ecx" type="button">Run ADD → ECX full State′</button>
        </div>
      </div>
      <div class="notice" id="lab-state-status">M5 state WASM witness ready check pending…</div>
      <div id="lab-state-result"></div>
      <div id="lab-state-proof"></div>
    </div>
    <div class="lab-state-shell" id="lab-memory-witness">
      <div class="lab-state-head">
        <div>
          <h3>M6 structural radix memory witness</h3>
          <p>Immutable 256-byte structural page: READ old root → WRITE one Byte8 → READ new root → READ old root again. JavaScript supplies only Offset8/Byte8 inputs and renders the compact proof.</p>
        </div>
        <button class="lab-run" id="lab-run-memory" type="button">Run structural memory</button>
      </div>
      <div class="lab-state-grid">
        <label class="lab-state-snapshot">
          <strong>Offset8</strong>
          <input id="lab-memory-offset" type="number" min="0" max="255" step="1" value="37">
        </label>
        <label class="lab-state-snapshot">
          <strong>Byte8 value</strong>
          <input id="lab-memory-value" type="number" min="0" max="255" step="1" value="171">
        </label>
      </div>
      <div class="notice" id="lab-memory-status">M6a WASM witness ready check pending…</div>
      <div id="lab-memory-result"></div>
      <div id="lab-memory-proof"></div>
    </div>
    <div class="lab-layout">
      <aside class="lab-catalog-shell">
        <div class="lab-catalog-head"><h3>Block catalog</h3><small id="lab-count">${registry.blocks.length} blocks</small></div>
        <input class="lab-search" id="lab-search" placeholder="Search AND, ADD, MUL…" aria-label="Search logical blocks">
        <div class="lab-categorybar" id="lab-categories"></div>
        <div class="lab-catalog" id="lab-catalog"></div>
      </aside>
      <div class="lab-shell">
        <div class="lab-definition" id="lab-definition"></div>
        <div class="lab-modebar">
          <span>Test mode</span>
          <div class="lab-tabs" id="lab-tabs"></div>
        </div>
        <div id="lab-status" class="notice">Loading A-Circuit WASM…</div>
        <div id="lab-workspace"></div>
      </div>
    </div>`;

  const overviewNotice = main.querySelector(".notice");
  if (overviewNotice) overviewNotice.after(section);
  else main.prepend(section);
  return section;
}

function setupArchitecturalStateWitness(section, wasm) {
  const shell = section.querySelector("#lab-state-witness");
  const status = section.querySelector("#lab-state-status");
  const result = section.querySelector("#lab-state-result");
  const proofTarget = section.querySelector("#lab-state-proof");
  const runAdd = section.querySelector("#lab-run-state-add");
  const runMul = section.querySelector("#lab-run-state-mul");
  const runEcx = section.querySelector("#lab-run-state-ecx");

  if (!shell || !status || !result || !proofTarget ||
      !runAdd || !runMul || !runEcx) {
    return;
  }
  if (wasm.amemory_i386_state_probe?.() !== 0x50a ||
      typeof wasm.amemory_i386_state_run_add32 !== "function") {
    status.textContent =
      "M5 architectural-state ABI is missing; refusing to fake a UI result.";
    status.className = "notice lab-error";
    runAdd.disabled = true;
    runMul.disabled = true;
    runEcx.disabled = true;
    return;
  }

  const hasWide =
    wasm.amemory_i386_state_wide_probe?.() === 0x50b &&
    typeof wasm.amemory_i386_state_run_mul32 === "function";
  const hasFull =
    wasm.amemory_i386_state_full_probe?.() === 0x50c &&
    typeof wasm.amemory_i386_state_run_add_ecx === "function";
  if (!hasWide) runMul.disabled = true;
  if (!hasFull) runEcx.disabled = true;

  status.textContent = hasWide && hasFull
    ? "M5a/M5b/M5c ready: Full GPR + EIP structural state is executable."
    : "M5 state partially available; unavailable witnesses are disabled.";
  status.className = "notice";

  const readState = (suffix) => {
    const value = (name) => {
      const getter = wasm["amemory_i386_state_" + name + "_" + suffix];
      if (typeof getter !== "function") {
        throw new Error("missing state getter " + name + "_" + suffix);
      }
      return getter() >>> 0;
    };
    return {
      eax: value("eax"),
      ebx: value("ebx"),
      ecx: value("ecx"),
      edx: value("edx"),
      esi: value("esi"),
      edi: value("edi"),
      ebp: value("ebp"),
      esp: value("esp"),
      eip: value("eip"),
    };
  };

  const cases = {
    add: {
      run: () => wasm.amemory_i386_state_run_add32(),
      label: "M5a",
      block: "M5A_STATE_ADD32",
      before: {
        eax: 0xffff_ffff, ebx: 0x1122_3344, ecx: 0x0102_0304,
        edx: 0x5566_7788, esi: 0x1111_2222, edi: 0x3333_4444,
        ebp: 0x5555_6666, esp: 0x7777_8888, eip: 0x0040_1000,
      },
      after: {
        eax: 0, ebx: 0x1122_3344, ecx: 0x0102_0304,
        edx: 0x5566_7788, esi: 0x1111_2222, edi: 0x3333_4444,
        ebp: 0x5555_6666, esp: 0x7777_8888, eip: 0x0040_1000,
      },
      flags: { defined: 0x0000_08d5, values: 0x0000_0055, undefined: 0 },
      note: "real ADD effect → EAX",
    },
    mul: {
      run: () => wasm.amemory_i386_state_run_mul32(),
      label: "M5b",
      block: "M5B_STATE_MUL32",
      before: {
        eax: 0xffff_ffff, ebx: 0x1122_3344, ecx: 0x0102_0304,
        edx: 0xa5a5_5a5a, esi: 0x1111_2222, edi: 0x3333_4444,
        ebp: 0x5555_6666, esp: 0x7777_8888, eip: 0x0040_1000,
      },
      after: {
        eax: 0xffff_fffe, ebx: 0x1122_3344, ecx: 0x0102_0304,
        edx: 0x0000_0001, esi: 0x1111_2222, edi: 0x3333_4444,
        ebp: 0x5555_6666, esp: 0x7777_8888, eip: 0x0040_1000,
      },
      flags: {
        defined: 0x0000_0801,
        values: 0x0000_0801,
        undefined: 0x0000_00d4,
      },
      note: "real MUL effect → EDX:EAX",
    },
    ecx: {
      run: () => wasm.amemory_i386_state_run_add_ecx(),
      label: "M5c",
      block: "M5C_STATE_ADD_ECX",
      before: {
        eax: 0x1020_3040, ebx: 0x1122_3344, ecx: 0xffff_ffff,
        edx: 0x5566_7788, esi: 0x1111_2222, edi: 0x3333_4444,
        ebp: 0x5555_6666, esp: 0x7777_8888, eip: 0x0040_1000,
      },
      after: {
        eax: 0x1020_3040, ebx: 0x1122_3344, ecx: 0,
        edx: 0x5566_7788, esi: 0x1111_2222, edi: 0x3333_4444,
        ebp: 0x5555_6666, esp: 0x7777_8888, eip: 0x0040_1000,
      },
      flags: { defined: 0x0000_08d5, values: 0x0000_0055, undefined: 0 },
      note: "real ADD effect → ECX; EIP protected",
    },
  };

  const assertSnapshot = (actual, expected, label) => {
    for (const [name, value] of Object.entries(expected)) {
      if (actual[name] !== (value >>> 0)) {
        throw new Error(
          label + " " + name.toUpperCase() + " mismatch"
        );
      }
    }
  };
  const registerHtml = (snapshot, expected, phase) =>
    ["eax", "ebx", "ecx", "edx", "esi", "edi", "ebp", "esp", "eip"]
      .map((name) => {
        const changed = expected &&
          expected.before?.[name] !== expected.after?.[name];
        const annotation = phase === "after" && changed ? " · updated" :
          phase === "after" ? " · preserved" : "";
        return `<code>${name.toUpperCase()} = ${hex32(snapshot[name])}${annotation}</code>`;
      })
      .join("");

  const executeStateWitness = async (kind) => {
    const spec = cases[kind];
    try {
      status.textContent =
        "Executing one-memory " + spec.note + " architectural transition…";
      status.className = "notice";
      result.innerHTML = "";
      proofTarget.innerHTML = "";

      if (spec.run() !== 1) {
        throw new Error(spec.label + " A-Circuit WASM rejected the state witness");
      }
      const before = readState("before");
      const after = readState("after");
      assertSnapshot(before, spec.before, spec.label + " before");
      assertSnapshot(after, spec.after, spec.label + " after");

      const flags = {
        defined: wasm.amemory_i386_state_flags_defined_mask() >>> 0,
        flagValues: wasm.amemory_i386_state_flags_value_mask() >>> 0,
        undefined: wasm.amemory_i386_state_flags_undefined_mask() >>> 0,
        preserve: 0,
      };
      const reactions = wasm.amemory_i386_state_reactions() >>> 0;
      const oldStateRetained =
        wasm.amemory_i386_state_old_state_retained() >>> 0;
      const atomicScope = wasm.amemory_i386_state_atomic_scope() >>> 0;
      const steadyDelta =
        wasm.amemory_i386_state_steady_link_delta() >>> 0;
      const quiescent = wasm.amemory_i386_state_quiescent() >>> 0;

      if (flags.defined !== spec.flags.defined ||
          flags.flagValues !== spec.flags.values ||
          flags.undefined !== spec.flags.undefined ||
          oldStateRetained !== 1 ||
          atomicScope !== 1 ||
          steadyDelta !== 0 ||
          quiescent !== 1) {
        throw new Error(spec.label + " state/flags/atomicity witness failed");
      }

      const flagHtml = FLAGS.map(([name, mask]) =>
        `<span class="lab-flag">${escapeHtml(
          flagState(name, mask, flags)
        )}</span>`
      ).join("");
      result.innerHTML = `
        <div class="lab-state-grid">
          <div class="lab-state-snapshot">
            <h4>Stateₜ · before</h4>
            <div class="lab-state-registers">
              ${registerHtml(before, spec, "before")}
            </div>
            <p class="lab-state-note">Old State remains physically present; currentness moves by structural publication, not pole mutation.</p>
          </div>
          <div class="lab-state-snapshot">
            <h4>Stateₜ₊₁ · atomically published</h4>
            <div class="lab-state-registers">
              ${registerHtml(after, spec, "after")}
            </div>
            <div class="lab-flags">${flagHtml}</div>
          </div>
        </div>
        <div class="lab-metrics">
          <div class="lab-metric"><small>Structural reactions</small><strong>${reactions}</strong></div>
          <div class="lab-metric"><small>One-member Scope throughout</small><strong>${atomicScope ? "YES" : "NO"}</strong></div>
          <div class="lab-metric"><small>Old state retained</small><strong>${oldStateRetained ? "YES" : "NO"}</strong></div>
          <div class="lab-metric"><small>Final quiescence</small><strong>${quiescent ? "YES" : "NO"}</strong></div>
          <div class="lab-metric"><small>Identical rerun Link growth</small><strong>${steadyDelta}</strong></div>
        </div>`;

      const { proof, transport } = collectBrowserProof(wasm);
      if (!proof || proof.block !== spec.block) {
        throw new Error(
          spec.label + " compact proof did not reconstruct the state witness"
        );
      }
      await renderProofPipeline(proofTarget, proof);
      status.textContent =
        spec.label + " PASS: " + spec.note +
        " atomically published a full State′ in one A-memory; " +
        transport + " proof rendered below.";
      status.className = "notice lab-ok";
    } catch (error) {
      status.textContent = error.message;
      status.className = "notice lab-error";
    }
  };

  runAdd.addEventListener("click", () => executeStateWitness("add"));
  runMul.addEventListener("click", () => executeStateWitness("mul"));
  runEcx.addEventListener("click", () => executeStateWitness("ecx"));
}

function setupMemoryWitness(section, wasm) {
  const shell = section.querySelector("#lab-memory-witness");
  const status = section.querySelector("#lab-memory-status");
  const result = section.querySelector("#lab-memory-result");
  const proofTarget = section.querySelector("#lab-memory-proof");
  const run = section.querySelector("#lab-run-memory");
  const offsetInput = section.querySelector("#lab-memory-offset");
  const valueInput = section.querySelector("#lab-memory-value");

  if (!shell || !status || !result || !proofTarget ||
      !run || !offsetInput || !valueInput) {
    return;
  }
  if (wasm.amemory_i386_memory_probe?.() !== 0x60a ||
      typeof wasm.amemory_i386_memory_run !== "function") {
    status.textContent =
      "M6a structural-memory ABI is missing; refusing to fake memory.";
    status.className = "notice lab-error";
    run.disabled = true;
    return;
  }
  status.textContent =
    "M6a ready: immutable structural radix page, no host semantic RAM.";
  status.className = "notice";

  const byteInput = (input, label) => {
    const value = Number(input.value);
    if (!Number.isInteger(value) || value < 0 || value > 255) {
      throw new Error(label + " must be an integer 0..255");
    }
    return value;
  };

  run.addEventListener("click", async () => {
    try {
      const offset = byteInput(offsetInput, "Offset8");
      const value = byteInput(valueInput, "Byte8");
      status.textContent =
        "Executing structural READ → WRITE → READ → persistence check…";
      status.className = "notice";
      result.innerHTML = "";
      proofTarget.innerHTML = "";

      if (wasm.amemory_i386_memory_run(offset, value) !== 1) {
        throw new Error("M6a A-Circuit WASM rejected memory inputs");
      }
      const observed = {
        offset: wasm.amemory_i386_memory_offset() >>> 0,
        write: wasm.amemory_i386_memory_write_value() >>> 0,
        before: wasm.amemory_i386_memory_before_value() >>> 0,
        after: wasm.amemory_i386_memory_after_value() >>> 0,
        oldAfter: wasm.amemory_i386_memory_old_after_value() >>> 0,
        oldRoot: wasm.amemory_i386_memory_old_root_ref() >>> 0,
        newRoot: wasm.amemory_i386_memory_new_root_ref() >>> 0,
        reactions: wasm.amemory_i386_memory_reactions() >>> 0,
        linksAfterLoad: wasm.amemory_i386_memory_links_after_load() >>> 0,
        linksFinal: wasm.amemory_i386_memory_links_final() >>> 0,
        steadyDelta: wasm.amemory_i386_memory_steady_link_delta() >>> 0,
        quiescent: wasm.amemory_i386_memory_quiescent() >>> 0,
      };
      if (observed.offset !== offset ||
          observed.write !== value ||
          observed.before !== 0 ||
          observed.after !== value ||
          observed.oldAfter !== 0 ||
          observed.oldRoot === observed.newRoot ||
          observed.steadyDelta !== 0 ||
          observed.quiescent !== 1) {
        throw new Error("M6a structural memory persistence check failed");
      }

      const { proof, transport } = collectBrowserProof(wasm);
      if (!proof || proof.block !== "M6A_RADIX_PAGE") {
        throw new Error("M6a compact proof did not reconstruct memory witness");
      }
      const memoryId = proof.load.memoryInstanceId;
      result.innerHTML = `
        <div class="lab-state-grid">
          <div class="lab-state-snapshot">
            <h4>Old page root</h4>
            <div class="lab-state-registers">
              <code>${escapeHtml(memoryId)}:L${observed.oldRoot}</code>
              <code>READ[${hex32(offset)}] = 0x${observed.before.toString(16).padStart(2,"0")}</code>
              <code>READ old root after WRITE = 0x${observed.oldAfter.toString(16).padStart(2,"0")}</code>
            </div>
          </div>
          <div class="lab-state-snapshot">
            <h4>New immutable page root</h4>
            <div class="lab-state-registers">
              <code>${escapeHtml(memoryId)}:L${observed.newRoot}</code>
              <code>WRITE[${hex32(offset)}] = 0x${observed.write.toString(16).padStart(2,"0")}</code>
              <code>READ new root = 0x${observed.after.toString(16).padStart(2,"0")}</code>
            </div>
          </div>
        </div>
        <div class="lab-metrics">
          <div class="lab-metric"><small>Structural reactions</small><strong>${observed.reactions}</strong></div>
          <div class="lab-metric"><small>Links after LOAD</small><strong>${observed.linksAfterLoad}</strong></div>
          <div class="lab-metric"><small>Links final</small><strong>${observed.linksFinal}</strong></div>
          <div class="lab-metric"><small>Old root preserved</small><strong>YES</strong></div>
          <div class="lab-metric"><small>Identical rerun Link growth</small><strong>${observed.steadyDelta}</strong></div>
        </div>`;
      await renderProofPipeline(proofTarget, proof);
      status.textContent =
        "M6a PASS: structural radix memory persisted old/new roots in one A-memory; " +
        transport + " proof rendered below.";
      status.className = "notice lab-ok";
    } catch (error) {
      status.textContent = error.message;
      status.className = "notice lab-error";
    }
  });
}

function setupCatalog(section, registry, selectBlock) {
  const catalog = section.querySelector("#lab-catalog");
  const categories = section.querySelector("#lab-categories");
  const search = section.querySelector("#lab-search");
  const count = section.querySelector("#lab-count");
  const allCategories = ["All", ...new Set(registry.blocks.map((block) => block.category))];
  let activeCategory = "All";

  const render = () => {
    const query = search.value.trim().toLowerCase();
    const visible = registry.blocks.filter((block) => {
      const categoryOk = activeCategory === "All" || block.category === activeCategory;
      const haystack = `${block.name} ${block.category} ${block.formula} ${block.opcode}`.toLowerCase();
      return categoryOk && (!query || haystack.includes(query));
    });
    catalog.innerHTML = visible.map(catalogCardHtml).join("");
    count.textContent = `${visible.length}/${registry.blocks.length} blocks`;
    catalog.querySelectorAll("[data-block-id]").forEach((button) => {
      button.addEventListener("click", () => selectBlock(button.dataset.blockId));
    });
    markSelected(section, section.dataset.selectedBlock);
  };

  categories.innerHTML = allCategories.map((category, index) =>
    `<button type="button" class="lab-category" data-category="${escapeHtml(category)}" aria-pressed="${index === 0}">${escapeHtml(category)}</button>`
  ).join("");
  categories.querySelectorAll("[data-category]").forEach((button) => {
    button.addEventListener("click", () => {
      activeCategory = button.dataset.category;
      categories.querySelectorAll("[data-category]").forEach((candidate) =>
        candidate.setAttribute("aria-pressed", String(candidate === button))
      );
      render();
    });
  });
  search.addEventListener("input", render);
  render();
}

function markSelected(section, id) {
  section.dataset.selectedBlock = id || "";
  section.querySelectorAll("[data-block-id]").forEach((button) =>
    button.setAttribute("aria-current", String(button.dataset.blockId === id))
  );
}

function readInputs(container, block) {
  const values = {};
  for (const def of block.inputs) {
    const control = container.querySelector(`[data-input="${def.key}"]`);
    if (!control) throw new Error(`missing input ${def.key}`);
    values[def.key] = parseTyped(def.type, control.value);
  }
  return values;
}

function abiArgs(block, values) {
  let a = 0, b = 0, flag = 0;
  for (const def of block.inputs) {
    const value = values[def.key];
    if (def.slot === "a") a = value;
    else if (def.slot === "b") b = value;
    else if (def.slot === "flag") flag = value;
    else throw new Error(`unsupported ABI slot ${def.slot}`);
  }
  return [block.opcode, a >>> 0, b >>> 0, flag >>> 0];
}

function collectOutcome(wasm) {
  return {
    value: wasm.amemory_i386_lab_value() >>> 0,
    valueHi: wasm.amemory_i386_lab_value_hi() >>> 0,
    writeback: wasm.amemory_i386_lab_writeback() >>> 0,
    defined: wasm.amemory_i386_lab_defined_mask() >>> 0,
    flagValues: wasm.amemory_i386_lab_value_mask() >>> 0,
    undefined: wasm.amemory_i386_lab_undefined_mask() >>> 0,
    preserve: wasm.amemory_i386_lab_preserve_mask() >>> 0,
    reactions: wasm.amemory_i386_lab_reactions() >>> 0,
    linksBuild: wasm.amemory_i386_lab_links_after_build() >>> 0,
    linksFirst: wasm.amemory_i386_lab_links_after_first() >>> 0,
    steadyDelta: wasm.amemory_i386_lab_steady_link_delta() >>> 0,
    quiescent: wasm.amemory_i386_lab_quiescent() >>> 0,
  };
}

function runBlock(wasm, block, values) {
  if (wasm.amemory_i386_lab_supports?.(block.opcode) !== 1) {
    throw new Error(`registry block ${block.name} is not supported by A-Circuit WASM`);
  }
  const ok = wasm.amemory_i386_lab_run(...abiArgs(block, values));
  if (ok !== 1) throw new Error(`${block.name}: A-Circuit rejected input`);
  const outcome = collectOutcome(wasm);
  const { proof, compactProof, transport } = collectBrowserProof(wasm);
  outcome.proof = proof;
  outcome.compactProof = compactProof;
  outcome.proofTransport = transport;
  return outcome;
}

function compactOutput(block, out) {
  const outputs = block.outputs?.length ? block.outputs : [{ key: "Result", type: "word32", slot: "value" }];
  return outputs.map((def) => {
    const value = outputValue(out, def);
    return `${escapeHtml(def.key)}=${def.type === "bit" ? value : hex32(value)}`;
  }).join("<br>");
}

function setDefinition(section, block) {
  section.querySelector("#lab-definition").innerHTML = blockDefinitionHtml(block);
}

function renderSingle(section, block, wasm) {
  const workspace = section.querySelector("#lab-workspace");
  workspace.innerHTML = `
    <div class="lab-constructor">
      <div>
        <div class="lab-stage-label">Editable INPUT ports</div>
        <div class="lab-port-stack" id="lab-single-inputs">
          ${block.inputs.map((def) => inputPortCard(def, def.default)).join("")}
        </div>
      </div>
      <div class="lab-chip">
        <small>${escapeHtml(block.category)} · opcode ${block.opcode}</small>
        <h3>${escapeHtml(block.name)}</h3>
        <code>${escapeHtml(block.formula)}</code>
        <small>real structural A-Circuit WASM</small>
        <button class="lab-run" id="lab-run-single" type="button">Run in A-memory</button>
      </div>
      <div>
        <div class="lab-stage-label">OUTPUT ports</div>
        <div class="lab-port-stack" id="lab-output-live">${outputPlaceholder(block)}</div>
      </div>
    </div>
    <div class="lab-evidence" id="lab-single-result"></div>
    <div id="lab-structural-proof"></div>`;

  const controls = workspace.querySelector("#lab-single-inputs");
  controls.querySelectorAll("[data-input]").forEach((control) => {
    const def = block.inputs.find((candidate) => candidate.key === control.dataset.input);
    const updatePreview = () => {
      const preview = control.closest("[data-port]").querySelector("[data-input-preview]");
      try {
        preview.innerHTML = inputPreviewHtml(def, parseTyped(def.type, control.value));
      } catch (error) {
        preview.innerHTML = `<span class="lab-error">${escapeHtml(error.message)}</span>`;
      }
    };
    control.addEventListener("input", updatePreview);
    control.addEventListener("change", updatePreview);
  });

  workspace.querySelector("#lab-run-single").addEventListener("click", async () => {
    const status = section.querySelector("#lab-status");
    try {
      const values = readInputs(controls, block);
      status.textContent = `Executing ${block.name} structurally…`;
      status.className = "notice";
      const out = runBlock(wasm, block, values);
      workspace.querySelector("#lab-output-live").innerHTML = outputHtml(block, out);
      workspace.querySelector("#lab-single-result").innerHTML = evidenceHtml(block, values, out);
      const proofTarget = workspace.querySelector("#lab-structural-proof");
      if (out.proof) {
        await renderProofPipeline(proofTarget, out.proof);
      } else {
        proofTarget.innerHTML = '<div class="notice">Full portable-Aset / one-memory proof is not yet enabled for this registry block. Proof coverage is being generalized across the structural block registry.</div>';
      }
      status.textContent = out.proofTransport === "compact"
        ? `${block.name}: real structural result returned by A-Circuit WASM; compact proof is the browser transport authority.`
        : `${block.name}: real structural result returned by A-Circuit WASM; no structural proof transport for this block.`;
      status.className = "notice lab-ok";
    } catch (error) {
      status.textContent = error.message;
      status.className = "notice lab-error";
    }
  });
}

function vectorExpectation(out, vector) {
  if (!vector.expect) return { label: "OBSERVED", ok: null };
  const checks = [];
  if (vector.expect.value !== undefined) checks.push(out.value === parseWord(vector.expect.value));
  if (vector.expect.valueHi !== undefined) checks.push(out.valueHi === parseWord(vector.expect.valueHi));
  if (vector.expect.writeback !== undefined) checks.push(out.writeback === Number(vector.expect.writeback));
  if (vector.expect.definedMask !== undefined) checks.push(out.defined === parseWord(vector.expect.definedMask));
  if (vector.expect.valueMask !== undefined) checks.push(out.flagValues === parseWord(vector.expect.valueMask));
  if (vector.expect.undefinedMask !== undefined) checks.push(out.undefined === parseWord(vector.expect.undefinedMask));
  if (vector.expect.preserveMask !== undefined) checks.push(out.preserve === parseWord(vector.expect.preserveMask));
  const ok = checks.every(Boolean);
  return { label: ok ? "PASS" : "FAIL", ok };
}

function vectorRowHtml(block, vector, index, custom = false) {
  const cells = block.inputs.map((def) => {
    const value = vector.inputs?.[def.key] ?? def.default;
    return `<td>${inputControl(def, value, true)}</td>`;
  }).join("");
  return `<tr data-vector-row="${index}" data-custom="${custom}">
    <td><input data-vector-name value="${escapeHtml(vector.name || "custom")}"></td>
    ${cells}
    <td data-vector-result>—</td>
    <td>
      <button class="lab-small" data-run-row type="button">Run</button>
      ${custom ? '<button class="lab-small" data-remove-row type="button">Remove</button>' : ""}
    </td>
  </tr>`;
}

function renderVectors(section, block, wasm) {
  const workspace = section.querySelector("#lab-workspace");
  const vectors = (block.vectors || []).map((v) => JSON.parse(JSON.stringify(v)));
  const headers = block.inputs.map((x) => `<th>${escapeHtml(x.key)}</th>`).join("");

  workspace.innerHTML = `
    <div class="lab-vector-actions">
      <button class="lab-run" id="lab-run-all" type="button">Run all canonical vectors</button>
      <button class="lab-small" id="lab-add-row" type="button">Add custom row</button>
    </div>
    <div class="lab-table-wrap">
      <table class="lab-table">
        <thead><tr><th>Case</th>${headers}<th>Observed</th><th>Action</th></tr></thead>
        <tbody id="lab-vector-body">${vectors.map((v,i)=>vectorRowHtml(block,v,i)).join("")}</tbody>
      </table>
    </div>`;

  const body = workspace.querySelector("#lab-vector-body");

  const runRow = (row) => {
    const status = section.querySelector("#lab-status");
    try {
      const vectorIndex = Number(row.dataset.vectorRow);
      const isCustom = row.dataset.custom === "true";
      const vector = !isCustom && Number.isInteger(vectorIndex) && vectorIndex < vectors.length ? vectors[vectorIndex] : {};
      const values = readInputs(row, block);
      const out = runBlock(wasm, block, values);
      const verdict = vectorExpectation(out, vector);
      row.querySelector("[data-vector-result]").innerHTML =
        `<strong class="${verdict.ok === false ? "lab-error" : "lab-ok"}">${verdict.label}</strong><br><code>${compactOutput(block, out)}</code><br><small>${out.reactions} reactions · Q=${out.quiescent} · ΔLinks=${out.steadyDelta}</small>`;
      status.textContent = `${block.name}: vector executed in A-memory.`;
      status.className = "notice";
    } catch (error) {
      row.querySelector("[data-vector-result]").innerHTML = `<span class="lab-error">${escapeHtml(error.message)}</span>`;
      status.textContent = error.message;
      status.className = "notice lab-error";
    }
  };

  const bindRow = (row) => {
    row.querySelector("[data-run-row]").addEventListener("click", () => runRow(row));
    row.querySelector("[data-remove-row]")?.addEventListener("click", () => row.remove());
  };
  [...body.querySelectorAll("[data-vector-row]")].forEach(bindRow);

  workspace.querySelector("#lab-run-all").addEventListener("click", () => {
    [...body.querySelectorAll("[data-vector-row]")].forEach(runRow);
  });

  workspace.querySelector("#lab-add-row").addEventListener("click", () => {
    const index = vectors.length + body.children.length;
    const holder = document.createElement("tbody");
    holder.innerHTML = vectorRowHtml(block, { name: "custom", inputs: {} }, index, true);
    const row = holder.firstElementChild;
    body.append(row);
    bindRow(row);
  });
}

function cartesianEntries(values, keys, index = 0, current = {}, out = []) {
  if (index === keys.length) {
    out.push({ ...current });
    return out;
  }
  const key = keys[index];
  for (const value of values[key] || []) {
    current[key] = value;
    cartesianEntries(values, keys, index + 1, current, out);
  }
  return out;
}

function renderSweep(section, block, wasm) {
  const workspace = section.querySelector("#lab-workspace");
  if (!block.sweep || block.sweep.kind !== "cartesian") {
    workspace.innerHTML = '<div class="notice">This block has no finite exhaustive sweep.</div>';
    return;
  }
  const keys = block.inputs.map((x) => x.key);
  const cases = cartesianEntries(block.sweep.values, keys);
  const headers = keys.map((x) => `<th>${escapeHtml(x)}</th>`).join("");
  workspace.innerHTML = `
    <div class="lab-vector-actions"><button class="lab-run" id="lab-run-sweep" type="button">Run exhaustive sweep (${cases.length})</button></div>
    <div class="lab-table-wrap"><table class="lab-table">
      <thead><tr>${headers}<th>Output</th><th>Evidence</th></tr></thead>
      <tbody>
        ${cases.map((values,i)=>`<tr data-sweep="${i}">${keys.map(k=>`<td><code>${values[k]}</code></td>`).join("")}<td>—</td><td>—</td></tr>`).join("")}
      </tbody>
    </table></div>`;

  workspace.querySelector("#lab-run-sweep").addEventListener("click", () => {
    const rows = [...workspace.querySelectorAll("[data-sweep]")];
    for (const row of rows) {
      const values = cases[Number(row.dataset.sweep)];
      const out = runBlock(wasm, block, values);
      row.children[keys.length].innerHTML = `<strong>${compactOutput(block, out)}</strong>`;
      row.children[keys.length + 1].innerHTML = `<small>${out.reactions} reactions · Q=${out.quiescent} · ΔLinks=${out.steadyDelta}</small>`;
    }
    const status = section.querySelector("#lab-status");
    status.textContent = `${block.name}: exhaustive ${cases.length}-case sweep completed in A-memory.`;
    status.className = "notice lab-ok";
  });
}

function renderMode(section, block, wasm, mode) {
  if (mode === "single") renderSingle(section, block, wasm);
  else if (mode === "vectors") renderVectors(section, block, wasm);
  else if (mode === "sweep") renderSweep(section, block, wasm);
}

function configureBlock(section, block, wasm) {
  markSelected(section, block.id);
  setDefinition(section, block);

  if (wasm.amemory_i386_lab_supports?.(block.opcode) !== 1) {
    section.querySelector("#lab-status").textContent = `${block.name}: registry/WASM mismatch — unsupported opcode.`;
    section.querySelector("#lab-status").className = "notice lab-error";
    section.querySelector("#lab-workspace").innerHTML = "";
    return;
  }

  const tabs = section.querySelector("#lab-tabs");
  tabs.innerHTML = "";
  const modes = block.modes || ["single"];

  const selectMode = (mode) => {
    [...tabs.querySelectorAll("[data-mode]")].forEach((button) =>
      button.setAttribute("aria-selected", String(button.dataset.mode === mode))
    );
    renderMode(section, block, wasm, mode);
  };

  for (const mode of modes) {
    const button = document.createElement("button");
    button.className = "lab-tab";
    button.dataset.mode = mode;
    button.type = "button";
    button.textContent = mode === "single" ? "Single / constructor" : mode === "vectors" ? "Vectors" : "Sweep";
    button.addEventListener("click", () => selectMode(mode));
    tabs.append(button);
  }

  section.querySelector("#lab-status").textContent = `${block.name} ready. Edit ports and run the real A-memory block.`;
  section.querySelector("#lab-status").className = "notice";
  selectMode(modes[0]);
}

async function bootLab() {
  const [registry, wasm] = await Promise.all([loadRegistry(), loadLabWasm()]);
  for (const block of registry.blocks) {
    if (wasm.amemory_i386_lab_supports?.(block.opcode) !== 1) {
      throw new Error(`registry entry ${block.id} has no WASM implementation`);
    }
  }

  const section = buildLab(registry);
  if (!section) return;
  setupArchitecturalStateWitness(section, wasm);
  setupMemoryWitness(section, wasm);
  const byId = new Map(registry.blocks.map((block) => [block.id, block]));
  const selectBlock = (id) => {
    const block = byId.get(id);
    if (!block) return;
    configureBlock(section, block, wasm);
  };
  setupCatalog(section, registry, selectBlock);
  selectBlock(byId.has("mux1") ? "mux1" : registry.blocks[0].id);
}

if (typeof document !== "undefined") {
  bootLab().catch((error) => {
    const main = document.querySelector("main");
    const node = document.createElement("div");
    node.className = "notice lab-error";
    node.textContent = `80386 lab failed closed: ${error.message}`;
    main?.prepend(node);
  });
}
