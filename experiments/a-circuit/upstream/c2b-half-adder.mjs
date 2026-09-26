import { materializeExactSequence, readExactSequence } from "../../../_upstream/anum_docs/ts/dist/src/exact-sequence.js";
import { Memory, ensureRootBasis } from "../../../_upstream/anum_docs/ts/dist/src/memory.js";
import {
  admitStructuralRule,
  defineStructuralInterpreter,
  defineStructuralRoleDictionary,
  defineStructuralRule,
} from "../../../_upstream/anum_docs/ts/dist/src/structural-rule.js";
import {
  V013CurrentScopeCursor,
  defineV013WorkingScope,
  reactV013StructuralScope,
} from "../../../_upstream/anum_docs/ts/dist/src/v013-structural-execution.js";

const ACCEPTED_MTS_V013_SHA = "3819b5fb2bae65506c2888e7c4fa0de387551846";

function assert(condition, message) {
  if (!condition) throw new Error("A-Circuit C2b: " + message);
}

function same(actual, expected, message) {
  assert(
    Object.is(actual, expected),
    message + ": expected " + String(expected) + ", got " + String(actual),
  );
}

function sameMembers(actual, expected, message) {
  same(actual.length, expected.length, message + ": cardinality");
  for (const member of expected) {
    assert(actual.includes(member), message + ": missing member");
  }
}

function call(memory, b, fn, arg) {
  return memory.ensure(b.O, memory.ensure(fn, arg));
}

function done(memory, b, value) {
  return memory.ensure(b.C, value);
}

function xorFrame(memory, caller, args) {
  return memory.ensureStartSelfClosed(memory.ensure(caller, args));
}

function andFrame(memory, caller, args, sum) {
  return memory.ensureStartSelfClosed(
    memory.ensure(caller, memory.ensure(args, sum)),
  );
}

function finishFrame(memory, caller, sum) {
  return memory.ensureStartSelfClosed(memory.ensure(caller, sum));
}

function admitBundleRule(memory, theory, triggerKey, roles, before, after) {
  const dictionary = defineStructuralRoleDictionary(memory, roles);
  const outputBundle = materializeExactSequence(memory, after);
  const rule = defineStructuralRule(
    memory,
    dictionary,
    memory.ensure(before, outputBundle),
  );
  const admission = admitStructuralRule(memory, theory, rule);

  // v0.13 structural execution discovers admitted Rules through the
  // trigger-key projection. This is an index, not program semantic authority.
  memory.ensure(triggerKey, admission);
  return rule;
}

function freshAnchors(memory, b, count) {
  let seed = memory.ensure(b.U, b.L);
  const result = [];
  for (let i = 0; i < count; i += 1) {
    seed = memory.ensure(seed, i % 2 === 0 ? b.O : b.C);
    result.push(seed);
  }
  return result;
}

function buildFixture() {
  const memory = new Memory();
  const b = ensureRootBasis(memory);
  const fresh = freshAnchors(memory, b, 220);
  const at = (i) => {
    const value = fresh[i];
    assert(value !== undefined, "fresh anchor " + i);
    return value;
  };

  const theory = memory.ensure(at(0), at(1));
  const authorityDictionary = defineStructuralRoleDictionary(memory, []);
  const grammar = memory.ensure(at(2), at(3));
  const interpreter =
    defineStructuralInterpreter(memory, authorityDictionary, grammar, theory);

  // Program/gate identities are ordinary Links. The execution kernel does not
  // know their names or semantics.
  const HALF = memory.ensure(at(4), at(5));
  const XOR = memory.ensure(at(6), at(7));
  const AND = memory.ensure(at(8), at(9));
  const K = memory.ensure(at(10), at(11));

  const ZERO = b.U;
  const ONE = b.L;

  const args = new Map();
  for (const a of [ZERO, ONE]) {
    for (const c of [ZERO, ONE]) {
      args.set(
        a + ":" + c,
        materializeExactSequence(memory, [a, c]),
      );
    }
  }

  // -----------------------------------------------------------------------
  // Generic OPEN:
  //
  //   K -> Call(HALF,args)
  //     =>
  //   XorFrame(K,args) -> Call(XOR,args)
  //
  // This contains no truth-table row.
  // -----------------------------------------------------------------------
  {
    const kRole = at(20);
    const argsRole = at(21);
    const before = memory.ensure(
      kRole,
      call(memory, b, HALF, argsRole),
    );
    const after = memory.ensure(
      xorFrame(memory, kRole, argsRole),
      call(memory, b, XOR, argsRole),
    );
    admitBundleRule(
      memory,
      theory,
      b.O,
      [kRole, argsRole],
      before,
      [after],
    );
  }

  // Standard XOR gate definition only.
  const xorRows = [
    [ZERO, ZERO, ZERO],
    [ZERO, ONE, ONE],
    [ONE, ZERO, ONE],
    [ONE, ONE, ZERO],
  ];

  let ruleSeed = 30;
  for (const [a, c, sum] of xorRows) {
    const argumentSequence = args.get(a + ":" + c);
    assert(argumentSequence !== undefined, "XOR argument sequence");

    const kRole = at(ruleSeed++);
    const before = memory.ensure(
      xorFrame(memory, kRole, argumentSequence),
      call(memory, b, XOR, argumentSequence),
    );
    const after = memory.ensure(
      andFrame(memory, kRole, argumentSequence, sum),
      call(memory, b, AND, argumentSequence),
    );
    admitBundleRule(
      memory,
      theory,
      b.O,
      [kRole],
      before,
      [after],
    );
  }

  // Standard AND gate definition only.
  //
  // Notice that 'sumRole' is NOT known by the AND truth table. It is captured
  // structurally from the state produced by XOR and carried through unchanged.
  const andRows = [
    [ZERO, ZERO, ZERO],
    [ZERO, ONE, ZERO],
    [ONE, ZERO, ZERO],
    [ONE, ONE, ONE],
  ];

  for (const [a, c, carry] of andRows) {
    const argumentSequence = args.get(a + ":" + c);
    assert(argumentSequence !== undefined, "AND argument sequence");

    const kRole = at(ruleSeed++);
    const sumRole = at(ruleSeed++);
    const before = memory.ensure(
      andFrame(memory, kRole, argumentSequence, sumRole),
      call(memory, b, AND, argumentSequence),
    );
    const after = memory.ensure(
      finishFrame(memory, kRole, sumRole),
      done(memory, b, carry),
    );
    admitBundleRule(
      memory,
      theory,
      b.O,
      [kRole, sumRole],
      before,
      [after],
    );
  }

  // -----------------------------------------------------------------------
  // Generic FINALIZE:
  //
  //   FinishFrame(K,sum) -> Done(carry)
  //     =>
  //   K -> ExactSequence_R([sum,carry])
  //
  // Both runtime values are role bindings. Host code does not read them and
  // does not construct the runtime result sequence.
  // -----------------------------------------------------------------------
  {
    const kRole = at(80);
    const sumRole = at(81);
    const carryRole = at(82);

    const before = memory.ensure(
      finishFrame(memory, kRole, sumRole),
      done(memory, b, carryRole),
    );
    const resultTemplate =
      materializeExactSequence(memory, [sumRole, carryRole]);
    const after = memory.ensure(kRole, resultTemplate);

    admitBundleRule(
      memory,
      theory,
      b.C,
      [kRole, sumRole, carryRole],
      before,
      [after],
    );
  }

  return Object.freeze({
    memory,
    b,
    theory,
    interpreter,
    HALF,
    XOR,
    AND,
    K,
    ZERO,
    ONE,
    args,
    fresh,
  });
}

function expectedBits(a, b, ZERO, ONE) {
  const av = a === ONE;
  const bv = b === ONE;
  return Object.freeze({
    sum: av !== bv ? ONE : ZERO,
    carry: av && bv ? ONE : ZERO,
  });
}

function runCase(f, a, b, seedBase) {
  const {
    memory,
    interpreter,
    HALF,
    XOR,
    AND,
    K,
    ZERO,
    ONE,
    args,
    fresh,
  } = f;

  const argumentSequence = args.get(a + ":" + b);
  assert(argumentSequence !== undefined, "case argument sequence");

  const initial = memory.ensure(
    K,
    call(memory, f.b, HALF, argumentSequence),
  );

  const scopeSeed = fresh[seedBase];
  assert(scopeSeed !== undefined, "scope seed");
  const initialScope =
    defineV013WorkingScope(memory, scopeSeed, interpreter, [initial]);
  const cursor = new V013CurrentScopeCursor(memory, initialScope);

  const expected = expectedBits(a, b, ZERO, ONE);
  const expectedResult =
    materializeExactSequence(memory, [expected.sum, expected.carry]);

  const expectedAfterOpen = memory.ensure(
    xorFrame(memory, K, argumentSequence),
    call(memory, f.b, XOR, argumentSequence),
  );
  const expectedAfterXor = memory.ensure(
    andFrame(memory, K, argumentSequence, expected.sum),
    call(memory, f.b, AND, argumentSequence),
  );
  const expectedAfterAnd = memory.ensure(
    finishFrame(memory, K, expected.sum),
    done(memory, f.b, expected.carry),
  );
  const expectedFinal = memory.ensure(K, expectedResult);

  const expectedMembers = [
    [expectedAfterOpen],
    [expectedAfterXor],
    [expectedAfterAnd],
    [expectedFinal],
  ];

  const matches = [];
  const handoffs = [];

  for (let step = 0; step < 4; step += 1) {
    const nextSeed = fresh[seedBase + 1 + step];
    assert(nextSeed !== undefined, "reaction seed " + step);
    const reaction = reactV013StructuralScope(memory, cursor, nextSeed);

    console.log(
      "C2B_TRACE",
      "input=" + (a === ONE ? "1" : "0") + (b === ONE ? "1" : "0"),
      "step=" + step,
      "matches=" + reaction.rawRuleMatches,
      "transitioned=" + reaction.transitionedMembers,
      "members=" + reaction.nextMembers.join(","),
    );

    assert(!reaction.quiescent, "step " + step + " must be active");
    same(reaction.rawRuleMatches, 1, "step " + step + " one Rule");
    same(reaction.transitionedMembers, 1, "step " + step + " one member");
    same(reaction.handoffCount, 1, "step " + step + " one handoff");
    sameMembers(
      reaction.nextMembers,
      expectedMembers[step],
      "step " + step + " expected structural state",
    );

    matches.push(reaction.rawRuleMatches);
    handoffs.push(reaction.handoffCount);
  }

  const quiescentSeed = fresh[seedBase + 5];
  assert(quiescentSeed !== undefined, "quiescent seed");
  const stableScope = cursor.currentScope();
  const quiescent =
    reactV013StructuralScope(memory, cursor, quiescentSeed);

  assert(quiescent.quiescent, "final result must be quiescent");
  same(quiescent.rawRuleMatches, 0, "quiescent match count");
  same(quiescent.handoffCount, 0, "quiescent handoff count");
  same(cursor.currentScope(), stableScope, "quiescence keeps current root");
  sameMembers(cursor.members(), [expectedFinal], "final result member");

  // Decode the actual runtime result, not the independently calculated
  // expected sequence.
  const finalPoles = memory.poles(cursor.members()[0]);
  same(finalPoles.start, K, "final caller/context preserved");
  const decoded = readExactSequence(memory, finalPoles.end);
  same(decoded.values.length, 2, "Half Adder returns two exact positions");
  same(decoded.values[0], expected.sum, "Sum position");
  same(decoded.values[1], expected.carry, "Carry position");

  return Object.freeze({
    a,
    b,
    sum: decoded.values[0],
    carry: decoded.values[1],
    matches: Object.freeze(matches),
    handoffs: Object.freeze(handoffs),
  });
}

function main() {
  const f = buildFixture();

  const rows = [
    runCase(f, f.ZERO, f.ZERO, 110),
    runCase(f, f.ZERO, f.ONE, 120),
    runCase(f, f.ONE, f.ZERO, 130),
    runCase(f, f.ONE, f.ONE, 140),
  ];

  const expected = [
    [f.ZERO, f.ZERO],
    [f.ONE, f.ZERO],
    [f.ONE, f.ZERO],
    [f.ZERO, f.ONE],
  ];

  rows.forEach((row, index) => {
    same(row.sum, expected[index][0], "row " + index + " Sum");
    same(row.carry, expected[index][1], "row " + index + " Carry");
    assert(row.matches.every((n) => n === 1), "row " + index + " one Rule per active step");
    assert(row.handoffs.every((n) => n === 1), "row " + index + " one handoff per active step");
  });

  console.log([
    "A_CIRCUIT_C2B=GREEN",
    "MTS_ACCEPTED_V013_SHA=" + ACCEPTED_MTS_V013_SHA,
    "HALF_ADDER=GATE_COMPOSED",
    "XOR_RULE_SET=SEPARATE",
    "AND_RULE_SET=SEPARATE",
    "HALF_GLOBAL_TRUTH_TABLE=ABSENT",
    "GENERIC_OPEN=TRUE",
    "GENERIC_FINALIZE=TRUE",
    "SUM_CARRIED_AS_STRUCTURAL_ROLE_BINDING=TRUE",
    "FINAL_SEQUENCE_ASSEMBLED_BY_STRUCTURAL_RULE=TRUE",
    "HOST_RUNTIME_VALUE_JOIN=0",
    "HOST_GATE_EVALUATOR=0",
    "ACTIVE_REACTIONS_PER_CASE=4",
    "FINAL_QUIESCENCE=TRUE",
  ].join(" "));
}

main();
