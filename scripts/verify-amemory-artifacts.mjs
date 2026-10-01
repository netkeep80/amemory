import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";

const INDEX_PATH = "contracts/amemory-artifacts.json";

function gitBlobSha(bytes) {
  const body = Buffer.from(bytes);
  const header = Buffer.from(`blob ${body.length}\0`);
  return createHash("sha1").update(header).update(body).digest("hex");
}

async function readJson(path) {
  return JSON.parse(await readFile(path, "utf8"));
}

async function verifySnapshot(snapshot) {
  assert.equal(snapshot.immutable, true, snapshot.id + " must be immutable");
  assert.match(snapshot.id, /^v\d+\.\d+$/);

  const [contractBytes, conformanceBytes] = await Promise.all([
    readFile(snapshot.contract.path),
    readFile(snapshot.conformance.path),
  ]);
  assert.equal(
    gitBlobSha(contractBytes),
    snapshot.contract.gitBlobSha,
    snapshot.id + " contract digest drift",
  );
  assert.equal(
    gitBlobSha(conformanceBytes),
    snapshot.conformance.gitBlobSha,
    snapshot.id + " conformance digest drift",
  );

  const contract = JSON.parse(contractBytes);
  const conformance = JSON.parse(conformanceBytes);
  assert.equal(contract.schema, snapshot.contract.schema);
  assert.equal(conformance.schema, snapshot.conformance.schema);
  assert.equal(conformance.contract, contract.schema);
  assert.equal(contract.status, snapshot.status);
  assert.equal(conformance.status, snapshot.status);
  assert.equal(contract.accepted, snapshot.accepted);
  assert.equal(conformance.accepted, snapshot.accepted);

  const lockPath = contract.normativeAuthority?.upstreamLock;
  const projectionPath = contract.normativeAuthority?.requirementsProjection;
  assert.equal(typeof lockPath, "string");
  assert.equal(typeof projectionPath, "string");
  const lock = await readJson(lockPath);
  await readFile(projectionPath);
  assert.equal(lock.generatedProjection, projectionPath);

  return { contract, conformance, lockPath, projectionPath };
}

const index = await readJson(INDEX_PATH);
assert.equal(index.schema, "amemory-artifact-lifecycle/v1");
assert.equal(index.authority.semanticRepository, "netkeep80/anum_docs");
assert.equal(index.authority.localContractOwner, "netkeep80/amemory");
assert.equal(index.authority.digestIsSemanticProof, false);
assert.equal(index.authority.semanticVerificationOwnerIssue, 331);

assert(Array.isArray(index.snapshots));
assert(index.snapshots.length >= 2);
const byId = new Map(index.snapshots.map((item) => [item.id, item]));
assert.equal(byId.size, index.snapshots.length, "duplicate snapshot id");

const verified = new Map();
for (const snapshot of index.snapshots) {
  verified.set(snapshot.id, await verifySnapshot(snapshot));
}

const current = byId.get(index.current.snapshotId);
assert(current, "current snapshot must be indexed");
assert.equal(current.historical, false);
assert.equal(index.current.contractPath, current.contract.path);
assert.equal(index.current.conformancePath, current.conformance.path);

for (const snapshot of index.snapshots) {
  if (snapshot.id !== current.id) {
    assert.equal(snapshot.historical, true);
  }
}

const currentVerified = verified.get(current.id);
assert.equal(
  currentVerified.contract.normativeAuthority.upstreamLock,
  index.upstreamProjectionLifecycle.currentLock,
);
assert.equal(
  currentVerified.contract.normativeAuthority.requirementsProjection,
  index.upstreamProjectionLifecycle.currentProjection,
);
assert.equal(
  index.upstreamProjectionLifecycle.ownerScript,
  "scripts/sync-mts-requirements.mjs",
);
await Promise.all([
  readFile(index.upstreamProjectionLifecycle.ownerScript),
  readFile(index.upstreamProjectionLifecycle.currentLock),
  readFile(index.upstreamProjectionLifecycle.currentProjection),
]);

const namespaces = index.versionNamespaces;
assert.equal(
  namespaces.acceptedMtsFoundation.source,
  "current.contract#/normativeAuthority/mtsVersion",
);
assert.equal(
  namespaces.executionProfile.source,
  "current.contract#/normativeAuthority/executionProfile",
);
assert.equal(namespaces.implementation.source, "VERSION");
assert.equal(namespaces.implementation.format, "semver");
const implementationVersion = (await readFile("VERSION", "utf8")).trim();
assert.match(implementationVersion, /^\d+\.\d+\.\d+$/);

for (const name of [
  "observabilityEvidence",
  "scenario",
  "proofRepresentation",
]) {
  const source = namespaces[name]?.source;
  assert.equal(typeof source, "string", name + " source missing");
  const path = source.split("#", 1)[0];
  await readFile(path);
}

const profile = currentVerified.contract.normativeAuthority.executionProfile;
assert.equal(typeof currentVerified.contract.normativeAuthority.mtsVersion, "string");
assert.equal(typeof profile?.id, "string");
assert.equal(typeof profile?.profileVersion, "string");
assert.equal(typeof profile?.commit, "string");
assert.notEqual(
  namespaces.acceptedMtsFoundation.source,
  namespaces.executionProfile.source,
  "foundation and execution-profile namespaces must stay distinct",
);

assert.equal(index.semanticVerification.ownerIssue, 331);
assert.equal(
  index.semanticVerification.artifactDigestMeaning,
  "identity-only-not-semantic-correctness",
);
for (const required of [
  "TRANSPORT_VALID",
  "TRACE_CONSISTENT",
  "SEMANTIC_REPLAY_VERIFIED",
  "SCALAR_ORACLE_VERIFIED",
]) {
  assert(index.semanticVerification.levels.includes(required));
}

// The lifecycle index is metadata only. Large contract/conformance bodies
// remain in their immutable versioned files, never duplicated as "current".
for (const forbidden of ["requirements", "mandatoryVectors", "backendMatrix"]) {
  assert.equal(
    Object.hasOwn(index, forbidden),
    false,
    "lifecycle index must not duplicate " + forbidden,
  );
}

console.log(
  "AMEMORY_ARTIFACT_LIFECYCLE=GREEN current=" +
    current.id +
    " snapshots=" +
    index.snapshots.length +
    " implementation=" +
    implementationVersion,
);
