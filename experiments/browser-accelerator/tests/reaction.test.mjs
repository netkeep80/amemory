import assert from "node:assert/strict";
import {
  R1_FIXTURE,
  R3_FIXTURE,
  assertR1Fixture,
  assertR2Fixture,
  assertR3Fixture,
  assertReactionStateExact,
  normalizeReactionState,
  pairAnum,
  perturbReactionState,
  splitPairAnum,
} from "../web/reaction.mjs";

assert.equal(assertR1Fixture(), true);
assert.equal(assertR2Fixture(), true);
assert.equal(assertR3Fixture(), true);

assert.deepEqual(splitPairAnum(R1_FIXTURE.current), {
  start: R1_FIXTURE.K,
  end: R1_FIXTURE.A,
});
assert.deepEqual(splitPairAnum(R1_FIXTURE.relation), {
  start: R1_FIXTURE.A,
  end: R1_FIXTURE.B,
});
assert.deepEqual(splitPairAnum(R1_FIXTURE.successor), {
  start: R1_FIXTURE.K,
  end: R1_FIXTURE.B,
});

assert.equal(pairAnum(R1_FIXTURE.K, R1_FIXTURE.A), R1_FIXTURE.current);
assert.equal(pairAnum(R1_FIXTURE.A, R1_FIXTURE.B), R1_FIXTURE.relation);
assert.equal(pairAnum(R1_FIXTURE.K, R1_FIXTURE.B), R1_FIXTURE.successor);
assert.equal(pairAnum(R1_FIXTURE.A, R3_FIXTURE.root), R3_FIXTURE.zeroRelation);
assert.equal(pairAnum(R1_FIXTURE.K, R3_FIXTURE.root), R3_FIXTURE.rootCurrent);
assert.equal(pairAnum(R3_FIXTURE.root, R1_FIXTURE.B), R3_FIXTURE.rootRelation);

assert.deepEqual(
  normalizeReactionState([R1_FIXTURE.successor, R1_FIXTURE.current]),
  [R1_FIXTURE.current, R1_FIXTURE.successor].sort(),
);
assert.equal(
  assertReactionStateExact([R1_FIXTURE.successor], [R1_FIXTURE.successor], "positive"),
  true,
);

assert.throws(
  () => splitPairAnum("98"),
  /must be PAIR Anum/,
);

assert.throws(
  () => assertReactionStateExact(
    [R1_FIXTURE.successor],
    [R1_FIXTURE.successor, R1_FIXTURE.successor],
    "duplicate-negative",
  ),
  /duplicate semantic member/,
);

assert.throws(
  () => assertReactionStateExact(
    [R1_FIXTURE.successor],
    perturbReactionState([R1_FIXTURE.successor]),
    "mismatch-negative",
  ),
  /mismatch/,
);

assert.throws(
  () => assertReactionStateExact(
    [R1_FIXTURE.successor],
    [R1_FIXTURE.current],
    "wrong-state-negative",
  ),
  /mismatch/,
);

console.log("AMEMORY_REACTION_R1_R2_R3_UNIT_WITNESSES=GREEN");
