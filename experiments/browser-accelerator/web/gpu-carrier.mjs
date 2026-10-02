export const PACKED_GPU_CARRIER_MAGIC = 0x414d4750;
export const PACKED_GPU_CARRIER_SCHEMA_VERSION = 1;
export const PACKED_CARRIER_SCHEMA_VERSION = 1;
export const PACKED_INCIDENCE_INDEX_SCHEMA_VERSION = 1;
export const PACKED_GPU_CARRIER_HEADER_WORDS = 16;

const U32_BYTES = 4;

function checkedInteger(value, label, { min = 0 } = {}) {
  if (!Number.isSafeInteger(value) || value < min) {
    throw new TypeError(`${label} must be a safe integer >= ${min}`);
  }
  return value;
}

function checkedAdd(left, right, label) {
  const value = left + right;
  if (!Number.isSafeInteger(value)) {
    throw new RangeError(`${label} exceeds JavaScript safe integer range`);
  }
  return value;
}

function wordsToBytes(words, label) {
  const bytes = words * U32_BYTES;
  if (!Number.isSafeInteger(bytes)) {
    throw new RangeError(`${label} exceeds JavaScript safe integer range`);
  }
  return bytes;
}

export function gpuCarrierLayoutForLinkCount(linkCount) {
  checkedInteger(linkCount, "linkCount", { min: 1 });

  const startsOffset = PACKED_GPU_CARRIER_HEADER_WORDS;
  const endsOffset = checkedAdd(startsOffset, linkCount, "endsOffset");
  const indexLength = checkedAdd(linkCount, 1, "indexLength");
  const startHeadOffset = checkedAdd(endsOffset, linkCount, "startHeadOffset");
  const endHeadOffset = checkedAdd(
    startHeadOffset,
    indexLength,
    "endHeadOffset",
  );
  const nextByStartOffset = checkedAdd(
    endHeadOffset,
    indexLength,
    "nextByStartOffset",
  );
  const nextByEndOffset = checkedAdd(
    nextByStartOffset,
    indexLength,
    "nextByEndOffset",
  );
  const totalWords = checkedAdd(
    nextByEndOffset,
    indexLength,
    "totalWords",
  );

  return Object.freeze({
    linkCount,
    rootHandle: 1,
    startsOffset,
    endsOffset,
    startHeadOffset,
    endHeadOffset,
    nextByStartOffset,
    nextByEndOffset,
    totalWords,
    totalBytes: wordsToBytes(totalWords, "totalBytes"),
  });
}

function requireUint32Array(words) {
  if (!(words instanceof Uint32Array)) {
    throw new TypeError("GPU carrier must be a Uint32Array");
  }
  if (words.length < PACKED_GPU_CARRIER_HEADER_WORDS) {
    throw new RangeError(
      `GPU carrier too short: ${words.length} words`,
    );
  }
}

function assertHeaderWord(actual, expected, label) {
  if (actual !== expected) {
    throw new Error(
      `GPU carrier ${label} mismatch: expected ${expected}, got ${actual}`,
    );
  }
}

export function parseGpuCarrierWords(words) {
  requireUint32Array(words);

  assertHeaderWord(words[0], PACKED_GPU_CARRIER_MAGIC, "magic");
  assertHeaderWord(
    words[1],
    PACKED_GPU_CARRIER_SCHEMA_VERSION,
    "ABI schema",
  );
  assertHeaderWord(
    words[2],
    PACKED_CARRIER_SCHEMA_VERSION,
    "carrier schema",
  );
  assertHeaderWord(
    words[3],
    PACKED_INCIDENCE_INDEX_SCHEMA_VERSION,
    "incidence schema",
  );

  const layout = gpuCarrierLayoutForLinkCount(words[4]);
  assertHeaderWord(words[5], layout.rootHandle, "ROOT handle");
  assertHeaderWord(words[6], layout.startsOffset, "starts offset");
  assertHeaderWord(words[7], layout.endsOffset, "ends offset");
  assertHeaderWord(words[8], layout.startHeadOffset, "start_head offset");
  assertHeaderWord(words[9], layout.endHeadOffset, "end_head offset");
  assertHeaderWord(
    words[10],
    layout.nextByStartOffset,
    "next_by_start offset",
  );
  assertHeaderWord(
    words[11],
    layout.nextByEndOffset,
    "next_by_end offset",
  );
  assertHeaderWord(words[12], layout.totalWords, "total words");
  assertHeaderWord(words[13], 0, "flags");
  assertHeaderWord(words[14], 0, "reserved[0]");
  assertHeaderWord(words[15], 0, "reserved[1]");

  if (words.length !== layout.totalWords) {
    throw new Error(
      `GPU carrier length mismatch: expected ${layout.totalWords}, got ${words.length}`,
    );
  }

  const sections = Object.freeze({
    starts: words.subarray(layout.startsOffset, layout.endsOffset),
    ends: words.subarray(layout.endsOffset, layout.startHeadOffset),
    startHead: words.subarray(
      layout.startHeadOffset,
      layout.endHeadOffset,
    ),
    endHead: words.subarray(
      layout.endHeadOffset,
      layout.nextByStartOffset,
    ),
    nextByStart: words.subarray(
      layout.nextByStartOffset,
      layout.nextByEndOffset,
    ),
    nextByEnd: words.subarray(
      layout.nextByEndOffset,
      layout.totalWords,
    ),
  });

  return Object.freeze({ words, layout, sections });
}

function normalizeGpuLimits(limits) {
  if (!limits || typeof limits !== "object") {
    throw new TypeError("WebGPU limits are required");
  }
  return Object.freeze({
    maxBufferSize: checkedInteger(
      Number(limits.maxBufferSize),
      "maxBufferSize",
      { min: 4 },
    ),
    maxStorageBufferBindingSize: checkedInteger(
      Number(limits.maxStorageBufferBindingSize),
      "maxStorageBufferBindingSize",
      { min: 4 },
    ),
    maxStorageBuffersPerShaderStage: checkedInteger(
      Number(limits.maxStorageBuffersPerShaderStage),
      "maxStorageBuffersPerShaderStage",
      { min: 1 },
    ),
  });
}

function sectionPlan(layout) {
  const entries = [
    ["starts", layout.startsOffset, layout.endsOffset],
    ["ends", layout.endsOffset, layout.startHeadOffset],
    ["startHead", layout.startHeadOffset, layout.endHeadOffset],
    ["endHead", layout.endHeadOffset, layout.nextByStartOffset],
    ["nextByStart", layout.nextByStartOffset, layout.nextByEndOffset],
    ["nextByEnd", layout.nextByEndOffset, layout.totalWords],
  ];
  return entries.map(([name, wordOffset, wordEnd], binding) =>
    Object.freeze({
      name,
      binding,
      wordOffset,
      wordLength: wordEnd - wordOffset,
      byteOffset: wordsToBytes(wordOffset, `${name}.byteOffset`),
      byteLength: wordsToBytes(
        wordEnd - wordOffset,
        `${name}.byteLength`,
      ),
    }),
  );
}

export function planGpuCarrierUpload(layout, rawLimits) {
  if (!layout || typeof layout !== "object") {
    throw new TypeError("GPU carrier layout is required");
  }
  const limits = normalizeGpuLimits(rawLimits);
  const bindingByteLimit = Math.min(
    limits.maxBufferSize,
    limits.maxStorageBufferBindingSize,
  );

  if (layout.totalBytes <= bindingByteLimit) {
    return Object.freeze({
      mode: "single",
      logicalWords: layout.totalWords,
      logicalBytes: layout.totalBytes,
      storageBufferCount: 1,
      buffers: Object.freeze([
        Object.freeze({
          name: "carrier",
          binding: 0,
          wordOffset: 0,
          wordLength: layout.totalWords,
          byteOffset: 0,
          byteLength: layout.totalBytes,
        }),
      ]),
      limits,
    });
  }

  const buffers = sectionPlan(layout);
  if (buffers.length > limits.maxStorageBuffersPerShaderStage) {
    return Object.freeze({
      mode: "unsupported",
      reason: "storage-buffer-binding-count",
      logicalWords: layout.totalWords,
      logicalBytes: layout.totalBytes,
      requiredStorageBuffers: buffers.length,
      limits,
    });
  }

  const oversized = buffers.find(
    (buffer) => buffer.byteLength > bindingByteLimit,
  );
  if (oversized) {
    return Object.freeze({
      mode: "unsupported",
      reason: "section-exceeds-storage-buffer-limit",
      section: oversized.name,
      sectionBytes: oversized.byteLength,
      logicalWords: layout.totalWords,
      logicalBytes: layout.totalBytes,
      limits,
    });
  }

  return Object.freeze({
    mode: "sections",
    logicalWords: layout.totalWords,
    logicalBytes: layout.totalBytes,
    storageBufferCount: buffers.length,
    hostHeaderWords: PACKED_GPU_CARRIER_HEADER_WORDS,
    buffers: Object.freeze(buffers),
    limits,
  });
}

export function planGpuCarrierWordsUpload(words, limits) {
  const parsed = parseGpuCarrierWords(words);
  return planGpuCarrierUpload(parsed.layout, limits);
}


export const DEFAULT_GPU_CARRIER_WASM_ABI = Object.freeze({
  available: "amemory_i386_lab_gpu_carrier_available",
  length: "amemory_i386_lab_gpu_carrier_word_len",
  pointer: "amemory_i386_lab_gpu_carrier_words_ptr",
  word: "amemory_i386_lab_gpu_carrier_word",
});

function wasmAbiFail(label, message) {
  throw new Error(`${label} ABI: ${message}`);
}

export function readGpuCarrierWordsAbi(
  wasm,
  names = DEFAULT_GPU_CARRIER_WASM_ABI,
  label = "GPU carrier",
) {
  const available = wasm?.[names.available];
  if (typeof available !== "function" || available() !== 1) {
    return null;
  }

  const length = wasm[names.length];
  if (typeof length !== "function") {
    wasmAbiFail(label, "word-length function missing");
  }
  const wordLength = length() >>> 0;
  if (wordLength < PACKED_GPU_CARRIER_HEADER_WORDS) {
    wasmAbiFail(label, "reported carrier shorter than header");
  }

  let words;
  const pointer = wasm[names.pointer];
  if (wasm.memory && typeof pointer === "function") {
    const ptr = pointer() >>> 0;
    if ((ptr & 3) !== 0) {
      wasmAbiFail(label, "u32 pointer is not 4-byte aligned");
    }
    const byteLength = wordLength * 4;
    if (ptr > wasm.memory.buffer.byteLength ||
        byteLength > wasm.memory.buffer.byteLength - ptr) {
      wasmAbiFail(label, "pointer outside WASM memory");
    }
    // Own the snapshot: a later WASM memory.grow must not invalidate the
    // carrier evidence that is about to be planned/uploaded.
    words = new Uint32Array(
      new Uint32Array(wasm.memory.buffer, ptr, wordLength),
    );
  } else {
    const word = wasm[names.word];
    if (typeof word !== "function") {
      wasmAbiFail(label, "word fallback function missing");
    }
    words = new Uint32Array(wordLength);
    for (let index = 0; index < wordLength; index += 1) {
      words[index] = word(index) >>> 0;
    }
  }

  return parseGpuCarrierWords(words);
}


function requireLookupHandle(parsed, handle, label) {
  checkedInteger(handle, label, { min: 1 });
  if (handle > parsed.layout.linkCount) {
    throw new RangeError(
      `${label} ${handle} exceeds Link count ${parsed.layout.linkCount}`,
    );
  }
  return handle;
}

export function gpuCarrierLogicalFingerprint(parsed) {
  let hash = 0x811c9dc5;
  for (const word of parsed.words) {
    hash ^= word >>> 0;
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return hash >>> 0;
}

export function expectedGpuCarrierLookup(parsed, {
  handle = 1,
  pole = 1,
} = {}) {
  requireLookupHandle(parsed, handle, "handle");
  requireLookupHandle(parsed, pole, "pole");

  const start = parsed.sections.starts[handle - 1] >>> 0;
  const end = parsed.sections.ends[handle - 1] >>> 0;
  const head = parsed.sections.startHead[pole] >>> 0;
  const incidence = [];
  const seen = new Set();
  let current = head;

  while (current !== 0) {
    if (current > parsed.layout.linkCount || seen.has(current)) {
      throw new Error("invalid/cyclic start-incidence chain");
    }
    seen.add(current);
    incidence.push(current);
    current = parsed.sections.nextByStart[current] >>> 0;
  }

  return Object.freeze({
    handle,
    pole,
    start,
    end,
    head,
    incidence: Object.freeze(incidence),
    logicalFingerprint: gpuCarrierLogicalFingerprint(parsed),
  });
}

function gpuLookupUsage() {
  if (typeof GPUBufferUsage === "undefined") {
    throw new Error("WebGPU buffer usage constants unavailable");
  }
  return GPUBufferUsage;
}

function createLookupBuffer(device, data, usage) {
  const bytes = Math.max(4, data.byteLength);
  const buffer = device.createBuffer({ size: bytes, usage });
  if (data.byteLength) {
    device.queue.writeBuffer(buffer, 0, data);
  }
  return buffer;
}

function singleLookupShader(handle, pole, linkCount) {
  return `
    const HANDLE: u32 = ${handle}u;
    const POLE: u32 = ${pole}u;
    const LINK_COUNT: u32 = ${linkCount}u;
    @group(0) @binding(0) var<storage, read> carrier: array<u32>;
    @group(0) @binding(1) var<storage, read_write> out: array<u32>;

    @compute @workgroup_size(1)
    fn main(@builtin(global_invocation_id) id: vec3<u32>) {
      if (id.x != 0u) { return; }
      out[0] = 0u;
      if (HANDLE == 0u || HANDLE > LINK_COUNT ||
          POLE == 0u || POLE > LINK_COUNT) {
        out[0] = 2u;
        return;
      }

      let starts_offset = carrier[6];
      let ends_offset = carrier[7];
      let start_head_offset = carrier[8];
      let next_by_start_offset = carrier[10];

      out[1] = carrier[starts_offset + HANDLE - 1u];
      out[2] = carrier[ends_offset + HANDLE - 1u];
      out[3] = carrier[start_head_offset + POLE];

      var current = out[3];
      var count = 0u;
      loop {
        if (current == 0u) { break; }
        if (current > LINK_COUNT || count >= LINK_COUNT) {
          out[0] = 3u;
          return;
        }
        out[5u + count] = current;
        count = count + 1u;
        current = carrier[next_by_start_offset + current];
      }
      out[4] = count;
      out[0] = 1u;
    }
  `;
}

function sectionLookupShader(handle, pole, linkCount) {
  return `
    const HANDLE: u32 = ${handle}u;
    const POLE: u32 = ${pole}u;
    const LINK_COUNT: u32 = ${linkCount}u;
    @group(0) @binding(0) var<storage, read> starts: array<u32>;
    @group(0) @binding(1) var<storage, read> ends: array<u32>;
    @group(0) @binding(2) var<storage, read> start_head: array<u32>;
    @group(0) @binding(3) var<storage, read> end_head: array<u32>;
    @group(0) @binding(4) var<storage, read> next_by_start: array<u32>;
    @group(0) @binding(5) var<storage, read> next_by_end: array<u32>;
    @group(0) @binding(6) var<storage, read_write> out: array<u32>;

    @compute @workgroup_size(1)
    fn main(@builtin(global_invocation_id) id: vec3<u32>) {
      if (id.x != 0u) { return; }
      out[0] = 0u;
      if (HANDLE == 0u || HANDLE > LINK_COUNT ||
          POLE == 0u || POLE > LINK_COUNT) {
        out[0] = 2u;
        return;
      }

      out[1] = starts[HANDLE - 1u];
      out[2] = ends[HANDLE - 1u];
      out[3] = start_head[POLE];

      var current = out[3];
      var count = 0u;
      loop {
        if (current == 0u) { break; }
        if (current > LINK_COUNT || count >= LINK_COUNT) {
          out[0] = 3u;
          return;
        }
        out[5u + count] = current;
        count = count + 1u;
        current = next_by_start[current];
      }
      out[4] = count;
      out[0] = 1u;
    }
  `;
}

export function gpuCarrierLookupShaderSource(
  mode,
  { handle = 1, pole = 1, linkCount } = {},
) {
  checkedInteger(linkCount, "linkCount", { min: 1 });
  checkedInteger(handle, "handle", { min: 1 });
  checkedInteger(pole, "pole", { min: 1 });
  if (mode === "single") {
    return singleLookupShader(handle, pole, linkCount);
  }
  if (mode === "sections") {
    return sectionLookupShader(handle, pole, linkCount);
  }
  throw new Error(`unsupported GPU carrier lookup mode: ${mode}`);
}

async function readLookupWords(device, source, wordLength) {
  const usage = gpuLookupUsage();
  const bytes = wordLength * 4;
  const readback = device.createBuffer({
    size: bytes,
    usage: usage.COPY_DST | usage.MAP_READ,
  });
  const encoder = device.createCommandEncoder();
  encoder.copyBufferToBuffer(source, 0, readback, 0, bytes);
  device.queue.submit([encoder.finish()]);
  await readback.mapAsync(GPUMapMode.READ, 0, bytes);
  const words = new Uint32Array(readback.getMappedRange(0, bytes).slice(0));
  readback.unmap();
  readback.destroy();
  return words;
}

export async function runGpuCarrierLookup(
  device,
  parsed,
  {
    handle = 1,
    pole = 1,
    maxDiagnosticLinks = 16_384,
  } = {},
) {
  if (!device?.createBuffer || !device?.queue) {
    throw new TypeError("WebGPU device is required");
  }
  requireLookupHandle(parsed, handle, "handle");
  requireLookupHandle(parsed, pole, "pole");
  checkedInteger(maxDiagnosticLinks, "maxDiagnosticLinks", { min: 1 });
  if (parsed.layout.linkCount > maxDiagnosticLinks) {
    throw new RangeError(
      `bounded GPU lookup refuses ${parsed.layout.linkCount} Links; ` +
      `limit is ${maxDiagnosticLinks}`,
    );
  }

  const expected = expectedGpuCarrierLookup(parsed, { handle, pole });
  const plan = planGpuCarrierUpload(parsed.layout, device.limits);
  if (plan.mode === "unsupported") {
    throw new Error(
      `WebGPU carrier upload unsupported: ${plan.reason}`,
    );
  }
  if (plan.mode === "sections" &&
      plan.storageBufferCount + 1 >
        Number(device.limits.maxStorageBuffersPerShaderStage)) {
    throw new Error(
      "WebGPU carrier lookup needs one additional storage output binding",
    );
  }

  const usage = gpuLookupUsage();
  const inputUsage = usage.STORAGE | usage.COPY_DST;
  const outputUsage = usage.STORAGE | usage.COPY_SRC;
  const inputBuffers = [];
  const outputWordLength = 5 + parsed.layout.linkCount;
  const output = createLookupBuffer(
    device,
    new Uint32Array(outputWordLength),
    outputUsage,
  );

  device.pushErrorScope?.("validation");
  try {
    let entries;
    if (plan.mode === "single") {
      const carrier = createLookupBuffer(
        device,
        parsed.words,
        inputUsage,
      );
      inputBuffers.push(carrier);
      entries = [
        { binding: 0, resource: { buffer: carrier } },
        { binding: 1, resource: { buffer: output } },
      ];
    } else {
      entries = plan.buffers.map((bufferPlan) => {
        const section = parsed.sections[bufferPlan.name];
        const buffer = createLookupBuffer(device, section, inputUsage);
        inputBuffers.push(buffer);
        return {
          binding: bufferPlan.binding,
          resource: { buffer },
        };
      });
      entries.push({ binding: 6, resource: { buffer: output } });
    }

    const shader = device.createShaderModule({
      code: gpuCarrierLookupShaderSource(plan.mode, {
        handle,
        pole,
        linkCount: parsed.layout.linkCount,
      }),
    });
    if (typeof shader.getCompilationInfo === "function") {
      const info = await shader.getCompilationInfo();
      const errors = info.messages.filter(
        (message) => message.type === "error",
      );
      if (errors.length) {
        throw new Error(
          "C4 carrier lookup WGSL compilation failed: " +
          errors.map((message) => message.message).join(" | "),
        );
      }
    }

    const descriptor = {
      layout: "auto",
      compute: { module: shader, entryPoint: "main" },
    };
    const pipeline =
      typeof device.createComputePipelineAsync === "function"
        ? await device.createComputePipelineAsync(descriptor)
        : device.createComputePipeline(descriptor);
    const bindGroup = device.createBindGroup({
      layout: pipeline.getBindGroupLayout(0),
      entries,
    });
    const encoder = device.createCommandEncoder();
    const pass = encoder.beginComputePass();
    pass.setPipeline(pipeline);
    pass.setBindGroup(0, bindGroup);
    pass.dispatchWorkgroups(1);
    pass.end();
    device.queue.submit([encoder.finish()]);
    await device.queue.onSubmittedWorkDone();

    const observedWords = await readLookupWords(
      device,
      output,
      outputWordLength,
    );
    if (observedWords[0] !== 1) {
      throw new Error(
        `GPU carrier lookup failed closed with status ${observedWords[0]}`,
      );
    }
    const incidenceCount = observedWords[4] >>> 0;
    if (incidenceCount > parsed.layout.linkCount) {
      throw new Error("GPU carrier lookup returned invalid incidence count");
    }
    const observed = {
      handle,
      pole,
      start: observedWords[1] >>> 0,
      end: observedWords[2] >>> 0,
      head: observedWords[3] >>> 0,
      incidence: Array.from(
        observedWords.subarray(5, 5 + incidenceCount),
        (value) => value >>> 0,
      ),
      logicalFingerprint: expected.logicalFingerprint,
    };

    if (observed.start !== expected.start ||
        observed.end !== expected.end ||
        observed.head !== expected.head ||
        observed.incidence.length !== expected.incidence.length ||
        observed.incidence.some(
          (value, index) => value !== expected.incidence[index],
        )) {
      throw new Error("WebGPU carrier lookup differential mismatch");
    }

    return Object.freeze({
      plan,
      expected,
      observed: Object.freeze({
        ...observed,
        incidence: Object.freeze(observed.incidence),
      }),
      differential: true,
    });
  } finally {
    output.destroy();
    for (const buffer of inputBuffers) buffer.destroy();
    if (typeof device.popErrorScope === "function") {
      const validationError = await device.popErrorScope();
      if (validationError) {
        throw new Error(
          "WebGPU carrier lookup validation failed: " +
          validationError.message,
        );
      }
    }
  }
}


function carrierPoles(parsed, handle, label = "handle") {
  requireLookupHandle(parsed, handle, label);
  return Object.freeze({
    start: parsed.sections.starts[handle - 1] >>> 0,
    end: parsed.sections.ends[handle - 1] >>> 0,
  });
}

function carrierIncidence(parsed, pole, byStart) {
  requireLookupHandle(parsed, pole, "incidence pole");
  const head = byStart
    ? parsed.sections.startHead[pole] >>> 0
    : parsed.sections.endHead[pole] >>> 0;
  const next = byStart
    ? parsed.sections.nextByStart
    : parsed.sections.nextByEnd;
  const result = [];
  const seen = new Set();
  let current = head;
  while (current !== 0) {
    if (current > parsed.layout.linkCount || seen.has(current)) {
      throw new Error("invalid/cyclic carrier incidence chain");
    }
    seen.add(current);
    result.push(current);
    current = next[current] >>> 0;
  }
  return result;
}

function findCarrierPair(parsed, start, end) {
  for (const handle of carrierIncidence(parsed, start, true)) {
    const poles = carrierPoles(parsed, handle);
    if (poles.start === start && poles.end === end) return handle;
  }
  return 0;
}

function findCarrierStartSelf(parsed, child) {
  for (const handle of carrierIncidence(parsed, child, false)) {
    const poles = carrierPoles(parsed, handle);
    if (poles.start === handle && poles.end === child) return handle;
  }
  return 0;
}

function findCarrierEndSelf(parsed, child) {
  for (const handle of carrierIncidence(parsed, child, true)) {
    const poles = carrierPoles(parsed, handle);
    if (poles.start === child && poles.end === handle) return handle;
  }
  return 0;
}

function readCarrierExactSequence(parsed, finalHandle, cap = 64) {
  const root = parsed.layout.rootHandle;
  if (finalHandle === root) return [];
  const reversed = [];
  const seen = new Set();
  let current = requireLookupHandle(parsed, finalHandle, "sequence");
  while (current !== root) {
    if (reversed.length >= cap || seen.has(current)) {
      throw new Error("bounded carrier exact-sequence decode failed");
    }
    seen.add(current);
    const cell = carrierPoles(parsed, current);
    if (cell.start !== current || cell.end === current) {
      throw new Error("invalid carrier exact-sequence cell");
    }
    const payload = carrierPoles(parsed, cell.end);
    reversed.push(payload.end);
    current = payload.start;
  }
  reversed.reverse();
  return reversed;
}

function readCarrierRoleDictionary(parsed, dictionary) {
  const poles = carrierPoles(parsed, dictionary, "role dictionary");
  if (poles.start !== dictionary || poles.end === dictionary) {
    throw new Error("invalid carrier role dictionary");
  }
  const roles = readCarrierExactSequence(parsed, poles.end, 16);
  if (new Set(roles).size !== roles.length) {
    throw new Error("duplicate carrier role");
  }
  return roles;
}

function unifyCarrierTemplate(parsed, template, claimed, roles) {
  const roleSet = new Set(roles);
  const bindings = new Map();
  const stack = [[template, claimed]];
  const seen = new Set();
  let visited = 0;
  while (stack.length) {
    if (++visited > 256) throw new Error("bounded carrier unification exceeded");
    const [t, c] = stack.pop();
    requireLookupHandle(parsed, t, "template");
    requireLookupHandle(parsed, c, "claimed");
    if (roleSet.has(t)) {
      if (bindings.has(t) && bindings.get(t) !== c) return null;
      bindings.set(t, c);
      continue;
    }
    const key = t + ":" + c;
    if (seen.has(key)) continue;
    seen.add(key);
    const tp = carrierPoles(parsed, t);
    const cp = carrierPoles(parsed, c);
    if ((tp.start === t) !== (cp.start === c) ||
        (tp.end === t) !== (cp.end === c)) {
      return null;
    }
    if (tp.end !== t) stack.push([tp.end, cp.end]);
    if (tp.start !== t) stack.push([tp.start, cp.start]);
  }
  for (const role of roles) if (!bindings.has(role)) return null;
  return bindings;
}

function discoverCarrierRule(parsed, rule, active) {
  const rulePoles = carrierPoles(parsed, rule, "rule");
  const roles = readCarrierRoleDictionary(parsed, rulePoles.start);
  const body = carrierPoles(parsed, rulePoles.end, "rule body");
  const bindings = unifyCarrierTemplate(parsed, body.start, active, roles);
  if (!bindings) return null;
  requireLookupHandle(parsed, body.end, "output bundle template");
  return Object.freeze({
    rule,
    roles,
    bindings,
    outputBundleTemplate: body.end,
  });
}

function semanticRootHandle(roots, role) {
  if (!Array.isArray(roots)) {
    throw new TypeError("semantic roots are required");
  }
  const root = roots.find((entry) => entry.role === role);
  if (!root) throw new Error("semantic root missing: " + role);
  return root.carrierRef >>> 0;
}

function discoverCarrierReaction(
  parsed,
  { currentHandle, interpreterHandle } = {},
) {
  requireLookupHandle(parsed, currentHandle, "current handle");
  requireLookupHandle(parsed, interpreterHandle, "interpreter handle");
  const interpreter = carrierPoles(parsed, interpreterHandle);
  const grammarTheory = carrierPoles(parsed, interpreter.end);
  const theoryHandle = grammarTheory.end;
  const current = carrierPoles(parsed, currentHandle);
  const endpoint = carrierPoles(parsed, current.end);
  const triggerKey = endpoint.start;
  const matches = [];

  for (const trigger of carrierIncidence(parsed, triggerKey, true)) {
    if (trigger === triggerKey) continue;
    const triggerPoles = carrierPoles(parsed, trigger);
    if (triggerPoles.start !== triggerKey) continue;
    const admission = triggerPoles.end;
    const admissionPoles = carrierPoles(parsed, admission);
    if (admissionPoles.start !== theoryHandle ||
        admissionPoles.end === admission) {
      continue;
    }
    const discovered = discoverCarrierRule(
      parsed,
      admissionPoles.end,
      currentHandle,
    );
    if (!discovered) continue;
    matches.push(Object.freeze({
      ...discovered,
      admission,
    }));
  }

  if (matches.length !== 1) {
    throw new Error(
      "bounded C4c3 discovery requires exactly one Rule image; got " +
      matches.length,
    );
  }
  const match = matches[0];
  return Object.freeze({
    currentHandle,
    interpreterHandle,
    theoryHandle,
    triggerKey,
    admissionHandle: match.admission,
    ruleHandle: match.rule,
    rawRuleMatches: 1,
    outputBundleTemplate: match.outputBundleTemplate,
    roleBindings: match.roles.map(
      (role) => Object.freeze({
        role,
        value: match.bindings.get(role),
      }),
    ),
  });
}

function createVirtualOverlay(parsed) {
  return {
    baseCount: parsed.layout.linkCount,
    starts: [],
    ends: [],
  };
}

function virtualPair(store, handle) {
  if (handle <= store.baseCount) return null;
  const index = handle - store.baseCount - 1;
  if (index < 0 || index >= store.starts.length) return null;
  return { start: store.starts[index], end: store.ends[index] };
}

function virtualPoles(parsed, store, handle) {
  if (handle >= 1 && handle <= store.baseCount) {
    return carrierPoles(parsed, handle, "virtual base handle");
  }
  const pair = virtualPair(store, handle);
  if (!pair) {
    throw new Error("virtual overlay handle out of range: " + handle);
  }
  return pair;
}

function readVirtualExactSequence(parsed, store, finalHandle, cap = 2) {
  const root = parsed.layout.rootHandle;
  if (finalHandle === root) return [];
  const reversed = [];
  const seen = new Set();
  let current = finalHandle;
  while (current !== root) {
    if (reversed.length >= cap || seen.has(current)) {
      throw new Error("bounded virtual exact-sequence decode failed");
    }
    seen.add(current);
    const cell = virtualPoles(parsed, store, current);
    if (cell.start !== current || cell.end === current) {
      throw new Error("invalid virtual exact-sequence cell");
    }
    const payload = virtualPoles(parsed, store, cell.end);
    reversed.push(payload.end);
    current = payload.start;
  }
  reversed.reverse();
  return reversed;
}

function virtualFindPair(parsed, store, start, end) {
  for (let i = 0; i < store.starts.length; i += 1) {
    if (store.starts[i] === start && store.ends[i] === end) {
      return store.baseCount + i + 1;
    }
  }
  if (start <= store.baseCount && end <= store.baseCount) {
    return findCarrierPair(parsed, start, end);
  }
  return 0;
}

function virtualFindStartSelf(parsed, store, child) {
  for (let i = 0; i < store.starts.length; i += 1) {
    const handle = store.baseCount + i + 1;
    if (store.starts[i] === handle && store.ends[i] === child) {
      return handle;
    }
  }
  return child <= store.baseCount
    ? findCarrierStartSelf(parsed, child)
    : 0;
}

function virtualFindEndSelf(parsed, store, child) {
  for (let i = 0; i < store.starts.length; i += 1) {
    const handle = store.baseCount + i + 1;
    if (store.starts[i] === child && store.ends[i] === handle) {
      return handle;
    }
  }
  return child <= store.baseCount
    ? findCarrierEndSelf(parsed, child)
    : 0;
}

function virtualAppendPair(store, start, end, maxAppend) {
  if (store.starts.length >= maxAppend) return 0;
  const handle = store.baseCount + store.starts.length + 1;
  store.starts.push(start === -1 ? handle : start);
  store.ends.push(end === -1 ? handle : end);
  return handle;
}

function instantiateVirtualTemplate(
  parsed,
  source,
  bindings,
  store,
  maxAppend,
  memo = new Map(),
  visiting = new Set(),
  depth = 0,
) {
  if (depth > 128) throw new Error("bounded C4c3 instantiation exceeded");
  if (bindings.has(source)) return bindings.get(source);
  if (memo.has(source)) return memo.get(source);
  if (visiting.has(source)) return 0;

  const poles = carrierPoles(parsed, source, "publication template");
  let value = 0;
  if (poles.start === source && poles.end === source) {
    value = parsed.layout.rootHandle;
  } else if (poles.start === source) {
    const child = instantiateVirtualTemplate(
      parsed,
      poles.end,
      bindings,
      store,
      maxAppend,
      memo,
      visiting,
      depth + 1,
    );
    value = child
      ? virtualFindStartSelf(parsed, store, child) ||
        virtualAppendPair(store, -1, child, maxAppend)
      : 0;
  } else if (poles.end === source) {
    const child = instantiateVirtualTemplate(
      parsed,
      poles.start,
      bindings,
      store,
      maxAppend,
      memo,
      visiting,
      depth + 1,
    );
    value = child
      ? virtualFindEndSelf(parsed, store, child) ||
        virtualAppendPair(store, child, -1, maxAppend)
      : 0;
  } else {
    visiting.add(source);
    const start = instantiateVirtualTemplate(
      parsed,
      poles.start,
      bindings,
      store,
      maxAppend,
      memo,
      visiting,
      depth + 1,
    );
    const end = instantiateVirtualTemplate(
      parsed,
      poles.end,
      bindings,
      store,
      maxAppend,
      memo,
      visiting,
      depth + 1,
    );
    visiting.delete(source);
    value = start && end
      ? virtualFindPair(parsed, store, start, end) ||
        virtualAppendPair(store, start, end, maxAppend)
      : 0;
  }
  if (value) memo.set(source, value);
  return value;
}

function publishCarrierReaction(
  parsed,
  discovery,
  { maxAppend = 64 } = {},
) {
  const bindings = new Map(
    discovery.roleBindings.map(({ role, value }) => [role, value]),
  );
  const store = createVirtualOverlay(parsed);
  const groundedBundle = instantiateVirtualTemplate(
    parsed,
    discovery.outputBundleTemplate,
    bindings,
    store,
    maxAppend,
  );
  if (!groundedBundle) {
    throw new Error("bounded C4c3 bundle instantiation failed");
  }
  const outputs = readVirtualExactSequence(parsed, store, groundedBundle, 2);
  if (outputs.length !== 1) {
    throw new Error(
      "bounded C4c3 publication requires exactly one output; got " +
      outputs.length,
    );
  }
  return Object.freeze({
    candidateHandle: outputs[0],
    groundedBundle,
    appendCount: store.starts.length,
    appendStarts: Object.freeze([...store.starts]),
    appendEnds: Object.freeze([...store.ends]),
  });
}

function oracleParsedWithCommittedAppend(parsed, appendStarts, appendEnds) {
  if (!Array.isArray(appendStarts) || !Array.isArray(appendEnds) ||
      appendStarts.length !== appendEnds.length) {
    throw new TypeError("resident oracle append arrays are invalid");
  }
  if (appendStarts.length === 0) return parsed;

  const baseCount = parsed.layout.linkCount;
  const linkCount = baseCount + appendStarts.length;
  const starts = new Uint32Array(linkCount);
  const ends = new Uint32Array(linkCount);
  starts.set(parsed.sections.starts, 0);
  ends.set(parsed.sections.ends, 0);
  for (let i = 0; i < appendStarts.length; i += 1) {
    const start = appendStarts[i] >>> 0;
    const end = appendEnds[i] >>> 0;
    if (start === 0 || end === 0 || start > linkCount || end > linkCount) {
      throw new Error("resident oracle append contains an invalid handle");
    }
    starts[baseCount + i] = start;
    ends[baseCount + i] = end;
  }

  const startHead = new Uint32Array(linkCount + 1);
  const endHead = new Uint32Array(linkCount + 1);
  const nextByStart = new Uint32Array(linkCount + 1);
  const nextByEnd = new Uint32Array(linkCount + 1);
  for (let handle = 1; handle <= linkCount; handle += 1) {
    const start = starts[handle - 1] >>> 0;
    const end = ends[handle - 1] >>> 0;
    nextByStart[handle] = startHead[start] >>> 0;
    startHead[start] = handle;
    nextByEnd[handle] = endHead[end] >>> 0;
    endHead[end] = handle;
  }

  return Object.freeze({
    words: parsed.words,
    layout: Object.freeze({ ...parsed.layout, linkCount }),
    sections: Object.freeze({
      starts,
      ends,
      startHead,
      endHead,
      nextByStart,
      nextByEnd,
    }),
  });
}

export function expectedGpuCarrierReaction(
  parsed,
  witness,
  options = {},
) {
  const discovery = discoverCarrierReaction(parsed, witness);
  const publication = publishCarrierReaction(
    parsed,
    discovery,
    options,
  );
  return Object.freeze({
    ...discovery,
    ...publication,
    logicalFingerprint: gpuCarrierLogicalFingerprint(parsed),
  });
}

export function deriveGpuCarrierReactionInput(parsed, roots) {
  const currentHandle = semanticRootHandle(roots, "scope.initial");
  const interpreterHandle = semanticRootHandle(
    roots,
    "execution.interpreter",
  );
  requireLookupHandle(parsed, currentHandle, "initial scope handle");
  requireLookupHandle(parsed, interpreterHandle, "interpreter handle");
  return Object.freeze({ currentHandle, interpreterHandle });
}

function carrierReactionCommonShader(args) {
  return [
    "const CURRENT: u32 = " + args.currentHandle + "u;",
    "const INTERPRETER: u32 = " + args.interpreterHandle + "u;",
    "const LINK_COUNT: u32 = " + args.linkCount + "u;",
    "const ROOT_HANDLE: u32 = " + args.rootHandle + "u;",
    "const MAX_ROLES: u32 = 16u;",
    "const MAX_STACK: u32 = 128u;",
    "const MAX_VISITS: u32 = 256u;",
    "const MAX_APPEND: u32 = 64u;",
    "const RESIDENT_CAPACITY: u32 = " + args.residentCapacity + "u;",
    "fn valid_base(h: u32) -> bool { return h > 0u && h <= LINK_COUNT; }",
    "fn find_pair_base(a: u32, b: u32) -> u32 {",
    "  if (!valid_base(a) || !valid_base(b)) { return 0u; }",
    "  var h = start_head(a); var n = 0u;",
    "  loop { if (h == 0u) { break; } if (!valid_base(h) || n >= LINK_COUNT) { return 0u; } if (s(h) == a && e(h) == b) { return h; } h = next_start(h); n = n + 1u; }",
    "  return 0u;",
    "}",
    "fn find_start_self_base(child: u32) -> u32 {",
    "  if (!valid_base(child)) { return 0u; }",
    "  var h = end_head(child); var n = 0u;",
    "  loop { if (h == 0u) { break; } if (!valid_base(h) || n >= LINK_COUNT) { return 0u; } if (s(h) == h && e(h) == child) { return h; } h = next_end(h); n = n + 1u; }",
    "  return 0u;",
    "}",
    "fn find_end_self_base(child: u32) -> u32 {",
    "  if (!valid_base(child)) { return 0u; }",
    "  var h = start_head(child); var n = 0u;",
    "  loop { if (h == 0u) { break; } if (!valid_base(h) || n >= LINK_COUNT) { return 0u; } if (s(h) == child && e(h) == h) { return h; } h = next_start(h); n = n + 1u; }",
    "  return 0u;",
    "}",
    "fn discover_rule(rule: u32, active_handle: u32, record_diag: bool) -> u32 {",
    "  if (record_diag) { discovery[9] = 20u; }",
    "  if (!valid_base(rule) || !valid_value(active_handle)) { return 0u; }",
    "  let dictionary = s(rule); let body = e(rule);",
    "  if (!valid_base(dictionary) || !valid_base(body) || s(dictionary) != dictionary || e(dictionary) == dictionary) { if (record_diag) { discovery[9] = 21u; } return 0u; }",
    "  var roles: array<u32, 16>; var bound: array<u32, 16>; var bound_set: array<u32, 16>;",
    "  var role_count = 0u; var seq = e(dictionary); var seq_guard = 0u;",
    "  loop {",
    "    if (seq == ROOT_HANDLE) { break; }",
    "    if (!valid_base(seq) || seq_guard >= 64u || s(seq) != seq || e(seq) == seq || role_count >= MAX_ROLES) { if (record_diag) { discovery[8] = role_count; discovery[9] = 22u; } return 0u; }",
    "    let cell = e(seq); if (!valid_base(cell)) { if (record_diag) { discovery[8] = role_count; discovery[9] = 23u; } return 0u; }",
    "    let role = e(cell); var duplicate = false; var ri = 0u;",
    "    loop { if (ri >= role_count) { break; } if (roles[ri] == role) { duplicate = true; break; } ri = ri + 1u; }",
    "    if (duplicate) { if (record_diag) { discovery[8] = role_count; discovery[9] = 24u; } return 0u; } roles[role_count] = role; role_count = role_count + 1u; seq = s(cell); seq_guard = seq_guard + 1u;",
    "  }",
    "  var role_left = 0u;",
    "  loop {",
    "    if (role_left >= role_count / 2u) { break; }",
    "    let role_right = role_count - role_left - 1u;",
    "    let role_swap = roles[role_left]; roles[role_left] = roles[role_right]; roles[role_right] = role_swap;",
    "    role_left = role_left + 1u;",
    "  }",
    "  let before = s(body); let bundle = e(body);",
    "  var ts: array<u32, 128>; var cs: array<u32, 128>; var seen_t: array<u32, 256>; var seen_c: array<u32, 256>; var sp = 1u; var visited = 0u; var seen_count = 0u; ts[0] = before; cs[0] = active_handle;",
    "  loop {",
    "    if (sp == 0u) { break; } if (visited >= MAX_VISITS) { if (record_diag) { discovery[8] = role_count; discovery[9] = 32u; } return 0u; }",
    "    sp = sp - 1u; let t = ts[sp]; let c = cs[sp]; visited = visited + 1u; if (!valid_base(t) || !valid_value(c)) { if (record_diag) { discovery[8] = role_count; discovery[9] = 25u; } return 0u; }",
    "    var role_index = 0xffffffffu; var r = 0u; loop { if (r >= role_count) { break; } if (roles[r] == t) { role_index = r; break; } r = r + 1u; }",
    "    if (role_index != 0xffffffffu) { if (bound_set[role_index] != 0u && bound[role_index] != c) { if (record_diag) { discovery[8] = role_count; discovery[9] = 26u; } return 0u; } bound[role_index] = c; bound_set[role_index] = 1u; continue; }",
    "    var already_seen = false; var si = 0u; loop { if (si >= seen_count) { break; } if (seen_t[si] == t && seen_c[si] == c) { already_seen = true; break; } si = si + 1u; }",
    "    if (already_seen) { continue; } if (seen_count >= MAX_VISITS) { if (record_diag) { discovery[8] = role_count; discovery[9] = 32u; } return 0u; } seen_t[seen_count] = t; seen_c[seen_count] = c; seen_count = seen_count + 1u;",
    "    let ta = s(t); let tb = e(t); let ca = value_start(c); let cb = value_end(c);",
    "    if ((ta == t) != (ca == c) || (tb == t) != (cb == c)) { if (record_diag) { discovery[8] = role_count; discovery[9] = 27u; discovery[10] = t; discovery[11] = c; discovery[12] = ta; discovery[13] = tb; discovery[14] = ca; discovery[15] = cb; if (role_count > 0u) { discovery[16] = roles[0]; } } return 0u; }",
    "    if (tb != t) { if (sp >= MAX_STACK) { if (record_diag) { discovery[8] = role_count; discovery[9] = 28u; } return 0u; } ts[sp] = tb; cs[sp] = cb; sp = sp + 1u; }",
    "    if (ta != t) { if (sp >= MAX_STACK) { if (record_diag) { discovery[8] = role_count; discovery[9] = 29u; } return 0u; } ts[sp] = ta; cs[sp] = ca; sp = sp + 1u; }",
    "  }",
    "  var rr = 0u; loop { if (rr >= role_count) { break; } if (bound_set[rr] == 0u) { if (record_diag) { discovery[8] = role_count; discovery[9] = 30u; } return 0u; } rr = rr + 1u; }",
    "  if (!valid_base(bundle)) { if (record_diag) { discovery[8] = role_count; discovery[9] = 31u; } return 0u; }",
    "  discovery[8] = role_count; discovery[9] = bundle;",
    "  var wi = 0u; loop { if (wi >= role_count) { break; } discovery[10u + wi * 2u] = roles[wi]; discovery[11u + wi * 2u] = bound[wi]; wi = wi + 1u; }",
    "  return bundle;",
    "}",
    "fn resident_start(i: u32) -> u32 { return resident[8u + i * 2u]; }",
    "fn resident_end(i: u32) -> u32 { return resident[9u + i * 2u]; }",
    "fn resident_handle(i: u32) -> u32 { return LINK_COUNT + i + 1u; }",
    "fn valid_value(h: u32) -> bool { return h > 0u && h <= LINK_COUNT + resident[0]; }",
    "fn value_start(h: u32) -> u32 { if (h <= LINK_COUNT) { return s(h); } let i = h - LINK_COUNT - 1u; if (i >= resident[0]) { return 0u; } return resident_start(i); }",
    "fn value_end(h: u32) -> u32 { if (h <= LINK_COUNT) { return e(h); } let i = h - LINK_COUNT - 1u; if (i >= resident[0]) { return 0u; } return resident_end(i); }",
    "fn read_single_output_bundle(bundle: u32) -> u32 {",
    "  if (bundle == ROOT_HANDLE || !valid_value(bundle)) { return 0u; }",
    "  let cell_start = value_start(bundle); let cell_end = value_end(bundle);",
    "  if (cell_start != bundle || cell_end == bundle || !valid_value(cell_end)) { return 0u; }",
    "  if (value_start(cell_end) != ROOT_HANDLE) { return 0u; }",
    "  return value_end(cell_end);",
    "}",
    "fn resident_can_append() -> bool {",
    "  if (resident[0] >= RESIDENT_CAPACITY || resident[0] - resident[1] >= MAX_APPEND) { resident[3] = 4u; return false; }",
    "  return true;",
    "}",
    "fn ensure_pair_resident(a: u32, b: u32) -> u32 {",
    "  var i = 0u; loop { if (i >= resident[0]) { break; } if (resident_start(i) == a && resident_end(i) == b) { return resident_handle(i); } i = i + 1u; }",
    "  let base = find_pair_base(a, b); if (base != 0u) { return base; }",
    "  if (!valid_value(a) || !valid_value(b) || !resident_can_append()) { return 0u; }",
    "  let count = resident[0]; let h = resident_handle(count); resident[8u + count * 2u] = a; resident[9u + count * 2u] = b; resident[0] = count + 1u; return h;",
    "}",
    "fn ensure_start_self_resident(child: u32) -> u32 {",
    "  var i = 0u; loop { if (i >= resident[0]) { break; } let h = resident_handle(i); if (resident_start(i) == h && resident_end(i) == child) { return h; } i = i + 1u; }",
    "  let base = find_start_self_base(child); if (base != 0u) { return base; }",
    "  if (!valid_value(child) || !resident_can_append()) { return 0u; } let count = resident[0]; let h = resident_handle(count); resident[8u + count * 2u] = h; resident[9u + count * 2u] = child; resident[0] = count + 1u; return h;",
    "}",
    "fn ensure_end_self_resident(child: u32) -> u32 {",
    "  var i = 0u; loop { if (i >= resident[0]) { break; } let h = resident_handle(i); if (resident_start(i) == child && resident_end(i) == h) { return h; } i = i + 1u; }",
    "  let base = find_end_self_base(child); if (base != 0u) { return base; }",
    "  if (!valid_value(child) || !resident_can_append()) { return 0u; } let count = resident[0]; let h = resident_handle(count); resident[8u + count * 2u] = child; resident[9u + count * 2u] = h; resident[0] = count + 1u; return h;",
    "}",
    "fn instantiate_output(source: u32) -> u32 {",
    "  let role_count = discovery[8]; if (role_count > MAX_ROLES || !valid_base(source)) { return 0u; }",
    "  var nodes: array<u32, 128>; var states: array<u32, 128>; var isp = 1u;",
    "  var memo_node: array<u32, 128>; var memo_value: array<u32, 128>; var memo_count = 0u;",
    "  nodes[0] = source; states[0] = 0u; var inst_guard = 0u;",
    "  loop {",
    "    if (isp == 0u) { break; } if (inst_guard >= 512u) { return 0u; } inst_guard = inst_guard + 1u;",
    "    let ix = isp - 1u; let node = nodes[ix]; var existing = 0u; var mi = 0u;",
    "    loop { if (mi >= memo_count) { break; } if (memo_node[mi] == node) { existing = memo_value[mi]; break; } mi = mi + 1u; }",
    "    if (existing != 0u) { isp = isp - 1u; continue; }",
    "    var role_value = 0u; var ri = 0u; loop { if (ri >= role_count) { break; } if (discovery[10u + ri * 2u] == node) { role_value = discovery[11u + ri * 2u]; break; } ri = ri + 1u; }",
    "    if (role_value != 0u) { if (memo_count >= MAX_STACK) { return 0u; } memo_node[memo_count] = node; memo_value[memo_count] = role_value; memo_count = memo_count + 1u; isp = isp - 1u; continue; }",
    "    if (!valid_base(node)) { return 0u; } let na = s(node); let nb = e(node);",
    "    if (states[ix] == 0u) {",
    "      states[ix] = 1u;",
    "      if (nb != node) { if (isp >= MAX_STACK) { return 0u; } nodes[isp] = nb; states[isp] = 0u; isp = isp + 1u; }",
    "      if (na != node) { if (isp >= MAX_STACK) { return 0u; } nodes[isp] = na; states[isp] = 0u; isp = isp + 1u; } continue;",
    "    }",
    "    var av = node; var bv = node;",
    "    if (na != node) { av = 0u; var ai = 0u; loop { if (ai >= memo_count) { break; } if (memo_node[ai] == na) { av = memo_value[ai]; break; } ai = ai + 1u; } if (av == 0u) { return 0u; } }",
    "    if (nb != node) { bv = 0u; var bi = 0u; loop { if (bi >= memo_count) { break; } if (memo_node[bi] == nb) { bv = memo_value[bi]; break; } bi = bi + 1u; } if (bv == 0u) { return 0u; } }",
    "    var value = 0u;",
    "    if (na == node && nb == node) { value = ROOT_HANDLE; } else if (na == node) { value = ensure_start_self_resident(bv); } else if (nb == node) { value = ensure_end_self_resident(av); } else { value = ensure_pair_resident(av, bv); }",
    "    if (value == 0u || memo_count >= MAX_STACK) { return 0u; } memo_node[memo_count] = node; memo_value[memo_count] = value; memo_count = memo_count + 1u; isp = isp - 1u;",
    "  }",
    "  var result = 0u; var oi = 0u; loop { if (oi >= memo_count) { break; } if (memo_node[oi] == source) { result = memo_value[oi]; break; } oi = oi + 1u; } return result;",
    "}",
    "@compute @workgroup_size(1)",
    "fn discover(@builtin(global_invocation_id) id: vec3<u32>) {",
    "  if (id.x != 0u) { return; } var z = 0u; loop { if (z >= 67u) { break; } discovery[z] = 0u; z = z + 1u; }",
    "  discovery[0] = 10u; discovery[2] = CURRENT; discovery[3] = INTERPRETER;",
    "  if (!valid_value(CURRENT) || !valid_base(INTERPRETER)) { discovery[0] = 2u; return; }",
    "  let grammar_theory = e(INTERPRETER); if (!valid_base(grammar_theory)) { discovery[0] = 3u; return; }",
    "  let theory = e(grammar_theory); discovery[4] = theory; let endpoint = value_end(CURRENT); if (!valid_base(theory) || !valid_value(endpoint)) { discovery[0] = 4u; return; }",
    "  let trigger_key = value_start(endpoint); discovery[5] = trigger_key; if (!valid_base(trigger_key)) { discovery[0] = 5u; return; }",
    "  var trigger = start_head(trigger_key); var guard = 0u; var matches = 0u; var candidates = 0u; var matched_rule = 0u; var matched_admission = 0u;",
    "  loop {",
    "    if (trigger == 0u) { break; } if (!valid_base(trigger) || guard >= LINK_COUNT) { discovery[0] = 6u; return; }",
    "    if (trigger != trigger_key && s(trigger) == trigger_key) {",
    "      let admission = e(trigger);",
    "      if (valid_base(admission) && s(admission) == theory && e(admission) != admission) {",
    "        candidates = candidates + 1u; discovery[42] = candidates;",
    "        let rule = e(admission); discovery[6] = rule; discovery[7] = admission; let record_diag = matches == 0u; let output_template = discover_rule(rule, CURRENT, record_diag);",
    "        if (output_template == 0u && record_diag && candidates <= 12u) { let trace = 43u + (candidates - 1u) * 2u; discovery[trace] = rule; discovery[trace + 1u] = (discovery[8] << 16u) | (discovery[9] & 0xffffu); }",
    "        if (output_template != 0u) { matches = matches + 1u; discovery[1] = matches; if (matches > 1u) { discovery[0] = 9u; return; } matched_rule = rule; matched_admission = admission; }",
    "      }",
    "    }",
    "    trigger = next_start(trigger); guard = guard + 1u;",
    "  }",
    "  if (matches != 1u) { discovery[0] = 8u; return; }",
    "  discovery[1] = matches; discovery[2] = CURRENT; discovery[3] = INTERPRETER; discovery[4] = theory; discovery[5] = trigger_key; discovery[6] = matched_rule; discovery[7] = matched_admission; discovery[0] = 1u;",
    "}",
    "@compute @workgroup_size(1)",
    "fn publish(@builtin(global_invocation_id) id: vec3<u32>) {",
    "  if (id.x != 0u) { return; } resident[1] = resident[0]; resident[2] = 0u; resident[3] = 0u; resident[4] = 0u; resident[5] = RESIDENT_CAPACITY;",
    "  if (discovery[0] != 1u || discovery[9] == 0u) { return; }",
    "  let grounded_bundle = instantiate_output(discovery[9]); if (grounded_bundle == 0u) { resident[0] = resident[1]; if (resident[3] == 0u) { resident[3] = 2u; } return; }",
    "  let candidate = read_single_output_bundle(grounded_bundle); if (candidate == 0u) { resident[0] = resident[1]; if (resident[3] == 0u) { resident[3] = 3u; } return; }",
    "  resident[2] = candidate; resident[3] = 1u; resident[4] = grounded_bundle;",
    "}",
  ].join("\n");
}

function singleReactionShader(args) {
  return [
    "@group(0) @binding(0) var<storage, read> carrier: array<u32>;",
    "@group(0) @binding(1) var<storage, read_write> discovery: array<u32>;",
    "@group(0) @binding(2) var<storage, read_write> resident: array<u32>;"
    "fn s(h: u32) -> u32 { return carrier[carrier[6] + h - 1u]; }",
    "fn e(h: u32) -> u32 { return carrier[carrier[7] + h - 1u]; }",
    "fn start_head(h: u32) -> u32 { return carrier[carrier[8] + h]; }",
    "fn end_head(h: u32) -> u32 { return carrier[carrier[9] + h]; }",
    "fn next_start(h: u32) -> u32 { return carrier[carrier[10] + h]; }",
    "fn next_end(h: u32) -> u32 { return carrier[carrier[11] + h]; }",
  ].join("\n") + "\n" + carrierReactionCommonShader(args);
}

function sectionReactionShader(args) {
  return [
    "@group(0) @binding(0) var<storage, read> starts: array<u32>;",
    "@group(0) @binding(1) var<storage, read> ends: array<u32>;",
    "@group(0) @binding(2) var<storage, read> start_heads: array<u32>;",
    "@group(0) @binding(3) var<storage, read> end_heads: array<u32>;",
    "@group(0) @binding(4) var<storage, read> next_starts: array<u32>;",
    "@group(0) @binding(5) var<storage, read> next_ends: array<u32>;",
    "@group(0) @binding(6) var<storage, read_write> discovery: array<u32>;",
    "@group(0) @binding(7) var<storage, read_write> resident: array<u32>;"
    "fn s(h: u32) -> u32 { return starts[h - 1u]; }",
    "fn e(h: u32) -> u32 { return ends[h - 1u]; }",
    "fn start_head(h: u32) -> u32 { return start_heads[h]; }",
    "fn end_head(h: u32) -> u32 { return end_heads[h]; }",
    "fn next_start(h: u32) -> u32 { return next_starts[h]; }",
    "fn next_end(h: u32) -> u32 { return next_ends[h]; }",
  ].join("\n") + "\n" + carrierReactionCommonShader(args);
}

export function gpuCarrierReactionShaderSource(
  mode,
  { currentHandle, interpreterHandle, linkCount, rootHandle = 1, residentCapacity = 256 } = {},
) {
  checkedInteger(linkCount, "linkCount", { min: 1 });
  checkedInteger(currentHandle, "currentHandle", { min: 1 });
  checkedInteger(interpreterHandle, "interpreterHandle", { min: 1 });
  checkedInteger(rootHandle, "rootHandle", { min: 1 });
  checkedInteger(residentCapacity, "residentCapacity", { min: 1 });
  const args = { currentHandle, interpreterHandle, linkCount, rootHandle, residentCapacity };
  if (mode === "single") return singleReactionShader(args);
  if (mode === "sections") return sectionReactionShader(args);
  throw new Error("unsupported GPU carrier reaction mode: " + mode);
}

// Truth boundary: this executor accepts only carrier + current/interpreter handles.
// Proof traces and expected successors are deliberately unavailable until readback.
export async function openGpuCarrierResidentSession(
  device,
  parsed,
  { maxDiagnosticLinks = 16_384, maxResidentAppend = 256 } = {},
) {
  if (!device?.createBuffer || !device?.queue) {
    throw new TypeError("WebGPU device is required");
  }
  checkedInteger(maxDiagnosticLinks, "maxDiagnosticLinks", { min: 1 });
  checkedInteger(maxResidentAppend, "maxResidentAppend", { min: 1 });
  if (parsed.layout.linkCount > maxDiagnosticLinks) {
    throw new RangeError("bounded GPU reaction carrier too large");
  }

  const plan = planGpuCarrierUpload(parsed.layout, device.limits);
  if (plan.mode === "unsupported") {
    throw new Error("WebGPU carrier upload unsupported: " + plan.reason);
  }
  if (plan.storageBufferCount + 2 >
      Number(device.limits.maxStorageBuffersPerShaderStage)) {
    throw new Error(
      "WebGPU carrier reaction needs discovery + resident append bindings",
    );
  }

  const usage = gpuLookupUsage();
  const inputUsage = usage.STORAGE | usage.COPY_DST;
  const residentUsage = usage.STORAGE | usage.COPY_SRC | usage.COPY_DST;
  const baseBuffers = [];
  let baseEntries = null;
  let residentBuffer = null;
  const residentWords = new Uint32Array(8 + maxResidentAppend * 2);
  residentWords[5] = maxResidentAppend;
  device.pushErrorScope?.("validation");
  try {
    if (plan.mode === "single") {
      const carrier = createLookupBuffer(device, parsed.words, inputUsage);
      baseBuffers.push(carrier);
      baseEntries = [{ binding: 0, resource: { buffer: carrier } }];
    } else {
      baseEntries = plan.buffers.map((part) => {
        const buffer = createLookupBuffer(
          device,
          parsed.sections[part.name],
          inputUsage,
        );
        baseBuffers.push(buffer);
        return { binding: part.binding, resource: { buffer } };
      });
    }
    residentBuffer = createLookupBuffer(
      device,
      residentWords,
      residentUsage,
    );
  } catch (error) {
    residentBuffer?.destroy();
    for (const buffer of baseBuffers) buffer.destroy();
    if (typeof device.popErrorScope === "function") {
      await device.popErrorScope();
    }
    throw error;
  }

  if (typeof device.popErrorScope === "function") {
    const validationError = await device.popErrorScope();
    if (validationError) {
      residentBuffer?.destroy();
      for (const buffer of baseBuffers) buffer.destroy();
      throw new Error(
        "WebGPU resident carrier upload validation failed: " +
        validationError.message,
      );
    }
  }

  const state = {
    plan,
    baseEntries,
    baseBuffers,
    residentBuffer,
    residentCapacity: maxResidentAppend,
    residentAppendCount: 0,
    residentBufferBytes: residentWords.byteLength,
    closed: false,
    reactionDispatchCount: 0,
  };
  const telemetry = () => Object.freeze({
    baseUploadCount: 1,
    baseUploadBytes: plan.logicalBytes,
    baseBufferCount: baseBuffers.length,
    residentAppendCount: state.residentAppendCount,
    residentCapacity: state.residentCapacity,
    residentBufferBytes: state.residentBufferBytes,
    reactionDispatchCount: state.reactionDispatchCount,
    closed: state.closed,
  });
  const close = () => {
    if (state.closed) return false;
    state.closed = true;
    residentBuffer.destroy();
    for (const buffer of baseBuffers) buffer.destroy();
    return true;
  };
  const reaction = async (input, { maxAppend = 64 } = {}) => {
    if (state.closed) {
      throw new Error("WebGPU resident carrier Session is closed");
    }
    const result = await runGpuCarrierReactionOnResidentBase(
      device,
      parsed,
      input,
      state,
      { maxAppend },
    );
    state.reactionDispatchCount += 1;
    return Object.freeze({
      ...result,
      residency: telemetry(),
    });
  };

  return Object.freeze({
    plan,
    logicalFingerprint: gpuCarrierLogicalFingerprint(parsed),
    telemetry,
    reaction,
    close,
  });
}

async function runGpuCarrierReactionOnResidentBase(
  device,
  parsed,
  input,
  resident,
  { maxAppend = 64 } = {},
) {
  if (resident.closed) {
    throw new Error("WebGPU resident carrier Session is closed");
  }
  checkedInteger(maxAppend, "maxAppend", { min: 1 });
  if (maxAppend !== 64) {
    throw new Error("C4c3 shader currently fixes maxAppend at 64");
  }
  checkedInteger(input?.currentHandle, "current handle", { min: 1 });
  const maxCurrentHandle =
    parsed.layout.linkCount + resident.residentAppendCount;
  if (input.currentHandle > maxCurrentHandle) {
    throw new RangeError(
      "current handle " + input.currentHandle +
      " exceeds resident Link count " + maxCurrentHandle,
    );
  }
  requireLookupHandle(parsed, input?.interpreterHandle, "interpreter handle");

  const plan = resident.plan;
  const baseEntries = resident.baseEntries;
  const usage = gpuLookupUsage();
  const outputUsage = usage.STORAGE | usage.COPY_SRC | usage.COPY_DST;
  let discovery = null;
  device.pushErrorScope?.("validation");
  try {
    discovery = createLookupBuffer(
      device,
      new Uint32Array(67),
      outputUsage,
    );
    const discoveryBinding = plan.mode === "single" ? 1 : 6;
    const residentBinding = plan.mode === "single" ? 2 : 7;

    const shader = device.createShaderModule({
      code: gpuCarrierReactionShaderSource(plan.mode, {
        currentHandle: input.currentHandle,
        interpreterHandle: input.interpreterHandle,
        linkCount: parsed.layout.linkCount,
        rootHandle: parsed.layout.rootHandle,
        residentCapacity: resident.residentCapacity,
      }),
    });
    if (typeof shader.getCompilationInfo === "function") {
      const info = await shader.getCompilationInfo();
      const errors = info.messages.filter((message) => message.type === "error");
      if (errors.length) {
        throw new Error(
          "C4c3 WGSL compilation failed: " +
          errors.map((message) => message.message).join(" | "),
        );
      }
    }
    const makePipeline = async (entryPoint) => {
      const descriptor = {
        layout: "auto",
        compute: { module: shader, entryPoint },
      };
      return typeof device.createComputePipelineAsync === "function"
        ? device.createComputePipelineAsync(descriptor)
        : device.createComputePipeline(descriptor);
    };
    const discoverPipeline = await makePipeline("discover");
    const publishPipeline = await makePipeline("publish");
    const commonEntries = [
      ...baseEntries,
      { binding: residentBinding, resource: { buffer: resident.residentBuffer } },
    ];
    const discoverGroup = device.createBindGroup({
      layout: discoverPipeline.getBindGroupLayout(0),
      entries: [
        ...commonEntries,
        { binding: discoveryBinding, resource: { buffer: discovery } },
      ].sort((a, b) => a.binding - b.binding),
    });
    const publishGroup = device.createBindGroup({
      layout: publishPipeline.getBindGroupLayout(0),
      entries: [
        ...commonEntries,
        { binding: discoveryBinding, resource: { buffer: discovery } },
      ].sort((a, b) => a.binding - b.binding),
    });

    const discoverEncoder = device.createCommandEncoder();
    const discoverPass = discoverEncoder.beginComputePass();
    discoverPass.setPipeline(discoverPipeline);
    discoverPass.setBindGroup(0, discoverGroup);
    discoverPass.dispatchWorkgroups(1);
    discoverPass.end();

    device.queue.submit([discoverEncoder.finish()]);
    await device.queue.onSubmittedWorkDone();

    // Diagnostic readback only. PUBLISH consumes the same GPU-resident
    // discovery buffer; neither CPU facts nor proof data seed GPU execution.
    const d = await readLookupWords(device, discovery, 67);
    if ((d[0] >>> 0) !== 1) {
      let cpuDiscovery = null;
      let cpuDiscoveryError = null;
      if (input.currentHandle <= parsed.layout.linkCount) {
        try {
          cpuDiscovery = discoverCarrierReaction(parsed, input);
        } catch (error) {
          cpuDiscoveryError = error?.message || String(error);
        }
      }
      let cpuRuleGpuRoles = 0;
      let cpuRuleGpuDiagnostic = 0;
      const cpuRule = cpuDiscovery?.ruleHandle ?? 0;
      const tracedCandidates = Math.min(d[42] >>> 0, 12);
      for (let index = 0; index < tracedCandidates; index += 1) {
        const trace = 43 + index * 2;
        if ((d[trace] >>> 0) === cpuRule) {
          const packed = d[trace + 1] >>> 0;
          cpuRuleGpuRoles = packed >>> 16;
          cpuRuleGpuDiagnostic = packed & 0xffff;
          break;
        }
      }
      throw new Error(
        "WebGPU DISCOVER failed closed with status " + (d[0] >>> 0) +
        " matches=" + (d[1] >>> 0) +
        " candidates=" + (d[42] >>> 0) +
        " current=" + (d[2] >>> 0) +
        " interpreter=" + (d[3] >>> 0) +
        " theory=" + (d[4] >>> 0) +
        " trigger=" + (d[5] >>> 0) +
        " rule=" + (d[6] >>> 0) +
        " admission=" + (d[7] >>> 0) +
        " roles=" + (d[8] >>> 0) +
        " diagnostic=" + (d[9] >>> 0) +
        " t=" + (d[10] >>> 0) +
        " c=" + (d[11] >>> 0) +
        " ta=" + (d[12] >>> 0) +
        " tb=" + (d[13] >>> 0) +
        " ca=" + (d[14] >>> 0) +
        " cb=" + (d[15] >>> 0) +
        " role0=" + (d[16] >>> 0) +
        " cpuRule=" + cpuRule +
        " cpuAdmission=" + (cpuDiscovery?.admissionHandle ?? 0) +
        " cpuRoles=" + (cpuDiscovery?.roleBindings?.length ?? 0) +
        " cpuRuleGpuRoles=" + cpuRuleGpuRoles +
        " cpuRuleGpuDiagnostic=" + cpuRuleGpuDiagnostic +
        (cpuDiscoveryError ? " cpuError=" + cpuDiscoveryError : ""),
      );
    }

    const publishEncoder = device.createCommandEncoder();
    const publishPass = publishEncoder.beginComputePass();
    publishPass.setPipeline(publishPipeline);
    publishPass.setBindGroup(0, publishGroup);
    publishPass.dispatchWorkgroups(1);
    publishPass.end();

    device.queue.submit([publishEncoder.finish()]);
    await device.queue.onSubmittedWorkDone();

    const residentWordLength = 8 + resident.residentCapacity * 2;
    const o = await readLookupWords(
      device,
      resident.residentBuffer,
      residentWordLength,
    );
    const committedCount = o[0] >>> 0;
    const reactionStartCount = o[1] >>> 0;
    const publishedHandle = o[2] >>> 0;
    const publishStatus = o[3] >>> 0;
    const groundedBundle = o[4] >>> 0;
    const capacity = o[5] >>> 0;
    if (capacity !== resident.residentCapacity ||
        reactionStartCount > committedCount ||
        committedCount > resident.residentCapacity) {
      throw new Error("WebGPU resident append metadata is invalid");
    }
    resident.residentAppendCount = committedCount;

    const appendCount = committedCount - reactionStartCount;
    const appendStarts = [];
    const appendEnds = [];
    for (let i = reactionStartCount; i < committedCount; i += 1) {
      appendStarts.push(o[8 + i * 2] >>> 0);
      appendEnds.push(o[9 + i * 2] >>> 0);
    }
    const committedBeforeStarts = [];
    const committedBeforeEnds = [];
    for (let i = 0; i < reactionStartCount; i += 1) {
      committedBeforeStarts.push(o[8 + i * 2] >>> 0);
      committedBeforeEnds.push(o[9 + i * 2] >>> 0);
    }
    if (publishStatus !== 1) {
      const error = new Error(
        publishStatus === 4
          ? "WebGPU resident append capacity exceeded"
          : "WebGPU PUBLISH failed closed with status " + publishStatus,
      );
      error.code = publishStatus === 4
        ? "RESIDENT_APPEND_CAPACITY_EXCEEDED"
        : "PUBLISH_FAILED";
      error.publishStatus = publishStatus;
      error.residentAppendCount = committedCount;
      error.residentCapacity = resident.residentCapacity;
      throw error;
    }

    const roleCount = d[8] >>> 0;
    const roleBindings = [];
    for (let i = 0; i < roleCount; i += 1) {
      roleBindings.push(Object.freeze({
        role: d[10 + i * 2] >>> 0,
        value: d[11 + i * 2] >>> 0,
      }));
    }
    const observed = Object.freeze({
      discoveryStatus: d[0] >>> 0,
      rawRuleMatches: d[1] >>> 0,
      currentHandle: d[2] >>> 0,
      interpreterHandle: d[3] >>> 0,
      theoryHandle: d[4] >>> 0,
      triggerKey: d[5] >>> 0,
      ruleHandle: d[6] >>> 0,
      admissionHandle: d[7] >>> 0,
      roleCount,
      roleBindings: Object.freeze(roleBindings),
      outputBundleTemplate: d[9] >>> 0,
      appendCount,
      appendStarts: Object.freeze(appendStarts),
      appendEnds: Object.freeze(appendEnds),
      residentAppendCount: committedCount,
      residentReactionStartCount: reactionStartCount,
      publishedHandle,
      publishStatus,
      groundedBundle,
    });

    // Oracle is materialized only after GPU DISCOVER + PUBLISH + readback.
    // It is comparison-only and is never re-uploaded into the GPU carrier.
    const oracleParsed = oracleParsedWithCommittedAppend(
      parsed,
      committedBeforeStarts,
      committedBeforeEnds,
    );
    const expected = expectedGpuCarrierReaction(
      oracleParsed,
      input,
      { maxAppend },
    );
    const sameBindings =
      observed.roleBindings.length === expected.roleBindings.length &&
      observed.roleBindings.every(
        (binding, i) =>
          binding.role === expected.roleBindings[i].role &&
          binding.value === expected.roleBindings[i].value,
      );
    const sameAppend =
      observed.appendCount === expected.appendCount &&
      observed.appendStarts.every(
        (value, i) => value === expected.appendStarts[i],
      ) &&
      observed.appendEnds.every(
        (value, i) => value === expected.appendEnds[i],
      );
    const mismatches = [];
    if (observed.discoveryStatus !== 1) mismatches.push("discoveryStatus");
    if (observed.publishStatus !== 1) mismatches.push("publishStatus");
    if (observed.rawRuleMatches !== expected.rawRuleMatches) mismatches.push("rawRuleMatches");
    if (observed.currentHandle !== expected.currentHandle) mismatches.push("currentHandle");
    if (observed.interpreterHandle !== expected.interpreterHandle) mismatches.push("interpreterHandle");
    if (observed.theoryHandle !== expected.theoryHandle) mismatches.push("theoryHandle");
    if (observed.triggerKey !== expected.triggerKey) mismatches.push("triggerKey");
    if (observed.ruleHandle !== expected.ruleHandle) mismatches.push("ruleHandle");
    if (observed.admissionHandle !== expected.admissionHandle) mismatches.push("admissionHandle");
    if (observed.outputBundleTemplate !== expected.outputBundleTemplate) mismatches.push("outputBundleTemplate");
    if (observed.groundedBundle !== expected.groundedBundle) mismatches.push("groundedBundle");
    if (observed.publishedHandle !== expected.candidateHandle) mismatches.push("publishedHandle");
    if (!sameBindings) mismatches.push("roleBindings");
    if (!sameAppend) mismatches.push("residentAppend");
    if (mismatches.length) {
      throw new Error(
        "WebGPU structural reaction diverged from post-readback CPU oracle: " +
        mismatches.join(", "),
      );
    }
    return Object.freeze({ plan, expected, observed, differential: true });
  } finally {
    discovery?.destroy();
    if (typeof device.popErrorScope === "function") {
      const validationError = await device.popErrorScope();
      if (validationError) {
        throw new Error(
          "WebGPU carrier reaction validation failed: " +
          validationError.message,
        );
      }
    }
  }
}

export async function runGpuCarrierReaction(
  device,
  parsed,
  input,
  { maxDiagnosticLinks = 16_384, maxAppend = 64 } = {},
) {
  const resident = await openGpuCarrierResidentSession(
    device,
    parsed,
    { maxDiagnosticLinks, maxResidentAppend: Math.max(64, maxAppend) },
  );
  try {
    return await resident.reaction(input, { maxAppend });
  } finally {
    resident.close();
  }
}
