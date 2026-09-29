import { readJsonAbi, readUtf8Abi } from "./i386-wasm-json.mjs";

const REGISTRY_ABI = {
  available: "amemory_scenario_preset_registry_available",
  length: "amemory_scenario_preset_registry_json_len",
  pointer: "amemory_scenario_preset_registry_json_ptr",
  byte: "amemory_scenario_preset_registry_json_byte",
};

const MANIFEST_ABI = {
  available: "amemory_scenario_preset_manifest_available",
  length: "amemory_scenario_preset_manifest_json_len",
  pointer: "amemory_scenario_preset_manifest_json_ptr",
  byte: "amemory_scenario_preset_manifest_json_byte",
};

function requireFunction(wasm, name) {
  const fn = wasm[name];
  if (typeof fn !== "function") {
    throw new Error(`scenario preset ABI: missing ${name}`);
  }
  return fn;
}

export function refreshScenarioPresetRegistry(wasm) {
  const refresh = requireFunction(
    wasm,
    "amemory_scenario_preset_registry_refresh",
  );
  if ((refresh() >>> 0) !== 1) {
    throw new Error("scenario preset registry refresh failed");
  }

  const registry = readJsonAbi(
    wasm,
    REGISTRY_ABI,
    "scenario preset registry",
  );
  if (registry === null ||
      registry.schemaVersion !== 1 ||
      !Array.isArray(registry.entries)) {
    throw new Error("scenario preset registry envelope mismatch");
  }
  return registry;
}

export function loadScenarioPresetManifestByIndex(wasm, index) {
  if (!Number.isInteger(index) || index < 0) {
    throw new Error(`scenario preset: invalid index ${index}`);
  }

  const load = requireFunction(
    wasm,
    "amemory_scenario_preset_manifest_load",
  );
  if ((load(index >>> 0) >>> 0) !== 1) {
    return null;
  }
  return readUtf8Abi(
    wasm,
    MANIFEST_ABI,
    "scenario preset manifest",
  );
}

export function findScenarioPreset(registry, scenarioId, scenarioVersion) {
  if (!registry || !Array.isArray(registry.entries)) {
    throw new Error("scenario preset: invalid registry");
  }
  const index = registry.entries.findIndex((entry) =>
    entry.scenarioId === scenarioId &&
    entry.scenarioVersion === scenarioVersion
  );
  if (index < 0) return null;
  return { index, summary: registry.entries[index] };
}

export function loadScenarioPresetManifest(
  wasm,
  registry,
  scenarioId,
  scenarioVersion,
) {
  const found = findScenarioPreset(
    registry,
    scenarioId,
    scenarioVersion,
  );
  if (found === null) return null;

  const source = loadScenarioPresetManifestByIndex(wasm, found.index);
  if (source === null) {
    throw new Error(
      `scenario preset manifest missing for ${scenarioId}@${scenarioVersion}`,
    );
  }
  return { ...found, source };
}
