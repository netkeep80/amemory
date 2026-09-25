import assert from "node:assert/strict";
import {
  ANUM_FIXTURES,
  compileAnumPlan,
  exportLocalTopology,
  localRef,
  normalizeAnum,
  requireLocalRef,
  topologyCount,
  cpuExport,
  cpuImportRaw,
  wasmU32,
} from "../web/anum-boundary.mjs";

for (const source of ANUM_FIXTURES) {
  assert.equal(normalizeAnum(source), source);
  const plan = compileAnumPlan(source);
  assert.ok(plan.nodes.length >= 1);
  assert.equal(plan.rootNode, plan.nodes.length - 1);
}

for (const source of ["", "5", "1", "9", "6", "11", "88", "19868x"]) {
  assert.throws(() => normalizeAnum(source), /Anum|token|truncated|trailing/);
}

assert.throws(
  () => compileAnumPlan("9".repeat(70) + "8"),
  /capacity exceeded/,
);

const cpu = {
  starts: new Uint32Array(64),
  ends: new Uint32Array(64),
  used: new Uint32Array(64),
};
cpu.used[1] = 1; cpu.starts[1] = 1; cpu.ends[1] = 1;
cpu.used[2] = 1; cpu.starts[2] = 2; cpu.ends[2] = 1;
cpu.used[3] = 1; cpu.starts[3] = 1; cpu.ends[3] = 3;
cpu.used[4] = 1; cpu.starts[4] = 2; cpu.ends[4] = 3;

const gpu = {
  starts: new Uint32Array(64),
  ends: new Uint32Array(64),
  used: new Uint32Array(64),
};
gpu.used[63] = 1; gpu.starts[63] = 63; gpu.ends[63] = 63;
gpu.used[62] = 1; gpu.starts[62] = 62; gpu.ends[62] = 63;
gpu.used[61] = 1; gpu.starts[61] = 63; gpu.ends[61] = 61;
gpu.used[60] = 1; gpu.starts[60] = 62; gpu.ends[60] = 61;

assert.equal(exportLocalTopology(cpu, 4), "19868");
assert.equal(exportLocalTopology(gpu, 60), "19868");
assert.equal(topologyCount(cpu), 4);
assert.equal(topologyCount(gpu), 4);

const cpuRef = localRef("cpu-A", 4);
const gpuRef = localRef("gpu-B", 60);
assert.equal(requireLocalRef(cpuRef, "cpu-A"), 4);
assert.equal(requireLocalRef(gpuRef, "gpu-B"), 60);
assert.throws(() => requireLocalRef(cpuRef, "gpu-B"), /foreign local handle/);
assert.throws(() => requireLocalRef(gpuRef, "cpu-A"), /foreign local handle/);

const cycle = {
  starts: new Uint32Array(cpu.starts),
  ends: new Uint32Array(cpu.ends),
  used: new Uint32Array(cpu.used),
};
cycle.used[5] = 1;
cycle.starts[5] = 5;
cycle.ends[5] = 6;
cycle.used[6] = 1;
cycle.starts[6] = 6;
cycle.ends[6] = 5;
assert.throws(() => exportLocalTopology(cycle, 5), /non-well-founded/);

console.log("PORTABLE_ANUM_BOUNDARY_NEGATIVE_WITNESSES=GREEN");


// WebAssembly JS exposes i32 results as signed Numbers. Rust u32::MAX therefore
// crosses the ABI as -1 and must be normalized before sentinel comparison.
assert.equal(wasmU32(-1), 0xffffffff);
assert.equal(wasmU32(0xffffffff), 0xffffffff);
assert.equal(wasmU32(63), 63);

const rejectingWasm = {
  anumCpuSetToken: () => 1,
  anumCpuImport: () => -1,
};
assert.equal(cpuImportRaw(rejectingWasm, "5"), null);

const rejectingExportWasm = {
  anumCpuExport: () => -1,
};
assert.throws(
  () => cpuExport(rejectingExportWasm, localRef("cpu-A", 1)),
  /CPU export rejected local topology/,
);
