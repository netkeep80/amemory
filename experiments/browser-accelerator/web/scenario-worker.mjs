import {
  beginScenarioLiveStepRun,
  closeScenarioLiveSession,
  openScenarioLiveSession,
  readScenarioTransportLimits,
  refreshScenarioLiveHistoryStatus,
  setScenarioLiveRetentionPolicy,
  stepScenarioLiveSession,
} from "./scenario-transport.mjs";

export const SCENARIO_WORKER_PROTOCOL_VERSION = 1;

const BUDGET_STOP_REASONS = new Set([
  "REACTION_BUDGET_EXCEEDED",
  "APPENDED_LINKS_BUDGET_EXCEEDED",
  "TOTAL_LINKS_BUDGET_EXCEEDED",
  "SCOPE_WIDTH_BUDGET_EXCEEDED",
  "MATCH_WORK_BUDGET_EXCEEDED",
  "UNIFICATION_WORK_BUDGET_EXCEEDED",
  "INSTANTIATION_WORK_BUDGET_EXCEEDED",
  "CARRIER_BYTES_BUDGET_EXCEEDED",
]);

let wasm = null;
let sessionOpen = false;
let sessionId = null;
let workerState = "BOOT";
let busyRequestId = null;
let cancelRequested = false;

const clone = (value) => value == null
  ? value
  : JSON.parse(JSON.stringify(value));

const boundaryYield = () =>
  new Promise((resolve) => setTimeout(resolve, 0));

const monotonicNowMs = () =>
  globalThis.performance?.now?.() ?? Date.now();

function normalizeWallClockLimitMs(value) {
  if (value == null) return null;
  if (!Number.isFinite(value) || value < 0) {
    throw protocolError(
      "INVALID_WALL_CLOCK_LIMIT",
      "wallClockLimitMs must be a finite non-negative number",
    );
  }
  return value;
}

function post(message) {
  globalThis.postMessage({
    schemaVersion: SCENARIO_WORKER_PROTOCOL_VERSION,
    ...message,
  });
}

function workerStatus() {
  return {
    state: workerState,
    sessionOpen,
    sessionId,
    busyRequestId,
    cancelRequested,
  };
}

function errorRecord(
  code,
  message,
  recovery = "KEEP_SESSION",
  details = null,
) {
  return {
    code,
    message,
    recovery,
    details,
    atReactionBoundary: false,
  };
}

function normalizeTransportError(error, runStarted = false) {
  const runner = error?.code === "RUNNER" ? error.error : null;
  const stopReason =
    runner?.stop_reason ?? runner?.stopReason ?? null;

  if (error?.code === "LIVE_SESSION_NOT_OPEN") {
    return errorRecord(
      "SESSION_NOT_OPEN",
      "Retained Session is not open",
      "OPEN_REQUIRED",
      error,
    );
  }

  if (runner?.code === "VALIDATION") {
    return errorRecord(
      "INVALID_INPUT",
      "Scenario input validation failed before CONFIGURE",
      "KEEP_SESSION",
      error,
    );
  }

  if (runner?.code === "EXECUTE_FAILED" &&
      BUDGET_STOP_REASONS.has(stopReason)) {
    return {
      ...errorRecord(
        "BUDGET_EXCEEDED",
        "Runtime budget stopped execution: " + stopReason,
        "REOPEN_REQUIRED",
        error,
      ),
      stopReason,
      atReactionBoundary: true,
    };
  }

  if (runner?.code === "EXECUTE_FAILED") {
    return {
      ...errorRecord(
        "EXECUTION_FAILED",
        "Runtime execution failed: " + (stopReason || "unknown stop"),
        "REOPEN_REQUIRED",
        error,
      ),
      stopReason,
      atReactionBoundary: true,
    };
  }

  return errorRecord(
    runStarted ? "EXECUTION_FAILED" : "REQUEST_FAILED",
    runStarted
      ? "Execution failed after the run started"
      : "Worker request failed",
    runStarted ? "REOPEN_REQUIRED" : "KEEP_SESSION",
    error,
  );
}

function protocolError(code, message, recovery = "KEEP_SESSION", details = null) {
  const error = new Error(message);
  error.workerError = errorRecord(code, message, recovery, details);
  return error;
}

function throwTransport(error, runStarted = false) {
  const normalized = normalizeTransportError(error, runStarted);
  const wrapped = new Error(normalized.message);
  wrapped.workerError = normalized;
  throw wrapped;
}

async function ensureWasm() {
  if (wasm !== null) {
    return { wasm, limits: readScenarioTransportLimits(wasm) };
  }

  workerState = "INITIALIZING";
  const wasmUrl = new URL("./amemory_a_circuit.wasm", import.meta.url);
  const response = await fetch(wasmUrl, { cache: "no-store" });
  if (!response.ok) {
    workerState = "FAILED";
    throw protocolError(
      "WASM_LOAD_FAILED",
      "A-Circuit WASM HTTP " + response.status,
      "RELOAD_WORKER",
    );
  }

  const bytes = await response.arrayBuffer();
  const loaded = await WebAssembly.instantiate(bytes, {});
  wasm = loaded.instance.exports;
  const limits = readScenarioTransportLimits(wasm);
  workerState = "READY";
  return { wasm, limits };
}

function closeOwnedSession(nextState = "CLOSED") {
  if (wasm !== null && sessionOpen) {
    closeScenarioLiveSession(wasm);
  }
  sessionOpen = false;
  sessionId = null;
  cancelRequested = false;
  workerState = nextState;
}

function setDefaultRetention() {
  const limits = readScenarioTransportLimits(wasm);
  const cap = limits.liveObserver;
  const policy = setScenarioLiveRetentionPolicy(wasm, {
    retentionMode: "RING",
    maxRetainedRuns: Math.min(8, cap.maxRetainedRuns),
    maxRetainedEvents: Math.min(8192, cap.maxRetainedEvents),
    maxRetainedBytes: Math.min(8 * 1024 * 1024, cap.maxRetainedBytes),
  });
  if (!policy.ok) throwTransport(policy.error, false);
  return limits;
}

async function openSession(payload) {
  await ensureWasm();
  if (sessionOpen) {
    throw protocolError(
      "SESSION_ALREADY_OPEN",
      "Worker already owns a retained Session",
      "CLOSE_FIRST",
      workerStatus(),
    );
  }

  const opened = openScenarioLiveSession(
    wasm,
    payload.manifest,
    payload.backend || "optimized-cpu",
  );
  if (!opened.ok) throwTransport(opened.error, false);

  sessionOpen = true;
  sessionId = opened.status?.sessionId ?? null;
  workerState = "OPEN";

  try {
    const limits = setDefaultRetention();
    const history = refreshScenarioLiveHistoryStatus(wasm);
    return {
      status: opened.status,
      limits,
      history: history.ok ? history.status : null,
      worker: workerStatus(),
    };
  } catch (error) {
    closeOwnedSession("FAILED");
    throw error;
  }
}

function requireOpenSession() {
  if (!sessionOpen) {
    throw protocolError(
      "SESSION_NOT_OPEN",
      "Retained Session is not open",
      "OPEN_REQUIRED",
      workerStatus(),
    );
  }
}

async function runToBoundaryCompletion(requestId, payload) {
  requireOpenSession();
  cancelRequested = false;

  const wallClockLimitMs =
    normalizeWallClockLimitMs(payload.wallClockLimitMs);
  const hostStartedMs = monotonicNowMs();

  const run = clone(payload.run);
  run.executionMode = "STEP";

  const begun = beginScenarioLiveStepRun(wasm, run);
  if (!begun.ok) throwTransport(begun.error, false);

  workerState = "RUNNING";
  const reports = [];
  const begin = begun.payload.begin;
  let latestStatus = begun.payload.status;

  const abortIfDeadlineExpired = () => {
    if (wallClockLimitMs === null) return;
    const elapsedMs = Math.max(0, monotonicNowMs() - hostStartedMs);
    if (elapsedMs < wallClockLimitMs) return;

    const completedSteps = reports.length;
    closeOwnedSession("WALL_CLOCK_SAFETY_ABORT");
    const aborted = {
      ...errorRecord(
        "WALL_CLOCK_SAFETY_ABORT",
        "Host wall-clock safety deadline expired at a reaction boundary",
        "REOPEN_REQUIRED",
        {
          completedSteps,
          wallClockLimitMs,
          elapsedMs,
          hardAbort: false,
        },
      ),
      atReactionBoundary: true,
    };
    const error = new Error(aborted.message);
    error.workerError = aborted;
    throw error;
  };

  post({
    type: "PROGRESS",
    requestId,
    progress: {
      phase: "RUN_BEGUN",
      sessionId,
      sessionRunId: begin?.sessionRunId ?? null,
      completedSteps: 0,
      status: latestStatus,
    },
  });

  // Before the first reaction is a safe boundary as well.
  await boundaryYield();

  while (true) {
    if (cancelRequested) {
      const completedSteps = reports.length;
      closeOwnedSession("CANCELLED");
      const cancelled = {
        ...errorRecord(
          "CANCELLED",
          "Execution cancelled at a reaction boundary",
          "REOPEN_REQUIRED",
          { completedSteps },
        ),
        atReactionBoundary: true,
      };
      const error = new Error(cancelled.message);
      error.workerError = cancelled;
      throw error;
    }

    abortIfDeadlineExpired();

    const stepped = stepScenarioLiveSession(wasm);
    if (!stepped.ok) {
      const normalized = normalizeTransportError(stepped.error, true);
      closeOwnedSession(
        normalized.code === "BUDGET_EXCEEDED"
          ? "BUDGET_EXCEEDED"
          : "FAILED",
      );
      const error = new Error(normalized.message);
      error.workerError = normalized;
      throw error;
    }

    latestStatus = stepped.payload.status;
    reports.push(stepped.payload.step);
    const latest = stepped.payload.step;

    // The reaction is complete and atomic here. If the host deadline elapsed
    // while it was running, the outcome is still operational non-success:
    // wall-clock time never becomes a semantic quiescence oracle.
    abortIfDeadlineExpired();

    post({
      type: "PROGRESS",
      requestId,
      progress: {
        phase: "REACTION_BOUNDARY",
        sessionId,
        sessionRunId: latest?.sessionRunId ?? begin?.sessionRunId ?? null,
        reactionIndex: latest?.evidence?.reactionIndex ?? null,
        completedSteps: reports.length,
        completed: latest?.completed === true,
        status: latestStatus,
        step: latest,
      },
    });

    if (latest?.completed === true) {
      workerState = "OPEN";
      cancelRequested = false;
      return {
        status: latestStatus,
        begin,
        reports,
        worker: workerStatus(),
      };
    }

    // Yield strictly after a complete atomic reaction so CANCEL and the
    // cooperative host deadline can be observed without splitting a reaction.
    await boundaryYield();
  }
}

async function beginStep(payload) {
  requireOpenSession();
  const run = clone(payload.run);
  run.executionMode = "STEP";
  const result = beginScenarioLiveStepRun(wasm, run);
  if (!result.ok) throwTransport(result.error, false);
  workerState = "STEPPING";
  return {
    status: result.payload.status,
    begin: result.payload.begin,
    worker: workerStatus(),
  };
}

async function singleStep() {
  requireOpenSession();
  const result = stepScenarioLiveSession(wasm);
  if (!result.ok) {
    const normalized = normalizeTransportError(result.error, true);
    closeOwnedSession(
      normalized.code === "BUDGET_EXCEEDED"
        ? "BUDGET_EXCEEDED"
        : "FAILED",
    );
    const error = new Error(normalized.message);
    error.workerError = normalized;
    throw error;
  }
  workerState = result.payload.step?.completed ? "OPEN" : "STEPPING";
  return {
    status: result.payload.status,
    step: result.payload.step,
    worker: workerStatus(),
  };
}

async function historyStatus() {
  requireOpenSession();
  const result = refreshScenarioLiveHistoryStatus(wasm);
  if (!result.ok) throwTransport(result.error, false);
  return { status: result.status, worker: workerStatus() };
}

async function dispatch(requestId, command, payload) {
  switch (command) {
    case "INIT": {
      const loaded = await ensureWasm();
      return { limits: loaded.limits, worker: workerStatus() };
    }
    case "OPEN":
      return openSession(payload || {});
    case "RUN":
      return runToBoundaryCompletion(requestId, payload || {});
    case "BEGIN_STEP":
      return beginStep(payload || {});
    case "STEP":
      return singleStep();
    case "HISTORY":
      return historyStatus();
    case "CLOSE":
      closeOwnedSession("CLOSED");
      return { closed: true, worker: workerStatus() };
    case "STATUS":
      return { worker: workerStatus() };
    default:
      throw protocolError(
        "UNKNOWN_COMMAND",
        "Unknown worker command: " + command,
      );
  }
}

function handleCancel(requestId, payload) {
  const targetRequestId = payload?.targetRequestId ?? busyRequestId;
  const accepted =
    busyRequestId !== null &&
    (targetRequestId === null || targetRequestId === busyRequestId);

  if (accepted) cancelRequested = true;

  post({
    type: "RESPONSE",
    requestId,
    ok: true,
    payload: {
      accepted,
      targetRequestId,
      worker: workerStatus(),
    },
  });
}

if (typeof globalThis.postMessage === "function") {
  globalThis.onmessage = async (event) => {
    const message = event?.data || {};
    const requestId = message.requestId ?? null;
    const command = message.command;

    if (message.schemaVersion !== SCENARIO_WORKER_PROTOCOL_VERSION ||
        requestId === null ||
        typeof command !== "string") {
      post({
        type: "RESPONSE",
        requestId,
        ok: false,
        error: errorRecord(
          "INVALID_ENVELOPE",
          "Invalid Worker request envelope",
        ),
      });
      return;
    }

    if (command === "CANCEL") {
      handleCancel(requestId, message.payload);
      return;
    }

    if (command === "STATUS") {
      post({
        type: "RESPONSE",
        requestId,
        ok: true,
        payload: { worker: workerStatus() },
      });
      return;
    }

    if (busyRequestId !== null) {
      post({
        type: "RESPONSE",
        requestId,
        ok: false,
        error: errorRecord(
          "WORKER_BUSY",
          "Worker is executing request " + busyRequestId,
          "WAIT_OR_CANCEL",
          workerStatus(),
        ),
      });
      return;
    }

    busyRequestId = requestId;
    try {
      const payload = await dispatch(
        requestId,
        command,
        message.payload || {},
      );
      post({ type: "RESPONSE", requestId, ok: true, payload });
    } catch (error) {
      const normalized = error?.workerError || errorRecord(
        "WORKER_FAILURE",
        error?.message || String(error),
        sessionOpen ? "KEEP_SESSION" : "OPEN_REQUIRED",
      );
      post({
        type: "RESPONSE",
        requestId,
        ok: false,
        error: normalized,
      });
    } finally {
      if (busyRequestId === requestId) busyRequestId = null;
    }
  };
}
