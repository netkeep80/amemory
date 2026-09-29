import fs from "node:fs";
import {
  createWorkbenchRun,
  deriveWorkbenchPipeline,
  normalizeWorkbenchInputs,
} from "../web/workbench.mjs";

const page = fs.readFileSync(
  "experiments/browser-accelerator/web/index.html",
  "utf8",
);
const source = fs.readFileSync(
  "experiments/browser-accelerator/web/workbench.mjs",
  "utf8",
);

for (const required of [
  'id="workbench-root"',
  'src="./workbench.mjs"',
]) {
  if (!page.includes(required)) {
    throw new Error("Workbench page wiring missing: " + required);
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
  "timing unavailable",
]) {
  if (!source.includes(required)) {
    throw new Error("Workbench real-runtime wiring missing: " + required);
  }
}

const manifest = {
  inputSchema: [
    { key: "S", type: "BIT" },
    { key: "A", type: "BIT" },
    { key: "B", type: "BIT" },
  ],
  initialInputs: { S: 0, A: 0, B: 1 },
  runSequence: [
    {
      runId: "preset-a",
      inputs: { S: 0, A: 0, B: 1 },
      executionMode: "TO_QUIESCENCE",
      assertions: [{ kind: "RESULT_FIELD_EQUALS", field: "value", expected: 0 }],
    },
    {
      runId: "preset-b",
      inputs: { S: 1, A: 0, B: 1 },
      executionMode: "TO_QUIESCENCE",
      assertions: [{ kind: "RESULT_FIELD_EQUALS", field: "value", expected: 1 }],
    },
  ],
};

const normalized = normalizeWorkbenchInputs(
  manifest.inputSchema,
  { S: "1", A: "0", B: "1" },
);
if (JSON.stringify(normalized) !== JSON.stringify({ S: 1, A: 0, B: 1 })) {
  throw new Error("Workbench input normalization mismatch");
}

const preset = createWorkbenchRun(manifest, 1, normalized, 3, "preset");
if (preset.runId !== "workbench-live-3" ||
    preset.assertions.length !== 1 ||
    preset.inputs.S !== 1) {
  throw new Error("Preset Workbench run mismatch");
}

const manual = createWorkbenchRun(
  manifest,
  0,
  { S: 1, A: 1, B: 0 },
  4,
  "manual",
);
if (manual.runId !== "workbench-live-4" ||
    manual.assertions.length !== 0 ||
    manual.inputs.A !== 1) {
  throw new Error("Manual mode retained stale preset assertions");
}

const openPipeline = deriveWorkbenchPipeline({
  prepareCount: 1,
  loadCount: 1,
  baseLinkCount: 123,
}, null);
if (openPipeline[0].state !== "done" ||
    openPipeline[1].state !== "done" ||
    openPipeline.slice(2).some((stage) => stage.state !== "waiting")) {
  throw new Error("Session-open pipeline state mismatch");
}

const runPipeline = deriveWorkbenchPipeline(
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
  },
);
if (runPipeline.some((stage) => stage.state !== "done")) {
  throw new Error("Executed pipeline is not driven by real run data");
}

console.log(
  "Workbench structural model PASS: same manifest preset/manual + retained-runtime wiring",
);
