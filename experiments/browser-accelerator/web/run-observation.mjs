export const NORMALIZED_RUN_OBSERVATION_SCHEMA_VERSION = 2;

export const METRIC_AVAILABILITY = Object.freeze({
  MEASURED: "MEASURED",
  UNAVAILABLE: "UNAVAILABLE",
  UNSUPPORTED: "UNSUPPORTED",
});

function freezeMetric(availability, value, reason = null) {
  if (!Object.values(METRIC_AVAILABILITY).includes(availability)) {
    throw new TypeError("invalid metric availability: " + availability);
  }
  if (availability === METRIC_AVAILABILITY.MEASURED) {
    if (value == null) throw new TypeError("MEASURED metric requires a value");
    return Object.freeze({ availability, value });
  }
  if (value != null) {
    throw new TypeError(
      availability + " metric must not carry a value surrogate",
    );
  }
  return Object.freeze({
    availability,
    value: null,
    ...(reason ? { reason } : {}),
  });
}

export function measured(value) {
  return freezeMetric(METRIC_AVAILABILITY.MEASURED, value);
}

export function unavailable(reason = null) {
  return freezeMetric(METRIC_AVAILABILITY.UNAVAILABLE, null, reason);
}

export function unsupported(reason = null) {
  return freezeMetric(METRIC_AVAILABILITY.UNSUPPORTED, null, reason);
}

function observed(value, reason = "runtime did not expose this measurement") {
  return value == null ? unavailable(reason) : measured(value);
}

function timed(value, timingAvailable, reason) {
  return timingAvailable === true && value != null
    ? measured(value)
    : unavailable(reason);
}

function numericFieldsAsMetrics(source, timingAvailable = null) {
  if (!source || typeof source !== "object") return Object.freeze({});
  const projected = {};
  for (const [key, value] of Object.entries(source)) {
    if (key === "timingAvailable" || typeof value !== "number") continue;
    if (key.endsWith("Ns") && timingAvailable === false) {
      projected[key] = unavailable("timing clock unavailable on this target");
    } else {
      projected[key] = measured(value);
    }
  }
  return Object.freeze(projected);
}

function normalizedEvent({
  sessionId,
  runId,
  backendId,
  sequence,
  stage,
  kind,
  reactionIndex = null,
  elapsedNs,
  payload = {},
  structuralFacts = null,
}) {
  return Object.freeze({
    schemaVersion: NORMALIZED_RUN_OBSERVATION_SCHEMA_VERSION,
    sessionId,
    runId,
    backendId,
    sequence,
    stage,
    kind,
    reactionIndex,
    elapsedNs,
    payload: Object.freeze(payload),
    structuralFacts,
  });
}

function normalizedProfile({
  sessionId,
  runId,
  manifestRunId,
  backendId,
  configurationReused,
  finalQuiescent,
  stopReason,
  links,
  activeReactionCount,
  timings,
  structural,
  resource,
}) {
  return Object.freeze({
    schemaVersion: NORMALIZED_RUN_OBSERVATION_SCHEMA_VERSION,
    sessionId,
    runId,
    manifestRunId,
    backendId,
    configurationReused,
    finalQuiescent,
    stopReason,
    links: Object.freeze(links),
    activeReactionCount,
    timings: Object.freeze(timings),
    structural: Object.freeze(structural),
    resource: Object.freeze(resource),
  });
}

function ensureRunIdentity({ sessionId, runId, manifestRunId, backendId }) {
  if (typeof sessionId !== "string" || sessionId.length === 0) {
    throw new TypeError("normalized observation requires sessionId");
  }
  if (!Number.isInteger(runId) || runId < 1) {
    throw new TypeError("normalized observation requires positive integer runId");
  }
  if (typeof manifestRunId !== "string" || manifestRunId.length === 0) {
    throw new TypeError("normalized observation requires manifestRunId");
  }
  if (typeof backendId !== "string" || backendId.length === 0) {
    throw new TypeError("normalized observation requires backendId");
  }
}

export function normalizeCpuScenarioRunV2(run) {
  const observedRun = run?.observed;
  const profile = observedRun?.profile;
  const pipeline = run?.pipelineProfile;
  const sessionId = observedRun?.sessionId;
  const runId = Number(observedRun?.runId ?? run?.sessionRunId);
  const manifestRunId = String(run?.manifestRunId ?? "");
  const backendId = String(observedRun?.backendId ?? "optimized-cpu");
  ensureRunIdentity({ sessionId, runId, manifestRunId, backendId });

  const timingAvailable =
    observedRun?.timingAvailable === true &&
    profile?.timingAvailable === true;
  const stageTimingAvailable = pipeline?.stages?.timingAvailable === true;
  const structuralTimingAvailable = profile?.structural?.timingAvailable === true;

  const events = (Array.isArray(observedRun?.events)
    ? observedRun.events
    : []).map((event, index) => normalizedEvent({
      sessionId,
      runId,
      backendId,
      sequence: Number.isInteger(event?.sequence) ? event.sequence : index,
      stage: event?.stage ?? "EXECUTE",
      kind: event?.kind ?? "REACTION_END",
      reactionIndex: event?.reactionIndex ?? null,
      elapsedNs: timed(
        event?.elapsedNs,
        observedRun?.timingAvailable === true,
        "event clock unavailable on this target",
      ),
      payload: {
        linksAfter: observed(event?.linksAfter),
        rawRuleMatches: observed(event?.rawRuleMatches),
        transitionedMembers: observed(event?.transitionedMembers),
        handoffCount: observed(event?.handoffCount),
        quiescent: observed(event?.quiescent),
      },
      structuralFacts: event?.structuralFacts ?? null,
    }));

  const normalized = Object.freeze({
    schemaVersion: NORMALIZED_RUN_OBSERVATION_SCHEMA_VERSION,
    events: Object.freeze(events),
    profile: normalizedProfile({
      sessionId,
      runId,
      manifestRunId,
      backendId,
      configurationReused: run?.configurationReused === true,
      finalQuiescent: observedRun?.finalQuiescent === true,
      stopReason:
        observedRun?.finalQuiescent === true ? "QUIESCENT" : "UNAVAILABLE",
      links: {
        base: observed(profile?.baseLinks),
        beforeConfigure: observed(run?.linksBeforeConfigure),
        afterConfigure: observed(run?.linksAfterConfigure),
        afterExecute: observed(
          pipeline?.linksAfterExecute ?? profile?.linksAfterRun,
        ),
      },
      activeReactionCount: observed(observedRun?.activeReactionCount),
      timings: {
        configureNs: timed(
          pipeline?.stages?.configureNs,
          stageTimingAvailable,
          "configure timing unavailable on this target",
        ),
        executeNs: timed(
          profile?.executeNs,
          timingAvailable,
          "execute timing unavailable on this target",
        ),
        resultProjectionNs: timed(
          pipeline?.stages?.resultNs,
          stageTimingAvailable,
          "result projection timing unavailable on this target",
        ),
        traceProjectionNs: timed(
          profile?.traceProjectionNs,
          timingAvailable,
          "trace projection timing unavailable on this target",
        ),
        dispatchKernelNs: unsupported("optimized-cpu has no WebGPU kernel"),
        readbackNs: unsupported("optimized-cpu has no WebGPU readback"),
      },
      structural: numericFieldsAsMetrics(
        profile?.structural,
        structuralTimingAvailable,
      ),
      resource: {
        baseUploadCount: unsupported("optimized-cpu has no GPU base upload"),
        baseUploadBytes: unsupported("optimized-cpu has no GPU base upload"),
        configurationUploadBytes: unsupported(
          "optimized-cpu has no GPU configuration upload",
        ),
        reactionDispatchCount: unsupported(
          "optimized-cpu has no GPU dispatch count",
        ),
        residentBufferBytes: unsupported(
          "optimized-cpu has no GPU resident buffer",
        ),
        readbackBytes: unsupported("optimized-cpu has no GPU readback"),
        denseCarrierAllocatedBytes: observed(
          profile?.denseCarrierAllocatedBytes,
          "dense carrier allocation was not reported",
        ),
        maxDenseCarrierBytes: observed(
          profile?.maxDenseCarrierBytes,
          "dense carrier budget was not reported",
        ),
      },
    }),
    readback: Object.freeze({
      finalScope: Object.freeze([...(observedRun?.finalScope ?? [])]),
      result: run?.result ?? null,
      stepCount: observedRun?.events?.length ?? null,
    }),
  });

  assertNormalizedRunObservationV2(normalized);
  return normalized;
}

export function normalizeWebGpuResidentRunV2({
  sessionId,
  runId,
  manifestRunId,
  configurationReused,
  baseLinkCount,
  linksBeforeConfigure,
  linksAfterConfigure,
  run,
  telemetryBeforeConfigure,
  telemetryAfterConfigure,
  telemetryBeforeExecute,
  telemetryAfterExecute,
  result = null,
}) {
  const backendId = "webgpu";
  ensureRunIdentity({ sessionId, runId, manifestRunId, backendId });

  const steps = Array.isArray(run?.steps) ? run.steps : [];
  const events = steps.map((step, index) => {
    const fact = step?.observed ?? {};
    return normalizedEvent({
      sessionId,
      runId,
      backendId,
      sequence: index,
      stage: "EXECUTE",
      kind: "REACTION_END",
      reactionIndex: index,
      elapsedNs: unavailable(
        "per-reaction WebGPU timing is not measured by this backend",
      ),
      payload: {
        linksAfter: observed(
          Number.isInteger(fact?.residentAppendCount)
            ? baseLinkCount + fact.residentAppendCount
            : null,
        ),
        rawRuleMatches: observed(fact?.rawRuleMatches),
        appendCount: observed(fact?.appendCount),
        currentHandle: observed(fact?.currentHandle),
        publishedHandle: observed(fact?.publishedHandle),
        ruleHandle: observed(fact?.ruleHandle),
        groundedBundle: observed(fact?.groundedBundle),
        quiescent: observed(fact?.quiescent),
      },
      structuralFacts: null,
    });
  });

  events.push(normalizedEvent({
    sessionId,
    runId,
    backendId,
    sequence: events.length,
    stage: "EXECUTE",
    kind: run?.stopReason === "QUIESCENT" ? "QUIESCENCE" : "RUN_END",
    reactionIndex: null,
    elapsedNs: unavailable(
      "run-level WebGPU event clock is not measured by this backend",
    ),
    payload: {
      stopReason: observed(run?.stopReason),
      quiescent: observed(run?.quiescent),
      finalCurrentHandle: observed(run?.finalCurrentHandle),
    },
  }));

  const configUploadBefore =
    telemetryBeforeConfigure?.configurationUploadBytes;
  const configUploadAfter =
    telemetryAfterConfigure?.configurationUploadBytes;
  const dispatchBefore = telemetryBeforeExecute?.reactionDispatchCount;
  const dispatchAfter = telemetryAfterExecute?.reactionDispatchCount;

  const normalized = Object.freeze({
    schemaVersion: NORMALIZED_RUN_OBSERVATION_SCHEMA_VERSION,
    events: Object.freeze(events),
    profile: normalizedProfile({
      sessionId,
      runId,
      manifestRunId,
      backendId,
      configurationReused: configurationReused === true,
      finalQuiescent: run?.quiescent === true,
      stopReason: run?.stopReason ?? "UNAVAILABLE",
      links: {
        base: observed(baseLinkCount),
        beforeConfigure: observed(linksBeforeConfigure),
        afterConfigure: observed(linksAfterConfigure),
        afterExecute: observed(
          Number.isInteger(telemetryAfterExecute?.residentAppendCount)
            ? baseLinkCount + telemetryAfterExecute.residentAppendCount
            : null,
        ),
      },
      activeReactionCount: observed(run?.activeReactionCount),
      timings: {
        configureNs: unavailable(
          "WebGPU configure timing is not measured by this backend",
        ),
        executeNs: unavailable(
          "WebGPU execute timing is not measured by this backend",
        ),
        resultProjectionNs: unavailable(
          "WebGPU result projection timing is not measured by this backend",
        ),
        traceProjectionNs: unsupported(
          "WebGPU backend does not expose CPU structural trace projection",
        ),
        dispatchKernelNs: unavailable(
          "GPU timestamp queries are not collected",
        ),
        readbackNs: unavailable(
          "WebGPU readback timing is not separately measured",
        ),
      },
      structural: {},
      resource: {
        baseUploadCount: observed(telemetryAfterExecute?.baseUploadCount),
        baseUploadBytes: observed(telemetryAfterExecute?.baseUploadBytes),
        configurationUploadBytes:
          Number.isInteger(configUploadBefore) &&
          Number.isInteger(configUploadAfter)
            ? measured(configUploadAfter - configUploadBefore)
            : unavailable("configuration upload byte delta unavailable"),
        reactionDispatchCount:
          Number.isInteger(dispatchBefore) && Number.isInteger(dispatchAfter)
            ? measured(dispatchAfter - dispatchBefore)
            : unavailable("reaction dispatch delta unavailable"),
        residentBufferBytes: observed(
          telemetryAfterExecute?.residentBufferBytes,
        ),
        readbackBytes: unavailable(
          "readback byte accounting is not instrumented yet",
        ),
        denseCarrierAllocatedBytes: unsupported(
          "WebGPU does not use optimized-cpu dense carrier accounting",
        ),
        maxDenseCarrierBytes: unsupported(
          "WebGPU does not use optimized-cpu dense carrier budget",
        ),
      },
    }),
    readback: Object.freeze({
      finalScope: Object.freeze(
        Number.isInteger(run?.finalCurrentHandle)
          ? [run.finalCurrentHandle]
          : [],
      ),
      result,
      stepCount: steps.length,
    }),
  });

  assertNormalizedRunObservationV2(normalized);
  return normalized;
}

export function assertNormalizedRunObservationV2(observation) {
  if (observation?.schemaVersion !==
      NORMALIZED_RUN_OBSERVATION_SCHEMA_VERSION) {
    throw new TypeError("normalized observation schema version mismatch");
  }
  if (!Array.isArray(observation?.events)) {
    throw new TypeError("normalized observation events must be an array");
  }
  if (observation?.profile?.schemaVersion !==
      NORMALIZED_RUN_OBSERVATION_SCHEMA_VERSION) {
    throw new TypeError("normalized profile schema version mismatch");
  }

  const visit = (value, path = "observation") => {
    if (!value || typeof value !== "object") return;
    if (Object.prototype.hasOwnProperty.call(value, "availability")) {
      const availability = value.availability;
      if (!Object.values(METRIC_AVAILABILITY).includes(availability)) {
        throw new TypeError("invalid metric availability at " + path);
      }
      if (availability === METRIC_AVAILABILITY.MEASURED) {
        if (value.value == null) {
          throw new TypeError("MEASURED metric has no value at " + path);
        }
      } else if (value.value !== null) {
        throw new TypeError(
          availability + " metric uses a value surrogate at " + path,
        );
      }
    }
    for (const [key, child] of Object.entries(value)) {
      visit(child, path + "." + key);
    }
  };
  visit(observation);
  return true;
}
