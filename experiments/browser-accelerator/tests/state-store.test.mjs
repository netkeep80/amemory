import assert from "node:assert/strict";
import {
  STATE_CELLS,
  applyAuthorizedModel,
  assertStateExact,
  assertStatePreserved,
  matrixToPairs,
  normalizeStatePairs,
  validatePhysicalPair,
} from "../web/state-store.mjs";

const round0 = [[1,2]];
const round1Commits = [[4,5],[1,3],[4,5]];
const round1 = applyAuthorizedModel(round0, round1Commits);
assert.deepEqual(normalizeStatePairs(round1), ["1:2","1:3","4:5"]);

const round2 = applyAuthorizedModel(round1, [[1,3],[9,9]]);
assert.deepEqual(normalizeStatePairs(round2), ["1:2","1:3","4:5","9:9"]);
assertStatePreserved(round1, round2);

// Duplicate/replay idempotence.
const replayed = applyAuthorizedModel(round2, [[1,3],[1,3],[9,9]]);
assertStateExact(round2, replayed, "idempotent replay");

// Request order does not affect normalized portable state.
const reordered = applyAuthorizedModel(round0, [[1,3],[4,5],[4,5]]);
assert.deepEqual(normalizeStatePairs(reordered), normalizeStatePairs(round1));

// Out-of-scope physical pairs fail closed in the model.
assert.equal(validatePhysicalPair([15,15]), true);
assert.equal(validatePhysicalPair([16,1]), false);
assert.equal(validatePhysicalPair([1,16]), false);
const rangeRejected = applyAuthorizedModel(round0, [[16,1],[1,16]]);
assertStateExact(round0, rangeRejected, "range rejected");

// State-loss negative: recreating from only round-2 commits is detectable.
const lost = applyAuthorizedModel([], [[1,3],[9,9]]);
assert.throws(
  () => assertStatePreserved(round1, lost, "lost-state-negative"),
  /prior pair disappeared/,
);
assert.throws(
  () => assertStateExact(round2, lost, "lost-state-exact-negative"),
  /mismatch|pair-count/,
);

// Unauthorized/non-requested pair must fail exact observation.
assert.throws(
  () => assertStateExact(round2, [...round2,[7,7]], "unauthorized-negative"),
  /pair-count|mismatch/,
);

// Matrix projection produces pair observation, not physical cell identity.
const matrix = new Uint32Array(STATE_CELLS);
matrix[1 * 16 + 2] = 1;
matrix[4 * 16 + 5] = 1;
assert.deepEqual(normalizeStatePairs(matrixToPairs(matrix)), ["1:2","4:5"]);

console.log("CPU_GPU_STATEFUL_STORE_NEGATIVE_WITNESSES=GREEN");
