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
  if (!condition) throw new Error("A-Circuit Full Adder: " + message);
}

function same(actual, expected, message) {
  assert(Object.is(actual, expected),
    message + ": expected " + String(expected) + ", got " + String(actual));
}

function call(memory, apply, fn, arg) {
  return memory.ensure(apply, memory.ensure(fn, arg));
}

function done(memory, tag, value) {
  return memory.ensure(tag, value);
}

function halfXorFrame(memory, caller, args) {
  return memory.ensureStartSelfClosed(memory.ensure(caller, args));
}

function halfAndFrame(memory, caller, args, sum) {
  return memory.ensureStartSelfClosed(
    memory.ensure(caller, memory.ensure(args, sum)),
  );
}

function halfFinishFrame(memory, caller, sum) {
  return memory.ensureStartSelfClosed(memory.ensure(caller, sum));
}

function taggedFrame(memory, tag, caller, value) {
  return memory.ensureStartSelfClosed(
    memory.ensure(tag, memory.ensure(caller, value)),
  );
}

function defineBundleRule(memory, theory, roles, before, after) {
  const dictionary = defineStructuralRoleDictionary(memory, roles);
  const bundle = materializeExactSequence(memory, after);
  const rule = defineStructuralRule(
    memory,
    dictionary,
    memory.ensure(before, bundle),
  );
  const admission = admitStructuralRule(memory, theory, rule);
  return { rule, admission };
}

function admitBundleRule(memory, theory, triggerKey, roles, before, after) {
  const { rule, admission } =
    defineBundleRule(memory, theory, roles, before, after);
  memory.ensure(triggerKey, admission);
  return rule;
}

function indexRuleFor(memory, triggerKeys, admission) {
  for (const key of triggerKeys) memory.ensure(key, admission);
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

function map2(memory, bits) {
  const out = new Map();
  for (const a of bits) {
    const second = new Map();
    out.set(a, second);
    for (const b of bits) {
      second.set(b, materializeExactSequence(memory, [a, b]));
    }
  }
  return out;
}

function get2(map, a, b) {
  const value = map.get(a)?.get(b);
  assert(value !== undefined, "missing 2-argument sequence");
  return value;
}

function map3(memory, bits) {
  const out = new Map();
  for (const a of bits) {
    const second = new Map();
    out.set(a, second);
    for (const b of bits) {
      const third = new Map();
      second.set(b, third);
      for (const c of bits) {
        third.set(c, materializeExactSequence(memory, [a, b, c]));
      }
    }
  }
  return out;
}

function get3(map, a, b, c) {
  const value = map.get(a)?.get(b)?.get(c);
  assert(value !== undefined, "missing 3-argument sequence");
  return value;
}

function buildFixture() {
  const memory = new Memory();
  const b = ensureRootBasis(memory);
  const fresh = freshAnchors(memory, b, 360);
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

  const HALF = memory.ensure(at(4), at(5));
  const XOR = memory.ensure(at(6), at(7));
  const AND = memory.ensure(at(8), at(9));
  const OR = memory.ensure(at(10), at(11));
  const FULL = memory.ensure(at(12), at(13));
  const K = memory.ensure(at(14), at(15));

  const H1_TAG = memory.ensure(at(16), at(17));
  const H2_TAG = memory.ensure(at(18), at(19));
  const OR_TAG = memory.ensure(at(20), at(21));
  const FULL_FINISH_TAG = memory.ensure(at(22), at(23));
  const FULL_DONE_TAG = memory.ensure(at(24), at(25));

  const APPLY = b.O;
  const HALF_DONE = b.C;
  const ZERO = b.U;
  const ONE = b.L;
  const bits = [ZERO, ONE];

  const args2 = map2(memory, bits);
  const args3 = map3(memory, bits);

  // FULL OPEN: decompose [a,b,cin] structurally and invoke first HALF.
  {
    const kRole = at(40);
    const aRole = at(41);
    const bRole = at(42);
    const cinRole = at(43);

    const argsTemplate =
      materializeExactSequence(memory, [aRole, bRole, cinRole]);
    const before = memory.ensure(
      kRole,
      call(memory, APPLY, FULL, argsTemplate),
    );

    const firstArgs = materializeExactSequence(memory, [aRole, bRole]);
    const after = memory.ensure(
      taggedFrame(memory, H1_TAG, kRole, cinRole),
      call(memory, APPLY, HALF, firstArgs),
    );

    admitBundleRule(
      memory, theory, b.O,
      [kRole, aRole, bRole, cinRole],
      before, [after],
    );
  }

  // Reusable HALF OPEN.
  {
    const callerRole = at(50);
    const argsRole = at(51);
    const before = memory.ensure(
      callerRole,
      call(memory, APPLY, HALF, argsRole),
    );
    const after = memory.ensure(
      halfXorFrame(memory, callerRole, argsRole),
      call(memory, APPLY, XOR, argsRole),
    );
    admitBundleRule(
      memory, theory, b.O,
      [callerRole, argsRole],
      before, [after],
    );
  }

  const xorRows = [
    [ZERO, ZERO, ZERO],
    [ZERO, ONE, ONE],
    [ONE, ZERO, ONE],
    [ONE, ONE, ZERO],
  ];

  let roleSeed = 60;
  for (const [a, c, sum] of xorRows) {
    const args = get2(args2, a, c);
    const callerRole = at(roleSeed++);

    const before = memory.ensure(
      halfXorFrame(memory, callerRole, args),
      call(memory, APPLY, XOR, args),
    );
    const after = memory.ensure(
      halfAndFrame(memory, callerRole, args, sum),
      call(memory, APPLY, AND, args),
    );
    admitBundleRule(memory, theory, b.O, [callerRole], before, [after]);
  }

  const andRows = [
    [ZERO, ZERO, ZERO],
    [ZERO, ONE, ZERO],
    [ONE, ZERO, ZERO],
    [ONE, ONE, ONE],
  ];

  for (const [a, c, carry] of andRows) {
    const args = get2(args2, a, c);
    const callerRole = at(roleSeed++);
    const sumRole = at(roleSeed++);

    const before = memory.ensure(
      halfAndFrame(memory, callerRole, args, sumRole),
      call(memory, APPLY, AND, args),
    );
    const after = memory.ensure(
      halfFinishFrame(memory, callerRole, sumRole),
      done(memory, HALF_DONE, carry),
    );
    admitBundleRule(
      memory, theory, b.O,
      [callerRole, sumRole],
      before, [after],
    );
  }

  // Reusable HALF FINALIZE.
  {
    const callerRole = at(90);
    const sumRole = at(91);
    const carryRole = at(92);
    const before = memory.ensure(
      halfFinishFrame(memory, callerRole, sumRole),
      done(memory, HALF_DONE, carryRole),
    );
    const result = materializeExactSequence(memory, [sumRole, carryRole]);
    const after = memory.ensure(callerRole, result);
    admitBundleRule(
      memory, theory, b.C,
      [callerRole, sumRole, carryRole],
      before, [after],
    );
  }

  const halfOutputs = [
    materializeExactSequence(memory, [ZERO, ZERO]),
    materializeExactSequence(memory, [ONE, ZERO]),
    materializeExactSequence(memory, [ZERO, ONE]),
  ];

  // H1 RETURN -> invoke second HALF(s1,cin), carrying c1 in caller frame.
  {
    const kRole = at(100);
    const cinRole = at(101);
    const s1Role = at(102);
    const c1Role = at(103);

    const before = memory.ensure(
      taggedFrame(memory, H1_TAG, kRole, cinRole),
      materializeExactSequence(memory, [s1Role, c1Role]),
    );
    const after = memory.ensure(
      taggedFrame(memory, H2_TAG, kRole, c1Role),
      call(
        memory,
        APPLY,
        HALF,
        materializeExactSequence(memory, [s1Role, cinRole]),
      ),
    );
    const { admission } = defineBundleRule(
      memory, theory,
      [kRole, cinRole, s1Role, c1Role],
      before, [after],
    );
    indexRuleFor(memory, halfOutputs, admission);
  }

  // H2 RETURN -> invoke OR(c1,c2), carrying sum in caller frame.
  {
    const kRole = at(110);
    const c1Role = at(111);
    const sumRole = at(112);
    const c2Role = at(113);

    const before = memory.ensure(
      taggedFrame(memory, H2_TAG, kRole, c1Role),
      materializeExactSequence(memory, [sumRole, c2Role]),
    );
    const after = memory.ensure(
      taggedFrame(memory, OR_TAG, kRole, sumRole),
      call(
        memory,
        APPLY,
        OR,
        materializeExactSequence(memory, [c1Role, c2Role]),
      ),
    );
    const { admission } = defineBundleRule(
      memory, theory,
      [kRole, c1Role, sumRole, c2Role],
      before, [after],
    );
    indexRuleFor(memory, halfOutputs, admission);
  }

  const orRows = [
    [ZERO, ZERO, ZERO],
    [ZERO, ONE, ONE],
    [ONE, ZERO, ONE],
    [ONE, ONE, ONE],
  ];

  for (const [c1, c2, cout] of orRows) {
    const args = get2(args2, c1, c2);
    const kRole = at(roleSeed++);
    const sumRole = at(roleSeed++);

    const before = memory.ensure(
      taggedFrame(memory, OR_TAG, kRole, sumRole),
      call(memory, APPLY, OR, args),
    );
    const after = memory.ensure(
      taggedFrame(memory, FULL_FINISH_TAG, kRole, sumRole),
      done(memory, FULL_DONE_TAG, cout),
    );

    admitBundleRule(
      memory, theory, b.O,
      [kRole, sumRole],
      before, [after],
    );
  }

  // FULL FINALIZE. Dedicated FULL_DONE_TAG avoids collision with generic
  // HALF FINALIZE namespace.
  {
    const kRole = at(140);
    const sumRole = at(141);
    const coutRole = at(142);
    const before = memory.ensure(
      taggedFrame(memory, FULL_FINISH_TAG, kRole, sumRole),
      done(memory, FULL_DONE_TAG, coutRole),
    );
    const result = materializeExactSequence(memory, [sumRole, coutRole]);
    const after = memory.ensure(kRole, result);

    admitBundleRule(
      memory, theory, FULL_DONE_TAG,
      [kRole, sumRole, coutRole],
      before, [after],
    );
  }

  return Object.freeze({
    memory, b, interpreter, FULL, K, APPLY, ZERO, ONE, args3, fresh,
  });
}

function runCase(f, ai, bi, ci, seedBase) {
  const bits = [f.ZERO, f.ONE];
  const args = get3(f.args3, bits[ai], bits[bi], bits[ci]);
  const initial = f.memory.ensure(
    f.K,
    call(f.memory, f.APPLY, f.FULL, args),
  );

  const scopeSeed = f.fresh[seedBase];
  assert(scopeSeed !== undefined, "scope seed");
  const initialScope = defineV013WorkingScope(
    f.memory, scopeSeed, f.interpreter, [initial],
  );
  const cursor = new V013CurrentScopeCursor(f.memory, initialScope);

  for (let step = 0; step < 13; step += 1) {
    const nextSeed = f.fresh[seedBase + step + 1];
    assert(nextSeed !== undefined, "reaction seed " + step);
    const reaction = reactV013StructuralScope(f.memory, cursor, nextSeed);

    assert(!reaction.quiescent, "step " + step + " active");
    same(reaction.rawRuleMatches, 1, "step " + step + " match count");
    same(reaction.transitionedMembers, 1, "step " + step + " transitioned count");
    same(reaction.handoffCount, 1, "step " + step + " handoff");
    same(reaction.nextMembers.length, 1, "step " + step + " scope size");
  }

  const stable = cursor.currentScope();
  const quiescent = reactV013StructuralScope(
    f.memory, cursor, f.fresh[seedBase + 14],
  );
  assert(quiescent.quiescent, "final quiescence");
  same(quiescent.rawRuleMatches, 0, "quiescent match count");
  same(quiescent.handoffCount, 0, "quiescent handoff");
  same(cursor.currentScope(), stable, "quiescent root stable");

  const final = cursor.members()[0];
  const poles = f.memory.poles(final);
  same(poles.start, f.K, "final caller");
  const result = readExactSequence(f.memory, poles.end);
  same(result.values.length, 2, "Full Adder result arity");

  const total = ai + bi + ci;
  const expectedSum = total % 2;
  const expectedCout = total >= 2 ? 1 : 0;

  same(result.values[0], bits[expectedSum], "Sum");
  same(result.values[1], bits[expectedCout], "Cout");

  console.log(
    "FULL_ADDER_TRACE",
    "input=" + ai + bi + ci,
    "sum=" + expectedSum,
    "cout=" + expectedCout,
    "active_steps=13",
  );

  return Object.freeze({ input: "" + ai + bi + ci, sum: expectedSum, cout: expectedCout });
}

function main() {
  const f = buildFixture();
  const rows = [];
  let seed = 180;

  for (let a = 0; a <= 1; a += 1) {
    for (let b = 0; b <= 1; b += 1) {
      for (let cin = 0; cin <= 1; cin += 1) {
        rows.push(runCase(f, a, b, cin, seed));
        seed += 16;
      }
    }
  }

  same(rows.length, 8, "all Full Adder rows");
  console.log([
    "A_CIRCUIT_M2_FULL_ADDER=GREEN",
    "MTS_ACCEPTED_V013_SHA=" + ACCEPTED_MTS_V013_SHA,
    "FULL_GLOBAL_TRUTH_TABLE=ABSENT",
    "HALF_REUSED_TWICE=TRUE",
    "OR_RULE_SET=SEPARATE",
    "HOST_RUNTIME_VALUE_JOIN=0",
    "HOST_GATE_EVALUATOR=0",
    "ACTIVE_REACTIONS_PER_CASE=13",
    "FINAL_QUIESCENCE=TRUE",
  ].join(" "));
}

main();
