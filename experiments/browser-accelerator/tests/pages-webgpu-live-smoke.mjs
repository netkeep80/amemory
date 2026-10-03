import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const pageUrl = process.env.PAGE_URL;
const expectedSha = process.env.EXPECTED_SOURCE_SHA;
if (!pageUrl) throw new Error("PAGE_URL is required");
if (!/^[0-9a-f]{40}$/.test(expectedSha || "")) {
  throw new Error("EXPECTED_SOURCE_SHA must be an exact Git SHA");
}

function chromeBinary() {
  const found = spawnSync(
    "bash",
    [
      "-lc",
      "command -v google-chrome || command -v google-chrome-stable || " +
        "command -v chromium || command -v chromium-browser",
    ],
    { encoding: "utf8" },
  );
  const binary = found.stdout.trim().split("\n")[0];
  if (!binary) throw new Error("Chrome/Chromium is not installed");
  return binary;
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
      if (message.error) pending.reject(new Error(JSON.stringify(message.error)));
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

async function waitFor(cdp, expression, label, attempts = 600) {
  for (let index = 0; index < attempts; index += 1) {
    if (await evaluate(cdp, expression)) return;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error("browser timeout waiting for " + label);
}

async function fetchJson(url, attempts = 100) {
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
  throw last || new Error("fetch failed: " + url);
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function stopChrome() {
  if (chrome.exitCode !== null || chrome.signalCode !== null) return;

  chrome.kill("SIGTERM");
  for (let attempt = 0; attempt < 50; attempt += 1) {
    if (chrome.exitCode !== null || chrome.signalCode !== null) return;
    await sleep(100);
  }

  chrome.kill("SIGKILL");
  for (let attempt = 0; attempt < 50; attempt += 1) {
    if (chrome.exitCode !== null || chrome.signalCode !== null) return;
    await sleep(100);
  }
}

const profile = fs.mkdtempSync(
  path.join(os.tmpdir(), "amemory-pages-webgpu-"),
);
let stderr = "";
const chrome = spawn(
  chromeBinary(),
  [
    "--headless=new",
    "--no-sandbox",
    "--disable-dev-shm-usage",
    "--disable-gpu-sandbox",
    "--enable-unsafe-webgpu",
    "--use-webgpu-adapter=swiftshader",
    "--remote-debugging-address=127.0.0.1",
    "--remote-debugging-port=0",
    "--user-data-dir=" + profile,
    "about:blank",
  ],
  { stdio: ["ignore", "pipe", "pipe"] },
);
chrome.stderr.on("data", (chunk) => {
  stderr += chunk.toString();
  if (stderr.length > 32_768) stderr = stderr.slice(-32_768);
});

async function devToolsPort(attempts = 200) {
  const marker = path.join(profile, "DevToolsActivePort");
  for (let index = 0; index < attempts; index += 1) {
    if (fs.existsSync(marker)) {
      const [line] = fs.readFileSync(marker, "utf8").trim().split("\n");
      const port = Number(line);
      if (Number.isInteger(port) && port > 0) return port;
    }
    if (chrome.exitCode !== null) {
      throw new Error(
        "Chrome exited before DevTools became ready: " +
        chrome.exitCode + "\n" + stderr,
      );
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error("Chrome DevToolsActivePort was not created\n" + stderr);
}

let cdp = null;
try {
  const debugPort = await devToolsPort();
  await fetchJson("http://127.0.0.1:" + debugPort + "/json/version");
  const pages = await fetchJson("http://127.0.0.1:" + debugPort + "/json/list");
  const page = pages.find((entry) => entry.type === "page");
  if (!page?.webSocketDebuggerUrl) {
    throw new Error("Chrome page debugger endpoint is missing");
  }

  cdp = new Cdp(page.webSocketDebuggerUrl);
  await cdp.open();
  await cdp.command("Page.enable");
  await cdp.command("Runtime.enable");

  const target = new URL(pageUrl);
  target.searchParams.set("webgpuWitness", "1");
  target.searchParams.set("smoke", expectedSha);
  await cdp.command("Page.navigate", { url: target.href });

  const state =
    "document.querySelector('#workbench-root')" +
    "?.__amemoryWorkbenchState";
  await waitFor(
    cdp,
    "Boolean(" + state + " && !" + state + ".loading)",
    "Workbench mount",
  );
  await waitFor(
    cdp,
    "['verified','unavailable','failed'].includes(" +
      state + "?.gpuWitness?.status)",
    "C4c3 WebGPU witness",
  );

  const result = await evaluate(cdp, "(() => ({build:" + state +
    ".build,witness:" + state + ".gpuWitness}))()");
  if (result?.build?.sha !== expectedSha) {
    throw new Error(
      "deployed Workbench SHA mismatch: " +
      JSON.stringify(result?.build),
    );
  }
  const witness = result?.witness;
  const active = witness?.activeReactionCounts;
  const steps = witness?.stepCounts;
  const config = witness?.configurationAppendCounts;
  const normalizedGpu = witness?.normalizedGpuRuns;
  const normalizedCpu = witness?.normalizedCpuRuns;
  if (witness?.status !== "verified" ||
      witness?.scenarioManifestDriven !== true ||
      witness?.staticPreparedCarrier !== true ||
      witness?.noExecutionSeed !== true ||
      witness?.postReadbackCpuDifferential !== true ||
      witness?.cpuReferenceSharesRustCore !== true ||
      witness?.sequentialResidentExecution !== true ||
      witness?.residentBaseReuse !== true ||
      witness?.returnToFirstReuse !== true ||
      witness?.allQuiescent !== true ||
      witness?.manifestAssertionsPassed !== true ||
      witness?.scenarioId !== "mux1-lifecycle" ||
      witness?.scenarioVersion !== "1.0.0" ||
      witness?.runCount !== 4 ||
      !Array.isArray(witness?.values) ||
      witness.values.length !== witness.runCount ||
      !Array.isArray(active) ||
      active.length !== witness.runCount ||
      !active.every((value) => Number.isInteger(value) && value > 0) ||
      !Array.isArray(steps) ||
      steps.length !== witness.runCount ||
      !steps.every((value, index) => value === active[index] + 1) ||
      !Array.isArray(config) ||
      config.length !== witness.runCount ||
      !config.slice(0, 3).every((value) =>
        Number.isInteger(value) && value > 0
      ) ||
      config[3] !== 0 ||
      witness?.baseUploadCount !== 1 ||
      witness?.configurationCommitCount !== witness.runCount ||
      witness?.configurationDispatchCount !== 3 ||
      witness?.reactionDispatchCount !==
        steps.reduce((sum, value) => sum + value, 0) ||
      witness?.normalizedObservationSchemaVersion !== 2 ||
      !Array.isArray(normalizedGpu) ||
      normalizedGpu.length !== witness.runCount ||
      !Array.isArray(normalizedCpu) ||
      normalizedCpu.length !== witness.runCount ||
      !Number.isInteger(witness?.baseUploadBytes) ||
      witness.baseUploadBytes <= 0 ||
      !Number.isInteger(witness?.residentBufferBytes) ||
      witness.residentBufferBytes <= 0 ||
      !Number.isInteger(witness?.residentAppendCount) ||
      witness.residentAppendCount <= 0 ||
      !Number.isInteger(witness?.residentCapacity) ||
      witness.residentCapacity < witness.residentAppendCount) {
    throw new Error(
      "live WebGPU Scenario witness failed: " +
      JSON.stringify(witness) + "\nChrome:\n" + stderr,
    );
  }

  for (let index = 0; index < witness.runCount; index += 1) {
    const gpu = normalizedGpu[index];
    const cpu = normalizedCpu[index];
    if (gpu?.schemaVersion !== 2 ||
        gpu?.profile?.schemaVersion !== 2 ||
        gpu?.profile?.backendId !== "webgpu" ||
        gpu?.profile?.stopReason !== "QUIESCENT" ||
        gpu?.profile?.finalQuiescent !== true ||
        gpu?.profile?.activeReactionCount?.availability !== "MEASURED" ||
        gpu.profile.activeReactionCount.value !== active[index] ||
        gpu?.profile?.resource?.baseUploadCount?.availability !== "MEASURED" ||
        gpu.profile.resource.baseUploadCount.value !== 1 ||
        gpu?.profile?.resource?.configurationUploadBytes?.availability !==
          "MEASURED" ||
        (index === 3 &&
          gpu.profile.resource.configurationUploadBytes.value !== 0) ||
        gpu?.profile?.resource?.readbackBytes?.availability !== "UNAVAILABLE" ||
        gpu.profile.resource.readbackBytes.value !== null ||
        gpu?.events?.length !== steps[index] + 1 ||
        gpu.events.at(-1)?.kind !== "QUIESCENCE" ||
        cpu?.schemaVersion !== 2 ||
        cpu?.profile?.schemaVersion !== 2 ||
        cpu?.profile?.backendId !== "optimized-cpu" ||
        cpu?.profile?.stopReason !== "QUIESCENT" ||
        cpu?.profile?.finalQuiescent !== true ||
        cpu?.profile?.activeReactionCount?.availability !== "MEASURED" ||
        cpu.profile.activeReactionCount.value !== active[index] ||
        cpu?.profile?.timings?.executeNs?.availability !== "UNAVAILABLE" ||
        cpu.profile.timings.executeNs.value !== null ||
        cpu?.profile?.resource?.baseUploadCount?.availability !==
          "UNSUPPORTED" ||
        cpu.profile.resource.baseUploadCount.value !== null) {
      throw new Error(
        "normalized CPU/WebGPU observation mismatch at run " + index +
        ": gpu=" + JSON.stringify(gpu) +
        " cpu=" + JSON.stringify(cpu),
      );
    }
  }

  console.log(
    "PAGES_WEBGPU_SCENARIO=PASS source=" +
    expectedSha.slice(0, 12) +
    " scenario=" + witness.scenarioId + "@" + witness.scenarioVersion +
    " runs=" + witness.runCount +
    " values=" + witness.values.join(",") +
    " active=" + active.join(",") +
    " config=" + config.join(",") +
    " reuse=" + witness.returnToFirstReuse +
    " uploads=" + witness.baseUploadCount,
  );

} finally {
  cdp?.close();
  try {
    await stopChrome();
    fs.rmSync(profile, {
      recursive: true,
      force: true,
      maxRetries: 10,
      retryDelay: 100,
    });
  } catch (cleanupError) {
    console.warn(
      "C4c3 cleanup warning: " +
      (cleanupError?.stack || cleanupError?.message || String(cleanupError)),
    );
  }
}
