const ui = {
  wasm: document.querySelector("#wasm"),
  cpu: document.querySelector("#cpu"),
  webgpu: document.querySelector("#webgpu"),
  compute: document.querySelector("#compute"),
  details: document.querySelector("#details"),
};

const details = [];

function report(element, text, ok = null) {
  element.textContent = text;
  if (ok === true) element.className = "ok";
  if (ok === false) element.className = "fail";
}

function log(line) {
  details.push(line);
  ui.details.textContent = details.join("\n");
}

async function loadWasm() {
  const response = await fetch("./amemory_browser_probe.wasm", { cache: "no-store" });
  if (!response.ok) {
    throw new Error(`WASM fetch failed: HTTP ${response.status}`);
  }

  const bytes = await response.arrayBuffer();
  const { instance } = await WebAssembly.instantiate(bytes, {});
  const probe = instance.exports.amemory_probe;
  const cpuStep = instance.exports.amemory_cpu_step;

  if (typeof probe !== "function" || typeof cpuStep !== "function") {
    throw new Error("Expected Rust/WASM exports are missing");
  }

  const marker = probe();
  if (marker !== 0xA013) {
    throw new Error(`Unexpected WASM probe marker: 0x${marker.toString(16)}`);
  }

  report(ui.wasm, "LOADED", true);
  log(`wasm.marker = 0x${marker.toString(16)}`);

  const cpuResult = cpuStep(41);
  if (cpuResult !== 42) {
    throw new Error(`WASM CPU control failed: expected 42, got ${cpuResult}`);
  }

  report(ui.cpu, "PASS (41 -> 42)", true);
  log(`wasm.cpu_step = ${cpuResult}`);
}

async function runWebGpu() {
  if (!("gpu" in navigator)) {
    report(ui.webgpu, "WEBGPU_UNAVAILABLE", false);
    report(ui.compute, "NOT_RUN", false);
    log("navigator.gpu is not available in this browser.");
    return;
  }

  let adapter;
  try {
    adapter = await navigator.gpu.requestAdapter();
  } catch (error) {
    report(ui.webgpu, "WEBGPU_INIT_FAILED", false);
    report(ui.compute, "NOT_RUN", false);
    log(`requestAdapter failed: ${error}`);
    return;
  }

  if (!adapter) {
    report(ui.webgpu, "WEBGPU_UNAVAILABLE", false);
    report(ui.compute, "NOT_RUN", false);
    log("WebGPU exists, but no GPUAdapter was returned.");
    return;
  }

  let device;
  try {
    device = await adapter.requestDevice();
  } catch (error) {
    report(ui.webgpu, "WEBGPU_INIT_FAILED", false);
    report(ui.compute, "NOT_RUN", false);
    log(`requestDevice failed: ${error}`);
    return;
  }

  report(ui.webgpu, "WEBGPU_AVAILABLE", true);
  log(`adapter.features = ${[...adapter.features].join(", ") || "(none exposed)"}`);
  log(`device.features = ${[...device.features].join(", ") || "(none exposed)"}`);

  const input = new Uint32Array([1, 2, 3, 4]);
  const expected = new Uint32Array([2, 3, 4, 5]);
  const byteLength = input.byteLength;

  const inputBuffer = device.createBuffer({
    size: byteLength,
    usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST,
  });

  const outputBuffer = device.createBuffer({
    size: byteLength,
    usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC,
  });

  const readbackBuffer = device.createBuffer({
    size: byteLength,
    usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
  });

  device.queue.writeBuffer(inputBuffer, 0, input);

  const shader = device.createShaderModule({
    code: `
      @group(0) @binding(0)
      var<storage, read> input_data: array<u32>;

      @group(0) @binding(1)
      var<storage, read_write> output_data: array<u32>;

      @compute @workgroup_size(64)
      fn main(@builtin(global_invocation_id) id: vec3<u32>) {
        let i = id.x;
        if (i < 4u) {
          output_data[i] = input_data[i] + 1u;
        }
      }
    `,
  });

  const pipeline = device.createComputePipeline({
    layout: "auto",
    compute: {
      module: shader,
      entryPoint: "main",
    },
  });

  const bindGroup = device.createBindGroup({
    layout: pipeline.getBindGroupLayout(0),
    entries: [
      { binding: 0, resource: { buffer: inputBuffer } },
      { binding: 1, resource: { buffer: outputBuffer } },
    ],
  });

  const encoder = device.createCommandEncoder();
  const pass = encoder.beginComputePass();
  pass.setPipeline(pipeline);
  pass.setBindGroup(0, bindGroup);
  pass.dispatchWorkgroups(1);
  pass.end();

  encoder.copyBufferToBuffer(outputBuffer, 0, readbackBuffer, 0, byteLength);
  device.queue.submit([encoder.finish()]);

  await readbackBuffer.mapAsync(GPUMapMode.READ, 0, byteLength);
  const copy = readbackBuffer.getMappedRange(0, byteLength).slice(0);
  readbackBuffer.unmap();

  const actual = new Uint32Array(copy);
  const passResult =
    actual.length === expected.length &&
    actual.every((value, index) => value === expected[index]);

  if (!passResult) {
    report(ui.compute, `FAIL [${[...actual].join(", ")}]`, false);
    log(`gpu.actual = [${[...actual].join(", ")}]`);
    log(`gpu.expected = [${[...expected].join(", ")}]`);
    return;
  }

  report(ui.compute, `PASS [${[...actual].join(", ")}]`, true);
  log(`gpu.input = [${[...input].join(", ")}]`);
  log(`gpu.output = [${[...actual].join(", ")}]`);

  inputBuffer.destroy();
  outputBuffer.destroy();
  readbackBuffer.destroy();
  device.destroy();
}

async function main() {
  log(`secureContext = ${window.isSecureContext}`);
  log(`location = ${location.href}`);

  try {
    await loadWasm();
  } catch (error) {
    report(ui.wasm, "FAILED", false);
    report(ui.cpu, "NOT_RUN", false);
    log(`WASM error: ${error?.stack || error}`);
  }

  try {
    await runWebGpu();
  } catch (error) {
    report(ui.webgpu, "WEBGPU_INIT_FAILED", false);
    report(ui.compute, "FAILED", false);
    log(`WebGPU error: ${error?.stack || error}`);
  }
}

main();
