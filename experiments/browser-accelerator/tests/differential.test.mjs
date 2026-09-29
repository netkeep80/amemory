import assert from "node:assert/strict";
import { assertExactU32, perturbFirst } from "../web/differential.mjs";
import {
  PACKED_GPU_CARRIER_HEADER_WORDS,
  PACKED_GPU_CARRIER_MAGIC,
  gpuCarrierLayoutForLinkCount,
  parseGpuCarrierWords,
  planGpuCarrierUpload,
  planGpuCarrierWordsUpload,
} from "../web/gpu-carrier.mjs";

const expected = new Uint32Array([7, 1, 2, 0, 7]);
const same = new Uint32Array([7, 1, 2, 0, 7]);

assert.equal(assertExactU32(expected, same, "positive"), true);

assert.throws(
  () => assertExactU32(expected, new Uint32Array([7, 1, 2]), "length-negative"),
  /length mismatch/,
);

assert.throws(
  () => assertExactU32(expected, new Uint32Array([7, 1, 3, 0, 7]), "value-negative"),
  /mismatch at index 2/,
);

const corrupted = perturbFirst(same);
assert.throws(
  () => assertExactU32(expected, corrupted, "deliberate-negative-control"),
  /mismatch at index 0/,
);

console.log("CPU_GPU_DIFFERENTIAL_CHECKER_NEGATIVE_CONTROL=GREEN");


function syntheticGpuCarrier(linkCount) {
  const layout = gpuCarrierLayoutForLinkCount(linkCount);
  const words = new Uint32Array(layout.totalWords);
  words[0] = PACKED_GPU_CARRIER_MAGIC;
  words[1] = 1;
  words[2] = 1;
  words[3] = 1;
  words[4] = linkCount;
  words[5] = 1;
  words[6] = layout.startsOffset;
  words[7] = layout.endsOffset;
  words[8] = layout.startHeadOffset;
  words[9] = layout.endHeadOffset;
  words[10] = layout.nextByStartOffset;
  words[11] = layout.nextByEndOffset;
  words[12] = layout.totalWords;
  return words;
}

const carrier3 = syntheticGpuCarrier(3);
const parsed3 = parseGpuCarrierWords(carrier3);
assert.equal(parsed3.layout.totalWords, 38);
assert.equal(parsed3.layout.totalBytes, 152);
assert.equal(parsed3.sections.starts.length, 3);
assert.equal(parsed3.sections.ends.length, 3);
assert.equal(parsed3.sections.startHead.length, 4);
assert.equal(parsed3.sections.endHead.length, 4);
assert.equal(parsed3.sections.nextByStart.length, 4);
assert.equal(parsed3.sections.nextByEnd.length, 4);
assert.equal(
  parsed3.sections.starts.buffer,
  carrier3.buffer,
  "section parsing must remain zero-copy",
);

const smallLimits = {
  maxBufferSize: 1 << 20,
  maxStorageBufferBindingSize: 1 << 20,
  maxStorageBuffersPerShaderStage: 8,
};
const singlePlan = planGpuCarrierWordsUpload(carrier3, smallLimits);
assert.equal(singlePlan.mode, "single");
assert.equal(singlePlan.storageBufferCount, 1);
assert.equal(singlePlan.buffers[0].byteLength, carrier3.byteLength);

const tenMillion = gpuCarrierLayoutForLinkCount(10_000_001);
assert.equal(tenMillion.totalWords, 60_000_026);
assert.equal(tenMillion.totalBytes, 240_000_104);

const sectionPlan = planGpuCarrierUpload(tenMillion, {
  maxBufferSize: 268_435_456,
  maxStorageBufferBindingSize: 134_217_728,
  maxStorageBuffersPerShaderStage: 8,
});
assert.equal(sectionPlan.mode, "sections");
assert.equal(sectionPlan.storageBufferCount, 6);
assert.deepEqual(
  sectionPlan.buffers.map((buffer) => buffer.name),
  [
    "starts",
    "ends",
    "startHead",
    "endHead",
    "nextByStart",
    "nextByEnd",
  ],
);
assert.ok(
  sectionPlan.buffers.every(
    (buffer) => buffer.byteLength <= 134_217_728,
  ),
);

const bindingCountFailure = planGpuCarrierUpload(tenMillion, {
  maxBufferSize: 268_435_456,
  maxStorageBufferBindingSize: 134_217_728,
  maxStorageBuffersPerShaderStage: 4,
});
assert.equal(bindingCountFailure.mode, "unsupported");
assert.equal(
  bindingCountFailure.reason,
  "storage-buffer-binding-count",
);

const hugeLayout = gpuCarrierLayoutForLinkCount(40_000_000);
const sectionSizeFailure = planGpuCarrierUpload(hugeLayout, {
  maxBufferSize: 268_435_456,
  maxStorageBufferBindingSize: 134_217_728,
  maxStorageBuffersPerShaderStage: 8,
});
assert.equal(sectionSizeFailure.mode, "unsupported");
assert.equal(
  sectionSizeFailure.reason,
  "section-exceeds-storage-buffer-limit",
);

for (const [label, mutate, pattern] of [
  [
    "magic",
    (words) => {
      words[0] ^= 1;
    },
    /magic mismatch/,
  ],
  [
    "schema",
    (words) => {
      words[1] += 1;
    },
    /ABI schema mismatch/,
  ],
  [
    "offset",
    (words) => {
      words[7] += 1;
    },
    /ends offset mismatch/,
  ],
  [
    "reserved",
    (words) => {
      words[15] = 1;
    },
    /reserved\[1\] mismatch/,
  ],
]) {
  const broken = carrier3.slice();
  mutate(broken);
  assert.throws(
    () => parseGpuCarrierWords(broken),
    pattern,
    `corrupted GPU carrier ${label} must fail closed`,
  );
}

assert.throws(
  () =>
    parseGpuCarrierWords(
      carrier3.subarray(0, PACKED_GPU_CARRIER_HEADER_WORDS - 1),
    ),
  /too short/,
);

console.log("PACKED_GPU_CARRIER_BROWSER_PLAN=GREEN");
