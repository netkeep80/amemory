export const FLAGS = [
  ["CF", 1 << 0],
  ["PF", 1 << 2],
  ["AF", 1 << 4],
  ["ZF", 1 << 6],
  ["SF", 1 << 7],
  ["OF", 1 << 11],
];

export function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

export function hex32(value) {
  return "0x" + (value >>> 0).toString(16).padStart(8, "0");
}

export function bin32(value) {
  return (value >>> 0).toString(2).padStart(32, "0").match(/.{1,4}/g).join(" ");
}

export function signed32(value) {
  return (value | 0).toString(10);
}

export function valueHtml(type, label, value, className = "lab-word") {
  const safeLabel = escapeHtml(label);
  if (type === "word32") {
    return \`
      <div class="\${className}">
        <strong>\${safeLabel}</strong>
        <code>\${hex32(value)}</code>
        <span>unsigned \${value >>> 0}</span>
        <span>signed \${signed32(value)}</span>
        <code class="lab-binary">\${bin32(value)}</code>
      </div>\`;
  }
  return \`
    <div class="\${className}">
      <strong>\${safeLabel}</strong>
      <code>\${escapeHtml(value)}</code>
    </div>\`;
}

export function flagState(name, mask, out) {
  if (out.defined & mask) return \`\${name}=SET(\${out.flagValues & mask ? 1 : 0})\`;
  if (out.undefined & mask) return \`\${name}=UNDEFINED\`;
  if (out.preserve & mask) return \`\${name}=PRESERVE\`;
  return \`\${name}=—\`;
}

export function outputValue(out, def) {
  if (def?.slot === "valueHi") return out.valueHi >>> 0;
  if (!def?.slot || def.slot === "value") return out.value >>> 0;
  throw new Error(\`unsupported output ABI slot \${def.slot}\`);
}

export function outputHtml(block, out) {
  const outputs = block.outputs?.length
    ? block.outputs
    : [{ key: "Result", type: "word32", slot: "value" }];

  const cards = outputs.map((def) =>
    valueHtml(def.type, def.key, outputValue(out, def), "lab-port-card lab-output-port lab-word")
  ).join("");

  const lo = outputs.find((def) => !def.slot || def.slot === "value");
  const hi = outputs.find((def) => def.slot === "valueHi");
  const combined = lo?.type === "word32" && hi?.type === "word32"
    ? \`<div class="lab-port-card lab-output-port lab-word">
        <strong>Combined Hi:Lo</strong>
        <code>\${hex32(outputValue(out, hi))}:\${hex32(outputValue(out, lo))}</code>
        <span>two exact u32 halves; never transported as one JavaScript Number</span>
      </div>\`
    : "";

  return cards + combined;
}

export function blockDefinitionHtml(block) {
  const inputs = (block.inputs || []).map((def) =>
    \`<code>\${escapeHtml(def.key)} : \${escapeHtml(def.type)} → ABI \${escapeHtml(def.slot || "—")}</code>\`
  ).join("");
  const outputs = (block.outputs || []).map((def) =>
    \`<code>\${escapeHtml(def.key)} : \${escapeHtml(def.type)} ← ABI \${escapeHtml(def.slot || "value")}</code>\`
  ).join("");
  const modes = (block.modes || ["single"]).map((mode) =>
    \`<span class="lab-pill">\${escapeHtml(mode)}</span>\`
  ).join("");

  return \`
    <div class="lab-definition-head">
      <div>
        <h3>\${escapeHtml(block.name)}</h3>
        <code>\${escapeHtml(block.formula)}</code>
      </div>
      <div class="lab-badges">
        <span class="lab-pill">\${escapeHtml(block.category)}</span>
        <span class="lab-pill">opcode \${block.opcode}</span>
        \${modes}
      </div>
    </div>
    <div class="lab-spec-grid">
      <div class="lab-spec"><strong>INPUT ABI</strong>\${inputs || "<span>none</span>"}</div>
      <div class="lab-spec"><strong>OUTPUT ABI</strong>\${outputs || "<span>none</span>"}</div>
    </div>\`;
}

export function catalogCardHtml(block) {
  return \`
    <button class="lab-block-card" data-block-id="\${escapeHtml(block.id)}" type="button">
      <span class="lab-card-top">
        <strong>\${escapeHtml(block.name)}</strong>
        <span class="lab-card-category">\${escapeHtml(block.category)}</span>
      </span>
      <code>\${escapeHtml(block.formula)}</code>
      <small>opcode \${block.opcode} · \${(block.inputs || []).length} in · \${(block.outputs || []).length} out</small>
    </button>\`;
}

export function inputPreviewHtml(def, value) {
  if (def.type === "word32") {
    const n = Number(value) >>> 0;
    return \`<div class="lab-input-preview">
      <code>\${hex32(n)}</code>
      <span>u=\${n} · s=\${signed32(n)}</span>
      <code>\${bin32(n)}</code>
    </div>\`;
  }
  return \`<div class="lab-input-preview"><code>\${escapeHtml(value)}</code></div>\`;
}

export function evidenceHtml(block, inputs, out) {
  const flags = FLAGS.map(([name, mask]) =>
    \`<span class="lab-flag">\${flagState(name, mask, out)}</span>\`
  ).join("");

  const inputSummary = (block.inputs || []).map((def) =>
    valueHtml(def.type, def.key, inputs[def.key], "lab-port-card lab-input-port lab-word")
  ).join("");

  return \`
    <div class="lab-evidence-io">
      <div>
        <div class="lab-stage-label">INPUT snapshot</div>
        <div class="lab-port-stack">\${inputSummary}</div>
      </div>
      <div>
        <div class="lab-stage-label">OUTPUT</div>
        <div class="lab-port-stack">\${outputHtml(block, out)}</div>
      </div>
    </div>
    <div class="lab-flags">\${flags}</div>
    <div class="lab-metrics">
      <div class="lab-metric"><small>WriteBack</small><strong>\${out.writeback}</strong></div>
      <div class="lab-metric"><small>Structural reactions</small><strong>\${out.reactions}</strong></div>
      <div class="lab-metric"><small>Final quiescence</small><strong>\${out.quiescent ? "YES" : "NO"}</strong></div>
      <div class="lab-metric"><small>Links build → first result</small><strong>\${out.linksBuild} → \${out.linksFirst}</strong></div>
      <div class="lab-metric"><small>Identical rerun Link growth</small><strong>\${out.steadyDelta}</strong></div>
    </div>
    <details class="lab-trace">
      <summary>Reaction evidence</summary>
      <p>\${out.reactions} active Structural Rule reactions completed. The A-Circuit runner requires exactly one rule match, one transition and one handoff on every active step, then a zero-match quiescent reaction.</p>
      <p>Repeated identical execution produced <strong>\${out.steadyDelta}</strong> additional Links.</p>
    </details>\`;
}
