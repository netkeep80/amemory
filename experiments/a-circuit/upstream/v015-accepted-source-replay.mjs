// A1-R2: transport adapter over the ONE upstream v0.15 compiler/denotation/verifier.
// No canonical source or semantic rule builder. No alternate metacompiler.
// Invocation restricted by Rust's private accepted-source descriptors.
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const [checkout, id, action, artifact] = process.argv.slice(2);
const pins = Object.freeze({
  "compact-and": {
    formal: "formal/v0.15/regression/compact-and.formal",
    json: "formal/v0.15/regression/compact-and.json",
    formalSha: "4a5bc92e661ee1d4e0c2c5d51769805b5406651bb491345200d088c9ad23e5f3",
    jsonSha: "cf5028f68500bba04ee10e06ac5512eec223078d8b428f4d5de1de6433ba5941",
    members: 4,
    expectedDigest: "d332f148d42562e8585330493c78a9aa2db3f76641eb660e9804008e5276639c",
  },
  "two-role-meta-rule": {
    formal: "formal/v0.15/regression/two-role-meta-rule.formal",
    json: "formal/v0.15/regression/two-role-meta-rule.json",
    formalSha: "f68e8629b5d476f49366f3f59e65ef64d523d5b5f8964dcf918283144ba292f9",
    jsonSha: "7597ac64d98b6243b3ffdc1a9bb5b6022aacb5a30ad134399c8b06457b9adf56",
    members: 3,
    // Computed only by an independent pinned upstream source replay, then
    // hard-pinned in Rust. Until independently observed: NOT_VERIFIED.
  },
});
function require_(condition, label) {
  if (!condition) throw Error("A1-R2: " + label);
}
const pin = Object.hasOwn(pins, id) ? pins[id] : undefined;
require_(pin && ["export", "verify", "probe"].includes(action) &&
  (action === "probe" || typeof artifact === "string"), "invalid invocation");
const sha = (bytes) => createHash("sha256").update(bytes).digest("hex");
const src = (format) => {
  const path = pin[format];
  const bytes = readFileSync(resolve(checkout, path));
  require_(sha(bytes) === pin[format + "Sha"], "approved source bytes mismatch: " + path);
  return {
    format, path, revision: "813a1569300b241a3b70eb282eecd8231db15e4b", bytes,
  };
};
const formal = src("formal"), json = src("json");
const upstream = (module) => import(pathToFileURL(resolve(checkout, "ts/dist/src", module)).href);
const [{ Memory, ensureRootBasis },
  { materializeNativeSyntaxGrammar },
  { compileV015DirectFormalSourceAnet },
  { compileV015DirectJsonSourceAnet },
  { materializeV015SourceAnetProfile, denoteV015ResolvedSourceAnet },
  { exportV015SemanticSnapshot, restoreV015SemanticSnapshot, verifyV015SemanticSnapshot }
] = await Promise.all([
  upstream("memory.js"),
  upstream("native-syntax-grammar.js"),
  upstream("v015-direct-formal-source.js"),
  upstream("v015-direct-json-source.js"),
  upstream("v015-source-anet.js"),
  upstream("v015-semantic-snapshot.js"),
]);

// Identical source-independent bootstrap grammar as the pinned upstream
// ts/test/v015-semantic-snapshot-transport.test.ts; no Role/AND dispatch.
function fixture(noise = 0) {
  const memory = new Memory();
  const basis = ensureRootBasis(memory);
  let unrelated = memory.ensure(basis.C, basis.L);
  for (let i = 0; i < noise; i++)
    unrelated = memory.ensure(unrelated, i % 2 === 0 ? basis.C : basis.O);
  let cursor = memory.ensure(basis.U, basis.L);
  const fresh = () => (cursor = memory.ensureStartSelfClosed(cursor));
  const profile = Object.freeze({
    blockForm: fresh(), bareForm: fresh(),
    bindingForm: fresh(), bundleForm: fresh(),
    itemRole: fresh(), bareValueRole: fresh(),
    bindingNameRole: fresh(), bindingValueRole: fresh(),
    bundleAnchorRole: fresh(), bundleBodyRole: fresh(),
  });
  const rules = [
    { form: profile.blockForm, fields: [
      { role: profile.itemRole, target: "child", min: 0, max: null },
    ] },
    { form: profile.bareForm, fields: [
      { role: profile.bareValueRole, target: "carrier", min: 1, max: 1 },
    ] },
    { form: profile.bindingForm, fields: [
      { role: profile.bindingNameRole, target: "carrier", min: 1, max: 1 },
      { role: profile.bindingValueRole, target: "carrier", min: 1, max: 1 },
    ] },
    { form: profile.bundleForm, fields: [
      { role: profile.bundleAnchorRole, target: "carrier", min: 1, max: 1 },
      { role: profile.bundleBodyRole, target: "child", min: 1, max: 1 },
    ] },
  ];
  const grammarRoot = materializeNativeSyntaxGrammar(memory, basis, {
    syntaxTag: fresh(), markerSeed: fresh(), rules,
  });
  const sourceAnetProfileRoot = materializeV015SourceAnetProfile(memory, profile);
  return { memory, basis, grammarRoot, sourceAnetProfileRoot };
}
function compile(f, source) {
  const args = [f.memory, f.basis, f.grammarRoot, f.sourceAnetProfileRoot, source.bytes];
  const result = source.format === "formal"
    ? compileV015DirectFormalSourceAnet(...args)
    : compileV015DirectJsonSourceAnet(...args);
  const denotation = denoteV015ResolvedSourceAnet(
    f.memory, f.basis, f.grammarRoot, f.sourceAnetProfileRoot, result.sourceAset);
  return {
    sourceAnet: result.sourceAset,
    snapshot: exportV015SemanticSnapshot(f.memory, result.sourceAset, denotation, source),
  };
}
const trusted = (source) => ({ ...fixture(), source, expectedSourceSha256: pin[source.format + "Sha"] });
const a = compile(fixture(), formal), b = compile(fixture(), json);
const eq = (x, y) => JSON.stringify(x) === JSON.stringify(y);
require_(a.sourceAnet === b.sourceAnet, "FORMAL/JSON native source parity");
require_(eq(a.snapshot.topology, b.snapshot.topology) &&
  eq(a.snapshot.members, b.snapshot.members), "FORMAL/JSON denotation parity");
require_(a.snapshot.source.nativeSourceSha256 === b.snapshot.source.nativeSourceSha256,
  "FORMAL/JSON native digest mismatch");
require_(a.snapshot.members.length === pin.members, "semantic membership cardinality");
require_(eq(a.snapshot, compile(fixture(23), formal).snapshot), "allocation noise changed semantic snapshot");
const evidence = verifyV015SemanticSnapshot(a.snapshot, trusted(formal));
require_(evidence.status === "SOURCE_REPLAY_VERIFIED" &&
  eq(evidence.members, a.snapshot.members) &&
  Object.isFrozen(evidence.members) && Object.isFrozen(evidence.topology),
  "independent immutable replay receipt missing");
verifyV015SemanticSnapshot(b.snapshot, trusted(json));
require_(pin.expectedDigest === undefined || a.snapshot.contentSha256 === pin.expectedDigest,
  "compact AND semantic snapshot regression");
console.log("SOURCE_ID=" + id + " SOURCE_REPLAY_DIGEST=" + a.snapshot.contentSha256);

const mutate = (source, edit) => {
  const modified = structuredClone(source);
  edit(modified);
  modified.contentSha256 = sha(JSON.stringify({
    schema: modified.schema, topology: modified.topology,
    members: modified.members, source: modified.source,
  }));
  return modified;
};
function mustReject(input, reason) {
  let rejected = false;
  try { verifyV015SemanticSnapshot(input, trusted(formal)); }
  catch { rejected = true; }
  require_(rejected, "independent source replay accepted " + reason);
}
const removed = mutate(a.snapshot, s => s.members.pop());
mustReject(removed, "positive membership deletion");
const extra = a.snapshot.topology.links.findIndex((_, i) =>
  i > 0 && !a.snapshot.members.includes(i));
require_(extra !== -1, "missing physical non-member for negative");
mustReject(mutate(a.snapshot, s => {
  s.members.push(extra); s.members.sort((x,y) => x-y);
}), "physical-only Link admitted");
mustReject(mutate(a.snapshot, s => {
  s.source.bytesSha256 = "0".repeat(64);
}), "source mutation");
mustReject(mutate(a.snapshot, s => {
  s.source.revision = "0".repeat(40);
}), "revision mutation");
mustReject(mutate(a.snapshot, s => {
  s.topology.links[1][0] = 999999;
}), "topology mutation");
const stable = JSON.stringify(evidence);
try { evidence.members.push(9999); } catch {}
require_(JSON.stringify(evidence) === stable, "replay evidence was mutable");
console.log("A1_R2_PINNED_SOURCE_INDEPENDENT_REPLAY_AND_FALSIFIERS=PASS");

if (action === "export") {
  writeFileSync(artifact, JSON.stringify(a.snapshot) + "\n", "utf8");
} else if (action === "verify") {
  const fromFile = JSON.parse(readFileSync(artifact, "utf8"));
  require_(eq(verifyV015SemanticSnapshot(fromFile, trusted(formal)), evidence),
    "independent external transport replay mismatch");
  console.log("EXTERNAL_TRANSPORT_FILE_SOURCE_REPLAY=PASS");
} else if (action === "probe") {
  // Probe is diagnostic, never a trusted admission receipt or production gate.
  console.log("A1_R2_PROBE_ONLY=TRUE");
}
