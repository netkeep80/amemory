const OPS = [
  { id: 1, name: "AND32", formula: "A & B", b: true, flag: false },
  { id: 2, name: "OR32", formula: "A | B", b: true, flag: false },
  { id: 3, name: "XOR32", formula: "A ^ B", b: true, flag: false },
  { id: 4, name: "NOT32", formula: "~A", b: false, flag: false },
  { id: 5, name: "TEST32", formula: "A & B (flags only)", b: true, flag: false },
  { id: 6, name: "ADD32", formula: "A + B", b: true, flag: false },
  { id: 7, name: "ADC32", formula: "A + B + CF", b: true, flag: true, flagLabel: "CF in" },
  { id: 8, name: "SUB32", formula: "A - B", b: true, flag: false },
  { id: 9, name: "SBB32", formula: "A - B - CF", b: true, flag: true, flagLabel: "CF / borrow in" },
  { id: 10, name: "CMP32", formula: "A - B (flags only)", b: true, flag: false },
  { id: 11, name: "MUX32", formula: "S ? B : A", b: true, flag: true, flagLabel: "Select S" },
];

const FLAGS = [
  ["CF", 1 << 0],
  ["PF", 1 << 2],
  ["AF", 1 << 4],
  ["ZF", 1 << 6],
  ["SF", 1 << 7],
  ["OF", 1 << 11],
];

function parseWord(text) {
  const value = text.trim();
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

function hex32(value) {
  return "0x" + (value >>> 0).toString(16).padStart(8, "0");
}

function bin32(value) {
  const bits = (value >>> 0).toString(2).padStart(32, "0");
  return bits.match(/.{1,4}/g).join(" ");
}

function signed32(value) {
  return (value | 0).toString(10);
}

function wordHtml(label, value) {
  return `
    <div class="lab-word">
      <strong>${label}</strong>
      <code>${hex32(value)}</code>
      <span>unsigned ${value >>> 0}</span>
      <span>signed ${signed32(value)}</span>
      <code class="lab-binary">${bin32(value)}</code>
    </div>`;
}

function flagState(name, mask, defined, values, undefined, preserve) {
  if (defined & mask) return `${name}=${values & mask ? 1 : 0}`;
  if (undefined & mask) return `${name}=UNDEFINED`;
  if (preserve & mask) return `${name}=PRESERVE`;
  return `${name}=—`;
}

async function loadLabWasm() {
  const response = await fetch("./amemory_a_circuit.wasm", { cache: "no-store" });
  if (!response.ok) throw new Error(`A-Circuit WASM HTTP ${response.status}`);
  const bytes = await response.arrayBuffer();
  const { instance } = await WebAssembly.instantiate(bytes, {});
  if (instance.exports.amemory_i386_lab_probe?.() !== 0x386) {
    throw new Error("unexpected A-Circuit WASM probe");
  }
  return instance.exports;
}

function buildLab() {
  const main = document.querySelector("main");
  if (!main) return null;

  const style = document.createElement("style");
  style.textContent = `
    .i386-lab { margin: 36px 0; }
    .lab-shell { background: var(--surface); border: 1px solid var(--line); border-radius: 16px; padding: 18px; box-shadow: var(--shadow); }
    .lab-controls { display: grid; grid-template-columns: 1.25fr 1fr 1fr .7fr auto; gap: 12px; align-items: end; }
    .lab-field { display: grid; gap: 6px; }
    .lab-field label { color: var(--muted); font-size: .8rem; }
    .lab-field input, .lab-field select, .lab-run {
      min-height: 42px; border-radius: 10px; border: 1px solid var(--line); background: var(--surface-2); color: var(--text); padding: 9px 11px;
    }
    .lab-run { cursor: pointer; font-weight: 760; padding-inline: 18px; }
    .lab-formula { margin: 14px 0; padding: 11px 13px; border-radius: 10px; background: var(--surface-2); font-family: ui-monospace, SFMono-Regular, Consolas, monospace; }
    .lab-flow { display: grid; grid-template-columns: 1fr auto 1fr; gap: 16px; align-items: stretch; margin-top: 16px; }
    .lab-side { display: grid; gap: 10px; }
    .lab-arrow { align-self: center; color: var(--muted); font-size: 2rem; font-weight: 800; }
    .lab-word { display: grid; gap: 4px; padding: 12px; border: 1px solid var(--line); border-radius: 12px; background: var(--surface-2); }
    .lab-word span { color: var(--muted); font-size: .84rem; }
    .lab-binary { overflow-wrap: anywhere; font-size: .8rem; }
    .lab-flags { display: flex; flex-wrap: wrap; gap: 7px; margin-top: 12px; }
    .lab-flag { padding: 6px 8px; border-radius: 8px; background: var(--surface-2); font-family: ui-monospace, SFMono-Regular, Consolas, monospace; font-size: .82rem; }
    .lab-metrics { display: grid; grid-template-columns: repeat(4,minmax(0,1fr)); gap: 10px; margin-top: 12px; }
    .lab-metric { padding: 10px 12px; border: 1px solid var(--line); border-radius: 10px; }
    .lab-metric small { display:block; color: var(--muted); }
    .lab-error { color: var(--bad); font-weight: 700; }
    @media(max-width:900px){ .lab-controls{grid-template-columns:1fr 1fr;} .lab-flow{grid-template-columns:1fr;} .lab-arrow{display:none;} .lab-metrics{grid-template-columns:1fr 1fr;} }
    @media(max-width:520px){ .lab-controls,.lab-metrics{grid-template-columns:1fr;} }
  `;
  document.head.append(style);

  const section = document.createElement("section");
  section.className = "i386-lab";
  section.innerHTML = `
    <h2>80386 structural logic lab</h2>
    <p>Runs the real <code>amemory-a-circuit</code> structural program compiled to WASM. JavaScript only supplies inputs and formats the returned witness.</p>
    <div class="lab-shell">
      <div class="lab-controls">
        <div class="lab-field"><label>Block</label><select id="lab-op"></select></div>
        <div class="lab-field"><label>A (hex or uint32)</label><input id="lab-a" value="0x12345678"></div>
        <div class="lab-field" id="lab-b-field"><label>B (hex or uint32)</label><input id="lab-b" value="0x0f0f00ff"></div>
        <div class="lab-field" id="lab-flag-field"><label id="lab-flag-label">Input bit</label><select id="lab-flag"><option value="0">0</option><option value="1">1</option></select></div>
        <button class="lab-run" id="lab-run">Run in A-memory</button>
      </div>
      <div class="lab-formula" id="lab-formula">—</div>
      <div id="lab-status" class="notice">Loading A-Circuit WASM…</div>
      <div id="lab-result"></div>
    </div>`;

  const raw = main.querySelector(".panel.raw");
  if (raw) main.insertBefore(section, raw);
  else main.append(section);

  const op = section.querySelector("#lab-op");
  for (const item of OPS) {
    const option = document.createElement("option");
    option.value = String(item.id);
    option.textContent = item.name;
    op.append(option);
  }
  return section;
}

function currentOp(section) {
  return OPS.find((x) => x.id === Number(section.querySelector("#lab-op").value)) || OPS[0];
}

function refreshControls(section) {
  const item = currentOp(section);
  section.querySelector("#lab-b-field").style.display = item.b ? "grid" : "none";
  section.querySelector("#lab-flag-field").style.display = item.flag ? "grid" : "none";
  section.querySelector("#lab-flag-label").textContent = item.flagLabel || "Input bit";
  section.querySelector("#lab-formula").textContent = `${item.name}: ${item.formula}`;
}

function renderResult(section, wasm, item, a, b, inputFlag) {
  const value = wasm.amemory_i386_lab_value() >>> 0;
  const writeback = wasm.amemory_i386_lab_writeback() >>> 0;
  const defined = wasm.amemory_i386_lab_defined_mask() >>> 0;
  const values = wasm.amemory_i386_lab_value_mask() >>> 0;
  const undefined = wasm.amemory_i386_lab_undefined_mask() >>> 0;
  const preserve = wasm.amemory_i386_lab_preserve_mask() >>> 0;
  const reactions = wasm.amemory_i386_lab_reactions() >>> 0;
  const linksBuild = wasm.amemory_i386_lab_links_after_build() >>> 0;
  const linksFirst = wasm.amemory_i386_lab_links_after_first() >>> 0;
  const steadyDelta = wasm.amemory_i386_lab_steady_link_delta() >>> 0;
  const quiescent = wasm.amemory_i386_lab_quiescent() >>> 0;

  const inputs = [wordHtml("A", a)];
  if (item.b) inputs.push(wordHtml("B", b));
  if (item.flag) {
    inputs.push(`<div class="lab-word"><strong>${item.flagLabel}</strong><code>${inputFlag}</code></div>`);
  }

  section.querySelector("#lab-result").innerHTML = `
    <div class="lab-flow">
      <div class="lab-side">${inputs.join("")}</div>
      <div class="lab-arrow">→</div>
      <div class="lab-side">
        ${wordHtml(writeback ? "Result / writeback value" : "Computed value / no writeback", value)}
        <div class="lab-word"><strong>WriteBack</strong><code>${writeback}</code><span>${writeback ? "architectural destination would be updated" : "flags-only operation"}</span></div>
      </div>
    </div>
    <div class="lab-flags">
      ${FLAGS.map(([name, mask]) => `<span class="lab-flag">${flagState(name, mask, defined, values, undefined, preserve)}</span>`).join("")}
    </div>
    <div class="lab-metrics">
      <div class="lab-metric"><small>Structural reactions</small><strong>${reactions}</strong></div>
      <div class="lab-metric"><small>Final quiescence</small><strong>${quiescent ? "YES" : "NO"}</strong></div>
      <div class="lab-metric"><small>Links build → first result</small><strong>${linksBuild} → ${linksFirst}</strong></div>
      <div class="lab-metric"><small>Identical rerun Link growth</small><strong>${steadyDelta}</strong></div>
    </div>`;
}

async function main() {
  const section = buildLab();
  if (!section) return;
  refreshControls(section);
  section.querySelector("#lab-op").addEventListener("change", () => refreshControls(section));

  const status = section.querySelector("#lab-status");
  let wasm;
  try {
    wasm = await loadLabWasm();
    status.textContent = "A-Circuit WASM loaded. Ready to execute structural blocks.";
    status.classList.add("ok");
  } catch (error) {
    status.textContent = `A-Circuit WASM failed: ${error.message}`;
    status.classList.add("lab-error");
    return;
  }

  section.querySelector("#lab-run").addEventListener("click", () => {
    try {
      const item = currentOp(section);
      const a = parseWord(section.querySelector("#lab-a").value);
      const b = item.b ? parseWord(section.querySelector("#lab-b").value) : 0;
      const inputFlag = item.flag ? Number(section.querySelector("#lab-flag").value) : 0;
      status.textContent = `Executing ${item.name} structurally…`;
      const ok = wasm.amemory_i386_lab_run(item.id, a, b, inputFlag);
      if (ok !== 1) throw new Error("A-Circuit rejected operation/input");
      renderResult(section, wasm, item, a, b, inputFlag);
      status.textContent = `${item.name} complete: structural result returned by WASM/A-memory.`;
    } catch (error) {
      status.textContent = error.message;
      status.classList.add("lab-error");
    }
  });
}

main();
