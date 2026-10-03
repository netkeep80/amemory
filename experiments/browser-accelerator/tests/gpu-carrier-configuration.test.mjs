import assert from "node:assert/strict";

import {
  planGpuResidentConfiguration,
} from "../web/gpu-carrier.mjs";

const parsed = {
  layout: { linkCount: 3, rootHandle: 1 },
  sections: {
    // L1=ROOT, L2=START(ROOT), L3=END(ROOT)
    starts: new Uint32Array([1, 2, 1]),
    ends: new Uint32Array([1, 1, 3]),
  },
};

const H = (handle) => ({ handle });
const O = (operation) => ({ operation });

const recipe = {
  interpreterHandle: 3,
  operations: [
    { kind: "PAIR", start: H(2), end: H(3) },   // L4
    { kind: "START", child: O(0) },              // L5
    { kind: "PAIR", start: H(3), end: O(1) },    // L6
  ],
  initial: O(2),
};

const first = planGpuResidentConfiguration(
  parsed,
  { starts: [], ends: [] },
  recipe,
  { residentCapacity: 8 },
);
assert.equal(first.appendCount, 3);
assert.deepEqual(first.appendStarts, [2, 5, 3]);
assert.deepEqual(first.appendEnds, [3, 4, 5]);
assert.deepEqual(first.operationHandles, [4, 5, 6]);
assert.equal(first.initialHandle, 6);
assert.equal(first.reusedOperationCount, 0);

const second = planGpuResidentConfiguration(
  parsed,
  { starts: [...first.residentStarts], ends: [...first.residentEnds] },
  recipe,
  { residentCapacity: 8 },
);
assert.equal(second.appendCount, 0);
assert.deepEqual(second.operationHandles, [4, 5, 6]);
assert.equal(second.initialHandle, first.initialHandle);
assert.equal(second.reusedOperationCount, 3);

// Canonical categories must not collapse ordinary PAIR(ROOT,ROOT) into ROOT,
// nor START(ROOT)/END(ROOT) into ROOT.
const category = planGpuResidentConfiguration(
  parsed,
  { starts: [], ends: [] },
  {
    interpreterHandle: 3,
    operations: [
      { kind: "START", child: H(1) },
      { kind: "END", child: H(1) },
      { kind: "PAIR", start: H(1), end: H(1) },
    ],
    initial: O(2),
  },
  { residentCapacity: 8 },
);
assert.deepEqual(category.operationHandles.slice(0, 2), [2, 3]);
assert.equal(category.appendCount, 1);
assert.equal(category.initialHandle, 4);
assert.deepEqual(category.appendStarts, [1]);
assert.deepEqual(category.appendEnds, [1]);

assert.throws(
  () => planGpuResidentConfiguration(
    parsed,
    { starts: [], ends: [] },
    {
      interpreterHandle: 3,
      operations: [{ kind: "PAIR", start: H(99), end: H(1) }],
      initial: O(0),
    },
    { residentCapacity: 8 },
  ),
  /exceeds current resident topology/,
);

assert.throws(
  () => planGpuResidentConfiguration(
    parsed,
    { starts: [], ends: [] },
    {
      interpreterHandle: 3,
      operations: [{ kind: "START", child: O(0) }],
      initial: O(0),
    },
    { residentCapacity: 8 },
  ),
  /future configuration operation/,
);

assert.throws(
  () => planGpuResidentConfiguration(
    parsed,
    { starts: [], ends: [] },
    recipe,
    { residentCapacity: 2 },
  ),
  /capacity exceeded/,
);

// Planner failures are side-effect free by construction.
const stableStarts = [2];
const stableEnds = [3];
assert.throws(
  () => planGpuResidentConfiguration(
    parsed,
    { starts: stableStarts, ends: stableEnds },
    recipe,
    { residentCapacity: 1 },
  ),
  /capacity exceeded/,
);
assert.deepEqual(stableStarts, [2]);
assert.deepEqual(stableEnds, [3]);

console.log("GPU_RESIDENT_CONFIGURATION_PLAN=GREEN");
