export const SCENARIO_WORKER_PROTOCOL_VERSION = 1;
export const DEFAULT_RUN_WALL_CLOCK_LIMIT_MS = 120_000;
export const DEFAULT_HARD_WATCHDOG_GRACE_MS = 1_000;

function nonNegativeFinite(value, label) {
  if (!Number.isFinite(value) || value < 0) {
    throw new Error(label + " must be a finite non-negative number");
  }
  return value;
}

export class ScenarioWorkerError extends Error {
  constructor(error) {
    super(error?.message || error?.code || "Scenario Worker error");
    this.name = "ScenarioWorkerError";
    Object.assign(this, error || {});
  }
}

function defaultWorkerUrl() {
  const current = new URL(import.meta.url);
  const worker = new URL("./scenario-worker.mjs", current);
  // Pages cache-busts the client module with ?v=<accepted SHA>. Reuse the
  // same query for the Worker entrypoint so both belong to one accepted build.
  worker.search = current.search;
  return worker;
}

export class ScenarioWorkerClient {
  constructor(options = {}) {
    const WorkerCtor = options.WorkerCtor || globalThis.Worker;
    if (typeof WorkerCtor !== "function") {
      throw new Error("Scenario Worker requires browser Worker support");
    }

    this.worker = new WorkerCtor(
      options.workerUrl || defaultWorkerUrl(),
      { type: "module", name: "amemory-scenario-worker" },
    );
    this.nextRequestId = 1;
    this.pending = new Map();
    this.activeRunRequestId = null;
    this.closed = false;
    this.runWallClockLimitMs = nonNegativeFinite(
      options.runWallClockLimitMs ?? DEFAULT_RUN_WALL_CLOCK_LIMIT_MS,
      "runWallClockLimitMs",
    );
    this.hardWatchdogGraceMs = nonNegativeFinite(
      options.hardWatchdogGraceMs ?? DEFAULT_HARD_WATCHDOG_GRACE_MS,
      "hardWatchdogGraceMs",
    );

    this.worker.onmessage = (event) => {
      this.#onMessage(event?.data || {});
    };
    this.worker.onerror = (event) => {
      const error = new ScenarioWorkerError({
        code: "WORKER_CRASHED",
        message: event?.message || "Scenario Worker crashed",
        recovery: "RECREATE_WORKER",
      });
      this.closed = true;
      this.#rejectPending(error);
    };
  }

  #clearEntryTimer(entry) {
    if (entry?.timer != null) clearTimeout(entry.timer);
  }

  #rejectPending(error) {
    for (const entry of this.pending.values()) {
      this.#clearEntryTimer(entry);
      entry.reject(error);
    }
    this.pending.clear();
    this.activeRunRequestId = null;
  }

  #hardAbortRun(requestId, wallClockLimitMs) {
    if (!this.pending.has(requestId)) return;

    const error = new ScenarioWorkerError({
      code: "WALL_CLOCK_SAFETY_ABORT",
      message: "Worker did not return to a reaction boundary before the host watchdog",
      recovery: "RECREATE_WORKER",
      atReactionBoundary: false,
      details: {
        wallClockLimitMs,
        hardAbort: true,
      },
    });

    this.closed = true;
    this.worker.terminate();
    this.#rejectPending(error);
  }

  #onMessage(message) {
    if (message.schemaVersion !== SCENARIO_WORKER_PROTOCOL_VERSION) {
      return;
    }

    const entry = this.pending.get(message.requestId);
    if (!entry) return;

    if (message.type === "PROGRESS") {
      entry.onProgress?.(message.progress);
      return;
    }
    if (message.type !== "RESPONSE") return;

    this.pending.delete(message.requestId);
    this.#clearEntryTimer(entry);
    if (this.activeRunRequestId === message.requestId) {
      this.activeRunRequestId = null;
    }

    if (message.ok) {
      entry.resolve(message.payload);
    } else {
      entry.reject(new ScenarioWorkerError(message.error));
    }
  }

  #request(command, payload = {}, options = {}) {
    if (this.closed) {
      return {
        requestId: null,
        promise: Promise.reject(new ScenarioWorkerError({
          code: "WORKER_CLIENT_CLOSED",
          message: "Scenario Worker client is closed",
          recovery: "RECREATE_WORKER",
        })),
      };
    }

    const requestId = this.nextRequestId++;
    const promise = new Promise((resolve, reject) => {
      const entry = {
        resolve,
        reject,
        onProgress: options.onProgress || null,
        timer: null,
      };
      this.pending.set(requestId, entry);
      if (options.hardTimeoutMs != null) {
        entry.timer = setTimeout(
          () => this.#hardAbortRun(
            requestId,
            options.wallClockLimitMs,
          ),
          options.hardTimeoutMs,
        );
      }
      this.worker.postMessage({
        schemaVersion: SCENARIO_WORKER_PROTOCOL_VERSION,
        requestId,
        command,
        payload,
      });
    });

    return { requestId, promise };
  }

  async init() {
    return this.#request("INIT").promise;
  }

  async open(manifest, backend = "optimized-cpu") {
    return this.#request("OPEN", { manifest, backend }).promise;
  }

  async run(run, options = {}) {
    const wallClockLimitMs = nonNegativeFinite(
      options.wallClockLimitMs ?? this.runWallClockLimitMs,
      "wallClockLimitMs",
    );
    const request = this.#request(
      "RUN",
      { run, wallClockLimitMs },
      {
        onProgress: options.onProgress,
        wallClockLimitMs,
        hardTimeoutMs:
          wallClockLimitMs + this.hardWatchdogGraceMs,
      },
    );
    this.activeRunRequestId = request.requestId;
    return request.promise;
  }

  async beginStep(run) {
    return this.#request("BEGIN_STEP", { run }).promise;
  }

  async step() {
    return this.#request("STEP").promise;
  }

  async history() {
    return this.#request("HISTORY").promise;
  }

  async status() {
    return this.#request("STATUS").promise;
  }

  async cancelActive() {
    const targetRequestId = this.activeRunRequestId;
    return this.#request("CANCEL", { targetRequestId }).promise;
  }

  async closeSession() {
    return this.#request("CLOSE").promise;
  }

  terminate() {
    if (this.closed) return;
    this.closed = true;
    this.worker.terminate();

    const error = new ScenarioWorkerError({
      code: "WORKER_TERMINATED",
      message: "Scenario Worker client terminated",
      recovery: "RECREATE_WORKER",
    });
    this.#rejectPending(error);
  }
}
