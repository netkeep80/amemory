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

export const R3_FIXTURE = Object.freeze({
  root: "8",
  zeroRelation: "1688",
  rootCurrent: "1988",
  rootRelation: "1816898",
});

export const R4_FIXTURE = Object.freeze({
  C: "998",
  relationBC: "116898998",
  successorC: "198998",
});

export const R5_FIXTURE = Object.freeze({
  context: "998",
  startValue: "98",
  endValue: "68",
  stateStart: "199898",
  stateEnd: "199868",
  relationStartEnd: "19868",
  relationEndStart: "16898",
});

export const R6_FIXTURE = Object.freeze({
  relationAC: "168998",
  rootRelationC: "18998",
});

export const R5_OBSERVATION_STEPS = 4;

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

export function assertR2Fixture() {
  const current = splitPairAnum(R1_FIXTURE.current);
  const wrongRelation = splitPairAnum(R1_FIXTURE.successor);
  must(current.end === R1_FIXTURE.A, "bad R2 current antecedent");
  must(wrongRelation.start === R1_FIXTURE.K, "bad R2 wrong-relation antecedent");
  must(wrongRelation.start !== current.end, "R2 relation unexpectedly applicable");
  return true;
}

export function assertR3Fixture() {
  const zero = splitPairAnum(R3_FIXTURE.zeroRelation);
  const rootCurrent = splitPairAnum(R3_FIXTURE.rootCurrent);
  const rootRelation = splitPairAnum(R3_FIXTURE.rootRelation);
  must(zero.start === R1_FIXTURE.A && zero.end === R3_FIXTURE.root, "bad R3 ZERO relation");
  must(rootCurrent.start === R1_FIXTURE.K && rootCurrent.end === R3_FIXTURE.root, "bad R3 root current");
  must(rootRelation.start === R3_FIXTURE.root && rootRelation.end === R1_FIXTURE.B, "bad R3 root relation");
  must(pairAnum(R1_FIXTURE.A, R3_FIXTURE.root) === R3_FIXTURE.zeroRelation, "noncanonical R3 ZERO");
  must(pairAnum(R1_FIXTURE.K, R3_FIXTURE.root) === R3_FIXTURE.rootCurrent, "noncanonical R3 root current");
  must(pairAnum(R3_FIXTURE.root, R1_FIXTURE.B) === R3_FIXTURE.rootRelation, "noncanonical R3 root relation");
  return true;
}

export function assertR4Fixture() {
  const relationBC = splitPairAnum(R4_FIXTURE.relationBC);
  const successorC = splitPairAnum(R4_FIXTURE.successorC);
  must(relationBC.start === R1_FIXTURE.B && relationBC.end === R4_FIXTURE.C, "bad R4 B->C relation");
  must(successorC.start === R1_FIXTURE.K && successorC.end === R4_FIXTURE.C, "bad R4 K->C successor");
  must(pairAnum(R1_FIXTURE.B, R4_FIXTURE.C) === R4_FIXTURE.relationBC, "noncanonical R4 B->C");
  must(pairAnum(R1_FIXTURE.K, R4_FIXTURE.C) === R4_FIXTURE.successorC, "noncanonical R4 K->C");
  return true;
}

export function assertR6Fixture() {
  const relationAC = splitPairAnum(R6_FIXTURE.relationAC);
  const rootRelationC = splitPairAnum(R6_FIXTURE.rootRelationC);
  must(relationAC.start === R1_FIXTURE.A && relationAC.end === R4_FIXTURE.C, "bad R6 A->C relation");
  must(rootRelationC.start === R3_FIXTURE.root && rootRelationC.end === R4_FIXTURE.C, "bad R6 ROOT->C relation");
  must(pairAnum(R1_FIXTURE.A, R4_FIXTURE.C) === R6_FIXTURE.relationAC, "noncanonical R6 A->C");
  must(pairAnum(R3_FIXTURE.root, R4_FIXTURE.C) === R6_FIXTURE.rootRelationC, "noncanonical R6 ROOT->C");
  return true;
}

export function assertR5Fixture() {
  const stateStart = splitPairAnum(R5_FIXTURE.stateStart);
  const stateEnd = splitPairAnum(R5_FIXTURE.stateEnd);
  const relationStartEnd = splitPairAnum(R5_FIXTURE.relationStartEnd);
  const relationEndStart = splitPairAnum(R5_FIXTURE.relationEndStart);
  const endNode = parseAnum(R5_FIXTURE.endValue);

  must(stateStart.start === R5_FIXTURE.context && stateStart.end === R5_FIXTURE.startValue, "bad R5 S_A");
  must(stateEnd.start === R5_FIXTURE.context && stateEnd.end === R5_FIXTURE.endValue, "bad R5 S_C");
  must(
    relationStartEnd.start === R5_FIXTURE.startValue && relationStartEnd.end === R5_FIXTURE.endValue,
    "bad R5 A->C relation",
  );
  must(
    relationEndStart.start === R5_FIXTURE.endValue && relationEndStart.end === R5_FIXTURE.startValue,
    "bad R5 C->A relation",
  );
  must(endNode.kind === "END", "R5 endValue must be structural END");
  must(pairAnum(R5_FIXTURE.context, R5_FIXTURE.startValue) === R5_FIXTURE.stateStart, "noncanonical R5 S_A");
  must(pairAnum(R5_FIXTURE.context, R5_FIXTURE.endValue) === R5_FIXTURE.stateEnd, "noncanonical R5 S_C");
  must(pairAnum(R5_FIXTURE.startValue, R5_FIXTURE.endValue) === R5_FIXTURE.relationStartEnd, "noncanonical R5 A->C");
  must(pairAnum(R5_FIXTURE.endValue, R5_FIXTURE.startValue) === R5_FIXTURE.relationEndStart, "noncanonical R5 C->A");
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
  for (const [name, source] of Object.entries({ ...R1_FIXTURE, ...R3_FIXTURE, ...R4_FIXTURE, ...R5_FIXTURE, ...R6_FIXTURE })) {
    const ref = cpuImportRaw(wasm, source);
    must(ref, "CPU rejected reaction Anum " + name + "=" + source);
    refs[name] = ref;
  }
  return refs;
}

async function importGpuFixture(device, pool) {
  const refs = {};
  for (const [name, source] of Object.entries({ ...R1_FIXTURE, ...R3_FIXTURE, ...R4_FIXTURE, ...R5_FIXTURE, ...R6_FIXTURE })) {
    const ref = await gpuImport(device, pool, source);
    must(ref, "GPU rejected reaction Anum " + name + "=" + source);
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
    ["root.start", topology.starts[handles.root], handles.root],
    ["root.end", topology.ends[handles.root], handles.root],
    ["zeroRelation.start", topology.starts[handles.zeroRelation], handles.A],
    ["zeroRelation.end", topology.ends[handles.zeroRelation], handles.root],
    ["rootCurrent.start", topology.starts[handles.rootCurrent], handles.K],
    ["rootCurrent.end", topology.ends[handles.rootCurrent], handles.root],
    ["rootRelation.start", topology.starts[handles.rootRelation], handles.root],
    ["rootRelation.end", topology.ends[handles.rootRelation], handles.B],
    ["C.start", topology.starts[handles.C], handles.C],
    ["C.end", topology.ends[handles.C], handles.K],
    ["relationBC.start", topology.starts[handles.relationBC], handles.B],
    ["relationBC.end", topology.ends[handles.relationBC], handles.C],
    ["successorC.start", topology.starts[handles.successorC], handles.K],
    ["successorC.end", topology.ends[handles.successorC], handles.C],
    ["R6.relationAC.start", topology.starts[handles.relationAC], handles.A],
    ["R6.relationAC.end", topology.ends[handles.relationAC], handles.C],
    ["R6.rootRelationC.start", topology.starts[handles.rootRelationC], handles.root],
    ["R6.rootRelationC.end", topology.ends[handles.rootRelationC], handles.C],
    ["R5.endValue.start", topology.starts[handles.endValue], handles.root],
    ["R5.endValue.end", topology.ends[handles.endValue], handles.endValue],
    ["R5.stateStart.start", topology.starts[handles.stateStart], handles.context],
    ["R5.stateStart.end", topology.ends[handles.stateStart], handles.startValue],
    ["R5.stateEnd.start", topology.starts[handles.stateEnd], handles.context],
    ["R5.stateEnd.end", topology.ends[handles.stateEnd], handles.endValue],
    ["R5.relationStartEnd.start", topology.starts[handles.relationStartEnd], handles.startValue],
    ["R5.relationStartEnd.end", topology.ends[handles.relationStartEnd], handles.endValue],
    ["R5.relationEndStart.start", topology.starts[handles.relationEndStart], handles.endValue],
    ["R5.relationEndStart.end", topology.ends[handles.relationEndStart], handles.startValue],
  ];
  for (const [label, actual, expected] of checks) {
    must(actual === expected, "GPU topology preflight " + label + ": expected " + expected + ", got " + actual);
  }
  return handles;
}

function cpuConfigureMany(wasm, currentRefs, relationRefs) {
  wasm.reactionReset();
  currentRefs.forEach((ref, index) => {
    must(
      wasm.reactionSetCurrentMember(index, requireLocalRef(ref, "cpu-A")) === 1,
      "CPU current rejected at " + index,
    );
  });
  must(wasm.reactionSetCurrentCount(currentRefs.length) === 1, "CPU current count rejected");
  relationRefs.forEach((ref, index) => {
    must(
      wasm.reactionSetTheoryRelation(index, requireLocalRef(ref, "cpu-A")) === 1,
      "CPU relation rejected at " + index,
    );
  });
  must(wasm.reactionSetTheoryCount(relationRefs.length) === 1, "CPU theory count rejected");
  must(wasm.reactionSnapshotTheory() === 1, "CPU snapshot failed");
  must(wasm.reactionSnapshotCount() === relationRefs.length, "CPU snapshot count mismatch");
}

function cpuConfigure(wasm, currentRef, relationRef) {
  cpuConfigureMany(wasm, [currentRef], [relationRef]);
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
    quiescent: wasmU32(wasm.reactionQuiescent()) === 1,
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
    quiescent: wasmU32(wasm.reactionQuiescent()) === 1,
  };
}

function runCpuInvalidFailure(wasm, refs) {
  wasm.reactionReset();
  must(wasm.reactionSetCurrentCount(1) === 1, "CPU invalid-control count rejected");
  must(wasm.reactionSetTheoryRelation(0, requireLocalRef(refs.successor, "cpu-A")) === 1, "CPU invalid-control relation rejected");
  must(wasm.reactionSetTheoryCount(1) === 1, "CPU invalid-control theory count rejected");
  must(wasm.reactionSnapshotTheory() === 1, "CPU invalid-control snapshot failed");
  const result = wasm.reactionRun();
  return {
    result,
    quiescent: wasmU32(wasm.reactionQuiescent()) === 1,
    handoff: wasmU32(wasm.reactionHandoffCount()),
  };
}

function runCpuZero(wasm, refs) {
  cpuConfigureMany(wasm, [refs.current], [refs.zeroRelation]);
  const oldBank = wasmU32(wasm.reactionCurrentBank());
  must(wasm.reactionRun() === 1, "CPU R3 ZERO failed");
  return {
    state: cpuCurrent(wasm),
    oldPhysical: cpuBank(wasm, oldBank),
    matched: wasmU32(wasm.reactionMatchedRelations()),
    handoff: wasmU32(wasm.reactionHandoffCount()),
    quiescent: wasmU32(wasm.reactionQuiescent()) === 1,
    oldBank,
    newBank: wasmU32(wasm.reactionCurrentBank()),
  };
}

function runCpuMixedDuplicate(wasm, refs) {
  cpuConfigureMany(
    wasm,
    [refs.current, refs.rootCurrent],
    [refs.zeroRelation, refs.relation, refs.rootRelation],
  );
  must(wasm.reactionRun() === 1, "CPU R3 mixed/duplicate failed");
  return {
    state: cpuCurrent(wasm),
    matched: wasmU32(wasm.reactionMatchedRelations()),
    handoff: wasmU32(wasm.reactionHandoffCount()),
    quiescent: wasmU32(wasm.reactionQuiescent()) === 1,
  };
}

function runCpuBatch(wasm, currentRefs, relationRefs) {
  cpuConfigureMany(wasm, currentRefs, relationRefs);
  must(wasm.reactionRun() === 1, "CPU R6 batch reaction failed");
  return {
    state: cpuCurrent(wasm),
    matched: wasmU32(wasm.reactionMatchedRelations()),
    handoff: wasmU32(wasm.reactionHandoffCount()),
    quiescent: wasmU32(wasm.reactionQuiescent()) === 1,
  };
}

function runCpuOutOfScope(wasm) {
  wasm.reactionReset();
  const accepted = wasm.reactionSetCurrentCount(17);
  return {
    rejected: accepted === 0,
    currentCount: wasmU32(wasm.reactionCurrentCount()),
    handoff: wasmU32(wasm.reactionHandoffCount()),
    quiescent: wasmU32(wasm.reactionQuiescent()) === 1,
  };
}

function runCpuTheoryTplus1(wasm, refs) {
  cpuConfigureMany(wasm, [refs.current], [refs.relation]);
  const tBefore = cpuCurrent(wasm);
  const tSnapshotCount = wasmU32(wasm.reactionSnapshotCount());

  must(
    wasm.reactionSetTheoryRelation(1, requireLocalRef(refs.relationBC, "cpu-A")) === 1,
    "CPU R4 live Theory admission rejected",
  );
  must(wasm.reactionSetTheoryCount(2) === 1, "CPU R4 live Theory count rejected");
  const liveTheoryCount = wasmU32(wasm.reactionTheoryCount());
  must(wasmU32(wasm.reactionSnapshotCount()) === 1, "CPU R4 snapshot_t leaked live Theory");

  must(wasm.reactionRun() === 1, "CPU R4 reaction t failed");
  const tAfter = cpuCurrent(wasm);
  const tMatched = wasmU32(wasm.reactionMatchedRelations());
  const tHandoff = wasmU32(wasm.reactionHandoffCount());
  const tQuiescent = wasmU32(wasm.reactionQuiescent()) === 1;

  const t1Before = cpuCurrent(wasm);
  must(wasm.reactionSnapshotTheory() === 1, "CPU R4 t+1 snapshot failed");
  const t1SnapshotCount = wasmU32(wasm.reactionSnapshotCount());
  must(wasm.reactionRun() === 1, "CPU R4 reaction t+1 failed");
  const t1After = cpuCurrent(wasm);
  const t1Matched = wasmU32(wasm.reactionMatchedRelations());
  const t1Handoff = wasmU32(wasm.reactionHandoffCount());
  const t1Quiescent = wasmU32(wasm.reactionQuiescent()) === 1;

  // Independent stale-snapshot negative control.
  cpuConfigureMany(wasm, [refs.successor], [refs.relation]);
  must(
    wasm.reactionSetTheoryRelation(1, requireLocalRef(refs.relationBC, "cpu-A")) === 1,
    "CPU R4 stale control admission rejected",
  );
  must(wasm.reactionSetTheoryCount(2) === 1, "CPU R4 stale control Theory count rejected");
  must(wasmU32(wasm.reactionSnapshotCount()) === 1, "CPU R4 stale control snapshot leaked");
  must(wasm.reactionRun() === 1, "CPU R4 stale control failed");
  const staleState = cpuCurrent(wasm);
  const staleMatched = wasmU32(wasm.reactionMatchedRelations());

  return {
    tBefore,
    tSnapshotCount,
    liveTheoryCount,
    tAfter,
    tMatched,
    tHandoff,
    tQuiescent,
    t1Before,
    t1SnapshotCount,
    t1After,
    t1Matched,
    t1Handoff,
    t1Quiescent,
    staleState,
    staleMatched,
  };
}

function runCpuRecurrenceEnd(wasm, refs) {
  cpuConfigureMany(
    wasm,
    [refs.stateStart],
    [refs.relationStartEnd, refs.relationEndStart],
  );
  const states = [cpuCurrent(wasm)];
  const matched = [];
  const handoffs = [];
  const quiescent = [];
  const banks = [wasmU32(wasm.reactionCurrentBank())];

  for (let step = 0; step < R5_OBSERVATION_STEPS; step += 1) {
    must(wasm.reactionRun() === 1, "CPU R5 reaction failed at step " + step);
    states.push(cpuCurrent(wasm));
    matched.push(wasmU32(wasm.reactionMatchedRelations()));
    handoffs.push(wasmU32(wasm.reactionHandoffCount()));
    quiescent.push(wasmU32(wasm.reactionQuiescent()) === 1);
    banks.push(wasmU32(wasm.reactionCurrentBank()));
  }

  return {
    states,
    matched,
    handoffs,
    quiescent,
    banks,
    snapshotCount: wasmU32(wasm.reactionSnapshotCount()),
    boundedReturn: true,
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

function createGpuReactionState(device, currentHandleOrHandles, relationHandleOrHandles) {
  const currentHandles = Array.isArray(currentHandleOrHandles)
    ? currentHandleOrHandles
    : [currentHandleOrHandles];
  const relationHandles = Array.isArray(relationHandleOrHandles)
    ? relationHandleOrHandles
    : [relationHandleOrHandles];
  must(currentHandles.length <= CAP, "GPU current fixture exceeds CAP");
  must(relationHandles.length <= CAP, "GPU theory fixture exceeds CAP");

  const selector = makeBuffer(device, 1, new Uint32Array([0]));
  const scopeWords = new Uint32Array(2 + CAP * 2);
  scopeWords.fill(NONE);
  scopeWords[0] = currentHandles.length;
  scopeWords[1] = 0;
  currentHandles.forEach((handle, index) => { scopeWords[2 + index] = handle; });
  const scope = makeBuffer(device, scopeWords.length, scopeWords);

  const theoryWords = new Uint32Array(1 + CAP);
  theoryWords.fill(NONE);
  theoryWords[0] = relationHandles.length;
  relationHandles.forEach((handle, index) => { theoryWords[1 + index] = handle; });
  const theory = makeBuffer(device, theoryWords.length, theoryWords);

  const snapshotWords = new Uint32Array(1 + CAP);
  snapshotWords.fill(NONE);
  snapshotWords[0] = relationHandles.length;
  relationHandles.forEach((handle, index) => { snapshotWords[1 + index] = handle; });
  const snapshot = makeBuffer(device, snapshotWords.length, snapshotWords);

  const status = makeBuffer(device, 5, new Uint32Array([0, 0, 0, 900, 0]));
  return { selector, scope, theory, snapshot, status };
}

function writeGpuLiveTheory(device, state, handles) {
  must(handles.length <= CAP, "GPU live Theory exceeds CAP");
  const words = new Uint32Array(1 + CAP);
  words.fill(NONE);
  words[0] = handles.length;
  handles.forEach((handle, index) => { words[1 + index] = handle; });
  device.queue.writeBuffer(state.theory, 0, words);
}

async function captureGpuTheorySnapshot(device, state) {
  const encoder = device.createCommandEncoder();
  encoder.copyBufferToBuffer(state.theory, 0, state.snapshot, 0, (1 + CAP) * 4);
  device.queue.submit([encoder.finish()]);
  await device.queue.onSubmittedWorkDone();
}

async function gpuTheoryCounts(device, state) {
  const [theory, snapshot] = await Promise.all([
    readWords(device, state.theory, 1 + CAP),
    readWords(device, state.snapshot, 1 + CAP),
  ]);
  return { live: theory[0], snapshot: snapshot[0] };
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
  "fn root_record(h: u32) -> bool {",
  "  return valid(h) && pool[h] == h && pool[END_BASE + h] == h;",
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
  "  status[0] = 0u; status[1] = 0u; status[2] = 0u; status[3] = 100u; status[4] = 0u;",
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
  "        let output = pool[END_BASE + relation];",
  "        matched = matched + 1u; member_matches = member_matches + 1u;",
  "        if (!root_record(output)) {",
  "          let candidate = find_pair(context, output);",
  "          if (candidate == NONE) { status[3] = 105u; return; }",
  "          var duplicate = false; var oi = 0u;",
  "          loop {",
  "            if (oi >= out_count) { break; }",
  "            if (scope[target_base + oi] == candidate) { duplicate = true; break; }",
  "            oi = oi + 1u;",
  "          }",
  "          if (!duplicate) {",
  "            if (out_count >= CAP) { status[3] = 106u; return; }",
  "            scope[target_base + out_count] = candidate;",
  "            out_count = out_count + 1u;",
  "          }",
  "        }",
  "      }",
  "      ri = ri + 1u;",
  "    }",
  "    if (member_matches == 0u) {",
  "      var duplicate_member = false; var pi = 0u;",
  "      loop {",
  "        if (pi >= out_count) { break; }",
  "        if (scope[target_base + pi] == member) { duplicate_member = true; break; }",
  "        pi = pi + 1u;",
  "      }",
  "      if (!duplicate_member) {",
  "        if (out_count >= CAP) { status[3] = 107u; return; }",
  "        scope[target_base + out_count] = member; out_count = out_count + 1u;",
  "      }",
  "    }",
  "    mi = mi + 1u;",
  "  }",
  "  status[1] = matched;",
  "  if (matched == 0u) { status[0] = 1u; status[3] = 200u + bank; status[4] = 1u; return; }",
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
    readWords(device, state.status, 5),
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
    quiescent: status[4] === 1,
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
      quiescent: after.quiescent,
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
      quiescent: after.quiescent,
    };
  } finally {
    destroyGpuState(state);
  }
}

async function runGpuInvalidFailure(device, pool, refs) {
  const state = createGpuReactionState(
    device,
    requireLocalRef(refs.current, pool.memory),
    requireLocalRef(refs.successor, pool.memory),
  );
  try {
    // Corrupt only the substrate current-member slot after constructing an
    // otherwise valid state. The kernel must fail closed and must not publish
    // semantic quiescence merely because no handoff occurs.
    device.queue.writeBuffer(state.scope, 8, new Uint32Array([NONE]));
    await gpuRun(device, pool, state);
    const status = await readWords(device, state.status, 5);
    return {
      result: status[0],
      handoff: status[2],
      diagnostic: status[3],
      quiescent: status[4] === 1,
    };
  } finally {
    destroyGpuState(state);
  }
}



async function runGpuZero(device, pool, refs) {
  const state = createGpuReactionState(
    device,
    requireLocalRef(refs.current, pool.memory),
    requireLocalRef(refs.zeroRelation, pool.memory),
  );
  try {
    const before = await gpuObserve(device, pool, state);
    await gpuRun(device, pool, state);
    const after = await gpuObserve(device, pool, state);
    must(after.status === 1, "GPU R3 ZERO failed: diagnostic=" + after.diagnostic);
    return {
      state: after.anums,
      oldPhysical: await gpuBank(device, pool, state, before.bank),
      matched: after.matched,
      handoff: after.handoff,
      quiescent: after.quiescent,
      oldBank: before.bank,
      newBank: after.bank,
    };
  } finally {
    destroyGpuState(state);
  }
}

async function runGpuMixedDuplicate(device, pool, refs) {
  const state = createGpuReactionState(
    device,
    [
      requireLocalRef(refs.current, pool.memory),
      requireLocalRef(refs.rootCurrent, pool.memory),
    ],
    [
      requireLocalRef(refs.zeroRelation, pool.memory),
      requireLocalRef(refs.relation, pool.memory),
      requireLocalRef(refs.rootRelation, pool.memory),
    ],
  );
  try {
    await gpuRun(device, pool, state);
    const after = await gpuObserve(device, pool, state);
    must(after.status === 1, "GPU R3 mixed/duplicate failed: diagnostic=" + after.diagnostic);
    return {
      state: after.anums,
      matched: after.matched,
      handoff: after.handoff,
      quiescent: after.quiescent,
    };
  } finally {
    destroyGpuState(state);
  }
}


async function runGpuBatch(device, pool, currentRefs, relationRefs) {
  const state = createGpuReactionState(
    device,
    currentRefs.map((ref) => requireLocalRef(ref, pool.memory)),
    relationRefs.map((ref) => requireLocalRef(ref, pool.memory)),
  );
  try {
    await gpuRun(device, pool, state);
    const after = await gpuObserve(device, pool, state);
    must(after.status === 1, "GPU R6 batch reaction failed: diagnostic=" + after.diagnostic);
    return {
      state: after.anums,
      matched: after.matched,
      handoff: after.handoff,
      quiescent: after.quiescent,
    };
  } finally {
    destroyGpuState(state);
  }
}

function runGpuOutOfScope(device, pool, refs) {
  const fiveDistinctValidPairs = [
    refs.current,
    refs.rootCurrent,
    refs.successor,
    refs.relation,
    refs.successorC,
  ].map((ref) => requireLocalRef(ref, pool.memory));
  try {
    const state = createGpuReactionState(
      device,
      fiveDistinctValidPairs,
      [requireLocalRef(refs.relation, pool.memory)],
    );
    destroyGpuState(state);
    return { rejected: false };
  } catch (error) {
    return {
      rejected: /exceeds CAP/.test(String(error?.message ?? error)),
      message: String(error?.message ?? error),
    };
  }
}

async function runGpuTheoryTplus1(device, pool, refs) {
  const relation = requireLocalRef(refs.relation, pool.memory);
  const relationBC = requireLocalRef(refs.relationBC, pool.memory);
  const state = createGpuReactionState(
    device,
    requireLocalRef(refs.current, pool.memory),
    relation,
  );
  try {
    const tBeforeObs = await gpuObserve(device, pool, state);
    const initialCounts = await gpuTheoryCounts(device, state);

    writeGpuLiveTheory(device, state, [relation, relationBC]);
    await device.queue.onSubmittedWorkDone();
    const liveCounts = await gpuTheoryCounts(device, state);

    await gpuRun(device, pool, state);
    const tAfterObs = await gpuObserve(device, pool, state);
    must(tAfterObs.status === 1, "GPU R4 reaction t failed: diagnostic=" + tAfterObs.diagnostic);

    const t1Before = tAfterObs.anums;
    await captureGpuTheorySnapshot(device, state);
    const t1Counts = await gpuTheoryCounts(device, state);
    await gpuRun(device, pool, state);
    const t1AfterObs = await gpuObserve(device, pool, state);
    must(t1AfterObs.status === 1, "GPU R4 reaction t+1 failed: diagnostic=" + t1AfterObs.diagnostic);

    // Independent stale-snapshot negative control.
    const stale = createGpuReactionState(
      device,
      requireLocalRef(refs.successor, pool.memory),
      relation,
    );
    try {
      writeGpuLiveTheory(device, stale, [relation, relationBC]);
      await device.queue.onSubmittedWorkDone();
      const staleCounts = await gpuTheoryCounts(device, stale);
      await gpuRun(device, pool, stale);
      const staleObs = await gpuObserve(device, pool, stale);
      must(staleObs.status === 1, "GPU R4 stale control failed: diagnostic=" + staleObs.diagnostic);

      return {
        tBefore: tBeforeObs.anums,
        tSnapshotCount: initialCounts.snapshot,
        liveTheoryCount: liveCounts.live,
        postAdmissionSnapshotCount: liveCounts.snapshot,
        tAfter: tAfterObs.anums,
        tMatched: tAfterObs.matched,
        tHandoff: tAfterObs.handoff,
        tQuiescent: tAfterObs.quiescent,
        t1Before,
        t1SnapshotCount: t1Counts.snapshot,
        t1After: t1AfterObs.anums,
        t1Matched: t1AfterObs.matched,
        t1Handoff: t1AfterObs.handoff,
        t1Quiescent: t1AfterObs.quiescent,
        staleState: staleObs.anums,
        staleMatched: staleObs.matched,
        staleSnapshotCount: staleCounts.snapshot,
      };
    } finally {
      destroyGpuState(stale);
    }
  } finally {
    destroyGpuState(state);
  }
}


async function runGpuRecurrenceEnd(device, pool, refs) {
  const state = createGpuReactionState(
    device,
    requireLocalRef(refs.stateStart, pool.memory),
    [
      requireLocalRef(refs.relationStartEnd, pool.memory),
      requireLocalRef(refs.relationEndStart, pool.memory),
    ],
  );
  try {
    const initial = await gpuObserve(device, pool, state);
    const states = [initial.anums];
    const matched = [];
    const handoffs = [];
    const quiescent = [];
    const banks = [initial.bank];

    for (let step = 0; step < R5_OBSERVATION_STEPS; step += 1) {
      await gpuRun(device, pool, state);
      const observed = await gpuObserve(device, pool, state);
      must(observed.status === 1, "GPU R5 reaction failed at step " + step + ": diagnostic=" + observed.diagnostic);
      states.push(observed.anums);
      matched.push(observed.matched);
      handoffs.push(observed.handoff);
      quiescent.push(observed.quiescent);
      banks.push(observed.bank);
    }

    const counts = await gpuTheoryCounts(device, state);
    return {
      states,
      matched,
      handoffs,
      quiescent,
      banks,
      snapshotCount: counts.snapshot,
      boundedReturn: true,
    };
  } finally {
    destroyGpuState(state);
  }
}

export async function runReactionBrowser(wasm, device) {
  assertR1Fixture();
  assertR2Fixture();
  assertR3Fixture();
  assertR4Fixture();
  assertR5Fixture();
  assertR6Fixture();
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
    must(cpu.quiescent === false && gpu.quiescent === false, "positive R1 incorrectly reported quiescence");
    must(cpu.snapshotCount === 1 && gpu.snapshotCount === 1, "snapshot count mismatch");

    const cpuNoMatch = runCpuNoMatch(wasm, cpuRefs);
    const gpuNoMatch = await runGpuNoMatch(device, gpuPool, gpuRefs);
    assertReactionStateExact([R1_FIXTURE.current], cpuNoMatch.state, "CPU no-match");
    assertReactionStateExact([R1_FIXTURE.current], gpuNoMatch.state, "GPU no-match");
    must(cpuNoMatch.matched === 0 && gpuNoMatch.matched === 0, "no-match relation was admitted");
    must(cpuNoMatch.handoff === 0 && gpuNoMatch.handoff === 0, "no-match performed handoff");
    must(cpuNoMatch.quiescent === true && gpuNoMatch.quiescent === true, "no-match did not report semantic quiescence");
    must(cpuNoMatch.beforeBank === cpuNoMatch.afterBank, "CPU no-match changed current Scope");
    must(gpuNoMatch.beforeBank === gpuNoMatch.afterBank, "GPU no-match changed current Scope");
    assertReactionStateExact(cpuNoMatch.state, gpuNoMatch.state, "CPU/GPU R2 quiescent differential");

    const cpuInvalid = runCpuInvalidFailure(wasm, cpuRefs);
    const gpuInvalid = await runGpuInvalidFailure(device, gpuPool, gpuRefs);
    must(cpuInvalid.result === 0 && gpuInvalid.result === 0, "invalid reaction unexpectedly succeeded");
    must(cpuInvalid.quiescent === false && gpuInvalid.quiescent === false, "failed reaction was misclassified as quiescent");
    must(cpuInvalid.handoff === 0 && gpuInvalid.handoff === 0, "failed reaction performed handoff");

    const cpuZero = runCpuZero(wasm, cpuRefs);
    const gpuZero = await runGpuZero(device, gpuPool, gpuRefs);
    assertReactionStateExact([], cpuZero.state, "CPU R3 ZERO successor");
    assertReactionStateExact([], gpuZero.state, "GPU R3 ZERO successor");
    assertReactionStateExact(cpuZero.state, gpuZero.state, "CPU/GPU R3 ZERO differential");
    assertReactionStateExact([R1_FIXTURE.current], cpuZero.oldPhysical, "CPU R3 ZERO old Scope");
    assertReactionStateExact([R1_FIXTURE.current], gpuZero.oldPhysical, "GPU R3 ZERO old Scope");
    must(cpuZero.matched === 1 && gpuZero.matched === 1, "R3 ZERO matchedRelations mismatch");
    must(cpuZero.handoff === 1 && gpuZero.handoff === 1, "R3 ZERO handoff mismatch");
    must(cpuZero.quiescent === false && gpuZero.quiescent === false, "R3 ZERO incorrectly reported quiescence");
    must(cpuZero.oldBank !== cpuZero.newBank && gpuZero.oldBank !== gpuZero.newBank, "R3 ZERO did not publish empty successor Scope");

    const cpuMixed = runCpuMixedDuplicate(wasm, cpuRefs);
    const gpuMixed = await runGpuMixedDuplicate(device, gpuPool, gpuRefs);
    assertReactionStateExact([R1_FIXTURE.successor], cpuMixed.state, "CPU R3 mixed successor");
    assertReactionStateExact([R1_FIXTURE.successor], gpuMixed.state, "GPU R3 mixed successor");
    assertReactionStateExact(cpuMixed.state, gpuMixed.state, "CPU/GPU R3 mixed differential");
    must(cpuMixed.state.length === 1 && gpuMixed.state.length === 1, "R3 duplicate outputs were not canonically converged");
    must(cpuMixed.matched === 3 && gpuMixed.matched === 3, "R3 mixed matchedRelations mismatch");
    must(cpuMixed.handoff === 1 && gpuMixed.handoff === 1, "R3 mixed handoff mismatch");
    must(cpuMixed.quiescent === false && gpuMixed.quiescent === false, "R3 mixed incorrectly reported quiescence");

    const cpuR4 = runCpuTheoryTplus1(wasm, cpuRefs);
    const gpuR4 = await runGpuTheoryTplus1(device, gpuPool, gpuRefs);

    assertReactionStateExact([R1_FIXTURE.current], cpuR4.tBefore, "CPU R4 t before");
    assertReactionStateExact([R1_FIXTURE.current], gpuR4.tBefore, "GPU R4 t before");
    must(cpuR4.tSnapshotCount === 1 && gpuR4.tSnapshotCount === 1, "R4 snapshot_t count mismatch");
    must(cpuR4.liveTheoryCount === 2 && gpuR4.liveTheoryCount === 2, "R4 live Theory count mismatch");
    must(gpuR4.postAdmissionSnapshotCount === 1, "GPU R4 live admission leaked into snapshot_t");

    assertReactionStateExact([R1_FIXTURE.successor], cpuR4.tAfter, "CPU R4 t after");
    assertReactionStateExact([R1_FIXTURE.successor], gpuR4.tAfter, "GPU R4 t after");
    assertReactionStateExact(cpuR4.tAfter, gpuR4.tAfter, "CPU/GPU R4 reaction t differential");
    must(cpuR4.tMatched === 1 && gpuR4.tMatched === 1, "R4 reaction t matched mismatch");
    must(cpuR4.tHandoff === 1 && gpuR4.tHandoff === 1, "R4 reaction t handoff mismatch");
    must(!cpuR4.tQuiescent && !gpuR4.tQuiescent, "R4 reaction t incorrectly quiescent");

    assertReactionStateExact([R1_FIXTURE.successor], cpuR4.t1Before, "CPU R4 t+1 before");
    assertReactionStateExact([R1_FIXTURE.successor], gpuR4.t1Before, "GPU R4 t+1 before");
    must(cpuR4.t1SnapshotCount === 2 && gpuR4.t1SnapshotCount === 2, "R4 snapshot_t+1 count mismatch");
    assertReactionStateExact([R4_FIXTURE.successorC], cpuR4.t1After, "CPU R4 t+1 after");
    assertReactionStateExact([R4_FIXTURE.successorC], gpuR4.t1After, "GPU R4 t+1 after");
    assertReactionStateExact(cpuR4.t1After, gpuR4.t1After, "CPU/GPU R4 trajectory differential");
    must(cpuR4.t1Matched === 1 && gpuR4.t1Matched === 1, "R4 reaction t+1 matched mismatch");
    must(cpuR4.t1Handoff === 1 && gpuR4.t1Handoff === 1, "R4 reaction t+1 handoff mismatch");
    must(!cpuR4.t1Quiescent && !gpuR4.t1Quiescent, "R4 reaction t+1 incorrectly quiescent");

    assertReactionStateExact([R1_FIXTURE.successor], cpuR4.staleState, "CPU R4 stale snapshot control");
    assertReactionStateExact([R1_FIXTURE.successor], gpuR4.staleState, "GPU R4 stale snapshot control");
    must(cpuR4.staleMatched === 0 && gpuR4.staleMatched === 0, "R4 stale snapshot saw new admission");
    must(gpuR4.staleSnapshotCount === 1, "GPU R4 stale control snapshot mutated");

    const cpuR5 = runCpuRecurrenceEnd(wasm, cpuRefs);
    const gpuR5 = await runGpuRecurrenceEnd(device, gpuPool, gpuRefs);
    const expectedR5 = [
      [R5_FIXTURE.stateStart],
      [R5_FIXTURE.stateEnd],
      [R5_FIXTURE.stateStart],
      [R5_FIXTURE.stateEnd],
      [R5_FIXTURE.stateStart],
    ];

    for (let i = 0; i < expectedR5.length; i += 1) {
      assertReactionStateExact(expectedR5[i], cpuR5.states[i], "CPU R5 S" + i);
      assertReactionStateExact(expectedR5[i], gpuR5.states[i], "GPU R5 S" + i);
      assertReactionStateExact(cpuR5.states[i], gpuR5.states[i], "CPU/GPU R5 S" + i + " differential");
    }
    must(cpuR5.snapshotCount === 2 && gpuR5.snapshotCount === 2, "R5 fixed TheorySnapshot count mismatch");
    must(cpuR5.matched.every((value) => value === 1) && gpuR5.matched.every((value) => value === 1), "R5 active-step matched mismatch");
    must(cpuR5.handoffs.every((value) => value === 1) && gpuR5.handoffs.every((value) => value === 1), "R5 active-step handoff mismatch");
    must(cpuR5.quiescent.every((value) => value === false) && gpuR5.quiescent.every((value) => value === false), "R5 recurrent cycle incorrectly quiescent");
    must(
      normalizeReactionState(cpuR5.states[0]).join("|") === normalizeReactionState(cpuR5.states[2]).join("|") &&
      normalizeReactionState(cpuR5.states[2]).join("|") === normalizeReactionState(cpuR5.states[4]).join("|") &&
      normalizeReactionState(gpuR5.states[0]).join("|") === normalizeReactionState(gpuR5.states[2]).join("|") &&
      normalizeReactionState(gpuR5.states[2]).join("|") === normalizeReactionState(gpuR5.states[4]).join("|"),
      "R5 semantic recurrence missing",
    );
    must(
      normalizeReactionState(cpuR5.states[1]).join("|") === normalizeReactionState(cpuR5.states[3]).join("|") &&
      normalizeReactionState(gpuR5.states[1]).join("|") === normalizeReactionState(gpuR5.states[3]).join("|"),
      "R5 END-state recurrence missing",
    );
    must(parseAnum(R5_FIXTURE.endValue).kind === "END", "R5 END fixture lost structural END aspect");
    must(
      cpuR5.states[1][0] === R5_FIXTURE.stateEnd && cpuR5.states[2][0] === R5_FIXTURE.stateStart &&
      gpuR5.states[1][0] === R5_FIXTURE.stateEnd && gpuR5.states[2][0] === R5_FIXTURE.stateStart,
      "R5 structural END incorrectly halted execution",
    );
    must(cpuR5.boundedReturn && gpuR5.boundedReturn, "R5 bounded witness did not return");

    // R6 closes the direct-evidence gaps found by global audit #45.
    // 1->N: one current truth must exhaust both admitted non-zero relations.
    const cpuR6OneToN = runCpuBatch(wasm, [cpuRefs.current], [cpuRefs.relation, cpuRefs.relationAC]);
    const gpuR6OneToN = await runGpuBatch(device, gpuPool, [gpuRefs.current], [gpuRefs.relation, gpuRefs.relationAC]);
    const r6Expected = [R1_FIXTURE.successor, R4_FIXTURE.successorC];
    assertReactionStateExact(r6Expected, cpuR6OneToN.state, "CPU R6 1->N");
    assertReactionStateExact(r6Expected, gpuR6OneToN.state, "GPU R6 1->N");
    assertReactionStateExact(cpuR6OneToN.state, gpuR6OneToN.state, "CPU/GPU R6 1->N differential");
    must(cpuR6OneToN.matched === 2 && gpuR6OneToN.matched === 2, "R6 1->N did not exhaust both admitted relations");
    must(cpuR6OneToN.handoff === 1 && gpuR6OneToN.handoff === 1, "R6 1->N handoff mismatch");

    // N->M: two current truths produce two distinct normalized successors.
    const cpuR6NToM = runCpuBatch(wasm, [cpuRefs.current, cpuRefs.rootCurrent], [cpuRefs.relation, cpuRefs.rootRelationC]);
    const gpuR6NToM = await runGpuBatch(device, gpuPool, [gpuRefs.current, gpuRefs.rootCurrent], [gpuRefs.relation, gpuRefs.rootRelationC]);
    assertReactionStateExact(r6Expected, cpuR6NToM.state, "CPU R6 N->M");
    assertReactionStateExact(r6Expected, gpuR6NToM.state, "GPU R6 N->M");
    assertReactionStateExact(cpuR6NToM.state, gpuR6NToM.state, "CPU/GPU R6 N->M differential");
    must(cpuR6NToM.matched === 2 && gpuR6NToM.matched === 2, "R6 N->M matched count mismatch");

    // P15 direct witness for this bounded prototype: reverse both physical
    // current iteration and Theory relation order; normalized semantics must not change.
    const cpuR6Reordered = runCpuBatch(wasm, [cpuRefs.rootCurrent, cpuRefs.current], [cpuRefs.rootRelationC, cpuRefs.relation]);
    const gpuR6Reordered = await runGpuBatch(device, gpuPool, [gpuRefs.rootCurrent, gpuRefs.current], [gpuRefs.rootRelationC, gpuRefs.relation]);
    assertReactionStateExact(cpuR6NToM.state, cpuR6Reordered.state, "CPU R6 reaction-order variation");
    assertReactionStateExact(gpuR6NToM.state, gpuR6Reordered.state, "GPU R6 reaction-order variation");
    assertReactionStateExact(cpuR6Reordered.state, gpuR6Reordered.state, "CPU/GPU R6 reordered differential");

    // Partial backends must reject structurally valid execution cardinalities
    // beyond their declared bounded substrate scope before semantic publication.
    const cpuR6OutOfScope = runCpuOutOfScope(wasm);
    const gpuR6OutOfScope = runGpuOutOfScope(device, gpuPool, gpuRefs);
    must(cpuR6OutOfScope.rejected && cpuR6OutOfScope.currentCount === 0 && cpuR6OutOfScope.handoff === 0,
      "CPU R6 out-of-scope execution did not fail closed");
    must(gpuR6OutOfScope.rejected, "GPU R6 out-of-scope execution did not fail closed");

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
    logs.push("reaction.r1.cpu.quiescent = " + cpu.quiescent);
    logs.push("reaction.r1.gpu.quiescent = " + gpu.quiescent);
    logs.push("reaction.r2.scope.cpu = [" + cpuNoMatch.state.join(", ") + "]");
    logs.push("reaction.r2.scope.gpu = [" + gpuNoMatch.state.join(", ") + "]");
    logs.push("reaction.r2.matched = CPU " + cpuNoMatch.matched + " / GPU " + gpuNoMatch.matched);
    logs.push("reaction.r2.handoff = CPU " + cpuNoMatch.handoff + " / GPU " + gpuNoMatch.handoff);
    logs.push("reaction.r2.quiescent = CPU " + cpuNoMatch.quiescent + " / GPU " + gpuNoMatch.quiescent);
    logs.push("reaction.r2.bank-unchanged = CPU " + cpuNoMatch.beforeBank + "->" + cpuNoMatch.afterBank + " / GPU " + gpuNoMatch.beforeBank + "->" + gpuNoMatch.afterBank);
    logs.push("reaction.r2.invalid-failure-quiescent = CPU " + cpuInvalid.quiescent + " / GPU " + gpuInvalid.quiescent);
    logs.push("reaction.r3.zero.scope.cpu = [" + cpuZero.state.join(", ") + "]");
    logs.push("reaction.r3.zero.scope.gpu = [" + gpuZero.state.join(", ") + "]");
    logs.push("reaction.r3.zero.matched = CPU " + cpuZero.matched + " / GPU " + gpuZero.matched);
    logs.push("reaction.r3.zero.handoff = CPU " + cpuZero.handoff + " / GPU " + gpuZero.handoff);
    logs.push("reaction.r3.zero.quiescent = CPU " + cpuZero.quiescent + " / GPU " + gpuZero.quiescent);
    logs.push("reaction.r3.zero.old-scope-retained = PASS");
    logs.push("reaction.r3.mixed.scope.cpu = [" + cpuMixed.state.join(", ") + "]");
    logs.push("reaction.r3.mixed.scope.gpu = [" + gpuMixed.state.join(", ") + "]");
    logs.push("reaction.r3.mixed.matched = CPU " + cpuMixed.matched + " / GPU " + gpuMixed.matched);
    logs.push("reaction.r3.mixed.handoff = CPU " + cpuMixed.handoff + " / GPU " + gpuMixed.handoff);
    logs.push("reaction.r3.mixed.quiescent = CPU " + cpuMixed.quiescent + " / GPU " + gpuMixed.quiescent);
    logs.push("reaction.r3.duplicate-convergence = PASS single canonical " + R1_FIXTURE.successor);
    logs.push("reaction.r4.t.before.cpu = [" + cpuR4.tBefore.join(", ") + "]");
    logs.push("reaction.r4.t.before.gpu = [" + gpuR4.tBefore.join(", ") + "]");
    logs.push("reaction.r4.t.snapshot-count = CPU " + cpuR4.tSnapshotCount + " / GPU " + gpuR4.tSnapshotCount);
    logs.push("reaction.r4.live-theory-count = CPU " + cpuR4.liveTheoryCount + " / GPU " + gpuR4.liveTheoryCount);
    logs.push("reaction.r4.t.after.cpu = [" + cpuR4.tAfter.join(", ") + "]");
    logs.push("reaction.r4.t.after.gpu = [" + gpuR4.tAfter.join(", ") + "]");
    logs.push("reaction.r4.t.matched = CPU " + cpuR4.tMatched + " / GPU " + gpuR4.tMatched);
    logs.push("reaction.r4.t.handoff = CPU " + cpuR4.tHandoff + " / GPU " + gpuR4.tHandoff);
    logs.push("reaction.r4.t+1.snapshot-count = CPU " + cpuR4.t1SnapshotCount + " / GPU " + gpuR4.t1SnapshotCount);
    logs.push("reaction.r4.t+1.before.cpu = [" + cpuR4.t1Before.join(", ") + "]");
    logs.push("reaction.r4.t+1.before.gpu = [" + gpuR4.t1Before.join(", ") + "]");
    logs.push("reaction.r4.t+1.after.cpu = [" + cpuR4.t1After.join(", ") + "]");
    logs.push("reaction.r4.t+1.after.gpu = [" + gpuR4.t1After.join(", ") + "]");
    logs.push("reaction.r4.t+1.matched = CPU " + cpuR4.t1Matched + " / GPU " + gpuR4.t1Matched);
    logs.push("reaction.r4.t+1.handoff = CPU " + cpuR4.t1Handoff + " / GPU " + gpuR4.t1Handoff);
    logs.push("reaction.r4.same-reaction-isolation = PASS");
    logs.push("reaction.r4.next-reaction-visibility = PASS");
    logs.push("reaction.r4.stale-snapshot-control = PASS");
    for (let i = 0; i < cpuR5.states.length; i += 1) {
      logs.push("reaction.r5.S" + i + ".cpu = [" + cpuR5.states[i].join(", ") + "]");
      logs.push("reaction.r5.S" + i + ".gpu = [" + gpuR5.states[i].join(", ") + "]");
    }
    logs.push("reaction.r5.snapshot-count = CPU " + cpuR5.snapshotCount + " / GPU " + gpuR5.snapshotCount);
    logs.push("reaction.r5.matched = CPU [" + cpuR5.matched.join(", ") + "] / GPU [" + gpuR5.matched.join(", ") + "]");
    logs.push("reaction.r5.handoff = CPU [" + cpuR5.handoffs.join(", ") + "] / GPU [" + gpuR5.handoffs.join(", ") + "]");
    logs.push("reaction.r5.quiescent = CPU [" + cpuR5.quiescent.join(", ") + "] / GPU [" + gpuR5.quiescent.join(", ") + "]");
    logs.push("reaction.r5.recurrence = PASS S0=S2=S4 and S1=S3");
    logs.push("reaction.r5.end-structure = PASS C=68 is END");
    logs.push("reaction.r5.end-continuation = PASS K->C -> K->A remains active");
    logs.push("reaction.r5.bounded-return = PASS");
    logs.push("reaction.r6.1-to-N.cpu = [" + cpuR6OneToN.state.join(", ") + "]");
    logs.push("reaction.r6.1-to-N.gpu = [" + gpuR6OneToN.state.join(", ") + "]");
    logs.push("reaction.r6.1-to-N.matched = CPU " + cpuR6OneToN.matched + " / GPU " + gpuR6OneToN.matched);
    logs.push("reaction.r6.N-to-M.cpu = [" + cpuR6NToM.state.join(", ") + "]");
    logs.push("reaction.r6.N-to-M.gpu = [" + gpuR6NToM.state.join(", ") + "]");
    logs.push("reaction.r6.order-variation = PASS");
    logs.push("reaction.r6.out-of-scope.cpu = PASS rejected count 17 > 16");
    logs.push("reaction.r6.out-of-scope.gpu = PASS rejected count 5 > 4");
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
      r2CpuState: cpuNoMatch.state,
      r2GpuState: gpuNoMatch.state,
      r2CpuMatched: cpuNoMatch.matched,
      r2GpuMatched: gpuNoMatch.matched,
      r2CpuHandoff: cpuNoMatch.handoff,
      r2GpuHandoff: gpuNoMatch.handoff,
      r2CpuQuiescent: cpuNoMatch.quiescent,
      r2GpuQuiescent: gpuNoMatch.quiescent,
      r2CpuBankUnchanged: cpuNoMatch.beforeBank === cpuNoMatch.afterBank,
      r2GpuBankUnchanged: gpuNoMatch.beforeBank === gpuNoMatch.afterBank,
      r2NormalizedDifferential: true,
      r2FailureNotQuiescent: cpuInvalid.quiescent === false && gpuInvalid.quiescent === false,
      r3ZeroCpuState: cpuZero.state,
      r3ZeroGpuState: gpuZero.state,
      r3ZeroCpuMatched: cpuZero.matched,
      r3ZeroGpuMatched: gpuZero.matched,
      r3ZeroCpuHandoff: cpuZero.handoff,
      r3ZeroGpuHandoff: gpuZero.handoff,
      r3ZeroCpuQuiescent: cpuZero.quiescent,
      r3ZeroGpuQuiescent: gpuZero.quiescent,
      r3ZeroOldScopeRetained: true,
      r3MixedCpuState: cpuMixed.state,
      r3MixedGpuState: gpuMixed.state,
      r3MixedCpuMatched: cpuMixed.matched,
      r3MixedGpuMatched: gpuMixed.matched,
      r3MixedCpuHandoff: cpuMixed.handoff,
      r3MixedGpuHandoff: gpuMixed.handoff,
      r3MixedCpuQuiescent: cpuMixed.quiescent,
      r3MixedGpuQuiescent: gpuMixed.quiescent,
      r3DuplicateConvergence: cpuMixed.state.length === 1 && gpuMixed.state.length === 1,
      r3NormalizedDifferential: true,
      r4TBeforeCpu: cpuR4.tBefore,
      r4TBeforeGpu: gpuR4.tBefore,
      r4TSnapshotCountCpu: cpuR4.tSnapshotCount,
      r4TSnapshotCountGpu: gpuR4.tSnapshotCount,
      r4LiveTheoryCountCpu: cpuR4.liveTheoryCount,
      r4LiveTheoryCountGpu: gpuR4.liveTheoryCount,
      r4TAfterCpu: cpuR4.tAfter,
      r4TAfterGpu: gpuR4.tAfter,
      r4TMatchedCpu: cpuR4.tMatched,
      r4TMatchedGpu: gpuR4.tMatched,
      r4THandoffCpu: cpuR4.tHandoff,
      r4THandoffGpu: gpuR4.tHandoff,
      r4T1SnapshotCountCpu: cpuR4.t1SnapshotCount,
      r4T1SnapshotCountGpu: gpuR4.t1SnapshotCount,
      r4T1BeforeCpu: cpuR4.t1Before,
      r4T1BeforeGpu: gpuR4.t1Before,
      r4T1AfterCpu: cpuR4.t1After,
      r4T1AfterGpu: gpuR4.t1After,
      r4T1MatchedCpu: cpuR4.t1Matched,
      r4T1MatchedGpu: gpuR4.t1Matched,
      r4T1HandoffCpu: cpuR4.t1Handoff,
      r4T1HandoffGpu: gpuR4.t1Handoff,
      r4SameReactionIsolation: true,
      r4NextReactionVisibility: true,
      r4StaleSnapshotControl: cpuR4.staleMatched === 0 && gpuR4.staleMatched === 0,
      r4NormalizedTrajectoryDifferential: true,
      r5StatesCpu: cpuR5.states,
      r5StatesGpu: gpuR5.states,
      r5SnapshotCountCpu: cpuR5.snapshotCount,
      r5SnapshotCountGpu: gpuR5.snapshotCount,
      r5MatchedCpu: cpuR5.matched,
      r5MatchedGpu: gpuR5.matched,
      r5HandoffCpu: cpuR5.handoffs,
      r5HandoffGpu: gpuR5.handoffs,
      r5QuiescentCpu: cpuR5.quiescent,
      r5QuiescentGpu: gpuR5.quiescent,
      r5Recurrence: true,
      r5EndStructure: true,
      r5EndContinuation: true,
      r5BoundedReturn: cpuR5.boundedReturn && gpuR5.boundedReturn,
      r5NormalizedTrajectoryDifferential: true,
      r6OneToNCpuState: cpuR6OneToN.state,
      r6OneToNGpuState: gpuR6OneToN.state,
      r6OneToNMatchedCpu: cpuR6OneToN.matched,
      r6OneToNMatchedGpu: gpuR6OneToN.matched,
      r6NToMCpuState: cpuR6NToM.state,
      r6NToMGpuState: gpuR6NToM.state,
      r6OrderVariationCpuState: cpuR6Reordered.state,
      r6OrderVariationGpuState: gpuR6Reordered.state,
      r6OrderVariation: true,
      r6CpuOutOfScopeRejected: cpuR6OutOfScope.rejected,
      r6GpuOutOfScopeRejected: gpuR6OutOfScope.rejected,
      r6NormalizedDifferential: true,
      negativeControls: true,
      logs,
    };
  } finally {
    destroyGpuAnumPool(gpuPool);
  }
}
