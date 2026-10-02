import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";

import {
  proofPipelineHtml,
  recursiveStructureHtml,
} from "../../browser-accelerator/web/proof-view.mjs";
import { gpuCarrierReactionShaderSource } from "../../browser-accelerator/web/gpu-carrier.mjs";

const root = process.cwd();

const gpuCarrierSource = fs.readFileSync(
  path.join(root, "experiments/browser-accelerator/web/gpu-carrier.mjs"),
  "utf8",
);
for (const required of [
  "const discoverEncoder = device.createCommandEncoder()",
  "const publishEncoder = device.createCommandEncoder()",
  "Diagnostic readback only",
  "WebGPU DISCOVER failed closed with status",
  "candidates = candidates + 1u",
  "discovery[42] = candidates",
  "let trace = 43u + (candidates - 1u) * 2u",
  "new Uint32Array(67)",
  "readLookupWords(device, discovery, 67)",
  "cpuDiscovery = discoverCarrierReaction(parsed, input)",
  "These CPU facts never seed, filter or retry GPU execution",
  "cpuRuleGpuDiagnostic",
  "cpuRuleGpuRoles",
]) {
  if (!gpuCarrierSource.includes(required)) {
    throw new Error("C4c3 split GPU submission contract missing: " + required);
  }
}

const workbenchSource = fs.readFileSync(
  path.join(root, "experiments/browser-accelerator/web/workbench.mjs"),
  "utf8",
);
for (const required of [
  "runWorkbenchWebGpuWitness",
  "Проверить WebGPU C4c3",
  "CPU = WebGPU = Rust/WASM",
  "webgpuWitness",
  "amemory_i386_lab_gpu_carrier_prepare",
  "sameU32Prefix",
  "expected.length >= prefixLength",
  "proofAppendCount === observed.appendCount",
  "gpuAppendCount=",
]) {
  if (!workbenchSource.includes(required)) {
    throw new Error("live WebGPU Workbench witness missing: " + required);
  }
}
for (const forbidden of [
  "scopeAfter: expected.",
  "publishedHandle: expected.",
  "sameU32Array",
]) {
  if (workbenchSource.includes(forbidden)) {
    throw new Error("Workbench WebGPU witness seeded expected result: " + forbidden);
  }
}

const pagesWebGpuSmokeSource = fs.readFileSync(
  path.join(root, "experiments/browser-accelerator/tests/pages-webgpu-live-smoke.mjs"),
  "utf8",
);
for (const required of [
  "--enable-unsafe-webgpu",
  "--use-webgpu-adapter=swiftshader",
]) {
  if (!pagesWebGpuSmokeSource.includes(required)) {
    throw new Error("compute-only WebGPU launch flag missing: " + required);
  }
}
for (const forbidden of [
  "--use-angle=vulkan",
  "--use-vulkan=swiftshader",
  "--enable-features=Vulkan",
]) {
  if (pagesWebGpuSmokeSource.includes(forbidden)) {
    throw new Error("C4c3 compute smoke regained presentation Vulkan: " + forbidden);
  }
}

for (const required of [
  "async function stopChrome()",
  'chrome.kill("SIGTERM")',
  'chrome.kill("SIGKILL")',
  "maxRetries: 10",
  "retryDelay: 100",
  "C4c3 cleanup warning",
]) {
  if (!pagesWebGpuSmokeSource.includes(required)) {
    throw new Error("C4c3 bounded cleanup contract missing: " + required);
  }
}
if (pagesWebGpuSmokeSource.includes(
  "fs.rmSync(profile, { recursive: true, force: true });",
)) {
  throw new Error("C4c3 smoke regained immediate profile removal race");
}

for (const mode of ["single", "sections"]) {
  const shader = gpuCarrierReactionShaderSource(mode, {
    currentHandle: 1,
    interpreterHandle: 1,
    linkCount: 2,
    rootHandle: 1,
  });
  if (/\bactive\b/.test(shader) || !shader.includes("active_handle")) {
    throw new Error("C4c3 generated WGSL uses reserved identifier active");
  }
  if (shader.includes("discovery[17] = candidates") || shader.includes("let trace = 18u +")) {
    throw new Error("C4c3 discovery diagnostics alias authoritative role bindings");
  }
  for (const marker of [
    "var role_left = 0u",
    "role_count - role_left - 1u",
    "roles[role_left] = roles[role_right]",
    "discovery[0] = 10u",
    "discovery[9] = 20u",
    "discovery[9] = 27u",
    "discovery[10] = t",
    "discovery[11] = c",
    "discovery[16] = roles[0]",
    "fn discover_rule(rule: u32, active_handle: u32, record_diag: bool)",
    "const MAX_VISITS: u32 = 256u",
    "var seen_t: array<u32, 256>",
    "var seen_c: array<u32, 256>",
    "seen_t[si] == t && seen_c[si] == c",
    "if (already_seen) { continue; }",
    "discovery[9] = 32u",
    "let record_diag = matches == 0u",
    "candidates <= 12u",
    "discovery[42] = candidates",
    "let trace = 43u + (candidates - 1u) * 2u",
    "(discovery[8] << 16u) | (discovery[9] & 0xffffu)",
    "discovery[9] = 30u",
  ]) {
    if (!shader.includes(marker)) {
      throw new Error("C4c3 WGSL lost canonical role order: " + marker);
    }
  }
}

const syntaxTargets = [
  "experiments/browser-accelerator/web/workbench.mjs",
  "experiments/browser-accelerator/web/proof-view.mjs",
  "experiments/browser-accelerator/web/i386-proof-transport.mjs",
  "experiments/browser-accelerator/web/i386-wasm-json.mjs",
  "experiments/browser-accelerator/web/gpu-carrier.mjs",
  "experiments/browser-accelerator/web/scenario-transport.mjs",
  "experiments/browser-accelerator/web/scenario-presets.mjs",
  "experiments/browser-accelerator/web/scenario-worker.mjs",
  "experiments/browser-accelerator/web/scenario-worker-client.mjs",
  "experiments/a-circuit/tests/workbench-worker-browser-e2e.mjs",
  "experiments/browser-accelerator/tests/pages-webgpu-live-smoke.mjs",
];

for (const target of syntaxTargets) {
  const checked = spawnSync(
    process.execPath,
    ["--check", path.join(root, target)],
    { stdio: "inherit" },
  );
  if (checked.status !== 0) {
    throw new Error("syntax check failed: " + target);
  }
}

const collapsed = recursiveStructureHtml("1".repeat(4096));
if (!collapsed.includes('class="proof-recursive-structure"') ||
    !collapsed.includes("<details") ||
    !collapsed.includes("4096 символов")) {
  throw new Error("collapsed recursive-structure renderer witness failed");
}

const proofFixture = {
  prepare: {
    compilerLabel: "compiler only",
    runtimeMemoryExists: false,
    compiledLinks: 12,
    carrierDuplets: [{ start: 1, end: 1 }, { start: 2, end: 1 }],
    semanticRoots: [
      { role: "function.mux1", source: "98" },
      { role: "execution.interpreter", source: "68" },
      { role: "invocation.call", source: "19868" },
    ],
    theoryAdmissions: ["L8"],
  },
  load: {
    memoryInstanceId: "A-memory#7",
    linksBeforeLoad: 1,
    linksAfterLoad: 12,
    importedDuplets: 2,
    carrierRoundTrip: true,
    semanticRoots: [
      { role: "function.mux1", source: "98", localHandle: 2 },
    ],
  },
  execute: {
    memoryInstanceId: "A-memory#7",
    activeReactionCount: 1,
    finalQuiescent: true,
    reactions: [{
      memoryInstanceId: "A-memory#7",
      step: 0,
      scopeBefore: ["19868"],
      rawRuleMatches: 1,
      transitionedMembers: 1,
      handoffCount: 1,
      scopeAfter: ["16898"],
      linksAfter: 12,
      quiescent: false,
    }],
  },
  result: {
    memoryInstanceId: "A-memory#7",
    resultRecursiveWire: "16898",
    resultSequenceAnum: "98",
    decodedValue: 1,
    oracleValue: 1,
    oracleMatches: true,
    linksFinal: 12,
    identicalRerunLinkDelta: 0,
    visualLinks: [{
      key: "A-memory#7:L1",
      startKey: "A-memory#7:L1",
      endKey: "A-memory#7:L1",
      localHandle: 1,
      label: "ROOT",
      tags: [],
    }],
  },
};

const proofHtml = proofPipelineHtml(proofFixture);
for (const required of [
  "ПОДГОТОВКА АСЕТИ",
  "ЗАГРУЗКА В АПАМЯТЬ",
  "ИСПОЛНЕНИЕ В ТОЙ ЖЕ АПАМЯТИ",
  "РЕЗУЛЬТАТ И ВИЗУАЛИЗАЦИЯ",
  "Рекурсивная запись связи результата",
  "ОДНА ИСПОЛНЯЕМАЯ АПАМЯТЬ",
  "A-memory#7",
  "Схема 2D",
  "Статическая 3D",
  "Живая физика 3D",
  "@mts/visual 0.5.0",
  "Octahedral Link3D",
  "Структурные правила, допущенные в теорию",
]) {
  if (!proofHtml.includes(required)) {
    throw new Error("proof browser renderer missing: " + required);
  }
}

const version = fs.readFileSync(
  path.join(root, "VERSION"),
  "utf8",
).trim();
const info = {
  schemaVersion: 2,
  version,
  mainSha: "a".repeat(40),
  acceptanceRunId: 1,
  builtAt: "2026-01-01T00:00:00Z",
};
const nodes = new Map([
  ["#amemory-version", { textContent: "", title: "" }],
  ["#browser-lab-sha", { textContent: "", title: "" }],
  ["#amemory-build", { textContent: "", title: "" }],
]);

globalThis.document = {
  querySelector(selector) {
    const node = nodes.get(selector);
    if (!node) throw new Error("unexpected selector " + selector);
    return node;
  },
};
globalThis.fetch = async (url, options) => {
  if (url !== "./build-info.json" || options?.cache !== "no-store") {
    throw new Error("unexpected build-info fetch contract");
  }
  return {
    ok: true,
    status: 200,
    async json() {
      return info;
    },
  };
};

const buildInfoSource = path.join(
  root,
  "experiments/browser-accelerator/web/build-info.js",
);
const buildInfoModule = path.join(
  os.tmpdir(),
  "amemory-build-info-" + process.pid + ".mjs",
);
fs.copyFileSync(buildInfoSource, buildInfoModule);
try {
  await import(
    pathToFileURL(buildInfoModule).href +
      "?acceptance=" + info.mainSha
  );
  await new Promise((resolve) => setTimeout(resolve, 0));
} finally {
  fs.rmSync(buildInfoModule, { force: true });
}

const versionText = nodes.get("#amemory-version").textContent;
const shaText = nodes.get("#browser-lab-sha").textContent;
if (versionText !== "v" + version ||
    shaText !== info.mainSha.slice(0, 12) ||
    /unavailable|unknown/i.test(versionText + " " + shaText)) {
  throw new Error("schema-v2 build identity fell back in browser consumer");
}

console.log(
  "WORKBENCH_BROWSER_ACCEPTANCE=PASS " +
  "syntax=" + syntaxTargets.length +
  " renderer=PASS buildIdentity=PASS",
);
