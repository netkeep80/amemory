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

function instantiateCarrierTemplate(
  parsed,
  source,
  bindings,
  memo = new Map(),
  visiting = new Set(),
  depth = 0,
) {
  if (depth > 128) throw new Error("bounded carrier instantiation exceeded");
  if (bindings.has(source)) return bindings.get(source);
  if (memo.has(source)) return memo.get(source);
  if (visiting.has(source)) return 0;
  const root = parsed.layout.rootHandle;
  const poles = carrierPoles(parsed, source, "template source");
  let value = 0;
  if (poles.start === source && poles.end === source) {
    value = root;
  } else if (poles.start === source) {
    const child = instantiateCarrierTemplate(
      parsed, poles.end, bindings, memo, visiting, depth + 1,
    );
    value = child ? findCarrierStartSelf(parsed, child) : 0;
  } else if (poles.end === source) {
    const child = instantiateCarrierTemplate(
      parsed, poles.start, bindings, memo, visiting, depth + 1,
    );
    value = child ? findCarrierEndSelf(parsed, child) : 0;
  } else {
    visiting.add(source);
    const start = instantiateCarrierTemplate(
      parsed, poles.start, bindings, memo, visiting, depth + 1,
    );
    const end = instantiateCarrierTemplate(
      parsed, poles.end, bindings, memo, visiting, depth + 1,
    );
    visiting.delete(source);
    value = start && end ? findCarrierPair(parsed, start, end) : 0;
  }
  if (value) memo.set(source, value);
  return value;
}

function applyCarrierRule(parsed, rule, active) {
  const rulePoles = carrierPoles(parsed, rule, "rule");
  const roles = readCarrierRoleDictionary(parsed, rulePoles.start);
  const body = carrierPoles(parsed, rulePoles.end, "rule body");
  const bindings = unifyCarrierTemplate(parsed, body.start, active, roles);
  if (!bindings) return null;
  const outputs = readCarrierExactSequence(parsed, body.end, 2);
  if (outputs.length !== 1) return null;
  const candidate = instantiateCarrierTemplate(parsed, outputs[0], bindings);
  if (!candidate) return null;
  return Object.freeze({ candidate, rule, roles: roles.length });
}

function compactRootHandle(compactProof, role) {
  const root = compactProof?.roots?.find((entry) => entry.role === role);
  if (!root) throw new Error("compact proof root missing: " + role);
  return root.carrierRef >>> 0;
}

export function expectedGpuCarrierReaction(
  parsed,
  { currentHandle, interpreterHandle } = {},
) {
  requireLookupHandle(parsed, currentHandle, "current handle");
  requireLookupHandle(parsed, interpreterHandle, "interpreter handle");
  const interpreter = carrierPoles(parsed, interpreterHandle);
  const grammarTheory = carrierPoles(parsed, interpreter.end);
  const theory = grammarTheory.end;
  const current = carrierPoles(parsed, currentHandle);
  const endpoint = carrierPoles(parsed, current.end);
  const triggerKey = endpoint.start;
  const candidates = [];
  let rawRuleMatches = 0;
  let firstRule = 0;
  let firstAdmission = 0;
  for (const trigger of carrierIncidence(parsed, triggerKey, true)) {
    if (trigger === triggerKey) continue;
    const triggerPoles = carrierPoles(parsed, trigger);
    if (triggerPoles.start !== triggerKey) continue;
    const admission = triggerPoles.end;
    const admissionPoles = carrierPoles(parsed, admission);
    if (admissionPoles.start !== theory || admissionPoles.end === admission) {
      continue;
    }
    const applied = applyCarrierRule(parsed, admissionPoles.end, currentHandle);
    if (!applied) continue;
    rawRuleMatches += 1;
    if (!firstRule) {
      firstRule = applied.rule;
      firstAdmission = admission;
    }
    candidates.push(applied.candidate);
  }
  if (!rawRuleMatches || !candidates.length) {
    throw new Error("CPU carrier oracle found no applicable structural rule");
  }
  const candidateHandle = candidates[0];
  if (candidates.some((candidate) => candidate !== candidateHandle)) {
    throw new Error("bounded C4c3 witness has divergent rule outputs");
  }
  return Object.freeze({
    currentHandle,
    interpreterHandle,
    theoryHandle: theory,
    triggerKey,
    admissionHandle: firstAdmission,
    ruleHandle: firstRule,
    rawRuleMatches,
    candidateHandle,
    logicalFingerprint: gpuCarrierLogicalFingerprint(parsed),
  });
}

export function deriveGpuCarrierReactionInput(parsed, compactProof) {
  if (!compactProof || compactProof.block !== "MUX1") {
    throw new Error("C4c3 requires the real MUX1 compact proof");
  }
  const currentHandle = compactRootHandle(compactProof, "scope.initial");
  const interpreterHandle = compactRootHandle(
    compactProof,
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
    "fn valid(h: u32) -> bool { return h > 0u && h <= LINK_COUNT; }",
    "fn find_pair(a: u32, b: u32) -> u32 {",
    "  var h = start_head(a); var n = 0u;",
    "  loop { if (h == 0u) { break; } if (!valid(h) || n >= LINK_COUNT) { return 0u; } if (s(h) == a && e(h) == b) { return h; } h = next_start(h); n = n + 1u; }",
    "  return 0u;",
    "}",
    "fn find_start_self(child: u32) -> u32 {",
    "  var h = end_head(child); var n = 0u;",
    "  loop { if (h == 0u) { break; } if (!valid(h) || n >= LINK_COUNT) { return 0u; } if (s(h) == h && e(h) == child) { return h; } h = next_end(h); n = n + 1u; }",
    "  return 0u;",
    "}",
    "fn find_end_self(child: u32) -> u32 {",
    "  var h = start_head(child); var n = 0u;",
    "  loop { if (h == 0u) { break; } if (!valid(h) || n >= LINK_COUNT) { return 0u; } if (s(h) == child && e(h) == h) { return h; } h = next_start(h); n = n + 1u; }",
    "  return 0u;",
    "}",
    "fn apply_rule(rule: u32, active: u32) -> u32 {",
    "  if (!valid(rule) || !valid(active)) { return 0u; }",
    "  let dictionary = s(rule); let body = e(rule);",
    "  if (!valid(dictionary) || !valid(body) || s(dictionary) != dictionary || e(dictionary) == dictionary) { return 0u; }",
    "  var roles: array<u32, 16>; var bound: array<u32, 16>; var bound_set: array<u32, 16>;",
    "  var role_count = 0u; var seq = e(dictionary); var seq_guard = 0u;",
    "  loop {",
    "    if (seq == ROOT_HANDLE) { break; }",
    "    if (!valid(seq) || seq_guard >= 64u || s(seq) != seq || e(seq) == seq || role_count >= MAX_ROLES) { return 0u; }",
    "    let cell = e(seq); if (!valid(cell)) { return 0u; }",
    "    let role = e(cell); var duplicate = false; var ri = 0u;",
    "    loop { if (ri >= role_count) { break; } if (roles[ri] == role) { duplicate = true; break; } ri = ri + 1u; }",
    "    if (duplicate) { return 0u; } roles[role_count] = role; role_count = role_count + 1u; seq = s(cell); seq_guard = seq_guard + 1u;",
    "  }",
    "  let before = s(body); let bundle = e(body);",
    "  var ts: array<u32, 128>; var cs: array<u32, 128>; var sp = 1u; var visited = 0u; ts[0] = before; cs[0] = active;",
    "  loop {",
    "    if (sp == 0u) { break; } if (visited >= MAX_STACK) { return 0u; }",
    "    sp = sp - 1u; let t = ts[sp]; let c = cs[sp]; visited = visited + 1u; if (!valid(t) || !valid(c)) { return 0u; }",
    "    var role_index = 0xffffffffu; var r = 0u; loop { if (r >= role_count) { break; } if (roles[r] == t) { role_index = r; break; } r = r + 1u; }",
    "    if (role_index != 0xffffffffu) { if (bound_set[role_index] != 0u && bound[role_index] != c) { return 0u; } bound[role_index] = c; bound_set[role_index] = 1u; continue; }",
    "    let ta = s(t); let tb = e(t); let ca = s(c); let cb = e(c);",
    "    if ((ta == t) != (ca == c) || (tb == t) != (cb == c)) { return 0u; }",
    "    if (tb != t) { if (sp >= MAX_STACK) { return 0u; } ts[sp] = tb; cs[sp] = cb; sp = sp + 1u; }",
    "    if (ta != t) { if (sp >= MAX_STACK) { return 0u; } ts[sp] = ta; cs[sp] = ca; sp = sp + 1u; }",
    "  }",
    "  var rr = 0u; loop { if (rr >= role_count) { break; } if (bound_set[rr] == 0u) { return 0u; } rr = rr + 1u; }",
    "  if (bundle == ROOT_HANDLE || !valid(bundle) || s(bundle) != bundle || e(bundle) == bundle) { return 0u; }",
    "  let bundle_cell = e(bundle); if (!valid(bundle_cell) || s(bundle_cell) != ROOT_HANDLE) { return 0u; } let output_template = e(bundle_cell);",
    "  var nodes: array<u32, 128>; var states: array<u32, 128>; var isp = 1u;",
    "  var memo_node: array<u32, 128>; var memo_value: array<u32, 128>; var memo_count = 0u;",
    "  nodes[0] = output_template; states[0] = 0u; var inst_guard = 0u;",
    "  loop {",
    "    if (isp == 0u) { break; } if (inst_guard >= 512u) { return 0u; } inst_guard = inst_guard + 1u;",
    "    let ix = isp - 1u; let node = nodes[ix]; var existing = 0u; var mi = 0u;",
    "    loop { if (mi >= memo_count) { break; } if (memo_node[mi] == node) { existing = memo_value[mi]; break; } mi = mi + 1u; }",
    "    if (existing != 0u) { isp = isp - 1u; continue; }",
    "    var role_value = 0u; var bri = 0u; loop { if (bri >= role_count) { break; } if (roles[bri] == node) { role_value = bound[bri]; break; } bri = bri + 1u; }",
    "    if (role_value != 0u) { if (memo_count >= MAX_STACK) { return 0u; } memo_node[memo_count] = node; memo_value[memo_count] = role_value; memo_count = memo_count + 1u; isp = isp - 1u; continue; }",
    "    if (!valid(node)) { return 0u; } let na = s(node); let nb = e(node);",
    "    if (states[ix] == 0u) {",
    "      states[ix] = 1u;",
    "      if (nb != node) { if (isp >= MAX_STACK) { return 0u; } nodes[isp] = nb; states[isp] = 0u; isp = isp + 1u; }",
    "      if (na != node) { if (isp >= MAX_STACK) { return 0u; } nodes[isp] = na; states[isp] = 0u; isp = isp + 1u; } continue;",
    "    }",
    "    var av = node; var bv = node;",
    "    if (na != node) { av = 0u; var ai = 0u; loop { if (ai >= memo_count) { break; } if (memo_node[ai] == na) { av = memo_value[ai]; break; } ai = ai + 1u; } if (av == 0u) { return 0u; } }",
    "    if (nb != node) { bv = 0u; var bi = 0u; loop { if (bi >= memo_count) { break; } if (memo_node[bi] == nb) { bv = memo_value[bi]; break; } bi = bi + 1u; } if (bv == 0u) { return 0u; } }",
    "    var value = 0u; if (na == node && nb == node) { value = ROOT_HANDLE; } else if (na == node) { value = find_start_self(bv); } else if (nb == node) { value = find_end_self(av); } else { value = find_pair(av, bv); }",
    "    if (value == 0u || memo_count >= MAX_STACK) { return 0u; } memo_node[memo_count] = node; memo_value[memo_count] = value; memo_count = memo_count + 1u; isp = isp - 1u;",
    "  }",
    "  var result = 0u; var oi = 0u; loop { if (oi >= memo_count) { break; } if (memo_node[oi] == output_template) { result = memo_value[oi]; break; } oi = oi + 1u; } return result;",
    "}",
    "@compute @workgroup_size(1)",
    "fn discover(@builtin(global_invocation_id) id: vec3<u32>) {",
    "  if (id.x != 0u) { return; } var z = 0u; loop { if (z >= 10u) { break; } discovery[z] = 0u; z = z + 1u; }",
    "  if (!valid(CURRENT) || !valid(INTERPRETER)) { discovery[0] = 2u; return; }",
    "  let grammar_theory = e(INTERPRETER); if (!valid(grammar_theory)) { discovery[0] = 3u; return; } let theory = e(grammar_theory); let endpoint = e(CURRENT);",
    "  if (!valid(theory) || !valid(endpoint)) { discovery[0] = 4u; return; } let trigger_key = s(endpoint); if (!valid(trigger_key)) { discovery[0] = 5u; return; }",
    "  var trigger = start_head(trigger_key); var guard = 0u; var matches = 0u; var candidate = 0u; var matched_rule = 0u; var matched_admission = 0u;",
    "  loop {",
    "    if (trigger == 0u) { break; } if (!valid(trigger) || guard >= LINK_COUNT) { discovery[0] = 6u; return; }",
    "    if (trigger != trigger_key && s(trigger) == trigger_key) {",
    "      let admission = e(trigger);",
    "      if (valid(admission) && s(admission) == theory && e(admission) != admission) { let rule = e(admission); let out = apply_rule(rule, CURRENT); if (out != 0u) { matches = matches + 1u; if (candidate == 0u) { candidate = out; matched_rule = rule; matched_admission = admission; } else if (candidate != out) { discovery[0] = 7u; return; } } }",
    "    }",
    "    trigger = next_start(trigger); guard = guard + 1u;",
    "  }",
    "  if (matches == 0u || candidate == 0u) { discovery[0] = 8u; return; }",
    "  discovery[1] = candidate; discovery[2] = matches; discovery[3] = CURRENT; discovery[4] = INTERPRETER; discovery[5] = theory; discovery[6] = trigger_key; discovery[7] = matched_rule; discovery[8] = matched_admission; discovery[0] = 1u;",
    "}",
    "@compute @workgroup_size(1)",
    "fn publish(@builtin(global_invocation_id) id: vec3<u32>) { if (id.x != 0u) { return; } published[0] = 0u; published[1] = 0u; if (discovery[0] != 1u || discovery[1] == 0u) { return; } published[1] = discovery[1]; published[0] = 1u; }",
  ].join("\n");
}

function singleReactionShader(args) {
  return [
    "@group(0) @binding(0) var<storage, read> carrier: array<u32>;",
    "@group(0) @binding(1) var<storage, read_write> discovery: array<u32>;",
    "@group(0) @binding(2) var<storage, read_write> published: array<u32>;",
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
    "@group(0) @binding(7) var<storage, read_write> published: array<u32>;",
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
  { currentHandle, interpreterHandle, linkCount, rootHandle = 1 } = {},
) {
  checkedInteger(linkCount, "linkCount", { min: 1 });
  checkedInteger(currentHandle, "currentHandle", { min: 1 });
  checkedInteger(interpreterHandle, "interpreterHandle", { min: 1 });
  checkedInteger(rootHandle, "rootHandle", { min: 1 });
  const args = { currentHandle, interpreterHandle, linkCount, rootHandle };
  if (mode === "single") return singleReactionShader(args);
  if (mode === "sections") return sectionReactionShader(args);
  throw new Error("unsupported GPU carrier reaction mode: " + mode);
}

export async function runGpuCarrierReaction(
  device,
  parsed,
  input,
  { maxDiagnosticLinks = 16_384 } = {},
) {
  if (!device?.createBuffer || !device?.queue) {
    throw new TypeError("WebGPU device is required");
  }
  checkedInteger(maxDiagnosticLinks, "maxDiagnosticLinks", { min: 1 });
  if (parsed.layout.linkCount > maxDiagnosticLinks) {
    throw new RangeError("bounded GPU reaction carrier too large");
  }
  requireLookupHandle(parsed, input?.currentHandle, "current handle");
  requireLookupHandle(parsed, input?.interpreterHandle, "interpreter handle");
  const plan = planGpuCarrierUpload(parsed.layout, device.limits);
  if (plan.mode === "unsupported") {
    throw new Error("WebGPU carrier upload unsupported: " + plan.reason);
  }
  if (plan.storageBufferCount + 1 >
      Number(device.limits.maxStorageBuffersPerShaderStage)) {
    throw new Error("WebGPU carrier reaction needs one discovery binding");
  }

  const usage = gpuLookupUsage();
  const inputUsage = usage.STORAGE | usage.COPY_DST;
  const outputUsage = usage.STORAGE | usage.COPY_SRC | usage.COPY_DST;
  const inputs = [];
  const discovery = createLookupBuffer(device, new Uint32Array(10), outputUsage);
  const published = createLookupBuffer(device, new Uint32Array(2), outputUsage);
  device.pushErrorScope?.("validation");
  try {
    let discoverEntries;
    if (plan.mode === "single") {
      const carrier = createLookupBuffer(device, parsed.words, inputUsage);
      inputs.push(carrier);
      discoverEntries = [
        { binding: 0, resource: { buffer: carrier } },
        { binding: 1, resource: { buffer: discovery } },
      ];
    } else {
      discoverEntries = plan.buffers.map((part) => {
        const buffer = createLookupBuffer(
          device,
          parsed.sections[part.name],
          inputUsage,
        );
        inputs.push(buffer);
        return { binding: part.binding, resource: { buffer } };
      });
      discoverEntries.push({ binding: 6, resource: { buffer: discovery } });
    }

    const shader = device.createShaderModule({
      code: gpuCarrierReactionShaderSource(plan.mode, {
        currentHandle: input.currentHandle,
        interpreterHandle: input.interpreterHandle,
        linkCount: parsed.layout.linkCount,
        rootHandle: parsed.layout.rootHandle,
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
    const discoverGroup = device.createBindGroup({
      layout: discoverPipeline.getBindGroupLayout(0),
      entries: discoverEntries,
    });
    const discoveryBinding = plan.mode === "single" ? 1 : 6;
    const publishedBinding = plan.mode === "single" ? 2 : 7;
    const publishGroup = device.createBindGroup({
      layout: publishPipeline.getBindGroupLayout(0),
      entries: [
        { binding: discoveryBinding, resource: { buffer: discovery } },
        { binding: publishedBinding, resource: { buffer: published } },
      ],
    });

    const encoder = device.createCommandEncoder();
    const discoverPass = encoder.beginComputePass();
    discoverPass.setPipeline(discoverPipeline);
    discoverPass.setBindGroup(0, discoverGroup);
    discoverPass.dispatchWorkgroups(1);
    discoverPass.end();
    const publishPass = encoder.beginComputePass();
    publishPass.setPipeline(publishPipeline);
    publishPass.setBindGroup(0, publishGroup);
    publishPass.dispatchWorkgroups(1);
    publishPass.end();
    device.queue.submit([encoder.finish()]);
    await device.queue.onSubmittedWorkDone();

    const [d, p] = await Promise.all([
      readLookupWords(device, discovery, 10),
      readLookupWords(device, published, 2),
    ]);
    const observed = Object.freeze({
      discoveryStatus: d[0] >>> 0,
      candidateHandle: d[1] >>> 0,
      rawRuleMatches: d[2] >>> 0,
      currentHandle: d[3] >>> 0,
      interpreterHandle: d[4] >>> 0,
      theoryHandle: d[5] >>> 0,
      triggerKey: d[6] >>> 0,
      ruleHandle: d[7] >>> 0,
      admissionHandle: d[8] >>> 0,
      publishStatus: p[0] >>> 0,
      publishedHandle: p[1] >>> 0,
    });

    // Independent oracle is intentionally computed only after GPU readback.
    // It cannot seed shader constants, candidate discovery, or publication.
    const expected = expectedGpuCarrierReaction(parsed, input);
    if (observed.discoveryStatus !== 1 ||
        observed.publishStatus !== 1 ||
        observed.candidateHandle !== expected.candidateHandle ||
        observed.publishedHandle !== expected.candidateHandle ||
        observed.rawRuleMatches !== expected.rawRuleMatches ||
        observed.currentHandle !== expected.currentHandle ||
        observed.interpreterHandle !== expected.interpreterHandle ||
        observed.theoryHandle !== expected.theoryHandle ||
        observed.triggerKey !== expected.triggerKey) {
      throw new Error(
        "WebGPU structural reaction diverged from CPU carrier oracle",
      );
    }
    return Object.freeze({ plan, expected, observed, differential: true });
  } finally {
    for (const buffer of inputs) buffer.destroy();
    discovery.destroy();
    published.destroy();
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
