import {
  deriveRecursiveSource,
  validateCompactProof,
} from "./i386-proof-transport.mjs";

export const PROOF_VERIFICATION_PROFILE_ID =
  "bounded-static-theory-structural-v1";

function fail(message) {
  throw new Error("proof verifier: " + message);
}

function sameArray(left, right) {
  return Array.isArray(left) &&
    Array.isArray(right) &&
    left.length === right.length &&
    left.every((value, index) => value === right[index]);
}

function formOf(pair, handle) {
  if (pair.start === handle && pair.end === handle) return "ROOT";
  if (pair.start === handle) return "START";
  if (pair.end === handle) return "END";
  return "PAIR";
}

class StructuralStore {
  constructor(starts, ends, label) {
    if (!Array.isArray(starts) || !Array.isArray(ends) ||
        starts.length !== ends.length || starts.length === 0) {
      fail(label + " has invalid topology columns");
    }

    this.label = label;
    this.pairs = [null];
    this.canonical = new Map();
    this.rootHandle = null;

    for (let index = 0; index < starts.length; index += 1) {
      const handle = index + 1;
      const pair = { start: starts[index], end: ends[index] };
      if (!Number.isInteger(pair.start) || !Number.isInteger(pair.end) ||
          pair.start < 1 || pair.start > starts.length ||
          pair.end < 1 || pair.end > starts.length) {
        fail(label + " L" + handle + " has out-of-range pole");
      }
      const key = this.keyFor(pair, handle);
      if (this.canonical.has(key)) {
        fail(label + " is non-canonical at L" + handle);
      }
      this.canonical.set(key, handle);
      this.pairs.push(pair);
      if (formOf(pair, handle) === "ROOT") {
        if (this.rootHandle !== null) {
          fail(label + " has multiple ROOT forms");
        }
        this.rootHandle = handle;
      }
    }

    if (this.rootHandle === null) {
      fail(label + " has no ROOT form");
    }
  }

  get linkCount() {
    return this.pairs.length - 1;
  }

  pair(handle) {
    const pair = this.pairs[handle];
    if (!pair) fail(this.label + " missing L" + handle);
    return pair;
  }

  keyFor(pair, handle) {
    const form = formOf(pair, handle);
    if (form === "ROOT") return "8";
    if (form === "START") return "9:" + pair.end;
    if (form === "END") return "6:" + pair.start;
    return "1:" + pair.start + ":" + pair.end;
  }

  ensureStart(end) {
    return this.ensure("START", null, end);
  }

  ensureEnd(start) {
    return this.ensure("END", start, null);
  }

  ensurePair(start, end) {
    return this.ensure("PAIR", start, end);
  }

  ensure(kind, start, end) {
    let key;
    if (kind === "START") key = "9:" + end;
    else if (kind === "END") key = "6:" + start;
    else if (kind === "PAIR") key = "1:" + start + ":" + end;
    else fail("unsupported constructor " + kind);

    const existing = this.canonical.get(key);
    if (existing !== undefined) return existing;

    const handle = this.pairs.length;
    const pair = kind === "START"
      ? { start: handle, end }
      : kind === "END"
        ? { start, end: handle }
        : { start, end };
    this.pairs.push(pair);
    this.canonical.set(key, handle);
    return handle;
  }

  sequence(handle) {
    const values = [];
    const seen = new Set();
    let current = handle;

    while (current !== this.rootHandle) {
      if (seen.has(current)) {
        fail(this.label + " cyclic ExactSequence at L" + current);
      }
      seen.add(current);
      const pair = this.pair(current);
      if (pair.start !== current || pair.end === current) {
        fail(this.label + " invalid ExactSequence node L" + current);
      }
      const cell = this.pair(pair.end);
      current = cell.start;
      values.push(cell.end);
    }

    values.reverse();
    return values;
  }
}

function pairObjects(starts, ends) {
  return starts.map((start, index) => ({ start, end: ends[index] }));
}

function rootDirectory(compact) {
  const roots = new Map();
  for (const root of compact.roots) {
    if (roots.has(root.role)) fail("duplicate root role " + root.role);
    roots.set(root.role, root.carrierRef);
  }
  return roots;
}

function requireRoot(roots, role) {
  const handle = roots.get(role);
  if (!Number.isInteger(handle)) fail("missing semantic root " + role);
  return handle;
}

function structurallyEqual(leftStore, left, rightStore, right) {
  const stack = [[left, right]];
  const seen = new Set();

  while (stack.length) {
    const [a, b] = stack.pop();
    const key = a + ":" + b;
    if (seen.has(key)) continue;
    seen.add(key);

    const pa = leftStore.pair(a);
    const pb = rightStore.pair(b);
    const fa = formOf(pa, a);
    const fb = formOf(pb, b);
    if (fa !== fb) return false;

    if (fa === "START") {
      stack.push([pa.end, pb.end]);
    } else if (fa === "END") {
      stack.push([pa.start, pb.start]);
    } else if (fa === "PAIR") {
      stack.push([pa.start, pb.start], [pa.end, pb.end]);
    }
  }

  return true;
}

function structuralSetEqual(leftStore, left, rightStore, right) {
  if (left.length !== right.length) return false;
  const used = new Set();

  for (const leftHandle of left) {
    let found = false;
    for (let index = 0; index < right.length; index += 1) {
      if (used.has(index)) continue;
      if (structurallyEqual(
        leftStore,
        leftHandle,
        rightStore,
        right[index],
      )) {
        used.add(index);
        found = true;
        break;
      }
    }
    if (!found) return false;
  }

  return true;
}

function decodeRules(store, roots) {
  const interpreter = requireRoot(roots, "execution.interpreter");
  const interpreterPair = store.pair(interpreter);
  const grammarTheory = store.pair(interpreterPair.end);
  const theory = grammarTheory.end;
  const rules = [];

  for (let admission = 1; admission <= store.linkCount; admission += 1) {
    const admitted = store.pair(admission);
    if (admitted.start !== theory || admitted.end === admission) continue;

    const rule = admitted.end;
    const rulePair = store.pair(rule);
    const dictionary = rulePair.start;
    const body = rulePair.end;
    const dictionaryPair = store.pair(dictionary);
    if (dictionaryPair.start !== dictionary ||
        dictionaryPair.end === dictionary) {
      continue;
    }

    let roles;
    try {
      roles = new Set(store.sequence(dictionaryPair.end));
    } catch {
      continue;
    }

    const bodyPair = store.pair(body);
    rules.push({
      admission,
      rule,
      roles,
      before: bodyPair.start,
      outputBundle: bodyPair.end,
    });
  }

  if (rules.length === 0) fail("no structural Rules derived from Theory");
  return { theory, rules };
}

function unify(store, template, subject, roles) {
  const todo = [[template, subject]];
  const seen = new Set();
  const bindings = new Map();

  while (todo.length) {
    const [left, right] = todo.pop();

    if (roles.has(left)) {
      const existing = bindings.get(left);
      if (existing !== undefined && existing !== right) return null;
      bindings.set(left, right);
      continue;
    }

    const key = left + ":" + right;
    if (seen.has(key)) continue;
    seen.add(key);

    const lp = store.pair(left);
    const rp = store.pair(right);
    const lf = formOf(lp, left);
    const rf = formOf(rp, right);
    if (lf !== rf) return null;

    if (lf === "START") {
      todo.push([lp.end, rp.end]);
    } else if (lf === "END") {
      todo.push([lp.start, rp.start]);
    } else if (lf === "PAIR") {
      todo.push([lp.start, rp.start], [lp.end, rp.end]);
    }
  }

  if (bindings.size !== roles.size) return null;
  return bindings;
}

function instantiate(store, template, bindings) {
  const memo = new Map(bindings);
  const frames = [[template, false]];

  while (frames.length) {
    const [handle, finishing] = frames.pop();
    if (memo.has(handle)) continue;

    const pair = store.pair(handle);
    const form = formOf(pair, handle);
    if (form === "ROOT") {
      memo.set(handle, store.rootHandle);
      continue;
    }

    if (finishing) {
      if (form === "START") {
        memo.set(handle, store.ensureStart(memo.get(pair.end)));
      } else if (form === "END") {
        memo.set(handle, store.ensureEnd(memo.get(pair.start)));
      } else {
        memo.set(
          handle,
          store.ensurePair(memo.get(pair.start), memo.get(pair.end)),
        );
      }
      continue;
    }

    frames.push([handle, true]);
    if (form === "START") {
      if (!memo.has(pair.end)) frames.push([pair.end, false]);
    } else if (form === "END") {
      if (!memo.has(pair.start)) frames.push([pair.start, false]);
    } else {
      if (!memo.has(pair.end)) frames.push([pair.end, false]);
      if (!memo.has(pair.start)) frames.push([pair.start, false]);
    }
  }

  const result = memo.get(template);
  if (!Number.isInteger(result)) fail("template instantiation did not finish");
  return result;
}

function decodeBit(store, handle, roots) {
  const zero = roots.get("data.bit.zero") ?? roots.get("data.zero");
  const one = roots.get("data.bit.one") ?? roots.get("data.one");
  if (handle === zero) return 0;
  if (handle === one) return 1;
  fail("result contains non-bit L" + handle);
}

function decodeWord(store, handle, roots, width) {
  const bits = store.sequence(handle);
  if (bits.length !== width) {
    fail("result word width is " + bits.length + ", expected " + width);
  }
  let value = 0;
  for (let index = 0; index < bits.length; index += 1) {
    value += decodeBit(store, bits[index], roots) * (2 ** index);
  }
  return value;
}

function decodeWord32(store, handle, roots) {
  return decodeWord(store, handle, roots, 32);
}

function decodeWord8(store, handle, roots) {
  return decodeWord(store, handle, roots, 8);
}

function verifyResultStructure(compact, actualStore, roots, finalScope) {
  if (finalScope.length !== 1) {
    fail("supported profile requires exactly one final Scope member");
  }

  const finalHandle = finalScope[0];
  const allPairs = actualStore.pairs.slice(1);
  const resultWire = deriveRecursiveSource(allPairs, finalHandle);
  if (resultWire !== compact.result.resultRecursiveWire) {
    fail("resultRecursiveWire does not identify final Scope structure");
  }

  const finalPair = actualStore.pair(finalHandle);
  const caller = requireRoot(roots, "context.caller");
  if (finalPair.start !== caller) {
    fail("final Scope caller does not match context.caller");
  }

  const endpoint = actualStore.pair(finalPair.end);
  const resultTag = requireRoot(roots, "result.tag");
  if (endpoint.start !== resultTag) {
    fail("final Scope endpoint does not match result.tag");
  }

  const payload = endpoint.end;
  const sequenceWire = deriveRecursiveSource(allPairs, payload);
  if (sequenceWire !== compact.result.resultSequenceAnum) {
    fail("resultSequenceAnum does not identify Result payload");
  }

  const fields = actualStore.sequence(payload);
  let decodedValue;
  let decodedValueHi = null;

  if (roots.has("function.memory.witness")) {
    if (fields.length !== 5) {
      fail("M6 radix Result payload does not have five fields");
    }
    decodedValue = decodeWord8(actualStore, fields[3], roots);
    decodedValueHi = decodeWord8(actualStore, fields[4], roots);
  } else if (roots.has("function.mul32")) {
    if (fields.length < 1) fail("MUL32 Result payload is empty");
    const wide = actualStore.sequence(fields[0]);
    if (wide.length !== 2) fail("MUL32 Wide64 payload is not lo/hi");
    decodedValue = decodeWord32(actualStore, wide[0], roots);
    decodedValueHi = decodeWord32(actualStore, wide[1], roots);
  } else if (roots.has("data.zero")) {
    if (fields.length < 1) fail("bit Result payload is empty");
    decodedValue = decodeBit(actualStore, fields[0], roots);
  } else {
    if (fields.length !== 3) {
      fail("ALU effect Result payload does not have three fields");
    }
    decodedValue = decodeWord32(actualStore, fields[1], roots);
  }

  if (compact.result.decodedValue !== decodedValue) {
    fail(
      "decodedValue mismatch: evidence=" + compact.result.decodedValue +
      " structure=" + decodedValue,
    );
  }
  if (decodedValueHi !== null &&
      compact.result.decodedValueHi !== decodedValueHi) {
    fail(
      "decodedValueHi mismatch: evidence=" + compact.result.decodedValueHi +
      " structure=" + decodedValueHi,
    );
  }

  return { decodedValue, decodedValueHi };
}

export function verifyCompactTraceConsistency(compact) {
  validateCompactProof(compact);
  if (compact.schemaVersion !== 2 ||
      compact.sourceProofSchemaVersion !== 4) {
    fail(
      "verification profile " + PROOF_VERIFICATION_PROFILE_ID +
      " supports compact schema 2 / source schema 4 only",
    );
  }

  const roots = rootDirectory(compact);
  const reactions = compact.execute.reactions;
  if (reactions.length === 0) fail("execution trace is empty");

  const initial = requireRoot(roots, "scope.initial");
  if (!sameArray(reactions[0].scopeBefore, [initial])) {
    fail("first scopeBefore does not equal scope.initial");
  }

  for (let index = 0; index < reactions.length; index += 1) {
    const step = reactions[index];
    if (index > 0 &&
        !sameArray(reactions[index - 1].scopeAfter, step.scopeBefore)) {
      fail("Scope chain breaks before reaction " + index);
    }

    if (step.quiescent) {
      if (step.rawRuleMatches !== 0 ||
          step.transitionedMembers !== 0 ||
          step.handoffCount !== 0 ||
          !sameArray(step.scopeBefore, step.scopeAfter)) {
        fail("quiescent reaction " + index + " has transition evidence");
      }
    } else if (step.rawRuleMatches === 0 || step.handoffCount !== 1) {
      fail("active reaction " + index + " has invalid match/handoff summary");
    }
  }

  if (!compact.execute.finalQuiescent ||
      reactions.at(-1).quiescent !== true) {
    fail("trace does not end in quiescence");
  }

  const starts = compact.topology.base.starts.concat(
    compact.topology.append.starts,
  );
  const ends = compact.topology.base.ends.concat(
    compact.topology.append.ends,
  );
  const actualStore = new StructuralStore(starts, ends, "evidence Store");
  const result = verifyResultStructure(
    compact,
    actualStore,
    roots,
    reactions.at(-1).scopeAfter,
  );

  return {
    schemaVersion: 1,
    profileId: PROOF_VERIFICATION_PROFILE_ID,
    transportValid: true,
    traceConsistent: true,
    reactions: reactions.length,
    finalScopeWidth: reactions.at(-1).scopeAfter.length,
    ...result,
  };
}

export function verifyCompactSemanticReplay(compact) {
  const consistency = verifyCompactTraceConsistency(compact);
  const roots = rootDirectory(compact);

  const replayStore = new StructuralStore(
    compact.topology.base.starts,
    compact.topology.base.ends,
    "replay base Store",
  );
  const actualStore = new StructuralStore(
    compact.topology.base.starts.concat(compact.topology.append.starts),
    compact.topology.base.ends.concat(compact.topology.append.ends),
    "evidence Store",
  );

  const { rules } = decodeRules(replayStore, roots);
  const derivedAdmissions = rules
    .map((rule) => rule.admission)
    .sort((a, b) => a - b);
  const declaredAdmissions = [...compact.theoryAdmissions]
    .sort((a, b) => a - b);
  if (new Set(declaredAdmissions).size !== declaredAdmissions.length ||
      !sameArray(derivedAdmissions, declaredAdmissions)) {
    fail("theoryAdmissions does not equal admissions derived from topology");
  }

  let current = [requireRoot(roots, "scope.initial")];
  let totalMatches = 0;

  for (let index = 0; index < compact.execute.reactions.length; index += 1) {
    const recorded = compact.execute.reactions[index];

    if (!structuralSetEqual(
      replayStore,
      current,
      actualStore,
      recorded.scopeBefore,
    )) {
      fail("semantic replay scopeBefore mismatch at reaction " + index);
    }

    const discovered = [];
    let matchCount = 0;
    let transitionedMembers = 0;

    for (const active of current) {
      const matches = [];
      for (const rule of rules) {
        const bindings = unify(
          replayStore,
          rule.before,
          active,
          rule.roles,
        );
        if (bindings !== null) {
          matches.push({ rule, bindings });
        }
      }
      if (matches.length > 0) transitionedMembers += 1;
      matchCount += matches.length;
      discovered.push({ active, matches });
    }

    const successors = [];
    const seen = new Set();
    const addSuccessor = (handle) => {
      if (!seen.has(handle)) {
        seen.add(handle);
        successors.push(handle);
      }
    };

    for (const { active, matches } of discovered) {
      if (matches.length === 0) {
        addSuccessor(active);
        continue;
      }
      for (const { rule, bindings } of matches) {
        const grounded = instantiate(
          replayStore,
          rule.outputBundle,
          bindings,
        );
        for (const successor of replayStore.sequence(grounded)) {
          addSuccessor(successor);
        }
      }
    }

    const quiescent = matchCount === 0;
    if (recorded.rawRuleMatches !== matchCount) {
      fail(
        "rawRuleMatches mismatch at reaction " + index +
        ": evidence=" + recorded.rawRuleMatches +
        " replay=" + matchCount,
      );
    }
    if (recorded.transitionedMembers !== transitionedMembers) {
      fail("transitionedMembers mismatch at reaction " + index);
    }
    if (recorded.quiescent !== quiescent) {
      fail("quiescence mismatch at reaction " + index);
    }
    if (recorded.handoffCount !== (quiescent ? 0 : 1)) {
      fail("handoff mismatch at reaction " + index);
    }
    if (replayStore.linkCount !== recorded.linksAfter) {
      fail(
        "linksAfter mismatch at reaction " + index +
        ": evidence=" + recorded.linksAfter +
        " replay=" + replayStore.linkCount,
      );
    }
    if (!structuralSetEqual(
      replayStore,
      successors,
      actualStore,
      recorded.scopeAfter,
    )) {
      fail("semantic replay scopeAfter mismatch at reaction " + index);
    }

    totalMatches += matchCount;
    current = successors;
  }

  if (!compact.execute.finalQuiescent ||
      compact.execute.reactions.at(-1).rawRuleMatches !== 0) {
    fail("semantic replay did not terminate at a recorded quiescent step");
  }

  verifyResultStructure(
    compact,
    actualStore,
    roots,
    compact.execute.reactions.at(-1).scopeAfter,
  );

  return {
    ...consistency,
    semanticReplayVerified: true,
    admittedRules: rules.length,
    replayLinks: replayStore.linkCount,
    totalRuleMatches: totalMatches,
  };
}

export function verifyCompactExecutionProof(compact) {
  return verifyCompactSemanticReplay(compact);
}
