import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";

import {
  proofPipelineHtml,
  recursiveStructureHtml,
} from "../../browser-accelerator/web/proof-view.mjs";

const root = process.cwd();

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
]) {
  if (!workbenchSource.includes(required)) {
    throw new Error("live WebGPU Workbench witness missing: " + required);
  }
}
for (const forbidden of [
  "scopeAfter: expected.",
  "publishedHandle: expected.",
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
