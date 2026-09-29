"use strict";

const crypto = require("crypto");
const fs = require("fs");
const path = require("path");
const acceptanceStarted = process.hrtime.bigint();
const registryPath = "experiments/browser-accelerator/web/i386-blocks.json";
const wasmPath =
  "experiments/a-circuit/target/wasm32-unknown-unknown/release/amemory_a_circuit.wasm";
const registryText = fs.readFileSync(registryPath, "utf8");
const registry = JSON.parse(registryText);
for (const removedUiPath of [
  "experiments/browser-accelerator/web/app.js",
  "experiments/browser-accelerator/web/i386-lab.js",
  "experiments/browser-accelerator/web/i386-lab-view.mjs",
  "experiments/browser-accelerator/web/i386-proof-view.mjs",
]) {
  if (fs.existsSync(removedUiPath)) {
    throw new Error("obsolete parallel UI file still exists: " + removedUiPath);
  }
}
const gpuCarrierSource = fs.readFileSync(
  "experiments/browser-accelerator/web/gpu-carrier.mjs",
  "utf8"
);
for (const forbiddenBlockName of ["MUX1", "XOR2", "AND2", "NOT1"]) {
  if (gpuCarrierSource.includes(forbiddenBlockName)) {
    throw new Error(
      "generic GPU carrier executor leaked block-specific semantics: " +
      forbiddenBlockName
    );
  }
}
const c4InputStart = gpuCarrierSource.indexOf(
  "export function deriveGpuCarrierReactionInput"
);
const c4InputEnd = gpuCarrierSource.indexOf(
  "function carrierReactionCommonShader",
  c4InputStart
);
const c4InputSource = gpuCarrierSource.slice(c4InputStart, c4InputEnd);
const c4RunStart = gpuCarrierSource.indexOf(
  "export async function runGpuCarrierReaction"
);
const c4RunSource = gpuCarrierSource.slice(c4RunStart);
const c4GpuReadback = c4RunSource.indexOf("const [d, o] = await Promise.all");
const c4CpuOracle = c4RunSource.indexOf(
  "const expected = expectedGpuCarrierReaction"
);
if (c4GpuReadback < 0 || c4CpuOracle < 0 || c4CpuOracle <= c4GpuReadback) {
  throw new Error("C4c3 CPU oracle must be computed only after GPU readback");
}
for (const forbiddenSeed of [
  "currentHandle: expected.currentHandle",
  "interpreterHandle: expected.interpreterHandle",
]) {
  if (c4RunSource.includes(forbiddenSeed)) {
    throw new Error("C4c3 CPU oracle leaked into shader input: " + forbiddenSeed);
  }
}
for (const forbidden of [
  "compactProof",
  "scopeAfter",
  "scopeBefore",
  "rawRuleMatches",
  "execute?.reactions",
]) {
  if (c4InputSource.includes(forbidden)) {
    throw new Error("C4c3 execution input leaked future proof data: " + forbidden);
  }
}
const proofViewSource = fs.readFileSync(
  "experiments/browser-accelerator/web/proof-view.mjs",
  "utf8"
);
for (const required of [
  "recursiveStructureHtml(root.source)",
  "recursiveStructureHtml(resultRecursiveWire)",
  'class="proof-recursive-structure"',
]) {
  if (!proofViewSource.includes(required)) {
    throw new Error("recursive-structure collapse wiring missing: " + required);
  }
}
const workbenchPageSource = fs.readFileSync(
  "experiments/browser-accelerator/web/index.html",
  "utf8"
);
const workbenchSource = fs.readFileSync(
  "experiments/browser-accelerator/web/workbench.mjs",
  "utf8"
);
for (const required of [
  'id="workbench-root"',
  'src="./workbench.mjs"',
  'main { width: 100%; max-width: none;',
  '<html lang="ru">',
  'Апамять — рабочая лаборатория',
  'Нотация МТС v0.14',
]) {
  if (!workbenchPageSource.includes(required)) {
    throw new Error("R4 Workbench page wiring missing: " + required);
  }
}
for (const forbidden of [
  'legacy-diagnostics',
  'src="./app.js"',
  'src="./i386-lab.js"',
  'Compatibility witnesses',
]) {
  if (workbenchPageSource.includes(forbidden)) {
    throw new Error("obsolete parallel UI wiring remains on Pages: " + forbidden);
  }
}
for (const required of [
  "refreshScenarioPresetRegistry",
  "loadScenarioPresetManifestByIndex",
  "openScenarioLiveSession",
  "runScenarioLiveSession",
  "refreshScenarioLiveHistoryStatus",
  "setScenarioLiveRetentionPolicy",
  "closeScenarioLiveSession",
  "PREPARE",
  "LOAD",
  "CONFIGURE",
  "EXECUTE",
  "RESULT",
  "EVIDENCE",
  "configurationReused",
  "время недоступно",
  "wb-levels",
  "data-stage",
  '["timeline", "log", "profile", "compare"]',
  "workbenchResultStatus",
  "workbenchStageDetails",
  "workbenchRecursiveStructure",
  "recursiveStructureHtml",
  "Готовый сценарий",
  "Открыть апамять",
  "Выполнить",
  "Простой",
  "Инженерный",
  "Доказательство",
  "Хронология",
  "Журнал",
  "Профиль",
  "Сравнение",
]) {
  if (!workbenchSource.includes(required)) {
    throw new Error("R4 Workbench runtime wiring missing: " + required);
  }
}
for (const required of [
  "Структурное доказательство:",
  "ПОДГОТОВКА АСЕТИ",
  "ЗАГРУЗКА В АПАМЯТЬ",
  "ИСПОЛНЕНИЕ В ТОЙ ЖЕ АПАМЯТИ",
  "РЕЗУЛЬТАТ И ВИЗУАЛИЗАЦИЯ",
  "Схема 2D",
  "Статическая 3D",
  "Живая физика 3D",
  "рекурсивная структура",
]) {
  if (!proofViewSource.includes(required)) {
    throw new Error("Russian proof UI surface missing: " + required);
  }
}
for (const forbidden of [
  "Scenario / constructor",
  ">Preset</button>",
  ">Manual</button>",
  ">Open A-memory</button>",
  ">Run</button>",
  ">Close</button>",
  "Simple view deliberately",
  "Structural proof:",
  "PREPARE ASET",
  "LOAD INTO A-MEMORY",
  "RESULT + VISUALIZATION",
  "Loading exact-pinned",
  "<html lang=\"en\">",
]) {
  if (workbenchPageSource.includes(forbidden) ||
      workbenchSource.includes(forbidden) ||
      proofViewSource.includes(forbidden)) {
    throw new Error("English UI regression: " + forbidden);
  }
}

const bytes = fs.readFileSync(wasmPath);
const sourceSha =
  process.env.AMEMORY_SOURCE_SHA || process.env.GITHUB_SHA || null;
if (sourceSha !== null && !/^[0-9a-f]{40}$/.test(sourceSha)) {
  throw new Error("invalid source SHA for acceptance report");
}
const version = fs.readFileSync("VERSION", "utf8").trim();
if (!version) throw new Error("VERSION is empty");
const PROGRAM_FINGERPRINT_RE =
  /^prepared-aset-fnv1a64-v1:[0-9a-f]{16}$/;
Promise.all([
  WebAssembly.instantiate(bytes, {}),
  import("../../browser-accelerator/web/i386-proof-transport.mjs"),
  import("../../browser-accelerator/web/proof-verifier.mjs"),
  import("../../browser-accelerator/web/proof-view.mjs"),
  import("../../browser-accelerator/web/gpu-carrier.mjs"),
  import("../../browser-accelerator/web/scenario-transport.mjs"),
  import("../../browser-accelerator/web/scenario-presets.mjs"),
  import("../../browser-accelerator/web/workbench.mjs"),
]).then(([
  {instance},
  { inflateCompactProof, validateCompactProof },
  { verifyCompactExecutionProof },
  { recursiveStructureHtml },
  {
    deriveGpuCarrierReactionInput,
    expectedGpuCarrierLookup,
    expectedGpuCarrierReaction,
    gpuCarrierLookupShaderSource,
    gpuCarrierReactionShaderSource,
    readGpuCarrierWordsAbi,
  },
  {
    clearScenarioLiveHistory,
    clearScenarioLiveHistoryExport,
    clearScenarioTransportOutput,
    closeScenarioLiveSession,
    executeLoadedScenarioManifest,
    executeScenarioManifest,
    exportScenarioLiveHistory,
    exportScenarioLiveRun,
    exportScenarioReplayBundle,
    importScenarioReplayBundle,
    openScenarioLiveSession,
    readScenarioError,
    readScenarioReport,
    readScenarioTransportLimits,
    readScenarioLiveHistoryExport,
    refreshScenarioLiveHistoryStatus,
    refreshScenarioLiveSessionStatus,
    runScenarioLiveSession,
    setScenarioLiveRetentionPolicy,
    writeScenarioManifest,
  },
  {
    loadScenarioPresetManifest,
    loadScenarioPresetManifestByIndex,
    refreshScenarioPresetRegistry,
  },
  {
    createWorkbenchRun,
    deriveWorkbenchPipeline,
    normalizeWorkbenchInputs,
    workbenchResultStatus,
    workbenchStageDetails,
    workbenchRecursiveStructure,
  },
]) => {
  const w = instance.exports;

  // R3c: ready-made scenarios come from the canonical manifest registry.
  // The raw manifest exposed through WASM must be byte-for-byte the same
  // document that native tests and the Scenario Runner consume.
  const canonicalScenarioSource = fs.readFileSync(
    "experiments/a-circuit/scenarios/mux1-lifecycle-v1.json",
    "utf8"
  );
  const presetRegistry = refreshScenarioPresetRegistry(w);
  if (presetRegistry.entries.length !== 5) {
    throw new Error("R3d4 preset registry count mismatch");
  }
  const presetSummary = presetRegistry.entries.find(
    (entry) => entry.scenarioId === "mux1-lifecycle"
  );
  if (!presetSummary ||
      presetSummary.scenarioVersion !== "1.0.0" ||
      presetSummary.programProfileId !== "a-circuit:mux1" ||
      presetSummary.family !== "mux" ||
      presetSummary.programId !== "mux1" ||
      presetSummary.runCount !== 4 ||
      !Array.isArray(presetSummary.inputSchema) ||
      presetSummary.inputSchema.length !== 3) {
    throw new Error("R3c manifest-derived MUX1 preset summary mismatch");
  }

  const xorPresetSummary = presetRegistry.entries.find(
    (entry) => entry.scenarioId === "xor32-lifecycle"
  );
  if (!xorPresetSummary ||
      xorPresetSummary.scenarioVersion !== "1.0.0" ||
      xorPresetSummary.programProfileId !== "a-circuit:logic-xor32" ||
      xorPresetSummary.family !== "logic-effect" ||
      xorPresetSummary.programId !== "xor32" ||
      xorPresetSummary.runCount !== 4 ||
      !Array.isArray(xorPresetSummary.inputSchema) ||
      xorPresetSummary.inputSchema.length !== 2) {
    throw new Error("R3d1 manifest-derived XOR32 preset summary mismatch");
  }

  const addPresetSummary = presetRegistry.entries.find(
    (entry) => entry.scenarioId === "add32-lifecycle"
  );
  if (!addPresetSummary ||
      addPresetSummary.scenarioVersion !== "1.0.0" ||
      addPresetSummary.programProfileId !== "a-circuit:arithmetic-add32" ||
      addPresetSummary.family !== "arithmetic-effect" ||
      addPresetSummary.programId !== "add32" ||
      addPresetSummary.runCount !== 4 ||
      !Array.isArray(addPresetSummary.inputSchema) ||
      addPresetSummary.inputSchema.length !== 2) {
    throw new Error("R3d2 manifest-derived ADD32 preset summary mismatch");
  }

  const shlPresetSummary = presetRegistry.entries.find(
    (entry) => entry.scenarioId === "shl32-lifecycle"
  );
  if (!shlPresetSummary ||
      shlPresetSummary.scenarioVersion !== "1.0.0" ||
      shlPresetSummary.programProfileId !== "a-circuit:shift-shl32" ||
      shlPresetSummary.family !== "shift-effect" ||
      shlPresetSummary.programId !== "shl32" ||
      shlPresetSummary.runCount !== 5 ||
      !Array.isArray(shlPresetSummary.inputSchema) ||
      shlPresetSummary.inputSchema.length !== 2) {
    throw new Error("R3d3 manifest-derived SHL32 preset summary mismatch");
  }


  const mulPresetSummary = presetRegistry.entries.find(
    (entry) => entry.scenarioId === "mul32-lifecycle"
  );
  if (!mulPresetSummary ||
      mulPresetSummary.scenarioVersion !== "1.0.0" ||
      mulPresetSummary.programProfileId !== "a-circuit:mul32" ||
      mulPresetSummary.family !== "mul" ||
      mulPresetSummary.programId !== "mul32" ||
      mulPresetSummary.runCount !== 4 ||
      !Array.isArray(mulPresetSummary.inputSchema) ||
      mulPresetSummary.inputSchema.length !== 2) {
    throw new Error("R3d4 manifest-derived MUL32 preset summary mismatch");
  }


  if (loadScenarioPresetManifestByIndex(w, 99) !== null ||
      readScenarioError(w)?.code !== "PRESET_REGISTRY") {
    throw new Error("R3c unknown preset index did not fail closed");
  }

  const preset = loadScenarioPresetManifest(
    w,
    presetRegistry,
    "mux1-lifecycle",
    "1.0.0"
  );
  if (preset === null ||
      preset.index !== 0 ||
      preset.source !== canonicalScenarioSource) {
    throw new Error("R3c canonical preset manifest round-trip mismatch");
  }
  const scenarioManifest = JSON.parse(preset.source);

  // R4c: the visible Workbench uses this same canonical Scenario model.
  const workbenchInputs = normalizeWorkbenchInputs(
    scenarioManifest.inputSchema,
    { S: "1", A: "0", B: "1" }
  );
  const workbenchPreset = createWorkbenchRun(
    scenarioManifest,
    1,
    workbenchInputs,
    3,
    "preset"
  );
  const workbenchManual = createWorkbenchRun(
    scenarioManifest,
    0,
    { S: "1", A: "1", B: "0" },
    4,
    "manual"
  );
  if (JSON.stringify(workbenchInputs) !==
        JSON.stringify({ S: 1, A: 0, B: 1 }) ||
      workbenchPreset.runId !== "workbench-live-3" ||
      workbenchPreset.assertions.length === 0 ||
      workbenchManual.runId !== "workbench-live-4" ||
      workbenchManual.assertions.length !== 0) {
    throw new Error("R4 Workbench preset/manual Scenario-model mismatch");
  }

  for (const bad of ["12garbage", "1e3", "0x10garbage", "-1", "+1", ""]) {
    let rejected = false;
    try {
      normalizeWorkbenchInputs(
        [{ key: "X", type: "WORD32" }],
        { X: bad }
      );
    } catch {
      rejected = true;
    }
    if (!rejected) {
      throw new Error("R4 strict numeric parser accepted " + JSON.stringify(bad));
    }
  }
  const strictNumbers = normalizeWorkbenchInputs(
    [
      { key: "B", type: "BIT" },
      { key: "C", type: "COUNT8" },
      { key: "W", type: "WORD32" },
    ],
    { B: "1", C: "0xff", W: "4294967295" }
  );
  if (JSON.stringify(strictNumbers) !==
      JSON.stringify({ B: 1, C: 255, W: 0xffffffff })) {
    throw new Error("R4 strict numeric parser changed valid typed inputs");
  }

  const workbenchOpenPipeline = deriveWorkbenchPipeline(
    { prepareCount: 1, loadCount: 1, baseLinkCount: 123 },
    null
  );
  const workbenchRunPipeline = deriveWorkbenchPipeline(
    { prepareCount: 1, loadCount: 1, baseLinkCount: 123 },
    {
      linksBeforeConfigure: 123,
      linksAfterConfigure: 130,
      pipelineProfile: {
        stages: {
          timingAvailable: false,
          configureNs: 0,
          executeNs: 0,
          resultNs: 0,
          evidenceNs: 0,
        },
      },
      observed: {
        activeReactionCount: 7,
        events: [{ kind: "RUN_START" }, { kind: "RUN_END" }],
      },
    }
  );
  if (workbenchOpenPipeline[0].state !== "done" ||
      workbenchOpenPipeline[1].state !== "done" ||
      workbenchOpenPipeline.slice(2).some(
        (stage) => stage.state !== "waiting"
      ) ||
      workbenchRunPipeline.some((stage) => stage.state !== "done")) {
    throw new Error("R4 Workbench pipeline is not driven by runtime evidence");
  }
  const workbenchReady = workbenchResultStatus(null);
  const workbenchManualStatus = workbenchResultStatus({
    assertionResults: [],
  });
  const workbenchLoadDetails = workbenchStageDetails(
    {
      sessionId: "s",
      loadCount: 1,
      baseLinkCount: 123,
      currentLinkCount: 130,
      sessionOpenProfile: { stages: { timingAvailable: false } },
    },
    null,
    "LOAD"
  );
  if (workbenchReady.kind !== "ready" ||
      workbenchManualStatus.kind !== "neutral" ||
      workbenchLoadDetails?.loadCount !== 1 ||
      workbenchLoadDetails?.baseLinkCount !== 123) {
    throw new Error("R4 progressive-disclosure evidence projection mismatch");
  }
  const workbenchRecursive = workbenchRecursiveStructure({
    result: { resultRecursiveWire: "1AB" },
  });
  if (workbenchRecursive?.key !== "resultRecursiveWire" ||
      workbenchRecursive?.value !== "1AB" ||
      !recursiveStructureHtml(workbenchRecursive.value).includes(
        'class="proof-recursive-structure"'
      )) {
    throw new Error("R4 generic recursive proof projection mismatch");
  }

  // R2d: browser transport must consume the generic Scenario Runner report,
  // not the legacy block-result slot. The manifest itself contains four runs
  // that execute on one real persistent CPU Session.
  const legacyResultBeforeScenario =
    w.amemory_i386_lab_result_available() >>> 0;
  const scenarioLimits = readScenarioTransportLimits(w);
  if (scenarioLimits.maxManifestBytes !== 1024 * 1024 ||
      scenarioLimits.maxSingleReportBytes !== 8 * 1024 * 1024 ||
      scenarioLimits.maxErrorBytes !== 512 * 1024 ||
      scenarioLimits.maxRetainedReports !== 1 ||
      scenarioLimits.maxRetainedErrors !== 1 ||
      scenarioLimits.retentionMode !== "LATEST" ||
      scenarioLimits.liveSessionRetained !== true ||
      scenarioLimits.batchSessionRetained !== false ||
      scenarioLimits.liveObserver?.defaultRetentionMode !== "RING" ||
      JSON.stringify(
        scenarioLimits.liveObserver?.supportedRetentionModes
      ) !== JSON.stringify(["LATEST", "RING", "EXPLICIT_EXPORT"]) ||
      scenarioLimits.liveObserver?.maxRetainedRuns !== 32 ||
      scenarioLimits.liveObserver?.maxRetainedEvents !== 65536 ||
      scenarioLimits.liveObserver?.maxRetainedBytes !== 16 * 1024 * 1024 ||
      scenarioLimits.liveObserver?.maxProfilePoints !== 64 ||
      scenarioLimits.liveObserver?.maxExportBytes !== 16 * 1024 * 1024) {
    throw new Error("R2e scenario transport limits/policy mismatch");
  }

  const oversizedManifest =
    "x".repeat(scenarioLimits.maxManifestBytes + 1);
  let oversizedRejected = false;
  try {
    writeScenarioManifest(w, oversizedManifest);
  } catch (error) {
    oversizedRejected =
      String(error).includes("exceeds") &&
      String(error).includes(String(scenarioLimits.maxManifestBytes));
  }
  if (!oversizedRejected) {
    throw new Error("R2e oversized manifest was not rejected client-side");
  }

  const scenarioManifestLength =
    writeScenarioManifest(w, scenarioManifest);
  const scenarioCpu = executeLoadedScenarioManifest(
    w,
    scenarioManifestLength,
    "optimized-cpu"
  );
  if (!scenarioCpu.ok || scenarioCpu.error !== null) {
    throw new Error("R2d optimized-cpu scenario transport failed");
  }
  const scenarioReport = scenarioCpu.report;
  if (scenarioReport.schemaVersion !== 1 ||
      scenarioReport.scenarioId !== "mux1-lifecycle" ||
      scenarioReport.overallPass !== true ||
      !Array.isArray(scenarioReport.runs) ||
      scenarioReport.runs.length !== 4 ||
      typeof scenarioReport.sessionId !== "string" ||
      !scenarioReport.sessionId.length ||
      scenarioReport.sessionOpenProfile?.sessionId !== scenarioReport.sessionId) {
    throw new Error("R2d scenario report envelope mismatch");
  }
  const openStages = scenarioReport.sessionOpenProfile?.stages;
  if (openStages?.timingAvailable !== false ||
      openStages?.prepareNs !== 0 ||
      openStages?.loadNs !== 0) {
    throw new Error(
      "R2d bare-WASM session-open timing must be explicitly unavailable"
    );
  }

  for (let index = 0; index < scenarioReport.runs.length; index += 1) {
    const run = scenarioReport.runs[index];
    const observed = run.observed;
    const profile = observed?.profile;
    const structural = profile?.structural;
    const stages = run.pipelineProfile?.stages;
    const reactionEvents = Array.isArray(observed?.events)
      ? observed.events.filter((event) => event.kind === "REACTION_END")
      : [];
    const nativeFacts = reactionEvents.flatMap(
      (event) => Array.isArray(event.structuralFacts)
        ? event.structuralFacts
        : []
    );

    if ((run.sessionRunId >>> 0) !== index + 1 ||
        observed?.sessionId !== scenarioReport.sessionId ||
        observed?.finalQuiescent !== true ||
        observed?.activeReactionCount !== 7 ||
        !run.pipelineProfile ||
        reactionEvents.length !== 8 ||
        nativeFacts.length === 0) {
      throw new Error("R2d run/session correlation mismatch at run " + index);
    }

    if (observed.timingAvailable !== false ||
        profile?.timingAvailable !== false ||
        structural?.timingAvailable !== false ||
        stages?.timingAvailable !== false ||
        profile?.executeNs !== 0 ||
        profile?.traceProjectionNs !== 0 ||
        structural?.discoveryNs !== 0 ||
        structural?.roleDecodeNs !== 0 ||
        structural?.unificationNs !== 0 ||
        structural?.instantiationNs !== 0 ||
        structural?.publicationNs !== 0 ||
        structural?.totalNs !== 0 ||
        stages?.configureNs !== 0 ||
        stages?.executeNs !== 0 ||
        stages?.resultNs !== 0 ||
        stages?.evidenceNs !== 0) {
      throw new Error(
        "R2d bare-WASM internal timings must be zero and explicitly unavailable"
      );
    }

    if (!(structural.triggerIncidenceCandidates > 0) ||
        !(structural.unificationAttempts > 0) ||
        !(structural.publicationOutputs > 0)) {
      throw new Error(
        "R2d bare-WASM profile lost real structural counters at run " + index
      );
    }
  }
  if (scenarioReport.runs[3].configurationReused !== true ||
      JSON.stringify(scenarioReport.runs[3].inputs) !==
        JSON.stringify(scenarioReport.runs[0].inputs)) {
    throw new Error("R2d return-to-first canonical reuse missing");
  }
  if ((w.amemory_i386_lab_result_available() >>> 0) !==
      legacyResultBeforeScenario) {
    throw new Error(
      "R2d generic scenario transport mutated legacy LabInstanceState result slot"
    );
  }

  // #332: the built-in program registry resolves the complete descriptor.
  // A known profileId with contradictory provenance must fail before LOAD.
  for (const [field, value] of [
    ["family", "not-mux"],
    ["programId", "not-mux1"],
    ["asetSource", "audit:unknown-program"],
  ]) {
    const tamperedManifest = JSON.parse(JSON.stringify(scenarioManifest));
    tamperedManifest.programProfile[field] = value;
    const rejected = openScenarioLiveSession(w, tamperedManifest);
    if (rejected.ok ||
        rejected.status !== null ||
        rejected.error?.code !== "RUNNER" ||
        rejected.error?.error?.code !== "PROGRAM_PROFILE_MISMATCH") {
      throw new Error("#332 contradictory " + field + " did not fail before LOAD");
    }
  }

  // R4a/R4b: separate browser Run calls reuse one retained CPU Session,
  // while observer history remains a bounded host-only concern.
  const liveOpened = openScenarioLiveSession(w, scenarioManifest);
  if (!liveOpened.ok ||
      liveOpened.status?.completedRuns !== 0 ||
      liveOpened.status?.prepareCount !== 1 ||
      liveOpened.status?.loadCount !== 1 ||
      liveOpened.status?.storeInstanceId !==
        liveOpened.status?.sessionId ||
      typeof liveOpened.status?.engineInstanceId !== "string" ||
      liveOpened.status.engineInstanceId.length === 0 ||
      !PROGRAM_FINGERPRINT_RE.test(
        liveOpened.status?.programFingerprint ?? ""
      ) ||
      JSON.stringify(liveOpened.status?.programProfile) !==
        JSON.stringify(scenarioManifest.programProfile) ||
      liveOpened.status?.manifestFieldSemantics?.programProfile !==
        "ENFORCED" ||
      liveOpened.status?.manifestFieldSemantics?.typedInputs !==
        "ENFORCED" ||
      liveOpened.status?.manifestFieldSemantics?.invariants !==
        "ADVISORY" ||
      liveOpened.status?.manifestFieldSemantics?.profilingPolicy !==
        "ADVISORY" ||
      liveOpened.status?.manifestFieldSemantics?.visualizationProfile !==
        "ADVISORY") {
    throw new Error("R4a retained Session open evidence mismatch");
  }
  const liveIdentity = {
    sessionId: liveOpened.status.sessionId,
    storeInstanceId: liveOpened.status.storeInstanceId,
    engineInstanceId: liveOpened.status.engineInstanceId,
    baseLinkCount: liveOpened.status.baseLinkCount,
    programFingerprint: liveOpened.status.programFingerprint,
  };

  const ringPolicy = setScenarioLiveRetentionPolicy(w, {
    retentionMode: "RING",
    maxRetainedRuns: 2,
    maxRetainedEvents: 512,
    maxRetainedBytes: 2 * 1024 * 1024,
  });
  if (!ringPolicy.ok ||
      ringPolicy.status?.retentionMode !== "RING" ||
      ringPolicy.status?.retainedRuns !== 0 ||
      ringPolicy.status?.profilePoints !== 0) {
    throw new Error("R4b RING policy activation mismatch");
  }

  const invalidLiveRun = {
    ...scenarioManifest.runSequence[0],
    runId: "invalid-live-input",
    inputs: {
      ...scenarioManifest.runSequence[0].inputs,
      S: "12garbage",
    },
  };
  const linksBeforeInvalid = liveOpened.status.currentLinkCount;
  const invalidLive = runScenarioLiveSession(w, invalidLiveRun);
  const statusAfterInvalid = refreshScenarioLiveSessionStatus(w);
  if (invalidLive.ok ||
      invalidLive.error?.code !== "RUNNER" ||
      invalidLive.error?.error?.code !== "VALIDATION" ||
      !statusAfterInvalid.ok ||
      statusAfterInvalid.status?.currentLinkCount !== linksBeforeInvalid ||
      statusAfterInvalid.status?.completedRuns !== 0 ||
      statusAfterInvalid.status?.prepareCount !== 1 ||
      statusAfterInvalid.status?.loadCount !== 1) {
    throw new Error("#332 invalid live input mutated Session before CONFIGURE");
  }

  const liveRun1 = {
    ...scenarioManifest.runSequence[0],
    runId: "live-run-1",
  };
  const liveRun2 = {
    ...scenarioManifest.runSequence[1],
    runId: "live-run-2",
  };
  const liveRun3 = {
    ...scenarioManifest.runSequence[0],
    runId: "live-run-3-return",
  };

  const liveFirst = runScenarioLiveSession(w, liveRun1);
  const liveSecond = runScenarioLiveSession(w, liveRun2);
  for (const [index, result] of [liveFirst, liveSecond].entries()) {
    const status = result.payload?.status;
    const run = result.payload?.run;
    if (!result.ok ||
        status?.sessionId !== liveIdentity.sessionId ||
        status?.storeInstanceId !== liveIdentity.storeInstanceId ||
        status?.engineInstanceId !== liveIdentity.engineInstanceId ||
        status?.baseLinkCount !== liveIdentity.baseLinkCount ||
        status?.programFingerprint !== liveIdentity.programFingerprint ||
        status?.prepareCount !== 1 ||
        status?.loadCount !== 1 ||
        status?.completedRuns !== index + 1 ||
        run?.sessionRunId !== index + 1 ||
        run?.observed?.sessionId !== liveIdentity.sessionId ||
        run?.observed?.finalQuiescent !== true) {
      throw new Error(
        "R4a separate Run call did not retain one Session at " + index
      );
    }
  }

  const historyAfterAB = refreshScenarioLiveHistoryStatus(w);
  if (!historyAfterAB.ok ||
      historyAfterAB.status?.retentionMode !== "RING" ||
      historyAfterAB.status?.retainedRuns !== 2 ||
      historyAfterAB.status?.firstRunId !== 1 ||
      historyAfterAB.status?.lastRunId !== 2 ||
      historyAfterAB.status?.latestRunId !== 2 ||
      historyAfterAB.status?.retainedEvents <= 0 ||
      historyAfterAB.status?.retainedBytes <= 0 ||
      historyAfterAB.status?.profilePoints !== 2 ||
      historyAfterAB.status?.exportAvailable !== false) {
    throw new Error("R4b RING counters after A/B mismatch");
  }

  const exportedA = exportScenarioLiveRun(w, 1);
  if (!exportedA.ok ||
      exportedA.export?.exportKind !== "SELECTED_RUN" ||
      exportedA.export?.sessionId !== liveIdentity.sessionId ||
      exportedA.export?.programFingerprint !==
        liveIdentity.programFingerprint ||
      exportedA.export?.runs?.length !== 1 ||
      exportedA.export.runs[0]?.sessionRunId !== 1) {
    throw new Error("R4b selected run A export mismatch");
  }
  const exportedABeforeClear = JSON.stringify(exportedA.export);

  const clearedHistory = clearScenarioLiveHistory(w);
  if (!clearedHistory.ok ||
      clearedHistory.status?.retainedRuns !== 0 ||
      clearedHistory.status?.retainedEvents !== 0 ||
      clearedHistory.status?.retainedBytes !== 0 ||
      clearedHistory.status?.latestRunId !== undefined ||
      clearedHistory.status?.profilePoints !== 2 ||
      clearedHistory.status?.exportAvailable !== true ||
      JSON.stringify(readScenarioLiveHistoryExport(w)) !==
        exportedABeforeClear) {
    throw new Error(
      "R4b raw-history clear changed profile/export observer state"
    );
  }

  const liveThird = runScenarioLiveSession(w, liveRun3);
  const thirdStatus = liveThird.payload?.status;
  const thirdRun = liveThird.payload?.run;
  if (!liveThird.ok ||
      thirdStatus?.sessionId !== liveIdentity.sessionId ||
      thirdStatus?.storeInstanceId !== liveIdentity.storeInstanceId ||
      thirdStatus?.engineInstanceId !== liveIdentity.engineInstanceId ||
      thirdStatus?.baseLinkCount !== liveIdentity.baseLinkCount ||
      thirdStatus?.programFingerprint !== liveIdentity.programFingerprint ||
      thirdStatus?.prepareCount !== 1 ||
      thirdStatus?.loadCount !== 1 ||
      thirdStatus?.completedRuns !== 3 ||
      thirdRun?.sessionRunId !== 3 ||
      thirdRun?.observed?.sessionId !== liveIdentity.sessionId ||
      thirdRun?.observed?.finalQuiescent !== true ||
      thirdRun?.configurationReused !== true ||
      JSON.stringify(thirdRun?.result) !==
        JSON.stringify(liveFirst.payload?.run?.result) ||
      thirdRun?.observed?.activeReactionCount !==
        liveFirst.payload?.run?.observed?.activeReactionCount) {
    throw new Error(
      "R4b run C changed retained Session or semantic behavior"
    );
  }

  const historyAfterC = refreshScenarioLiveHistoryStatus(w);
  if (!historyAfterC.ok ||
      historyAfterC.status?.retainedRuns !== 1 ||
      historyAfterC.status?.firstRunId !== 3 ||
      historyAfterC.status?.lastRunId !== 3 ||
      historyAfterC.status?.profilePoints !== 3 ||
      historyAfterC.status?.exportAvailable !== true ||
      JSON.stringify(readScenarioLiveHistoryExport(w)) !==
        exportedABeforeClear) {
    throw new Error("R4b run C history/profile/export mismatch");
  }

  const exportedAvailable = exportScenarioLiveHistory(w);
  if (!exportedAvailable.ok ||
      exportedAvailable.export?.exportKind !== "AVAILABLE_HISTORY" ||
      exportedAvailable.export?.runs?.length !== 1 ||
      exportedAvailable.export.runs[0]?.sessionRunId !== 3) {
    throw new Error("R4b available-history export mismatch");
  }

  // LATEST is real policy behavior, not an alias for RING.
  const latestPolicy = setScenarioLiveRetentionPolicy(w, {
    retentionMode: "LATEST",
    maxRetainedRuns: 8,
    maxRetainedEvents: 512,
    maxRetainedBytes: 2 * 1024 * 1024,
  });
  if (!latestPolicy.ok ||
      latestPolicy.status?.retentionMode !== "LATEST") {
    throw new Error("R4b LATEST policy activation mismatch");
  }
  const liveFourth = runScenarioLiveSession(w, {
    ...liveRun2,
    runId: "live-run-4-latest-a",
  });
  const liveFifth = runScenarioLiveSession(w, {
    ...liveRun1,
    runId: "live-run-5-latest-b",
  });
  const latestStatus = refreshScenarioLiveHistoryStatus(w);
  if (!liveFourth.ok || !liveFifth.ok || !latestStatus.ok ||
      latestStatus.status?.retainedRuns !== 1 ||
      latestStatus.status?.firstRunId !== 5 ||
      latestStatus.status?.lastRunId !== 5 ||
      latestStatus.status?.profilePoints !== 5) {
    throw new Error("R4b LATEST did not retain exactly the newest run");
  }

  // EXPLICIT_EXPORT retains no automatic raw ring, but exposes the latest
  // complete report for an explicit pin/export action.
  const explicitPolicy = setScenarioLiveRetentionPolicy(w, {
    retentionMode: "EXPLICIT_EXPORT",
    maxRetainedRuns: 8,
    maxRetainedEvents: 512,
    maxRetainedBytes: 2 * 1024 * 1024,
  });
  if (!explicitPolicy.ok ||
      explicitPolicy.status?.retentionMode !== "EXPLICIT_EXPORT" ||
      explicitPolicy.status?.retainedRuns !== 0) {
    throw new Error("R4b EXPLICIT_EXPORT policy activation mismatch");
  }
  const liveSixth = runScenarioLiveSession(w, {
    ...liveRun2,
    runId: "live-run-6-explicit",
  });
  const explicitStatus = refreshScenarioLiveHistoryStatus(w);
  if (!liveSixth.ok || !explicitStatus.ok ||
      explicitStatus.status?.retainedRuns !== 0 ||
      explicitStatus.status?.latestRunId !== 6 ||
      explicitStatus.status?.profilePoints !== 6) {
    throw new Error("R4b EXPLICIT_EXPORT automatic retention mismatch");
  }
  const exportedSixth = exportScenarioLiveRun(w, 6);
  if (!exportedSixth.ok ||
      exportedSixth.export?.runs?.length !== 1 ||
      exportedSixth.export.runs[0]?.sessionRunId !== 6) {
    throw new Error("R4b EXPLICIT_EXPORT pin mismatch");
  }

  clearScenarioLiveHistoryExport(w);
  const noExportStatus = refreshScenarioLiveHistoryStatus(w);
  if (!noExportStatus.ok ||
      noExportStatus.status?.exportAvailable !== false ||
      readScenarioLiveHistoryExport(w) !== null) {
    throw new Error("R4b explicit export clear mismatch");
  }

  const liveRefreshed = refreshScenarioLiveSessionStatus(w);
  if (!liveRefreshed.ok ||
      liveRefreshed.status?.completedRuns !== 6 ||
      liveRefreshed.status?.sessionId !== liveIdentity.sessionId ||
      liveRefreshed.status?.engineInstanceId !==
        liveIdentity.engineInstanceId) {
    throw new Error("R4a retained Session status refresh mismatch");
  }
  if (!closeScenarioLiveSession(w)) {
    throw new Error("R4a explicit Session close reported no live Session");
  }
  const afterClose = runScenarioLiveSession(w, {
    ...liveRun1,
    runId: "live-run-after-close",
  });
  if (afterClose.ok ||
      afterClose.error?.code !== "LIVE_SESSION_NOT_OPEN") {
    throw new Error("R4a run after close did not fail closed");
  }
  const historyAfterClose = refreshScenarioLiveHistoryStatus(w);
  if (historyAfterClose.ok ||
      historyAfterClose.error?.code !== "LIVE_SESSION_NOT_OPEN" ||
      readScenarioLiveHistoryExport(w) !== null) {
    throw new Error("R4b close did not release observer host state");
  }
  if ((w.amemory_i386_lab_result_available() >>> 0) !==
      legacyResultBeforeScenario) {
    throw new Error(
      "R4a/R4b retained Scenario Session mutated legacy LabInstanceState"
    );
  }

  // R2e: observer-output cleanup// R2e: observer-output cleanup is separate from manifest/runtime input.
  // Clearing the latest report/error must not alter legacy state and the same
  // already-loaded manifest must remain executable.
  clearScenarioTransportOutput(w);
  if (readScenarioReport(w) !== null ||
      readScenarioError(w) !== null ||
      (w.amemory_i386_lab_result_available() >>> 0) !==
        legacyResultBeforeScenario) {
    throw new Error("R2e output cleanup leaked into semantic/legacy state");
  }

  // A rejected oversized manifest must not have overwritten the valid
  // manifest that was already loaded before observer-output cleanup.
  const scenarioCpuRerun = executeLoadedScenarioManifest(
    w,
    scenarioManifestLength,
    "optimized-cpu"
  );
  if (!scenarioCpuRerun.ok ||
      scenarioCpuRerun.report?.overallPass !== true ||
      scenarioCpuRerun.report?.runs?.length !== 4) {
    throw new Error("R2e cleared-output manifest rerun failed");
  }
  for (let index = 0; index < scenarioReport.runs.length; index += 1) {
    const before = scenarioReport.runs[index];
    const after = scenarioCpuRerun.report.runs[index];
    if (JSON.stringify(before.inputs) !== JSON.stringify(after.inputs) ||
        JSON.stringify(before.result) !== JSON.stringify(after.result) ||
        before.observed?.activeReactionCount !==
          after.observed?.activeReactionCount ||
        before.observed?.finalQuiescent !==
          after.observed?.finalQuiescent) {
      throw new Error(
        "R2e observer cleanup changed semantic rerun at run " + index
      );
    }
  }

  // R3d1: the second preset uses the exact same registry and Scenario
  // transport. There is intentionally no XOR-specific browser runner.
  const canonicalXorSource = fs.readFileSync(
    "experiments/a-circuit/scenarios/xor32-lifecycle-v1.json",
    "utf8"
  );
  const xorPreset = loadScenarioPresetManifest(
    w,
    presetRegistry,
    "xor32-lifecycle",
    "1.0.0"
  );
  if (xorPreset === null || xorPreset.source !== canonicalXorSource) {
    throw new Error("R3d1 XOR32 canonical preset round-trip mismatch");
  }
  const xorScenario = JSON.parse(xorPreset.source);
  const xorCpu = executeScenarioManifest(
    w,
    xorScenario,
    "optimized-cpu"
  );
  if (!xorCpu.ok ||
      xorCpu.error !== null ||
      xorCpu.report?.scenarioId !== "xor32-lifecycle" ||
      xorCpu.report?.overallPass !== true ||
      xorCpu.report?.runs?.length !== 4) {
    throw new Error("R3d1 generic XOR32 Scenario execution failed");
  }
  for (let index = 0; index < xorCpu.report.runs.length; index += 1) {
    const run = xorCpu.report.runs[index];
    if ((run.sessionRunId >>> 0) !== index + 1 ||
        run.observed?.sessionId !== xorCpu.report.sessionId ||
        run.observed?.finalQuiescent !== true ||
        run.observed?.activeReactionCount !== 147 ||
        run.oracleMatches !== true ||
        !run.pipelineProfile ||
        run.observed?.profile?.structural?.unificationAttempts <= 0) {
      throw new Error(
        "R3d1 XOR32 persistent-session evidence mismatch at run " + index
      );
    }
  }
  if (xorCpu.report.runs[2].result?.fields?.value !== "0x1d3b5687" ||
      xorCpu.report.runs[3].configurationReused !== true ||
      JSON.stringify(xorCpu.report.runs[3].inputs) !==
        JSON.stringify(xorCpu.report.runs[0].inputs)) {
    throw new Error("R3d1 XOR32 result/canonical reuse mismatch");
  }
  if ((w.amemory_i386_lab_result_available() >>> 0) !==
      legacyResultBeforeScenario) {
    throw new Error(
      "R3d1 XOR32 generic Scenario path mutated legacy LabInstanceState"
    );
  }

  // R3d2: arithmetic uses the same generic preset registry and Scenario
  // transport. There is no ADD-specific browser executor.
  const canonicalAddSource = fs.readFileSync(
    "experiments/a-circuit/scenarios/add32-lifecycle-v1.json",
    "utf8"
  );
  const addPreset = loadScenarioPresetManifest(
    w,
    presetRegistry,
    "add32-lifecycle",
    "1.0.0"
  );
  if (addPreset === null || addPreset.source !== canonicalAddSource) {
    throw new Error("R3d2 ADD32 canonical preset round-trip mismatch");
  }
  const addScenario = JSON.parse(addPreset.source);
  const addCpu = executeScenarioManifest(
    w,
    addScenario,
    "optimized-cpu"
  );
  if (!addCpu.ok ||
      addCpu.error !== null ||
      addCpu.report?.scenarioId !== "add32-lifecycle" ||
      addCpu.report?.overallPass !== true ||
      addCpu.report?.runs?.length !== 4) {
    throw new Error("R3d2 generic ADD32 Scenario execution failed");
  }
  for (let index = 0; index < addCpu.report.runs.length; index += 1) {
    const run = addCpu.report.runs[index];
    if ((run.sessionRunId >>> 0) !== index + 1 ||
        run.observed?.sessionId !== addCpu.report.sessionId ||
        run.observed?.finalQuiescent !== true ||
        run.observed?.activeReactionCount !== 609 ||
        run.oracleMatches !== true ||
        !run.pipelineProfile ||
        run.observed?.profile?.structural?.unificationAttempts <= 0) {
      throw new Error(
        "R3d2 ADD32 persistent-session evidence mismatch at run " + index
      );
    }
  }
  if (addCpu.report.runs[1].result?.fields?.value !== "0x00000000" ||
      addCpu.report.runs[1].result?.fields?.valueMask !== "0x00000055" ||
      addCpu.report.runs[2].result?.fields?.value !== "0x80000000" ||
      addCpu.report.runs[2].result?.fields?.valueMask !== "0x00000894" ||
      addCpu.report.runs[3].configurationReused !== true ||
      JSON.stringify(addCpu.report.runs[3].inputs) !==
        JSON.stringify(addCpu.report.runs[0].inputs)) {
    throw new Error("R3d2 ADD32 result/flags/canonical reuse mismatch");
  }
  if ((w.amemory_i386_lab_result_available() >>> 0) !==
      legacyResultBeforeScenario) {
    throw new Error(
      "R3d2 ADD32 generic Scenario path mutated legacy LabInstanceState"
    );
  }

  // R3d3: SHIFT also uses the exact same generic preset/Scenario path.
  // COUNT=33 stays a distinct structural input and masks to COUNT=1 only
  // through the loaded SHL32 program.
  const canonicalShlSource = fs.readFileSync(
    "experiments/a-circuit/scenarios/shl32-lifecycle-v1.json",
    "utf8"
  );
  const shlPreset = loadScenarioPresetManifest(
    w,
    presetRegistry,
    "shl32-lifecycle",
    "1.0.0"
  );
  if (shlPreset === null || shlPreset.source !== canonicalShlSource) {
    throw new Error("R3d3 SHL32 canonical preset round-trip mismatch");
  }
  const shlScenario = JSON.parse(shlPreset.source);
  const shlCpu = executeScenarioManifest(
    w,
    shlScenario,
    "optimized-cpu"
  );
  if (!shlCpu.ok ||
      shlCpu.error !== null ||
      shlCpu.report?.scenarioId !== "shl32-lifecycle" ||
      shlCpu.report?.overallPass !== true ||
      shlCpu.report?.runs?.length !== 5) {
    throw new Error("R3d3 generic SHL32 Scenario execution failed");
  }
  const expectedShlReactions = [1, 84, 82, 84, 1];
  for (let index = 0; index < shlCpu.report.runs.length; index += 1) {
    const run = shlCpu.report.runs[index];
    if ((run.sessionRunId >>> 0) !== index + 1 ||
        run.observed?.sessionId !== shlCpu.report.sessionId ||
        run.observed?.finalQuiescent !== true ||
        run.observed?.activeReactionCount !== expectedShlReactions[index] ||
        run.oracleMatches !== true ||
        !run.pipelineProfile ||
        run.observed?.profile?.structural?.unificationAttempts <= 0) {
      throw new Error(
        "R3d3 SHL32 persistent-session evidence mismatch at run " + index
      );
    }
  }
  if (JSON.stringify(shlCpu.report.runs[1].result) !==
        JSON.stringify(shlCpu.report.runs[3].result) ||
      shlCpu.report.runs[3].configurationReused !== false ||
      shlCpu.report.runs[2].result?.fields?.undefinedMask !== "0x00000810" ||
      shlCpu.report.runs[4].configurationReused !== true ||
      JSON.stringify(shlCpu.report.runs[4].inputs) !==
        JSON.stringify(shlCpu.report.runs[0].inputs)) {
    throw new Error("R3d3 SHL32 masking/flags/canonical reuse mismatch");
  }
  if ((w.amemory_i386_lab_result_available() >>> 0) !==
      legacyResultBeforeScenario) {
    throw new Error(
      "R3d3 SHL32 generic Scenario path mutated legacy LabInstanceState"
    );
  }

  // R3d4: raw MUL32 Wide64 uses the same generic preset/Scenario path.
  const canonicalMulSource = fs.readFileSync(
    "experiments/a-circuit/scenarios/mul32-lifecycle-v1.json",
    "utf8"
  );
  const mulPreset = loadScenarioPresetManifest(
    w,
    presetRegistry,
    "mul32-lifecycle",
    "1.0.0"
  );
  if (mulPreset === null || mulPreset.source !== canonicalMulSource) {
    throw new Error("R3d4 MUL32 canonical preset round-trip mismatch");
  }
  const mulScenario = JSON.parse(mulPreset.source);
  const mulCpu = executeScenarioManifest(
    w,
    mulScenario,
    "optimized-cpu"
  );
  if (!mulCpu.ok ||
      mulCpu.error !== null ||
      mulCpu.report?.scenarioId !== "mul32-lifecycle" ||
      mulCpu.report?.overallPass !== true ||
      mulCpu.report?.runs?.length !== 4) {
    throw new Error("R3d4 generic MUL32 Scenario execution failed");
  }
  const expectedMulReactions = [33, 1071, 1071, 33];
  for (let index = 0; index < mulCpu.report.runs.length; index += 1) {
    const run = mulCpu.report.runs[index];
    if ((run.sessionRunId >>> 0) !== index + 1 ||
        run.observed?.sessionId !== mulCpu.report.sessionId ||
        run.observed?.finalQuiescent !== true ||
        run.observed?.activeReactionCount !== expectedMulReactions[index] ||
        run.oracleMatches !== true ||
        !run.pipelineProfile ||
        run.observed?.profile?.structural?.unificationAttempts <= 0) {
      throw new Error(
        "R3d4 MUL32 persistent-session evidence mismatch at run " + index
      );
    }
  }
  if (mulCpu.report.runs[2].result?.fields?.lo !== "0x00000000" ||
      mulCpu.report.runs[2].result?.fields?.hi !== "0x00000001" ||
      mulCpu.report.runs[3].configurationReused !== true ||
      JSON.stringify(mulCpu.report.runs[3].inputs) !==
        JSON.stringify(mulCpu.report.runs[0].inputs)) {
    throw new Error("R3d4 MUL32 Wide64/canonical reuse mismatch");
  }
  if ((w.amemory_i386_lab_result_available() >>> 0) !==
      legacyResultBeforeScenario) {
    throw new Error(
      "R3d4 MUL32 generic Scenario path mutated legacy LabInstanceState"
    );
  }

  // R3e: a canonical preset can be edited as the same manifest model,
  // exported with exact execution provenance, imported, and replayed through
  // the same generic transport. This is reproducibility, not retained-session
  // UI state; R4 owns the latter.
  const editedScenario = JSON.parse(JSON.stringify(scenarioManifest));
  editedScenario.title += " — manual edit witness";
  editedScenario.runSequence.push({
    runId: "run-5-manual-select-b-zero",
    inputs: { S: 1, A: 1, B: 0 },
    executionMode: "TO_QUIESCENCE",
    maxReactions: 64,
    assertions: [
      { kind: "RESULT_FIELD_EQUALS", field: "value", expected: 0 },
      { kind: "QUIESCENT_EQUALS", expected: true },
      { kind: "REACTION_COUNT_EQUALS", expected: 7 },
    ],
  });

  const editedCpu = executeScenarioManifest(
    w,
    editedScenario,
    "optimized-cpu"
  );
  if (!editedCpu.ok ||
      editedCpu.report?.overallPass !== true ||
      editedCpu.report?.runs?.length !== 5 ||
      editedCpu.report.runs[4]?.sessionRunId !== 5 ||
      editedCpu.report.runs.some(
        (run) => run.observed?.sessionId !== editedCpu.report.sessionId
      )) {
    throw new Error("R3e edited manifest did not run in one Session");
  }

  const editedProvenance = editedCpu.report.provenance;
  if (editedProvenance?.amemoryVersion !== version ||
      editedProvenance?.buildSha !== sourceSha ||
      JSON.stringify(editedCpu.report?.programProfile) !==
        JSON.stringify(editedScenario.programProfile) ||
      JSON.stringify(editedProvenance?.programProfile) !==
        JSON.stringify(editedScenario.programProfile) ||
      !/^prepared-aset-fnv1a64-v1:[0-9a-f]{16}$/.test(
        editedProvenance?.programFingerprint ?? ""
      )) {
    throw new Error("R3e exact replay provenance is incomplete");
  }

  const replayBundle = exportScenarioReplayBundle(
    editedScenario,
    editedCpu.report
  );
  const replayText = JSON.stringify(replayBundle);
  const importedReplay = importScenarioReplayBundle(replayText);
  if (importedReplay.schemaVersion !== 1 ||
      importedReplay.manifest.runSequence.length !== 5 ||
      importedReplay.backend !== "optimized-cpu" ||
      importedReplay.provenance.buildSha !== sourceSha ||
      importedReplay.provenance.programFingerprint !==
        editedProvenance.programFingerprint) {
    throw new Error("R3e replay bundle export/import mismatch");
  }

  const replayCpu = executeScenarioManifest(
    w,
    importedReplay.manifest,
    importedReplay.backend
  );
  if (!replayCpu.ok ||
      replayCpu.report?.overallPass !== true ||
      replayCpu.report?.sessionId === editedCpu.report.sessionId ||
      replayCpu.report?.provenance?.programFingerprint !==
        importedReplay.provenance.programFingerprint ||
      replayCpu.report?.provenance?.buildSha !==
        importedReplay.provenance.buildSha) {
    throw new Error("R3e imported replay provenance mismatch");
  }
  const editedSemanticResults = editedCpu.report.runs.map(
    (run) => run.result?.fields
  );
  const replaySemanticResults = replayCpu.report.runs.map(
    (run) => run.result?.fields
  );
  if (JSON.stringify(editedSemanticResults) !==
      JSON.stringify(replaySemanticResults)) {
    throw new Error("R3e imported replay changed semantic Results");
  }

  const tamperedReplay = JSON.parse(replayText);
  tamperedReplay.provenance.programProfileId = "a-circuit:not-mux1";
  let tamperRejected = false;
  try {
    importScenarioReplayBundle(tamperedReplay);
  } catch (error) {
    tamperRejected = String(error).includes("programProfileId");
  }
  if (!tamperRejected) {
    throw new Error("R3e replay provenance mismatch did not fail closed");
  }

  const descriptorTamper = JSON.parse(replayText);
  descriptorTamper.provenance.programProfile.asetSource =
    "audit:unknown-program";
  let descriptorRejected = false;
  try {
    importScenarioReplayBundle(descriptorTamper);
  } catch (error) {
    descriptorRejected = String(error).includes("canonical programProfile");
  }
  if (!descriptorRejected) {
    throw new Error("R3e full program descriptor tamper did not fail closed");
  }

  const scenarioGpu = executeScenarioManifest(
    w,
    scenarioManifest,
    "webgpu"
  );
  if (scenarioGpu.ok ||
      scenarioGpu.report !== null ||
      scenarioGpu.error?.code !== "RUNNER" ||
      scenarioGpu.error?.error?.code !== "UNSUPPORTED_BACKEND" ||
      scenarioGpu.error?.error?.backend !== "webgpu") {
    throw new Error("R2d unsupported WebGPU did not fail closed");
  }

  const recursiveUiSample = "8916<>&".repeat(256);
  const recursiveUiHtml = recursiveStructureHtml(recursiveUiSample);
  if (!recursiveUiHtml.includes('class="proof-recursive-structure"') ||
      recursiveUiHtml.includes("<details class=\"proof-recursive-structure\" open") ||
      !recursiveUiHtml.includes(
        'data-recursive-length="' + recursiveUiSample.length + '"'
      ) ||
      !recursiveUiHtml.includes(recursiveUiSample.length + " символов") ||
      !recursiveUiHtml.includes("&lt;&gt;&amp;") ||
      recursiveUiHtml.includes("8916<>&")) {
    throw new Error("recursive structure UI is not default-collapsed / exact-safe");
  }
  for (const retired of [
    "amemory_i386_lab_proof_available",
    "amemory_i386_lab_proof_json_len",
    "amemory_i386_lab_proof_json_ptr",
    "amemory_i386_lab_proof_json_byte",
    "amemory_i386_lab_value",
    "amemory_i386_lab_value_hi",
    "amemory_i386_lab_writeback",
    "amemory_i386_lab_defined_mask",
    "amemory_i386_lab_value_mask",
    "amemory_i386_lab_undefined_mask",
    "amemory_i386_lab_preserve_mask",
    "amemory_i386_lab_reactions",
    "amemory_i386_lab_links_after_build",
    "amemory_i386_lab_links_after_first",
    "amemory_i386_lab_steady_link_delta",
    "amemory_i386_lab_quiescent",
    "amemory_i386_state_eax_before",
    "amemory_i386_state_ebx_before",
    "amemory_i386_state_ecx_before",
    "amemory_i386_state_edx_before",
    "amemory_i386_state_esi_before",
    "amemory_i386_state_edi_before",
    "amemory_i386_state_ebp_before",
    "amemory_i386_state_esp_before",
    "amemory_i386_state_eip_before",
    "amemory_i386_state_eax_after",
    "amemory_i386_state_ebx_after",
    "amemory_i386_state_ecx_after",
    "amemory_i386_state_edx_after",
    "amemory_i386_state_esi_after",
    "amemory_i386_state_edi_after",
    "amemory_i386_state_ebp_after",
    "amemory_i386_state_esp_after",
    "amemory_i386_state_eip_after",
    "amemory_i386_state_flags_defined_mask",
    "amemory_i386_state_flags_value_mask",
    "amemory_i386_state_flags_undefined_mask",
    "amemory_i386_state_reactions",
    "amemory_i386_state_old_state_retained",
    "amemory_i386_state_atomic_scope",
    "amemory_i386_state_steady_link_delta",
    "amemory_i386_state_quiescent",
    "amemory_i386_memory_offset",
    "amemory_i386_memory_write_value",
    "amemory_i386_memory_before_value",
    "amemory_i386_memory_after_value",
    "amemory_i386_memory_old_after_value",
    "amemory_i386_memory_old_root_ref",
    "amemory_i386_memory_new_root_ref",
    "amemory_i386_memory_reactions",
    "amemory_i386_memory_links_after_load",
    "amemory_i386_memory_links_final",
    "amemory_i386_memory_steady_link_delta",
    "amemory_i386_memory_quiescent",
    "amemory_i386_memory32_address",
    "amemory_i386_memory32_page24",
    "amemory_i386_memory32_offset8",
    "amemory_i386_memory32_write_value",
    "amemory_i386_memory32_before_value",
    "amemory_i386_memory32_after_value",
    "amemory_i386_memory32_old_after_value",
    "amemory_i386_memory32_old_root_ref",
    "amemory_i386_memory32_new_root_ref",
    "amemory_i386_memory32_reactions",
    "amemory_i386_memory32_links_after_load",
    "amemory_i386_memory32_links_final",
    "amemory_i386_memory32_steady_link_delta",
    "amemory_i386_memory32_quiescent",
    "amemory_i386_word_memory_width",
    "amemory_i386_word_memory_address",
    "amemory_i386_word_memory_write_value",
    "amemory_i386_word_memory_before_value",
    "amemory_i386_word_memory_after_value",
    "amemory_i386_word_memory_old_after_value",
    "amemory_i386_word_memory_old_root_ref",
    "amemory_i386_word_memory_new_root_ref",
    "amemory_i386_word_memory_reactions",
    "amemory_i386_word_memory_links_after_load",
    "amemory_i386_word_memory_links_final",
    "amemory_i386_word_memory_steady_link_delta",
    "amemory_i386_word_memory_atomic_scope",
    "amemory_i386_word_memory_crosses_page",
    "amemory_i386_word_memory_quiescent",
    "amemory_i386_fetch_eip_before",
    "amemory_i386_fetch_eip_after",
    "amemory_i386_fetch_byte",
    "amemory_i386_fetch_initial_root_ref",
    "amemory_i386_fetch_final_root_ref",
    "amemory_i386_fetch_seeded_write",
    "amemory_i386_fetch_state_preserved",
    "amemory_i386_fetch_old_state_retained",
    "amemory_i386_fetch_atomic_scope",
    "amemory_i386_fetch_reactions",
    "amemory_i386_fetch_links_after_load",
    "amemory_i386_fetch_links_final",
    "amemory_i386_fetch_steady_link_delta",
    "amemory_i386_fetch_quiescent",
    "amemory_i386_stack_esp_before",
    "amemory_i386_stack_esp_after",
    "amemory_i386_stack_value",
    "amemory_i386_stack_initial_root_ref",
    "amemory_i386_stack_final_root_ref",
    "amemory_i386_stack_state_preserved",
    "amemory_i386_stack_old_state_retained",
    "amemory_i386_stack_old_memory_retained",
    "amemory_i386_stack_atomic_scope",
    "amemory_i386_stack_reactions",
    "amemory_i386_stack_links_after_load",
    "amemory_i386_stack_links_final",
    "amemory_i386_stack_steady_link_delta",
    "amemory_i386_stack_quiescent",
  ]) {
    if (typeof w[retired] !== "undefined") {
      throw new Error("retired schema-v3 browser ABI still exported: " + retired);
    }
  }
  if (w.amemory_i386_lab_probe() !== 0x386) throw new Error("bad lab probe");
  for (const block of registry.blocks) {
    if (w.amemory_i386_lab_supports(block.opcode) !== 1) {
      throw new Error("registry/WASM mismatch: " + block.id);
    }
  }
  const proofCoveredOpcodes = new Set();
  const proofMeasurements = new Map();
  let compactRendererProjectionChecked = false;
  let compactLegacyV1CompatibilityChecked = false;
  let compactMutationMatrixChecked = false;
  let futureScopeBeforeNegativeChecked = false;
  let semanticReplayCorruptionChecked = false;
  const semanticReplayVerifiedBlocks = new Set();
  const jsonBytes = (value) =>
    Buffer.byteLength(JSON.stringify(value), "utf8");
  const readCurrentCompactProof = (label) => {
    if (w.amemory_i386_lab_compact_proof_available() !== 1) {
      throw new Error(label + " compact structural proof JSON missing");
    }
    const len = w.amemory_i386_lab_compact_proof_json_len() >>> 0;
    const ptr = w.amemory_i386_lab_compact_proof_json_ptr() >>> 0;
    if (!len || ptr + len > w.memory.buffer.byteLength) {
      throw new Error(label + " compact proof JSON pointer/length invalid");
    }
    const compact = JSON.parse(
      Buffer.from(w.memory.buffer, ptr, len).toString("utf8")
    );
    if (compact.schemaVersion !== 2 ||
        compact.representationId !== "amemory-proof-compact-json" ||
        compact.representationVersion !== "0.2.0" ||
        compact.sourceProofSchemaVersion !== 4) {
      throw new Error(label + " compact proof representation mismatch");
    }
    if (typeof compact.result?.resultRecursiveWire !== "string" ||
        Object.prototype.hasOwnProperty.call(compact.result, "resultAnum") ||
        typeof compact.result?.resultSequenceAnum !== "string") {
      throw new Error(label + " compact proof result representation mismatch");
    }
    return compact;
  };
  const readCurrentLabResult = (label, expected) => {
    if (w.amemory_i386_lab_result_available() !== 1) {
      throw new Error(label + " structured result JSON missing");
    }
    const len = w.amemory_i386_lab_result_json_len() >>> 0;
    const ptr = w.amemory_i386_lab_result_json_ptr() >>> 0;
    if (!len || ptr + len > w.memory.buffer.byteLength) {
      throw new Error(label + " structured result JSON pointer/length invalid");
    }
    const result = JSON.parse(
      Buffer.from(w.memory.buffer, ptr, len).toString("utf8")
    );
    if (result.schemaVersion !== 1 ||
        result.representationId !== "amemory-i386-lab-result-json" ||
        result.representationVersion !== "0.1.0" ||
        result.instanceId !== 0 ||
        result.witnessKind !== "registry-block") {
      throw new Error(label + " structured result representation mismatch");
    }
    const operation = result.operation || {};
    if ((operation.opcode >>> 0) !== (expected.op >>> 0) ||
        (operation.a >>> 0) !== (expected.a >>> 0) ||
        (operation.b >>> 0) !== (expected.b >>> 0) ||
        (operation.inputFlag >>> 0) !== (expected.flag >>> 0)) {
      throw new Error(label + " structured result request mismatch");
    }
    if (!result.outcome || result.compactProofAvailable !== true) {
      throw new Error(label + " structured result outcome/proof marker missing");
    }
    return result;
  };
  const readCurrentWitnessPayload = (label, expectedKind) => {
    if (w.amemory_i386_lab_result_available() !== 1) {
      throw new Error(label + " structured witness JSON missing");
    }
    const len = w.amemory_i386_lab_result_json_len() >>> 0;
    const ptr = w.amemory_i386_lab_result_json_ptr() >>> 0;
    if (!len || ptr + len > w.memory.buffer.byteLength) {
      throw new Error(label + " structured witness pointer/length invalid");
    }
    const result = JSON.parse(
      Buffer.from(w.memory.buffer, ptr, len).toString("utf8")
    );
    if (result.schemaVersion !== 1 ||
        result.representationId !== "amemory-i386-lab-result-json" ||
        result.representationVersion !== "0.1.0" ||
        result.instanceId !== 0 ||
        result.witnessKind !== expectedKind ||
        result.compactProofAvailable !== true ||
        !result.payload ||
        typeof result.payload !== "object") {
      throw new Error(label + " structured witness envelope mismatch");
    }
    return result.payload;
  };
  if (w.amemory_i386_lab_gpu_carrier_prepare() !== 1) {
    throw new Error("C4c1 real-WASM GPU carrier producer rejected");
  }
  const gpuCarrier = readGpuCarrierWordsAbi(
    w,
    undefined,
    "C4c1 real-WASM carrier"
  );
  if (!gpuCarrier) {
    throw new Error("C4c1 real-WASM GPU carrier missing");
  }
  const gpuCarrierProof = readCurrentCompactProof("C4c1 GPU carrier");
  const gpuCarrierPayload =
    readCurrentWitnessPayload("C4c1 GPU carrier", "gpu-carrier");
  const gpuBase = gpuCarrierProof.topology?.base;
  if (!gpuBase ||
      !Array.isArray(gpuBase.starts) ||
      !Array.isArray(gpuBase.ends) ||
      gpuBase.starts.length !== gpuBase.ends.length) {
    throw new Error("C4c1 compact base topology missing");
  }
  if ((gpuCarrier.layout.linkCount >>> 0) !==
        (gpuCarrierProof.prepare.compiledLinks >>> 0) ||
      (gpuCarrier.layout.linkCount >>> 0) !==
        (gpuCarrierPayload.linkCount >>> 0) ||
      (gpuCarrier.words.length >>> 0) !==
        (gpuCarrierPayload.wordLength >>> 0) ||
      gpuCarrier.layout.rootHandle !== 1) {
    throw new Error("C4c1 carrier/proof metadata identity mismatch");
  }
  if (gpuCarrier.sections.starts.length !== gpuBase.starts.length ||
      gpuCarrier.sections.ends.length !== gpuBase.ends.length) {
    throw new Error("C4c1 carrier/proof topology length mismatch");
  }
  for (let index = 0; index < gpuBase.starts.length; index += 1) {
    if ((gpuCarrier.sections.starts[index] >>> 0) !==
          (gpuBase.starts[index] >>> 0) ||
        (gpuCarrier.sections.ends[index] >>> 0) !==
          (gpuBase.ends[index] >>> 0)) {
      throw new Error(
        "C4c1 carrier/proof topology mismatch at Link " + (index + 1)
      );
    }
  }

  const c4LookupOracle = expectedGpuCarrierLookup(gpuCarrier, {
    handle: Math.min(2, gpuCarrier.layout.linkCount),
    pole: gpuCarrier.layout.rootHandle,
  });
  if (!c4LookupOracle.incidence.length ||
      c4LookupOracle.head !== c4LookupOracle.incidence[0]) {
    throw new Error("C4c2 CPU lookup oracle has invalid ROOT incidence");
  }
  for (const mode of ["single", "sections"]) {
    const shader = gpuCarrierLookupShaderSource(mode, {
      handle: c4LookupOracle.handle,
      pole: c4LookupOracle.pole,
      linkCount: gpuCarrier.layout.linkCount,
    });
    if (!shader.includes("next_by_start") &&
        !shader.includes("next_by_start_offset")) {
      throw new Error("C4c2 " + mode + " shader lost incidence traversal");
    }
    if (!shader.includes("@compute @workgroup_size(1)")) {
      throw new Error("C4c2 " + mode + " shader is not a bounded compute witness");
    }
  }

  const c4ReactionInput =
    deriveGpuCarrierReactionInput(gpuCarrier, gpuCarrierProof.roots);
  const c4ReactionOracle = expectedGpuCarrierReaction(
    gpuCarrier,
    c4ReactionInput
  );

  // Only after independent carrier execution do we inspect the proof trace.
  const c4FirstReaction = gpuCarrierProof.execute?.reactions?.[0];
  const c4ProofAppend = gpuCarrierProof.topology?.append;
  const c4ProofAppendCount = c4FirstReaction
    ? (c4FirstReaction.linksAfter >>> 0) -
      (gpuCarrierProof.load.linksAfterLoad >>> 0)
    : -1;
  const c4SameAppend =
    c4ProofAppendCount === c4ReactionOracle.appendCount &&
    Array.isArray(c4ProofAppend?.starts) &&
    Array.isArray(c4ProofAppend?.ends) &&
    c4ReactionOracle.appendStarts.every(
      (value, index) => value === (c4ProofAppend.starts[index] >>> 0)
    ) &&
    c4ReactionOracle.appendEnds.every(
      (value, index) => value === (c4ProofAppend.ends[index] >>> 0)
    );
  if (!c4FirstReaction ||
      c4FirstReaction.quiescent ||
      c4FirstReaction.scopeBefore?.length !== 1 ||
      c4FirstReaction.scopeAfter?.length !== 1 ||
      (c4FirstReaction.scopeBefore[0] >>> 0) !==
        c4ReactionInput.currentHandle ||
      c4ReactionOracle.candidateHandle !==
        (c4FirstReaction.scopeAfter[0] >>> 0) ||
      c4ReactionOracle.rawRuleMatches !==
        (c4FirstReaction.rawRuleMatches >>> 0) ||
      !c4SameAppend) {
    throw new Error("C4c3 independent publish/append disagrees with MUX1 proof");
  }
  for (const mode of ["single", "sections"]) {
    const shader = gpuCarrierReactionShaderSource(mode, {
      currentHandle: c4ReactionInput.currentHandle,
      interpreterHandle: c4ReactionInput.interpreterHandle,
      linkCount: gpuCarrier.layout.linkCount,
      rootHandle: gpuCarrier.layout.rootHandle,
    });
    for (const marker of [
      "fn discover_rule",
      "fn discover",
      "fn publish",
      "start_head",
      "next_start",
      "ensure_pair_overlay",
      "ensure_start_self_overlay",
      "ensure_end_self_overlay",
      "read_single_output_bundle",
      "grounded_bundle",
    ]) {
      if (!shader.includes(marker)) {
        throw new Error("C4c3 " + mode + " shader missing " + marker);
      }
    }
  }

  const carrierInstance = w.amemory_i386_lab_instance_create() >>> 0;
  if (carrierInstance === 0xffffffff ||
      w.amemory_i386_lab_instance_gpu_carrier_prepare(carrierInstance) !== 1 ||
      w.amemory_i386_lab_instance_gpu_carrier_available(carrierInstance) !== 1) {
    throw new Error("C4c1 instance-scoped carrier prepare failed");
  }
  if (w.amemory_i386_lab_instance_destroy(carrierInstance) !== 1 ||
      w.amemory_i386_lab_instance_gpu_carrier_available(carrierInstance) !== 0) {
    throw new Error("C4c1 destroyed instance retained carrier state");
  }

  const proofMetrics = (proof, registryBlock) => {
    const { visualLinks, ...resultMetadata } = proof.result;
    const memoryId = proof.result.memoryInstanceId;
    if (proof.schemaVersion !== 4 ||
        typeof proof.result.resultRecursiveWire !== "string" ||
        Object.prototype.hasOwnProperty.call(proof.result, "resultAnum") ||
        typeof proof.result.resultSequenceAnum !== "string") {
      throw new Error(registryBlock.id + " source proof result schema mismatch");
    }
    const parseVisualHandle = (key) => {
      const prefix = memoryId + ":L";
      if (typeof key !== "string" || !key.startsWith(prefix)) {
        throw new Error(
          registryBlock.id + " visual topology contains a foreign memory key"
        );
      }
      const handle = Number(key.slice(prefix.length));
      if (!Number.isInteger(handle) || handle < 1) {
        throw new Error(registryBlock.id + " visual topology has invalid handle");
      }
      return handle;
    };
    if (visualLinks.length !== proof.result.linksFinal) {
      throw new Error(registryBlock.id + " visual topology count mismatch");
    }

    const baseCarrier = proof.prepare.carrierDuplets;
    if (baseCarrier.length !== proof.prepare.compiledLinks) {
      throw new Error(registryBlock.id + " prepared carrier count mismatch");
    }
    const appendDelta = [];
    const overlays = [];
    for (let index = 0; index < visualLinks.length; index += 1) {
      const handle = index + 1;
      const visual = visualLinks[index];
      if (visual.localHandle !== handle ||
          parseVisualHandle(visual.key) !== handle) {
        throw new Error(registryBlock.id + " visual topology is not dense");
      }
      const start = parseVisualHandle(visual.startKey);
      const end = parseVisualHandle(visual.endKey);
      if (start > visualLinks.length || end > visualLinks.length) {
        throw new Error(registryBlock.id + " visual pole is out of range");
      }

      if (handle <= baseCarrier.length) {
        const prepared = baseCarrier[index];
        if (prepared.start !== start || prepared.end !== end) {
          throw new Error(
            registryBlock.id + " prepared carrier/runtime prefix mismatch"
          );
        }
      } else {
        appendDelta.push({ start, end });
      }

      if (visual.label || (Array.isArray(visual.tags) && visual.tags.length)) {
        overlays.push({
          localHandle: handle,
          label: visual.label,
          tags: visual.tags,
        });
      }
    }

    const reconstructed = baseCarrier.concat(appendDelta);
    if (reconstructed.length !== proof.result.linksFinal) {
      throw new Error(registryBlock.id + " reconstructed topology count mismatch");
    }
    for (let index = 0; index < reconstructed.length; index += 1) {
      const visual = visualLinks[index];
      if (reconstructed[index].start !== parseVisualHandle(visual.startKey) ||
          reconstructed[index].end !== parseVisualHandle(visual.endKey)) {
        throw new Error(registryBlock.id + " reconstructed topology mismatch");
      }
    }

    const splitPairs = (pairs) => ({
      starts: pairs.map((pair) => pair.start),
      ends: pairs.map((pair) => pair.end),
    });
    const flattenPairs = (pairs) =>
      pairs.flatMap((pair) => [pair.start, pair.end]);

    const splitTopology = {
      schemaVersion: 1,
      representationId: "amemory-runtime-topology-json",
      representationVersion: "0.1.0",
      memoryInstanceId: memoryId,
      base: splitPairs(baseCarrier),
      append: splitPairs(appendDelta),
      overlays,
    };
    const flatTopology = {
      schemaVersion: 1,
      representationId: "amemory-runtime-topology-flat-json",
      representationVersion: "0.1.0",
      memoryInstanceId: memoryId,
      base: flattenPairs(baseCarrier),
      append: flattenPairs(appendDelta),
      overlays,
    };

    const reconstructSplitTopology = (transport) => {
      if (transport.memoryInstanceId !== memoryId) {
        throw new Error(registryBlock.id + " split topology changed memory id");
      }
      const decode = (part) => {
        if (!Array.isArray(part.starts) || !Array.isArray(part.ends) ||
            part.starts.length !== part.ends.length) {
          throw new Error(registryBlock.id + " invalid split topology arrays");
        }
        return part.starts.map((start, index) => ({
          start,
          end: part.ends[index],
        }));
      };
      return decode(transport.base).concat(decode(transport.append));
    };
    const reconstructFlatTopology = (transport) => {
      if (transport.memoryInstanceId !== memoryId) {
        throw new Error(registryBlock.id + " flat topology changed memory id");
      }
      const decode = (part) => {
        if (!Array.isArray(part) || part.length % 2 !== 0) {
          throw new Error(registryBlock.id + " invalid flat topology array");
        }
        const out = [];
        for (let index = 0; index < part.length; index += 2) {
          out.push({ start: part[index], end: part[index + 1] });
        }
        return out;
      };
      return decode(transport.base).concat(decode(transport.append));
    };
    const assertCompactRoundTrip = (candidate, label) => {
      if (candidate.length !== reconstructed.length) {
        throw new Error(registryBlock.id + " " + label + " count mismatch");
      }
      for (let index = 0; index < candidate.length; index += 1) {
        if (candidate[index].start !== reconstructed[index].start ||
            candidate[index].end !== reconstructed[index].end) {
          throw new Error(
            registryBlock.id + " " + label + " topology mismatch"
          );
        }
      }
    };
    assertCompactRoundTrip(
      reconstructSplitTopology(splitTopology),
      "split compact"
    );
    assertCompactRoundTrip(
      reconstructFlatTopology(flatTopology),
      "flat compact"
    );
    for (const overlay of overlays) {
      if (!Number.isInteger(overlay.localHandle) ||
          overlay.localHandle < 1 ||
          overlay.localHandle > reconstructed.length) {
        throw new Error(registryBlock.id + " overlay handle out of range");
      }
    }
    const deriveRecursiveSource = (rootHandle) => {
      const output = [];
      const active = new Set();
      const stack = [{ kind: "visit", handle: rootHandle }];

      while (stack.length !== 0) {
        const frame = stack.pop();
        if (frame.kind === "exit") {
          active.delete(frame.handle);
          continue;
        }

        const handle = frame.handle;
        if (!Number.isInteger(handle) ||
            handle < 1 ||
            handle > baseCarrier.length) {
          throw new Error(
            registryBlock.id + " recursive witness handle out of carrier range"
          );
        }
        const pair = baseCarrier[handle - 1];
        const start = pair.start;
        const end = pair.end;

        if (start === handle && end === handle) {
          output.push("8");
          continue;
        }
        if (active.has(handle)) {
          throw new Error(
            registryBlock.id + " recursive witness is non-well-founded"
          );
        }

        active.add(handle);
        stack.push({ kind: "exit", handle });

        if (start === handle) {
          output.push("9");
          stack.push({ kind: "visit", handle: end });
        } else if (end === handle) {
          output.push("6");
          stack.push({ kind: "visit", handle: start });
        } else {
          output.push("1");
          stack.push({ kind: "visit", handle: end });
          stack.push({ kind: "visit", handle: start });
        }
      }

      return output.join("");
    };

    const preparedRoots = proof.prepare.semanticRoots;
    const loadedRoots = proof.load.semanticRoots;
    if (!Array.isArray(preparedRoots) ||
        !Array.isArray(loadedRoots) ||
        preparedRoots.length !== loadedRoots.length) {
      throw new Error(registryBlock.id + " semantic root stage mismatch");
    }
    const loadedByRole = new Map(
      loadedRoots.map((root) => [root.role, root])
    );
    let derivedRootWitnessCount = 0;
    let derivedRootWitnessBytes = 0;
    const locatorOnlyRoots = preparedRoots.map((root) => {
      if (!Number.isInteger(root.carrierRef) ||
          root.carrierRef < 1 ||
          root.carrierRef > proof.prepare.compiledLinks) {
        throw new Error(registryBlock.id + " invalid semantic root CarrierRef");
      }
      const loaded = loadedByRole.get(root.role);
      if (!loaded ||
          loaded.carrierRef !== root.carrierRef ||
          loaded.localHandle !== root.carrierRef ||
          loaded.source !== root.source) {
        throw new Error(
          registryBlock.id + " semantic root CarrierRef differential mismatch"
        );
      }

      const derivedSource = deriveRecursiveSource(root.carrierRef);
      if (derivedSource !== root.source) {
        throw new Error(
          registryBlock.id + " lazily derived recursive root witness mismatch"
        );
      }
      derivedRootWitnessCount += 1;
      derivedRootWitnessBytes += Buffer.byteLength(derivedSource, "utf8");

      return {
        role: root.role,
        carrierRef: root.carrierRef,
      };
    });
    const recursiveSources = preparedRoots.map(
      (root) => root.source || ""
    );
    const parseLocalRef = (value, maxHandle, label) => {
      if (typeof value !== "string" || !/^L\d+$/.test(value)) {
        throw new Error(registryBlock.id + " invalid " + label + " reference");
      }
      const handle = Number(value.slice(1));
      if (!Number.isInteger(handle) || handle < 1 || handle > maxHandle) {
        throw new Error(registryBlock.id + " out-of-range " + label + " reference");
      }
      return handle;
    };

    const compactTheoryAdmissions = proof.prepare.theoryAdmissions.map(
      (value) => parseLocalRef(value, baseCarrier.length, "Theory")
    );
    if (JSON.stringify(
      compactTheoryAdmissions.map((handle) => "L" + handle)
    ) !== JSON.stringify(proof.prepare.theoryAdmissions)) {
      throw new Error(registryBlock.id + " Theory compact round-trip mismatch");
    }

    const compactReactions = proof.execute.reactions.map((step) => {
      if (step.memoryInstanceId !== memoryId) {
        throw new Error(registryBlock.id + " reaction changed memory id");
      }
      return {
        step: step.step,
        scopeBefore: step.scopeBefore.map(
          (value) => parseLocalRef(value, step.linksAfter, "Scope-before")
        ),
        rawRuleMatches: step.rawRuleMatches,
        transitionedMembers: step.transitionedMembers,
        handoffCount: step.handoffCount,
        scopeAfter: step.scopeAfter.map(
          (value) => parseLocalRef(value, step.linksAfter, "Scope-after")
        ),
        linksAfter: step.linksAfter,
        quiescent: step.quiescent,
      };
    });

    const { memoryInstanceId: resultMemoryId, ...compactResult } =
      resultMetadata;
    if (resultMemoryId !== memoryId ||
        proof.load.memoryInstanceId !== memoryId ||
        proof.execute.memoryInstanceId !== memoryId) {
      throw new Error(registryBlock.id + " compact proof memory id mismatch");
    }

    const compactTopology = {
      base: splitTopology.base,
      append: splitTopology.append,
      overlays,
    };
    const compactProof = {
      schemaVersion: 2,
      representationId: "amemory-proof-compact-json",
      representationVersion: "0.2.0",
      sourceProofSchemaVersion: proof.schemaVersion,
      block: proof.block,
      memoryInstanceId: memoryId,
      prepare: {
        runtimeMemoryExists: proof.prepare.runtimeMemoryExists,
        compiledLinks: proof.prepare.compiledLinks,
      },
      topology: compactTopology,
      roots: locatorOnlyRoots,
      theoryAdmissions: compactTheoryAdmissions,
      load: {
        linksBeforeLoad: proof.load.linksBeforeLoad,
        linksAfterLoad: proof.load.linksAfterLoad,
        importedDuplets: proof.load.importedDuplets,
        carrierRoundTrip: proof.load.carrierRoundTrip,
      },
      execute: {
        activeReactionCount: proof.execute.activeReactionCount,
        finalQuiescent: proof.execute.finalQuiescent,
        reactions: compactReactions,
      },
      result: compactResult,
    };

    const producedCompactProof = readCurrentCompactProof(registryBlock.id);
    if (JSON.stringify(producedCompactProof) !== JSON.stringify(compactProof)) {
      throw new Error(
        registryBlock.id + " Rust compact producer != independent JS shadow"
      );
    }

    const semanticReplayTargets = new Set([
      "mux1",
      "xor32",
      "add32",
      "mul32-raw",
    ]);
    if (semanticReplayTargets.has(registryBlock.id)) {
      const verification = verifyCompactExecutionProof(producedCompactProof);
      if (verification.profileId !==
            "bounded-static-theory-structural-v1" ||
          verification.transportValid !== true ||
          verification.traceConsistent !== true ||
          verification.semanticReplayVerified !== true ||
          verification.admittedRules <= 0 ||
          verification.reactions !==
            producedCompactProof.execute.reactions.length) {
        throw new Error(
          registryBlock.id + " independent semantic replay verification failed"
        );
      }
      semanticReplayVerifiedBlocks.add(registryBlock.id);
    }

    if (registryBlock.id === "mux1" && !semanticReplayCorruptionChecked) {
      const requireCodecValidButReplayRejected = (label, mutate) => {
        const corrupted = JSON.parse(JSON.stringify(producedCompactProof));
        mutate(corrupted);

        validateCompactProof(corrupted);

        let rejected = false;
        try {
          verifyCompactExecutionProof(corrupted);
        } catch {
          rejected = true;
        }
        if (!rejected) {
          throw new Error(
            "MUX1 semantic verifier accepted codec-valid corruption: " + label
          );
        }
      };

      requireCodecValidButReplayRejected("forged-result", (p) => {
        p.result.decodedValue =
          p.result.decodedValue === 0 ? 1 : 0;
        if (Object.prototype.hasOwnProperty.call(p.result, "oracleValue")) {
          p.result.oracleValue = p.result.decodedValue;
        }
        if (Object.prototype.hasOwnProperty.call(p.result, "oracleMatches")) {
          p.result.oracleMatches = true;
        }
      });
      requireCodecValidButReplayRejected("broken-scope-chain", (p) => {
        if (p.execute.reactions.length < 2) {
          throw new Error("MUX1 verifier test needs at least two reactions");
        }
        const current = p.execute.reactions[1].scopeBefore[0];
        p.execute.reactions[1].scopeBefore = [current === 1 ? 2 : 1];
      });
      requireCodecValidButReplayRejected("forged-match-count", (p) => {
        p.execute.reactions[0].rawRuleMatches += 999;
      });
      requireCodecValidButReplayRejected("forged-result-wire", (p) => {
        p.result.resultRecursiveWire = "8";
        p.result.resultSequenceAnum = "8";
      });
      requireCodecValidButReplayRejected("forged-theory-directory", (p) => {
        p.theoryAdmissions = p.theoryAdmissions.slice(1);
      });
      semanticReplayCorruptionChecked = true;
    }

    if (!compactLegacyV1CompatibilityChecked) {
      const legacyCompactProof = JSON.parse(JSON.stringify(producedCompactProof));
      legacyCompactProof.schemaVersion = 1;
      legacyCompactProof.representationVersion = "0.1.0";
      legacyCompactProof.sourceProofSchemaVersion = 3;
      legacyCompactProof.result.resultAnum =
        legacyCompactProof.result.resultRecursiveWire;
      delete legacyCompactProof.result.resultRecursiveWire;

      const legacyRendererProof = inflateCompactProof(legacyCompactProof);
      if (legacyRendererProof.schemaVersion !== 3 ||
          legacyRendererProof.result.resultAnum !==
            producedCompactProof.result.resultRecursiveWire ||
          legacyRendererProof.result.resultSequenceAnum !==
            producedCompactProof.result.resultSequenceAnum ||
          Object.prototype.hasOwnProperty.call(
            legacyRendererProof.result, "resultRecursiveWire"
          )) {
        throw new Error(
          registryBlock.id + " legacy compact v1 compatibility projection failed"
        );
      }
      compactLegacyV1CompatibilityChecked = true;
    }

    const rendererProof = inflateCompactProof(producedCompactProof);
    if (JSON.stringify(rendererProof) !== JSON.stringify(proof)) {
      throw new Error(
        registryBlock.id + " compact renderer projection mismatch"
      );
    }
    compactRendererProjectionChecked = true;

    const rejectCompactMutation = (label, mutate) => {
      const corrupted = JSON.parse(JSON.stringify(producedCompactProof));
      mutate(corrupted);
      let rejected = false;
      try {
        inflateCompactProof(corrupted);
      } catch {
        rejected = true;
      }
      if (!rejected) {
        throw new Error(
          registryBlock.id + " compact-only mutation was accepted: " + label
        );
      }
    };

    if (!compactMutationMatrixChecked) {
      const baseLen = producedCompactProof.topology.base.starts.length;
      const finalLinks = producedCompactProof.result.linksFinal;
      if (baseLen < 2 || producedCompactProof.roots.length === 0 ||
          producedCompactProof.execute.reactions.length === 0) {
        throw new Error(
          registryBlock.id + " cannot exercise compact mutation matrix"
        );
      }
      const mutations = [
        ["schema-version", (p) => { p.schemaVersion += 1; }],
        ["representation-id", (p) => { p.representationId = "invalid"; }],
        ["representation-version", (p) => { p.representationVersion = "invalid"; }],
        ["source-schema", (p) => { p.sourceProofSchemaVersion += 1; }],
        ["empty-memory-id", (p) => { p.memoryInstanceId = ""; }],
        ["base-column-length", (p) => { p.topology.base.ends.pop(); }],
        ["append-column-length", (p) => { p.topology.append.ends.push(1); }],
        ["base-endpoint-range", (p) => { p.topology.base.starts[0] = baseLen + 1; }],
        ["overlay-range", (p) => {
          p.topology.overlays.push({ localHandle: finalLinks + 1, label: null, tags: [] });
        }],
        ["duplicate-overlay", (p) => {
          const sample = p.topology.overlays[0] ??
            { localHandle: 1, label: null, tags: [] };
          p.topology.overlays.push(JSON.parse(JSON.stringify(sample)));
          p.topology.overlays.push(JSON.parse(JSON.stringify(sample)));
        }],
        ["duplicate-root", (p) => {
          p.roots.push(JSON.parse(JSON.stringify(p.roots[0])));
        }],
        ["root-range", (p) => { p.roots[0].carrierRef = baseLen + 1; }],
        ["root-cycle", (p) => {
          const root = p.roots[0].carrierRef;
          const other = root === 1 ? 2 : 1;
          p.topology.base.starts[root - 1] = other;
          p.topology.base.ends[root - 1] = other;
          p.topology.base.starts[other - 1] = root;
          p.topology.base.ends[other - 1] = root;
        }],
        ["theory-range", (p) => { p.theoryAdmissions = [baseLen + 1]; }],
        ["load-count", (p) => { p.load.importedDuplets += 1; }],
        ["reaction-step", (p) => { p.execute.reactions[0].step += 1; }],
        ["reaction-links-decrease", (p) => {
          p.execute.reactions[0].linksAfter = p.load.linksAfterLoad - 1;
        }],
        ["scope-after-range", (p) => {
          p.execute.reactions[0].scopeAfter = [finalLinks + 1];
        }],
        ["active-summary", (p) => { p.execute.activeReactionCount += 1; }],
        ["quiescence-summary", (p) => { p.execute.finalQuiescent = !p.execute.finalQuiescent; }],
        ["missing-result-recursive-wire", (p) => { delete p.result.resultRecursiveWire; }],
        ["mixed-legacy-result-anum", (p) => {
          p.result.resultAnum = p.result.resultRecursiveWire;
        }],
        ["result-links-final", (p) => { p.result.linksFinal += 1; }],
        ["rerun-delta", (p) => { p.result.identicalRerunLinkDelta += 1; }],
        ["redundant-result-memory-id", (p) => { p.result.memoryInstanceId = p.memoryInstanceId; }],
        ["redundant-result-visual-links", (p) => { p.result.visualLinks = []; }],
      ];
      for (const [label, mutate] of mutations) {
        rejectCompactMutation(label, mutate);
      }
      compactMutationMatrixChecked = true;
    }

    if (!futureScopeBeforeNegativeChecked) {
      let beforeGrowing = producedCompactProof.load.linksAfterLoad;
      for (const step of producedCompactProof.execute.reactions) {
        if (step.linksAfter > beforeGrowing) {
          const growingIndex = step.step;
          rejectCompactMutation("future-scope-before", (p) => {
            p.execute.reactions[growingIndex].scopeBefore = [beforeGrowing + 1];
          });
          futureScopeBeforeNegativeChecked = true;
          break;
        }
        beforeGrowing = step.linksAfter;
      }
    }

    const decodedCompactTopology =
      compactTopology.base.starts.map((start, index) => ({
        start,
        end: compactTopology.base.ends[index],
      })).concat(
        compactTopology.append.starts.map((start, index) => ({
          start,
          end: compactTopology.append.ends[index],
        }))
      );
    assertCompactRoundTrip(decodedCompactTopology, "compact proof");

    const overlayByHandle = new Map(
      compactTopology.overlays.map((overlay) => [overlay.localHandle, overlay])
    );
    const inflatedVisualLinks = decodedCompactTopology.map((pair, index) => {
      const handle = index + 1;
      const overlay = overlayByHandle.get(handle);
      return {
        key: memoryId + ":L" + handle,
        startKey: memoryId + ":L" + pair.start,
        endKey: memoryId + ":L" + pair.end,
        localHandle: handle,
        label: overlay?.label ?? null,
        tags: overlay?.tags ?? [],
      };
    });
    if (JSON.stringify(inflatedVisualLinks) !== JSON.stringify(visualLinks)) {
      throw new Error(registryBlock.id + " compact visual DTO mismatch");
    }

    const inflatedPreparedRoots = compactProof.roots.map((root) => ({
      role: root.role,
      carrierRef: root.carrierRef,
      source: deriveRecursiveSource(root.carrierRef),
    }));
    if (JSON.stringify(inflatedPreparedRoots) !== JSON.stringify(preparedRoots)) {
      throw new Error(registryBlock.id + " compact root directory mismatch");
    }
    const inflatedLoadedRoots = compactProof.roots.map((root) => ({
      role: root.role,
      carrierRef: root.carrierRef,
      source: deriveRecursiveSource(root.carrierRef),
      localHandle: root.carrierRef,
    }));
    if (JSON.stringify(inflatedLoadedRoots) !== JSON.stringify(loadedRoots)) {
      throw new Error(registryBlock.id + " compact loaded-root mismatch");
    }

    const inflatedReactions = compactProof.execute.reactions.map((step) => ({
      memoryInstanceId: memoryId,
      step: step.step,
      scopeBefore: step.scopeBefore.map((handle) => "L" + handle),
      rawRuleMatches: step.rawRuleMatches,
      transitionedMembers: step.transitionedMembers,
      handoffCount: step.handoffCount,
      scopeAfter: step.scopeAfter.map((handle) => "L" + handle),
      linksAfter: step.linksAfter,
      quiescent: step.quiescent,
    }));
    if (JSON.stringify(inflatedReactions) !==
        JSON.stringify(proof.execute.reactions)) {
      throw new Error(registryBlock.id + " compact reaction trace mismatch");
    }

    const reactionScopes = proof.execute.reactions.map((step) => ({
      scopeBefore: step.scopeBefore,
      scopeAfter: step.scopeAfter,
    }));
    const carrierDuplets = proof.prepare.carrierDuplets;
    return {
      opcode: registryBlock.opcode,
      id: registryBlock.id,
      block: proof.block,
      schemaVersion: proof.schemaVersion,
      counts: {
        compiledLinks: proof.prepare.compiledLinks,
        carrierDuplets: carrierDuplets.length,
        semanticRoots: proof.prepare.semanticRoots.length,
        theoryAdmissions: proof.prepare.theoryAdmissions.length,
        activeReactions: proof.execute.activeReactionCount,
        traceEntries: proof.execute.reactions.length,
        finalLinks: proof.result.linksFinal,
        visualLinks: visualLinks.length,
      },
      reconstructedRendererProofBytes: jsonBytes(proof),
      compiledLinks: proof.prepare.compiledLinks,
      carrierDuplets: carrierDuplets.length,
      theoryAdmissions: proof.prepare.theoryAdmissions.length,
      activeReactions: proof.execute.activeReactionCount,
      traceEntries: proof.execute.reactions.length,
      finalLinks: proof.result.linksFinal,
      visualLinks: visualLinks.length,
      compactProofPrototype: {
        representationId: compactProof.representationId,
        representationVersion: compactProof.representationVersion,
        proofJsonBytes: jsonBytes(compactProof),
        producerProofJsonBytes: jsonBytes(producedCompactProof),
        browserTransport: "compact-only",
        topologyJsonBytes: jsonBytes(compactProof.topology),
        rootsJsonBytes: jsonBytes(compactProof.roots),
        theoryAdmissionsJsonBytes: jsonBytes(compactProof.theoryAdmissions),
        loadJsonBytes: jsonBytes(compactProof.load),
        executionJsonBytes: jsonBytes(compactProof.execute),
        reactionsJsonBytes: jsonBytes(compactProof.execute.reactions),
        resultJsonBytes: jsonBytes(compactProof.result),
      },
      topologyPrototype: {
        baseLinks: baseCarrier.length,
        appendLinks: appendDelta.length,
        overlayEntries: overlays.length,
        overlayJsonBytes: jsonBytes(overlays),
        currentTopologyJsonBytes:
          jsonBytes(baseCarrier) + jsonBytes(visualLinks),
        splitTopologyJsonBytes: jsonBytes(splitTopology),
        flatTopologyJsonBytes: jsonBytes(flatTopology),
        splitRepresentationId: splitTopology.representationId,
        flatRepresentationId: flatTopology.representationId,
        representationVersion: "0.1.0",
        denseBaseU32FloorBytes: baseCarrier.length * 2 * 4,
        denseAppendU32FloorBytes: appendDelta.length * 2 * 4,
        compactTopologyFloorBytes:
          baseCarrier.length * 2 * 4 +
          appendDelta.length * 2 * 4 +
          jsonBytes(overlays),
      },
      bytes: {
        proofJson: jsonBytes(proof),
        prepareJson: jsonBytes(proof.prepare),
        carrierJson: jsonBytes(carrierDuplets),
        carrierDenseU32Floor: carrierDuplets.length * 2 * 4,
        semanticRootsJson: jsonBytes(preparedRoots),
        semanticRootLocatorOnlyJson: jsonBytes(locatorOnlyRoots),
        derivedRootWitnessCount,
        derivedRootWitnessBytes,
        semanticRootRecursiveSources: recursiveSources.reduce(
          (sum, value) => sum + Buffer.byteLength(value, "utf8"),
          0
        ),
        theoryAdmissionsJson: jsonBytes(proof.prepare.theoryAdmissions),
        loadJson: jsonBytes(proof.load),
        executeJson: jsonBytes(proof.execute),
        reactionsJson: jsonBytes(proof.execute.reactions),
        reactionScopeRefsJson: jsonBytes(reactionScopes),
        resultJson: jsonBytes(proof.result),
        resultMetadataJson: jsonBytes(resultMetadata),
        visualLinksJson: jsonBytes(visualLinks),
      },
    };
  };
  const captureProofMeasurement = (proof, registryBlock) => {
    if (!registryBlock) {
      throw new Error("cannot measure proof without registry block");
    }
    proofMeasurements.set(
      registryBlock.opcode,
      proofMetrics(proof, registryBlock)
    );
  };
  const assertStateEquals = (actual, expected, label) => {
    for (const [name, value] of Object.entries(expected)) {
      if ((actual[name] >>> 0) !== (value >>> 0)) {
        throw new Error(
          label + " " + name.toUpperCase() + " mismatch: " +
          actual[name].toString(16) + " != " + value.toString(16)
        );
      }
    }
  };


  // M5a is deliberately not a new ALU opcode. It is a dedicated
  // architectural-state witness composed from the real ADD32 structural
  // producer and the shared ALU_EFFECT_RESULT/FlagPatch ABI.
  if (typeof w.amemory_i386_state_probe !== "function" ||
      w.amemory_i386_state_probe() !== 0x50a) {
    throw new Error("M5a state WASM probe missing");
  }
  if (w.amemory_i386_state_run_add32() !== 1) {
    throw new Error("M5a real ADD32 state transition rejected");
  }
  const statePayloadM5a =
    readCurrentWitnessPayload("M5a", "architectural-state");
  if (statePayloadM5a.block !== "M5A_STATE_ADD32") {
    throw new Error("M5a structured state block mismatch");
  }
  assertStateEquals(statePayloadM5a.before, {
    eax: 0xffffffff,
    ebx: 0x11223344,
    ecx: 0x01020304,
    edx: 0x55667788,
    esi: 0x11112222,
    edi: 0x33334444,
    ebp: 0x55556666,
    esp: 0x77778888,
    eip: 0x00401000,
  }, "M5a before");
  assertStateEquals(statePayloadM5a.after, {
    eax: 0,
    ebx: 0x11223344,
    ecx: 0x01020304,
    edx: 0x55667788,
    esi: 0x11112222,
    edi: 0x33334444,
    ebp: 0x55556666,
    esp: 0x77778888,
    eip: 0x00401000,
  }, "M5a after");
  if ((statePayloadM5a.flags.defined >>> 0) !== 0x000008d5 ||
      (statePayloadM5a.flags.flagValues >>> 0) !== 0x00000055 ||
      (statePayloadM5a.flags.undefined >>> 0) !== 0) {
    throw new Error("M5a structural EFLAGS successor mismatch");
  }
  if ((statePayloadM5a.oldStateRetained >>> 0) !== 1 ||
      (statePayloadM5a.atomicScope >>> 0) !== 1 ||
      (statePayloadM5a.steadyDelta >>> 0) !== 0 ||
      (statePayloadM5a.quiescent >>> 0) !== 1 ||
      (statePayloadM5a.reactions >>> 0) <= 1) {
    throw new Error("M5a atomic/currentness/rerun witness failed");
  }

  const stateProof =
    inflateCompactProof(readCurrentCompactProof("M5A_STATE_ADD32"));
  if (stateProof.block !== "M5A_STATE_ADD32" ||
      stateProof.schemaVersion !== 4 ||
      !stateProof.result.oracleMatches ||
      (stateProof.result.decodedValue >>> 0) !== 0 ||
      (stateProof.result.decodedValueHi >>> 0) !== 0x11223344 ||
      stateProof.result.identicalRerunLinkDelta !== 0) {
    throw new Error("M5a compact structural proof result mismatch");
  }
  const stateMemoryId = stateProof.load.memoryInstanceId;
  if (!stateMemoryId ||
      stateProof.execute.memoryInstanceId !== stateMemoryId ||
      stateProof.result.memoryInstanceId !== stateMemoryId ||
      !stateProof.execute.reactions.every(
        (step) => step.memoryInstanceId === stateMemoryId
      )) {
    throw new Error("M5a state transition used more than one runtime A-memory");
  }
  if (stateProof.prepare.runtimeMemoryExists !== false ||
      !stateProof.load.carrierRoundTrip ||
      stateProof.load.linksBeforeLoad !== 1 ||
      stateProof.load.linksAfterLoad !== stateProof.prepare.compiledLinks ||
      stateProof.result.linksFinal <= stateProof.load.linksAfterLoad ||
      !stateProof.execute.finalQuiescent ||
      stateProof.execute.activeReactionCount !==
        (statePayloadM5a.reactions >>> 0)) {
    throw new Error("M5a PREPARE/LOAD/EXECUTE pipeline mismatch");
  }
  const stateRoles = new Map(
    stateProof.prepare.semanticRoots.map((root) => [root.role, root])
  );
  for (const role of [
    "function.effect.arithmetic",
    "state.schema.tag",
    "state.apply.frame_tag",
    "state.register.eax",
    "state.register.ebx",
    "state.register.edx",
    "state.flag.undefined",
    "state.before",
    "state.target",
    "state.continuation",
    "result.alu_effect_tag",
    "data.bit.zero",
    "data.bit.one",
    "execution.interpreter",
    "execution.theory",
    "execution.apply",
    "scope.initial",
    "context.result",
  ]) {
    if (!stateRoles.has(role)) {
      throw new Error("M5a prepared Aset missing semantic root " + role);
    }
  }
  if (stateRoles.has("state.after")) {
    throw new Error("M5a host/preparation injected a successor state");
  }
  if (stateRoles.get("state.target").carrierRef !==
      stateRoles.get("state.register.eax").carrierRef) {
    throw new Error("M5a state target is not structural EAX identity");
  }
  if (!stateProof.execute.reactions.every(
    (step) => step.scopeBefore.length === 1 && step.scopeAfter.length === 1
  )) {
    throw new Error("M5a exposed a partial multi-member architectural state");
  }
  const stateBeforeRef = stateRoles.get("state.before").carrierRef;
  const stateBeforeVisualKey = stateMemoryId + ":L" + stateBeforeRef;
  if (!stateProof.result.visualLinks.some(
    (link) => link.key === stateBeforeVisualKey
  )) {
    throw new Error("M5a old state was not retained physically in one A-memory");
  }
  const stateActive = stateProof.execute.reactions.filter(
    (step) => !step.quiescent
  );
  const finalStateScope = stateActive[stateActive.length - 1]?.scopeAfter?.[0];
  const finalStateHandle = typeof finalStateScope === "string" &&
      /^L\d+$/.test(finalStateScope)
    ? Number(finalStateScope.slice(1))
    : 0;
  if (finalStateHandle <= stateProof.load.linksAfterLoad) {
    throw new Error("M5a successor was not structurally materialized at runtime");
  }

  // M5b: the existing real MUL32 effect must publish EDX:EAX + flags
  // as one architectural successor. It is not a new opcode.
  if (typeof w.amemory_i386_state_wide_probe !== "function" ||
      w.amemory_i386_state_wide_probe() !== 0x50b) {
    throw new Error("M5b wide-state WASM probe missing");
  }
  if (w.amemory_i386_state_run_mul32() !== 1) {
    throw new Error("M5b real MUL32 state transition rejected");
  }
  const statePayloadM5b =
    readCurrentWitnessPayload("M5b", "architectural-state");
  if (statePayloadM5b.block !== "M5B_STATE_MUL32") {
    throw new Error("M5b structured state block mismatch");
  }
  assertStateEquals(statePayloadM5b.before, {
    eax: 0xffffffff,
    ebx: 0x11223344,
    ecx: 0x01020304,
    edx: 0xa5a55a5a,
    esi: 0x11112222,
    edi: 0x33334444,
    ebp: 0x55556666,
    esp: 0x77778888,
    eip: 0x00401000,
  }, "M5b before");
  assertStateEquals(statePayloadM5b.after, {
    eax: 0xfffffffe,
    ebx: 0x11223344,
    ecx: 0x01020304,
    edx: 0x00000001,
    esi: 0x11112222,
    edi: 0x33334444,
    ebp: 0x55556666,
    esp: 0x77778888,
    eip: 0x00401000,
  }, "M5b after");
  if ((statePayloadM5b.flags.defined >>> 0) !== 0x00000801 ||
      (statePayloadM5b.flags.flagValues >>> 0) !== 0x00000801 ||
      (statePayloadM5b.flags.undefined >>> 0) !== 0x000000d4) {
    throw new Error("M5b MUL EFLAGS successor mismatch");
  }
  if ((statePayloadM5b.oldStateRetained >>> 0) !== 1 ||
      (statePayloadM5b.atomicScope >>> 0) !== 1 ||
      (statePayloadM5b.steadyDelta >>> 0) !== 0 ||
      (statePayloadM5b.quiescent >>> 0) !== 1 ||
      (statePayloadM5b.reactions >>> 0) <= 2) {
    throw new Error("M5b atomic/currentness/rerun witness failed");
  }

  const wideStateProof =
    inflateCompactProof(readCurrentCompactProof("M5B_STATE_MUL32"));
  if (wideStateProof.block !== "M5B_STATE_MUL32" ||
      wideStateProof.schemaVersion !== 4 ||
      !wideStateProof.result.oracleMatches ||
      (wideStateProof.result.decodedValue >>> 0) !== 0xfffffffe ||
      (wideStateProof.result.decodedValueHi >>> 0) !== 0x00000001 ||
      wideStateProof.result.identicalRerunLinkDelta !== 0) {
    throw new Error("M5b compact structural proof result mismatch");
  }
  const wideMemoryId = wideStateProof.load.memoryInstanceId;
  if (!wideMemoryId ||
      wideStateProof.execute.memoryInstanceId !== wideMemoryId ||
      wideStateProof.result.memoryInstanceId !== wideMemoryId ||
      !wideStateProof.execute.reactions.every(
        (step) => step.memoryInstanceId === wideMemoryId
      )) {
    throw new Error("M5b transition used more than one runtime A-memory");
  }
  if (wideStateProof.prepare.runtimeMemoryExists !== false ||
      !wideStateProof.load.carrierRoundTrip ||
      wideStateProof.load.linksBeforeLoad !== 1 ||
      wideStateProof.load.linksAfterLoad !==
        wideStateProof.prepare.compiledLinks ||
      wideStateProof.result.linksFinal <= wideStateProof.load.linksAfterLoad ||
      !wideStateProof.execute.finalQuiescent ||
      wideStateProof.execute.activeReactionCount !==
        (statePayloadM5b.reactions >>> 0)) {
    throw new Error("M5b PREPARE/LOAD/EXECUTE pipeline mismatch");
  }
  const wideRoles = new Map(
    wideStateProof.prepare.semanticRoots.map((root) => [root.role, root])
  );
  for (const role of [
    "function.effect.mul",
    "state.schema.tag",
    "state.apply.wide_frame_tag",
    "state.register.eax",
    "state.register.ebx",
    "state.register.edx",
    "state.flag.undefined",
    "state.before",
    "state.low_target",
    "state.high_target",
    "state.continuation",
    "result.wide_alu_effect_tag",
    "data.bit.zero",
    "data.bit.one",
    "execution.interpreter",
    "execution.theory",
    "execution.apply",
    "scope.initial",
    "context.result",
  ]) {
    if (!wideRoles.has(role)) {
      throw new Error("M5b prepared Aset missing semantic root " + role);
    }
  }
  if (wideRoles.has("state.after")) {
    throw new Error("M5b host/preparation injected a successor state");
  }
  if (wideRoles.get("state.low_target").carrierRef !==
        wideRoles.get("state.register.eax").carrierRef ||
      wideRoles.get("state.high_target").carrierRef !==
        wideRoles.get("state.register.edx").carrierRef ||
      wideRoles.get("state.low_target").carrierRef ===
        wideRoles.get("state.high_target").carrierRef) {
    throw new Error("M5b wide targets are not distinct structural EAX/EDX");
  }
  if (!wideStateProof.execute.reactions.every(
    (step) => step.scopeBefore.length === 1 && step.scopeAfter.length === 1
  )) {
    throw new Error("M5b exposed a partial multi-destination state");
  }
  const wideBeforeRef = wideRoles.get("state.before").carrierRef;
  if (!wideStateProof.result.visualLinks.some(
    (link) => link.key === wideMemoryId + ":L" + wideBeforeRef
  )) {
    throw new Error("M5b old state was not retained physically");
  }
  const wideActive = wideStateProof.execute.reactions.filter(
    (step) => !step.quiescent
  );
  const wideFinalScope =
    wideActive[wideActive.length - 1]?.scopeAfter?.[0];
  const wideFinalHandle = typeof wideFinalScope === "string" &&
      /^L\d+$/.test(wideFinalScope)
    ? Number(wideFinalScope.slice(1))
    : 0;
  if (wideFinalHandle <= wideStateProof.load.linksAfterLoad) {
    throw new Error("M5b successor was not materialized at runtime");
  }

  // M5c: the same real ADD effect targets ECX in a complete
  // GPR+EIP state. EIP is present and preserved, not an ordinary ALU target.
  if (typeof w.amemory_i386_state_full_probe !== "function" ||
      w.amemory_i386_state_full_probe() !== 0x50c) {
    throw new Error("M5c full-state WASM probe missing");
  }
  if (w.amemory_i386_state_run_add_ecx() !== 1) {
    throw new Error("M5c real ADD->ECX state transition rejected");
  }
  const statePayloadM5c =
    readCurrentWitnessPayload("M5c", "architectural-state");
  if (statePayloadM5c.block !== "M5C_STATE_ADD_ECX") {
    throw new Error("M5c structured state block mismatch");
  }
  assertStateEquals(statePayloadM5c.before, {
    eax: 0x10203040,
    ebx: 0x11223344,
    ecx: 0xffffffff,
    edx: 0x55667788,
    esi: 0x11112222,
    edi: 0x33334444,
    ebp: 0x55556666,
    esp: 0x77778888,
    eip: 0x00401000,
  }, "M5c before");
  assertStateEquals(statePayloadM5c.after, {
    eax: 0x10203040,
    ebx: 0x11223344,
    ecx: 0,
    edx: 0x55667788,
    esi: 0x11112222,
    edi: 0x33334444,
    ebp: 0x55556666,
    esp: 0x77778888,
    eip: 0x00401000,
  }, "M5c after");
  if ((statePayloadM5c.flags.defined >>> 0) !== 0x000008d5 ||
      (statePayloadM5c.flags.flagValues >>> 0) !== 0x00000055 ||
      (statePayloadM5c.flags.undefined >>> 0) !== 0 ||
      (statePayloadM5c.oldStateRetained >>> 0) !== 1 ||
      (statePayloadM5c.atomicScope >>> 0) !== 1 ||
      (statePayloadM5c.steadyDelta >>> 0) !== 0 ||
      (statePayloadM5c.quiescent >>> 0) !== 1) {
    throw new Error("M5c flags/currentness/atomicity witness failed");
  }

  const fullStateProof =
    inflateCompactProof(readCurrentCompactProof("M5C_STATE_ADD_ECX"));
  if (fullStateProof.block !== "M5C_STATE_ADD_ECX" ||
      !fullStateProof.result.oracleMatches ||
      (fullStateProof.result.decodedValue >>> 0) !== 0 ||
      (fullStateProof.result.decodedValueHi >>> 0) !== 0x00401000 ||
      fullStateProof.result.identicalRerunLinkDelta !== 0) {
    throw new Error("M5c compact structural proof result mismatch");
  }
  const fullMemoryId = fullStateProof.load.memoryInstanceId;
  if (!fullMemoryId ||
      fullStateProof.execute.memoryInstanceId !== fullMemoryId ||
      fullStateProof.result.memoryInstanceId !== fullMemoryId ||
      !fullStateProof.execute.reactions.every(
        (step) => step.memoryInstanceId === fullMemoryId &&
          step.scopeBefore.length === 1 &&
          step.scopeAfter.length === 1
      )) {
    throw new Error("M5c full-state proof is not one-memory/atomic");
  }
  const fullRoles = new Map(
    fullStateProof.prepare.semanticRoots.map((root) => [root.role, root])
  );
  for (const role of [
    "function.effect.arithmetic",
    "state.schema.tag",
    "state.apply.frame_tag",
    "state.register.eax",
    "state.register.ebx",
    "state.register.ecx",
    "state.register.edx",
    "state.register.esi",
    "state.register.edi",
    "state.register.ebp",
    "state.register.esp",
    "state.eip",
    "state.before",
    "state.target",
    "state.continuation",
    "result.alu_effect_tag",
    "scope.initial",
    "context.result",
  ]) {
    if (!fullRoles.has(role)) {
      throw new Error("M5c prepared Aset missing semantic root " + role);
    }
  }
  if (fullRoles.has("state.after")) {
    throw new Error("M5c host/preparation injected a successor state");
  }
  if (fullRoles.get("state.target").carrierRef !==
        fullRoles.get("state.register.ecx").carrierRef ||
      fullRoles.get("state.target").carrierRef ===
        fullRoles.get("state.eip").carrierRef) {
    throw new Error("M5c target is not structural ECX or aliases EIP");
  }
  const fullBeforeRef = fullRoles.get("state.before").carrierRef;
  if (!fullStateProof.result.visualLinks.some(
    (link) => link.key === fullMemoryId + ":L" + fullBeforeRef
  )) {
    throw new Error("M5c old full State was not retained physically");
  }
  const fullActive = fullStateProof.execute.reactions.filter(
    (step) => !step.quiescent
  );
  const fullFinalScope =
    fullActive[fullActive.length - 1]?.scopeAfter?.[0];
  const fullFinalHandle = typeof fullFinalScope === "string" &&
      /^L\d+$/.test(fullFinalScope)
    ? Number(fullFinalScope.slice(1))
    : 0;
  if (fullFinalHandle <= fullStateProof.load.linksAfterLoad) {
    throw new Error("M5c successor was not materialized at runtime");
  }

  // M6a: addressable memory is structural and persistent, not a host map.
  if (typeof w.amemory_i386_memory_probe !== "function" ||
      w.amemory_i386_memory_probe() !== 0x60a) {
    throw new Error("M6a structural-memory WASM probe missing");
  }
  if (w.amemory_i386_memory_run(0x25, 0xab) !== 1) {
    throw new Error("M6a structural memory witness rejected");
  }
  const memoryPayloadM6a =
    readCurrentWitnessPayload("M6a", "memory-radix");
  if (memoryPayloadM6a.block !== "M6A_RADIX_PAGE" ||
      (memoryPayloadM6a.address >>> 0) !== 0x25 ||
      (memoryPayloadM6a.offset >>> 0) !== 0x25 ||
      (memoryPayloadM6a.write >>> 0) !== 0xab ||
      (memoryPayloadM6a.before >>> 0) !== 0 ||
      (memoryPayloadM6a.after >>> 0) !== 0xab ||
      (memoryPayloadM6a.oldAfter >>> 0) !== 0 ||
      (memoryPayloadM6a.oldRoot >>> 0) === (memoryPayloadM6a.newRoot >>> 0) ||
      (memoryPayloadM6a.steadyDelta >>> 0) !== 0 ||
      (memoryPayloadM6a.quiescent >>> 0) !== 1 ||
      (memoryPayloadM6a.reactions >>> 0) <= 40) {
    throw new Error("M6a structured memory result/persistence mismatch");
  }
  if (w.amemory_i386_memory_run(256, 0xab) !== 0 ||
      w.amemory_i386_memory_run(0x25, 256) !== 0) {
    throw new Error("M6a WASM ABI accepted out-of-range Byte8/Offset8");
  }

  // Re-run the valid witness because rejected ABI inputs intentionally do not
  // replace the last accepted compact proof.
  if (w.amemory_i386_memory_run(0x25, 0xab) !== 1) {
    throw new Error("M6a valid rerun rejected");
  }
  const memoryPayloadM6aFinal =
    readCurrentWitnessPayload("M6a rerun", "memory-radix");
  const memoryProof =
    inflateCompactProof(readCurrentCompactProof("M6A_RADIX_PAGE"));
  if (memoryProof.block !== "M6A_RADIX_PAGE" ||
      memoryProof.schemaVersion !== 4 ||
      !memoryProof.result.oracleMatches ||
      (memoryProof.result.decodedValue >>> 0) !== 0xab ||
      (memoryProof.result.decodedValueHi >>> 0) !== 0 ||
      memoryProof.result.identicalRerunLinkDelta !== 0) {
    throw new Error("M6a compact structural proof result mismatch");
  }
  const memoryProofId = memoryProof.load.memoryInstanceId;
  if (!memoryProofId ||
      memoryProof.execute.memoryInstanceId !== memoryProofId ||
      memoryProof.result.memoryInstanceId !== memoryProofId ||
      !memoryProof.execute.reactions.every(
        (step) => step.memoryInstanceId === memoryProofId &&
          step.scopeBefore.length === 1 &&
          step.scopeAfter.length === 1
      )) {
    throw new Error("M6a proof is not one-memory/single-scope");
  }
  if (memoryProof.prepare.runtimeMemoryExists !== false ||
      !memoryProof.load.carrierRoundTrip ||
      memoryProof.load.linksBeforeLoad !== 1 ||
      memoryProof.load.linksAfterLoad !== memoryProof.prepare.compiledLinks ||
      !memoryProof.execute.finalQuiescent ||
      memoryProof.execute.activeReactionCount !==
        (memoryPayloadM6aFinal.reactions >>> 0)) {
    throw new Error("M6a PREPARE/LOAD/EXECUTE witness mismatch");
  }
  const memoryRoles = new Map(
    memoryProof.prepare.semanticRoots.map((root) => [root.role, root])
  );
  for (const role of [
    "function.memory.witness",
    "function.memory.read",
    "function.memory.write",
    "memory.zero_root",
    "memory.result_tag",
    "data.offset8",
    "data.byte8",
    "data.bit.zero",
    "data.bit.one",
    "execution.interpreter",
    "execution.theory",
    "execution.apply",
    "scope.initial",
    "context.result",
  ]) {
    if (!memoryRoles.has(role)) {
      throw new Error("M6a prepared Aset missing semantic root " + role);
    }
  }
  const oldRootRef = memoryPayloadM6aFinal.oldRoot >>> 0;
  const newRootRef = memoryPayloadM6aFinal.newRoot >>> 0;
  if (oldRootRef !== memoryRoles.get("memory.zero_root").carrierRef ||
      oldRootRef > memoryProof.load.linksAfterLoad ||
      newRootRef <= memoryProof.load.linksAfterLoad ||
      newRootRef > memoryProof.result.linksFinal) {
    throw new Error("M6a old/new root runtime locator boundary mismatch");
  }
  const oldRootKey = memoryProofId + ":L" + oldRootRef;
  const newRootKey = memoryProofId + ":L" + newRootRef;
  const visualKeysForMemory = new Set(
    memoryProof.result.visualLinks.map((link) => link.key)
  );
  if (!visualKeysForMemory.has(oldRootKey) ||
      !visualKeysForMemory.has(newRootKey)) {
    throw new Error("M6a visual topology omitted old/new page root");
  }
  if ((memoryPayloadM6aFinal.linksAfterLoad >>> 0) !==
        memoryProof.load.linksAfterLoad ||
      (memoryPayloadM6aFinal.linksFinal >>> 0) !==
        memoryProof.result.linksFinal) {
    throw new Error("M6a WASM/proof Link-count mismatch");
  }

  // M6b: full Address32 is structurally split into Page24 + Offset8.
  if (typeof w.amemory_i386_memory32_probe !== "function" ||
      w.amemory_i386_memory32_probe() !== 0x60b) {
    throw new Error("M6b Address32 structural-memory WASM probe missing");
  }
  if (w.amemory_i386_memory32_run(0x00123425, 0xab) !== 1) {
    throw new Error("M6b Address32 structural memory witness rejected");
  }
  const memoryPayloadM6b =
    readCurrentWitnessPayload("M6b", "memory-byte");
  if (memoryPayloadM6b.block !== "M6B_MEMORY32" ||
      (memoryPayloadM6b.address >>> 0) !== 0x00123425 ||
      (memoryPayloadM6b.page24 >>> 0) !== 0x001234 ||
      (memoryPayloadM6b.offset8 >>> 0) !== 0x25 ||
      (memoryPayloadM6b.write >>> 0) !== 0xab ||
      (memoryPayloadM6b.before >>> 0) !== 0 ||
      (memoryPayloadM6b.after >>> 0) !== 0xab ||
      (memoryPayloadM6b.oldAfter >>> 0) !== 0 ||
      (memoryPayloadM6b.oldRoot >>> 0) === (memoryPayloadM6b.newRoot >>> 0) ||
      (memoryPayloadM6b.steadyDelta >>> 0) !== 0 ||
      (memoryPayloadM6b.quiescent >>> 0) !== 1 ||
      (memoryPayloadM6b.reactions >>> 0) <= 80) {
    throw new Error("M6b structured Address32 result/persistence mismatch");
  }

  if (w.amemory_i386_memory32_run(0x00123425, 256) !== 0) {
    throw new Error("M6b WASM ABI accepted out-of-range Byte8");
  }

  // Restore the accepted proof after the deliberate invalid-input check.
  if (w.amemory_i386_memory32_run(0x00123425, 0xab) !== 1) {
    throw new Error("M6b valid rerun rejected");
  }
  const memoryPayloadM6bFinal =
    readCurrentWitnessPayload("M6b rerun", "memory-byte");
  const memory32Proof =
    inflateCompactProof(readCurrentCompactProof("M6B_MEMORY32"));
  if (memory32Proof.block !== "M6B_MEMORY32" ||
      memory32Proof.schemaVersion !== 4 ||
      !memory32Proof.result.oracleMatches ||
      (memory32Proof.result.decodedValue >>> 0) !== 0xab ||
      (memory32Proof.result.decodedValueHi >>> 0) !== 0 ||
      memory32Proof.result.identicalRerunLinkDelta !== 0) {
    throw new Error("M6b compact structural proof result mismatch");
  }
  const memory32Id = memory32Proof.load.memoryInstanceId;
  if (!memory32Id ||
      memory32Proof.execute.memoryInstanceId !== memory32Id ||
      memory32Proof.result.memoryInstanceId !== memory32Id ||
      !memory32Proof.execute.reactions.every(
        (step) => step.memoryInstanceId === memory32Id &&
          step.scopeBefore.length === 1 &&
          step.scopeAfter.length === 1
      )) {
    throw new Error("M6b proof is not one-memory/single-scope");
  }
  if (memory32Proof.prepare.runtimeMemoryExists !== false ||
      !memory32Proof.load.carrierRoundTrip ||
      memory32Proof.load.linksBeforeLoad !== 1 ||
      memory32Proof.load.linksAfterLoad !== memory32Proof.prepare.compiledLinks ||
      !memory32Proof.execute.finalQuiescent ||
      memory32Proof.execute.activeReactionCount !==
        (memoryPayloadM6bFinal.reactions >>> 0)) {
    throw new Error("M6b PREPARE/LOAD/EXECUTE witness mismatch");
  }
  const memory32Roles = new Map(
    memory32Proof.prepare.semanticRoots.map((root) => [root.role, root])
  );
  for (const role of [
    "function.memory32.read",
    "function.memory32.write",
    "memory32.zero_root",
    "memory32.read_result_tag",
    "memory32.write_result_tag",
    "data.address32",
    "data.byte8",
    "data.bit.zero",
    "data.bit.one",
    "execution.interpreter",
    "execution.theory",
    "execution.apply",
    "scope.initial",
    "context.result",
  ]) {
    if (!memory32Roles.has(role)) {
      throw new Error("M6b prepared Aset missing semantic root " + role);
    }
  }
  const oldMemoryRoot = memoryPayloadM6bFinal.oldRoot >>> 0;
  const newMemoryRoot = memoryPayloadM6bFinal.newRoot >>> 0;
  if (oldMemoryRoot !== memory32Roles.get("memory32.zero_root").carrierRef ||
      oldMemoryRoot > memory32Proof.load.linksAfterLoad ||
      newMemoryRoot <= memory32Proof.load.linksAfterLoad ||
      newMemoryRoot > memory32Proof.result.linksFinal) {
    throw new Error("M6b old/new MemoryRoot runtime locator boundary mismatch");
  }
  const memory32Visual = new Set(
    memory32Proof.result.visualLinks.map((link) => link.key)
  );
  if (!memory32Visual.has(memory32Id + ":L" + oldMemoryRoot) ||
      !memory32Visual.has(memory32Id + ":L" + newMemoryRoot)) {
    throw new Error("M6b visual topology omitted old/new MemoryRoot");
  }
  if ((memoryPayloadM6bFinal.linksAfterLoad >>> 0) !==
        memory32Proof.load.linksAfterLoad ||
      (memoryPayloadM6bFinal.linksFinal >>> 0) !==
        memory32Proof.result.linksFinal) {
    throw new Error("M6b WASM/proof Link-count mismatch");
  }

  // M6c: little-endian Word16/Word32 over the same Address32 byte memory.
  if (typeof w.amemory_i386_word_memory_probe !== "function" ||
      w.amemory_i386_word_memory_probe() !== 0x60c) {
    throw new Error("M6c structural word-memory WASM probe missing");
  }

  if (w.amemory_i386_word_memory_run(16, 0x000000ff, 0xabcd) !== 1) {
    throw new Error("M6c Word16 cross-page witness rejected");
  }
  const memoryPayloadM6c16 =
    readCurrentWitnessPayload("M6c Word16", "memory-word");
  if (memoryPayloadM6c16.block !== "M6C_MEMORY_WORD16" ||
      (memoryPayloadM6c16.width >>> 0) !== 16 ||
      (memoryPayloadM6c16.address >>> 0) !== 0x000000ff ||
      (memoryPayloadM6c16.write >>> 0) !== 0xabcd ||
      (memoryPayloadM6c16.before >>> 0) !== 0 ||
      (memoryPayloadM6c16.after >>> 0) !== 0xabcd ||
      (memoryPayloadM6c16.oldAfter >>> 0) !== 0 ||
      (memoryPayloadM6c16.oldRoot >>> 0) === (memoryPayloadM6c16.newRoot >>> 0) ||
      (memoryPayloadM6c16.atomicScope >>> 0) !== 1 ||
      (memoryPayloadM6c16.crossesPage >>> 0) !== 1 ||
      (memoryPayloadM6c16.steadyDelta >>> 0) !== 0 ||
      (memoryPayloadM6c16.quiescent >>> 0) !== 1) {
    throw new Error("M6c Word16 structured cross-page witness failed");
  }
  const word16Proof =
    inflateCompactProof(readCurrentCompactProof("M6C_MEMORY_WORD16"));
  if (word16Proof.block !== "M6C_MEMORY_WORD16" ||
      !word16Proof.result.oracleMatches ||
      (word16Proof.result.decodedValue >>> 0) !== 0xabcd ||
      word16Proof.result.identicalRerunLinkDelta !== 0) {
    throw new Error("M6c Word16 compact proof mismatch");
  }

  if (w.amemory_i386_word_memory_run(32, 0x000000fe, 0x12345678) !== 1) {
    throw new Error("M6c Word32 cross-page witness rejected");
  }
  const memoryPayloadM6c32 =
    readCurrentWitnessPayload("M6c Word32", "memory-word");
  if (memoryPayloadM6c32.block !== "M6C_MEMORY_WORD32" ||
      (memoryPayloadM6c32.width >>> 0) !== 32 ||
      (memoryPayloadM6c32.address >>> 0) !== 0x000000fe ||
      (memoryPayloadM6c32.write >>> 0) !== 0x12345678 ||
      (memoryPayloadM6c32.before >>> 0) !== 0 ||
      (memoryPayloadM6c32.after >>> 0) !== 0x12345678 ||
      (memoryPayloadM6c32.oldAfter >>> 0) !== 0 ||
      (memoryPayloadM6c32.oldRoot >>> 0) === (memoryPayloadM6c32.newRoot >>> 0) ||
      (memoryPayloadM6c32.atomicScope >>> 0) !== 1 ||
      (memoryPayloadM6c32.crossesPage >>> 0) !== 1 ||
      (memoryPayloadM6c32.steadyDelta >>> 0) !== 0 ||
      (memoryPayloadM6c32.quiescent >>> 0) !== 1 ||
      (memoryPayloadM6c32.reactions >>> 0) <= 100) {
    throw new Error("M6c Word32 structured result/atomicity/persistence mismatch");
  }
  if (w.amemory_i386_word_memory_run(16, 0x100, 0x10000) !== 0 ||
      w.amemory_i386_word_memory_run(8, 0x100, 0xab) !== 0) {
    throw new Error("M6c WASM ABI accepted invalid width/value");
  }

  // Restore the accepted Word32 compact proof after negative ABI checks.
  if (w.amemory_i386_word_memory_run(32, 0x000000fe, 0x12345678) !== 1) {
    throw new Error("M6c Word32 valid rerun rejected");
  }
  const memoryPayloadM6cFinal =
    readCurrentWitnessPayload("M6c Word32 rerun", "memory-word");
  const wordProof =
    inflateCompactProof(readCurrentCompactProof("M6C_MEMORY_WORD32"));
  if (wordProof.block !== "M6C_MEMORY_WORD32" ||
      wordProof.schemaVersion !== 4 ||
      !wordProof.result.oracleMatches ||
      (wordProof.result.decodedValue >>> 0) !== 0x12345678 ||
      (wordProof.result.decodedValueHi >>> 0) !== 0 ||
      wordProof.result.identicalRerunLinkDelta !== 0) {
    throw new Error("M6c Word32 compact proof result mismatch");
  }
  const wordMemoryId = wordProof.load.memoryInstanceId;
  if (!wordMemoryId ||
      wordProof.execute.memoryInstanceId !== wordMemoryId ||
      wordProof.result.memoryInstanceId !== wordMemoryId ||
      !wordProof.execute.reactions.every(
        (step) => step.memoryInstanceId === wordMemoryId &&
          step.scopeBefore.length === 1 &&
          step.scopeAfter.length === 1
      )) {
    throw new Error("M6c proof is not one-memory/atomic-scope");
  }
  if (wordProof.prepare.runtimeMemoryExists !== false ||
      !wordProof.load.carrierRoundTrip ||
      wordProof.load.linksBeforeLoad !== 1 ||
      wordProof.load.linksAfterLoad !== wordProof.prepare.compiledLinks ||
      !wordProof.execute.finalQuiescent ||
      wordProof.execute.activeReactionCount !==
        (memoryPayloadM6cFinal.reactions >>> 0)) {
    throw new Error("M6c PREPARE/LOAD/EXECUTE witness mismatch");
  }
  const wordRoles = new Map(
    wordProof.prepare.semanticRoots.map((root) => [root.role, root])
  );
  for (const role of [
    "function.memory_word.read",
    "function.memory_word.write",
    "function.memory_word.address_next",
    "memory_word.zero_root",
    "memory_word.read_result_tag",
    "memory_word.write_result_tag",
    "data.address32",
    "data.word",
    "data.bit.zero",
    "data.bit.one",
    "execution.interpreter",
    "execution.theory",
    "execution.apply",
    "scope.initial",
    "context.result",
  ]) {
    if (!wordRoles.has(role)) {
      throw new Error("M6c prepared Aset missing semantic root " + role);
    }
  }
  const oldWordRoot = memoryPayloadM6cFinal.oldRoot >>> 0;
  const newWordRoot = memoryPayloadM6cFinal.newRoot >>> 0;
  if (oldWordRoot !== wordRoles.get("memory_word.zero_root").carrierRef ||
      oldWordRoot > wordProof.load.linksAfterLoad ||
      newWordRoot <= wordProof.load.linksAfterLoad ||
      newWordRoot > wordProof.result.linksFinal) {
    throw new Error("M6c old/new MemoryRoot runtime locator boundary mismatch");
  }
  const wordVisual = new Set(
    wordProof.result.visualLinks.map((link) => link.key)
  );
  if (!wordVisual.has(wordMemoryId + ":L" + oldWordRoot) ||
      !wordVisual.has(wordMemoryId + ":L" + newWordRoot)) {
    throw new Error("M6c visual topology omitted old/new MemoryRoot");
  }
  if ((memoryPayloadM6cFinal.linksAfterLoad >>> 0) !==
        wordProof.load.linksAfterLoad ||
      (memoryPayloadM6cFinal.linksFinal >>> 0) !==
        wordProof.result.linksFinal) {
    throw new Error("M6c WASM/proof Link-count mismatch");
  }


  // M6d State+MemoryRoot FETCH/STACK: real WASM acceptance, not UI-only wiring.
  if (typeof w.amemory_i386_fetch_probe !== "function" ||
      w.amemory_i386_fetch_probe() !== 0x60d ||
      typeof w.amemory_i386_fetch_run !== "function") {
    throw new Error("M6d2 FETCH WASM ABI missing");
  }
  if (w.amemory_i386_fetch_run(0x000000ff, 0x90, 1) !== 1) {
    throw new Error("M6d2 seeded FETCH real-WASM witness rejected");
  }
  const fetchPayload =
    readCurrentWitnessPayload("M6d2 seeded FETCH", "instruction-fetch");
  if (fetchPayload.block !== "M6D2_FETCH_SEEDED" ||
      (fetchPayload.eipBefore >>> 0) !== 0x000000ff ||
      (fetchPayload.eipAfter >>> 0) !== 0x00000100 ||
      (fetchPayload.byte >>> 0) !== 0x90 ||
      (fetchPayload.seededWrite >>> 0) !== 1 ||
      (fetchPayload.initialRoot >>> 0) === (fetchPayload.finalRoot >>> 0) ||
      (fetchPayload.statePreserved >>> 0) !== 1 ||
      (fetchPayload.oldStateRetained >>> 0) !== 1 ||
      (fetchPayload.atomicScope >>> 0) !== 1 ||
      (fetchPayload.steadyDelta >>> 0) !== 0 ||
      (fetchPayload.quiescent >>> 0) !== 1 ||
      (fetchPayload.reactions >>> 0) === 0) {
    throw new Error("M6d2 seeded FETCH structured State/MemoryRoot witness failed");
  }

  let fetchProof =
    inflateCompactProof(readCurrentCompactProof("M6D2_FETCH_SEEDED"));
  if (fetchProof.block !== "M6D2_FETCH_SEEDED" ||
      fetchProof.schemaVersion !== 4 ||
      !fetchProof.result.oracleMatches ||
      (fetchProof.result.decodedValue >>> 0) !== 0x90 ||
      (fetchProof.result.decodedValueHi >>> 0) !== 0x00000100 ||
      fetchProof.result.identicalRerunLinkDelta !== 0 ||
      !fetchProof.execute.finalQuiescent ||
      fetchProof.execute.activeReactionCount !==
        (fetchPayload.reactions >>> 0)) {
    throw new Error("M6d2 seeded FETCH compact proof mismatch");
  }
  const fetchMemoryId = fetchProof.load.memoryInstanceId;
  if (!fetchMemoryId ||
      fetchProof.execute.memoryInstanceId !== fetchMemoryId ||
      fetchProof.result.memoryInstanceId !== fetchMemoryId ||
      !fetchProof.execute.reactions.every(
        (step) => step.memoryInstanceId === fetchMemoryId &&
          step.scopeBefore.length === 1 &&
          step.scopeAfter.length === 1
      ) ||
      (fetchPayload.linksAfterLoad >>> 0) !==
        fetchProof.load.linksAfterLoad ||
      (fetchPayload.linksFinal >>> 0) !==
        fetchProof.result.linksFinal) {
    throw new Error("M6d2 FETCH proof is not one-memory / one-member Scope");
  }

  // Explicit wrapping fetch is part of M6d2 architectural acceptance.
  if (w.amemory_i386_fetch_run(0xffffffff, 0, 0) !== 1) {
    throw new Error("M6d2 0xffffffff -> 0 FETCH wrap rejected");
  }
  const fetchWrapPayload =
    readCurrentWitnessPayload("M6d2 wrap FETCH", "instruction-fetch");
  if (fetchWrapPayload.block !== "M6D2_FETCH_ZERO" ||
      (fetchWrapPayload.eipBefore >>> 0) !== 0xffffffff ||
      (fetchWrapPayload.eipAfter >>> 0) !== 0 ||
      (fetchWrapPayload.byte >>> 0) !== 0 ||
      (fetchWrapPayload.seededWrite >>> 0) !== 0 ||
      (fetchWrapPayload.initialRoot >>> 0) !==
        (fetchWrapPayload.finalRoot >>> 0)) {
    throw new Error("M6d2 wrap structured payload mismatch");
  }
  fetchProof = inflateCompactProof(readCurrentCompactProof("M6D2_FETCH_ZERO"));
  if (fetchProof.block !== "M6D2_FETCH_ZERO" ||
      !fetchProof.result.oracleMatches ||
      (fetchProof.result.decodedValueHi >>> 0) !== 0) {
    throw new Error("M6d2 wrap compact proof mismatch");
  }
  if (w.amemory_i386_fetch_run(0x100, 0x100, 1) !== 0 ||
      w.amemory_i386_fetch_run(0x100, 0x90, 2) !== 0) {
    throw new Error("M6d2 FETCH ABI accepted invalid Byte8/seed flag");
  }

  // Restore the seeded proof after negative controls.
  if (w.amemory_i386_fetch_run(0x000000ff, 0x90, 1) !== 1) {
    throw new Error("M6d2 seeded FETCH valid rerun rejected");
  }
  fetchProof = inflateCompactProof(readCurrentCompactProof("M6D2_FETCH_SEEDED"));
  if (fetchProof.block !== "M6D2_FETCH_SEEDED" ||
      !fetchProof.result.oracleMatches) {
    throw new Error("M6d2 seeded FETCH proof was not restored");
  }

  if (typeof w.amemory_i386_stack_probe !== "function" ||
      w.amemory_i386_stack_probe() !== 0x60e ||
      typeof w.amemory_i386_stack_run !== "function") {
    throw new Error("M6d3 STACK WASM ABI missing");
  }
  if (w.amemory_i386_stack_run(0x00000103, 0x12345678) !== 1) {
    throw new Error("M6d3 PUSH32->POP32 real-WASM witness rejected");
  }
  const stackPayload =
    readCurrentWitnessPayload("M6d3 STACK", "stack-roundtrip");
  if (stackPayload.block !== "M6D3_STACK_ROUNDTRIP" ||
      (stackPayload.espBefore >>> 0) !== 0x00000103 ||
      (stackPayload.espAfter >>> 0) !== 0x00000103 ||
      (stackPayload.value >>> 0) !== 0x12345678 ||
      (stackPayload.initialRoot >>> 0) === (stackPayload.finalRoot >>> 0) ||
      (stackPayload.statePreserved >>> 0) !== 1 ||
      (stackPayload.oldStateRetained >>> 0) !== 1 ||
      (stackPayload.oldMemoryRetained >>> 0) !== 1 ||
      (stackPayload.atomicScope >>> 0) !== 1 ||
      (stackPayload.steadyDelta >>> 0) !== 0 ||
      (stackPayload.quiescent >>> 0) !== 1 ||
      (stackPayload.reactions >>> 0) === 0) {
    throw new Error("M6d3 structured STACK State/MemoryRoot witness failed");
  }

  const stackProof =
    inflateCompactProof(readCurrentCompactProof("M6D3_STACK_ROUNDTRIP"));
  if (stackProof.block !== "M6D3_STACK_ROUNDTRIP" ||
      stackProof.schemaVersion !== 4 ||
      !stackProof.result.oracleMatches ||
      (stackProof.result.decodedValue >>> 0) !== 0x12345678 ||
      (stackProof.result.decodedValueHi >>> 0) !== 0x00000103 ||
      stackProof.result.identicalRerunLinkDelta !== 0 ||
      !stackProof.execute.finalQuiescent ||
      stackProof.execute.activeReactionCount !==
        (stackPayload.reactions >>> 0)) {
    throw new Error("M6d3 STACK compact proof mismatch");
  }
  const stackMemoryId = stackProof.load.memoryInstanceId;
  if (!stackMemoryId ||
      stackProof.execute.memoryInstanceId !== stackMemoryId ||
      stackProof.result.memoryInstanceId !== stackMemoryId ||
      !stackProof.execute.reactions.every(
        (step) => step.memoryInstanceId === stackMemoryId &&
          step.scopeBefore.length === 1 &&
          step.scopeAfter.length === 1
      ) ||
      (stackPayload.linksAfterLoad >>> 0) !==
        stackProof.load.linksAfterLoad ||
      (stackPayload.linksFinal >>> 0) !==
        stackProof.result.linksFinal) {
    throw new Error("M6d3 STACK proof is not one-memory / one-member Scope");
  }

  // Real WASM execution smoke: composed structural MUX1.
  if (w.amemory_i386_lab_run(12, 1, 0, 1) !== 1) {
    throw new Error("MUX1 WASM execution rejected");
  }
  const muxStructured = readCurrentLabResult("MUX1", {
    op: 12, a: 1, b: 0, flag: 1,
  });
  if ((muxStructured.outcome.value >>> 0) !== 0) {
    throw new Error("MUX1 WASM wrong result");
  }
  if ((muxStructured.outcome.reactions >>> 0) !== 7) {
    throw new Error("MUX1 WASM wrong reaction count");
  }
  if ((muxStructured.outcome.quiescent >>> 0) !== 1) {
    throw new Error("MUX1 WASM did not reach quiescence");
  }
  if ((muxStructured.outcome.steadyLinkDelta >>> 0) !== 0) {
    throw new Error("MUX1 repeated run grew Links");
  }
  const defaultResultBeforeInstances = JSON.stringify(muxStructured);
  const proof = inflateCompactProof(readCurrentCompactProof("MUX1"));
  if (proof.schemaVersion !== 4) {
    throw new Error("MUX1 reconstructed renderer proof schema is not v4");
  }
  proofCoveredOpcodes.add(12);
  captureProofMeasurement(
    proof,
    registry.blocks.find((block) => block.opcode === 12)
  );
  const memoryId = proof.load.memoryInstanceId;
  if (!memoryId || proof.execute.memoryInstanceId !== memoryId || proof.result.memoryInstanceId !== memoryId) {
    throw new Error("proof changed runtime A-memory identity");
  }
  if (proof.prepare.runtimeMemoryExists !== false) {
    throw new Error("runtime A-memory existed during PREPARE stage");
  }
  const roles = new Set(proof.prepare.semanticRoots.map((root) => root.role));
  for (const role of ["function.mux1", "function.dependency.xor2", "function.dependency.and2", "data.select", "data.a", "data.b", "execution.interpreter", "execution.theory", "invocation.call", "scope.initial"]) {
    if (!roles.has(role)) throw new Error("prepared Aset missing semantic root " + role);
  }
  if (!Array.isArray(proof.prepare.theoryAdmissions) || proof.prepare.theoryAdmissions.length === 0) {
    throw new Error("prepared Aset did not expose admitted Structural Rules");
  }
  if (!proof.load.carrierRoundTrip || proof.load.linksBeforeLoad !== 1 || proof.load.linksAfterLoad <= 1) {
    throw new Error("packed carrier load witness failed");
  }
  if (proof.load.linksAfterLoad !== proof.prepare.compiledLinks) {
    throw new Error("runtime A-memory does not contain the complete compiled Aset");
  }
  if (!proof.execute.reactions.every((step) => step.memoryInstanceId === memoryId)) {
    throw new Error("reaction trace used more than one A-memory");
  }
  if (!proof.execute.finalQuiescent || proof.execute.activeReactionCount !== 7) {
    throw new Error("MUX1 proof execution trajectory mismatch");
  }
  if (!proof.result.oracleMatches || proof.result.identicalRerunLinkDelta !== 0) {
    throw new Error("MUX1 proof result/or rerun witness failed");
  }
  if (proof.result.visualLinks.length !== proof.result.linksFinal) {
    throw new Error("visual snapshot does not contain every runtime Link");
  }
  const visualKeys = new Set(proof.result.visualLinks.map((link) => link.key));
  for (const link of proof.result.visualLinks) {
    if (!link.key.startsWith(memoryId + ":L")) throw new Error("visual Link from foreign memory");
    if (!visualKeys.has(link.startKey) || !visualKeys.has(link.endKey)) {
      throw new Error("visual topology has foreign/missing pole");
    }
  }
  // B2: bounded instance table must preserve independent result/proof transports.
  if (typeof w.amemory_i386_lab_instance_create !== "function" ||
      typeof w.amemory_i386_lab_instance_run !== "function" ||
      typeof w.amemory_i386_lab_instance_destroy !== "function" ||
      (w.amemory_i386_lab_instance_capacity() >>> 0) < 3) {
    throw new Error("B2 instance runtime ABI missing");
  }
  const instanceA = w.amemory_i386_lab_instance_create() >>> 0;
  const instanceB = w.amemory_i386_lab_instance_create() >>> 0;
  if (!instanceA || !instanceB ||
      instanceA === 0xffffffff || instanceB === 0xffffffff ||
      instanceA === instanceB) {
    throw new Error("B2 could not allocate two independent instances");
  }

  const readInstanceText = (instanceId, kind) => {
    const prefix = kind === "result"
      ? "amemory_i386_lab_instance_result_"
      : "amemory_i386_lab_instance_compact_proof_";
    const available = w[prefix + "available"];
    const length = w[prefix + "json_len"];
    const pointer = w[prefix + "json_ptr"];
    if (available(instanceId) !== 1) {
      throw new Error("B2 " + kind + " unavailable for instance " + instanceId);
    }
    const len = length(instanceId) >>> 0;
    const ptr = pointer(instanceId) >>> 0;
    if (!len || ptr + len > w.memory.buffer.byteLength) {
      throw new Error("B2 " + kind + " pointer/length invalid");
    }
    return Buffer.from(w.memory.buffer, ptr, len).toString("utf8");
  };

  if (w.amemory_i386_lab_instance_run(instanceA, 12, 1, 0, 1) !== 1) {
    throw new Error("B2 instance A MUX1 run rejected");
  }
  const aResultBefore = readInstanceText(instanceA, "result");
  const aProofBefore = readInstanceText(instanceA, "proof");
  const aResult = JSON.parse(aResultBefore);
  if (aResult.instanceId !== instanceA ||
      aResult.operation.opcode !== 12 ||
      (aResult.outcome.value >>> 0) !== 0 ||
      aResult.compactProofAvailable !== true) {
    throw new Error("B2 instance A structured result mismatch");
  }

  if (w.amemory_i386_lab_instance_run(instanceB, 12, 1, 0, 0) !== 1) {
    throw new Error("B2 instance B MUX1 run rejected");
  }
  const bResultBefore = readInstanceText(instanceB, "result");
  const bProofBefore = readInstanceText(instanceB, "proof");
  const bResult = JSON.parse(bResultBefore);
  if (bResult.instanceId !== instanceB ||
      bResult.operation.opcode !== 12 ||
      (bResult.outcome.value >>> 0) !== 1 ||
      aResultBefore === bResultBefore) {
    throw new Error("B2 instance B structured result mismatch");
  }

  if (readInstanceText(instanceA, "result") !== aResultBefore ||
      readInstanceText(instanceA, "proof") !== aProofBefore) {
    throw new Error("B2 instance B overwrote instance A evidence");
  }
  const defaultResultAfterInstances = JSON.stringify(
    readCurrentLabResult("MUX1 default after instance runs", {
      op: 12, a: 1, b: 0, flag: 1,
    })
  );
  if (defaultResultAfterInstances !== defaultResultBeforeInstances) {
    throw new Error("B2 non-default instance mutated default structured result");
  }

  if (w.amemory_i386_lab_instance_destroy(instanceA) !== 1 ||
      w.amemory_i386_lab_instance_result_available(instanceA) !== 0 ||
      w.amemory_i386_lab_instance_compact_proof_available(instanceA) !== 0 ||
      w.amemory_i386_lab_instance_run(instanceA, 12, 1, 0, 1) !== 0) {
    throw new Error("B2 destroyed instance did not fail closed");
  }
  if (readInstanceText(instanceB, "result") !== bResultBefore ||
      readInstanceText(instanceB, "proof") !== bProofBefore ||
      w.amemory_i386_lab_instance_run(instanceB, 12, 1, 0, 0) !== 1) {
    throw new Error("B2 destroying A disturbed live instance B");
  }
  if (w.amemory_i386_lab_instance_destroy(instanceB) !== 1 ||
      w.amemory_i386_lab_instance_destroy(instanceB) !== 0 ||
      w.amemory_i386_lab_instance_destroy(0) !== 0) {
    throw new Error("B2 instance destruction controls failed");
  }

  const run = (op, a, b, flag, expected, label) => {
    if (w.amemory_i386_lab_run(op, a, b, flag) !== 1) {
      throw new Error(label + " WASM execution rejected");
    }
    const outcome = readCurrentLabResult(
      label, { op, a, b, flag }
    ).outcome;
    if ((outcome.value >>> 0) !== (expected >>> 0)) {
      throw new Error(label + " WASM wrong result");
    }
    if ((outcome.quiescent >>> 0) !== 1) {
      throw new Error(label + " WASM did not reach quiescence");
    }
    if ((outcome.steadyLinkDelta >>> 0) !== 0) {
      throw new Error(label + " repeated run grew Links");
    }
    return outcome;
  };


  const readCurrentProof = (label) => {
    const proof = inflateCompactProof(readCurrentCompactProof(label));
    const registryBlock = registry.blocks.find(
      (block) => block.name === proof.block
    );
    if (!registryBlock) {
      throw new Error(label + " proof block is absent from registry: " + proof.block);
    }
    proofCoveredOpcodes.add(registryBlock.opcode);
    captureProofMeasurement(proof, registryBlock);
    if (!Array.isArray(proof.prepare?.theoryAdmissions) ||
        proof.prepare.theoryAdmissions.length === 0 ||
        !proof.prepare.theoryAdmissions.every(
          (ref) => typeof ref === "string" && /^L\d+$/.test(ref)
        )) {
      throw new Error(label + " Theory admissions are not packed-carrier Link references");
    }
    return proof;
  };

  const verifyLogicProof = (op, a, b, expected, expectedWriteback, block) => {
    const outcome = run(op, a, b, 0, expected, block);
    if ((outcome.writeback >>> 0) !== (expectedWriteback >>> 0)) {
      throw new Error(block + " WASM writeback mismatch");
    }
    const proof = readCurrentProof(block);
    if (proof.block !== block) {
      throw new Error(block + " proof block identity mismatch");
    }

    const memoryId = proof.load.memoryInstanceId;
    if (!memoryId ||
        proof.execute.memoryInstanceId !== memoryId ||
        proof.result.memoryInstanceId !== memoryId) {
      throw new Error(block + " proof changed runtime A-memory identity");
    }
    if (proof.prepare.runtimeMemoryExists !== false) {
      throw new Error(block + " runtime A-memory existed during PREPARE");
    }
    if (!Array.isArray(proof.prepare.theoryAdmissions) ||
        proof.prepare.theoryAdmissions.length === 0) {
      throw new Error(block + " proof did not expose Theory admissions");
    }
    if (!proof.load.carrierRoundTrip ||
        proof.load.linksBeforeLoad !== 1 ||
        proof.load.linksAfterLoad !== proof.prepare.compiledLinks) {
      throw new Error(block + " packed carrier load witness failed");
    }
    if (!proof.execute.finalQuiescent ||
        !proof.execute.reactions.every((step) => step.memoryInstanceId === memoryId)) {
      throw new Error(block + " execution witness is not one-memory/quiescent");
    }
    if (!proof.result.oracleMatches ||
        proof.result.identicalRerunLinkDelta !== 0 ||
        (proof.result.decodedValue >>> 0) !== (expected >>> 0)) {
      throw new Error(block + " structural result/oracle/rerun witness failed");
    }

    const roles = new Set(
      proof.prepare.semanticRoots.map((root) => root.role)
    );
    for (const role of [
      "data.a.word",
      "data.bit.zero",
      "data.bit.one",
      "execution.interpreter",
      "execution.theory",
      "execution.apply",
      "invocation.call",
      "scope.initial",
      "result.tag",
    ]) {
      if (!roles.has(role)) {
        throw new Error(block + " prepared Aset missing semantic root " + role);
      }
    }
    if (op === 4) {
      for (const role of ["function.effect.not", "function.word_not", "function.gate.not1"]) {
        if (!roles.has(role)) {
          throw new Error(block + " prepared Aset missing " + role);
        }
      }
    } else {
      if (!roles.has("function.effect.binary") ||
          !roles.has("function.word_binary") ||
          !roles.has("data.b.word") ||
          !roles.has("data.writeback") ||
          ![...roles].some((role) => role.startsWith("function.gate."))) {
        throw new Error(block + " prepared Aset missing binary logic structure");
      }
      const refByRole = new Map(
        proof.prepare.semanticRoots.map((root) => [root.role, root.carrierRef])
      );
      const expectedBitRole =
        expectedWriteback === 0 ? "data.bit.zero" : "data.bit.one";
      if (refByRole.get("data.writeback") !== refByRole.get(expectedBitRole)) {
        throw new Error(block + " structural writeback bit mismatch");
      }
    }

    if (proof.result.visualLinks.length !== proof.result.linksFinal) {
      throw new Error(block + " visual snapshot does not contain every runtime Link");
    }
    const visualKeys = new Set(
      proof.result.visualLinks.map((link) => link.key)
    );
    for (const link of proof.result.visualLinks) {
      if (!link.key.startsWith(memoryId + ":L") ||
          !visualKeys.has(link.startKey) ||
          !visualKeys.has(link.endKey)) {
        throw new Error(block + " visual topology has a foreign/missing Link pole");
      }
    }
  };

  verifyLogicProof(1, 0xaaaaaaaa, 0x0f0f0f0f, 0x0a0a0a0a, 1, "AND32");
  verifyLogicProof(2, 0xaaaa0000, 0x00005555, 0xaaaa5555, 1, "OR32");
  verifyLogicProof(3, 0xffff0000, 0x0f0f0f0f, 0xf0f00f0f, 1, "XOR32");
  verifyLogicProof(4, 0x12345678, 0, 0xedcba987, 1, "NOT32");
  verifyLogicProof(5, 0xf0f01234, 0x0ff0ffff, 0x00f01234, 0, "TEST32");

  const verifyArithmeticProof = (
    op, a, b, inputFlag, expected, expectedWriteback,
    expectedX, expectedMode, block
  ) => {
    const outcome = run(op, a, b, inputFlag, expected, block);
    if ((outcome.writeback >>> 0) !== (expectedWriteback >>> 0)) {
      throw new Error(block + " WASM writeback mismatch");
    }
    const proof = readCurrentProof(block);
    if (proof.block !== block) {
      throw new Error(block + " proof block identity mismatch");
    }

    const memoryId = proof.load.memoryInstanceId;
    if (!memoryId ||
        proof.execute.memoryInstanceId !== memoryId ||
        proof.result.memoryInstanceId !== memoryId ||
        !proof.execute.reactions.every((step) => step.memoryInstanceId === memoryId)) {
      throw new Error(block + " proof changed runtime A-memory identity");
    }
    if (proof.prepare.runtimeMemoryExists !== false ||
        !proof.load.carrierRoundTrip ||
        proof.load.linksBeforeLoad !== 1 ||
        proof.load.linksAfterLoad !== proof.prepare.compiledLinks) {
      throw new Error(block + " packed carrier witness failed");
    }
    if (!proof.execute.finalQuiescent ||
        !proof.result.oracleMatches ||
        proof.result.identicalRerunLinkDelta !== 0 ||
        (proof.result.decodedValue >>> 0) !== (expected >>> 0)) {
      throw new Error(block + " arithmetic structural result witness failed");
    }

    const refByRole = new Map(
      proof.prepare.semanticRoots.map((root) => [root.role, root.carrierRef])
    );
    for (const role of [
      "function.effect.arithmetic",
      "function.flagged_arithmetic",
      "data.a.word",
      "data.b.word",
      "data.x",
      "data.mode",
      "data.writeback",
      "data.bit.zero",
      "data.bit.one",
      "execution.interpreter",
      "execution.theory",
      "execution.apply",
      "invocation.call",
      "scope.initial",
      "result.tag",
    ]) {
      if (!refByRole.has(role)) {
        throw new Error(block + " prepared Aset missing " + role);
      }
    }
    const bitRef = (value) =>
      refByRole.get(value === 0 ? "data.bit.zero" : "data.bit.one");
    if (refByRole.get("data.x") !== bitRef(expectedX) ||
        refByRole.get("data.mode") !== bitRef(expectedMode) ||
        refByRole.get("data.writeback") !== bitRef(expectedWriteback)) {
      throw new Error(block + " structural X/Mode/WriteBack selector mismatch");
    }

    if (proof.result.visualLinks.length !== proof.result.linksFinal) {
      throw new Error(block + " visual snapshot does not contain every runtime Link");
    }
  };

  verifyArithmeticProof(6, 0xffffffff, 1, 0, 0, 1, 0, 0, "ADD32");
  verifyArithmeticProof(7, 0xffffffff, 0, 1, 0, 1, 1, 0, "ADC32");
  verifyArithmeticProof(8, 0, 1, 0, 0xffffffff, 1, 0, 1, "SUB32");
  verifyArithmeticProof(9, 0, 0, 1, 0xffffffff, 1, 1, 1, "SBB32");
  verifyArithmeticProof(10, 7, 9, 0, 0xfffffffe, 0, 0, 1, "CMP32");

  const verifyMux32Proof = (select, a, b, expected) => {
    run(11, a, b, select, expected, "MUX32");
    const proof = readCurrentProof("MUX32");
    if (proof.block !== "MUX32") {
      throw new Error("MUX32 proof block identity mismatch");
    }
    const memoryId = proof.load.memoryInstanceId;
    if (!memoryId ||
        proof.execute.memoryInstanceId !== memoryId ||
        proof.result.memoryInstanceId !== memoryId ||
        !proof.execute.reactions.every((step) => step.memoryInstanceId === memoryId)) {
      throw new Error("MUX32 proof changed runtime A-memory identity");
    }
    if (!proof.load.carrierRoundTrip ||
        proof.load.linksBeforeLoad !== 1 ||
        proof.load.linksAfterLoad !== proof.prepare.compiledLinks ||
        proof.execute.activeReactionCount !== 257 ||
        !proof.execute.finalQuiescent ||
        !proof.result.oracleMatches ||
        proof.result.identicalRerunLinkDelta !== 0 ||
        (proof.result.decodedValue >>> 0) !== (expected >>> 0)) {
      throw new Error("MUX32 structural proof witness failed");
    }

    const refByRole = new Map(
      proof.prepare.semanticRoots.map((root) => [root.role, root.carrierRef])
    );
    for (const role of [
      "function.mux32",
      "function.mux1",
      "function.gate.xor2",
      "function.gate.and2",
      "data.select",
      "data.a.word",
      "data.b.word",
      "data.bit.zero",
      "data.bit.one",
      "execution.interpreter",
      "execution.theory",
      "execution.apply",
      "invocation.call",
      "scope.initial",
      "result.word_tag",
    ]) {
      if (!refByRole.has(role)) {
        throw new Error("MUX32 prepared Aset missing " + role);
      }
    }
    const selectBit =
      refByRole.get(select === 0 ? "data.bit.zero" : "data.bit.one");
    if (refByRole.get("data.select") !== selectBit) {
      throw new Error("MUX32 structural select mismatch");
    }
    if (proof.result.visualLinks.length !== proof.result.linksFinal) {
      throw new Error("MUX32 visual snapshot is incomplete");
    }
  };

  verifyMux32Proof(1, 0xaaaaaaaa, 0x55555555, 0x55555555);

  const runWide = (op, a, b, expectedLo, expectedHi, reactions, label) => {
    if (w.amemory_i386_lab_run(op, a, b, 0) !== 1) {
      throw new Error(label + " WASM execution rejected");
    }
    const outcome = readCurrentLabResult(
      label, { op, a, b, flag: 0 }
    ).outcome;
    if ((outcome.value >>> 0) !== (expectedLo >>> 0)) {
      throw new Error(label + " WASM wrong low half");
    }
    if ((outcome.valueHi >>> 0) !== (expectedHi >>> 0)) {
      throw new Error(label + " WASM wrong high half");
    }
    if ((outcome.reactions >>> 0) !== (reactions >>> 0)) {
      throw new Error(label + " WASM wrong reaction count");
    }
    if ((outcome.quiescent >>> 0) !== 1) {
      throw new Error(label + " WASM did not reach quiescence");
    }
    if ((outcome.steadyLinkDelta >>> 0) !== 0) {
      throw new Error(label + " repeated run grew Links");
    }
    return outcome;
  };

  const verifyShiftProof = (op, value, count, expected, block) => {
    const outcome = run(
      op, value, count, 0, expected, block + " count=" + count
    );
    const proof = readCurrentProof(block);
    if (proof.block !== block) {
      throw new Error(block + " proof block identity mismatch");
    }
    const memoryId = proof.load.memoryInstanceId;
    if (!memoryId ||
        proof.execute.memoryInstanceId !== memoryId ||
        proof.result.memoryInstanceId !== memoryId ||
        !proof.execute.reactions.every((step) => step.memoryInstanceId === memoryId)) {
      throw new Error(block + " proof changed runtime A-memory identity");
    }
    if (proof.prepare.runtimeMemoryExists !== false ||
        !proof.load.carrierRoundTrip ||
        proof.load.linksBeforeLoad !== 1 ||
        proof.load.linksAfterLoad !== proof.prepare.compiledLinks ||
        !proof.execute.finalQuiescent ||
        !proof.result.oracleMatches ||
        proof.result.identicalRerunLinkDelta !== 0 ||
        (proof.result.decodedValue >>> 0) !== (expected >>> 0)) {
      throw new Error(block + " structural shift witness failed");
    }

    const refByRole = new Map(
      proof.prepare.semanticRoots.map((root) => [root.role, root.carrierRef])
    );
    const witnessByRole = new Map(
      proof.prepare.semanticRoots.map((root) => [root.role, root.source])
    );
    for (const role of [
      "function.shift.selected",
      "function.shift.shl",
      "function.shift.shr",
      "function.shift.sar",
      "data.value.word",
      "data.count.word",
      "execution.interpreter",
      "execution.theory",
      "execution.apply",
      "invocation.call",
      "scope.initial",
      "result.tag",
      "result.flag.set_tag",
      "result.flag.undefined_tag",
    ]) {
      if (!refByRole.has(role)) {
        throw new Error(block + " prepared Aset missing " + role);
      }
    }
    const selectedRole = op === 13
      ? "function.shift.shl"
      : op === 14
        ? "function.shift.shr"
        : "function.shift.sar";
    if (refByRole.get("function.shift.selected") !== refByRole.get(selectedRole)) {
      throw new Error(block + " selected structural function mismatch");
    }
    if (proof.result.visualLinks.length !== proof.result.linksFinal) {
      throw new Error(block + " visual snapshot is incomplete");
    }
    return {
      countRef: refByRole.get("data.count.word"),
      countWitness: witnessByRole.get("data.count.word"),
      value: outcome.value >>> 0,
      defined: outcome.definedMask >>> 0,
      values: outcome.valueMask >>> 0,
      undefined: outcome.undefinedMask >>> 0,
      preserve: outcome.preserveMask >>> 0,
      reactions: outcome.reactions >>> 0,
    };
  };

  const shl1 = verifyShiftProof(13, 0x80000001, 1, 0x00000002, "SHL32");
  verifyShiftProof(14, 0x80000001, 1, 0x40000000, "SHR32");
  verifyShiftProof(15, 0x80000001, 1, 0xc0000000, "SAR32");
  const shl32 = verifyShiftProof(13, 0x80000001, 32, 0x80000001, "SHL32");
  if (shl32.defined !== 0 ||
      shl32.undefined !== 0 ||
      shl32.preserve !== 0x000008d5 ||
      shl32.reactions !== 1) {
    throw new Error("masked-zero SHL32 must preserve all status flags in one structural reaction");
  }
  const shl33 = verifyShiftProof(13, 0x80000001, 33, 0x00000002, "SHL32");
  if (shl1.countWitness === shl33.countWitness) {
    throw new Error("Count8 1 and 33 must be distinct portable structural inputs before masking");
  }
  for (const key of ["value", "defined", "values", "undefined", "preserve", "reactions"]) {
    if (shl1[key] !== shl33[key]) {
      throw new Error("Count8 1 and 33 failed structural masking equivalence: " + key);
    }
  }
  const verifyRotateProof = (
    op, value, count, cfIn, expected, block, carry
  ) => {
    const outcome = run(
      op, value, count, cfIn, expected, block + " count=" + count
    );
    const proof = readCurrentProof(block);
    const memoryId = proof.load.memoryInstanceId;
    if (proof.block !== block ||
        !memoryId ||
        proof.execute.memoryInstanceId !== memoryId ||
        proof.result.memoryInstanceId !== memoryId ||
        !proof.execute.reactions.every((step) => step.memoryInstanceId === memoryId) ||
        !proof.load.carrierRoundTrip ||
        !proof.execute.finalQuiescent ||
        !proof.result.oracleMatches ||
        proof.result.identicalRerunLinkDelta !== 0 ||
        (proof.result.decodedValue >>> 0) !== (expected >>> 0)) {
      throw new Error(block + " structural rotate witness failed");
    }

    const refByRole = new Map(
      proof.prepare.semanticRoots.map((root) => [root.role, root.carrierRef])
    );
    const witnessByRole = new Map(
      proof.prepare.semanticRoots.map((root) => [root.role, root.source])
    );
    const required = carry
      ? [
          "function.rotate_carry.selected",
          "function.rotate_carry.rcl",
          "function.rotate_carry.rcr",
          "function.gate.xor2",
          "data.value.word",
          "data.count.word",
          "data.cf_in",
          "data.bit.zero",
          "data.bit.one",
          "execution.interpreter",
          "execution.theory",
          "execution.apply",
          "invocation.call",
          "scope.initial",
          "result.tag",
          "result.flag.set_tag",
          "result.flag.undefined_tag",
        ]
      : [
          "function.rotate.selected",
          "function.rotate.rol",
          "function.rotate.ror",
          "function.gate.xor2",
          "data.value.word",
          "data.count.word",
          "execution.interpreter",
          "execution.theory",
          "execution.apply",
          "invocation.call",
          "scope.initial",
          "result.tag",
          "result.flag.set_tag",
          "result.flag.undefined_tag",
        ];
    for (const role of required) {
      if (!refByRole.has(role)) {
        throw new Error(block + " prepared Aset missing " + role);
      }
    }

    const selectedRole = carry
      ? (op === 18 ? "function.rotate_carry.rcl" : "function.rotate_carry.rcr")
      : (op === 16 ? "function.rotate.rol" : "function.rotate.ror");
    const selectedKey = carry
      ? "function.rotate_carry.selected"
      : "function.rotate.selected";
    if (refByRole.get(selectedKey) !== refByRole.get(selectedRole)) {
      throw new Error(block + " selected structural function mismatch");
    }
    if (carry) {
      const cfSource = refByRole.get(
        cfIn === 0 ? "data.bit.zero" : "data.bit.one"
      );
      if (refByRole.get("data.cf_in") !== cfSource) {
        throw new Error(block + " CF-in is not a structural bit input");
      }
    }
    if (proof.result.visualLinks.length !== proof.result.linksFinal) {
      throw new Error(block + " visual snapshot is incomplete");
    }

    return {
      countRef: refByRole.get("data.count.word"),
      countWitness: witnessByRole.get("data.count.word"),
      value: outcome.value >>> 0,
      defined: outcome.definedMask >>> 0,
      values: outcome.valueMask >>> 0,
      undefined: outcome.undefinedMask >>> 0,
      preserve: outcome.preserveMask >>> 0,
      reactions: outcome.reactions >>> 0,
    };
  };

  const rol1 = verifyRotateProof(16, 0x80000001, 1, 0, 0x00000003, "ROL32", false);
  verifyRotateProof(17, 0x00000001, 1, 0, 0x80000000, "ROR32", false);
  const rol2 = verifyRotateProof(16, 0x80000001, 2, 0, 0x00000006, "ROL32", false);
  const rol32 = verifyRotateProof(16, 0x80000001, 32, 0, 0x80000001, "ROL32", false);
  const rol33 = verifyRotateProof(16, 0x80000001, 33, 0, 0x00000003, "ROL32", false);
  if (rol1.defined !== 0x00000801 || rol1.undefined !== 0 ||
      rol2.defined !== 0x00000001 || rol2.undefined !== 0x00000800 ||
      rol32.defined !== 0 || rol32.undefined !== 0 ||
      rol32.preserve !== 0x000008d5 || rol32.reactions !== 1 ||
      rol1.countWitness === rol33.countWitness) {
    throw new Error("ROL32 flag/masking proof failed");
  }
  for (const key of ["value", "defined", "values", "undefined", "preserve", "reactions"]) {
    if (rol1[key] !== rol33[key]) {
      throw new Error("ROL32 Count8 1/33 masking mismatch: " + key);
    }
  }

  const rcl1 = verifyRotateProof(18, 0x80000000, 1, 1, 0x00000001, "RCL32", true);
  verifyRotateProof(19, 0x00000001, 1, 0, 0x00000000, "RCR32", true);
  const rcl2 = verifyRotateProof(18, 0x80000000, 2, 1, 0x00000003, "RCL32", true);
  const rcl32 = verifyRotateProof(18, 0x80000000, 32, 1, 0x80000000, "RCL32", true);
  const rcl33 = verifyRotateProof(18, 0x80000000, 33, 1, 0x00000001, "RCL32", true);
  if (rcl1.defined !== 0x00000801 || rcl1.undefined !== 0 ||
      rcl2.defined !== 0x00000001 || rcl2.undefined !== 0x00000800 ||
      rcl32.defined !== 0 || rcl32.undefined !== 0 ||
      rcl32.preserve !== 0x000008d5 || rcl32.reactions !== 1 ||
      rcl1.countWitness === rcl33.countWitness) {
    throw new Error("RCL32 flag/masking proof failed");
  }
  for (const key of ["value", "defined", "values", "undefined", "preserve", "reactions"]) {
    if (rcl1[key] !== rcl33[key]) {
      throw new Error("RCL32 Count8 1/33 masking mismatch: " + key);
    }
  }
  const verifyUnaryProof = (
    op, value, expected, block, cfDefined, cfSet
  ) => {
    const outcome = run(op, value, 0, 0, expected, block);
    const proof = readCurrentProof(block);
    const memoryId = proof.load.memoryInstanceId;
    if (proof.block !== block ||
        !memoryId ||
        proof.execute.memoryInstanceId !== memoryId ||
        proof.result.memoryInstanceId !== memoryId ||
        !proof.execute.reactions.every((step) => step.memoryInstanceId === memoryId) ||
        !proof.load.carrierRoundTrip ||
        !proof.execute.finalQuiescent ||
        !proof.result.oracleMatches ||
        proof.result.identicalRerunLinkDelta !== 0 ||
        (proof.result.decodedValue >>> 0) !== (expected >>> 0)) {
      throw new Error(block + " structural unary witness failed");
    }

    const refByRole = new Map(
      proof.prepare.semanticRoots.map((root) => [root.role, root.carrierRef])
    );
    for (const role of [
      "function.unary.selected",
      "function.unary.inc",
      "function.unary.dec",
      "function.unary.neg",
      "function.flagged_arithmetic",
      "result.flagged_tag",
      "data.value.word",
      "execution.interpreter",
      "execution.theory",
      "execution.apply",
      "invocation.call",
      "scope.initial",
      "result.tag",
      "result.flag.set_tag",
      "result.flag.cf",
      "result.flag.pf",
      "result.flag.af",
      "result.flag.zf",
      "result.flag.sf",
      "result.flag.of",
    ]) {
      if (!refByRole.has(role)) {
        throw new Error(block + " prepared Aset missing " + role);
      }
    }
    const selectedRole = op === 20
      ? "function.unary.inc"
      : op === 21
        ? "function.unary.dec"
        : "function.unary.neg";
    if (refByRole.get("function.unary.selected") !== refByRole.get(selectedRole)) {
      throw new Error(block + " selected structural function mismatch");
    }

    const defined = outcome.definedMask >>> 0;
    const values = outcome.valueMask >>> 0;
    const undefined = outcome.undefinedMask >>> 0;
    const preserve = outcome.preserveMask >>> 0;
    if (undefined !== 0) {
      throw new Error(block + " unexpectedly produced undefined status flags");
    }
    if (cfDefined) {
      if ((defined & 1) !== 1 || (preserve & 1) !== 0 ||
          ((values & 1) !== 0) !== cfSet) {
        throw new Error(block + " CF must be structurally defined by the six-action patch");
      }
    } else {
      if ((defined & 1) !== 0 || (preserve & 1) !== 1) {
        throw new Error(block + " CF must be structurally absent/preserved by the five-action patch");
      }
    }
    if (proof.result.visualLinks.length !== proof.result.linksFinal) {
      throw new Error(block + " visual snapshot is incomplete");
    }
  };

  verifyUnaryProof(20, 0x7fffffff, 0x80000000, "INC32", false, false);
  verifyUnaryProof(21, 0x80000000, 0x7fffffff, "DEC32", false, false);
  verifyUnaryProof(22, 0x00000001, 0xffffffff, "NEG32", true, true);
  verifyUnaryProof(22, 0x00000000, 0x00000000, "NEG32", true, false);

  const rawMulOutcome = runWide(
    23, 0xffffffff, 2, 0xfffffffe, 1, 33 + 1038, "MUL32 raw Wide64"
  );
  if ((rawMulOutcome.preserveMask >>> 0) !== 0x000008d5) {
    throw new Error("MUL32 raw must preserve status flags");
  }
  const rawMulProof = readCurrentProof("MUL32 raw Wide64");
  if (rawMulProof.block !== "MUL32 raw Wide64" ||
      (rawMulProof.result.decodedValue >>> 0) !== 0xfffffffe ||
      (rawMulProof.result.decodedValueHi >>> 0) !== 1 ||
      (rawMulProof.result.oracleValue >>> 0) !== 0xfffffffe ||
      (rawMulProof.result.oracleValueHi >>> 0) !== 1 ||
      !rawMulProof.result.oracleMatches ||
      rawMulProof.result.identicalRerunLinkDelta !== 0 ||
      !rawMulProof.execute.finalQuiescent) {
    throw new Error("MUL32 raw Wide64 structural proof result failed");
  }
  const rawMulRoles = new Set(
    rawMulProof.prepare.semanticRoots.map((root) => root.role)
  );
  for (const role of [
    "function.mul32",
    "function.dependency.add64",
    "result.dependency.add64_tag",
    "data.a.word",
    "data.b.word",
    "execution.interpreter",
    "execution.theory",
    "execution.apply",
    "invocation.call",
    "scope.initial",
    "result.tag",
  ]) {
    if (!rawMulRoles.has(role)) {
      throw new Error("MUL32 raw proof missing semantic root " + role);
    }
  }
  if (rawMulProof.result.visualLinks.length !== rawMulProof.result.linksFinal) {
    throw new Error("MUL32 raw visual snapshot is incomplete");
  }

  const mulEffectOutcome = runWide(
    24, 0xffffffff, 2, 0xfffffffe, 1, 97 + 1038, "x86 MUL32 effect"
  );
  if ((mulEffectOutcome.definedMask >>> 0) !== 0x00000801) {
    throw new Error("x86 MUL32 wrong defined flag mask");
  }
  if ((mulEffectOutcome.valueMask >>> 0) !== 0x00000801) {
    throw new Error("x86 MUL32 must set CF and OF when high half is non-zero");
  }
  if ((mulEffectOutcome.undefinedMask >>> 0) !== 0x000000d4) {
    throw new Error("x86 MUL32 wrong undefined flag mask");
  }
  if ((mulEffectOutcome.preserveMask >>> 0) !== 0) {
    throw new Error("x86 MUL32 must not preserve status flags");
  }
  const mulEffectProof = readCurrentProof("x86 MUL32 effect");
  if (mulEffectProof.block !== "x86 MUL32 effect" ||
      (mulEffectProof.result.decodedValue >>> 0) !== 0xfffffffe ||
      (mulEffectProof.result.decodedValueHi >>> 0) !== 1 ||
      (mulEffectProof.result.oracleValue >>> 0) !== 0xfffffffe ||
      (mulEffectProof.result.oracleValueHi >>> 0) !== 1 ||
      !mulEffectProof.result.oracleMatches ||
      mulEffectProof.result.identicalRerunLinkDelta !== 0 ||
      !mulEffectProof.execute.finalQuiescent) {
    throw new Error("x86 MUL32 effect structural proof result failed");
  }
  const mulEffectRoles = new Set(
    mulEffectProof.prepare.semanticRoots.map((root) => root.role)
  );
  for (const role of [
    "function.effect.mul",
    "function.dependency.mul32",
    "function.dependency.or2",
    "result.dependency.mul32_tag",
    "result.flag.set_tag",
    "result.flag.undefined_tag",
    "result.flag.cf",
    "result.flag.pf",
    "result.flag.af",
    "result.flag.zf",
    "result.flag.sf",
    "result.flag.of",
    "data.a.word",
    "data.b.word",
    "execution.interpreter",
    "execution.theory",
    "execution.apply",
    "invocation.call",
    "scope.initial",
    "result.tag",
  ]) {
    if (!mulEffectRoles.has(role)) {
      throw new Error("x86 MUL32 effect proof missing semantic root " + role);
    }
  }
  if (mulEffectProof.result.visualLinks.length !== mulEffectProof.result.linksFinal) {
    throw new Error("x86 MUL32 effect visual snapshot is incomplete");
  }

  const missingProofOpcodes = registry.blocks
    .map((block) => block.opcode)
    .filter((opcode) => !proofCoveredOpcodes.has(opcode));
  if (missingProofOpcodes.length !== 0) {
    throw new Error(
      "registry opcodes without structural proof: " +
      missingProofOpcodes.join(",")
    );
  }
  if (proofCoveredOpcodes.size !== registry.blocks.length) {
    throw new Error("proof coverage set does not match registry cardinality");
  }

  if (proofMeasurements.size !== registry.blocks.length) {
    throw new Error(
      "proof measurement coverage mismatch: " +
      proofMeasurements.size + "/" + registry.blocks.length
    );
  }
  for (const required of ["mux1", "xor32", "add32", "mul32-raw"]) {
    if (!semanticReplayVerifiedBlocks.has(required)) {
      throw new Error(
        "missing independent semantic replay verification for " + required
      );
    }
  }
  if (!semanticReplayCorruptionChecked) {
    throw new Error("semantic replay corruption matrix was not exercised");
  }
  if (!compactRendererProjectionChecked) {
    throw new Error("compact renderer projection witness was not executed");
  }
  if (!compactMutationMatrixChecked) {
    throw new Error("compact-only mutation matrix was not executed");
  }
  if (!futureScopeBeforeNegativeChecked) {
    throw new Error("future scopeBefore compact negative control was not executed");
  }
  const measuredProofs = [...proofMeasurements.values()]
    .sort((left, right) => left.opcode - right.opcode);
  const sumBytes = (field) =>
    measuredProofs.reduce((sum, item) => sum + item.bytes[field], 0);
  const maxBy = (field) =>
    measuredProofs.reduce((best, item) =>
      item.bytes[field] > best.bytes[field] ? item : best
    );
  const reportPath =
    process.env.AMEMORY_WEB_LAB_REPORT ||
    "experiments/a-circuit/target/web-lab-acceptance-report.json";
  const report = {
    schemaVersion: 1,
    kind: "amemory-web-lab-acceptance-measurements",
    informationalOnly: true,
    sourceSha,
    version,
    wasmSha256: crypto.createHash("sha256").update(bytes).digest("hex"),
    registrySha256: crypto
      .createHash("sha256")
      .update(registryText, "utf8")
      .digest("hex"),
    registryBlocks: registry.blocks.length,
    rendererProofSchemaVersion: rawMulProof.schemaVersion,
    proofCoveredOpcodes: proofCoveredOpcodes.size,
    browserProofTransport: {
      representationId: "amemory-proof-compact-json",
      representationVersion: "0.2.0",
      schemaV3AbiRemoved: true,
      totalTransportJsonBytes: measuredProofs.reduce(
        (sum, item) => sum + item.compactProofPrototype.producerProofJsonBytes,
        0
      ),
    },
    wasmBytes: bytes.length,
    elapsedMs: Number(process.hrtime.bigint() - acceptanceStarted) / 1e6,
    proofMeasurements: measuredProofs,
    proofMeasurementSummary: {
      measuredProofs: measuredProofs.length,
      totalReconstructedRendererProofJsonBytes: sumBytes("proofJson"),
      totalCarrierJsonBytes: sumBytes("carrierJson"),
      totalCarrierDenseU32FloorBytes: sumBytes("carrierDenseU32Floor"),
      totalReactionsJsonBytes: sumBytes("reactionsJson"),
      totalVisualLinksJsonBytes: sumBytes("visualLinksJson"),
      compactProofPrototype: (() => {
        const totalReconstructedRendererProofJsonBytes = sumBytes("proofJson");
        const totalCompactProofJsonBytes = measuredProofs.reduce(
          (sum, item) => sum + item.compactProofPrototype.proofJsonBytes,
          0
        );
        return {
          representationId: "amemory-proof-compact-json",
          representationVersion: "0.2.0",
          totalReconstructedRendererProofJsonBytes,
          totalCompactProofJsonBytes,
          totalProducerCompactProofJsonBytes: measuredProofs.reduce(
            (sum, item) =>
              sum + item.compactProofPrototype.producerProofJsonBytes,
            0
          ),
          reconstructedRendererToCompactRatio:
            totalReconstructedRendererProofJsonBytes / totalCompactProofJsonBytes,
          totalCompactTopologyJsonBytes: measuredProofs.reduce(
            (sum, item) => sum + item.compactProofPrototype.topologyJsonBytes,
            0
          ),
          totalCompactRootsJsonBytes: measuredProofs.reduce(
            (sum, item) => sum + item.compactProofPrototype.rootsJsonBytes,
            0
          ),
          totalCompactReactionsJsonBytes: measuredProofs.reduce(
            (sum, item) => sum + item.compactProofPrototype.reactionsJsonBytes,
            0
          ),
          totalCompactResultJsonBytes: measuredProofs.reduce(
            (sum, item) => sum + item.compactProofPrototype.resultJsonBytes,
            0
          ),
          maxCompactProof: measuredProofs.reduce((best, item) =>
            item.compactProofPrototype.proofJsonBytes >
            best.compactProofPrototype.proofJsonBytes ? item : best
          ),
        };
      })(),
      semanticRootLocatorPrototype: {
        totalFullRootsJsonBytes: sumBytes("semanticRootsJson"),
        totalLocatorOnlyJsonBytes: sumBytes("semanticRootLocatorOnlyJson"),
        totalRecursiveSourceBytes: sumBytes("semanticRootRecursiveSources"),
        totalDerivedWitnesses: measuredProofs.reduce(
          (sum, item) => sum + item.bytes.derivedRootWitnessCount,
          0
        ),
        totalDerivedWitnessBytes: sumBytes("derivedRootWitnessBytes"),
        derivationMismatchCount: 0,
      },
      topologyPrototype: (() => {
        const totalCurrentTopologyJsonBytes = measuredProofs.reduce(
          (sum, item) => sum + item.topologyPrototype.currentTopologyJsonBytes,
          0
        );
        const totalSplitTopologyJsonBytes = measuredProofs.reduce(
          (sum, item) => sum + item.topologyPrototype.splitTopologyJsonBytes,
          0
        );
        const totalFlatTopologyJsonBytes = measuredProofs.reduce(
          (sum, item) => sum + item.topologyPrototype.flatTopologyJsonBytes,
          0
        );
        const totalCompactTopologyFloorBytes = measuredProofs.reduce(
          (sum, item) => sum + item.topologyPrototype.compactTopologyFloorBytes,
          0
        );
        return {
          representationVersion: "0.1.0",
          totalCurrentTopologyJsonBytes,
          totalSplitTopologyJsonBytes,
          totalFlatTopologyJsonBytes,
          totalCompactTopologyFloorBytes,
          currentToSplitRatio:
            totalCurrentTopologyJsonBytes / totalSplitTopologyJsonBytes,
          currentToFlatRatio:
            totalCurrentTopologyJsonBytes / totalFlatTopologyJsonBytes,
          preferredJsonCandidate:
            totalFlatTopologyJsonBytes <= totalSplitTopologyJsonBytes
              ? "amemory-runtime-topology-flat-json"
              : "amemory-runtime-topology-json",
          totalAppendLinks: measuredProofs.reduce(
            (sum, item) => sum + item.topologyPrototype.appendLinks,
            0
          ),
          totalOverlayEntries: measuredProofs.reduce(
            (sum, item) => sum + item.topologyPrototype.overlayEntries,
            0
          ),
        };
      })(),
      maxProofJson: maxBy("proofJson"),
      maxReactionsJson: maxBy("reactionsJson"),
      maxVisualLinksJson: maxBy("visualLinksJson"),
      maxRecursiveRootSources: maxBy("semanticRootRecursiveSources"),
    },
    selectedProofs: {
      rawMul32: proofMeasurements.get(23),
      mul32Effect: proofMeasurements.get(24),
    },
  };
  fs.mkdirSync(path.dirname(reportPath), { recursive: true });
  fs.writeFileSync(reportPath, JSON.stringify(report, null, 2) + "\n");
  console.log("WEB_LAB_ACCEPTANCE_REPORT=" + reportPath);
  console.log(JSON.stringify(report));

  console.log("WASM registry + compact-only one-A-memory proofs for all 24 opcodes PASS");
}).catch((error) => {
  console.error(error);
  process.exit(1);
});
