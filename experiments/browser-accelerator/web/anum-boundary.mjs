export const ANUM_FIXTURES = Object.freeze([
  "8",
  "98",
  "68",
  "19868",
  "16898",
  "198698",
  "119868968",
]);

export const LOCAL_HANDLE_NONE = 0xffffffff;
export const LOCAL_HANDLE_MAX = 63;
export const GPU_ROOT_HANDLE = 63;
const GPU_BUFFER_WORDS = 64 * 3;
const GPU_BUFFER_BYTES = GPU_BUFFER_WORDS * Uint32Array.BYTES_PER_ELEMENT;

function parseNode(source, cursor) {
  if (cursor.index >= source.length) {
    throw new Error("truncated Anum");
  }
  const token = source[cursor.index++];
  if (token === "8") return { kind: "ROOT" };
  if (token === "9") return { kind: "START", child: parseNode(source, cursor) };
  if (token === "6") return { kind: "END", child: parseNode(source, cursor) };
  if (token === "1") {
    return {
      kind: "PAIR",
      start: parseNode(source, cursor),
      end: parseNode(source, cursor),
    };
  }
  throw new Error(`malformed Anum token '${token}'`);
}

export function parseAnum(source) {
  if (typeof source !== "string" || source.length === 0) {
    throw new Error("Anum must be a non-empty string");
  }
  const cursor = { index: 0 };
  const root = parseNode(source, cursor);
  if (cursor.index !== source.length) {
    throw new Error(`trailing garbage at offset ${cursor.index}`);
  }
  return root;
}

export function formatAnum(node) {
  if (node.kind === "ROOT") return "8";
  if (node.kind === "START") return "9" + formatAnum(node.child);
  if (node.kind === "END") return "6" + formatAnum(node.child);
  if (node.kind === "PAIR") return "1" + formatAnum(node.start) + formatAnum(node.end);
  throw new Error(`unknown Anum node kind: ${node.kind}`);
}

export function normalizeAnum(source) {
  return formatAnum(parseAnum(source));
}

export function compileAnumPlan(source) {
  const parsed = parseAnum(source);
  const nodes = [];

  function emit(node) {
    if (node.kind === "ROOT") {
      const index = nodes.length;
      nodes.push({ kind: 0, a: LOCAL_HANDLE_NONE, b: LOCAL_HANDLE_NONE });
      return index;
    }
    if (node.kind === "START") {
      const child = emit(node.child);
      const index = nodes.length;
      nodes.push({ kind: 1, a: child, b: LOCAL_HANDLE_NONE });
      return index;
    }
    if (node.kind === "END") {
      const child = emit(node.child);
      const index = nodes.length;
      nodes.push({ kind: 2, a: child, b: LOCAL_HANDLE_NONE });
      return index;
    }
    const start = emit(node.start);
    const end = emit(node.end);
    const index = nodes.length;
    nodes.push({ kind: 3, a: start, b: end });
    return index;
  }

  const rootNode = emit(parsed);
  if (nodes.length > LOCAL_HANDLE_MAX) {
    throw new Error(
      `prototype local capacity exceeded: plan has ${nodes.length} nodes, max ${LOCAL_HANDLE_MAX}`,
    );
  }
  return { source, nodes, rootNode };
}

export function localRef(memory, value) {
  if (!Number.isInteger(value) || value < 1 || value > LOCAL_HANDLE_MAX) {
    throw new Error(`invalid local handle value: ${value}`);
  }
  return Object.freeze({ memory, value });
}

export function requireLocalRef(ref, memory) {
  if (!ref || ref.memory !== memory || !Number.isInteger(ref.value)) {
    throw new Error(`foreign local handle rejected by ${memory}`);
  }
  return ref.value;
}

function validateTopology(topology) {
  const { starts, ends, used } = topology;
  if (!starts || !ends || !used || starts.length < 64 || ends.length < 64 || used.length < 64) {
    throw new Error("invalid local topology buffers");
  }
}

export function topologyCount(topology) {
  validateTopology(topology);
  let count = 0;
  for (let h = 1; h <= LOCAL_HANDLE_MAX; h += 1) {
    if (topology.used[h] !== 0) count += 1;
  }
  return count;
}

export function exportLocalTopology(topology, handle) {
  validateTopology(topology);
  const visiting = new Uint8Array(64);

  function walk(h) {
    if (!Number.isInteger(h) || h < 1 || h > LOCAL_HANDLE_MAX || topology.used[h] === 0) {
      throw new Error(`unknown local handle: ${h}`);
    }
    if (visiting[h]) throw new Error(`non-well-founded local topology at handle ${h}`);
    visiting[h] = 1;

    const start = topology.starts[h];
    const end = topology.ends[h];
    let out;
    if (start === h && end === h) {
      out = "8";
    } else if (start === h) {
      out = "9" + walk(end);
    } else if (end === h) {
      out = "6" + walk(start);
    } else {
      out = "1" + walk(start) + walk(end);
    }

    visiting[h] = 0;
    return out;
  }

  return walk(handle);
}

function sourceTokens(source) {
  if (typeof source !== "string") return [];
  return Array.from(source, (ch) => {
    const code = ch.charCodeAt(0) - 48;
    return code >= 0 && code <= 9 ? code : 255;
  });
}

export function cpuResetPool(wasm) {
  wasm.anumCpuResetPool();
}

export function cpuPoolCount(wasm) {
  return wasm.anumCpuPoolCount();
}

export function cpuImportRaw(wasm, source, memory = "cpu-A") {
  const tokens = sourceTokens(source);
  if (tokens.length > 256) return null;
  for (let i = 0; i < tokens.length; i += 1) {
    if (wasm.anumCpuSetToken(i, tokens[i]) !== 1) return null;
  }
  const handle = wasm.anumCpuImport(tokens.length);
  if (handle === LOCAL_HANDLE_NONE) return null;
  return localRef(memory, handle);
}

export function cpuExport(wasm, ref, memory = "cpu-A") {
  const handle = requireLocalRef(ref, memory);
  const length = wasm.anumCpuExport(handle);
  if (length === LOCAL_HANDLE_NONE) throw new Error("CPU export rejected local topology");
  let result = "";
  for (let i = 0; i < length; i += 1) {
    const token = wasm.anumCpuOutputGet(i);
    if (![1, 6, 8, 9].includes(token)) {
      throw new Error(`CPU export emitted invalid token ${token}`);
    }
    result += String(token);
  }
  return result;
}

export function createGpuAnumPool(device, memory = "gpu-B") {
  const initial = new Uint32Array(GPU_BUFFER_WORDS);
  initial[GPU_ROOT_HANDLE] = GPU_ROOT_HANDLE;
  initial[64 + GPU_ROOT_HANDLE] = GPU_ROOT_HANDLE;
  initial[128 + GPU_ROOT_HANDLE] = 1;

  const buffer = device.createBuffer({
    size: GPU_BUFFER_BYTES,
    usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC | GPUBufferUsage.COPY_DST,
  });
  device.queue.writeBuffer(buffer, 0, initial);
  return { memory, buffer };
}

export async function readGpuTopology(device, gpuPool) {
  const readback = device.createBuffer({
    size: GPU_BUFFER_BYTES,
    usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
  });
  const encoder = device.createCommandEncoder();
  encoder.copyBufferToBuffer(gpuPool.buffer, 0, readback, 0, GPU_BUFFER_BYTES);
  device.queue.submit([encoder.finish()]);
  await readback.mapAsync(GPUMapMode.READ, 0, GPU_BUFFER_BYTES);
  const words = new Uint32Array(readback.getMappedRange(0, GPU_BUFFER_BYTES).slice(0));
  readback.unmap();
  readback.destroy();
  return {
    starts: words.slice(0, 64),
    ends: words.slice(64, 128),
    used: words.slice(128, 192),
  };
}

export async function gpuPoolCount(device, gpuPool) {
  return topologyCount(await readGpuTopology(device, gpuPool));
}

export async function gpuImport(device, gpuPool, source) {
  const plan = compileAnumPlan(source);
  const planWords = new Uint32Array(plan.nodes.length * 4);
  plan.nodes.forEach((node, index) => {
    planWords[index * 4] = node.kind;
    planWords[index * 4 + 1] = node.a;
    planWords[index * 4 + 2] = node.b;
    planWords[index * 4 + 3] = 0;
  });

  const scratch = device.createBuffer({
    size: GPU_BUFFER_BYTES,
    usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC | GPUBufferUsage.COPY_DST,
  });
  const planBuffer = device.createBuffer({
    size: planWords.byteLength,
    usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
  });
  const nodeHandles = device.createBuffer({
    size: plan.nodes.length * Uint32Array.BYTES_PER_ELEMENT,
    usage: GPUBufferUsage.STORAGE,
  });
  const status = device.createBuffer({
    size: 8,
    usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC | GPUBufferUsage.COPY_DST,
  });
  const statusReadback = device.createBuffer({
    size: 8,
    usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
  });

  device.queue.writeBuffer(planBuffer, 0, planWords);
  device.queue.writeBuffer(status, 0, new Uint32Array([0, LOCAL_HANDLE_NONE]));

  const shader = device.createShaderModule({
    code: `
      const CAP: u32 = 64u;
      const ROOT: u32 = 63u;
      const NONE: u32 = 0xffffffffu;

      @group(0) @binding(0) var<storage, read_write> pool: array<u32>;
      @group(0) @binding(1) var<storage, read> plan: array<u32>;
      @group(0) @binding(2) var<storage, read_write> node_handles: array<u32>;
      @group(0) @binding(3) var<storage, read_write> status: array<u32>;

      fn used(h: u32) -> bool {
        return pool[128u + h] != 0u;
      }

      @compute @workgroup_size(1)
      fn main(@builtin(global_invocation_id) id: vec3<u32>) {
        if (id.x != 0u) { return; }

        var i = 0u;
        loop {
          if (i >= ${plan.nodes.length}u) { break; }

          let kind = plan[i * 4u];
          var a = NONE;
          var b = NONE;

          if (kind == 1u || kind == 2u) {
            let child_index = plan[i * 4u + 1u];
            if (child_index >= i) {
              status[0] = 0u;
              status[1] = NONE;
              return;
            }
            a = node_handles[child_index];
            if (a == NONE) {
              status[0] = 0u;
              status[1] = NONE;
              return;
            }
          } else if (kind == 3u) {
            let start_index = plan[i * 4u + 1u];
            let end_index = plan[i * 4u + 2u];
            if (start_index >= i || end_index >= i) {
              status[0] = 0u;
              status[1] = NONE;
              return;
            }
            a = node_handles[start_index];
            b = node_handles[end_index];
            if (a == NONE || b == NONE) {
              status[0] = 0u;
              status[1] = NONE;
              return;
            }
          }

          if (kind == 0u) {
            node_handles[i] = ROOT;
            i = i + 1u;
            continue;
          }

          var found = NONE;
          var h = 1u;
          loop {
            if (h >= ROOT) { break; }
            if (used(h)) {
              let s = pool[h];
              let e = pool[64u + h];
              if ((kind == 1u && s == h && e == a && e != h) ||
                  (kind == 2u && s == a && e == h && s != h) ||
                  (kind == 3u && s == a && e == b && s != h && e != h)) {
                found = h;
                break;
              }
            }
            h = h + 1u;
          }

          if (found == NONE) {
            var candidate = ROOT - 1u;
            loop {
              if (!used(candidate)) {
                found = candidate;
                break;
              }
              if (candidate == 1u) { break; }
              candidate = candidate - 1u;
            }

            if (found == NONE) {
              status[0] = 0u;
              status[1] = NONE;
              return;
            }

            pool[128u + found] = 1u;
            if (kind == 1u) {
              pool[found] = found;
              pool[64u + found] = a;
            } else if (kind == 2u) {
              pool[found] = a;
              pool[64u + found] = found;
            } else if (kind == 3u) {
              pool[found] = a;
              pool[64u + found] = b;
            } else {
              status[0] = 0u;
              status[1] = NONE;
              return;
            }
          }

          node_handles[i] = found;
          i = i + 1u;
        }

        status[0] = 1u;
        status[1] = node_handles[${plan.rootNode}u];
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
      { binding: 0, resource: { buffer: scratch } },
      { binding: 1, resource: { buffer: planBuffer } },
      { binding: 2, resource: { buffer: nodeHandles } },
      { binding: 3, resource: { buffer: status } },
    ],
  });

  const encoder = device.createCommandEncoder();
  encoder.copyBufferToBuffer(gpuPool.buffer, 0, scratch, 0, GPU_BUFFER_BYTES);
  const pass = encoder.beginComputePass();
  pass.setPipeline(pipeline);
  pass.setBindGroup(0, bindGroup);
  pass.dispatchWorkgroups(1);
  pass.end();
  encoder.copyBufferToBuffer(status, 0, statusReadback, 0, 8);
  device.queue.submit([encoder.finish()]);

  await statusReadback.mapAsync(GPUMapMode.READ, 0, 8);
  const result = new Uint32Array(statusReadback.getMappedRange(0, 8).slice(0));
  statusReadback.unmap();

  if (result[0] === 1) {
    const commit = device.createCommandEncoder();
    commit.copyBufferToBuffer(scratch, 0, gpuPool.buffer, 0, GPU_BUFFER_BYTES);
    device.queue.submit([commit.finish()]);
    await device.queue.onSubmittedWorkDone();
  }

  scratch.destroy();
  planBuffer.destroy();
  nodeHandles.destroy();
  status.destroy();
  statusReadback.destroy();

  if (result[0] !== 1 || result[1] === LOCAL_HANDLE_NONE) return null;
  return localRef(gpuPool.memory, result[1]);
}

export async function gpuExport(device, gpuPool, ref) {
  const handle = requireLocalRef(ref, gpuPool.memory);
  const topology = await readGpuTopology(device, gpuPool);
  return exportLocalTopology(topology, handle);
}

export function destroyGpuAnumPool(gpuPool) {
  gpuPool.buffer.destroy();
}

function must(condition, message) {
  if (!condition) throw new Error(message);
}

export async function runAnumBoundaryBrowser(wasm, device) {
  const logs = [];
  cpuResetPool(wasm);
  const gpuPool = createGpuAnumPool(device);

  try {
    const cpuRefs = new Map();
    const gpuRefs = new Map();

    for (const source of ANUM_FIXTURES) {
      const ref = cpuImportRaw(wasm, source);
      must(ref, `CPU import rejected valid fixture ${source}`);
      const exported = cpuExport(wasm, ref);
      must(exported === source, `CPU export mismatch ${source} != ${exported}`);
      cpuRefs.set(source, ref);
    }

    for (const source of [...ANUM_FIXTURES].reverse()) {
      const ref = await gpuImport(device, gpuPool, source);
      must(ref, `GPU import rejected valid fixture ${source}`);
      const exported = await gpuExport(device, gpuPool, ref);
      must(exported === source, `GPU export mismatch ${source} != ${exported}`);
      gpuRefs.set(source, ref);
    }

    for (const source of ANUM_FIXTURES) {
      must(cpuExport(wasm, cpuRefs.get(source)) === await gpuExport(device, gpuPool, gpuRefs.get(source)),
        `two-memory differential mismatch for ${source}`);
    }

    const sample = "19868";
    const cpuSample = cpuRefs.get(sample);
    const gpuSample = gpuRefs.get(sample);
    must(cpuSample.value !== gpuSample.value,
      `local handles unexpectedly equal for sample ${sample}: ${cpuSample.value}`);

    const cpuAgain = cpuImportRaw(wasm, sample);
    const gpuAgain = await gpuImport(device, gpuPool, sample);
    must(cpuAgain.value === cpuSample.value, "CPU canonical local reuse failed");
    must(gpuAgain.value === gpuSample.value, "GPU canonical local reuse failed");

    const cpuBeforeNegatives = cpuPoolCount(wasm);
    const gpuBeforeNegatives = await gpuPoolCount(device, gpuPool);
    const malformed = ["5", "1", "9", "88", "19868x"];

    for (const source of malformed) {
      const before = cpuPoolCount(wasm);
      must(cpuImportRaw(wasm, source) === null, `CPU accepted invalid Anum ${source}`);
      must(cpuPoolCount(wasm) === before, `CPU partial publication on invalid Anum ${source}`);

      const gpuBefore = await gpuPoolCount(device, gpuPool);
      let rejected = false;
      try {
        const ref = await gpuImport(device, gpuPool, source);
        rejected = ref === null;
      } catch {
        rejected = true;
      }
      must(rejected, `GPU accepted invalid Anum ${source}`);
      must(await gpuPoolCount(device, gpuPool) === gpuBefore,
        `GPU partial publication on invalid Anum ${source}`);
    }

    let foreignRejected = false;
    try {
      await gpuExport(device, gpuPool, cpuSample);
    } catch {
      foreignRejected = true;
    }
    must(foreignRejected, "foreign CPU handle was accepted by GPU memory");

    const capacitySource = "9".repeat(70) + "8";
    const cpuBeforeCapacity = cpuPoolCount(wasm);
    must(cpuImportRaw(wasm, capacitySource) === null, "CPU capacity exhaustion did not fail closed");
    must(cpuPoolCount(wasm) === cpuBeforeCapacity, "CPU capacity failure partially published");

    const gpuBeforeCapacity = await gpuPoolCount(device, gpuPool);
    let gpuCapacityRejected = false;
    try {
      const ref = await gpuImport(device, gpuPool, capacitySource);
      gpuCapacityRejected = ref === null;
    } catch {
      gpuCapacityRejected = true;
    }
    must(gpuCapacityRejected, "GPU capacity exhaustion did not fail closed");
    must(await gpuPoolCount(device, gpuPool) === gpuBeforeCapacity,
      "GPU capacity failure partially published");

    must(cpuPoolCount(wasm) === cpuBeforeNegatives, "CPU negative witnesses changed semantic pool");
    must(await gpuPoolCount(device, gpuPool) === gpuBeforeNegatives,
      "GPU negative witnesses changed semantic pool");

    logs.push(`anum.sample.source = ${sample}`);
    logs.push(`anum.sample.cpu.handle = ${cpuSample.value}`);
    logs.push(`anum.sample.gpu.handle = ${gpuSample.value}`);
    logs.push(`anum.sample.cpu.export = ${cpuExport(wasm, cpuSample)}`);
    logs.push(`anum.sample.gpu.export = ${await gpuExport(device, gpuPool, gpuSample)}`);

    return {
      cpuImport: true,
      gpuImport: true,
      cpuExport: true,
      gpuExport: true,
      differential: true,
      handlesDiffer: true,
      canonicalReuse: true,
      negativeControls: true,
      sample: {
        source: sample,
        cpuHandle: cpuSample.value,
        gpuHandle: gpuSample.value,
        cpuExport: cpuExport(wasm, cpuSample),
        gpuExport: await gpuExport(device, gpuPool, gpuSample),
      },
      logs,
    };
  } finally {
    destroyGpuAnumPool(gpuPool);
  }
}
