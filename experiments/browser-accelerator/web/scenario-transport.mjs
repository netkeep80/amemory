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
      limits.liveSessionRetained !== false) {
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
