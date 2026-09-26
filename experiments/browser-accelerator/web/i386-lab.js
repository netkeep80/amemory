import {
  blockDefinitionHtml,
  catalogCardHtml,
  escapeHtml,
  evidenceHtml,
  hex32,
  inputPreviewHtml,
  outputHtml,
  outputValue,
} from "./i386-lab-view.mjs";

function parseWord(text) {
  const value = String(text).trim();
  if (!value) throw new Error("empty value");
  let n;
  if (/^0x[0-9a-f]+$/i.test(value)) n = Number.parseInt(value.slice(2), 16);
  else if (/^[0-9]+$/.test(value)) n = Number.parseInt(value, 10);
  else throw new Error(\`invalid 32-bit value: \${text}\`);
  if (!Number.isFinite(n) || n < 0 || n > 0xffff_ffff) {
    throw new Error(\`outside uint32: \${text}\`);
  }
  return n >>> 0;
}

function parseTyped(type, value) {
  if (type === "word32") return parseWord(value);
  if (type === "bit") {
    const n = Number(value);
    if (n !== 0 && n !== 1) throw new Error(\`bit must be 0 or 1: \${value}\`);
    return n;
  }
  if (type === "count8") {
    const n = Number(value);
    if (!Number.isInteger(n) || n < 0 || n > 255) {
      throw new Error(\`count8 must be 0..255: \${value}\`);
    }
    return n;
  }
  throw new Error(\`unsupported input type: \${type}\`);
}

async function loadRegistry() {
  const response = await fetch("./i386-blocks.json", { cache: "no-store" });
  if (!response.ok) throw new Error(\`block registry HTTP \${response.status}\`);
  const registry = await response.json();
  if (registry.schemaVersion !== 1 || !Array.isArray(registry.blocks)) {
    throw new Error("unsupported block registry");
  }
  return registry;
}

async function loadLabWasm() {
  const response = await fetch("./amemory_a_circuit.wasm", { cache: "no-store" });
  if (!response.ok) throw new Error(\`A-Circuit WASM HTTP \${response.status}\`);
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
  style.textContent = \`
    .i386-lab { margin: 34px 0 44px; }
    .i386-lab > h2 { margin-bottom: 6px; }
    .i386-lab > p { max-width: 920px; margin-top: 0; }
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
    .lab-hidden { display:none !important; }
    @media(max-width:1050px){ .lab-layout{grid-template-columns:1fr;} .lab-catalog-shell{position:static;max-height:none;} .lab-catalog{grid-template-columns:repeat(3,minmax(0,1fr));} }
    @media(max-width:820px){ .lab-catalog{grid-template-columns:repeat(2,minmax(0,1fr));} .lab-constructor{grid-template-columns:1fr;} .lab-chip::before,.lab-chip::after,.lab-input-port::after,.lab-output-port::before{display:none;} .lab-evidence-io,.lab-spec-grid,.lab-metrics{grid-template-columns:1fr 1fr;} }
    @media(max-width:560px){ .lab-catalog{grid-template-columns:1fr;} .lab-evidence-io,.lab-spec-grid,.lab-metrics{grid-template-columns:1fr;} .lab-modebar{align-items:flex-start;flex-direction:column;} }
  \`;
  document.head.append(style);
}

function inputControl(def, value, rowMode = false) {
  const label = rowMode ? \` aria-label="\${escapeHtml(def.key)}"\` : "";
  if (def.type === "bit") {
    return \`<select data-input="\${escapeHtml(def.key)}" data-type="bit"\${label}>
      <option value="0"\${Number(value) === 0 ? " selected" : ""}>0</option>
      <option value="1"\${Number(value) === 1 ? " selected" : ""}>1</option>
    </select>\`;
  }
  if (def.type === "count8") {
    return \`<input type="number" min="0" max="255" step="1" data-input="\${escapeHtml(def.key)}" data-type="count8" value="\${escapeHtml(value)}"\${label}>\`;
  }
  return \`<input data-input="\${escapeHtml(def.key)}" data-type="\${escapeHtml(def.type)}" value="\${escapeHtml(value)}"\${label}>\`;
}

function inputPortCard(def, value) {
  let preview;
  try {
    preview = inputPreviewHtml(def, parseTyped(def.type, value));
  } catch {
    preview = '<div class="lab-input-preview lab-error">invalid input</div>';
  }
  return \`
    <div class="lab-port-card lab-input-port" data-port="\${escapeHtml(def.key)}">
      <div class="lab-port-head">
        <strong>\${escapeHtml(def.key)}</strong>
        <small>\${escapeHtml(def.type)} · ABI \${escapeHtml(def.slot)}</small>
      </div>
      \${inputControl(def, value)}
      <div data-input-preview>\${preview}</div>
    </div>\`;
}

function outputPlaceholder(block) {
  return (block.outputs || [{ key: "Result", type: "word32" }]).map((def) => \`
    <div class="lab-port-card lab-output-port">
      <div class="lab-port-head">
        <strong>\${escapeHtml(def.key)}</strong>
        <small>\${escapeHtml(def.type)} · ABI \${escapeHtml(def.slot || "value")}</small>
      </div>
      <span class="lab-placeholder">Run the real A-Circuit block to observe this port.</span>
    </div>\`).join("");
}

function buildLab(registry) {
  const main = document.querySelector("main");
  if (!main) return null;
  styleLab();

  const section = document.createElement("section");
  section.className = "i386-lab";
  section.id = "i386-lab";
  section.innerHTML = \`
    <h2>80386 A-memory logic-block constructor</h2>
    <p>Select a real block, edit its input ports and execute the structural A-Circuit implementation compiled to WASM. The constructor shows the block ABI, outputs, x86 flag patch and structural execution evidence; JavaScript only drives UI and formatting.</p>
    <div class="lab-layout">
      <aside class="lab-catalog-shell">
        <div class="lab-catalog-head"><h3>Block catalog</h3><small id="lab-count">\${registry.blocks.length} blocks</small></div>
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
    </div>\`;

  const overviewNotice = main.querySelector(".notice");
  if (overviewNotice) overviewNotice.after(section);
  else main.prepend(section);
  return section;
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
      const haystack = \`\${block.name} \${block.category} \${block.formula} \${block.opcode}\`.toLowerCase();
      return categoryOk && (!query || haystack.includes(query));
    });
    catalog.innerHTML = visible.map(catalogCardHtml).join("");
    count.textContent = \`\${visible.length}/\${registry.blocks.length} blocks\`;
    catalog.querySelectorAll("[data-block-id]").forEach((button) => {
      button.addEventListener("click", () => selectBlock(button.dataset.blockId));
    });
    markSelected(section, section.dataset.selectedBlock);
  };

  categories.innerHTML = allCategories.map((category, index) =>
    \`<button type="button" class="lab-category" data-category="\${escapeHtml(category)}" aria-pressed="\${index === 0}">\${escapeHtml(category)}</button>\`
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
    const control = container.querySelector(\`[data-input="\${def.key}"]\`);
    if (!control) throw new Error(\`missing input \${def.key}\`);
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
    else throw new Error(\`unsupported ABI slot \${def.slot}\`);
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
    throw new Error(\`registry block \${block.name} is not supported by A-Circuit WASM\`);
  }
  const ok = wasm.amemory_i386_lab_run(...abiArgs(block, values));
  if (ok !== 1) throw new Error(\`\${block.name}: A-Circuit rejected input\`);
  return collectOutcome(wasm);
}

function compactOutput(block, out) {
  const outputs = block.outputs?.length ? block.outputs : [{ key: "Result", type: "word32", slot: "value" }];
  return outputs.map((def) => {
    const value = outputValue(out, def);
    return \`\${escapeHtml(def.key)}=\${def.type === "bit" ? value : hex32(value)}\`;
  }).join("<br>");
}

function setDefinition(section, block) {
  section.querySelector("#lab-definition").innerHTML = blockDefinitionHtml(block);
}

function renderSingle(section, block, wasm) {
  const workspace = section.querySelector("#lab-workspace");
  workspace.innerHTML = \`
    <div class="lab-constructor">
      <div>
        <div class="lab-stage-label">Editable INPUT ports</div>
        <div class="lab-port-stack" id="lab-single-inputs">
          \${block.inputs.map((def) => inputPortCard(def, def.default)).join("")}
        </div>
      </div>
      <div class="lab-chip">
        <small>\${escapeHtml(block.category)} · opcode \${block.opcode}</small>
        <h3>\${escapeHtml(block.name)}</h3>
        <code>\${escapeHtml(block.formula)}</code>
        <small>real structural A-Circuit WASM</small>
        <button class="lab-run" id="lab-run-single" type="button">Run in A-memory</button>
      </div>
      <div>
        <div class="lab-stage-label">OUTPUT ports</div>
        <div class="lab-port-stack" id="lab-output-live">\${outputPlaceholder(block)}</div>
      </div>
    </div>
    <div class="lab-evidence" id="lab-single-result"></div>\`;

  const controls = workspace.querySelector("#lab-single-inputs");
  controls.querySelectorAll("[data-input]").forEach((control) => {
    const def = block.inputs.find((candidate) => candidate.key === control.dataset.input);
    const updatePreview = () => {
      const preview = control.closest("[data-port]").querySelector("[data-input-preview]");
      try {
        preview.innerHTML = inputPreviewHtml(def, parseTyped(def.type, control.value));
      } catch (error) {
        preview.innerHTML = \`<span class="lab-error">\${escapeHtml(error.message)}</span>\`;
      }
    };
    control.addEventListener("input", updatePreview);
    control.addEventListener("change", updatePreview);
  });

  workspace.querySelector("#lab-run-single").addEventListener("click", () => {
    const status = section.querySelector("#lab-status");
    try {
      const values = readInputs(controls, block);
      status.textContent = \`Executing \${block.name} structurally…\`;
      status.className = "notice";
      const out = runBlock(wasm, block, values);
      workspace.querySelector("#lab-output-live").innerHTML = outputHtml(block, out);
      workspace.querySelector("#lab-single-result").innerHTML = evidenceHtml(block, values, out);
      status.textContent = \`\${block.name}: real structural result returned by A-Circuit WASM.\`;
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
    return \`<td>\${inputControl(def, value, true)}</td>\`;
  }).join("");
  return \`<tr data-vector-row="\${index}" data-custom="\${custom}">
    <td><input data-vector-name value="\${escapeHtml(vector.name || "custom")}"></td>
    \${cells}
    <td data-vector-result>—</td>
    <td>
      <button class="lab-small" data-run-row type="button">Run</button>
      \${custom ? '<button class="lab-small" data-remove-row type="button">Remove</button>' : ""}
    </td>
  </tr>\`;
}

function renderVectors(section, block, wasm) {
  const workspace = section.querySelector("#lab-workspace");
  const vectors = (block.vectors || []).map((v) => JSON.parse(JSON.stringify(v)));
  const headers = block.inputs.map((x) => \`<th>\${escapeHtml(x.key)}</th>\`).join("");

  workspace.innerHTML = \`
    <div class="lab-vector-actions">
      <button class="lab-run" id="lab-run-all" type="button">Run all canonical vectors</button>
      <button class="lab-small" id="lab-add-row" type="button">Add custom row</button>
    </div>
    <div class="lab-table-wrap">
      <table class="lab-table">
        <thead><tr><th>Case</th>\${headers}<th>Observed</th><th>Action</th></tr></thead>
        <tbody id="lab-vector-body">\${vectors.map((v,i)=>vectorRowHtml(block,v,i)).join("")}</tbody>
      </table>
    </div>\`;

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
        \`<strong class="\${verdict.ok === false ? "lab-error" : "lab-ok"}">\${verdict.label}</strong><br><code>\${compactOutput(block, out)}</code><br><small>\${out.reactions} reactions · Q=\${out.quiescent} · ΔLinks=\${out.steadyDelta}</small>\`;
      status.textContent = \`\${block.name}: vector executed in A-memory.\`;
      status.className = "notice";
    } catch (error) {
      row.querySelector("[data-vector-result]").innerHTML = \`<span class="lab-error">\${escapeHtml(error.message)}</span>\`;
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
  const headers = keys.map((x) => \`<th>\${escapeHtml(x)}</th>\`).join("");
  workspace.innerHTML = \`
    <div class="lab-vector-actions"><button class="lab-run" id="lab-run-sweep" type="button">Run exhaustive sweep (\${cases.length})</button></div>
    <div class="lab-table-wrap"><table class="lab-table">
      <thead><tr>\${headers}<th>Output</th><th>Evidence</th></tr></thead>
      <tbody>
        \${cases.map((values,i)=>\`<tr data-sweep="\${i}">\${keys.map(k=>\`<td><code>\${values[k]}</code></td>\`).join("")}<td>—</td><td>—</td></tr>\`).join("")}
      </tbody>
    </table></div>\`;

  workspace.querySelector("#lab-run-sweep").addEventListener("click", () => {
    const rows = [...workspace.querySelectorAll("[data-sweep]")];
    for (const row of rows) {
      const values = cases[Number(row.dataset.sweep)];
      const out = runBlock(wasm, block, values);
      row.children[keys.length].innerHTML = \`<strong>\${compactOutput(block, out)}</strong>\`;
      row.children[keys.length + 1].innerHTML = \`<small>\${out.reactions} reactions · Q=\${out.quiescent} · ΔLinks=\${out.steadyDelta}</small>\`;
    }
    const status = section.querySelector("#lab-status");
    status.textContent = \`\${block.name}: exhaustive \${cases.length}-case sweep completed in A-memory.\`;
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
    section.querySelector("#lab-status").textContent = \`\${block.name}: registry/WASM mismatch — unsupported opcode.\`;
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

  section.querySelector("#lab-status").textContent = \`\${block.name} ready. Edit ports and run the real A-memory block.\`;
  section.querySelector("#lab-status").className = "notice";
  selectMode(modes[0]);
}

async function bootLab() {
  const [registry, wasm] = await Promise.all([loadRegistry(), loadLabWasm()]);
  for (const block of registry.blocks) {
    if (wasm.amemory_i386_lab_supports?.(block.opcode) !== 1) {
      throw new Error(\`registry entry \${block.id} has no WASM implementation\`);
    }
  }

  const section = buildLab(registry);
  if (!section) return;
  const byId = new Map(registry.blocks.map((block) => [block.id, block]));
  const selectBlock = (id) => {
    const block = byId.get(id);
    if (!block) return;
    configureBlock(section, block, wasm);
  };
  setupCatalog(section, registry, selectBlock);
  selectBlock(registry.blocks[0].id);
}

if (typeof document !== "undefined") {
  bootLab().catch((error) => {
    const main = document.querySelector("main");
    const node = document.createElement("div");
    node.className = "notice lab-error";
    node.textContent = \`80386 lab failed closed: \${error.message}\`;
    main?.prepend(node);
  });
}
