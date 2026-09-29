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
