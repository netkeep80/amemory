import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";

import {
  proofPipelineHtml,
  recursiveStructureHtml,
} from "../../browser-accelerator/web/proof-view.mjs";
import {
  gpuCarrierReactionShaderSource,
  openGpuCarrierResidentSession,
} from "../../browser-accelerator/web/gpu-carrier.mjs";

import {
  METRIC_AVAILABILITY,
  measured,
  normalizeCpuScenarioRunV2,
  normalizeWebGpuResidentRunV2,
  unavailable,
  unsupported,
} from "../../browser-accelerator/web/run-observation.mjs";

import {
  workbenchBackendAvailability,
  workbenchDefaultBackend,
} from "../../browser-accelerator/web/workbench.mjs";

// Keep the generic resident configuration planner executable without changing
// CI governance. This module is assertions-only and performs no GPU execution.
await import("../../browser-accelerator/tests/gpu-carrier-configuration.test.mjs");

const multiBackendManifest = {
  supportedBackends: ["optimized-cpu", "webgpu", "linksdb"],
};
const cpuAvailability =
  workbenchBackendAvailability(multiBackendManifest, "optimized-cpu");
const gpuAvailability =
  workbenchBackendAvailability(multiBackendManifest, "webgpu");
const linksdbAvailability =
  workbenchBackendAvailability(multiBackendManifest, "linksdb");
if (!cpuAvailability.scenarioSupported ||
    !cpuAvailability.sessionHostSupported ||
    !cpuAvailability.executable ||
    !gpuAvailability.scenarioSupported ||
    gpuAvailability.sessionHostSupported ||
    gpuAvailability.executable ||
    !linksdbAvailability.scenarioSupported ||
    linksdbAvailability.sessionHostSupported ||
    linksdbAvailability.executable ||
    workbenchDefaultBackend(multiBackendManifest) !== "optimized-cpu") {
  throw new Error(
    "Workbench conflates Scenario backend support with Session-host availability",
  );
}

const metricZero = measured(0);
const metricUnavailable = unavailable("not measured");
const metricUnsupported = unsupported("not supported");
if (metricZero.availability !== METRIC_AVAILABILITY.MEASURED ||
    metricZero.value !== 0 ||
    metricUnavailable.availability !== METRIC_AVAILABILITY.UNAVAILABLE ||
    metricUnavailable.value !== null ||
    metricUnsupported.availability !== METRIC_AVAILABILITY.UNSUPPORTED ||
    metricUnsupported.value !== null) {
  throw new Error(
    "normalized metric availability conflates measured zero with absence",
  );
}

const normalizedCpuFixture = normalizeCpuScenarioRunV2({
  manifestRunId: "cpu-fixture",
  sessionRunId: 1,
  configurationReused: false,
  linksBeforeConfigure: 10,
  linksAfterConfigure: 12,
  result: { fields: { value: 1 } },
  observed: {
    sessionId: "cpu-session#fixture",
    runId: 1,
    backendId: "optimized-cpu",
    timingAvailable: false,
    finalScope: [20],
    activeReactionCount: 1,
    finalQuiescent: true,
    events: [{
      sequence: 0,
      elapsedNs: 0,
      stage: "EXECUTE",
      kind: "REACTION_END",
      reactionIndex: 0,
      linksAfter: 20,
      rawRuleMatches: 1,
      transitionedMembers: 1,
      handoffCount: 1,
      quiescent: false,
    }],
    profile: {
      timingAvailable: false,
      baseLinks: 10,
      linksAfterRun: 20,
      executeNs: 0,
      traceProjectionNs: 0,
      denseCarrierAllocatedBytes: 64,
      maxDenseCarrierBytes: 128,
      structural: {
        timingAvailable: false,
        triggerIncidenceCandidates: 3,
        totalNs: 0,
      },
    },
  },
  pipelineProfile: {
    linksAfterExecute: 20,
    stages: {
      timingAvailable: false,
      configureNs: 0,
      resultNs: 0,
    },
  },
});
if (normalizedCpuFixture.profile.timings.executeNs.availability !==
      METRIC_AVAILABILITY.UNAVAILABLE ||
    normalizedCpuFixture.profile.timings.executeNs.value !== null ||
    normalizedCpuFixture.events[0].elapsedNs.availability !==
      METRIC_AVAILABILITY.UNAVAILABLE ||
    normalizedCpuFixture.events[0].elapsedNs.value !== null ||
    normalizedCpuFixture.profile.resource.baseUploadCount.availability !==
      METRIC_AVAILABILITY.UNSUPPORTED ||
    normalizedCpuFixture.profile.resource.baseUploadCount.value !== null ||
    normalizedCpuFixture.profile.structural.triggerIncidenceCandidates
      ?.availability !== METRIC_AVAILABILITY.MEASURED ||
    normalizedCpuFixture.profile.structural.triggerIncidenceCandidates
      ?.value !== 3 ||
    normalizedCpuFixture.profile.structural.totalNs?.availability !==
      METRIC_AVAILABILITY.UNAVAILABLE ||
    normalizedCpuFixture.profile.structural.totalNs?.value !== null) {
  throw new Error(
    "CPU normalized projection serialized unavailable timing as a value",
  );
}

const normalizedGpuFixture = normalizeWebGpuResidentRunV2({
  sessionId: "webgpu-session#fixture",
  runId: 1,
  manifestRunId: "gpu-fixture",
  configurationReused: true,
  baseLinkCount: 10,
  linksBeforeConfigure: 10,
  linksAfterConfigure: 10,
  run: {
    stopReason: "APPENDED_LINKS_BUDGET_EXCEEDED",
    quiescent: false,
    activeReactionCount: 1,
    finalCurrentHandle: 11,
    steps: [{
      observed: {
        residentAppendCount: 1,
        rawRuleMatches: 1,
        appendCount: 1,
        currentHandle: 10,
        publishedHandle: 11,
        ruleHandle: 5,
        groundedBundle: 7,
        quiescent: false,
      },
    }],
  },
  telemetryBeforeConfigure: {
    configurationUploadBytes: 12,
  },
  telemetryAfterConfigure: {
    configurationUploadBytes: 12,
  },
  telemetryBeforeExecute: {
    reactionDispatchCount: 2,
  },
  telemetryAfterExecute: {
    reactionDispatchCount: 3,
    baseUploadCount: 1,
    baseUploadBytes: 100,
    residentBufferBytes: 1024,
    residentAppendCount: 1,
  },
  result: { fields: { value: 1 } },
});
if (normalizedGpuFixture.profile.stopReason !==
      "APPENDED_LINKS_BUDGET_EXCEEDED" ||
    normalizedGpuFixture.profile.finalQuiescent !== false ||
    normalizedGpuFixture.events.at(-1)?.kind !== "RUN_END" ||
    normalizedGpuFixture.profile.resource.configurationUploadBytes
      ?.availability !== METRIC_AVAILABILITY.MEASURED ||
    normalizedGpuFixture.profile.resource.configurationUploadBytes
      ?.value !== 0 ||
    normalizedGpuFixture.profile.resource.readbackBytes?.availability !==
      METRIC_AVAILABILITY.UNAVAILABLE ||
    normalizedGpuFixture.profile.resource.readbackBytes?.value !== null) {
  throw new Error(
    "WebGPU normalized projection lost stop reason or availability semantics",
  );
}

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
  "openGpuCarrierResidentSession",
  "discoverCarrierReactionMatches",
  "matches == 0u",
  "discovery[0] = 11u",
  "runToQuiescence",
  "REACTION_BUDGET_EXCEEDED",
  "APPENDED_LINKS_BUDGET_EXCEEDED",
  "AMBIGUOUS_RULE_MATCH",
  "sessionId: state.sessionId",
  "baseUploadCount: 1",
  "reactionDispatchCount",
  "resident.close()",
  "runGpuCarrierReactionOnResidentBase",
]) {
  if (!gpuCarrierSource.includes(required)) {
    throw new Error("C4c3 split GPU submission contract missing: " + required);
  }
}

const workbenchSource = fs.readFileSync(
  path.join(root, "experiments/browser-accelerator/web/workbench.mjs"),
  "utf8",
);

if (!gpuCarrierSource.includes("let slot = expected + i;") ||
    gpuCarrierSource.includes("let target = expected + i;")) {
  throw new Error(
    "resident configuration WGSL uses a reserved/unstable local identifier",
  );
}
for (const required of [
  "WORKBENCH_HOST_BACKENDS",
  "workbenchBackendAvailability",
  "sessionHostSupported",
  "недоступен как Workbench Session",
  "Обычный Workbench Session не подменяет их CPU",
]) {
  if (!workbenchSource.includes(required)) {
    throw new Error("Workbench host/backend truthfulness missing: " + required);
  }
}

if (!workbenchSource.includes("supportedBackends") ||
    workbenchSource.includes("supportedИсполнительs")) {
  throw new Error(
    "Workbench backend selector is not bound to canonical supportedBackends",
  );
}
for (const required of [
  "runWorkbenchWebGpuWitness",
  "supportedBackends",
  "Проверить WebGPU Scenario ×4",
  "WebGPU ↔ Rust/WASM после readback",
  "amemory_i386_lab_gpu_carrier_prepare",
  "GPU_WITNESS_RESULT_ABI",
  "refreshScenarioPresetRegistry",
  "loadScenarioPresetManifest",
  '"mux1-lifecycle"',
  '"1.0.0"',
  "mux1ResidentConfigurationRecipe",
  "resident.configure(recipe)",
  "resident.runToQuiescence",
  "decodeMux1ResidentResult",
  "executeScenarioManifest",
  "postReadbackCpuDifferential: true",
  "cpuReferenceSharesRustCore: true",
  "scenarioManifestDriven: true",
  "staticPreparedCarrier: true",
  "noExecutionSeed: true",
  "returnToFirstReuse",
  "configurationAppendCounts",
  "configurationCommitCount",
  "configurationDispatchCount",
  "baseUploadCount",
  "reactionDispatchCount",
  "normalizeWebGpuResidentRunV2",
  "normalizeCpuScenarioRunV2",
  "normalizedObservationSchemaVersion: 2",
  "normalizedGpuRuns",
  "normalizedCpuRuns",
]) {
  if (!workbenchSource.includes(required)) {
    throw new Error("live WebGPU Scenario witness missing: " + required);
  }
}
for (const forbidden of [
  "GPU_WITNESS_COMPACT_ABI",
  "deriveGpuCarrierReactionInput(",
  "CPU = WebGPU = Rust/WASM",
  "scopeAfter: expected.",
  "publishedHandle: expected.",
]) {
  if (workbenchSource.includes(forbidden)) {
    throw new Error(
      "Workbench WebGPU Scenario regained execution seeding/overclaim: " +
      forbidden,
    );
  }
}

const pagesWebGpuSmokeSource = fs.readFileSync(
  path.join(root, "experiments/browser-accelerator/tests/pages-webgpu-live-smoke.mjs"),
  "utf8",
);
for (const required of [
  "--enable-unsafe-webgpu",
  "--use-webgpu-adapter=swiftshader",
  "scenarioManifestDriven",
  "staticPreparedCarrier",
  "noExecutionSeed",
  "postReadbackCpuDifferential",
  "cpuReferenceSharesRustCore",
  "returnToFirstReuse",
  "manifestAssertionsPassed",
  "runCount",
  "values",
  "activeReactionCounts",
  "configurationAppendCounts",
  "baseUploadCount",
  "configurationCommitCount",
  "configurationDispatchCount",
  "reactionDispatchCount",
  "normalizedObservationSchemaVersion",
  "normalizedGpuRuns",
  "normalizedCpuRuns",
  "readbackBytes",
  "UNAVAILABLE",
  "UNSUPPORTED",
  "PAGES_WEBGPU_SCENARIO=PASS",
]) {
  if (!pagesWebGpuSmokeSource.includes(required)) {
    throw new Error("WebGPU Scenario live smoke contract missing: " + required);
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

const savedGpuBufferUsage = globalThis.GPUBufferUsage;
globalThis.GPUBufferUsage = {
  STORAGE: 1,
  COPY_DST: 2,
  COPY_SRC: 4,
};
const fakeResidentBuffers = [];
let fakeResidentWrites = 0;
const fakeResidentDevice = {
  limits: {
    maxBufferSize: 1 << 20,
    maxStorageBufferBindingSize: 1 << 20,
    maxStorageBuffersPerShaderStage: 8,
  },
  queue: {
    writeBuffer() { fakeResidentWrites += 1; },
  },
  createBuffer(descriptor) {
    const buffer = {
      descriptor,
      destroyCount: 0,
      destroy() { this.destroyCount += 1; },
    };
    fakeResidentBuffers.push(buffer);
    return buffer;
  },
};
const fakeResidentParsed = {
  layout: {
    linkCount: 1,
    rootHandle: 1,
    totalWords: 1,
    totalBytes: 4,
  },
  words: new Uint32Array([1]),
  sections: {},
};
const fakeResident = await openGpuCarrierResidentSession(
  fakeResidentDevice,
  fakeResidentParsed,
);
const fakeResidentOpen = fakeResident.telemetry();
if (typeof fakeResident.sessionId !== "string" ||
    fakeResident.sessionId.length === 0 ||
    fakeResidentOpen.sessionId !== fakeResident.sessionId ||
    fakeResidentOpen.baseUploadCount !== 1 ||
    fakeResidentOpen.baseUploadBytes !== 4 ||
    fakeResidentOpen.baseBufferCount !== 1 ||
    fakeResidentOpen.residentAppendCount !== 0 ||
    fakeResidentOpen.residentCapacity !== 256 ||
    !Number.isInteger(fakeResidentOpen.residentBufferBytes) ||
    fakeResidentOpen.residentBufferBytes <= 0 ||
    fakeResidentOpen.reactionDispatchCount !== 0 ||
    fakeResidentOpen.closed !== false ||
    fakeResidentWrites !== 2) {
  throw new Error(
    "resident GPU base ownership telemetry mismatch: " +
    JSON.stringify(fakeResidentOpen),
  );
}
if (fakeResident.close() !== true || fakeResident.close() !== false ||
    fakeResidentBuffers.length !== 2 ||
    fakeResidentBuffers[0]?.destroyCount !== 1 ||
    fakeResidentBuffers[1]?.destroyCount !== 1 ||
    fakeResident.telemetry().closed !== true) {
  throw new Error("resident GPU base close is not bounded/idempotent");
}
let closedResidentRejected = false;
try {
  await fakeResident.reaction({
    currentHandle: 1,
    interpreterHandle: 1,
  });
} catch (error) {
  closedResidentRejected =
    String(error?.message || error).includes("Session is closed");
}
if (!closedResidentRejected) {
  throw new Error("closed resident GPU Session did not fail closed");
}
if (savedGpuBufferUsage === undefined) {
  delete globalThis.GPUBufferUsage;
} else {
  globalThis.GPUBufferUsage = savedGpuBufferUsage;
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
    "const RESIDENT_CAPACITY: u32 = 256u",
    "fn resident_can_append() -> bool",
    "fn ensure_pair_resident(a: u32, b: u32) -> u32",
    "let endpoint = value_end(CURRENT)",
    "if (matches == 0u)",
    "discovery[0] = 11u",
  ]) {
    if (!shader.includes(marker)) {
      throw new Error("C4c3 WGSL lost resident/canonical contract: " + marker);
    }
  }
  for (const forbidden of [
    "ensure_pair_overlay",
    "ensure_start_self_overlay",
    "ensure_end_self_overlay",
    "var<storage, read_write> overlay",
  ]) {
    if (shader.includes(forbidden)) {
      throw new Error("C4c3 WGSL retained transient overlay path: " + forbidden);
    }
  }
}

const syntaxTargets = [
  "experiments/browser-accelerator/web/workbench.mjs",
  "experiments/browser-accelerator/web/proof-view.mjs",
  "experiments/browser-accelerator/web/i386-proof-transport.mjs",
  "experiments/browser-accelerator/web/i386-wasm-json.mjs",
  "experiments/browser-accelerator/web/gpu-carrier.mjs",
  "experiments/browser-accelerator/web/run-observation.mjs",
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
