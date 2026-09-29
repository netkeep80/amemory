import { readJsonAbi } from "./i386-wasm-json.mjs";

const REPLAY_BUNDLE_SCHEMA_VERSION = 1;
const SOURCE_SHA_RE = /^[0-9a-f]{40}$/;
const PROGRAM_FINGERPRINT_RE =
  /^prepared-aset-fnv1a64-v1:[0-9a-f]{16}$/;

const BACKENDS = new Map([
  ["optimized-cpu", 0],
  ["webgpu", 1],
  ["linksdb", 2],
]);

const LIVE_RETENTION_MODES = new Map([
  ["LATEST", 0],
  ["RING", 1],
  ["EXPLICIT_EXPORT", 2],
]);

const REPORT_ABI = {
  available: "amemory_scenario_report_available",
  length: "amemory_scenario_report_json_len",
  pointer: "amemory_scenario_report_json_ptr",
  byte: "amemory_scenario_report_json_byte",
};

const ERROR_ABI = {
  available: "amemory_scenario_error_available",
  length: "amemory_scenario_error_json_len",
  pointer: "amemory_scenario_error_json_ptr",
  byte: "amemory_scenario_error_json_byte",
};

const LIMITS_ABI = {
  available: "amemory_scenario_transport_limits_available",
  length: "amemory_scenario_transport_limits_json_len",
  pointer: "amemory_scenario_transport_limits_json_ptr",
  byte: "amemory_scenario_transport_limits_json_byte",
};

const LIVE_OUTPUT_ABI = {
  available: "amemory_scenario_live_output_available",
  length: "amemory_scenario_live_output_json_len",
  pointer: "amemory_scenario_live_output_json_ptr",
  byte: "amemory_scenario_live_output_json_byte",
};

const LIVE_HISTORY_ABI = {
  available: "amemory_scenario_live_history_available",
  length: "amemory_scenario_live_history_json_len",
  pointer: "amemory_scenario_live_history_json_ptr",
  byte: "amemory_scenario_live_history_json_byte",
};

const LIVE_HISTORY_EXPORT_ABI = {
  available: "amemory_scenario_live_history_export_json_available",
  length: "amemory_scenario_live_history_export_json_len",
  pointer: "amemory_scenario_live_history_export_json_ptr",
  byte: "amemory_scenario_live_history_export_json_byte",
};

function requireFunction(wasm, name) {
  const fn = wasm[name];
  if (typeof fn !== "function") {
    throw new Error(`scenario transport ABI: missing ${name}`);
  }
  return fn;
}

export function readScenarioTransportLimits(wasm) {
  const limits = readJsonAbi(
    wasm,
    LIMITS_ABI,
    "scenario transport limits",
  );
  if (limits === null ||
      limits.schemaVersion !== 1 ||
      !Number.isInteger(limits.maxManifestBytes) ||
      !Number.isInteger(limits.maxSingleReportBytes) ||
      !Number.isInteger(limits.maxErrorBytes) ||
      limits.maxManifestBytes <= 0 ||
      limits.maxSingleReportBytes <= 0 ||
      limits.maxErrorBytes <= 0 ||
      limits.maxRetainedReports !== 1 ||
      limits.maxRetainedErrors !== 1 ||
      limits.retentionMode !== "LATEST" ||
      limits.liveSessionRetained !== true ||
      limits.batchSessionRetained !== false ||
      limits.liveObserver?.schemaVersion !== 1 ||
      limits.liveObserver?.defaultRetentionMode !== "RING" ||
      JSON.stringify(limits.liveObserver?.supportedRetentionModes) !==
        JSON.stringify(["LATEST", "RING", "EXPLICIT_EXPORT"]) ||
      !Number.isInteger(limits.liveObserver?.maxRetainedRuns) ||
      !Number.isInteger(limits.liveObserver?.maxRetainedEvents) ||
      !Number.isInteger(limits.liveObserver?.maxRetainedBytes) ||
      !Number.isInteger(limits.liveObserver?.maxSingleReportBytes) ||
      !Number.isInteger(limits.liveObserver?.maxProfilePoints) ||
      !Number.isInteger(limits.liveObserver?.maxExportBytes) ||
      limits.liveObserver.maxRetainedRuns <= 0 ||
      limits.liveObserver.maxRetainedEvents <= 0 ||
      limits.liveObserver.maxRetainedBytes <= 0 ||
      limits.liveObserver.maxSingleReportBytes <= 0 ||
      limits.liveObserver.maxProfilePoints <= 0 ||
      limits.liveObserver.maxExportBytes <= 0) {
    throw new Error("scenario transport: invalid limits/capability envelope");
  }
  return limits;
}

export function clearScenarioTransportOutput(wasm) {
  requireFunction(wasm, "amemory_scenario_transport_clear_output")();
}

export function writeScenarioManifest(wasm, manifest) {
  const text = typeof manifest === "string"
    ? manifest
    : JSON.stringify(manifest);
  const bytes = new TextEncoder().encode(text);
  const limits = readScenarioTransportLimits(wasm);
  if (bytes.length > limits.maxManifestBytes) {
    throw new Error(
      `scenario transport: manifest ${bytes.length} bytes exceeds ` +
      `${limits.maxManifestBytes}`,
    );
  }

  requireFunction(wasm, "amemory_scenario_manifest_clear")();
  const setByte = requireFunction(wasm, "amemory_scenario_manifest_set_byte");
  for (let index = 0; index < bytes.length; index += 1) {
    if ((setByte(index, bytes[index]) >>> 0) !== 1) {
      throw new Error(`scenario transport ABI: rejected manifest byte ${index}`);
    }
  }
  return bytes.length >>> 0;
}

export function readScenarioReport(wasm) {
  return readJsonAbi(wasm, REPORT_ABI, "scenario report");
}

export function readScenarioError(wasm) {
  return readJsonAbi(wasm, ERROR_ABI, "scenario error");
}

export function executeLoadedScenarioManifest(
  wasm,
  length,
  backend = "optimized-cpu",
) {
  const backendCode = BACKENDS.get(backend);
  if (backendCode === undefined) {
    throw new Error(`scenario transport: unknown backend ${backend}`);
  }
  if (!Number.isInteger(length) || length < 0) {
    throw new Error(`scenario transport: invalid manifest length ${length}`);
  }

  const execute = requireFunction(wasm, "amemory_scenario_execute_json");
  const success = execute(length >>> 0, backendCode) >>> 0;
  const report = readScenarioReport(wasm);
  const error = readScenarioError(wasm);

  if (success === 1) {
    if (report === null || error !== null) {
      throw new Error(
        "scenario transport: success must expose report and no error",
      );
    }
    return { ok: true, report, error: null };
  }

  if (success !== 0 || report !== null || error === null) {
    throw new Error(
      "scenario transport: failure must expose error and no report",
    );
  }
  return { ok: false, report: null, error };
}

export function executeScenarioManifest(
  wasm,
  manifest,
  backend = "optimized-cpu",
) {
  const length = writeScenarioManifest(wasm, manifest);
  return executeLoadedScenarioManifest(wasm, length, backend);
}

export function readScenarioLiveOutput(wasm) {
  return readJsonAbi(wasm, LIVE_OUTPUT_ABI, "scenario live output");
}

export function openScenarioLiveSession(
  wasm,
  manifest,
  backend = "optimized-cpu",
) {
  const backendCode = BACKENDS.get(backend);
  if (backendCode === undefined) {
    throw new Error(`scenario live session: unknown backend ${backend}`);
  }
  const length = writeScenarioManifest(wasm, manifest);
  const open = requireFunction(wasm, "amemory_scenario_live_open_json");
  const success = open(length >>> 0, backendCode) >>> 0;
  const status = readScenarioLiveOutput(wasm);
  const error = readScenarioError(wasm);
  if (success === 1) {
    if (status === null || error !== null) {
      throw new Error(
        "scenario live session: open success must expose status and no error",
      );
    }
    return { ok: true, status, error: null };
  }
  if (success !== 0 || status !== null || error === null) {
    throw new Error(
      "scenario live session: open failure must expose error and no status",
    );
  }
  return { ok: false, status: null, error };
}

export function writeScenarioLiveRun(wasm, run) {
  const text = typeof run === "string" ? run : JSON.stringify(run);
  const bytes = new TextEncoder().encode(text);
  const limits = readScenarioTransportLimits(wasm);
  if (bytes.length > limits.maxManifestBytes) {
    throw new Error(
      `scenario live session: run ${bytes.length} bytes exceeds ` +
      `${limits.maxManifestBytes}`,
    );
  }
  requireFunction(wasm, "amemory_scenario_live_run_clear")();
  const setByte = requireFunction(
    wasm,
    "amemory_scenario_live_run_set_byte",
  );
  for (let index = 0; index < bytes.length; index += 1) {
    if ((setByte(index, bytes[index]) >>> 0) !== 1) {
      throw new Error(
        `scenario live session ABI: rejected run byte ${index}`,
      );
    }
  }
  return bytes.length >>> 0;
}

export function runScenarioLiveSession(wasm, run) {
  const length = writeScenarioLiveRun(wasm, run);
  const execute = requireFunction(
    wasm,
    "amemory_scenario_live_execute_json",
  );
  const success = execute(length) >>> 0;
  const payload = readScenarioLiveOutput(wasm);
  const error = readScenarioError(wasm);
  if (success === 1) {
    if (payload === null || error !== null ||
        payload.schemaVersion !== 1 ||
        payload.status === null ||
        payload.run === null) {
      throw new Error(
        "scenario live session: run success has invalid payload/error state",
      );
    }
    return { ok: true, payload, error: null };
  }
  if (success !== 0 || payload !== null || error === null) {
    throw new Error(
      "scenario live session: run failure must expose error only",
    );
  }
  return { ok: false, payload: null, error };
}

export function refreshScenarioLiveSessionStatus(wasm) {
  const refresh = requireFunction(
    wasm,
    "amemory_scenario_live_status_refresh",
  );
  const success = refresh() >>> 0;
  const status = readScenarioLiveOutput(wasm);
  const error = readScenarioError(wasm);
  if (success === 1) {
    if (status === null || error !== null) {
      throw new Error(
        "scenario live session: status refresh has invalid output",
      );
    }
    return { ok: true, status, error: null };
  }
  if (success !== 0 || status !== null || error === null) {
    throw new Error(
      "scenario live session: status failure must expose error only",
    );
  }
  return { ok: false, status: null, error };
}


export function readScenarioLiveHistoryStatus(wasm) {
  return readJsonAbi(wasm, LIVE_HISTORY_ABI, "scenario live history");
}

export function readScenarioLiveHistoryExport(wasm) {
  return readJsonAbi(
    wasm,
    LIVE_HISTORY_EXPORT_ABI,
    "scenario live history export",
  );
}

function requireLiveHistoryStatus(wasm, label) {
  const status = readScenarioLiveHistoryStatus(wasm);
  const error = readScenarioError(wasm);
  if (status === null || error !== null || status.schemaVersion !== 1) {
    throw new Error(`${label}: invalid status/error state`);
  }
  return status;
}

export function setScenarioLiveRetentionPolicy(
  wasm,
  {
    retentionMode,
    maxRetainedRuns,
    maxRetainedEvents,
    maxRetainedBytes,
  },
) {
  const modeCode = LIVE_RETENTION_MODES.get(retentionMode);
  if (modeCode === undefined) {
    throw new Error(
      `scenario live history: unknown retention mode ${retentionMode}`,
    );
  }
  for (const [name, value] of Object.entries({
    maxRetainedRuns,
    maxRetainedEvents,
    maxRetainedBytes,
  })) {
    if (!Number.isInteger(value) || value <= 0) {
      throw new Error(
        `scenario live history: invalid ${name} ${value}`,
      );
    }
  }
  const setPolicy = requireFunction(
    wasm,
    "amemory_scenario_live_history_policy_set",
  );
  const success = setPolicy(
    modeCode,
    maxRetainedRuns >>> 0,
    maxRetainedEvents >>> 0,
    maxRetainedBytes >>> 0,
  ) >>> 0;
  if (success !== 1) {
    return {
      ok: false,
      status: null,
      error: readScenarioError(wasm),
    };
  }
  return {
    ok: true,
    status: requireLiveHistoryStatus(
      wasm,
      "scenario live history policy",
    ),
    error: null,
  };
}

export function refreshScenarioLiveHistoryStatus(wasm) {
  const refresh = requireFunction(
    wasm,
    "amemory_scenario_live_history_status_refresh",
  );
  const success = refresh() >>> 0;
  if (success !== 1) {
    return {
      ok: false,
      status: null,
      error: readScenarioError(wasm),
    };
  }
  return {
    ok: true,
    status: requireLiveHistoryStatus(
      wasm,
      "scenario live history refresh",
    ),
    error: null,
  };
}

export function clearScenarioLiveHistory(wasm) {
  const clear = requireFunction(
    wasm,
    "amemory_scenario_live_history_clear",
  );
  const success = clear() >>> 0;
  if (success !== 1) {
    return {
      ok: false,
      status: null,
      error: readScenarioError(wasm),
    };
  }
  const refreshed = refreshScenarioLiveHistoryStatus(wasm);
  if (!refreshed.ok) {
    return refreshed;
  }
  return refreshed;
}

function requireHistoryExport(wasm, label) {
  const exported = readScenarioLiveHistoryExport(wasm);
  const error = readScenarioError(wasm);
  if (exported === null ||
      error !== null ||
      exported.schemaVersion !== 1 ||
      exported.representationId !==
        "amemory-live-observer-history-json" ||
      exported.representationVersion !== "0.1.0" ||
      !Array.isArray(exported.runs)) {
    throw new Error(`${label}: invalid export/error state`);
  }
  return exported;
}

export function exportScenarioLiveRun(wasm, sessionRunId) {
  if (!Number.isInteger(sessionRunId) ||
      sessionRunId <= 0 ||
      sessionRunId > 0xffffffff) {
    throw new Error(
      `scenario live history: invalid sessionRunId ${sessionRunId}`,
    );
  }
  const exportRun = requireFunction(
    wasm,
    "amemory_scenario_live_history_export_run",
  );
  const success = exportRun(sessionRunId >>> 0) >>> 0;
  if (success !== 1) {
    return {
      ok: false,
      export: null,
      error: readScenarioError(wasm),
    };
  }
  return {
    ok: true,
    export: requireHistoryExport(
      wasm,
      "scenario live selected-run export",
    ),
    error: null,
  };
}

export function exportScenarioLiveHistory(wasm) {
  const exportAvailable = requireFunction(
    wasm,
    "amemory_scenario_live_history_export_available",
  );
  const success = exportAvailable() >>> 0;
  if (success !== 1) {
    return {
      ok: false,
      export: null,
      error: readScenarioError(wasm),
    };
  }
  return {
    ok: true,
    export: requireHistoryExport(
      wasm,
      "scenario live available-history export",
    ),
    error: null,
  };
}

export function clearScenarioLiveHistoryExport(wasm) {
  requireFunction(
    wasm,
    "amemory_scenario_live_history_export_clear",
  )();
  if (readScenarioLiveHistoryExport(wasm) !== null) {
    throw new Error("scenario live history: export clear failed");
  }
}

export function closeScenarioLiveSession(wasm) {
  const close = requireFunction(wasm, "amemory_scenario_live_close");
  const closed = close() >>> 0;
  const output = readScenarioLiveOutput(wasm);
  const error = readScenarioError(wasm);
  if ((closed !== 0 && closed !== 1) ||
      output !== null ||
      error !== null) {
    throw new Error(
      "scenario live session: close must clear live output and error",
    );
  }
  return closed === 1;
}

function cloneJson(value, label) {
  try {
    return JSON.parse(JSON.stringify(value));
  } catch (error) {
    throw new Error(
      `scenario replay: ${label} is not JSON-serializable: ${error}`,
    );
  }
}

function normalizeManifest(manifest) {
  if (typeof manifest === "string") {
    try {
      return JSON.parse(manifest);
    } catch (error) {
      throw new Error(`scenario replay: invalid manifest JSON: ${error}`);
    }
  }
  if (manifest === null || typeof manifest !== "object" ||
      Array.isArray(manifest)) {
    throw new Error("scenario replay: manifest must be an object");
  }
  return cloneJson(manifest, "manifest");
}

function requireReplayProvenance(manifest, backend, provenance) {
  if (!BACKENDS.has(backend)) {
    throw new Error(`scenario replay: unknown backend ${backend}`);
  }
  if (provenance === null || typeof provenance !== "object" ||
      Array.isArray(provenance)) {
    throw new Error("scenario replay: provenance is required");
  }
  if (typeof provenance.amemoryVersion !== "string" ||
      provenance.amemoryVersion.length === 0) {
    throw new Error("scenario replay: amemoryVersion is required");
  }
  if (!SOURCE_SHA_RE.test(provenance.buildSha ?? "")) {
    throw new Error("scenario replay: exact 40-hex buildSha is required");
  }
  if (!PROGRAM_FINGERPRINT_RE.test(
    provenance.programFingerprint ?? "",
  )) {
    throw new Error(
      "scenario replay: versioned prepared-program fingerprint is required",
    );
  }
  if (provenance.scenarioVersion !== manifest.scenarioVersion) {
    throw new Error("scenario replay: scenarioVersion provenance mismatch");
  }
  if (provenance.programProfileId !==
      manifest.programProfile?.profileId) {
    throw new Error("scenario replay: programProfileId provenance mismatch");
  }
}

export function exportScenarioReplayBundle(manifest, report) {
  const normalizedManifest = normalizeManifest(manifest);
  if (report === null || typeof report !== "object" ||
      Array.isArray(report)) {
    throw new Error("scenario replay: execution report is required");
  }
  if (report.scenarioVersion !== normalizedManifest.scenarioVersion ||
      report.programProfile?.profileId !==
        normalizedManifest.programProfile?.profileId) {
    throw new Error("scenario replay: report does not match manifest");
  }

  const backend = report.backend;
  const provenance = cloneJson(report.provenance, "provenance");
  requireReplayProvenance(normalizedManifest, backend, provenance);

  return {
    schemaVersion: REPLAY_BUNDLE_SCHEMA_VERSION,
    manifest: normalizedManifest,
    backend,
    provenance,
  };
}

export function importScenarioReplayBundle(source) {
  let bundle;
  if (typeof source === "string") {
    try {
      bundle = JSON.parse(source);
    } catch (error) {
      throw new Error(`scenario replay: invalid bundle JSON: ${error}`);
    }
  } else {
    bundle = cloneJson(source, "bundle");
  }

  if (bundle === null || typeof bundle !== "object" ||
      Array.isArray(bundle) ||
      bundle.schemaVersion !== REPLAY_BUNDLE_SCHEMA_VERSION) {
    throw new Error(
      `scenario replay: expected schemaVersion ${REPLAY_BUNDLE_SCHEMA_VERSION}`,
    );
  }

  const manifest = normalizeManifest(bundle.manifest);
  const provenance = cloneJson(bundle.provenance, "provenance");
  requireReplayProvenance(manifest, bundle.backend, provenance);

  return {
    schemaVersion: REPLAY_BUNDLE_SCHEMA_VERSION,
    manifest,
    backend: bundle.backend,
    provenance,
  };
}
