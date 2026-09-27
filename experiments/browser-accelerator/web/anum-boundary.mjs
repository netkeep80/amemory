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

export function wasmU32(value) {
  return value >>> 0;
}
export const GPU_ROOT_HANDLE = 63;
const GPU_BUFFER_WORDS = 64 * 3;
const GPU_BUFFER_BYTES = GPU_BUFFER_WORDS * Uint32Array.BYTES_PER_ELEMENT;
const SEMANTIC_ORIENTATION_BINDINGS = new WeakMap();

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


function readTechnicalPoles(topology, handle) {
  if (!Number.isInteger(handle) ||
      handle < 1 ||
      handle > LOCAL_HANDLE_MAX ||
      topology.used[handle] === 0) {
    throw new Error(`unknown local handle: ${handle}`);
  }
  return Object.freeze({
    start: topology.starts[handle],
    end: topology.ends[handle],
  });
}

function observeOneSidedMarker(topology, marker) {
  const poles = readTechnicalPoles(topology, marker);
  const firstSelfClosed = poles.start === marker && poles.end !== marker;
  const secondSelfClosed = poles.end === marker && poles.start !== marker;
  if (firstSelfClosed === secondSelfClosed) {
    throw new Error(`orientation marker must be proper one-sided self-incidence: ${marker}`);
  }
  return Object.freeze({
    marker,
    body: firstSelfClosed ? poles.end : poles.start,
    firstSelfClosed,
  });
}

function invertRecursiveNode(node) {
  if (node.kind === "ROOT") return { kind: "ROOT" };
  if (node.kind === "START") {
    return { kind: "END", child: invertRecursiveNode(node.child) };
  }
  if (node.kind === "END") {
    return { kind: "START", child: invertRecursiveNode(node.child) };
  }
  if (node.kind === "PAIR") {
    return {
      kind: "PAIR",
      start: invertRecursiveNode(node.end),
      end: invertRecursiveNode(node.start),
    };
  }
  throw new Error(`unknown recursive node kind: ${node.kind}`);
}

/**
 * Accepted-MTS-v0.14 context-relative semantic orientation over one concrete
 * local topology.
 *
 * ID/J is derived from two actual proper one-sided Link markers. It is not an
 * input authority. The captured topology remains the technical carrier;
 * semantic START_K/END_K are read through this view.
 */
export function createContextRelativeOrientation(
  topology,
  referenceMarker,
  contextMarker,
) {
  validateTopology(topology);
  const reference = observeOneSidedMarker(topology, referenceMarker);
  const context = observeOneSidedMarker(topology, contextMarker);
  const transport =
    reference.firstSelfClosed === context.firstSelfClosed ? "ID" : "J";

  function poles(handle) {
    const technical = readTechnicalPoles(topology, handle);
    return transport === "ID"
      ? technical
      : Object.freeze({ start: technical.end, end: technical.start });
  }

  function semanticRecursiveWire(handle) {
    const visiting = new Uint8Array(64);

    function walk(h) {
      const semantic = poles(h);
      if (visiting[h]) {
        throw new Error(`non-well-founded semantic topology at handle ${h}`);
      }

      if (semantic.start === h && semantic.end === h) return "8";

      visiting[h] = 1;
      let out;
      if (semantic.start === h) {
        out = "9" + walk(semantic.end);
      } else if (semantic.end === h) {
        out = "6" + walk(semantic.start);
      } else {
        out = "1" + walk(semantic.start) + walk(semantic.end);
      }
      visiting[h] = 0;
      return out;
    }

    return walk(handle);
  }

  function technicalRecursiveWire(semanticSource) {
    const parsed = parseAnum(semanticSource);
    return formatAnum(
      transport === "ID" ? parsed : invertRecursiveNode(parsed),
    );
  }

  return Object.freeze({
    transport,
    referenceMarker: reference.marker,
    referenceBody: reference.body,
    contextMarker: context.marker,
    contextBody: context.body,
    poles,
    semanticRecursiveWire,
    technicalRecursiveWire,
  });
}

function createBackendSemanticOrientation(
  owner,
  memory,
  topology,
  referenceRef,
  contextRef,
) {
  if ((typeof owner !== "object" && typeof owner !== "function") || owner === null) {
    throw new Error("semantic orientation requires a concrete backend memory owner");
  }
  const referenceMarker = requireLocalRef(referenceRef, memory);
  const contextMarker = requireLocalRef(contextRef, memory);
  const view = createContextRelativeOrientation(topology, referenceMarker, contextMarker);
  const binding = Object.freeze({
    memory,
    referenceMarker,
    contextMarker,
    transport: view.transport,
  });
  SEMANTIC_ORIENTATION_BINDINGS.set(binding, Object.freeze({
    owner,
    memory,
    referenceMarker,
    contextMarker,
    referenceBody: view.referenceBody,
    contextBody: view.contextBody,
    transport: view.transport,
  }));
  return binding;
}

function requireBackendSemanticOrientation(owner, memory, topology, binding) {
  const authority = SEMANTIC_ORIENTATION_BINDINGS.get(binding);
  if (!authority || authority.owner !== owner || authority.memory !== memory) {
    throw new Error(`foreign semantic orientation rejected by ${memory}`);
  }
  const view = createContextRelativeOrientation(
    topology,
    authority.referenceMarker,
    authority.contextMarker,
  );
  if (view.transport !== authority.transport ||
      view.referenceBody !== authority.referenceBody ||
      view.contextBody !== authority.contextBody) {
    throw new Error("semantic orientation marker authority changed");
  }
  return view;
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

export function readCpuTopology(wasm) {
  if (typeof wasm.anumCpuUsed !== "function" ||
      typeof wasm.anumCpuStart !== "function" ||
      typeof wasm.anumCpuEnd !== "function") {
    throw new Error("CPU technical topology exports are missing");
  }

  const starts = new Uint32Array(64);
  const ends = new Uint32Array(64);
  const used = new Uint32Array(64);
  for (let handle = 1; handle <= LOCAL_HANDLE_MAX; handle += 1) {
    const occupied = wasmU32(wasm.anumCpuUsed(handle));
    if (occupied === LOCAL_HANDLE_NONE) {
      throw new Error(`CPU topology rejected handle ${handle}`);
    }
    if (occupied === 0) continue;
    if (occupied !== 1) {
      throw new Error(`CPU topology emitted invalid used flag ${occupied}`);
    }
    const start = wasmU32(wasm.anumCpuStart(handle));
    const end = wasmU32(wasm.anumCpuEnd(handle));
    if (start === LOCAL_HANDLE_NONE || end === LOCAL_HANDLE_NONE) {
      throw new Error(`CPU topology omitted technical poles for ${handle}`);
    }
    used[handle] = 1;
    starts[handle] = start;
    ends[handle] = end;
  }
  return { starts, ends, used };
}

export function bindCpuSemanticOrientation(
  wasm,
  referenceRef,
  contextRef,
  memory = "cpu-A",
) {
  return createBackendSemanticOrientation(
    wasm,
    memory,
    readCpuTopology(wasm),
    referenceRef,
    contextRef,
  );
}

export function cpuResetPool(wasm) {
  wasm.anumCpuResetPool();
}

export function cpuPoolCount(wasm) {
  return wasm.anumCpuPoolCount();
}

function writeCpuTokens(wasm, source) {
  const tokens = sourceTokens(source);
  if (tokens.length > 256) return null;
  for (let i = 0; i < tokens.length; i += 1) {
    if (wasm.anumCpuSetToken(i, tokens[i]) !== 1) return null;
  }
  return tokens.length;
}

export function cpuImportRaw(wasm, source, memory = "cpu-A") {
  const tokenCount = writeCpuTokens(wasm, source);
  if (tokenCount === null) return null;
  const handle = wasmU32(wasm.anumCpuImport(tokenCount));
  if (handle === LOCAL_HANDLE_NONE) return null;
  return localRef(memory, handle);
}

export function cpuImportBatchAtomic(wasm, sources, memory = "cpu-A") {
  if (!Array.isArray(sources) || sources.length === 0) return null;
  if (typeof wasm.anumCpuLoadBegin !== "function" ||
      typeof wasm.anumCpuLoadMember !== "function" ||
      typeof wasm.anumCpuLoadCommit !== "function" ||
      typeof wasm.anumCpuLoadAbort !== "function") {
    throw new Error("CPU atomic Aset load exports are missing");
  }

  if (wasm.anumCpuLoadBegin() !== 1) return null;
  const handles = [];
  try {
    for (const source of sources) {
      const tokenCount = writeCpuTokens(wasm, source);
      if (tokenCount === null) {
        wasm.anumCpuLoadAbort();
        return null;
      }
      const handle = wasmU32(wasm.anumCpuLoadMember(tokenCount));
      if (handle === LOCAL_HANDLE_NONE) {
        wasm.anumCpuLoadAbort();
        return null;
      }
      handles.push(handle);
    }

    if (wasm.anumCpuLoadCommit() !== 1) {
      wasm.anumCpuLoadAbort();
      return null;
    }
    return handles.map((handle) => localRef(memory, handle));
  } catch (error) {
    wasm.anumCpuLoadAbort();
    throw error;
  }
}

export function cpuExport(wasm, ref, memory = "cpu-A") {
  const handle = requireLocalRef(ref, memory);
  const length = wasmU32(wasm.anumCpuExport(handle));
  if (length === LOCAL_HANDLE_NONE) throw new Error("CPU export rejected local topology");
  let result = "";
  for (let i = 0; i < length; i += 1) {
    const token = wasmU32(wasm.anumCpuOutputGet(i));
    if (![1, 6, 8, 9].includes(token)) {
      throw new Error(`CPU export emitted invalid token ${token}`);
    }
    result += String(token);
  }
  return result;
}

export function cpuImportSemanticRecursiveWire(
  wasm,
  orientation,
  semanticSource,
  memory = "cpu-A",
) {
  const topology = readCpuTopology(wasm);
  const view = requireBackendSemanticOrientation(wasm, memory, topology, orientation);
  const technicalSource = view.technicalRecursiveWire(semanticSource);
  return cpuImportRaw(wasm, technicalSource, memory);
}

export function cpuExportSemanticRecursiveWire(
  wasm,
  orientation,
  ref,
  memory = "cpu-A",
) {
  const handle = requireLocalRef(ref, memory);
  const topology = readCpuTopology(wasm);
  const view = requireBackendSemanticOrientation(wasm, memory, topology, orientation);
  return view.semanticRecursiveWire(handle);
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

export async function gpuImportBatchAtomic(device, gpuPool, sources) {
  if (!Array.isArray(sources) || sources.length === 0) return null;

  // The live GPU pool remains untouched until the staging pool contains the
  // entire valid Aset. Per-member gpuImport commits only into this scratch pool.
  const staging = createGpuAnumPool(device, gpuPool.memory + ":staging");
  const handles = [];
  try {
    for (const source of sources) {
      let ref;
      try {
        ref = await gpuImport(device, staging, source);
      } catch {
        return null;
      }
      if (!ref) return null;
      handles.push(ref.value);
    }

    const encoder = device.createCommandEncoder();
    encoder.copyBufferToBuffer(staging.buffer, 0, gpuPool.buffer, 0, GPU_BUFFER_BYTES);
    device.queue.submit([encoder.finish()]);
    await device.queue.onSubmittedWorkDone();

    return handles.map((handle) => localRef(gpuPool.memory, handle));
  } finally {
    destroyGpuAnumPool(staging);
  }
}

export async function gpuExport(device, gpuPool, ref) {
  const handle = requireLocalRef(ref, gpuPool.memory);
  const topology = await readGpuTopology(device, gpuPool);
  return exportLocalTopology(topology, handle);
}

export async function bindGpuSemanticOrientation(
  device,
  gpuPool,
  referenceRef,
  contextRef,
) {
  return createBackendSemanticOrientation(
    gpuPool,
    gpuPool.memory,
    await readGpuTopology(device, gpuPool),
    referenceRef,
    contextRef,
  );
}

export async function gpuImportSemanticRecursiveWire(
  device,
  gpuPool,
  orientation,
  semanticSource,
) {
  const topology = await readGpuTopology(device, gpuPool);
  const view = requireBackendSemanticOrientation(
    gpuPool,
    gpuPool.memory,
    topology,
    orientation,
  );
  const technicalSource = view.technicalRecursiveWire(semanticSource);
  return gpuImport(device, gpuPool, technicalSource);
}

export async function gpuExportSemanticRecursiveWire(
  device,
  gpuPool,
  orientation,
  ref,
) {
  const handle = requireLocalRef(ref, gpuPool.memory);
  const topology = await readGpuTopology(device, gpuPool);
  const view = requireBackendSemanticOrientation(
    gpuPool,
    gpuPool.memory,
    topology,
    orientation,
  );
  return view.semanticRecursiveWire(handle);
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

    // Multi-Anum Aset load is atomic across the whole batch, not only per Anum.
    // First prove a malformed middle member cannot replace the published pool.
    const cpuBeforeBatchFailure = cpuPoolCount(wasm);
    const gpuBeforeBatchFailure = await gpuPoolCount(device, gpuPool);
    must(cpuImportBatchAtomic(wasm, ["98", "5", "68"]) === null,
      "CPU accepted malformed multi-Anum Aset");
    must(cpuPoolCount(wasm) === cpuBeforeBatchFailure,
      "CPU failed batch partially published Aset");

    const gpuFailedBatch = await gpuImportBatchAtomic(device, gpuPool, ["98", "5", "68"]);
    must(gpuFailedBatch === null, "GPU accepted malformed multi-Anum Aset");
    must(await gpuPoolCount(device, gpuPool) === gpuBeforeBatchFailure,
      "GPU failed batch partially published Aset");

    // Successful replacement batch shares ROOT/O/C across the PAIR and must
    // canonicalize to exactly four Links in both independently allocated memories.
    const asetSources = ["98", "68", "19868"];
    const cpuBatchRefs = cpuImportBatchAtomic(wasm, asetSources);
    const gpuBatchRefs = await gpuImportBatchAtomic(device, gpuPool, asetSources);
    must(cpuBatchRefs && gpuBatchRefs, "atomic multi-Anum Aset load failed");
    must(cpuPoolCount(wasm) === 4, "CPU Aset canonical Link count mismatch");
    must(await gpuPoolCount(device, gpuPool) === 4, "GPU Aset canonical Link count mismatch");
    for (let i = 0; i < asetSources.length; i += 1) {
      must(cpuExport(wasm, cpuBatchRefs[i]) === asetSources[i],
        "CPU batch export mismatch for " + asetSources[i]);
      must(await gpuExport(device, gpuPool, gpuBatchRefs[i]) === asetSources[i],
        "GPU batch export mismatch for " + asetSources[i]);
      must(cpuExport(wasm, cpuBatchRefs[i]) === await gpuExport(device, gpuPool, gpuBatchRefs[i]),
        "atomic Aset CPU/GPU differential mismatch for " + asetSources[i]);
    }

    // Accepted-v0.14 semantic transport over the real CPU/WASM and WebGPU
    // materializers. The 98/68 fixtures below are technical marker Links only;
    // relative ID/J is derived from their actual one-sided topology.
    const cpuDirectOrientation = bindCpuSemanticOrientation(
      wasm, cpuBatchRefs[0], cpuBatchRefs[0],
    );
    const cpuMirrorOrientation = bindCpuSemanticOrientation(
      wasm, cpuBatchRefs[0], cpuBatchRefs[1],
    );
    const gpuDirectOrientation = await bindGpuSemanticOrientation(
      device, gpuPool, gpuBatchRefs[0], gpuBatchRefs[0],
    );
    const gpuMirrorOrientation = await bindGpuSemanticOrientation(
      device, gpuPool, gpuBatchRefs[0], gpuBatchRefs[1],
    );

    must(cpuDirectOrientation.transport === "ID", "CPU Direct orientation mismatch");
    must(cpuMirrorOrientation.transport === "J", "CPU Mirror orientation mismatch");
    must(gpuDirectOrientation.transport === "ID", "GPU Direct orientation mismatch");
    must(gpuMirrorOrientation.transport === "J", "GPU Mirror orientation mismatch");

    const cpuDirectSemanticRefs = new Map();
    const cpuMirrorSemanticRefs = new Map();
    const gpuDirectSemanticRefs = new Map();
    const gpuMirrorSemanticRefs = new Map();
    for (const source of ANUM_FIXTURES) {
      const cpuDirectRef = cpuImportSemanticRecursiveWire(
        wasm, cpuDirectOrientation, source,
      );
      const cpuMirrorRef = cpuImportSemanticRecursiveWire(
        wasm, cpuMirrorOrientation, source,
      );
      const gpuDirectRef = await gpuImportSemanticRecursiveWire(
        device, gpuPool, gpuDirectOrientation, source,
      );
      const gpuMirrorRef = await gpuImportSemanticRecursiveWire(
        device, gpuPool, gpuMirrorOrientation, source,
      );
      must(cpuDirectRef && cpuMirrorRef && gpuDirectRef && gpuMirrorRef,
        `semantic import rejected fixture ${source}`);

      cpuDirectSemanticRefs.set(source, cpuDirectRef);
      cpuMirrorSemanticRefs.set(source, cpuMirrorRef);
      gpuDirectSemanticRefs.set(source, gpuDirectRef);
      gpuMirrorSemanticRefs.set(source, gpuMirrorRef);

      const cpuDirectWire = cpuExportSemanticRecursiveWire(
        wasm, cpuDirectOrientation, cpuDirectRef,
      );
      const cpuMirrorWire = cpuExportSemanticRecursiveWire(
        wasm, cpuMirrorOrientation, cpuMirrorRef,
      );
      const gpuDirectWire = await gpuExportSemanticRecursiveWire(
        device, gpuPool, gpuDirectOrientation, gpuDirectRef,
      );
      const gpuMirrorWire = await gpuExportSemanticRecursiveWire(
        device, gpuPool, gpuMirrorOrientation, gpuMirrorRef,
      );
      must(cpuDirectWire === source, `CPU Direct semantic mismatch ${source}`);
      must(cpuMirrorWire === source, `CPU Mirror semantic mismatch ${source}`);
      must(gpuDirectWire === source, `GPU Direct semantic mismatch ${source}`);
      must(gpuMirrorWire === source, `GPU Mirror semantic mismatch ${source}`);
      must(cpuDirectWire === gpuMirrorWire,
        `cross-gauge CPU/GPU semantic differential mismatch ${source}`);
      must(cpuMirrorWire === gpuDirectWire,
        `opposite cross-gauge CPU/GPU semantic differential mismatch ${source}`);
    }

    const semanticO = "98";
    must(cpuExport(wasm, cpuMirrorSemanticRefs.get(semanticO)) === "68",
      "CPU Mirror technical O did not swap gauge");
    must(await gpuExport(device, gpuPool, gpuMirrorSemanticRefs.get(semanticO)) === "68",
      "GPU Mirror technical O did not swap gauge");
    must(cpuExportSemanticRecursiveWire(
      wasm, cpuMirrorOrientation, cpuMirrorSemanticRefs.get(semanticO),
    ) === semanticO, "CPU Mirror semantic O changed");
    must(await gpuExportSemanticRecursiveWire(
      device, gpuPool, gpuMirrorOrientation, gpuMirrorSemanticRefs.get(semanticO),
    ) === semanticO, "GPU Mirror semantic O changed");

    const cpuBeforeSemanticMalformed = cpuPoolCount(wasm);
    let cpuSemanticMalformedRejected = false;
    try {
      cpuImportSemanticRecursiveWire(wasm, cpuMirrorOrientation, "1986x");
    } catch {
      cpuSemanticMalformedRejected = true;
    }
    must(cpuSemanticMalformedRejected, "CPU semantic malformed source accepted");
    must(cpuPoolCount(wasm) === cpuBeforeSemanticMalformed,
      "CPU semantic malformed source partially published");

    const gpuBeforeSemanticMalformed = await gpuPoolCount(device, gpuPool);
    let gpuSemanticMalformedRejected = false;
    try {
      await gpuImportSemanticRecursiveWire(
        device, gpuPool, gpuMirrorOrientation, "1986x",
      );
    } catch {
      gpuSemanticMalformedRejected = true;
    }
    must(gpuSemanticMalformedRejected, "GPU semantic malformed source accepted");
    must(await gpuPoolCount(device, gpuPool) === gpuBeforeSemanticMalformed,
      "GPU semantic malformed source partially published");

    const gpuBeforeForeignOrientation = await gpuPoolCount(device, gpuPool);
    let foreignOrientationRejected = false;
    try {
      await gpuImportSemanticRecursiveWire(
        device, gpuPool, cpuMirrorOrientation, semanticO,
      );
    } catch {
      foreignOrientationRejected = true;
    }
    must(foreignOrientationRejected, "foreign CPU orientation accepted by GPU memory");
    must(await gpuPoolCount(device, gpuPool) === gpuBeforeForeignOrientation,
      "foreign semantic orientation changed GPU memory");

    logs.push(`aset.atomic.sources = [${asetSources.join(", ")}]`);
    logs.push(`aset.atomic.cpu.links = ${cpuPoolCount(wasm)}`);
    logs.push(`aset.atomic.gpu.links = ${await gpuPoolCount(device, gpuPool)}`);
    logs.push("aset.atomic.failed-middle = NO_PARTIAL_PUBLICATION");
    logs.push("semantic.orientation.cpu = ID/J_FROM_REAL_MARKERS");
    logs.push("semantic.orientation.gpu = ID/J_FROM_REAL_MARKERS");
    logs.push("semantic.transport.cpu-gpu = GAUGE_INVARIANT");
    logs.push("semantic.transport.fail-closed = TRUE");

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
      atomicAsetLoad: true,
      atomicAsetDifferential: true,
      atomicAsetLinkCount: 4,
      semanticTransport: true,
      semanticDirectMirror: true,
      semanticCrossBackendDifferential: true,
      semanticFailClosed: true,
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
