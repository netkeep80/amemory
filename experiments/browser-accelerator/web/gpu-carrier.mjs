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
