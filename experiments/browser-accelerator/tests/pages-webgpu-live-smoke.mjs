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
    "--enable-features=Vulkan",
    "--use-angle=vulkan",
    "--use-vulkan=swiftshader",
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
  if (witness?.status !== "verified" ||
      witness?.tripleDifferential !== true ||
      witness?.cpuGpuDifferential !== true ||
      witness?.publishedHandle !== witness?.rustWasmScopeAfter ||
      !Number.isInteger(witness?.appendCount) ||
      witness.appendCount <= 0) {
    throw new Error(
      "live C4c3 WebGPU witness failed: " +
      JSON.stringify(witness) + "\nChrome:\n" + stderr,
    );
  }

  console.log(
    "PAGES_WEBGPU_C4C3=PASS source=" +
    expectedSha.slice(0, 12) +
    " mode=" + witness.planMode +
    " rule=L" + witness.ruleHandle +
    " scope=L" + witness.publishedHandle +
    " append=" + witness.appendCount,
  );
} finally {
  cdp?.close();
  chrome.kill("SIGTERM");
  fs.rmSync(profile, { recursive: true, force: true });
}
