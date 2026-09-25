import assert from "node:assert/strict";
import {
  assertCanonicalNewPairs,
  newPairsFromFlags,
  normalizePairSet,
} from "../web/canonicalization.mjs";

const existing = [[1, 2], [9, 9]];
const candidates = [[1, 2], [4, 5], [4, 5], [1, 3], [9, 9], [1, 3]];
const flags = new Uint32Array([1, 4, 2, 4, 1, 2]);
const expectedNew = [[1, 3], [4, 5]];

const actualNew = newPairsFromFlags(candidates, flags);
assert.equal(assertCanonicalNewPairs(expectedNew, actualNew, "positive"), true);

// Existing pairs must never enter the new commit set.
for (const pair of actualNew) {
  assert.equal(
    normalizePairSet(existing).includes(normalizePairSet([pair])[0]),
    false,
    "existing pair leaked into new commit set",
  );
}

// Negative: duplicate in canonical output must be rejected.
assert.throws(
  () => assertCanonicalNewPairs(expectedNew, [[1, 3], [4, 5], [4, 5]], "duplicate-negative"),
  /duplicate pair present/,
);

// Negative: missing/new wrong pair must be rejected.
assert.throws(
  () => assertCanonicalNewPairs(expectedNew, [[1, 3], [7, 8]], "wrong-pair-negative"),
  /mismatch/,
);

// Reordering physical candidate occurrences must preserve normalized semantic new-pair set.
const reorderedCandidates = [[1, 3], [9, 9], [4, 5], [1, 2], [1, 3], [4, 5]];
const reorderedFlags = new Uint32Array([4, 1, 4, 1, 2, 2]);
const reorderedNew = newPairsFromFlags(reorderedCandidates, reorderedFlags);
assert.deepEqual(normalizePairSet(actualNew), normalizePairSet(reorderedNew));

// Representative positions differ across the two batches; semantic comparison ignores them.
assert.notDeepEqual(
  Array.from(flags).map((f, i) => (f & 4 ? i : -1)).filter((i) => i >= 0),
  Array.from(reorderedFlags).map((f, i) => (f & 4 ? i : -1)).filter((i) => i >= 0),
);
assert.deepEqual(normalizePairSet(actualNew), normalizePairSet(reorderedNew));

console.log("CPU_GPU_CANONICALIZATION_NEGATIVE_WITNESSES=GREEN");
