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


function requireContextId(value, label) {
  const id = requireUint(value, label);
  if (id === 0) fail(label + " must be non-zero");
  return id;
}

function requireContextIdArray(value, label) {
  if (value == null) return [];
  if (!Array.isArray(value)) fail(label + " must be an array");
  const ids = value.map((item, index) =>
    requireContextId(item, label + "[" + index + "]"));
  for (let index = 1; index < ids.length; index += 1) {
    if (ids[index - 1] >= ids[index]) {
      fail(label + " must be strictly increasing and duplicate-free");
    }
  }
  return ids;
}

function addContextParents(target, handle, ids) {
  const current = target.get(handle) ?? [];
  const merged = new Set(current);
  for (const id of ids) merged.add(id);
  target.set(handle, [...merged].sort((left, right) => left - right));
}

function verifyContextLifecycle(
  facts,
  matched,
  instantiated,
  activePublished,
  rawRuleMatches,
) {
  const created = facts.filter((fact) => fact.kind === "CONTEXT_CREATED");
  const updated = facts.filter((fact) => fact.kind === "CONTEXT_UPDATED");
  const contextPublished = facts.filter((fact) =>
    fact.kind === "CONTEXT_PUBLISHED");
  const collapsed = facts.filter((fact) => fact.kind === "CONTEXT_COLLAPSED");

  if (created.length !== rawRuleMatches ||
      updated.length !== rawRuleMatches ||
      contextPublished.length !== rawRuleMatches ||
      collapsed.length !== rawRuleMatches) {
    fail("Context lifecycle counts disagree with rawRuleMatches");
  }
  if (rawRuleMatches === 0) {
    return { count: 0, contexts: [] };
  }

  const states = new Map();
  const groupIds = new Set();
  for (const fact of facts) {
    if (fact.kind === "CONTEXT_CREATED") {
      const id = requireContextId(fact.contextId, "CONTEXT_CREATED contextId");
      const groupId = requireContextId(
        fact.contextGroupId,
        "CONTEXT_CREATED contextGroupId",
      );
      if (states.has(id)) fail("duplicate Context id " + id);
      const parentContextIds = requireContextIdArray(
        fact.parentContextIds,
        "CONTEXT_CREATED parentContextIds",
      );
      states.set(id, {
        id,
        state: "CREATED",
        groupId,
        parentContextIds,
        active: requireHandle(fact.active, "CONTEXT_CREATED active"),
        rule: requireHandle(fact.rule, "CONTEXT_CREATED rule"),
        outputBundleTemplate: requireHandle(
          fact.outputBundleTemplate,
          "CONTEXT_CREATED outputBundleTemplate",
        ),
        groundedBundle: null,
        outputs: null,
      });
      groupIds.add(groupId);
      continue;
    }

    if (fact.kind === "CONTEXT_UPDATED") {
      const id = requireContextId(fact.contextId, "CONTEXT_UPDATED contextId");
      const state = states.get(id);
      if (!state || state.state !== "CREATED") {
        fail("Context " + id + " updated outside CREATED state");
      }
      if (requireContextId(
        fact.contextGroupId,
        "CONTEXT_UPDATED contextGroupId",
      ) !== state.groupId) {
        fail("Context " + id + " changed atomic group");
      }
      state.groundedBundle = requireHandle(
        fact.groundedBundle,
        "CONTEXT_UPDATED groundedBundle",
      );
      state.state = "UPDATED";
      continue;
    }

    if (fact.kind === "CONTEXT_PUBLISHED") {
      const id = requireContextId(
        fact.contextId,
        "CONTEXT_PUBLISHED contextId",
      );
      const state = states.get(id);
      if (!state || state.state !== "UPDATED") {
        fail("Context " + id + " published outside UPDATED state");
      }
      if (requireContextId(
        fact.contextGroupId,
        "CONTEXT_PUBLISHED contextGroupId",
      ) !== state.groupId) {
        fail("Context " + id + " changed atomic group");
      }
      state.outputs = requireHandleArray(
        fact.outputs,
        "CONTEXT_PUBLISHED outputs",
      );
      state.state = "PUBLISHED";
      continue;
    }

    if (fact.kind === "CONTEXT_COLLAPSED") {
      const id = requireContextId(
        fact.contextId,
        "CONTEXT_COLLAPSED contextId",
      );
      const state = states.get(id);
      if (!state || state.state !== "PUBLISHED") {
        fail("Context " + id + " collapsed outside PUBLISHED state");
      }
      if (requireContextId(
        fact.contextGroupId,
        "CONTEXT_COLLAPSED contextGroupId",
      ) !== state.groupId) {
        fail("Context " + id + " changed atomic group");
      }
      state.state = "COLLAPSED";
    }
  }

  if (states.size !== rawRuleMatches ||
      [...states.values()].some((state) => state.state !== "COLLAPSED")) {
    fail("not every Context completed create/update/publish/collapse");
  }
  if (groupIds.size !== 1) {
    fail("sibling Contexts do not share one atomic reaction group");
  }

  const createdKeys = countByKey(created, (fact) =>
    requireHandle(fact.active, "CONTEXT_CREATED active") + ":" +
    requireHandle(fact.rule, "CONTEXT_CREATED rule"));
  const matchedKeys = countByKey(matched, (fact) =>
    requireHandle(fact.active, "RULE_MATCHED active") + ":" +
    requireHandle(fact.rule, "RULE_MATCHED rule"));
  if (!sameCounts(createdKeys, matchedKeys)) {
    fail("Context creation does not match RuleMatched identities");
  }

  const contextGroundedKeys = countByKey(
    [...states.values()],
    (state) => state.active + ":" + state.rule + ":" + state.groundedBundle,
  );
  const instantiatedKeys = countByKey(instantiated, (fact) =>
    requireHandle(fact.active, "INSTANTIATED active") + ":" +
    requireHandle(fact.rule, "INSTANTIATED rule") + ":" +
    requireHandle(fact.groundedBundle, "INSTANTIATED groundedBundle"));
  if (!sameCounts(contextGroundedKeys, instantiatedKeys)) {
    fail("Context grounding disagrees with persistent instantiation");
  }

  const contextPublishKeys = countByKey(
    [...states.values()],
    (state) => state.active + ":" + state.rule + ":" +
      JSON.stringify(state.outputs),
  );
  const persistentPublishKeys = countByKey(activePublished, (fact) =>
    requireHandle(fact.active, "PUBLISHED active") + ":" +
    requireHandle(fact.rule, "PUBLISHED rule") + ":" +
    JSON.stringify(requireHandleArray(fact.outputs, "PUBLISHED outputs")));
  if (!sameCounts(contextPublishKeys, persistentPublishKeys)) {
    fail("Context publication disagrees with persistent publication");
  }

  return {
    count: states.size,
    contexts: [...states.values()].map((state) => ({
      id: state.id,
      groupId: state.groupId,
      parentContextIds: [...state.parentContextIds],
      active: state.active,
      rule: state.rule,
      groundedBundle: state.groundedBundle,
      outputs: [...state.outputs],
    })),
  };
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
    "CONTEXT_CREATED",
    "INSTANTIATED",
    "CONTEXT_UPDATED",
    "PUBLISHED",
    "CONTEXT_PUBLISHED",
    "CONTEXT_COLLAPSED",
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
  const contextLifecycle = verifyContextLifecycle(
    facts,
    matched,
    instantiated,
    activePublished,
    rawRuleMatches,
  );
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
    contexts: contextLifecycle.count,
    contextLineage: contextLifecycle.contexts,
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
  let expectedParentsByActive = new Map();
  const seenContextIds = new Set();
  let previousContextId = null;
  let previousContextGroupId = null;

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

    if (verified.contextLineage.length > 0) {
      const reactionGroupId = verified.contextLineage[0].groupId;
      if (previousContextGroupId != null &&
          reactionGroupId !== previousContextGroupId + 1) {
        fail("Context group ids are not contiguous across reactions");
      }
      for (const context of verified.contextLineage) {
        if (context.groupId !== reactionGroupId) {
          fail("one reaction exposed multiple Context groups");
        }
        if (seenContextIds.has(context.id)) {
          fail("Context id was reused inside one execution run");
        }
        if (previousContextId != null && context.id !== previousContextId + 1) {
          fail("Context ids are not contiguous across reactions");
        }
        const expectedParents =
          expectedParentsByActive.get(context.active) ?? [];
        if (!sameArray(context.parentContextIds, expectedParents)) {
          fail(
            "Context parent provenance mismatch for active " +
            context.active,
          );
        }
        for (const parentContextId of context.parentContextIds) {
          if (!seenContextIds.has(parentContextId)) {
            fail("Context references an unseen/future parent");
          }
        }
        seenContextIds.add(context.id);
        previousContextId = context.id;
      }
      previousContextGroupId = reactionGroupId;
    }

    const nextParentsByActive = new Map();
    const facts = evidence.structuralFacts;
    for (const fact of facts) {
      if (fact.kind === "PUBLISHED" && fact.preserved === true) {
        const active = requireHandle(fact.active, "preserved active");
        addContextParents(
          nextParentsByActive,
          active,
          expectedParentsByActive.get(active) ?? [],
        );
      }
    }
    for (const context of verified.contextLineage) {
      for (const output of context.outputs) {
        addContextParents(nextParentsByActive, output, [context.id]);
      }
    }
    for (const active of evidence.scopeAfter) {
      if (!nextParentsByActive.has(active)) {
        nextParentsByActive.set(active, []);
      }
    }
    expectedParentsByActive = nextParentsByActive;

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


function requireCompactTopologyColumn(value, label) {
  if (!Array.isArray(value)) fail(label + " must be an array");
  return value.map((item, index) =>
    requireHandle(item, label + "[" + index + "]"));
}

function compactTopologyColumns(compact) {
  const topology = requireObject(compact.topology, "compact.topology");
  const base = requireObject(topology.base, "compact.topology.base");
  const append = requireObject(topology.append, "compact.topology.append");
  const baseStarts = requireCompactTopologyColumn(
    base.starts,
    "compact.topology.base.starts",
  );
  const baseEnds = requireCompactTopologyColumn(
    base.ends,
    "compact.topology.base.ends",
  );
  const appendStarts = requireCompactTopologyColumn(
    append.starts,
    "compact.topology.append.starts",
  );
  const appendEnds = requireCompactTopologyColumn(
    append.ends,
    "compact.topology.append.ends",
  );
  if (baseStarts.length !== baseEnds.length ||
      appendStarts.length !== appendEnds.length) {
    fail("compact topology columns have different lengths");
  }
  return {
    starts: baseStarts.concat(appendStarts),
    ends: baseEnds.concat(appendEnds),
  };
}

export function verifyLiveTraceAgainstCompactProof(reports, rawCompactProof) {
  const live = verifyLiveStepTrace(reports);
  if (!live.completed) {
    fail("compact cross-check requires a completed live trace");
  }

  const compact = requireObject(rawCompactProof, "compact proof");
  if (compact.schemaVersion !== 2 ||
      compact.representationId !== "amemory-proof-compact-json" ||
      compact.representationVersion !== "0.2.0") {
    fail("unsupported compact proof representation");
  }

  const execute = requireObject(compact.execute, "compact.execute");
  if (!Array.isArray(execute.reactions) || execute.reactions.length === 0) {
    fail("compact execution trace is empty");
  }
  if (execute.reactions.length !== reports.length) {
    fail("live/compact reaction counts differ");
  }
  if (requireUint(
    execute.activeReactionCount,
    "compact.execute.activeReactionCount",
  ) !== live.activeReactionCount) {
    fail("live/compact active reaction counts differ");
  }
  if (execute.finalQuiescent !== true) {
    fail("compact execution is not final-quiescent");
  }

  for (let index = 0; index < reports.length; index += 1) {
    const evidence = reports[index].evidence;
    const recorded = requireObject(
      execute.reactions[index],
      "compact.execute.reactions[" + index + "]",
    );
    const scopeBefore = requireHandleArray(
      recorded.scopeBefore,
      "compact reaction scopeBefore",
    );
    const scopeAfter = requireHandleArray(
      recorded.scopeAfter,
      "compact reaction scopeAfter",
    );
    if (recorded.step !== index ||
        !sameArray(scopeBefore, evidence.scopeBefore) ||
        !sameArray(scopeAfter, evidence.scopeAfter) ||
        recorded.rawRuleMatches !== evidence.rawRuleMatches ||
        recorded.transitionedMembers !== evidence.transitionedMembers ||
        recorded.handoffCount !== evidence.handoffCount ||
        recorded.linksAfter !== evidence.linksAfter ||
        recorded.quiescent !== evidence.quiescent) {
      fail("live/compact reaction mismatch at " + index);
    }
  }

  const columns = compactTopologyColumns(compact);
  const result = requireObject(compact.result, "compact.result");
  const finalEvidence = reports.at(-1).evidence;
  const linksFinal = requireUint(result.linksFinal, "compact.result.linksFinal");
  if (columns.starts.length !== linksFinal ||
      columns.ends.length !== linksFinal ||
      finalEvidence.linksAfter !== linksFinal) {
    fail("live/compact final persistent Store size differs");
  }

  let comparedCreatedLinks = 0;
  for (const report of reports) {
    for (const fact of report.evidence.structuralFacts) {
      if (fact.kind !== "INSTANTIATED") continue;
      for (const link of fact.createdLinks) {
        const handle = requireHandle(link.handle, "live created Link handle");
        if (handle > linksFinal ||
            columns.starts[handle - 1] !== link.start ||
            columns.ends[handle - 1] !== link.end) {
          fail("live created Link L" + handle + " differs from compact topology");
        }
        comparedCreatedLinks += 1;
      }
    }
  }
  if (comparedCreatedLinks !== live.createdLinks) {
    fail("live/compact created-Link accounting differs");
  }

  const compactFinalScope = requireHandleArray(
    execute.reactions.at(-1).scopeAfter,
    "compact final scope",
  );
  if (!sameArray(live.finalScope, compactFinalScope)) {
    fail("live/compact final Scope differs");
  }

  return {
    schemaVersion: TRACE_VERIFIER_SCHEMA_VERSION,
    traceConsistent: true,
    compactCrossConsistent: true,
    reactions: reports.length,
    activeReactionCount: live.activeReactionCount,
    comparedCreatedLinks,
    finalLinks: linksFinal,
    finalScope: [...live.finalScope],
  };
}
