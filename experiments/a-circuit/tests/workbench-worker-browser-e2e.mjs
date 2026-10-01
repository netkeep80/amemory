import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import http from "node:http";
import os from "node:os";
import path from "node:path";

const root = process.cwd();
const webRoot = path.join(
  root,
  "experiments/browser-accelerator/web",
);
const wasmPath = path.join(
  root,
  "experiments/a-circuit/target/wasm32-unknown-unknown/release/" +
    "amemory_a_circuit.wasm",
);
const version = fs.readFileSync(
  path.join(root, "VERSION"),
  "utf8",
).trim();
const sourceSha =
  process.env.AMEMORY_SOURCE_SHA || "a".repeat(40);

if (!fs.existsSync(wasmPath)) {
  throw new Error("real A-Circuit WASM is missing: " + wasmPath);
}

function chromeBinary() {
  const found = spawnSync(
    "bash",
    [
      "-lc",
      "command -v google-chrome || " +
        "command -v google-chrome-stable || " +
        "command -v chromium || command -v chromium-browser",
    ],
    { encoding: "utf8" },
  );
  const binary = found.stdout.trim().split("\n")[0];
  if (!binary) {
    throw new Error("headless Chrome/Chromium is not installed");
  }
  return binary;
}


function contentType(file) {
  if (file.endsWith(".html")) return "text/html; charset=utf-8";
  if (file.endsWith(".mjs") || file.endsWith(".js")) {
    return "text/javascript; charset=utf-8";
  }
  if (file.endsWith(".json")) return "application/json";
  if (file.endsWith(".wasm")) return "application/wasm";
  return "application/octet-stream";
}

function startServer() {
  const buildInfo = JSON.stringify({
    schemaVersion: 2,
    version,
    mainSha: sourceSha,
    acceptanceRunId: 0,
    builtAt: "2026-01-01T00:00:00Z",
  });

  return new Promise((resolve, reject) => {
    const server = http.createServer((request, response) => {
      try {
        const url = new URL(
          request.url || "/",
          "http://127.0.0.1",
        );
        const pathname = decodeURIComponent(url.pathname);

        if (pathname === "/build-info.json") {
          response.writeHead(200, {
            "content-type": "application/json",
            "cache-control": "no-store",
          });
          response.end(buildInfo);
          return;
        }
        if (pathname === "/amemory_a_circuit.wasm") {
          response.writeHead(200, {
            "content-type": "application/wasm",
            "cache-control": "no-store",
          });
          fs.createReadStream(wasmPath).pipe(response);
          return;
        }

        const relative = pathname === "/"
          ? "index.html"
          : pathname.replace(/^\/+/, "");
        const file = path.resolve(webRoot, relative);
        if (!file.startsWith(path.resolve(webRoot) + path.sep) &&
            file !== path.join(webRoot, "index.html")) {
          response.writeHead(403);
          response.end("forbidden");
          return;
        }
        if (!fs.existsSync(file) || !fs.statSync(file).isFile()) {
          response.writeHead(404);
          response.end("not found");
          return;
        }

        response.writeHead(200, {
          "content-type": contentType(file),
          "cache-control": "no-store",
        });
        fs.createReadStream(file).pipe(response);
      } catch (error) {
        response.writeHead(500);
        response.end(String(error));
      }
    });
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      resolve(server);
    });
  });
}

async function pollJson(url, attempts = 100) {
  let last = null;
  for (let index = 0; index < attempts; index += 1) {
    try {
      const response = await fetch(url);
      if (response.ok) return response.json();
      last = new Error("HTTP " + response.status);
    } catch (error) {
      last = error;
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw last || new Error("poll failed: " + url);
}

class Cdp {
  constructor(url) {
    this.socket = new WebSocket(url);
    this.nextId = 1;
    this.pending = new Map();
  }

  async open() {
    await new Promise((resolve, reject) => {
      this.socket.addEventListener("open", resolve, { once: true });
      this.socket.addEventListener("error", reject, { once: true });
    });
    this.socket.addEventListener("message", (event) => {
      const message = JSON.parse(event.data);
      if (!message.id) return;
      const pending = this.pending.get(message.id);
      if (!pending) return;
      this.pending.delete(message.id);
      if (message.error) pending.reject(
        new Error(JSON.stringify(message.error)),
      );
      else pending.resolve(message.result || {});
    });
  }

  command(method, params = {}) {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.socket.send(JSON.stringify({ id, method, params }));
    });
  }

  close() {
    this.socket.close();
  }
}

async function evaluate(cdp, expression) {
  const result = await cdp.command("Runtime.evaluate", {
    expression,
    awaitPromise: true,
    returnByValue: true,
    userGesture: true,
  });
  if (result.exceptionDetails) {
    throw new Error(
      "browser evaluation failed: " +
      (result.exceptionDetails.text || JSON.stringify(result.exceptionDetails)),
    );
  }
  return result.result?.value;
}

async function waitFor(cdp, expression, label, attempts = 300) {
  for (let index = 0; index < attempts; index += 1) {
    if (await evaluate(cdp, expression)) return;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error("browser timeout waiting for " + label);
}

const server = await startServer();
const port = server.address().port;
const profile = fs.mkdtempSync(
  path.join(os.tmpdir(), "amemory-chrome-"),
);
let chromeStderr = "";
const chrome = spawn(
  chromeBinary(),
  [
    "--headless=new",
    "--no-sandbox",
    "--disable-gpu",
    "--disable-dev-shm-usage",
    "--remote-debugging-address=127.0.0.1",
    "--remote-debugging-port=0",
    "--user-data-dir=" + profile,
    "about:blank",
  ],
  { stdio: ["ignore", "pipe", "pipe"] },
);
chrome.stderr.on("data", (chunk) => {
  chromeStderr += chunk.toString();
  if (chromeStderr.length > 16_384) {
    chromeStderr = chromeStderr.slice(-16_384);
  }
});

async function waitForDevToolsPort(attempts = 150) {
  const marker = path.join(profile, "DevToolsActivePort");
  for (let index = 0; index < attempts; index += 1) {
    if (fs.existsSync(marker)) {
      const [portLine] = fs
        .readFileSync(marker, "utf8")
        .trim()
        .split("\n");
      const port = Number(portLine);
      if (Number.isInteger(port) && port > 0) return port;
    }
    if (chrome.exitCode !== null) {
      throw new Error(
        "Chrome exited before DevTools became ready: " +
        chrome.exitCode + "\n" + chromeStderr,
      );
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(
    "Chrome DevToolsActivePort was not created\n" + chromeStderr,
  );
}

let cdp = null;
try {
  const debugPort = await waitForDevToolsPort();
  await pollJson(
    "http://127.0.0.1:" + debugPort + "/json/version",
  );
  const pages = await pollJson(
    "http://127.0.0.1:" + debugPort + "/json/list",
  );
  const page = pages.find((entry) => entry.type === "page");
  if (!page?.webSocketDebuggerUrl) {
    throw new Error("Chrome page debugger endpoint is missing");
  }

  cdp = new Cdp(page.webSocketDebuggerUrl);
  await cdp.open();
  await cdp.command("Page.enable");
  await cdp.command("Runtime.enable");
  await cdp.command("Page.navigate", {
    url: "http://127.0.0.1:" + port + "/index.html",
  });

  const state =
    "document.querySelector('#workbench-root')" +
    "?.__amemoryWorkbenchState";

  await waitFor(
    cdp,
    `Boolean(${state}?.session?.sessionId && !${state}?.loading)`,
    "initial retained Worker Session",
  );

  // Switch to XOR32 so manual WORD32 validation is testable.
  await evaluate(cdp, `(() => {
    const select = document.querySelector("#wb-scenario");
    select.value = "1";
    select.dispatchEvent(new Event("change", { bubbles: true }));
    return true;
  })()`);
  await waitFor(
    cdp,
    `${state}?.manifest?.scenarioId === "xor32-lifecycle" &&
      ${state}?.session === null && !${state}?.loading`,
    "XOR32 manifest switch and explicit close",
  );

  await evaluate(cdp, `document.querySelector("#wb-open").click()`);
  await waitFor(
    cdp,
    `Boolean(${state}?.session?.sessionId && !${state}?.loading)`,
    "XOR32 Session open",
  );

  const opened = await evaluate(cdp, `(() => ({
    sessionId: ${state}.session.sessionId,
    storeInstanceId: ${state}.session.storeInstanceId,
    engineInstanceId: ${state}.session.engineInstanceId,
    prepareCount: ${state}.session.prepareCount,
    loadCount: ${state}.session.loadCount,
  }))()`);
  if (opened.prepareCount !== 1 || opened.loadCount !== 1) {
    throw new Error("Worker Session was not PREPARE/LOAD exactly once");
  }

  await evaluate(cdp, `(() => {
    document.querySelector('[data-mode="manual"]').click();
    const set = (key, value) => {
      const input = document.querySelector('[data-input-key="' + key + '"]');
      input.value = value;
      input.dispatchEvent(new Event("change", { bubbles: true }));
    };
    set("A", "0x12345678");
    set("B", "0xffffffff");
    document.querySelector("#wb-run").click();
    return true;
  })()`);
  await waitFor(
    cdp,
    `Boolean(${state}?.run?.sessionRunId === 1 && !${state}?.loading)`,
    "first Worker run",
  );

  const first = await evaluate(cdp, `(() => ({
    sessionId: ${state}.session.sessionId,
    storeInstanceId: ${state}.session.storeInstanceId,
    engineInstanceId: ${state}.session.engineInstanceId,
    completedRuns: ${state}.session.completedRuns,
    value: ${state}.run.result.fields.value,
    finalQuiescent: ${state}.run.observed.finalQuiescent,
  }))()`);
  if (first.sessionId !== opened.sessionId ||
      first.storeInstanceId !== opened.storeInstanceId ||
      first.engineInstanceId !== opened.engineInstanceId ||
      first.completedRuns !== 1 ||
      first.value !== "0xedcba987" ||
      first.finalQuiescent !== true) {
    throw new Error("first retained Worker run mismatch: " +
      JSON.stringify(first));
  }

  await evaluate(cdp, `(() => {
    const set = (key, value) => {
      const input = document.querySelector('[data-input-key="' + key + '"]');
      input.value = value;
      input.dispatchEvent(new Event("change", { bubbles: true }));
    };
    set("A", "0xaaaaaaaa");
    set("B", "0x55555555");
    document.querySelector("#wb-run").click();
    return true;
  })()`);
  await waitFor(
    cdp,
    `Boolean(${state}?.run?.sessionRunId === 2 && !${state}?.loading)`,
    "second Worker run after reconfigure",
  );

  const second = await evaluate(cdp, `(() => ({
    sessionId: ${state}.session.sessionId,
    storeInstanceId: ${state}.session.storeInstanceId,
    engineInstanceId: ${state}.session.engineInstanceId,
    completedRuns: ${state}.session.completedRuns,
    value: ${state}.run.result.fields.value,
    links: ${state}.session.currentLinkCount,
  }))()`);
  if (second.sessionId !== opened.sessionId ||
      second.storeInstanceId !== opened.storeInstanceId ||
      second.engineInstanceId !== opened.engineInstanceId ||
      second.completedRuns !== 2 ||
      second.value !== "0xffffffff") {
    throw new Error("retained reconfigure run mismatch: " +
      JSON.stringify(second));
  }

  // Open the actual Proof presentation entrypoint in the real browser.
  await evaluate(cdp, `(() => {
    document.querySelector('[data-level="proof"]').click();
    const proofTab = document.querySelector('[data-tab="proof"]');
    if (!proofTab) throw new Error("Proof tab is missing");
    proofTab.click();
    return true;
  })()`);
  await waitFor(
    cdp,
    `${state}?.level === "proof" &&
      ${state}?.tab === "proof" &&
      Boolean(document.querySelector(".wb-verification")) &&
      Boolean(document.querySelector(".proof-recursive-structure"))`,
    "Proof/renderer entrypoint",
  );

  // Invalid WORD32 is rejected in the main-thread typed-input adapter before
  // any Worker CONFIGURE request is sent. Session identity/carrier stay intact.
  await evaluate(cdp, `(() => {
    const input = document.querySelector('[data-input-key="A"]');
    input.value = "not-a-number";
    input.dispatchEvent(new Event("change", { bubbles: true }));
    document.querySelector("#wb-run").click();
    return true;
  })()`);
  await waitFor(
    cdp,
    `Boolean(${state}?.error && !${state}?.loading)`,
    "invalid input rejection",
  );
  const invalid = await evaluate(cdp, `(() => ({
    sessionId: ${state}.session.sessionId,
    completedRuns: ${state}.session.completedRuns,
    links: ${state}.session.currentLinkCount,
    error: ${state}.error,
  }))()`);
  if (invalid.sessionId !== opened.sessionId ||
      invalid.completedRuns !== 2 ||
      invalid.links !== second.links ||
      !/требуется|число/i.test(invalid.error)) {
    throw new Error("invalid input mutated retained Session: " +
      JSON.stringify(invalid));
  }

  await evaluate(cdp, `document.querySelector("#wb-close").click()`);
  await waitFor(
    cdp,
    `${state}?.session === null && !${state}?.loading`,
    "explicit Session close",
  );

  const afterClose = await evaluate(cdp, `(async () => {
    const mod = await import("./workbench.mjs");
    const s = ${state};
    const run = mod.createWorkbenchRun(
      s.manifest, 0, { A: "0x1", B: "0x2" }, 3, "manual"
    );
    try {
      await s.worker.run(run);
      return { ok: true };
    } catch (error) {
      return {
        ok: false,
        code: error.code,
        recovery: error.recovery,
      };
    }
  })()`);
  if (afterClose.ok ||
      afterClose.code !== "SESSION_NOT_OPEN" ||
      afterClose.recovery !== "OPEN_REQUIRED") {
    throw new Error("run-after-close did not fail closed: " +
      JSON.stringify(afterClose));
  }

  const unsupported = await evaluate(cdp, `(async () => {
    const s = ${state};
    try {
      await s.worker.open(s.manifest, "webgpu");
      return { ok: true };
    } catch (error) {
      return {
        ok: false,
        code: error.code,
        recovery: error.recovery,
        transportCode: error.details?.code,
        runnerCode: error.details?.error?.code,
      };
    }
  })()`);
  if (unsupported.ok ||
      unsupported.transportCode !== "RUNNER" ||
      unsupported.runnerCode !== "UNSUPPORTED_BACKEND") {
    throw new Error("unsupported backend silently fell back: " +
      JSON.stringify(unsupported));
  }

  // Cancellation is requested from main thread after a completed reaction.
  // Worker yields only at reaction boundaries and then explicitly closes the
  // failed/cancelled Session. Reopen is a separate explicit command.
  const cancelled = await evaluate(cdp, `(async () => {
    const mod = await import("./workbench.mjs");
    const s = ${state};
    const reopened = await s.worker.open(s.manifest, "optimized-cpu");
    const oldSessionId = reopened.status.sessionId;
    const run = mod.createWorkbenchRun(
      s.manifest,
      0,
      { A: "0x12345678", B: "0xffffffff" },
      1,
      "manual",
    );
    let boundaries = 0;
    try {
      await s.worker.run(run, {
        onProgress(progress) {
          if (progress.phase === "REACTION_BOUNDARY") {
            boundaries += 1;
            if (boundaries === 1) void s.worker.cancelActive();
          }
        },
      });
      return { ok: true };
    } catch (error) {
      const status = await s.worker.status();
      const reopenedAgain = await s.worker.open(
        s.manifest,
        "optimized-cpu",
      );
      const newSessionId = reopenedAgain.status.sessionId;
      await s.worker.closeSession();
      return {
        ok: false,
        code: error.code,
        recovery: error.recovery,
        atReactionBoundary: error.atReactionBoundary,
        boundaries,
        workerState: status.worker.state,
        sessionOpen: status.worker.sessionOpen,
        oldSessionId,
        newSessionId,
      };
    }
  })()`);
  if (cancelled.ok ||
      cancelled.code !== "CANCELLED" ||
      cancelled.recovery !== "REOPEN_REQUIRED" ||
      cancelled.atReactionBoundary !== true ||
      cancelled.boundaries < 1 ||
      cancelled.workerState !== "CANCELLED" ||
      cancelled.sessionOpen !== false ||
      cancelled.oldSessionId === cancelled.newSessionId) {
    throw new Error("reaction-boundary cancellation mismatch: " +
      JSON.stringify(cancelled));
  }

  // Wall-clock is a host safety boundary only. A zero limit deterministically
  // expires at the first safe boundary before a reaction can be reported as a
  // semantic success; explicit reopen must create a new Session identity.
  const watchdog = await evaluate(cdp, `(async () => {
    const mod = await import("./workbench.mjs");
    const s = ${state};
    const reopened = await s.worker.open(s.manifest, "optimized-cpu");
    const oldSessionId = reopened.status.sessionId;
    const run = mod.createWorkbenchRun(
      s.manifest,
      0,
      { A: "0x12345678", B: "0xffffffff" },
      1,
      "manual",
    );
    try {
      await s.worker.run(run, { wallClockLimitMs: 0 });
      return { ok: true };
    } catch (error) {
      const status = await s.worker.status();
      const reopenedAgain = await s.worker.open(
        s.manifest,
        "optimized-cpu",
      );
      const newSessionId = reopenedAgain.status.sessionId;
      await s.worker.closeSession();
      return {
        ok: false,
        code: error.code,
        recovery: error.recovery,
        atReactionBoundary: error.atReactionBoundary,
        hardAbort: error.details?.hardAbort,
        completedSteps: error.details?.completedSteps,
        workerState: status.worker.state,
        sessionOpen: status.worker.sessionOpen,
        oldSessionId,
        newSessionId,
      };
    }
  })()`);
  if (watchdog.ok ||
      watchdog.code !== "WALL_CLOCK_SAFETY_ABORT" ||
      watchdog.recovery !== "REOPEN_REQUIRED" ||
      watchdog.atReactionBoundary !== true ||
      watchdog.hardAbort !== false ||
      watchdog.completedSteps !== 0 ||
      watchdog.workerState !== "WALL_CLOCK_SAFETY_ABORT" ||
      watchdog.sessionOpen !== false ||
      watchdog.oldSessionId === watchdog.newSessionId) {
    throw new Error("reaction-boundary watchdog mismatch: " +
      JSON.stringify(watchdog));
  }

  // The client also owns a harder watchdog for a Worker that never returns to
  // a reaction boundary at all. A silent Worker makes that path deterministic
  // without adding timing-sensitive semantic execution to CI.
  const hardWatchdog = await evaluate(cdp, `(async () => {
    const { ScenarioWorkerClient } =
      await import("./scenario-worker-client.mjs");
    class SilentWorker {
      constructor() {
        this.onmessage = null;
        this.onerror = null;
        this.terminated = false;
      }
      postMessage() {}
      terminate() { this.terminated = true; }
    }

    const client = new ScenarioWorkerClient({
      WorkerCtor: SilentWorker,
      runWallClockLimitMs: 0,
      hardWatchdogGraceMs: 0,
    });
    try {
      await client.run({});
      return { ok: true };
    } catch (error) {
      let closedCode = null;
      try {
        await client.init();
      } catch (closed) {
        closedCode = closed.code;
      }
      return {
        ok: false,
        code: error.code,
        recovery: error.recovery,
        atReactionBoundary: error.atReactionBoundary,
        hardAbort: error.details?.hardAbort,
        terminated: client.worker.terminated,
        closedCode,
      };
    }
  })()`);
  if (hardWatchdog.ok ||
      hardWatchdog.code !== "WALL_CLOCK_SAFETY_ABORT" ||
      hardWatchdog.recovery !== "RECREATE_WORKER" ||
      hardWatchdog.atReactionBoundary !== false ||
      hardWatchdog.hardAbort !== true ||
      hardWatchdog.terminated !== true ||
      hardWatchdog.closedCode !== "WORKER_CLIENT_CLOSED") {
    throw new Error("hard Worker watchdog mismatch: " +
      JSON.stringify(hardWatchdog));
  }

  console.log(
    "WORKBENCH_WORKER_BROWSER_E2E=GREEN " +
    JSON.stringify({
      sessionId: opened.sessionId,
      firstRun: first.value,
      secondRun: second.value,
      cancellationBoundaries: cancelled.boundaries,
      watchdogBoundaryAbort: watchdog.code,
      hardWatchdogAbort: hardWatchdog.code,
    }),
  );
} finally {
  try { cdp?.close(); } catch {}

  if (chrome.exitCode === null) {
    chrome.kill("SIGTERM");
    await new Promise((resolve) => {
      const timer = setTimeout(resolve, 2_000);
      chrome.once("exit", () => {
        clearTimeout(timer);
        resolve();
      });
    });
  }
  if (chrome.exitCode === null) {
    chrome.kill("SIGKILL");
    await new Promise((resolve) => {
      chrome.once("exit", resolve);
    });
  }

  await new Promise((resolve) => server.close(resolve));

  // Chrome may report its parent process exit before profile-writing child
  // processes have released every Default/ file. Cleanup is test hygiene,
  // not semantic evidence, so give those writers a bounded grace period and
  // never turn an already-GREEN browser execution into a false semantic RED.
  await new Promise((resolve) => setTimeout(resolve, 250));
  try {
    fs.rmSync(profile, {
      recursive: true,
      force: true,
      maxRetries: 20,
      retryDelay: 200,
    });
  } catch (error) {
    console.warn(
      "WORKBENCH_WORKER_BROWSER_E2E_PROFILE_CLEANUP_WARNING " +
      JSON.stringify({
        code: error?.code || "UNKNOWN",
        path: error?.path || profile,
      }),
    );
  }
}
