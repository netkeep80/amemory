import assert from "node:assert/strict";
import { assertExactU32, perturbFirst } from "../web/differential.mjs";

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
