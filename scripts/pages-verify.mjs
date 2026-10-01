import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";

const root = process.cwd();
const site = path.join(root, "_site");

function requiredSha(name) {
  const value = process.env[name] ?? "";
  if (!/^[0-9a-f]{40}$/.test(value)) {
    throw new Error(name + " must be an exact 40-hex Git SHA");
  }
  return value;
}

function requireFile(relative) {
  const target = path.join(site, relative);
  if (!fs.statSync(target, { throwIfNoEntry: false })?.isFile()) {
    throw new Error("required Pages file missing: " + relative);
  }
  return target;
}

function requireAbsent(relative) {
  if (fs.existsSync(path.join(site, relative))) {
    throw new Error("obsolete Pages file must be absent: " + relative);
  }
}

function requireText(source, needle, label) {
  if (!source.includes(needle)) {
    throw new Error(label + " missing: " + needle);
  }
}

function checkSyntax(relative) {
  const result = spawnSync(
    process.execPath,
    ["--check", path.join(site, relative)],
    { stdio: "inherit" },
  );
  if (result.status !== 0) {
    throw new Error("syntax check failed: " + relative);
  }
}

const sourceSha = requiredSha("ACCEPTED_SOURCE_SHA");
const version = fs.readFileSync(path.join(root, "VERSION"), "utf8").trim();
if (!/^\d+\.\d+\.\d+$/.test(version)) {
  throw new Error("VERSION is not SemVer");
}

for (const relative of [
  "workbench.mjs",
  "proof-view.mjs",
  "scenario-transport.mjs",
  "scenario-presets.mjs",
  "scenario-worker.mjs",
  "scenario-worker-client.mjs",
  "i386-blocks.json",
  "amemory_a_circuit.wasm",
  "mts-visual-lock.json",
  "build-info.json",
  "build-info.js",
  "exact-evidence.json",
  "vendor/mts-visual/index.js",
  "vendor/mts-visual/blueprint-svg.js",
  "vendor/mts-visual/three/index.js",
  "vendor/three/three.module.js",
  "vendor/three/addons/controls/OrbitControls.js",
  "vendor/mts-visual-core.bundle.js",
  "vendor/mts-visual-three.bundle.js",
]) {
  requireFile(relative);
}

for (const relative of [
  "app.js",
  "i386-lab.js",
  "i386-lab-view.mjs",
  "i386-proof-view.mjs",
]) {
  requireAbsent(relative);
}

const proofSource = fs.readFileSync(
  path.join(site, "proof-view.mjs"),
  "utf8",
);
for (const required of [
  'import("./vendor/mts-visual-core.bundle.js")',
  'import("./vendor/mts-visual-three.bundle.js")',
  "createOctahedralLivePhysics3D",
  "createOctahedralThreeLiveRenderer",
]) {
  requireText(proofSource, required, "published proof renderer");
}
if (proofSource.includes("vendor/mts-visual/three/index.js")) {
  throw new Error(
    "published proof renderer still references unbundled 3D entrypoint",
  );
}

const indexSource = fs.readFileSync(path.join(site, "index.html"), "utf8");
requireText(
  indexSource,
  '<script type="module" src="./workbench.mjs"></script>',
  "published index",
);
for (const forbidden of [
  "legacy-diagnostics",
  'src="./app.js"',
  'src="./i386-lab.js"',
]) {
  if (indexSource.includes(forbidden)) {
    throw new Error("published page retains obsolete UI wiring: " + forbidden);
  }
}

for (const relative of [
  "workbench.mjs",
  "proof-view.mjs",
  "scenario-transport.mjs",
  "scenario-presets.mjs",
  "scenario-worker.mjs",
  "scenario-worker-client.mjs",
]) {
  checkSyntax(relative);
}

const tempRoot = path.join(
  os.tmpdir(),
  "amemory-pages-verify-" + process.pid,
);
fs.mkdirSync(tempRoot, { recursive: true });
const coreTemp = path.join(tempRoot, "mts-visual-core.bundle.mjs");
const threeTemp = path.join(tempRoot, "mts-visual-three.bundle.mjs");
fs.copyFileSync(
  path.join(site, "vendor/mts-visual-core.bundle.js"),
  coreTemp,
);
fs.copyFileSync(
  path.join(site, "vendor/mts-visual-three.bundle.js"),
  threeTemp,
);

try {
  const core = await import(pathToFileURL(coreTemp).href + "?v=" + sourceSha);
  for (const name of [
    "buildBlueprintSvgScene",
    "createOctahedralLivePhysics3D",
    "estimateOctahedralPerformance3D",
    "createMonolithicLinkSpringPhysics3D",
    "deriveMonolithicNetworkShape3D",
  ]) {
    if (typeof core[name] !== "function") {
      throw new Error("mts_visual core export missing: " + name);
    }
  }

  const network = {
    links: Array.from({ length: 64 }, (_, index) => ({
      key: "L" + index,
      startKey: "L" + index,
      endKey: "L" + index,
    })),
  };
  const controller = core.createOctahedralLivePhysics3D(network, {
    aspectRatio: 2 * Math.SQRT2,
    stiffness: 1,
    simulationSpeed: 1,
  });
  const template = controller.template;
  const positions = controller.positions;
  let minY = Infinity;
  let maxY = -Infinity;
  let minZ = Infinity;
  let maxZ = -Infinity;
  for (let link = 0; link < 64; link += 1) {
    let y = 0;
    let z = 0;
    for (const vertex of template.centerTriangle) {
      const offset = (link * template.vertexCount + vertex) * 3;
      y += positions[offset + 1];
      z += positions[offset + 2];
    }
    y /= 3;
    z /= 3;
    minY = Math.min(minY, y);
    maxY = Math.max(maxY, y);
    minZ = Math.min(minZ, z);
    maxZ = Math.max(maxZ, z);
  }
  for (const value of positions) {
    if (!Number.isFinite(value)) {
      throw new Error("mts_visual physics produced non-finite position");
    }
  }
  if (!(maxY - minY > 1e-3 && maxZ - minZ > 1e-3)) {
    throw new Error("mts_visual 3D witness collapsed to a flat layout");
  }

  const three = await import(
    pathToFileURL(threeTemp).href + "?v=" + sourceSha
  );
  for (const name of [
    "createVisualThreeRenderer",
    "createOctahedralThreeLiveRenderer",
    "destroyOctahedralThreeLiveRenderer",
  ]) {
    if (typeof three[name] !== "function") {
      throw new Error("mts_visual three export missing: " + name);
    }
  }

  const info = JSON.parse(
    fs.readFileSync(path.join(site, "build-info.json"), "utf8"),
  );
  if (info.version !== version) {
    throw new Error(
      "Pages version mismatch: " + info.version + " != " + version,
    );
  }
  if (info.mainSha !== sourceSha) {
    throw new Error(
      "Pages SHA mismatch: " + info.mainSha + " != " + sourceSha,
    );
  }

  const visualLock = JSON.parse(
    fs.readFileSync(path.join(site, "mts-visual-lock.json"), "utf8"),
  );
  if (visualLock.commit !== "ccb23ffacb6e534690b1e88b35f3056b93929951") {
    throw new Error("Pages artifact has wrong mts_visual commit");
  }
  if (visualLock.packageVersion !== "0.5.0") {
    throw new Error("Pages artifact has wrong mts_visual package version");
  }

  const registry = JSON.parse(
    fs.readFileSync(path.join(site, "i386-blocks.json"), "utf8"),
  );
  if (!Array.isArray(registry.blocks) || registry.blocks.length < 24) {
    throw new Error("Pages artifact lost 80386 registry blocks");
  }

  const evidence = JSON.parse(
    fs.readFileSync(path.join(site, "exact-evidence.json"), "utf8"),
  );
  if (evidence.sourceSha !== sourceSha) {
    throw new Error("exact evidence source SHA mismatch");
  }
  if (evidence.version !== version) {
    throw new Error("exact evidence version mismatch");
  }
  if (evidence.proofCoveredOpcodes !== registry.blocks.length) {
    throw new Error("exact evidence registry/proof coverage mismatch");
  }
  if (
    evidence.registryBlocks !== undefined &&
    evidence.proofCoveredOpcodes !== evidence.registryBlocks
  ) {
    throw new Error("exact evidence internal registry/proof count mismatch");
  }

  const buildInfoTemp = path.join(tempRoot, "build-info.mjs");
  fs.copyFileSync(path.join(site, "build-info.js"), buildInfoTemp);
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

  await import(
    pathToFileURL(buildInfoTemp).href + "?acceptance=" + info.mainSha
  );
  await new Promise((resolve) => setTimeout(resolve, 0));

  const versionText = nodes.get("#amemory-version").textContent;
  const shaText = nodes.get("#browser-lab-sha").textContent;
  if (
    versionText !== "v" + info.version ||
    shaText !== info.mainSha.slice(0, 12) ||
    /unavailable|unknown/i.test(versionText + " " + shaText)
  ) {
    throw new Error(
      "browser build badge did not consume generated build-info.json",
    );
  }

  console.log(
    "PAGES_STAGED_PAYLOAD=PASS v" +
      version +
      " @ " +
      sourceSha.slice(0, 12) +
      " / " +
      registry.blocks.length +
      " blocks / mts_visual " +
      visualLock.commit.slice(0, 12),
  );
} finally {
  fs.rmSync(tempRoot, { recursive: true, force: true });
}
