"use strict";

const crypto = require("crypto");
const fs = require("fs");
const path = require("path");
const acceptanceStarted = process.hrtime.bigint();
const registryPath = "experiments/browser-accelerator/web/i386-blocks.json";
const wasmPath =
  "experiments/a-circuit/target/wasm32-unknown-unknown/release/amemory_a_circuit.wasm";
const registryText = fs.readFileSync(registryPath, "utf8");
const registry = JSON.parse(registryText);
const bytes = fs.readFileSync(wasmPath);
const sourceSha =
  process.env.AMEMORY_SOURCE_SHA || process.env.GITHUB_SHA || null;
if (sourceSha !== null && !/^[0-9a-f]{40}$/.test(sourceSha)) {
  throw new Error("invalid source SHA for acceptance report");
}
const version = fs.readFileSync("VERSION", "utf8").trim();
if (!version) throw new Error("VERSION is empty");
WebAssembly.instantiate(bytes, {}).then(({instance}) => {
  const w = instance.exports;
  if (w.amemory_i386_lab_probe() !== 0x386) throw new Error("bad lab probe");
  for (const block of registry.blocks) {
    if (w.amemory_i386_lab_supports(block.opcode) !== 1) {
      throw new Error("registry/WASM mismatch: " + block.id);
    }
  }
  const proofCoveredOpcodes = new Set();

  // Real WASM execution smoke: composed structural MUX1.
  if (w.amemory_i386_lab_run(12, 1, 0, 1) !== 1) {
    throw new Error("MUX1 WASM execution rejected");
  }
  if (w.amemory_i386_lab_value() !== 0) {
    throw new Error("MUX1 WASM wrong result");
  }
  if (w.amemory_i386_lab_reactions() !== 7) {
    throw new Error("MUX1 WASM wrong reaction count");
  }
  if (w.amemory_i386_lab_quiescent() !== 1) {
    throw new Error("MUX1 WASM did not reach quiescence");
  }
  if (w.amemory_i386_lab_steady_link_delta() !== 0) {
    throw new Error("MUX1 repeated run grew Links");
  }
  if (w.amemory_i386_lab_proof_available() !== 1) {
    throw new Error("MUX1 structural proof JSON missing");
  }
  const proofLen = w.amemory_i386_lab_proof_json_len() >>> 0;
  const proofPtr = w.amemory_i386_lab_proof_json_ptr() >>> 0;
  if (!proofLen || proofPtr + proofLen > w.memory.buffer.byteLength) {
    throw new Error("MUX1 proof JSON pointer/length invalid");
  }
  const proofText = Buffer
    .from(w.memory.buffer, proofPtr, proofLen)
    .toString("utf8");
  const proof = JSON.parse(proofText);
  if (proof.schemaVersion !== 3) {
    throw new Error("MUX1 proof schema is not v3");
  }
  proofCoveredOpcodes.add(12);
  const memoryId = proof.load.memoryInstanceId;
  if (!memoryId || proof.execute.memoryInstanceId !== memoryId || proof.result.memoryInstanceId !== memoryId) {
    throw new Error("proof changed runtime A-memory identity");
  }
  if (proof.prepare.runtimeMemoryExists !== false) {
    throw new Error("runtime A-memory existed during PREPARE stage");
  }
  const roles = new Set(proof.prepare.semanticRoots.map((root) => root.role));
  for (const role of ["function.mux1", "function.dependency.xor2", "function.dependency.and2", "data.select", "data.a", "data.b", "execution.interpreter", "execution.theory", "invocation.call", "scope.initial"]) {
    if (!roles.has(role)) throw new Error("prepared Aset missing semantic root " + role);
  }
  if (!Array.isArray(proof.prepare.theoryAdmissions) || proof.prepare.theoryAdmissions.length === 0) {
    throw new Error("prepared Aset did not expose admitted Structural Rules");
  }
  if (!proof.load.carrierRoundTrip || proof.load.linksBeforeLoad !== 1 || proof.load.linksAfterLoad <= 1) {
    throw new Error("packed carrier load witness failed");
  }
  if (proof.load.linksAfterLoad !== proof.prepare.compiledLinks) {
    throw new Error("runtime A-memory does not contain the complete compiled Aset");
  }
  if (!proof.execute.reactions.every((step) => step.memoryInstanceId === memoryId)) {
    throw new Error("reaction trace used more than one A-memory");
  }
  if (!proof.execute.finalQuiescent || proof.execute.activeReactionCount !== 7) {
    throw new Error("MUX1 proof execution trajectory mismatch");
  }
  if (!proof.result.oracleMatches || proof.result.identicalRerunLinkDelta !== 0) {
    throw new Error("MUX1 proof result/or rerun witness failed");
  }
  if (proof.result.visualLinks.length !== proof.result.linksFinal) {
    throw new Error("visual snapshot does not contain every runtime Link");
  }
  const visualKeys = new Set(proof.result.visualLinks.map((link) => link.key));
  for (const link of proof.result.visualLinks) {
    if (!link.key.startsWith(memoryId + ":L")) throw new Error("visual Link from foreign memory");
    if (!visualKeys.has(link.startKey) || !visualKeys.has(link.endKey)) {
      throw new Error("visual topology has foreign/missing pole");
    }
  }
  const run = (op, a, b, flag, expected, label) => {
    if (w.amemory_i386_lab_run(op, a, b, flag) !== 1) {
      throw new Error(label + " WASM execution rejected");
    }
    if ((w.amemory_i386_lab_value() >>> 0) !== (expected >>> 0)) {
      throw new Error(label + " WASM wrong result");
    }
    if (w.amemory_i386_lab_quiescent() !== 1) {
      throw new Error(label + " WASM did not reach quiescence");
    }
    if (w.amemory_i386_lab_steady_link_delta() !== 0) {
      throw new Error(label + " repeated run grew Links");
    }
  };


  const readCurrentProof = (label) => {
    if (w.amemory_i386_lab_proof_available() !== 1) {
      throw new Error(label + " structural proof JSON missing");
    }
    const len = w.amemory_i386_lab_proof_json_len() >>> 0;
    const ptr = w.amemory_i386_lab_proof_json_ptr() >>> 0;
    if (!len || ptr + len > w.memory.buffer.byteLength) {
      throw new Error(label + " proof JSON pointer/length invalid");
    }
    const proof = JSON.parse(
      Buffer.from(w.memory.buffer, ptr, len).toString("utf8")
    );
    if (proof.schemaVersion !== 3) {
      throw new Error(label + " proof schema is not v3");
    }
    const registryBlock = registry.blocks.find(
      (block) => block.name === proof.block
    );
    if (!registryBlock) {
      throw new Error(label + " proof block is absent from registry: " + proof.block);
    }
    proofCoveredOpcodes.add(registryBlock.opcode);
    if (!Array.isArray(proof.prepare?.theoryAdmissions) ||
        proof.prepare.theoryAdmissions.length === 0 ||
        !proof.prepare.theoryAdmissions.every(
          (ref) => typeof ref === "string" && /^L\d+$/.test(ref)
        )) {
      throw new Error(label + " Theory admissions are not packed-carrier Link references");
    }
    return proof;
  };

  const verifyLogicProof = (op, a, b, expected, expectedWriteback, block) => {
    run(op, a, b, 0, expected, block);
    if ((w.amemory_i386_lab_writeback() >>> 0) !== (expectedWriteback >>> 0)) {
      throw new Error(block + " WASM writeback mismatch");
    }
    const proof = readCurrentProof(block);
    if (proof.block !== block) {
      throw new Error(block + " proof block identity mismatch");
    }

    const memoryId = proof.load.memoryInstanceId;
    if (!memoryId ||
        proof.execute.memoryInstanceId !== memoryId ||
        proof.result.memoryInstanceId !== memoryId) {
      throw new Error(block + " proof changed runtime A-memory identity");
    }
    if (proof.prepare.runtimeMemoryExists !== false) {
      throw new Error(block + " runtime A-memory existed during PREPARE");
    }
    if (!Array.isArray(proof.prepare.theoryAdmissions) ||
        proof.prepare.theoryAdmissions.length === 0) {
      throw new Error(block + " proof did not expose Theory admissions");
    }
    if (!proof.load.carrierRoundTrip ||
        proof.load.linksBeforeLoad !== 1 ||
        proof.load.linksAfterLoad !== proof.prepare.compiledLinks) {
      throw new Error(block + " packed carrier load witness failed");
    }
    if (!proof.execute.finalQuiescent ||
        !proof.execute.reactions.every((step) => step.memoryInstanceId === memoryId)) {
      throw new Error(block + " execution witness is not one-memory/quiescent");
    }
    if (!proof.result.oracleMatches ||
        proof.result.identicalRerunLinkDelta !== 0 ||
        (proof.result.decodedValue >>> 0) !== (expected >>> 0)) {
      throw new Error(block + " structural result/oracle/rerun witness failed");
    }

    const roles = new Set(
      proof.prepare.semanticRoots.map((root) => root.role)
    );
    for (const role of [
      "data.a.word",
      "data.bit.zero",
      "data.bit.one",
      "execution.interpreter",
      "execution.theory",
      "execution.apply",
      "invocation.call",
      "scope.initial",
      "result.tag",
    ]) {
      if (!roles.has(role)) {
        throw new Error(block + " prepared Aset missing semantic root " + role);
      }
    }
    if (op === 4) {
      for (const role of ["function.effect.not", "function.word_not", "function.gate.not1"]) {
        if (!roles.has(role)) {
          throw new Error(block + " prepared Aset missing " + role);
        }
      }
    } else {
      if (!roles.has("function.effect.binary") ||
          !roles.has("function.word_binary") ||
          !roles.has("data.b.word") ||
          !roles.has("data.writeback") ||
          ![...roles].some((role) => role.startsWith("function.gate."))) {
        throw new Error(block + " prepared Aset missing binary logic structure");
      }
      const sourceByRole = new Map(
        proof.prepare.semanticRoots.map((root) => [root.role, root.source])
      );
      const expectedBitRole =
        expectedWriteback === 0 ? "data.bit.zero" : "data.bit.one";
      if (sourceByRole.get("data.writeback") !== sourceByRole.get(expectedBitRole)) {
        throw new Error(block + " structural writeback bit mismatch");
      }
    }

    if (proof.result.visualLinks.length !== proof.result.linksFinal) {
      throw new Error(block + " visual snapshot does not contain every runtime Link");
    }
    const visualKeys = new Set(
      proof.result.visualLinks.map((link) => link.key)
    );
    for (const link of proof.result.visualLinks) {
      if (!link.key.startsWith(memoryId + ":L") ||
          !visualKeys.has(link.startKey) ||
          !visualKeys.has(link.endKey)) {
        throw new Error(block + " visual topology has a foreign/missing Link pole");
      }
    }
  };

  verifyLogicProof(1, 0xaaaaaaaa, 0x0f0f0f0f, 0x0a0a0a0a, 1, "AND32");
  verifyLogicProof(2, 0xaaaa0000, 0x00005555, 0xaaaa5555, 1, "OR32");
  verifyLogicProof(3, 0xffff0000, 0x0f0f0f0f, 0xf0f00f0f, 1, "XOR32");
  verifyLogicProof(4, 0x12345678, 0, 0xedcba987, 1, "NOT32");
  verifyLogicProof(5, 0xf0f01234, 0x0ff0ffff, 0x00f01234, 0, "TEST32");

  const verifyArithmeticProof = (
    op, a, b, inputFlag, expected, expectedWriteback,
    expectedX, expectedMode, block
  ) => {
    run(op, a, b, inputFlag, expected, block);
    if ((w.amemory_i386_lab_writeback() >>> 0) !== (expectedWriteback >>> 0)) {
      throw new Error(block + " WASM writeback mismatch");
    }
    const proof = readCurrentProof(block);
    if (proof.block !== block) {
      throw new Error(block + " proof block identity mismatch");
    }

    const memoryId = proof.load.memoryInstanceId;
    if (!memoryId ||
        proof.execute.memoryInstanceId !== memoryId ||
        proof.result.memoryInstanceId !== memoryId ||
        !proof.execute.reactions.every((step) => step.memoryInstanceId === memoryId)) {
      throw new Error(block + " proof changed runtime A-memory identity");
    }
    if (proof.prepare.runtimeMemoryExists !== false ||
        !proof.load.carrierRoundTrip ||
        proof.load.linksBeforeLoad !== 1 ||
        proof.load.linksAfterLoad !== proof.prepare.compiledLinks) {
      throw new Error(block + " packed carrier witness failed");
    }
    if (!proof.execute.finalQuiescent ||
        !proof.result.oracleMatches ||
        proof.result.identicalRerunLinkDelta !== 0 ||
        (proof.result.decodedValue >>> 0) !== (expected >>> 0)) {
      throw new Error(block + " arithmetic structural result witness failed");
    }

    const sourceByRole = new Map(
      proof.prepare.semanticRoots.map((root) => [root.role, root.source])
    );
    for (const role of [
      "function.effect.arithmetic",
      "function.flagged_arithmetic",
      "data.a.word",
      "data.b.word",
      "data.x",
      "data.mode",
      "data.writeback",
      "data.bit.zero",
      "data.bit.one",
      "execution.interpreter",
      "execution.theory",
      "execution.apply",
      "invocation.call",
      "scope.initial",
      "result.tag",
    ]) {
      if (!sourceByRole.has(role)) {
        throw new Error(block + " prepared Aset missing " + role);
      }
    }
    const bitSource = (value) =>
      sourceByRole.get(value === 0 ? "data.bit.zero" : "data.bit.one");
    if (sourceByRole.get("data.x") !== bitSource(expectedX) ||
        sourceByRole.get("data.mode") !== bitSource(expectedMode) ||
        sourceByRole.get("data.writeback") !== bitSource(expectedWriteback)) {
      throw new Error(block + " structural X/Mode/WriteBack selector mismatch");
    }

    if (proof.result.visualLinks.length !== proof.result.linksFinal) {
      throw new Error(block + " visual snapshot does not contain every runtime Link");
    }
  };

  verifyArithmeticProof(6, 0xffffffff, 1, 0, 0, 1, 0, 0, "ADD32");
  verifyArithmeticProof(7, 0xffffffff, 0, 1, 0, 1, 1, 0, "ADC32");
  verifyArithmeticProof(8, 0, 1, 0, 0xffffffff, 1, 0, 1, "SUB32");
  verifyArithmeticProof(9, 0, 0, 1, 0xffffffff, 1, 1, 1, "SBB32");
  verifyArithmeticProof(10, 7, 9, 0, 0xfffffffe, 0, 0, 1, "CMP32");

  const verifyMux32Proof = (select, a, b, expected) => {
    run(11, a, b, select, expected, "MUX32");
    const proof = readCurrentProof("MUX32");
    if (proof.block !== "MUX32") {
      throw new Error("MUX32 proof block identity mismatch");
    }
    const memoryId = proof.load.memoryInstanceId;
    if (!memoryId ||
        proof.execute.memoryInstanceId !== memoryId ||
        proof.result.memoryInstanceId !== memoryId ||
        !proof.execute.reactions.every((step) => step.memoryInstanceId === memoryId)) {
      throw new Error("MUX32 proof changed runtime A-memory identity");
    }
    if (!proof.load.carrierRoundTrip ||
        proof.load.linksBeforeLoad !== 1 ||
        proof.load.linksAfterLoad !== proof.prepare.compiledLinks ||
        proof.execute.activeReactionCount !== 257 ||
        !proof.execute.finalQuiescent ||
        !proof.result.oracleMatches ||
        proof.result.identicalRerunLinkDelta !== 0 ||
        (proof.result.decodedValue >>> 0) !== (expected >>> 0)) {
      throw new Error("MUX32 structural proof witness failed");
    }

    const sourceByRole = new Map(
      proof.prepare.semanticRoots.map((root) => [root.role, root.source])
    );
    for (const role of [
      "function.mux32",
      "function.mux1",
      "function.gate.xor2",
      "function.gate.and2",
      "data.select",
      "data.a.word",
      "data.b.word",
      "data.bit.zero",
      "data.bit.one",
      "execution.interpreter",
      "execution.theory",
      "execution.apply",
      "invocation.call",
      "scope.initial",
      "result.word_tag",
    ]) {
      if (!sourceByRole.has(role)) {
        throw new Error("MUX32 prepared Aset missing " + role);
      }
    }
    const selectBit =
      sourceByRole.get(select === 0 ? "data.bit.zero" : "data.bit.one");
    if (sourceByRole.get("data.select") !== selectBit) {
      throw new Error("MUX32 structural select mismatch");
    }
    if (proof.result.visualLinks.length !== proof.result.linksFinal) {
      throw new Error("MUX32 visual snapshot is incomplete");
    }
  };

  verifyMux32Proof(1, 0xaaaaaaaa, 0x55555555, 0x55555555);

  const runWide = (op, a, b, expectedLo, expectedHi, reactions, label) => {
    if (w.amemory_i386_lab_run(op, a, b, 0) !== 1) {
      throw new Error(label + " WASM execution rejected");
    }
    if ((w.amemory_i386_lab_value() >>> 0) !== (expectedLo >>> 0)) {
      throw new Error(label + " WASM wrong low half");
    }
    if ((w.amemory_i386_lab_value_hi() >>> 0) !== (expectedHi >>> 0)) {
      throw new Error(label + " WASM wrong high half");
    }
    if (w.amemory_i386_lab_reactions() !== reactions) {
      throw new Error(label + " WASM wrong reaction count");
    }
    if (w.amemory_i386_lab_quiescent() !== 1) {
      throw new Error(label + " WASM did not reach quiescence");
    }
    if (w.amemory_i386_lab_steady_link_delta() !== 0) {
      throw new Error(label + " repeated run grew Links");
    }
  };

  const verifyShiftProof = (op, value, count, expected, block) => {
    run(op, value, count, 0, expected, block + " count=" + count);
    const proof = readCurrentProof(block);
    if (proof.block !== block) {
      throw new Error(block + " proof block identity mismatch");
    }
    const memoryId = proof.load.memoryInstanceId;
    if (!memoryId ||
        proof.execute.memoryInstanceId !== memoryId ||
        proof.result.memoryInstanceId !== memoryId ||
        !proof.execute.reactions.every((step) => step.memoryInstanceId === memoryId)) {
      throw new Error(block + " proof changed runtime A-memory identity");
    }
    if (proof.prepare.runtimeMemoryExists !== false ||
        !proof.load.carrierRoundTrip ||
        proof.load.linksBeforeLoad !== 1 ||
        proof.load.linksAfterLoad !== proof.prepare.compiledLinks ||
        !proof.execute.finalQuiescent ||
        !proof.result.oracleMatches ||
        proof.result.identicalRerunLinkDelta !== 0 ||
        (proof.result.decodedValue >>> 0) !== (expected >>> 0)) {
      throw new Error(block + " structural shift witness failed");
    }

    const sourceByRole = new Map(
      proof.prepare.semanticRoots.map((root) => [root.role, root.source])
    );
    for (const role of [
      "function.shift.selected",
      "function.shift.shl",
      "function.shift.shr",
      "function.shift.sar",
      "data.value.word",
      "data.count.word",
      "execution.interpreter",
      "execution.theory",
      "execution.apply",
      "invocation.call",
      "scope.initial",
      "result.tag",
      "result.flag.set_tag",
      "result.flag.undefined_tag",
    ]) {
      if (!sourceByRole.has(role)) {
        throw new Error(block + " prepared Aset missing " + role);
      }
    }
    const selectedRole = op === 13
      ? "function.shift.shl"
      : op === 14
        ? "function.shift.shr"
        : "function.shift.sar";
    if (sourceByRole.get("function.shift.selected") !== sourceByRole.get(selectedRole)) {
      throw new Error(block + " selected structural function mismatch");
    }
    if (proof.result.visualLinks.length !== proof.result.linksFinal) {
      throw new Error(block + " visual snapshot is incomplete");
    }
    return {
      countSource: sourceByRole.get("data.count.word"),
      value: w.amemory_i386_lab_value() >>> 0,
      defined: w.amemory_i386_lab_defined_mask() >>> 0,
      values: w.amemory_i386_lab_value_mask() >>> 0,
      undefined: w.amemory_i386_lab_undefined_mask() >>> 0,
      preserve: w.amemory_i386_lab_preserve_mask() >>> 0,
      reactions: w.amemory_i386_lab_reactions() >>> 0,
    };
  };

  const shl1 = verifyShiftProof(13, 0x80000001, 1, 0x00000002, "SHL32");
  verifyShiftProof(14, 0x80000001, 1, 0x40000000, "SHR32");
  verifyShiftProof(15, 0x80000001, 1, 0xc0000000, "SAR32");
  const shl32 = verifyShiftProof(13, 0x80000001, 32, 0x80000001, "SHL32");
  if (shl32.defined !== 0 ||
      shl32.undefined !== 0 ||
      shl32.preserve !== 0x000008d5 ||
      shl32.reactions !== 1) {
    throw new Error("masked-zero SHL32 must preserve all status flags in one structural reaction");
  }
  const shl33 = verifyShiftProof(13, 0x80000001, 33, 0x00000002, "SHL32");
  if (shl1.countSource === shl33.countSource) {
    throw new Error("Count8 1 and 33 must be distinct structural inputs before masking");
  }
  for (const key of ["value", "defined", "values", "undefined", "preserve", "reactions"]) {
    if (shl1[key] !== shl33[key]) {
      throw new Error("Count8 1 and 33 failed structural masking equivalence: " + key);
    }
  }
  const verifyRotateProof = (
    op, value, count, cfIn, expected, block, carry
  ) => {
    run(op, value, count, cfIn, expected, block + " count=" + count);
    const proof = readCurrentProof(block);
    const memoryId = proof.load.memoryInstanceId;
    if (proof.block !== block ||
        !memoryId ||
        proof.execute.memoryInstanceId !== memoryId ||
        proof.result.memoryInstanceId !== memoryId ||
        !proof.execute.reactions.every((step) => step.memoryInstanceId === memoryId) ||
        !proof.load.carrierRoundTrip ||
        !proof.execute.finalQuiescent ||
        !proof.result.oracleMatches ||
        proof.result.identicalRerunLinkDelta !== 0 ||
        (proof.result.decodedValue >>> 0) !== (expected >>> 0)) {
      throw new Error(block + " structural rotate witness failed");
    }

    const sourceByRole = new Map(
      proof.prepare.semanticRoots.map((root) => [root.role, root.source])
    );
    const required = carry
      ? [
          "function.rotate_carry.selected",
          "function.rotate_carry.rcl",
          "function.rotate_carry.rcr",
          "function.gate.xor2",
          "data.value.word",
          "data.count.word",
          "data.cf_in",
          "data.bit.zero",
          "data.bit.one",
          "execution.interpreter",
          "execution.theory",
          "execution.apply",
          "invocation.call",
          "scope.initial",
          "result.tag",
          "result.flag.set_tag",
          "result.flag.undefined_tag",
        ]
      : [
          "function.rotate.selected",
          "function.rotate.rol",
          "function.rotate.ror",
          "function.gate.xor2",
          "data.value.word",
          "data.count.word",
          "execution.interpreter",
          "execution.theory",
          "execution.apply",
          "invocation.call",
          "scope.initial",
          "result.tag",
          "result.flag.set_tag",
          "result.flag.undefined_tag",
        ];
    for (const role of required) {
      if (!sourceByRole.has(role)) {
        throw new Error(block + " prepared Aset missing " + role);
      }
    }

    const selectedRole = carry
      ? (op === 18 ? "function.rotate_carry.rcl" : "function.rotate_carry.rcr")
      : (op === 16 ? "function.rotate.rol" : "function.rotate.ror");
    const selectedKey = carry
      ? "function.rotate_carry.selected"
      : "function.rotate.selected";
    if (sourceByRole.get(selectedKey) !== sourceByRole.get(selectedRole)) {
      throw new Error(block + " selected structural function mismatch");
    }
    if (carry) {
      const cfSource = sourceByRole.get(
        cfIn === 0 ? "data.bit.zero" : "data.bit.one"
      );
      if (sourceByRole.get("data.cf_in") !== cfSource) {
        throw new Error(block + " CF-in is not a structural bit input");
      }
    }
    if (proof.result.visualLinks.length !== proof.result.linksFinal) {
      throw new Error(block + " visual snapshot is incomplete");
    }

    return {
      countSource: sourceByRole.get("data.count.word"),
      value: w.amemory_i386_lab_value() >>> 0,
      defined: w.amemory_i386_lab_defined_mask() >>> 0,
      values: w.amemory_i386_lab_value_mask() >>> 0,
      undefined: w.amemory_i386_lab_undefined_mask() >>> 0,
      preserve: w.amemory_i386_lab_preserve_mask() >>> 0,
      reactions: w.amemory_i386_lab_reactions() >>> 0,
    };
  };

  const rol1 = verifyRotateProof(16, 0x80000001, 1, 0, 0x00000003, "ROL32", false);
  verifyRotateProof(17, 0x00000001, 1, 0, 0x80000000, "ROR32", false);
  const rol2 = verifyRotateProof(16, 0x80000001, 2, 0, 0x00000006, "ROL32", false);
  const rol32 = verifyRotateProof(16, 0x80000001, 32, 0, 0x80000001, "ROL32", false);
  const rol33 = verifyRotateProof(16, 0x80000001, 33, 0, 0x00000003, "ROL32", false);
  if (rol1.defined !== 0x00000801 || rol1.undefined !== 0 ||
      rol2.defined !== 0x00000001 || rol2.undefined !== 0x00000800 ||
      rol32.defined !== 0 || rol32.undefined !== 0 ||
      rol32.preserve !== 0x000008d5 || rol32.reactions !== 1 ||
      rol1.countSource === rol33.countSource) {
    throw new Error("ROL32 flag/masking proof failed");
  }
  for (const key of ["value", "defined", "values", "undefined", "preserve", "reactions"]) {
    if (rol1[key] !== rol33[key]) {
      throw new Error("ROL32 Count8 1/33 masking mismatch: " + key);
    }
  }

  const rcl1 = verifyRotateProof(18, 0x80000000, 1, 1, 0x00000001, "RCL32", true);
  verifyRotateProof(19, 0x00000001, 1, 0, 0x00000000, "RCR32", true);
  const rcl2 = verifyRotateProof(18, 0x80000000, 2, 1, 0x00000003, "RCL32", true);
  const rcl32 = verifyRotateProof(18, 0x80000000, 32, 1, 0x80000000, "RCL32", true);
  const rcl33 = verifyRotateProof(18, 0x80000000, 33, 1, 0x00000001, "RCL32", true);
  if (rcl1.defined !== 0x00000801 || rcl1.undefined !== 0 ||
      rcl2.defined !== 0x00000001 || rcl2.undefined !== 0x00000800 ||
      rcl32.defined !== 0 || rcl32.undefined !== 0 ||
      rcl32.preserve !== 0x000008d5 || rcl32.reactions !== 1 ||
      rcl1.countSource === rcl33.countSource) {
    throw new Error("RCL32 flag/masking proof failed");
  }
  for (const key of ["value", "defined", "values", "undefined", "preserve", "reactions"]) {
    if (rcl1[key] !== rcl33[key]) {
      throw new Error("RCL32 Count8 1/33 masking mismatch: " + key);
    }
  }
  const verifyUnaryProof = (
    op, value, expected, block, cfDefined, cfSet
  ) => {
    run(op, value, 0, 0, expected, block);
    const proof = readCurrentProof(block);
    const memoryId = proof.load.memoryInstanceId;
    if (proof.block !== block ||
        !memoryId ||
        proof.execute.memoryInstanceId !== memoryId ||
        proof.result.memoryInstanceId !== memoryId ||
        !proof.execute.reactions.every((step) => step.memoryInstanceId === memoryId) ||
        !proof.load.carrierRoundTrip ||
        !proof.execute.finalQuiescent ||
        !proof.result.oracleMatches ||
        proof.result.identicalRerunLinkDelta !== 0 ||
        (proof.result.decodedValue >>> 0) !== (expected >>> 0)) {
      throw new Error(block + " structural unary witness failed");
    }

    const sourceByRole = new Map(
      proof.prepare.semanticRoots.map((root) => [root.role, root.source])
    );
    for (const role of [
      "function.unary.selected",
      "function.unary.inc",
      "function.unary.dec",
      "function.unary.neg",
      "function.flagged_arithmetic",
      "result.flagged_tag",
      "data.value.word",
      "execution.interpreter",
      "execution.theory",
      "execution.apply",
      "invocation.call",
      "scope.initial",
      "result.tag",
      "result.flag.set_tag",
      "result.flag.cf",
      "result.flag.pf",
      "result.flag.af",
      "result.flag.zf",
      "result.flag.sf",
      "result.flag.of",
    ]) {
      if (!sourceByRole.has(role)) {
        throw new Error(block + " prepared Aset missing " + role);
      }
    }
    const selectedRole = op === 20
      ? "function.unary.inc"
      : op === 21
        ? "function.unary.dec"
        : "function.unary.neg";
    if (sourceByRole.get("function.unary.selected") !== sourceByRole.get(selectedRole)) {
      throw new Error(block + " selected structural function mismatch");
    }

    const defined = w.amemory_i386_lab_defined_mask() >>> 0;
    const values = w.amemory_i386_lab_value_mask() >>> 0;
    const undefined = w.amemory_i386_lab_undefined_mask() >>> 0;
    const preserve = w.amemory_i386_lab_preserve_mask() >>> 0;
    if (undefined !== 0) {
      throw new Error(block + " unexpectedly produced undefined status flags");
    }
    if (cfDefined) {
      if ((defined & 1) !== 1 || (preserve & 1) !== 0 ||
          ((values & 1) !== 0) !== cfSet) {
        throw new Error(block + " CF must be structurally defined by the six-action patch");
      }
    } else {
      if ((defined & 1) !== 0 || (preserve & 1) !== 1) {
        throw new Error(block + " CF must be structurally absent/preserved by the five-action patch");
      }
    }
    if (proof.result.visualLinks.length !== proof.result.linksFinal) {
      throw new Error(block + " visual snapshot is incomplete");
    }
  };

  verifyUnaryProof(20, 0x7fffffff, 0x80000000, "INC32", false, false);
  verifyUnaryProof(21, 0x80000000, 0x7fffffff, "DEC32", false, false);
  verifyUnaryProof(22, 0x00000001, 0xffffffff, "NEG32", true, true);
  verifyUnaryProof(22, 0x00000000, 0x00000000, "NEG32", true, false);

  runWide(23, 0xffffffff, 2, 0xfffffffe, 1, 33 + 1038, "MUL32 raw Wide64");
  if ((w.amemory_i386_lab_preserve_mask() >>> 0) !== 0x000008d5) {
    throw new Error("MUL32 raw must preserve status flags");
  }
  const rawMulProof = readCurrentProof("MUL32 raw Wide64");
  if (rawMulProof.block !== "MUL32 raw Wide64" ||
      (rawMulProof.result.decodedValue >>> 0) !== 0xfffffffe ||
      (rawMulProof.result.decodedValueHi >>> 0) !== 1 ||
      (rawMulProof.result.oracleValue >>> 0) !== 0xfffffffe ||
      (rawMulProof.result.oracleValueHi >>> 0) !== 1 ||
      !rawMulProof.result.oracleMatches ||
      rawMulProof.result.identicalRerunLinkDelta !== 0 ||
      !rawMulProof.execute.finalQuiescent) {
    throw new Error("MUL32 raw Wide64 structural proof result failed");
  }
  const rawMulRoles = new Set(
    rawMulProof.prepare.semanticRoots.map((root) => root.role)
  );
  for (const role of [
    "function.mul32",
    "function.dependency.add64",
    "result.dependency.add64_tag",
    "data.a.word",
    "data.b.word",
    "execution.interpreter",
    "execution.theory",
    "execution.apply",
    "invocation.call",
    "scope.initial",
    "result.tag",
  ]) {
    if (!rawMulRoles.has(role)) {
      throw new Error("MUL32 raw proof missing semantic root " + role);
    }
  }
  if (rawMulProof.result.visualLinks.length !== rawMulProof.result.linksFinal) {
    throw new Error("MUL32 raw visual snapshot is incomplete");
  }

  runWide(24, 0xffffffff, 2, 0xfffffffe, 1, 97 + 1038, "x86 MUL32 effect");
  if ((w.amemory_i386_lab_defined_mask() >>> 0) !== 0x00000801) {
    throw new Error("x86 MUL32 wrong defined flag mask");
  }
  if ((w.amemory_i386_lab_value_mask() >>> 0) !== 0x00000801) {
    throw new Error("x86 MUL32 must set CF and OF when high half is non-zero");
  }
  if ((w.amemory_i386_lab_undefined_mask() >>> 0) !== 0x000000d4) {
    throw new Error("x86 MUL32 wrong undefined flag mask");
  }
  if ((w.amemory_i386_lab_preserve_mask() >>> 0) !== 0) {
    throw new Error("x86 MUL32 must not preserve status flags");
  }
  const mulEffectProof = readCurrentProof("x86 MUL32 effect");
  if (mulEffectProof.block !== "x86 MUL32 effect" ||
      (mulEffectProof.result.decodedValue >>> 0) !== 0xfffffffe ||
      (mulEffectProof.result.decodedValueHi >>> 0) !== 1 ||
      (mulEffectProof.result.oracleValue >>> 0) !== 0xfffffffe ||
      (mulEffectProof.result.oracleValueHi >>> 0) !== 1 ||
      !mulEffectProof.result.oracleMatches ||
      mulEffectProof.result.identicalRerunLinkDelta !== 0 ||
      !mulEffectProof.execute.finalQuiescent) {
    throw new Error("x86 MUL32 effect structural proof result failed");
  }
  const mulEffectRoles = new Set(
    mulEffectProof.prepare.semanticRoots.map((root) => root.role)
  );
  for (const role of [
    "function.effect.mul",
    "function.dependency.mul32",
    "function.dependency.or2",
    "result.dependency.mul32_tag",
    "result.flag.set_tag",
    "result.flag.undefined_tag",
    "result.flag.cf",
    "result.flag.pf",
    "result.flag.af",
    "result.flag.zf",
    "result.flag.sf",
    "result.flag.of",
    "data.a.word",
    "data.b.word",
    "execution.interpreter",
    "execution.theory",
    "execution.apply",
    "invocation.call",
    "scope.initial",
    "result.tag",
  ]) {
    if (!mulEffectRoles.has(role)) {
      throw new Error("x86 MUL32 effect proof missing semantic root " + role);
    }
  }
  if (mulEffectProof.result.visualLinks.length !== mulEffectProof.result.linksFinal) {
    throw new Error("x86 MUL32 effect visual snapshot is incomplete");
  }

  const missingProofOpcodes = registry.blocks
    .map((block) => block.opcode)
    .filter((opcode) => !proofCoveredOpcodes.has(opcode));
  if (missingProofOpcodes.length !== 0) {
    throw new Error(
      "registry opcodes without structural proof: " +
      missingProofOpcodes.join(",")
    );
  }
  if (proofCoveredOpcodes.size !== registry.blocks.length) {
    throw new Error("proof coverage set does not match registry cardinality");
  }

  const reportPath =
    process.env.AMEMORY_WEB_LAB_REPORT ||
    "experiments/a-circuit/target/web-lab-acceptance-report.json";
  const proofMetrics = (proof) => ({
    block: proof.block,
    schemaVersion: proof.schemaVersion,
    compiledLinks: proof.prepare.compiledLinks,
    carrierDuplets: proof.prepare.carrierDuplets.length,
    theoryAdmissions: proof.prepare.theoryAdmissions.length,
    activeReactions: proof.execute.activeReactionCount,
    traceEntries: proof.execute.reactions.length,
    finalLinks: proof.result.linksFinal,
    visualLinks: proof.result.visualLinks.length,
    serializedProofBytes: Buffer.byteLength(JSON.stringify(proof), "utf8"),
  });
  const report = {
    schemaVersion: 1,
    kind: "amemory-web-lab-acceptance-measurements",
    informationalOnly: true,
    sourceSha,
    version,
    wasmSha256: crypto.createHash("sha256").update(bytes).digest("hex"),
    registrySha256: crypto
      .createHash("sha256")
      .update(registryText, "utf8")
      .digest("hex"),
    registryBlocks: registry.blocks.length,
    proofSchemaVersion: rawMulProof.schemaVersion,
    proofCoveredOpcodes: proofCoveredOpcodes.size,
    wasmBytes: bytes.length,
    elapsedMs: Number(process.hrtime.bigint() - acceptanceStarted) / 1e6,
    selectedProofs: {
      rawMul32: proofMetrics(rawMulProof),
      mul32Effect: proofMetrics(mulEffectProof),
    },
  };
  fs.mkdirSync(path.dirname(reportPath), { recursive: true });
  fs.writeFileSync(reportPath, JSON.stringify(report, null, 2) + "\n");
  console.log("WEB_LAB_ACCEPTANCE_REPORT=" + reportPath);
  console.log(JSON.stringify(report));

  console.log("WASM registry + one-A-memory schema-v3 proofs for all 24 opcodes PASS");
}).catch((error) => {
  console.error(error);
  process.exit(1);
});
