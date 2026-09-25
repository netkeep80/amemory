import { assertExactU32, perturbFirst } from "./differential.mjs";
import {
  assertCanonicalNewPairs,
  newPairsFromFlags,
  normalizePairSet,
} from "./canonicalization.mjs";

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

  if ([probe, cpuStep, incidenceFlag, canonicalSetExisting, canonicalSetCandidate, canonicalFlag]
      .some((fn) => typeof fn !== "function")) {
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

  return { incidenceFlag, canonicalSetExisting, canonicalSetCandidate, canonicalFlag };
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

  device.destroy();
}

main();
