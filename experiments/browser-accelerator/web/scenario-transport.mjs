import { readJsonAbi } from "./i386-wasm-json.mjs";

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

function requireFunction(wasm, name) {
  const fn = wasm[name];
  if (typeof fn !== "function") {
    throw new Error(`scenario transport ABI: missing ${name}`);
  }
  return fn;
}

export function writeScenarioManifest(wasm, manifest) {
  const text = typeof manifest === "string"
    ? manifest
    : JSON.stringify(manifest);
  const bytes = new TextEncoder().encode(text);

  requireFunction(wasm, "amemory_scenario_manifest_clear")();
  const setByte = requireFunction(wasm, "amemory_scenario_manifest_set_byte");
  for (let index = 0; index < bytes.length; index += 1) {
    if ((setByte(index, bytes[index]) >>> 0) !== 1) {
      throw new Error(`scenario transport ABI: rejected manifest byte ${index}`);
    }
  }
  return bytes.length >>> 0;
}

export function executeScenarioManifest(
  wasm,
  manifest,
  backend = "optimized-cpu",
) {
  const backendCode = BACKENDS.get(backend);
  if (backendCode === undefined) {
    throw new Error(`scenario transport: unknown backend ${backend}`);
  }

  const length = writeScenarioManifest(wasm, manifest);
  const execute = requireFunction(wasm, "amemory_scenario_execute_json");
  const success = execute(length, backendCode) >>> 0;

  const report = readJsonAbi(
    wasm,
    REPORT_ABI,
    "scenario report",
  );
  const error = readJsonAbi(
    wasm,
    ERROR_ABI,
    "scenario error",
  );

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
