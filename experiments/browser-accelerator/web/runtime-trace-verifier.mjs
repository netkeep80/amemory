const TRACE_VERIFIER_SCHEMA_VERSION = 1;

function fail(message) {
  throw new Error("live trace consistency: " + message);
}

function requireObject(value, label) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    fail(label + " must be an object");
  }
  return value;
}

function requireUint(value, label) {
  if (!Number.isInteger(value) || value < 0 || value > 0xffff_ffff) {
    fail(label + " must be a uint32");
  }
  return value;
}

function requireHandle(value, label) {
  const handle = requireUint(value, label);
  if (handle === 0) fail(label + " must be a non-zero Link handle");
  return handle;
}

function requireBool(value, label) {
  if (typeof value !== "boolean") fail(label + " must be boolean");
  return value;
}

function requireHandleArray(value, label) {
  if (!Array.isArray(value)) fail(label + " must be an array");
  return value.map((item, index) =>
    requireHandle(item, label + "[" + index + "]"));
}

function sameArray(left, right) {
  return left.length === right.length &&
    left.every((value, index) => value === right[index]);
}

function countByKey(items, keyOf) {
  const counts = new Map();
  for (const item of items) {
    const key = keyOf(item);
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  return counts;
}

function sameCounts(left, right) {
  if (left.size !== right.size) return false;
  for (const [key, value] of left) {
    if (right.get(key) !== value) return false;
  }
  return true;
}

function requireFacts(evidence) {
  if (!Array.isArray(evidence.structuralFacts)) {
    fail("structuralFacts are required for TRACE consistency");
  }
  return evidence.structuralFacts.map((fact, index) => {
    requireObject(fact, "structuralFacts[" + index + "]");
    if (typeof fact.kind !== "string") {
      fail("structuralFacts[" + index + "].kind must be a string");
    }
    return fact;
  });
}

function verifyCreatedLinks(instantiated, linksBefore, linksAfter) {
  let expectedHandle = linksBefore + 1;
  let createdCount = 0;

  for (let index = 0; index < instantiated.length; index += 1) {
    const fact = instantiated[index];
    if (!Array.isArray(fact.createdLinks)) {
      fail("INSTANTIATED[" + index + "].createdLinks must be an array");
    }
    for (let linkIndex = 0; linkIndex < fact.createdLinks.length; linkIndex += 1) {
      const link = requireObject(
        fact.createdLinks[linkIndex],
        "INSTANTIATED[" + index + "].createdLinks[" + linkIndex + "]",
      );
      const handle = requireHandle(
        link.handle,
        "created Link handle",
      );
      const start = requireHandle(link.start, "created Link start");
      const end = requireHandle(link.end, "created Link end");

      if (handle !== expectedHandle) {
        fail(
          "persistent Link delta is not contiguous: expected handle " +
          expectedHandle + ", got " + handle,
        );
      }
      if (start > handle || end > handle) {
        fail(
          "created Link " + handle +
          " refers to a future/nonexistent pole",
        );
      }

      expectedHandle += 1;
      createdCount += 1;
    }
  }

  if (linksBefore + createdCount !== linksAfter) {
    fail(
      "persistent Link delta count mismatch: before=" + linksBefore +
      " created=" + createdCount + " after=" + linksAfter,
    );
  }
  return createdCount;
}

function verifyPublishedScope(published, scopeAfter) {
  const reconstructed = [];
  const seen = new Set();

  for (let index = 0; index < published.length; index += 1) {
    const fact = published[index];
    const outputs = requireHandleArray(
      fact.outputs,
      "PUBLISHED[" + index + "].outputs",
    );
    const preserved = requireBool(
      fact.preserved,
      "PUBLISHED[" + index + "].preserved",
    );

    if (preserved) {
      if (fact.rule != null) {
        fail("preserved PUBLISHED fact must not carry a rule");
      }
      const active = requireHandle(
        fact.active,
        "preserved PUBLISHED active",
      );
      if (outputs.length !== 1 || outputs[0] !== active) {
        fail("preserved PUBLISHED fact must publish only its active Link");
      }
    } else {
      requireHandle(fact.rule, "PUBLISHED rule");
    }

    for (const output of outputs) {
      if (!seen.has(output)) {
        seen.add(output);
        reconstructed.push(output);
      }
    }
  }

  if (!sameArray(reconstructed, scopeAfter)) {
    fail(
      "PUBLISHED outputs do not reconstruct scopeAfter: expected " +
      JSON.stringify(scopeAfter) + ", got " + JSON.stringify(reconstructed),
    );
  }
}

export function verifyLiveReactionEvidence(rawEvidence) {
  const evidence = requireObject(rawEvidence, "evidence");
  if (evidence.schemaVersion !== TRACE_VERIFIER_SCHEMA_VERSION) {
    fail("unsupported evidence schemaVersion " + evidence.schemaVersion);
  }

  const scopeBefore = requireHandleArray(evidence.scopeBefore, "scopeBefore");
  const scopeAfter = requireHandleArray(evidence.scopeAfter, "scopeAfter");
  const linksBefore = requireUint(evidence.linksBefore, "linksBefore");
  const linksAfter = requireUint(evidence.linksAfter, "linksAfter");
  const rawRuleMatches = requireUint(
    evidence.rawRuleMatches,
    "rawRuleMatches",
  );
  const transitionedMembers = requireUint(
    evidence.transitionedMembers,
    "transitionedMembers",
  );
  const handoffCount = requireUint(evidence.handoffCount, "handoffCount");
  const quiescent = requireBool(evidence.quiescent, "quiescent");

  if (linksAfter < linksBefore) {
    fail("linksAfter is smaller than linksBefore");
  }

  const facts = requireFacts(evidence);
  const allowedKinds = new Set([
    "DISCOVERY_COMPLETE",
    "RULE_MATCHED",
    "INSTANTIATED",
    "PUBLISHED",
    "SCOPE_COMMITTED",
  ]);
  for (const fact of facts) {
    if (!allowedKinds.has(fact.kind)) {
      fail("unsupported structural fact kind " + fact.kind);
    }
  }

  const discovery = facts.filter((fact) =>
    fact.kind === "DISCOVERY_COMPLETE");
  const matched = facts.filter((fact) => fact.kind === "RULE_MATCHED");
  const instantiated = facts.filter((fact) => fact.kind === "INSTANTIATED");
  const published = facts.filter((fact) => fact.kind === "PUBLISHED");
  const committed = facts.filter((fact) => fact.kind === "SCOPE_COMMITTED");

  if (discovery.length !== scopeBefore.length) {
    fail("DISCOVERY_COMPLETE count does not equal scopeBefore width");
  }
  const discoveredActives = discovery.map((fact, index) =>
    requireHandle(fact.active, "DISCOVERY_COMPLETE[" + index + "].active"));
  if (!sameArray(discoveredActives, scopeBefore)) {
    fail("DISCOVERY_COMPLETE active order does not equal scopeBefore");
  }

  let discoveredMatchTotal = 0;
  let discoveredTransitioned = 0;
  let inertMembers = 0;
  for (let index = 0; index < discovery.length; index += 1) {
    const count = requireUint(
      discovery[index].matchedRules,
      "DISCOVERY_COMPLETE[" + index + "].matchedRules",
    );
    discoveredMatchTotal += count;
    if (count > 0) discoveredTransitioned += 1;
    else inertMembers += 1;
  }

  if (discoveredMatchTotal !== rawRuleMatches ||
      matched.length !== rawRuleMatches ||
      instantiated.length !== rawRuleMatches) {
    fail("match/instantiation counts disagree with rawRuleMatches");
  }
  if (discoveredTransitioned !== transitionedMembers) {
    fail("transitionedMembers disagrees with discovery evidence");
  }

  const matchKeys = countByKey(matched, (fact) =>
    requireHandle(fact.active, "RULE_MATCHED active") + ":" +
    requireHandle(fact.rule, "RULE_MATCHED rule"));
  const instantiatedKeys = countByKey(instantiated, (fact) =>
    requireHandle(fact.active, "INSTANTIATED active") + ":" +
    requireHandle(fact.rule, "INSTANTIATED rule"));
  const activePublished = published.filter((fact) => fact.preserved === false);
  const publishedKeys = countByKey(activePublished, (fact) =>
    requireHandle(fact.active, "PUBLISHED active") + ":" +
    requireHandle(fact.rule, "PUBLISHED rule"));

  if (!sameCounts(matchKeys, instantiatedKeys) ||
      !sameCounts(matchKeys, publishedKeys)) {
    fail("matched / instantiated / published rule identities disagree");
  }

  const preserved = published.filter((fact) => fact.preserved === true);
  if (preserved.length !== inertMembers) {
    fail("preserved publication count disagrees with inert discovery");
  }

  const createdLinks = verifyCreatedLinks(
    instantiated,
    linksBefore,
    linksAfter,
  );
  verifyPublishedScope(published, scopeAfter);

  if (committed.length !== 1 || facts.at(-1) !== committed[0]) {
    fail("exactly one final SCOPE_COMMITTED fact is required");
  }
  const scopeCommit = committed[0];
  const oldScope = requireHandleArray(scopeCommit.oldScope, "oldScope");
  const nextScope = requireHandleArray(scopeCommit.nextScope, "nextScope");
  if (!sameArray(oldScope, scopeBefore) ||
      !sameArray(nextScope, scopeAfter) ||
      scopeCommit.quiescent !== quiescent ||
      scopeCommit.handoffCount !== handoffCount) {
    fail("SCOPE_COMMITTED summary disagrees with reaction evidence");
  }

  if (quiescent) {
    if (rawRuleMatches !== 0 ||
        transitionedMembers !== 0 ||
        handoffCount !== 0 ||
        linksBefore !== linksAfter ||
        !sameArray(scopeBefore, scopeAfter) ||
        instantiated.length !== 0) {
      fail("quiescent reaction carries transition evidence");
    }
  } else if (rawRuleMatches === 0 || handoffCount !== 1) {
    fail("active reaction has invalid match/handoff summary");
  }

  return {
    schemaVersion: TRACE_VERIFIER_SCHEMA_VERSION,
    traceConsistent: true,
    createdLinks,
    publishedFacts: published.length,
    scopeWidthBefore: scopeBefore.length,
    scopeWidthAfter: scopeAfter.length,
    quiescent,
  };
}

export function verifyLiveStepTrace(reports) {
  if (!Array.isArray(reports) || reports.length === 0) {
    fail("step trace must contain at least one reaction");
  }

  let sessionId = null;
  let sessionRunId = null;
  let activeReactionCount = 0;
  let createdLinks = 0;
  let completed = false;

  for (let index = 0; index < reports.length; index += 1) {
    const report = requireObject(reports[index], "reports[" + index + "]");
    const evidence = requireObject(
      report.evidence,
      "reports[" + index + "].evidence",
    );
    const verified = verifyLiveReactionEvidence(evidence);
    createdLinks += verified.createdLinks;

    if (evidence.reactionIndex !== index) {
      fail("reactionIndex is not contiguous at report " + index);
    }
    if (report.sessionRunId !== evidence.runId) {
      fail("sessionRunId does not equal evidence.runId");
    }

    if (index === 0) {
      sessionId = evidence.sessionId;
      sessionRunId = report.sessionRunId;
    } else {
      const previous = reports[index - 1].evidence;
      if (evidence.sessionId !== sessionId ||
          report.sessionRunId !== sessionRunId) {
        fail("session/run identity changed inside one step trace");
      }
      if (!sameArray(previous.scopeAfter, evidence.scopeBefore)) {
        fail("Scope chain breaks before reaction " + index);
      }
      if (previous.linksAfter !== evidence.linksBefore) {
        fail("persistent Link-count chain breaks before reaction " + index);
      }
    }

    if (!verified.quiescent) activeReactionCount += 1;
    if (report.activeReactionCount !== activeReactionCount) {
      fail("activeReactionCount mismatch at report " + index);
    }

    const isCompleted = report.completed === true;
    if (isCompleted) {
      if (index !== reports.length - 1 || !verified.quiescent) {
        fail("completed report must be the final quiescent reaction");
      }
      if (!report.result || typeof report.result !== "object") {
        fail("completed trace must expose a final result");
      }
      completed = true;
    } else if (report.result != null) {
      fail("nonterminal reaction must not expose a final result");
    }
  }

  return {
    schemaVersion: TRACE_VERIFIER_SCHEMA_VERSION,
    traceConsistent: true,
    reactions: reports.length,
    activeReactionCount,
    createdLinks,
    completed,
    sessionId,
    sessionRunId,
    finalScope: [...reports.at(-1).evidence.scopeAfter],
  };
}
