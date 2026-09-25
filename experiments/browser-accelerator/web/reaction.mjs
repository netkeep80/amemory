import {
  cpuExport,
  cpuImportRaw,
  cpuResetPool,
  createGpuAnumPool,
  destroyGpuAnumPool,
  gpuExport,
  gpuImport,
  readGpuTopology,
  localRef,
  normalizeAnum,
  parseAnum,
  formatAnum,
  requireLocalRef,
  wasmU32,
} from "./anum-boundary.mjs";

export const R1_FIXTURE = Object.freeze({
  K: "98",
  A: "68",
  B: "16898",
  current: "19868",
  relation: "16816898",
  successor: "19816898",
});

const NONE = 0xffffffff;
const CAP = 4;

function must(condition, message) {
  if (!condition) throw new Error(message);
}

export function pairAnum(start, end) {
  return normalizeAnum("1" + normalizeAnum(start) + normalizeAnum(end));
}

export function splitPairAnum(source) {
  const node = parseAnum(source);
  if (node.kind !== "PAIR") {
    throw new Error("reaction record must be PAIR Anum: " + source);
  }
  return Object.freeze({
    start: formatAnum(node.start),
    end: formatAnum(node.end),
  });
}

export function assertR1Fixture() {
  const current = splitPairAnum(R1_FIXTURE.current);
  const relation = splitPairAnum(R1_FIXTURE.relation);
  const successor = splitPairAnum(R1_FIXTURE.successor);
  must(current.start === R1_FIXTURE.K && current.end === R1_FIXTURE.A, "bad R1 current");
  must(relation.start === R1_FIXTURE.A && relation.end === R1_FIXTURE.B, "bad R1 relation");
  must(successor.start === R1_FIXTURE.K && successor.end === R1_FIXTURE.B, "bad R1 successor");
  must(pairAnum(R1_FIXTURE.K, R1_FIXTURE.A) === R1_FIXTURE.current, "noncanonical current");
  must(pairAnum(R1_FIXTURE.A, R1_FIXTURE.B) === R1_FIXTURE.relation, "noncanonical relation");
  must(pairAnum(R1_FIXTURE.K, R1_FIXTURE.B) === R1_FIXTURE.successor, "noncanonical successor");
  return true;
}

export function normalizeReactionState(anums) {
  return [...new Set(anums.map(normalizeAnum))].sort();
}

export function assertReactionStateExact(expected, actual, label = "reaction") {
  const left = normalizeReactionState(expected);
  const right = normalizeReactionState(actual);
  if (right.length !== actual.length) {
    throw new Error(label + ": duplicate semantic member");
  }
  if (left.length !== right.length) {
    throw new Error(label + ": member-count mismatch " + left.length + " != " + right.length);
  }
  for (let i = 0; i < left.length; i += 1) {
    if (left[i] !== right[i]) {
      throw new Error(label + ": mismatch at " + i + ": expected " + left[i] + ", got " + right[i]);
    }
  }
  return true;
}

export function perturbReactionState(anums) {
  if (anums.length === 0) throw new Error("cannot perturb empty reaction state");
  const copy = [...anums];
  copy[0] = copy[0] === "8" ? "98" : "8";
  return copy;
}

function importCpuFixture(wasm) {
  cpuResetPool(wasm);
  const refs = {};
  for (const [name, source] of Object.entries(R1_FIXTURE)) {
    const ref = cpuImportRaw(wasm, source);
    must(ref, "CPU rejected R1 Anum " + name + "=" + source);
    refs[name] = ref;
  }
  return refs;
}

async function importGpuFixture(device, pool) {
  const refs = {};
  for (const [name, source] of Object.entries(R1_FIXTURE)) {
    const ref = await gpuImport(device, pool, source);
    must(ref, "GPU rejected R1 Anum " + name + "=" + source);
    refs[name] = ref;
  }
  return refs;
}

async function validateGpuFixtureTopology(device, pool, refs) {
  const topology = await readGpuTopology(device, pool);
  const handles = Object.fromEntries(
    Object.entries(refs).map(([name, ref]) => [name, requireLocalRef(ref, pool.memory)]),
  );
  for (const [name, handle] of Object.entries(handles)) {
    must(topology.used[handle] !== 0, "GPU topology preflight: unused handle for " + name + "=" + handle);
  }
  const checks = [
    ["current.start", topology.starts[handles.current], handles.K],
    ["current.end", topology.ends[handles.current], handles.A],
    ["relation.start", topology.starts[handles.relation], handles.A],
    ["relation.end", topology.ends[handles.relation], handles.B],
    ["successor.start", topology.starts[handles.successor], handles.K],
    ["successor.end", topology.ends[handles.successor], handles.B],
  ];
  for (const [label, actual, expected] of checks) {
    must(actual === expected, "GPU topology preflight " + label + ": expected " + expected + ", got " + actual);
  }
  return handles;
}

function cpuConfigure(wasm, currentRef, relationRef) {
  wasm.reactionReset();
  must(wasm.reactionSetCurrentMember(0, requireLocalRef(currentRef, "cpu-A")) === 1, "CPU current rejected");
  must(wasm.reactionSetCurrentCount(1) === 1, "CPU current count rejected");
  must(wasm.reactionSetTheoryRelation(0, requireLocalRef(relationRef, "cpu-A")) === 1, "CPU relation rejected");
  must(wasm.reactionSetTheoryCount(1) === 1, "CPU theory count rejected");
  must(wasm.reactionSnapshotTheory() === 1, "CPU snapshot failed");
  must(wasm.reactionSnapshotCount() === 1, "CPU snapshot count mismatch");
}

function cpuCurrent(wasm) {
  const count = wasmU32(wasm.reactionCurrentCount());
  const out = [];
  for (let i = 0; i < count; i += 1) {
    out.push(cpuExport(wasm, localRef("cpu-A", wasmU32(wasm.reactionCurrentMember(i)))));
  }
  return normalizeReactionState(out);
}

function cpuBank(wasm, bank) {
  const count = wasmU32(wasm.reactionBankCount(bank));
  const out = [];
  for (let i = 0; i < count; i += 1) {
    out.push(cpuExport(wasm, localRef("cpu-A", wasmU32(wasm.reactionBankMember(bank, i)))));
  }
  return normalizeReactionState(out);
}

function runCpuPositive(wasm, refs) {
  cpuConfigure(wasm, refs.current, refs.relation);
  const before = cpuCurrent(wasm);
  const oldBank = wasmU32(wasm.reactionCurrentBank());

  must(
    wasm.reactionSetTheoryRelation(0, requireLocalRef(refs.successor, "cpu-A")) === 1,
    "CPU post-snapshot mutation rejected",
  );

  must(wasm.reactionRun() === 1, "CPU reaction failed");
  return {
    before,
    after: cpuCurrent(wasm),
    oldPhysical: cpuBank(wasm, oldBank),
    matched: wasmU32(wasm.reactionMatchedRelations()),
    handoff: wasmU32(wasm.reactionHandoffCount()),
    snapshotCount: wasmU32(wasm.reactionSnapshotCount()),
  };
}

function runCpuNoMatch(wasm, refs) {
  cpuConfigure(wasm, refs.current, refs.successor);
  const beforeBank = wasmU32(wasm.reactionCurrentBank());
  must(wasm.reactionRun() === 1, "CPU no-match failed");
  return {
    state: cpuCurrent(wasm),
    beforeBank,
    afterBank: wasmU32(wasm.reactionCurrentBank()),
    matched: wasmU32(wasm.reactionMatchedRelations()),
    handoff: wasmU32(wasm.reactionHandoffCount()),
  };
}

function gpuUsage() {
  return GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC | GPUBufferUsage.COPY_DST;
}

function makeBuffer(device, words, data = null) {
  const buffer = device.createBuffer({ size: words * 4, usage: gpuUsage() });
  device.queue.writeBuffer(buffer, 0, data || new Uint32Array(words));
  return buffer;
}

function createGpuReactionState(device, currentHandle, relationHandle) {
  const selector = makeBuffer(device, 1, new Uint32Array([0]));
  const scopeWords = new Uint32Array(2 + CAP * 2);
  scopeWords.fill(NONE);
  scopeWords[0] = 1;
  scopeWords[1] = 0;
  scopeWords[2] = currentHandle;
  const scope = makeBuffer(device, scopeWords.length, scopeWords);

  const theoryWords = new Uint32Array(1 + CAP);
  theoryWords.fill(NONE);
  theoryWords[0] = 1;
  theoryWords[1] = relationHandle;
  const theory = makeBuffer(device, theoryWords.length, theoryWords);

  const snapshotWords = new Uint32Array(1 + CAP);
  snapshotWords.fill(NONE);
  snapshotWords[0] = 1;
  snapshotWords[1] = relationHandle;
  const snapshot = makeBuffer(device, snapshotWords.length, snapshotWords);

  const status = makeBuffer(device, 4, new Uint32Array([0, 0, 0, 900]));
  return { selector, scope, theory, snapshot, status };
}

function destroyGpuState(state) {
  for (const buffer of Object.values(state)) buffer.destroy();
}

async function readWords(device, buffer, words) {
  const bytes = words * 4;
  const readback = device.createBuffer({
    size: bytes,
    usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
  });
  const encoder = device.createCommandEncoder();
  encoder.copyBufferToBuffer(buffer, 0, readback, 0, bytes);
  device.queue.submit([encoder.finish()]);
  await readback.mapAsync(GPUMapMode.READ, 0, bytes);
  const out = new Uint32Array(readback.getMappedRange(0, bytes).slice(0));
  readback.unmap();
  readback.destroy();
  return out;
}

const R1_WGSL = [
  "const CAP: u32 = 4u;",
  "const NONE: u32 = 0xffffffffu;",
  "const END_BASE: u32 = 64u;",
  "const USED_BASE: u32 = 128u;",
  "@group(0) @binding(0) var<storage, read> pool: array<u32>;",
  "@group(0) @binding(1) var<storage, read_write> scope: array<u32>;",
  "@group(0) @binding(2) var<storage, read> snapshot: array<u32>;",
  "@group(0) @binding(3) var<storage, read_write> published: array<atomic<u32>>;",
  "@group(0) @binding(4) var<storage, read_write> status: array<u32>;",
  "fn valid(h: u32) -> bool { return h > 0u && h < 64u && pool[USED_BASE + h] != 0u; }",
  "fn pair_record(h: u32) -> bool {",
  "  if (!valid(h)) { return false; }",
  "  let s = pool[h]; let e = pool[END_BASE + h];",
  "  return s != h && e != h && valid(s) && valid(e);",
  "}",
  "fn base(bank: u32) -> u32 { return 2u + bank * CAP; }",
  "fn find_pair(s: u32, e: u32) -> u32 {",
  "  var h = 1u;",
  "  loop {",
  "    if (h >= 64u) { break; }",
  "    if (valid(h) && pool[h] == s && pool[END_BASE + h] == e && s != h && e != h) { return h; }",
  "    h = h + 1u;",
  "  }",
  "  return NONE;",
  "}",
  "@compute @workgroup_size(1)",
  "fn main(@builtin(global_invocation_id) id: vec3<u32>) {",
  "  if (id.x != 0u) { return; }",
  "  status[0] = 0u; status[1] = 0u; status[2] = 0u; status[3] = 100u;",
  "  let bank = atomicLoad(&published[0]);",
  "  if (bank > 1u) { status[3] = 101u; return; }",
  "  let current_count = scope[bank];",
  "  let snapshot_count = snapshot[0];",
  "  if (current_count > CAP || snapshot_count > CAP) { status[3] = 102u; return; }",
  "  let target_bank = 1u - bank;",
  "  let current_base = base(bank); let target_base = base(target_bank);",
  "  var i = 0u;",
  "  loop { if (i >= CAP) { break; } scope[target_base + i] = NONE; i = i + 1u; }",
  "  var out_count = 0u; var matched = 0u; var mi = 0u;",
  "  loop {",
  "    if (mi >= current_count) { break; }",
  "    let member = scope[current_base + mi];",
  "    if (!pair_record(member)) { status[3] = 103u; return; }",
  "    let context = pool[member]; let antecedent = pool[END_BASE + member];",
  "    var member_matches = 0u; var ri = 0u;",
  "    loop {",
  "      if (ri >= snapshot_count) { break; }",
  "      let relation = snapshot[1u + ri];",
  "      if (!pair_record(relation)) { status[3] = 104u; return; }",
  "      if (pool[relation] == antecedent) {",
  "        let candidate = find_pair(context, pool[END_BASE + relation]);",
  "        if (candidate == NONE) { status[3] = 105u; return; }",
  "        if (out_count >= CAP) { status[3] = 106u; return; }",
  "        scope[target_base + out_count] = candidate;",
  "        out_count = out_count + 1u; matched = matched + 1u; member_matches = member_matches + 1u;",
  "      }",
  "      ri = ri + 1u;",
  "    }",
  "    if (member_matches == 0u) {",
  "      if (out_count >= CAP) { status[3] = 107u; return; }",
  "      scope[target_base + out_count] = member; out_count = out_count + 1u;",
  "    }",
  "    mi = mi + 1u;",
  "  }",
  "  status[1] = matched;",
  "  if (matched == 0u) { status[0] = 1u; status[3] = 200u + bank; return; }",
  "  scope[target_bank] = out_count;",
  "  atomicStore(&published[0], target_bank);",
  "  status[0] = 1u; status[2] = 1u; status[3] = 210u + target_bank;",
  "}",
].join("\n");

async function gpuRun(device, pool, state) {
  device.pushErrorScope("validation");
  try {
    const shader = device.createShaderModule({ code: R1_WGSL });
    if (typeof shader.getCompilationInfo === "function") {
      const info = await shader.getCompilationInfo();
      const errors = info.messages.filter((message) => message.type === "error");
      if (errors.length > 0) {
        throw new Error("WGSL compilation failed: " + errors.map((message) => message.lineNum + ":" + message.linePos + " " + message.message).join(" | "));
      }
    }
    const descriptor = { layout: "auto", compute: { module: shader, entryPoint: "main" } };
    const pipeline = typeof device.createComputePipelineAsync === "function"
      ? await device.createComputePipelineAsync(descriptor)
      : device.createComputePipeline(descriptor);
    const bindGroup = device.createBindGroup({
      layout: pipeline.getBindGroupLayout(0),
      entries: [
        { binding: 0, resource: { buffer: pool.buffer } },
        { binding: 1, resource: { buffer: state.scope } },
        { binding: 2, resource: { buffer: state.snapshot } },
        { binding: 3, resource: { buffer: state.selector } },
        { binding: 4, resource: { buffer: state.status } },
      ],
    });
    const encoder = device.createCommandEncoder();
    const pass = encoder.beginComputePass();
    pass.setPipeline(pipeline);
    pass.setBindGroup(0, bindGroup);
    pass.dispatchWorkgroups(1);
    pass.end();
    device.queue.submit([encoder.finish()]);
    await device.queue.onSubmittedWorkDone();
  } finally {
    const validationError = await device.popErrorScope();
    if (validationError) throw new Error("WebGPU validation failed: " + validationError.message);
  }
}

async function gpuObserve(device, pool, state) {
  const [selector, scope, status] = await Promise.all([
    readWords(device, state.selector, 1),
    readWords(device, state.scope, 2 + CAP * 2),
    readWords(device, state.status, 4),
  ]);
  const bank = selector[0];
  must(bank <= 1, "GPU invalid current bank");
  const count = scope[bank];
  must(count <= CAP, "GPU current count exceeds cap");
  const base = 2 + bank * CAP;
  const anums = [];
  for (let i = 0; i < count; i += 1) {
    anums.push(await gpuExport(device, pool, localRef(pool.memory, scope[base + i])));
  }
  return {
    bank,
    anums: normalizeReactionState(anums),
    status: status[0],
    matched: status[1],
    handoff: status[2],
    diagnostic: status[3],
  };
}

async function gpuBank(device, pool, state, bank) {
  const scope = await readWords(device, state.scope, 2 + CAP * 2);
  const count = scope[bank];
  const base = 2 + bank * CAP;
  const out = [];
  for (let i = 0; i < count; i += 1) {
    out.push(await gpuExport(device, pool, localRef(pool.memory, scope[base + i])));
  }
  return normalizeReactionState(out);
}

async function runGpuPositive(device, pool, refs) {
  const state = createGpuReactionState(
    device,
    requireLocalRef(refs.current, pool.memory),
    requireLocalRef(refs.relation, pool.memory),
  );
  try {
    const before = await gpuObserve(device, pool, state);
    const oldBank = before.bank;

    device.queue.writeBuffer(
      state.theory,
      4,
      new Uint32Array([requireLocalRef(refs.successor, pool.memory)]),
    );

    await gpuRun(device, pool, state);
    const after = await gpuObserve(device, pool, state);
    must(after.status === 1, "GPU reaction failed: diagnostic=" + after.diagnostic + ", matched=" + after.matched + ", handoff=" + after.handoff + ", bank=" + after.bank);
    return {
      before: before.anums,
      after: after.anums,
      oldPhysical: await gpuBank(device, pool, state, oldBank),
      matched: after.matched,
      handoff: after.handoff,
      snapshotCount: 1,
    };
  } finally {
    destroyGpuState(state);
  }
}

async function runGpuNoMatch(device, pool, refs) {
  const state = createGpuReactionState(
    device,
    requireLocalRef(refs.current, pool.memory),
    requireLocalRef(refs.successor, pool.memory),
  );
  try {
    const before = await gpuObserve(device, pool, state);
    await gpuRun(device, pool, state);
    const after = await gpuObserve(device, pool, state);
    must(after.status === 1, "GPU no-match failed: diagnostic=" + after.diagnostic + ", matched=" + after.matched + ", handoff=" + after.handoff + ", bank=" + after.bank);
    return {
      state: after.anums,
      beforeBank: before.bank,
      afterBank: after.bank,
      matched: after.matched,
      handoff: after.handoff,
    };
  } finally {
    destroyGpuState(state);
  }
}

export async function runReactionBrowser(wasm, device) {
  assertR1Fixture();
  const logs = [];
  const cpuRefs = importCpuFixture(wasm);
  const gpuPool = createGpuAnumPool(device, "gpu-reaction-B");

  try {
    const gpuRefs = await importGpuFixture(device, gpuPool);
    const gpuTopologyHandles = await validateGpuFixtureTopology(device, gpuPool, gpuRefs);
    const cpu = runCpuPositive(wasm, cpuRefs);
    const gpu = await runGpuPositive(device, gpuPool, gpuRefs);

    assertReactionStateExact([R1_FIXTURE.current], cpu.before, "CPU before");
    assertReactionStateExact([R1_FIXTURE.current], gpu.before, "GPU before");
    assertReactionStateExact([R1_FIXTURE.successor], cpu.after, "CPU successor");
    assertReactionStateExact([R1_FIXTURE.successor], gpu.after, "GPU successor");
    assertReactionStateExact(cpu.after, gpu.after, "CPU/GPU successor differential");
    assertReactionStateExact([R1_FIXTURE.current], cpu.oldPhysical, "CPU old Scope");
    assertReactionStateExact([R1_FIXTURE.current], gpu.oldPhysical, "GPU old Scope");

    must(cpu.matched === 1 && gpu.matched === 1, "matchedRelations mismatch");
    must(cpu.handoff === 1 && gpu.handoff === 1, "handoffCount mismatch");
    must(cpu.snapshotCount === 1 && gpu.snapshotCount === 1, "snapshot count mismatch");

    const cpuNoMatch = runCpuNoMatch(wasm, cpuRefs);
    const gpuNoMatch = await runGpuNoMatch(device, gpuPool, gpuRefs);
    assertReactionStateExact([R1_FIXTURE.current], cpuNoMatch.state, "CPU no-match");
    assertReactionStateExact([R1_FIXTURE.current], gpuNoMatch.state, "GPU no-match");
    must(cpuNoMatch.matched === 0 && gpuNoMatch.matched === 0, "no-match relation was admitted");
    must(cpuNoMatch.handoff === 0 && gpuNoMatch.handoff === 0, "no-match performed handoff");
    must(cpuNoMatch.beforeBank === cpuNoMatch.afterBank, "CPU no-match changed current Scope");
    must(gpuNoMatch.beforeBank === gpuNoMatch.afterBank, "GPU no-match changed current Scope");

    let mismatchDetected = false;
    try {
      assertReactionStateExact(cpu.after, perturbReactionState(gpu.after), "deliberate mismatch");
    } catch (error) {
      mismatchDetected = true;
      logs.push("reaction.negative.mismatch = " + error.message);
    }
    must(mismatchDetected, "deliberate mismatch was accepted");

    let foreignDetected = false;
    try {
      requireLocalRef(cpuRefs.current, gpuPool.memory);
    } catch (error) {
      foreignDetected = true;
      logs.push("reaction.negative.foreign = " + error.message);
    }
    must(foreignDetected, "foreign local handle was accepted");

    const handlesDiffer =
      requireLocalRef(cpuRefs.current, "cpu-A") !== requireLocalRef(gpuRefs.current, gpuPool.memory) &&
      requireLocalRef(cpuRefs.successor, "cpu-A") !== requireLocalRef(gpuRefs.successor, gpuPool.memory);
    must(handlesDiffer, "CPU/GPU local reaction handles did not differ");

    logs.push("reaction.profile = minimal-portable-amemory-execution@0.1.0");
    logs.push("reaction.gpu.topology = K " + gpuTopologyHandles.K + ", A " + gpuTopologyHandles.A + ", B " + gpuTopologyHandles.B + ", current " + gpuTopologyHandles.current + ", relation " + gpuTopologyHandles.relation + ", successor " + gpuTopologyHandles.successor);
    logs.push("reaction.before.cpu = [" + cpu.before.join(", ") + "]");
    logs.push("reaction.before.gpu = [" + gpu.before.join(", ") + "]");
    logs.push("reaction.after.cpu = [" + cpu.after.join(", ") + "]");
    logs.push("reaction.after.gpu = [" + gpu.after.join(", ") + "]");
    logs.push("reaction.cpu.matched = " + cpu.matched);
    logs.push("reaction.gpu.matched = " + gpu.matched);
    logs.push("reaction.cpu.handoff = " + cpu.handoff);
    logs.push("reaction.gpu.handoff = " + gpu.handoff);
    logs.push("reaction.handles.current = CPU " + cpuRefs.current.value + " != GPU " + gpuRefs.current.value);
    logs.push("reaction.handles.successor = CPU " + cpuRefs.successor.value + " != GPU " + gpuRefs.successor.value);
    logs.push("reaction.snapshot.isolation = PASS");
    logs.push("reaction.old-scope-retained = PASS");

    return {
      cpuBefore: cpu.before,
      gpuBefore: gpu.before,
      cpuAfter: cpu.after,
      gpuAfter: gpu.after,
      cpuMatched: cpu.matched,
      gpuMatched: gpu.matched,
      cpuHandoff: cpu.handoff,
      gpuHandoff: gpu.handoff,
      normalizedDifferential: true,
      handlesDiffer,
      snapshotIsolation: true,
      oldScopeRetained: true,
      negativeControls: true,
      logs,
    };
  } finally {
    destroyGpuAnumPool(gpuPool);
  }
}
