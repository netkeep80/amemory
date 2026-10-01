import path from "node:path";
import { pathToFileURL } from "node:url";

const [coreArg, threeArg] = process.argv.slice(2);
if (!coreArg) {
  throw new Error("usage: mts-visual-bundle-smoke.mjs <core.mjs> [three.mjs]");
}

const coreUrl = pathToFileURL(path.resolve(coreArg)).href +
  "?smoke=" + Date.now();
const core = await import(coreUrl);

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

const estimate = core.estimateOctahedralPerformance3D(
  1_000_000,
  2 * Math.SQRT2,
);
if (estimate.linkCount !== 1_000_000 ||
    estimate.pairwiseSemanticLinkEvaluations !== 0 ||
    !Number.isFinite(estimate.gpuDynamicBytes)) {
  throw new Error("mts_visual million-Link estimate smoke failed");
}

const network = {
  links: Array.from(
    { length: 64 },
    (_, i) => ({ key: "L" + i, startKey: "L" + i, endKey: "L" + i }),
  ),
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
    throw new Error("mts_visual live-physics position is not finite");
  }
}
if (!(maxY - minY > 1e-3 && maxZ - minZ > 1e-3)) {
  throw new Error("mts_visual live-physics layout collapsed");
}

if (threeArg) {
  const threeUrl = pathToFileURL(path.resolve(threeArg)).href +
    "?smoke=" + Date.now();
  const three = await import(threeUrl);
  for (const name of [
    "createVisualThreeRenderer",
    "createOctahedralThreeLiveRenderer",
    "destroyOctahedralThreeLiveRenderer",
  ]) {
    if (typeof three[name] !== "function") {
      throw new Error("mts_visual three export missing: " + name);
    }
  }
}

console.log(
  "MTS_VISUAL_BUNDLE_SMOKE=PASS " +
  "links=1000000 physicsLinks=64 three=" + Boolean(threeArg),
);
