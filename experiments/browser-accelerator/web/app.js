import { assertExactU32, perturbFirst } from "./differential.mjs";
import {
  assertCanonicalNewPairs,
  newPairsFromFlags,
  normalizePairSet,
} from "./canonicalization.mjs";
import {
  STATE_CELLS,
  STATE_SIDE,
  assertStateExact,
  assertStatePreserved,
  formatStatePairs,
  matrixToPairs,
} from "./state-store.mjs";
import { runAnumBoundaryBrowser } from "./anum-boundary.mjs";
import { runReactionBrowser } from "./reaction.mjs";

const ui = {
  wasm: document.querySelector("#wasm"),
  cpu: document.querySelector("#cpu"),
  webgpu: document.querySelector("#webgpu"),
  compute: document.querySelector("#compute"),
  incidenceCpu: document.querySelector("#incidence-cpu"),
  incidenceGpu: document.querySelector("#incidence-gpu"),
  differential: document.querySelector("#differential"),
  negative: document.querySelector("#negative"),
  canonicalCpu: document.querySelector("#canonical-cpu"),
  canonicalGpu: document.querySelector("#canonical-gpu"),
  canonicalPairsCpu: document.querySelector("#canonical-pairs-cpu"),
  canonicalPairsGpu: document.querySelector("#canonical-pairs-gpu"),
  canonicalDiff: document.querySelector("#canonical-diff"),
  canonicalNegative: document.querySelector("#canonical-negative"),
  stateR0Cpu: document.querySelector("#state-r0-cpu"),
  stateR0Gpu: document.querySelector("#state-r0-gpu"),
  stateR1Cpu: document.querySelector("#state-r1-cpu"),
  stateR1Gpu: document.querySelector("#state-r1-gpu"),
  stateR2Cpu: document.querySelector("#state-r2-cpu"),
  stateR2Gpu: document.querySelector("#state-r2-gpu"),
  stateDiff: document.querySelector("#state-diff"),
  stateNegative: document.querySelector("#state-negative"),
  anumImportCpu: document.querySelector("#anum-import-cpu"),
  anumImportGpu: document.querySelector("#anum-import-gpu"),
  anumExportCpu: document.querySelector("#anum-export-cpu"),
  anumExportGpu: document.querySelector("#anum-export-gpu"),
  anumDiff: document.querySelector("#anum-diff"),
  anumHandles: document.querySelector("#anum-handles"),
  anumReuse: document.querySelector("#anum-reuse"),
  anumNegative: document.querySelector("#anum-negative"),
  reactionBeforeCpu: document.querySelector("#reaction-before-cpu"),
  reactionBeforeGpu: document.querySelector("#reaction-before-gpu"),
  reactionAfterCpu: document.querySelector("#reaction-after-cpu"),
  reactionAfterGpu: document.querySelector("#reaction-after-gpu"),
  reactionMatched: document.querySelector("#reaction-matched"),
  reactionHandoff: document.querySelector("#reaction-handoff"),
  reactionDiff: document.querySelector("#reaction-diff"),
  reactionHandles: document.querySelector("#reaction-handles"),
  reactionScope: document.querySelector("#reaction-scope"),
  reactionNegative: document.querySelector("#reaction-negative"),
  reactionR2Cpu: document.querySelector("#reaction-r2-cpu"),
  reactionR2Gpu: document.querySelector("#reaction-r2-gpu"),
  reactionR2Matched: document.querySelector("#reaction-r2-matched"),
  reactionR2Handoff: document.querySelector("#reaction-r2-handoff"),
  reactionR2Quiescence: document.querySelector("#reaction-r2-quiescence"),
  reactionR2Bank: document.querySelector("#reaction-r2-bank"),
  reactionR2Diff: document.querySelector("#reaction-r2-diff"),
  reactionR2Failure: document.querySelector("#reaction-r2-failure"),
  reactionR3ZeroCpu: document.querySelector("#reaction-r3-zero-cpu"),
  reactionR3ZeroGpu: document.querySelector("#reaction-r3-zero-gpu"),
  reactionR3ZeroMatched: document.querySelector("#reaction-r3-zero-matched"),
  reactionR3ZeroHandoff: document.querySelector("#reaction-r3-zero-handoff"),
  reactionR3ZeroQuiescence: document.querySelector("#reaction-r3-zero-quiescence"),
  reactionR3ZeroOldScope: document.querySelector("#reaction-r3-zero-old-scope"),
  reactionR3MixedCpu: document.querySelector("#reaction-r3-mixed-cpu"),
  reactionR3MixedGpu: document.querySelector("#reaction-r3-mixed-gpu"),
  reactionR3MixedMatched: document.querySelector("#reaction-r3-mixed-matched"),
  reactionR3MixedHandoff: document.querySelector("#reaction-r3-mixed-handoff"),
  reactionR3MixedQuiescence: document.querySelector("#reaction-r3-mixed-quiescence"),
  reactionR3Duplicates: document.querySelector("#reaction-r3-duplicates"),
  reactionR3Diff: document.querySelector("#reaction-r3-diff"),
  reactionR4TBefore: document.querySelector("#reaction-r4-t-before"),
  reactionR4TSnapshot: document.querySelector("#reaction-r4-t-snapshot"),
  reactionR4LiveTheory: document.querySelector("#reaction-r4-live-theory"),
  reactionR4TAfter: document.querySelector("#reaction-r4-t-after"),
  reactionR4TMatched: document.querySelector("#reaction-r4-t-matched"),
  reactionR4THandoff: document.querySelector("#reaction-r4-t-handoff"),
  reactionR4T1Snapshot: document.querySelector("#reaction-r4-t1-snapshot"),
  reactionR4T1Before: document.querySelector("#reaction-r4-t1-before"),
  reactionR4T1After: document.querySelector("#reaction-r4-t1-after"),
  reactionR4T1Matched: document.querySelector("#reaction-r4-t1-matched"),
  reactionR4T1Handoff: document.querySelector("#reaction-r4-t1-handoff"),
  reactionR4Isolation: document.querySelector("#reaction-r4-isolation"),
  reactionR4Visibility: document.querySelector("#reaction-r4-visibility"),
  reactionR4Stale: document.querySelector("#reaction-r4-stale"),
  reactionR4Diff: document.querySelector("#reaction-r4-diff"),
  reactionR5S0: document.querySelector("#reaction-r5-s0"),
  reactionR5S1: document.querySelector("#reaction-r5-s1"),
  reactionR5S2: document.querySelector("#reaction-r5-s2"),
  reactionR5S3: document.querySelector("#reaction-r5-s3"),
  reactionR5S4: document.querySelector("#reaction-r5-s4"),
  reactionR5Snapshot: document.querySelector("#reaction-r5-snapshot"),
  reactionR5Matched: document.querySelector("#reaction-r5-matched"),
  reactionR5Handoff: document.querySelector("#reaction-r5-handoff"),
  reactionR5Quiescence: document.querySelector("#reaction-r5-quiescence"),
  reactionR5Recurrence: document.querySelector("#reaction-r5-recurrence"),
  reactionR5EndStructure: document.querySelector("#reaction-r5-end-structure"),
  reactionR5EndContinuation: document.querySelector("#reaction-r5-end-continuation"),
  reactionR5Bounded: document.querySelector("#reaction-r5-bounded"),
  reactionR5Diff: document.querySelector("#reaction-r5-diff"),
  reactionR6OneMany: document.querySelector("#reaction-r6-one-many"),
  reactionR6NM: document.querySelector("#reaction-r6-n-m"),
  reactionR6Order: document.querySelector("#reaction-r6-order"),
  reactionR6Scope: document.querySelector("#reaction-r6-scope"),
  reactionR6Diff: document.querySelector("#reaction-r6-diff"),
  details: document.querySelector("#details"),
};

const details = [];

function report(element, text, ok = null) {
  element.textContent = text;
  if (ok === true) element.className = "ok";
  if (ok === false) element.className = "fail";
}

function log(line) {
  details.push(line);
  ui.details.textContent = details.join("\n");
}

async function loadWasm() {
  const response = await fetch("./amemory_browser_probe.wasm", { cache: "no-store" });
  if (!response.ok) throw new Error(`WASM fetch failed: HTTP ${response.status}`);

  const bytes = await response.arrayBuffer();
  const { instance } = await WebAssembly.instantiate(bytes, {});
  const probe = instance.exports.amemory_probe;
  const cpuStep = instance.exports.amemory_cpu_step;
  const incidenceFlag = instance.exports.amemory_incidence_flag;
  const canonicalSetExisting = instance.exports.amemory_canonical_set_existing;
  const canonicalSetCandidate = instance.exports.amemory_canonical_set_candidate;
  const canonicalFlag = instance.exports.amemory_canonical_flag;
  const stateReset = instance.exports.amemory_state_reset;
  const stateCommit = instance.exports.amemory_state_commit;
  const stateGet = instance.exports.amemory_state_get;
  const anumCpuResetPool = instance.exports.amemory_anum_cpu_reset_pool;
  const anumCpuSetToken = instance.exports.amemory_anum_cpu_set_token;
  const anumCpuImport = instance.exports.amemory_anum_cpu_import;
  const anumCpuExport = instance.exports.amemory_anum_cpu_export;
  const anumCpuOutputGet = instance.exports.amemory_anum_cpu_output_get;
  const anumCpuPoolCount = instance.exports.amemory_anum_cpu_pool_count;
  const reactionReset = instance.exports.amemory_reaction_reset;
  const reactionSetCurrentMember = instance.exports.amemory_reaction_set_current_member;
  const reactionSetCurrentCount = instance.exports.amemory_reaction_set_current_count;
  const reactionSetTheoryRelation = instance.exports.amemory_reaction_set_theory_relation;
  const reactionSetTheoryCount = instance.exports.amemory_reaction_set_theory_count;
  const reactionSnapshotTheory = instance.exports.amemory_reaction_snapshot_theory;
  const reactionRun = instance.exports.amemory_reaction_run;
  const reactionCurrentBank = instance.exports.amemory_reaction_current_bank;
  const reactionCurrentCount = instance.exports.amemory_reaction_current_count;
  const reactionCurrentMember = instance.exports.amemory_reaction_current_member;
  const reactionBankCount = instance.exports.amemory_reaction_bank_count;
  const reactionBankMember = instance.exports.amemory_reaction_bank_member;
  const reactionTheoryCount = instance.exports.amemory_reaction_theory_count;
  const reactionSnapshotCount = instance.exports.amemory_reaction_snapshot_count;
  const reactionMatchedRelations = instance.exports.amemory_reaction_matched_relations;
  const reactionHandoffCount = instance.exports.amemory_reaction_handoff_count;
  const reactionQuiescent = instance.exports.amemory_reaction_quiescent;

  if ([
    probe, cpuStep, incidenceFlag,
    canonicalSetExisting, canonicalSetCandidate, canonicalFlag,
    stateReset, stateCommit, stateGet,
    anumCpuResetPool, anumCpuSetToken, anumCpuImport, anumCpuExport,
    anumCpuOutputGet, anumCpuPoolCount,
    reactionReset, reactionSetCurrentMember, reactionSetCurrentCount,
    reactionSetTheoryRelation, reactionSetTheoryCount, reactionSnapshotTheory,
    reactionRun, reactionCurrentBank, reactionCurrentCount, reactionCurrentMember,
    reactionBankCount, reactionBankMember, reactionTheoryCount, reactionSnapshotCount,
    reactionMatchedRelations, reactionHandoffCount, reactionQuiescent,
  ].some((fn) => typeof fn !== "function")) {
    throw new Error("Expected Rust/WASM exports are missing");
  }

  const marker = probe();
  if (marker !== 0xA013) {
    throw new Error(`Unexpected WASM probe marker: 0x${marker.toString(16)}`);
  }

  report(ui.wasm, "LOADED", true);
  log(`wasm.marker = 0x${marker.toString(16)}`);

  const cpuResult = cpuStep(41);
  if (cpuResult !== 42) {
    throw new Error(`WASM CPU control failed: expected 42, got ${cpuResult}`);
  }
  report(ui.cpu, "PASS (41 -> 42)", true);
  log(`wasm.cpu_step = ${cpuResult}`);

  return {
    incidenceFlag,
    canonicalSetExisting,
    canonicalSetCandidate,
    canonicalFlag,
    stateReset,
    stateCommit,
    stateGet,
    anumCpuResetPool,
    anumCpuSetToken,
    anumCpuImport,
    anumCpuExport,
    anumCpuOutputGet,
    anumCpuPoolCount,
    reactionReset,
    reactionSetCurrentMember,
    reactionSetCurrentCount,
    reactionSetTheoryRelation,
    reactionSetTheoryCount,
    reactionSnapshotTheory,
    reactionRun,
    reactionCurrentBank,
    reactionCurrentCount,
    reactionCurrentMember,
    reactionBankCount,
    reactionBankMember,
    reactionTheoryCount,
    reactionSnapshotCount,
    reactionMatchedRelations,
    reactionHandoffCount,
    reactionQuiescent,
  };
}

async function initWebGpu() {
  if (!("gpu" in navigator)) {
    report(ui.webgpu, "WEBGPU_UNAVAILABLE", false);
    return null;
  }

  let adapter;
  try {
    adapter = await navigator.gpu.requestAdapter();
  } catch (error) {
    report(ui.webgpu, "WEBGPU_INIT_FAILED", false);
    log(`requestAdapter failed: ${error}`);
    return null;
  }

  if (!adapter) {
    report(ui.webgpu, "WEBGPU_UNAVAILABLE", false);
    log("WebGPU exists, but no GPUAdapter was returned.");
    return null;
  }

  try {
    const device = await adapter.requestDevice();
    report(ui.webgpu, "WEBGPU_AVAILABLE", true);
    log(`adapter.features = ${[...adapter.features].join(", ") || "(none exposed)"}`);
    log(`device.features = ${[...device.features].join(", ") || "(none exposed)"}`);
    return device;
  } catch (error) {
    report(ui.webgpu, "WEBGPU_INIT_FAILED", false);
    log(`requestDevice failed: ${error}`);
    return null;
  }
}

async function runSmoke(device) {
  const input = new Uint32Array([1, 2, 3, 4]);
  const expected = new Uint32Array([2, 3, 4, 5]);
  const byteLength = input.byteLength;

  const inputBuffer = device.createBuffer({
    size: byteLength,
    usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
  });
  const outputBuffer = device.createBuffer({
    size: byteLength,
    usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
  });
  const readbackBuffer = device.createBuffer({
    size: byteLength,
    usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
  });

  device.queue.writeBuffer(inputBuffer, 0, input);

  const shader = device.createShaderModule({
    code: `
      @group(0) @binding(0) var<storage, read> input_data: array<u32>;
      @group(0) @binding(1) var<storage, read_write> output_data: array<u32>;

      @compute @workgroup_size(64)
      fn main(@builtin(global_invocation_id) id: vec3<u32>) {
        let i = id.x;
        if (i < 4u) {
          output_data[i] = input_data[i] + 1u;
        }
      }
    `,
  });

  const pipeline = device.createComputePipeline({
    layout: "auto",
    compute: { module: shader, entryPoint: "main" },
  });
  const bindGroup = device.createBindGroup({
    layout: pipeline.getBindGroupLayout(0),
    entries: [
      { binding: 0, resource: { buffer: inputBuffer } },
      { binding: 1, resource: { buffer: outputBuffer } },
    ],
  });

  const encoder = device.createCommandEncoder();
  const pass = encoder.beginComputePass();
  pass.setPipeline(pipeline);
  pass.setBindGroup(0, bindGroup);
  pass.dispatchWorkgroups(1);
  pass.end();
  encoder.copyBufferToBuffer(outputBuffer, 0, readbackBuffer, 0, byteLength);
  device.queue.submit([encoder.finish()]);

  await readbackBuffer.mapAsync(GPUMapMode.READ, 0, byteLength);
  const actual = new Uint32Array(readbackBuffer.getMappedRange(0, byteLength).slice(0));
  readbackBuffer.unmap();

  assertExactU32(expected, actual, "GPU smoke");
  report(ui.compute, `PASS [${[...actual].join(", ")}]`, true);
  log(`gpu.smoke.output = [${[...actual].join(", ")}]`);

  inputBuffer.destroy();
  outputBuffer.destroy();
  readbackBuffer.destroy();
}

function cpuIncidence(wasm, fixture) {
  return Uint32Array.from(
    fixture.links.map(([start, end]) =>
      wasm.incidenceFlag(start, end, fixture.queryStart, fixture.queryEnd)
    ),
  );
}

async function gpuIncidence(device, fixture) {
  const flatLinks = new Uint32Array(fixture.links.flat());
  const query = new Uint32Array([fixture.queryStart, fixture.queryEnd]);
  const outputBytes = fixture.links.length * Uint32Array.BYTES_PER_ELEMENT;

  const linksBuffer = device.createBuffer({
    size: flatLinks.byteLength,
    usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
  });
  const queryBuffer = device.createBuffer({
    size: query.byteLength,
    usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
  });
  const outputBuffer = device.createBuffer({
    size: outputBytes,
    usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
  });
  const readbackBuffer = device.createBuffer({
    size: outputBytes,
    usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
  });

  device.queue.writeBuffer(linksBuffer, 0, flatLinks);
  device.queue.writeBuffer(queryBuffer, 0, query);

  const shader = device.createShaderModule({
    code: `
      @group(0) @binding(0) var<storage, read> links: array<u32>;
      @group(0) @binding(1) var<storage, read> query: array<u32>;
      @group(0) @binding(2) var<storage, read_write> flags: array<u32>;

      @compute @workgroup_size(64)
      fn main(@builtin(global_invocation_id) id: vec3<u32>) {
        let i = id.x;
        if (i < ${fixture.links.length}u) {
          let start = links[i * 2u];
          let end = links[i * 2u + 1u];
          var f = 0u;

          if (start == query[0]) {
            f = f | 1u;
          }
          if (end == query[1]) {
            f = f | 2u;
          }
          if ((f & 3u) == 3u) {
            f = f | 4u;
          }
          flags[i] = f;
        }
      }
    `,
  });

  const pipeline = device.createComputePipeline({
    layout: "auto",
    compute: { module: shader, entryPoint: "main" },
  });
  const bindGroup = device.createBindGroup({
    layout: pipeline.getBindGroupLayout(0),
    entries: [
      { binding: 0, resource: { buffer: linksBuffer } },
      { binding: 1, resource: { buffer: queryBuffer } },
      { binding: 2, resource: { buffer: outputBuffer } },
    ],
  });

  const encoder = device.createCommandEncoder();
  const pass = encoder.beginComputePass();
  pass.setPipeline(pipeline);
  pass.setBindGroup(0, bindGroup);
  pass.dispatchWorkgroups(Math.ceil(fixture.links.length / 64));
  pass.end();
  encoder.copyBufferToBuffer(outputBuffer, 0, readbackBuffer, 0, outputBytes);
  device.queue.submit([encoder.finish()]);

  await readbackBuffer.mapAsync(GPUMapMode.READ, 0, outputBytes);
  const result = new Uint32Array(readbackBuffer.getMappedRange(0, outputBytes).slice(0));
  readbackBuffer.unmap();

  linksBuffer.destroy();
  queryBuffer.destroy();
  outputBuffer.destroy();
  readbackBuffer.destroy();

  return result;
}

async function runDifferential(wasm, device) {
  const fixture = {
    links: [
      [1, 2],
      [1, 3],
      [4, 2],
      [4, 5],
      [1, 2],
    ],
    queryStart: 1,
    queryEnd: 2,
  };
  const expected = new Uint32Array([7, 1, 2, 0, 7]);

  const cpu = cpuIncidence(wasm, fixture);
  assertExactU32(expected, cpu, "Rust/WASM incidence oracle");
  report(ui.incidenceCpu, `PASS [${[...cpu].join(", ")}]`, true);

  const gpu = await gpuIncidence(device, fixture);
  assertExactU32(expected, gpu, "WebGPU incidence");
  report(ui.incidenceGpu, `PASS [${[...gpu].join(", ")}]`, true);

  assertExactU32(cpu, gpu, "CPU/GPU differential");
  report(ui.differential, "PASS", true);

  let mismatchDetected = false;
  try {
    assertExactU32(cpu, perturbFirst(gpu), "negative control");
  } catch (error) {
    mismatchDetected = true;
    log(`negative.control = ${error.message}`);
  }
  if (!mismatchDetected) {
    throw new Error("negative control failed: deliberate mismatch was accepted");
  }
  report(ui.negative, "PASS (mismatch detected)", true);

  log(`incidence.cpu = [${[...cpu].join(", ")}]`);
  log(`incidence.gpu = [${[...gpu].join(", ")}]`);
}

function cpuCanonicalization(wasm, fixture) {
  fixture.existing.forEach(([start, end], index) => {
    if (wasm.canonicalSetExisting(index, start, end) !== 1) {
      throw new Error(`WASM existing pair index out of range: ${index}`);
    }
  });
  fixture.candidates.forEach(([start, end], index) => {
    if (wasm.canonicalSetCandidate(index, start, end) !== 1) {
      throw new Error(`WASM candidate pair index out of range: ${index}`);
    }
  });

  return Uint32Array.from(
    fixture.candidates.map((_, index) =>
      wasm.canonicalFlag(index, fixture.existing.length)
    ),
  );
}

async function gpuCanonicalization(device, fixture) {
  const existing = new Uint32Array(fixture.existing.flat());
  const candidates = new Uint32Array(fixture.candidates.flat());
  const outputBytes = fixture.candidates.length * Uint32Array.BYTES_PER_ELEMENT;

  const existingBuffer = device.createBuffer({
    size: Math.max(existing.byteLength, 4),
    usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
  });
  const candidateBuffer = device.createBuffer({
    size: candidates.byteLength,
    usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
  });
  const outputBuffer = device.createBuffer({
    size: outputBytes,
    usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
  });
  const readbackBuffer = device.createBuffer({
    size: outputBytes,
    usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
  });

  if (existing.byteLength > 0) device.queue.writeBuffer(existingBuffer, 0, existing);
  device.queue.writeBuffer(candidateBuffer, 0, candidates);

  const shader = device.createShaderModule({
    code: `
      @group(0) @binding(0) var<storage, read> existing_pairs: array<u32>;
      @group(0) @binding(1) var<storage, read> candidate_pairs: array<u32>;
      @group(0) @binding(2) var<storage, read_write> flags: array<u32>;

      @compute @workgroup_size(64)
      fn main(@builtin(global_invocation_id) id: vec3<u32>) {
        let i = id.x;
        if (i < ${fixture.candidates.length}u) {
          let start = candidate_pairs[i * 2u];
          let end = candidate_pairs[i * 2u + 1u];

          var exists = false;
          var e = 0u;
          loop {
            if (e >= ${fixture.existing.length}u) { break; }
            if (existing_pairs[e * 2u] == start && existing_pairs[e * 2u + 1u] == end) {
              exists = true;
              break;
            }
            e = e + 1u;
          }

          var prior = false;
          var j = 0u;
          loop {
            if (j >= i) { break; }
            if (candidate_pairs[j * 2u] == start && candidate_pairs[j * 2u + 1u] == end) {
              prior = true;
              break;
            }
            j = j + 1u;
          }

          var f = 0u;
          if (exists) { f = f | 1u; }
          if (prior) { f = f | 2u; }
          if (!exists && !prior) { f = f | 4u; }
          flags[i] = f;
        }
      }
    `,
  });

  const pipeline = device.createComputePipeline({
    layout: "auto",
    compute: { module: shader, entryPoint: "main" },
  });
  const bindGroup = device.createBindGroup({
    layout: pipeline.getBindGroupLayout(0),
    entries: [
      { binding: 0, resource: { buffer: existingBuffer } },
      { binding: 1, resource: { buffer: candidateBuffer } },
      { binding: 2, resource: { buffer: outputBuffer } },
    ],
  });

  const encoder = device.createCommandEncoder();
  const pass = encoder.beginComputePass();
  pass.setPipeline(pipeline);
  pass.setBindGroup(0, bindGroup);
  pass.dispatchWorkgroups(Math.ceil(fixture.candidates.length / 64));
  pass.end();
  encoder.copyBufferToBuffer(outputBuffer, 0, readbackBuffer, 0, outputBytes);
  device.queue.submit([encoder.finish()]);

  await readbackBuffer.mapAsync(GPUMapMode.READ, 0, outputBytes);
  const result = new Uint32Array(readbackBuffer.getMappedRange(0, outputBytes).slice(0));
  readbackBuffer.unmap();

  existingBuffer.destroy();
  candidateBuffer.destroy();
  outputBuffer.destroy();
  readbackBuffer.destroy();

  return result;
}

async function runCanonicalization(wasm, device) {
  const fixture = {
    existing: [[1, 2], [9, 9]],
    candidates: [[1, 2], [4, 5], [4, 5], [1, 3], [9, 9], [1, 3]],
  };
  const expectedFlags = new Uint32Array([1, 4, 2, 4, 1, 2]);
  const expectedNewPairs = [[1, 3], [4, 5]];

  const cpu = cpuCanonicalization(wasm, fixture);
  assertExactU32(expectedFlags, cpu, "Rust/WASM canonicalization");
  report(ui.canonicalCpu, `PASS [${[...cpu].join(", ")}]`, true);

  const gpu = await gpuCanonicalization(device, fixture);
  assertExactU32(expectedFlags, gpu, "WebGPU canonicalization");
  report(ui.canonicalGpu, `PASS [${[...gpu].join(", ")}]`, true);

  assertExactU32(cpu, gpu, "CPU/GPU canonicalization differential");
  report(ui.canonicalDiff, "PASS", true);

  const cpuPairs = newPairsFromFlags(fixture.candidates, cpu);
  const gpuPairs = newPairsFromFlags(fixture.candidates, gpu);
  assertCanonicalNewPairs(expectedNewPairs, cpuPairs, "CPU canonical new pairs");
  assertCanonicalNewPairs(expectedNewPairs, gpuPairs, "GPU canonical new pairs");
  assertCanonicalNewPairs(cpuPairs, gpuPairs, "CPU/GPU canonical new-pair set");

  const formatPairs = (pairs) =>
    "[" + normalizePairSet(pairs).map((key) => "(" + key.replace(":", ",") + ")").join(", ") + "]";

  report(ui.canonicalPairsCpu, `PASS ${formatPairs(cpuPairs)}`, true);
  report(ui.canonicalPairsGpu, `PASS ${formatPairs(gpuPairs)}`, true);

  let negativeDetected = false;
  try {
    assertExactU32(cpu, perturbFirst(gpu), "canonicalization negative flag");
  } catch (error) {
    negativeDetected = true;
    log(`canonical.negative.flag = ${error.message}`);
  }
  if (!negativeDetected) {
    throw new Error("canonicalization negative flag mismatch was accepted");
  }

  let duplicateDetected = false;
  try {
    assertCanonicalNewPairs(
      expectedNewPairs,
      [...gpuPairs, gpuPairs[0]],
      "canonicalization duplicate-output negative",
    );
  } catch (error) {
    duplicateDetected = true;
    log(`canonical.negative.duplicate = ${error.message}`);
  }
  if (!duplicateDetected) {
    throw new Error("duplicate canonical output was accepted");
  }

  report(ui.canonicalNegative, "PASS (mismatches detected)", true);
  log(`canonical.cpu.flags = [${[...cpu].join(", ")}]`);
  log(`canonical.gpu.flags = [${[...gpu].join(", ")}]`);
  log(`canonical.new = ${formatPairs(cpuPairs)}`);
}

function cpuStatePairs(wasm) {
  const pairs = [];
  for (let start = 0; start < STATE_SIDE; start += 1) {
    for (let end = 0; end < STATE_SIDE; end += 1) {
      const value = wasm.stateGet(start, end);
      if (value === 0xffffffff) {
        throw new Error("WASM state query unexpectedly exceeded physical scope");
      }
      if (value !== 0) pairs.push([start, end]);
    }
  }
  return pairs;
}

function cpuStateCommitBatch(wasm, pairs) {
  return Uint32Array.from(
    pairs.map(([start, end]) => wasm.stateCommit(start, end)),
  );
}

function createGpuState(device) {
  const state = device.createBuffer({
    size: STATE_CELLS * Uint32Array.BYTES_PER_ELEMENT,
    usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC | GPUBufferUsage.COPY_DST,
  });
  device.queue.writeBuffer(state, 0, new Uint32Array(STATE_CELLS));
  return state;
}

async function gpuStateCommitBatch(device, stateBuffer, pairs) {
  const requests = new Uint32Array(pairs.flat());
  const statusesBytes = pairs.length * Uint32Array.BYTES_PER_ELEMENT;

  const requestsBuffer = device.createBuffer({
    size: Math.max(requests.byteLength, 4),
    usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
  });
  const statusesBuffer = device.createBuffer({
    size: Math.max(statusesBytes, 4),
    usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
  });
  const readbackBuffer = device.createBuffer({
    size: Math.max(statusesBytes, 4),
    usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
  });

  if (requests.byteLength > 0) device.queue.writeBuffer(requestsBuffer, 0, requests);

  const shader = device.createShaderModule({
    code: `
      @group(0) @binding(0) var<storage, read_write> state_cells: array<atomic<u32>>;
      @group(0) @binding(1) var<storage, read> requests: array<u32>;
      @group(0) @binding(2) var<storage, read_write> statuses: array<u32>;

      @compute @workgroup_size(64)
      fn main(@builtin(global_invocation_id) id: vec3<u32>) {
        let i = id.x;
        if (i < ${pairs.length}u) {
          let start = requests[i * 2u];
          let end = requests[i * 2u + 1u];

          if (start < ${STATE_SIDE}u && end < ${STATE_SIDE}u) {
            let cell = start * ${STATE_SIDE}u + end;
            atomicStore(&state_cells[cell], 1u);
            statuses[i] = 1u;
          } else {
            statuses[i] = 0u;
          }
        }
      }
    `,
  });

  const pipeline = device.createComputePipeline({
    layout: "auto",
    compute: { module: shader, entryPoint: "main" },
  });
  const bindGroup = device.createBindGroup({
    layout: pipeline.getBindGroupLayout(0),
    entries: [
      { binding: 0, resource: { buffer: stateBuffer } },
      { binding: 1, resource: { buffer: requestsBuffer } },
      { binding: 2, resource: { buffer: statusesBuffer } },
    ],
  });

  if (pairs.length > 0) {
    const encoder = device.createCommandEncoder();
    const pass = encoder.beginComputePass();
    pass.setPipeline(pipeline);
    pass.setBindGroup(0, bindGroup);
    pass.dispatchWorkgroups(Math.ceil(pairs.length / 64));
    pass.end();
    encoder.copyBufferToBuffer(statusesBuffer, 0, readbackBuffer, 0, statusesBytes);
    device.queue.submit([encoder.finish()]);
    await readbackBuffer.mapAsync(GPUMapMode.READ, 0, statusesBytes);
    const statuses = new Uint32Array(readbackBuffer.getMappedRange(0, statusesBytes).slice(0));
    readbackBuffer.unmap();

    requestsBuffer.destroy();
    statusesBuffer.destroy();
    readbackBuffer.destroy();
    return statuses;
  }

  requestsBuffer.destroy();
  statusesBuffer.destroy();
  readbackBuffer.destroy();
  return new Uint32Array();
}

async function gpuStatePairs(device, stateBuffer) {
  const byteLength = STATE_CELLS * Uint32Array.BYTES_PER_ELEMENT;
  const readback = device.createBuffer({
    size: byteLength,
    usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
  });
  const encoder = device.createCommandEncoder();
  encoder.copyBufferToBuffer(stateBuffer, 0, readback, 0, byteLength);
  device.queue.submit([encoder.finish()]);
  await readback.mapAsync(GPUMapMode.READ, 0, byteLength);
  const cells = new Uint32Array(readback.getMappedRange(0, byteLength).slice(0));
  readback.unmap();
  readback.destroy();
  return matrixToPairs(cells);
}

async function runStatefulStore(wasm, device) {
  wasm.stateReset();
  const gpuState = createGpuState(device);

  const observe = async (expected, cpuEl, gpuEl, label) => {
    const cpuPairs = cpuStatePairs(wasm);
    const gpuPairs = await gpuStatePairs(device, gpuState);
    assertStateExact(expected, cpuPairs, label + " CPU");
    assertStateExact(expected, gpuPairs, label + " GPU");
    assertStateExact(cpuPairs, gpuPairs, label + " differential");
    report(cpuEl, `PASS ${formatStatePairs(cpuPairs)}`, true);
    report(gpuEl, `PASS ${formatStatePairs(gpuPairs)}`, true);
    return { cpuPairs, gpuPairs };
  };

  const initial = [[1,2]];
  assertExactU32(new Uint32Array([1]), cpuStateCommitBatch(wasm, initial), "CPU state round0 status");
  assertExactU32(new Uint32Array([1]), await gpuStateCommitBatch(device, gpuState, initial), "GPU state round0 status");
  const r0 = await observe(initial, ui.stateR0Cpu, ui.stateR0Gpu, "round0");

  const round1Commits = [[4,5],[1,3],[4,5]];
  assertExactU32(new Uint32Array([1,1,1]), cpuStateCommitBatch(wasm, round1Commits), "CPU state round1 status");
  assertExactU32(new Uint32Array([1,1,1]), await gpuStateCommitBatch(device, gpuState, round1Commits), "GPU state round1 status");
  const expected1 = [[1,2],[1,3],[4,5]];
  const r1 = await observe(expected1, ui.stateR1Cpu, ui.stateR1Gpu, "round1");
  assertStatePreserved(r0.cpuPairs, r1.cpuPairs, "CPU round0->1");
  assertStatePreserved(r0.gpuPairs, r1.gpuPairs, "GPU round0->1");

  const round2Commits = [[1,3],[9,9]];
  assertExactU32(new Uint32Array([1,1]), cpuStateCommitBatch(wasm, round2Commits), "CPU state round2 status");
  assertExactU32(new Uint32Array([1,1]), await gpuStateCommitBatch(device, gpuState, round2Commits), "GPU state round2 status");
  const expected2 = [[1,2],[1,3],[4,5],[9,9]];
  const r2 = await observe(expected2, ui.stateR2Cpu, ui.stateR2Gpu, "round2");
  assertStatePreserved(r1.cpuPairs, r2.cpuPairs, "CPU round1->2");
  assertStatePreserved(r1.gpuPairs, r2.gpuPairs, "GPU round1->2");
  report(ui.stateDiff, "PASS", true);

  // Out-of-scope commits must fail closed and leave state unchanged.
  assertExactU32(new Uint32Array([0]), cpuStateCommitBatch(wasm, [[16,1]]), "CPU out-of-range status");
  assertExactU32(new Uint32Array([0]), await gpuStateCommitBatch(device, gpuState, [[16,1]]), "GPU out-of-range status");
  assertStateExact(expected2, cpuStatePairs(wasm), "CPU range fail-closed");
  assertStateExact(expected2, await gpuStatePairs(device, gpuState), "GPU range fail-closed");

  // Deliberate state-loss witness: a recreated state containing only round2 commits must fail.
  let lossDetected = false;
  try {
    assertStatePreserved(r1.gpuPairs, [[1,3],[9,9]], "state-loss negative control");
  } catch (error) {
    lossDetected = true;
    log(`state.negative.loss = ${error.message}`);
  }
  if (!lossDetected) throw new Error("state loss negative control was accepted");

  // Deliberate unauthorized extra pair must fail exact comparison.
  let unauthorizedDetected = false;
  try {
    assertStateExact(expected2, [...r2.gpuPairs,[7,7]], "unauthorized pair negative");
  } catch (error) {
    unauthorizedDetected = true;
    log(`state.negative.unauthorized = ${error.message}`);
  }
  if (!unauthorizedDetected) throw new Error("unauthorized pair negative control was accepted");

  report(ui.stateNegative, "PASS (state loss/range/extra pair detected)", true);
  log(`state.round2 = ${formatStatePairs(r2.gpuPairs)}`);

  gpuState.destroy();
}

async function main() {
  log(`secureContext = ${window.isSecureContext}`);
  log(`location = ${location.href}`);

  let wasm = null;
  try {
    wasm = await loadWasm();
  } catch (error) {
    report(ui.wasm, "FAILED", false);
    report(ui.cpu, "NOT_RUN", false);
    report(ui.incidenceCpu, "NOT_RUN", false);
    log(`WASM error: ${error?.stack || error}`);
  }

  const device = await initWebGpu();
  if (!device) {
    report(ui.compute, "NOT_RUN", false);
    report(ui.incidenceGpu, "NOT_RUN", false);
    report(ui.differential, "NOT_RUN", false);
    report(ui.negative, "NOT_RUN", false);
    report(ui.canonicalGpu, "NOT_RUN", false);
    report(ui.canonicalDiff, "NOT_RUN", false);
    report(ui.canonicalNegative, "NOT_RUN", false);
    report(ui.stateR0Gpu, "NOT_RUN", false);
    report(ui.stateR1Gpu, "NOT_RUN", false);
    report(ui.stateR2Gpu, "NOT_RUN", false);
    report(ui.stateDiff, "NOT_RUN", false);
    report(ui.stateNegative, "NOT_RUN", false);
    report(ui.anumImportCpu, wasm ? "WAITING_FOR_GPU" : "NOT_RUN", false);
    report(ui.anumImportGpu, "NOT_RUN", false);
    report(ui.anumExportCpu, wasm ? "WAITING_FOR_GPU" : "NOT_RUN", false);
    report(ui.anumExportGpu, "NOT_RUN", false);
    report(ui.anumDiff, "NOT_RUN", false);
    report(ui.anumHandles, "NOT_RUN", false);
    report(ui.anumReuse, "NOT_RUN", false);
    report(ui.anumNegative, "NOT_RUN", false);
    for (const element of [
      ui.reactionBeforeCpu, ui.reactionBeforeGpu, ui.reactionAfterCpu, ui.reactionAfterGpu,
      ui.reactionMatched, ui.reactionHandoff, ui.reactionDiff, ui.reactionHandles,
      ui.reactionScope, ui.reactionNegative,
      ui.reactionR2Cpu, ui.reactionR2Gpu, ui.reactionR2Matched, ui.reactionR2Handoff,
      ui.reactionR2Quiescence, ui.reactionR2Bank, ui.reactionR2Diff, ui.reactionR2Failure,
      ui.reactionR3ZeroCpu, ui.reactionR3ZeroGpu, ui.reactionR3ZeroMatched, ui.reactionR3ZeroHandoff,
      ui.reactionR3ZeroQuiescence, ui.reactionR3ZeroOldScope, ui.reactionR3MixedCpu, ui.reactionR3MixedGpu,
      ui.reactionR3MixedMatched, ui.reactionR3MixedHandoff, ui.reactionR3MixedQuiescence,
      ui.reactionR3Duplicates, ui.reactionR3Diff,
      ui.reactionR4TBefore, ui.reactionR4TSnapshot, ui.reactionR4LiveTheory, ui.reactionR4TAfter,
      ui.reactionR4TMatched, ui.reactionR4THandoff, ui.reactionR4T1Snapshot, ui.reactionR4T1Before,
      ui.reactionR4T1After, ui.reactionR4T1Matched, ui.reactionR4T1Handoff, ui.reactionR4Isolation,
      ui.reactionR4Visibility, ui.reactionR4Stale, ui.reactionR4Diff,
      ui.reactionR5S0, ui.reactionR5S1, ui.reactionR5S2, ui.reactionR5S3, ui.reactionR5S4,
      ui.reactionR5Snapshot, ui.reactionR5Matched, ui.reactionR5Handoff, ui.reactionR5Quiescence,
      ui.reactionR5Recurrence, ui.reactionR5EndStructure, ui.reactionR5EndContinuation,
      ui.reactionR5Bounded, ui.reactionR5Diff,
    ]) report(element, wasm ? "WAITING_FOR_GPU" : "NOT_RUN", false);
    return;
  }

  try {
    await runSmoke(device);
  } catch (error) {
    report(ui.compute, "FAILED", false);
    log(`GPU smoke error: ${error?.stack || error}`);
  }

  if (wasm) {
    try {
      await runDifferential(wasm, device);
    } catch (error) {
      report(ui.differential, "FAIL", false);
      if (ui.incidenceCpu.textContent === "WAITING") report(ui.incidenceCpu, "FAIL", false);
      if (ui.incidenceGpu.textContent === "WAITING") report(ui.incidenceGpu, "FAIL", false);
      if (ui.negative.textContent === "WAITING") report(ui.negative, "FAIL", false);
      log(`Differential error: ${error?.stack || error}`);
    }
  }

  if (wasm) {
    try {
      await runCanonicalization(wasm, device);
    } catch (error) {
      report(ui.canonicalDiff, "FAIL", false);
      if (ui.canonicalCpu.textContent === "WAITING") report(ui.canonicalCpu, "FAIL", false);
      if (ui.canonicalGpu.textContent === "WAITING") report(ui.canonicalGpu, "FAIL", false);
      if (ui.canonicalPairsCpu.textContent === "WAITING") report(ui.canonicalPairsCpu, "FAIL", false);
      if (ui.canonicalPairsGpu.textContent === "WAITING") report(ui.canonicalPairsGpu, "FAIL", false);
      if (ui.canonicalNegative.textContent === "WAITING") report(ui.canonicalNegative, "FAIL", false);
      log(`Canonicalization error: ${error?.stack || error}`);
    }
  }

  if (wasm) {
    try {
      await runStatefulStore(wasm, device);
    } catch (error) {
      report(ui.stateDiff, "FAIL", false);
      for (const element of [
        ui.stateR0Cpu, ui.stateR0Gpu, ui.stateR1Cpu, ui.stateR1Gpu,
        ui.stateR2Cpu, ui.stateR2Gpu, ui.stateNegative,
      ]) {
        if (element.textContent === "WAITING") report(element, "FAIL", false);
      }
      log(`State-store error: ${error?.stack || error}`);
    }
  }

  if (wasm) {
    try {
      const anum = await runAnumBoundaryBrowser(wasm, device);
      report(ui.anumImportCpu, "PASS", anum.cpuImport);
      report(ui.anumImportGpu, "PASS", anum.gpuImport);
      report(ui.anumExportCpu, "PASS", anum.cpuExport);
      report(ui.anumExportGpu, "PASS", anum.gpuExport);
      report(ui.anumDiff, "PASS", anum.differential);
      report(
        ui.anumHandles,
        `PASS (CPU ${anum.sample.cpuHandle} != GPU ${anum.sample.gpuHandle})`,
        anum.handlesDiffer,
      );
      report(ui.anumReuse, "PASS", anum.canonicalReuse);
      report(
        ui.anumNegative,
        "PASS (malformed/truncated/trailing/foreign/capacity rejected)",
        anum.negativeControls,
      );
      for (const line of anum.logs) log(line);
    } catch (error) {
      for (const element of [
        ui.anumImportCpu, ui.anumImportGpu, ui.anumExportCpu, ui.anumExportGpu,
        ui.anumDiff, ui.anumHandles, ui.anumReuse, ui.anumNegative,
      ]) {
        if (element.textContent === "WAITING") report(element, "FAIL", false);
      }
      log(`Anum-boundary error: ${error?.stack || error}`);
    }
  }

  if (wasm) {
    try {
      const reaction = await runReactionBrowser(wasm, device);
      report(ui.reactionBeforeCpu, "PASS [" + reaction.cpuBefore.join(", ") + "]", true);
      report(ui.reactionBeforeGpu, "PASS [" + reaction.gpuBefore.join(", ") + "]", true);
      report(ui.reactionAfterCpu, "PASS [" + reaction.cpuAfter.join(", ") + "]", true);
      report(ui.reactionAfterGpu, "PASS [" + reaction.gpuAfter.join(", ") + "]", true);
      report(
        ui.reactionMatched,
        "PASS (CPU " + reaction.cpuMatched + " / GPU " + reaction.gpuMatched + ")",
        reaction.cpuMatched === 1 && reaction.gpuMatched === 1,
      );
      report(
        ui.reactionHandoff,
        "PASS (CPU " + reaction.cpuHandoff + " / GPU " + reaction.gpuHandoff + ")",
        reaction.cpuHandoff === 1 && reaction.gpuHandoff === 1,
      );
      report(ui.reactionDiff, "PASS", reaction.normalizedDifferential);
      report(ui.reactionHandles, "PASS (backend-local handles differ)", reaction.handlesDiffer);
      report(
        ui.reactionScope,
        "PASS (snapshot isolated / old Scope retained)",
        reaction.snapshotIsolation && reaction.oldScopeRetained,
      );
      report(
        ui.reactionNegative,
        "PASS (mismatch/no-match/foreign detected)",
        reaction.negativeControls,
      );
      report(ui.reactionR2Cpu, "PASS [" + reaction.r2CpuState.join(", ") + "]", true);
      report(ui.reactionR2Gpu, "PASS [" + reaction.r2GpuState.join(", ") + "]", true);
      report(
        ui.reactionR2Matched,
        "PASS (CPU " + reaction.r2CpuMatched + " / GPU " + reaction.r2GpuMatched + ")",
        reaction.r2CpuMatched === 0 && reaction.r2GpuMatched === 0,
      );
      report(
        ui.reactionR2Handoff,
        "PASS (CPU " + reaction.r2CpuHandoff + " / GPU " + reaction.r2GpuHandoff + ")",
        reaction.r2CpuHandoff === 0 && reaction.r2GpuHandoff === 0,
      );
      report(
        ui.reactionR2Quiescence,
        "PASS (CPU " + reaction.r2CpuQuiescent + " / GPU " + reaction.r2GpuQuiescent + ")",
        reaction.r2CpuQuiescent && reaction.r2GpuQuiescent,
      );
      report(
        ui.reactionR2Bank,
        "PASS (published Scope unchanged)",
        reaction.r2CpuBankUnchanged && reaction.r2GpuBankUnchanged,
      );
      report(ui.reactionR2Diff, "PASS", reaction.r2NormalizedDifferential);
      report(
        ui.reactionR2Failure,
        "PASS (runtime failure != quiescence)",
        reaction.r2FailureNotQuiescent,
      );
      report(ui.reactionR3ZeroCpu, "PASS [" + reaction.r3ZeroCpuState.join(", ") + "]", true);
      report(ui.reactionR3ZeroGpu, "PASS [" + reaction.r3ZeroGpuState.join(", ") + "]", true);
      report(
        ui.reactionR3ZeroMatched,
        "PASS (CPU " + reaction.r3ZeroCpuMatched + " / GPU " + reaction.r3ZeroGpuMatched + ")",
        reaction.r3ZeroCpuMatched === 1 && reaction.r3ZeroGpuMatched === 1,
      );
      report(
        ui.reactionR3ZeroHandoff,
        "PASS (CPU " + reaction.r3ZeroCpuHandoff + " / GPU " + reaction.r3ZeroGpuHandoff + ")",
        reaction.r3ZeroCpuHandoff === 1 && reaction.r3ZeroGpuHandoff === 1,
      );
      report(
        ui.reactionR3ZeroQuiescence,
        "PASS (CPU " + reaction.r3ZeroCpuQuiescent + " / GPU " + reaction.r3ZeroGpuQuiescent + ")",
        !reaction.r3ZeroCpuQuiescent && !reaction.r3ZeroGpuQuiescent,
      );
      report(
        ui.reactionR3ZeroOldScope,
        "PASS (old Scope retained physically)",
        reaction.r3ZeroOldScopeRetained,
      );
      report(ui.reactionR3MixedCpu, "PASS [" + reaction.r3MixedCpuState.join(", ") + "]", true);
      report(ui.reactionR3MixedGpu, "PASS [" + reaction.r3MixedGpuState.join(", ") + "]", true);
      report(
        ui.reactionR3MixedMatched,
        "PASS (CPU " + reaction.r3MixedCpuMatched + " / GPU " + reaction.r3MixedGpuMatched + ")",
        reaction.r3MixedCpuMatched === 3 && reaction.r3MixedGpuMatched === 3,
      );
      report(
        ui.reactionR3MixedHandoff,
        "PASS (CPU " + reaction.r3MixedCpuHandoff + " / GPU " + reaction.r3MixedGpuHandoff + ")",
        reaction.r3MixedCpuHandoff === 1 && reaction.r3MixedGpuHandoff === 1,
      );
      report(
        ui.reactionR3MixedQuiescence,
        "PASS (CPU " + reaction.r3MixedCpuQuiescent + " / GPU " + reaction.r3MixedGpuQuiescent + ")",
        !reaction.r3MixedCpuQuiescent && !reaction.r3MixedGpuQuiescent,
      );
      report(
        ui.reactionR3Duplicates,
        "PASS (single canonical successor)",
        reaction.r3DuplicateConvergence,
      );
      report(ui.reactionR3Diff, "PASS", reaction.r3NormalizedDifferential);
      report(
        ui.reactionR4TBefore,
        "PASS (CPU [" + reaction.r4TBeforeCpu.join(", ") + "] / GPU [" + reaction.r4TBeforeGpu.join(", ") + "])",
        true,
      );
      report(
        ui.reactionR4TSnapshot,
        "PASS (CPU " + reaction.r4TSnapshotCountCpu + " / GPU " + reaction.r4TSnapshotCountGpu + ")",
        reaction.r4TSnapshotCountCpu === 1 && reaction.r4TSnapshotCountGpu === 1,
      );
      report(
        ui.reactionR4LiveTheory,
        "PASS (CPU " + reaction.r4LiveTheoryCountCpu + " / GPU " + reaction.r4LiveTheoryCountGpu + ")",
        reaction.r4LiveTheoryCountCpu === 2 && reaction.r4LiveTheoryCountGpu === 2,
      );
      report(
        ui.reactionR4TAfter,
        "PASS (CPU [" + reaction.r4TAfterCpu.join(", ") + "] / GPU [" + reaction.r4TAfterGpu.join(", ") + "])",
        true,
      );
      report(
        ui.reactionR4TMatched,
        "PASS (CPU " + reaction.r4TMatchedCpu + " / GPU " + reaction.r4TMatchedGpu + ")",
        reaction.r4TMatchedCpu === 1 && reaction.r4TMatchedGpu === 1,
      );
      report(
        ui.reactionR4THandoff,
        "PASS (CPU " + reaction.r4THandoffCpu + " / GPU " + reaction.r4THandoffGpu + ")",
        reaction.r4THandoffCpu === 1 && reaction.r4THandoffGpu === 1,
      );
      report(
        ui.reactionR4T1Snapshot,
        "PASS (CPU " + reaction.r4T1SnapshotCountCpu + " / GPU " + reaction.r4T1SnapshotCountGpu + ")",
        reaction.r4T1SnapshotCountCpu === 2 && reaction.r4T1SnapshotCountGpu === 2,
      );
      report(
        ui.reactionR4T1Before,
        "PASS (CPU [" + reaction.r4T1BeforeCpu.join(", ") + "] / GPU [" + reaction.r4T1BeforeGpu.join(", ") + "])",
        true,
      );
      report(
        ui.reactionR4T1After,
        "PASS (CPU [" + reaction.r4T1AfterCpu.join(", ") + "] / GPU [" + reaction.r4T1AfterGpu.join(", ") + "])",
        true,
      );
      report(
        ui.reactionR4T1Matched,
        "PASS (CPU " + reaction.r4T1MatchedCpu + " / GPU " + reaction.r4T1MatchedGpu + ")",
        reaction.r4T1MatchedCpu === 1 && reaction.r4T1MatchedGpu === 1,
      );
      report(
        ui.reactionR4T1Handoff,
        "PASS (CPU " + reaction.r4T1HandoffCpu + " / GPU " + reaction.r4T1HandoffGpu + ")",
        reaction.r4T1HandoffCpu === 1 && reaction.r4T1HandoffGpu === 1,
      );
      report(ui.reactionR4Isolation, "PASS", reaction.r4SameReactionIsolation);
      report(ui.reactionR4Visibility, "PASS", reaction.r4NextReactionVisibility);
      report(ui.reactionR4Stale, "PASS (new admission invisible without new snapshot)", reaction.r4StaleSnapshotControl);
      report(ui.reactionR4Diff, "PASS", reaction.r4NormalizedTrajectoryDifferential);
      const r5StateLabel = (index) =>
        "PASS (CPU [" + reaction.r5StatesCpu[index].join(", ") + "] / GPU [" +
        reaction.r5StatesGpu[index].join(", ") + "])";
      report(ui.reactionR5S0, r5StateLabel(0), true);
      report(ui.reactionR5S1, r5StateLabel(1), true);
      report(ui.reactionR5S2, r5StateLabel(2), true);
      report(ui.reactionR5S3, r5StateLabel(3), true);
      report(ui.reactionR5S4, r5StateLabel(4), true);
      report(
        ui.reactionR5Snapshot,
        "PASS (CPU " + reaction.r5SnapshotCountCpu + " / GPU " + reaction.r5SnapshotCountGpu + ")",
        reaction.r5SnapshotCountCpu === 2 && reaction.r5SnapshotCountGpu === 2,
      );
      report(
        ui.reactionR5Matched,
        "PASS (CPU [" + reaction.r5MatchedCpu.join(", ") + "] / GPU [" + reaction.r5MatchedGpu.join(", ") + "])",
        reaction.r5MatchedCpu.every((value) => value === 1) &&
          reaction.r5MatchedGpu.every((value) => value === 1),
      );
      report(
        ui.reactionR5Handoff,
        "PASS (CPU [" + reaction.r5HandoffCpu.join(", ") + "] / GPU [" + reaction.r5HandoffGpu.join(", ") + "])",
        reaction.r5HandoffCpu.every((value) => value === 1) &&
          reaction.r5HandoffGpu.every((value) => value === 1),
      );
      report(
        ui.reactionR5Quiescence,
        "PASS (CPU [" + reaction.r5QuiescentCpu.join(", ") + "] / GPU [" + reaction.r5QuiescentGpu.join(", ") + "])",
        reaction.r5QuiescentCpu.every((value) => value === false) &&
          reaction.r5QuiescentGpu.every((value) => value === false),
      );
      report(ui.reactionR5Recurrence, "PASS (S0=S2=S4, S1=S3)", reaction.r5Recurrence);
      report(ui.reactionR5EndStructure, "PASS (C=68 is structural END)", reaction.r5EndStructure);
      report(
        ui.reactionR5EndContinuation,
        "PASS (K⟼C -> K⟼A remains active)",
        reaction.r5EndContinuation,
      );
      report(ui.reactionR5Bounded, "PASS (4 active steps, witness returned)", reaction.r5BoundedReturn);
      report(ui.reactionR5Diff, "PASS", reaction.r5NormalizedTrajectoryDifferential);
      report(
        ui.reactionR6OneMany,
        "PASS (CPU [" + reaction.r6OneToNCpuState.join(", ") + "] / GPU [" + reaction.r6OneToNGpuState.join(", ") + "])",
        reaction.r6OneToNMatchedCpu === 2 && reaction.r6OneToNMatchedGpu === 2,
      );
      report(
        ui.reactionR6NM,
        "PASS (CPU [" + reaction.r6NToMCpuState.join(", ") + "] / GPU [" + reaction.r6NToMGpuState.join(", ") + "])",
        reaction.r6NToMCpuState.length === 2 && reaction.r6NToMGpuState.length === 2,
      );
      report(ui.reactionR6Order, "PASS (reversed current + Theory order)", reaction.r6OrderVariation);
      report(
        ui.reactionR6Scope,
        "PASS (CPU 17>16 rejected / GPU 5>4 rejected)",
        reaction.r6CpuOutOfScopeRejected && reaction.r6GpuOutOfScopeRejected,
      );
      report(ui.reactionR6Diff, "PASS", reaction.r6NormalizedDifferential);
      for (const line of reaction.logs) log(line);
    } catch (error) {
      for (const element of [
        ui.reactionBeforeCpu, ui.reactionBeforeGpu, ui.reactionAfterCpu, ui.reactionAfterGpu,
        ui.reactionMatched, ui.reactionHandoff, ui.reactionDiff, ui.reactionHandles,
        ui.reactionScope, ui.reactionNegative,
        ui.reactionR2Cpu, ui.reactionR2Gpu, ui.reactionR2Matched, ui.reactionR2Handoff,
        ui.reactionR2Quiescence, ui.reactionR2Bank, ui.reactionR2Diff, ui.reactionR2Failure,
        ui.reactionR3ZeroCpu, ui.reactionR3ZeroGpu, ui.reactionR3ZeroMatched, ui.reactionR3ZeroHandoff,
        ui.reactionR3ZeroQuiescence, ui.reactionR3ZeroOldScope, ui.reactionR3MixedCpu, ui.reactionR3MixedGpu,
        ui.reactionR3MixedMatched, ui.reactionR3MixedHandoff, ui.reactionR3MixedQuiescence,
        ui.reactionR3Duplicates, ui.reactionR3Diff,
        ui.reactionR4TBefore, ui.reactionR4TSnapshot, ui.reactionR4LiveTheory, ui.reactionR4TAfter,
        ui.reactionR4TMatched, ui.reactionR4THandoff, ui.reactionR4T1Snapshot, ui.reactionR4T1Before,
        ui.reactionR4T1After, ui.reactionR4T1Matched, ui.reactionR4T1Handoff, ui.reactionR4Isolation,
        ui.reactionR4Visibility, ui.reactionR4Stale, ui.reactionR4Diff,
        ui.reactionR5S0, ui.reactionR5S1, ui.reactionR5S2, ui.reactionR5S3, ui.reactionR5S4,
        ui.reactionR5Snapshot, ui.reactionR5Matched, ui.reactionR5Handoff, ui.reactionR5Quiescence,
        ui.reactionR5Recurrence, ui.reactionR5EndStructure, ui.reactionR5EndContinuation,
        ui.reactionR5Bounded, ui.reactionR5Diff,
        ui.reactionR6OneMany, ui.reactionR6NM, ui.reactionR6Order, ui.reactionR6Scope, ui.reactionR6Diff,
      ]) {
        if (element.textContent === "WAITING") report(element, "FAIL", false);
      }
      log("Reaction R1/R2/R3/R4/R5/R6 error: " + (error?.stack || error));
    }
  }

  device.destroy();
}

main();
