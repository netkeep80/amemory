const FLAGS = [
  ["CF", 1 << 0],
  ["PF", 1 << 2],
  ["AF", 1 << 4],
  ["ZF", 1 << 6],
  ["SF", 1 << 7],
  ["OF", 1 << 11],
];

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
  throw new Error(\`unsupported input type: \${type}\`);
}

function hex32(value) {
  return "0x" + (value >>> 0).toString(16).padStart(8, "0");
}

function bin32(value) {
  return (value >>> 0).toString(2).padStart(32, "0").match(/.{1,4}/g).join(" ");
}

function signed32(value) {
  return (value | 0).toString(10);
}

function wordHtml(label, value) {
  return \`
    <div class="lab-word">
      <strong>\${label}</strong>
      <code>\${hex32(value)}</code>
      <span>unsigned \${value >>> 0}</span>
      <span>signed \${signed32(value)}</span>
      <code class="lab-binary">\${bin32(value)}</code>
    </div>\`;
}

function scalarHtml(label, value) {
  return \`
    <div class="lab-word">
      <strong>\${label}</strong>
      <code>\${value}</code>
    </div>\`;
}

function flagState(name, mask, defined, values, undefined, preserve) {
  if (defined & mask) return \`\${name}=SET(\${values & mask ? 1 : 0})\`;
  if (undefined & mask) return \`\${name}=UNDEFINED\`;
  if (preserve & mask) return \`\${name}=PRESERVE\`;
  return \`\${name}=—\`;
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
    .i386-lab { margin: 36px 0; }
    .lab-shell { background: var(--surface); border: 1px solid var(--line); border-radius: 16px; padding: 18px; box-shadow: var(--shadow); }
    .lab-top { display:grid; grid-template-columns:1.3fr 1fr; gap:12px; align-items:end; }
    .lab-tabs { display:flex; flex-wrap:wrap; gap:7px; }
    .lab-tab,.lab-run,.lab-small {
      min-height:40px; border-radius:10px; border:1px solid var(--line);
      background:var(--surface-2); color:var(--text); padding:8px 12px; cursor:pointer;
    }
    .lab-tab[aria-selected="true"] { font-weight:800; outline:2px solid var(--accent); }
    .lab-definition { margin:14px 0; padding:11px 13px; border-radius:10px; background:var(--surface-2); }
    .lab-controls { display:grid; grid-template-columns:repeat(4,minmax(0,1fr)); gap:12px; align-items:end; }
    .lab-field { display:grid; gap:6px; }
    .lab-field label { color:var(--muted); font-size:.8rem; }
    .lab-field input,.lab-field select {
      min-height:42px; border-radius:10px; border:1px solid var(--line);
      background:var(--surface-2); color:var(--text); padding:9px 11px;
    }
    .lab-flow { display:grid; grid-template-columns:1fr auto 1fr; gap:16px; align-items:stretch; margin-top:16px; }
    .lab-side { display:grid; gap:10px; }
    .lab-arrow { align-self:center; color:var(--muted); font-size:2rem; font-weight:800; }
    .lab-word { display:grid; gap:4px; padding:12px; border:1px solid var(--line); border-radius:12px; background:var(--surface-2); }
    .lab-word span { color:var(--muted); font-size:.84rem; }
    .lab-binary { overflow-wrap:anywhere; font-size:.8rem; }
    .lab-flags { display:flex; flex-wrap:wrap; gap:7px; margin-top:12px; }
    .lab-flag { padding:6px 8px; border-radius:8px; background:var(--surface-2); font-family:ui-monospace,SFMono-Regular,Consolas,monospace; font-size:.82rem; }
    .lab-metrics { display:grid; grid-template-columns:repeat(4,minmax(0,1fr)); gap:10px; margin-top:12px; }
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
    @media(max-width:900px){ .lab-top,.lab-controls,.lab-flow{grid-template-columns:1fr 1fr;} .lab-arrow{display:none;} .lab-metrics{grid-template-columns:1fr 1fr;} }
    @media(max-width:560px){ .lab-top,.lab-controls,.lab-flow,.lab-metrics{grid-template-columns:1fr;} }
  \`;
  document.head.append(style);
}

function inputControl(def, value, rowMode = false) {
  if (def.type === "bit") {
    return \`<select data-input="\${def.key}" data-type="bit"><option value="0"\${Number(value) === 0 ? " selected" : ""}>0</option><option value="1"\${Number(value) === 1 ? " selected" : ""}>1</option></select>\`;
  }
  const escaped = String(value).replaceAll('"', "&quot;");
  return \`<input data-input="\${def.key}" data-type="\${def.type}" value="\${escaped}"\${rowMode ? ' aria-label="' + def.key + '"' : ""}>\`;
}

function buildLab(registry) {
  const main = document.querySelector("main");
  if (!main) return null;
  styleLab();

  const section = document.createElement("section");
  section.className = "i386-lab";
  section.innerHTML = \`
    <h2>80386 A-memory interactive test suite</h2>
    <p>Editable tests execute the real <code>amemory-a-circuit</code> structural implementation compiled to WASM. JavaScript only edits inputs, invokes the block and formats returned evidence.</p>
    <div class="lab-shell">
      <div class="lab-top">
        <div class="lab-field"><label>Logical block</label><select id="lab-block"></select></div>
        <div class="lab-tabs" id="lab-tabs"></div>
      </div>
      <div class="lab-definition" id="lab-definition"></div>
      <div id="lab-status" class="notice">Loading A-Circuit WASM…</div>
      <div id="lab-workspace"></div>
    </div>\`;

  const raw = main.querySelector(".panel.raw");
  if (raw) main.insertBefore(section, raw);
  else main.append(section);

  const select = section.querySelector("#lab-block");
  for (const block of registry.blocks) {
    const option = document.createElement("option");
    option.value = block.id;
    option.textContent = \`\${block.category} · \${block.name}\`;
    select.append(option);
  }
  return section;
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

function valueDisplay(type, label, value) {
  return type === "word32" ? wordHtml(label, value) : scalarHtml(label, value);
}

function renderEvidence(target, block, inputs, out) {
  const left = block.inputs.map((def) => valueDisplay(def.type, def.key, inputs[def.key])).join("");
  const outputType = block.outputs?.[0]?.type || "word32";
  const outputName = block.outputs?.[0]?.key || "Result";
  const flags = FLAGS.map(([name, mask]) =>
    \`<span class="lab-flag">\${flagState(name, mask, out.defined, out.flagValues, out.undefined, out.preserve)}</span>\`
  ).join("");

  target.innerHTML = \`
    <div class="lab-flow">
      <div class="lab-side"><h3>INPUT</h3>\${left}</div>
      <div class="lab-arrow">→</div>
      <div class="lab-side">
        <h3>OUTPUT</h3>
        \${valueDisplay(outputType, outputName, out.value)}
        <div class="lab-word"><strong>WriteBack</strong><code>\${out.writeback}</code><span>\${out.writeback ? "destination update enabled" : "computed value is flags-only / non-writeback"}</span></div>
      </div>
    </div>
    <div class="lab-flags">\${flags}</div>
    <div class="lab-metrics">
      <div class="lab-metric"><small>Structural reactions</small><strong>\${out.reactions}</strong></div>
      <div class="lab-metric"><small>Final quiescence</small><strong>\${out.quiescent ? "YES" : "NO"}</strong></div>
      <div class="lab-metric"><small>Links build → first result</small><strong>\${out.linksBuild} → \${out.linksFirst}</strong></div>
      <div class="lab-metric"><small>Identical rerun Link growth</small><strong>\${out.steadyDelta}</strong></div>
    </div>
    <details class="lab-trace">
      <summary>Reaction evidence</summary>
      <p>\${out.reactions} active Structural Rule reactions completed. The A-Circuit runner asserts exactly one rule match, one transition and one handoff on every active step, then requires a zero-match final quiescent reaction.</p>
      <p>Repeated identical execution produced <strong>\${out.steadyDelta}</strong> additional Links.</p>
    </details>\`;
}

function setDefinition(section, block) {
  section.querySelector("#lab-definition").innerHTML =
    \`<strong>\${block.name}</strong> · <code>\${block.formula}</code> · opcode \${block.opcode}\`;
}

function renderSingle(section, block, wasm) {
  const workspace = section.querySelector("#lab-workspace");
  workspace.innerHTML = \`
    <div class="lab-controls" id="lab-single-inputs"></div>
    <div class="lab-vector-actions"><button class="lab-run" id="lab-run-single">Run in A-memory</button></div>
    <div id="lab-single-result"></div>\`;

  const controls = workspace.querySelector("#lab-single-inputs");
  for (const def of block.inputs) {
    const field = document.createElement("div");
    field.className = "lab-field";
    field.innerHTML = \`<label>\${def.key} · \${def.type}</label>\${inputControl(def, def.default)}\`;
    controls.append(field);
  }

  workspace.querySelector("#lab-run-single").addEventListener("click", () => {
    const status = section.querySelector("#lab-status");
    try {
      const values = readInputs(controls, block);
      status.textContent = \`Executing \${block.name} structurally…\`;
      const out = runBlock(wasm, block, values);
      renderEvidence(workspace.querySelector("#lab-single-result"), block, values, out);
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
  if (vector.expect.writeback !== undefined) checks.push(out.writeback === Number(vector.expect.writeback));
  const ok = checks.every(Boolean);
  return { label: ok ? "PASS" : "FAIL", ok };
}

function vectorRowHtml(block, vector, index, custom = false) {
  const cells = block.inputs.map((def) => {
    const value = vector.inputs?.[def.key] ?? def.default;
    return \`<td>\${inputControl(def, value, true)}</td>\`;
  }).join("");
  return \`<tr data-vector-row="\${index}">
    <td><input data-vector-name value="\${String(vector.name || "custom").replaceAll('"', "&quot;")}"></td>
    \${cells}
    <td data-vector-result>—</td>
    <td><button class="lab-small" data-run-row>Run</button></td>
  </tr>\`;
}

function renderVectors(section, block, wasm) {
  const workspace = section.querySelector("#lab-workspace");
  const vectors = (block.vectors || []).map((v) => JSON.parse(JSON.stringify(v)));
  const headers = block.inputs.map((x) => \`<th>\${x.key}</th>\`).join("");

  workspace.innerHTML = \`
    <div class="lab-vector-actions">
      <button class="lab-run" id="lab-run-all">Run all</button>
      <button class="lab-small" id="lab-add-row">Add custom row</button>
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
      const vector = Number.isInteger(vectorIndex) && vectorIndex < vectors.length ? vectors[vectorIndex] : {};
      const values = readInputs(row, block);
      const out = runBlock(wasm, block, values);
      const verdict = vectorExpectation(out, vector);
      const cell = row.querySelector("[data-vector-result]");
      cell.innerHTML = \`<strong class="\${verdict.ok === false ? "lab-error" : "lab-ok"}">\${verdict.label}</strong><br><code>\${block.outputs?.[0]?.type === "bit" ? out.value : hex32(out.value)}</code><br><small>\${out.reactions} reactions · Q=\${out.quiescent} · ΔLinks=\${out.steadyDelta}</small>\`;
      status.textContent = \`\${block.name}: vector executed in A-memory.\`;
      status.className = "notice";
    } catch (error) {
      row.querySelector("[data-vector-result]").innerHTML = \`<span class="lab-error">\${error.message}</span>\`;
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
    row.removeAttribute("data-vector-row");
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
  const headers = keys.map((x) => \`<th>\${x}</th>\`).join("");
  workspace.innerHTML = \`
    <div class="lab-vector-actions"><button class="lab-run" id="lab-run-sweep">Run exhaustive sweep (\${cases.length})</button></div>
    <div class="lab-table-wrap"><table class="lab-table">
      <thead><tr>\${headers}<th>Output</th><th>Evidence</th></tr></thead>
      <tbody id="lab-sweep-body">
        \${cases.map((values,i)=>\`<tr data-sweep="\${i}">\${keys.map(k=>\`<td><code>\${values[k]}</code></td>\`).join("")}<td>—</td><td>—</td></tr>\`).join("")}
      </tbody>
    </table></div>\`;

  workspace.querySelector("#lab-run-sweep").addEventListener("click", () => {
    const rows = [...workspace.querySelectorAll("[data-sweep]")];
    for (const row of rows) {
      const values = cases[Number(row.dataset.sweep)];
      const out = runBlock(wasm, block, values);
      row.children[keys.length].innerHTML = \`<strong>\${out.value}</strong>\`;
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
  let currentMode = modes[0];

  const selectMode = (mode) => {
    currentMode = mode;
    [...tabs.querySelectorAll("[data-mode]")].forEach((button) => {
      button.setAttribute("aria-selected", String(button.dataset.mode === mode));
    });
    renderMode(section, block, wasm, mode);
  };

  for (const mode of modes) {
    const button = document.createElement("button");
    button.className = "lab-tab";
    button.dataset.mode = mode;
    button.textContent = mode === "single" ? "Single" : mode === "vectors" ? "Vectors" : "Sweep";
    button.addEventListener("click", () => selectMode(mode));
    tabs.append(button);
  }

  section.querySelector("#lab-status").textContent = \`\${block.name} ready. Edit inputs and run the real A-memory block.\`;
  section.querySelector("#lab-status").className = "notice";
  selectMode(currentMode);
}

async function main() {
  const [registry, wasm] = await Promise.all([loadRegistry(), loadLabWasm()]);
  const section = buildLab(registry);
  if (!section) return;

  const byId = new Map(registry.blocks.map((block) => [block.id, block]));
  const select = section.querySelector("#lab-block");
  const refresh = () => configureBlock(section, byId.get(select.value), wasm);
  select.addEventListener("change", refresh);

  for (const block of registry.blocks) {
    if (wasm.amemory_i386_lab_supports?.(block.opcode) !== 1) {
      throw new Error(\`registry entry \${block.id} has no WASM implementation\`);
    }
  }

  refresh();
}

main().catch((error) => {
  const main = document.querySelector("main");
  const node = document.createElement("div");
  node.className = "notice lab-error";
  node.textContent = \`80386 lab failed closed: \${error.message}\`;
  main?.prepend(node);
});
