"use strict";

const fs = require("fs");
const registry = JSON.parse(
  fs.readFileSync("experiments/browser-accelerator/web/i386-blocks.json", "utf8")
);
if (registry.schemaVersion !== 1) throw new Error("unexpected registry schema");
if (!Array.isArray(registry.blocks) || registry.blocks.length < 24) {
  throw new Error("expected at least 24 registered blocks");
}
const ids = new Set();
const opcodes = new Set();
for (const block of registry.blocks) {
  if (!block.id || !Number.isInteger(block.opcode)) throw new Error("bad block identity");
  if (ids.has(block.id)) throw new Error("duplicate block id " + block.id);
  if (opcodes.has(block.opcode)) throw new Error("duplicate opcode " + block.opcode);
  ids.add(block.id);
  opcodes.add(block.opcode);
  if (!Array.isArray(block.inputs) || !Array.isArray(block.modes)) {
    throw new Error("bad block schema " + block.id);
  }
}
console.log("registry blocks=" + registry.blocks.length);
