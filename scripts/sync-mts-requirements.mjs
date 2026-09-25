import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";

export const REQUIRED_LAWS = Array.from({ length: 13 }, (_, i) => `L${i + 1}`);

export function stableJson(value) {
  return JSON.stringify(value, null, 2) + "\n";
}

export function validateLock(lock) {
  if (lock?.schema !== "amemory-upstream-mts-lock/v0.1") {
    throw new Error("unexpected upstream lock schema");
  }
  if (lock.floatingRefsAllowed !== false) {
    throw new Error("floating upstream refs must be forbidden");
  }
  if (!/^[0-9a-f]{40}$/.test(lock.acceptedCommit ?? "")) {
    throw new Error("acceptedCommit must be an exact 40-hex Git commit");
  }
  if (!lock.repository || !lock.acceptedMtsVersion) {
    throw new Error("repository and acceptedMtsVersion are required");
  }
  for (const name of ["contract", "conformance", "traceability", "acceptance"]) {
    const artifact = lock.artifacts?.[name];
    if (!artifact?.path || !/^[0-9a-f]{40}$/.test(artifact?.blobSha ?? "")) {
      throw new Error(`invalid pinned artifact: ${name}`);
    }
  }
}

export function gitBlobSha(bytes) {
  const body = Buffer.from(bytes);
  const header = Buffer.from(`blob ${body.length}\0`);
  return createHash("sha1").update(header).update(body).digest("hex");
}

export function verifyBlobSha(bytes, expected, label = "artifact") {
  const actual = gitBlobSha(bytes);
  if (actual !== expected) {
    throw new Error(`${label} blob SHA mismatch: expected ${expected}, got ${actual}`);
  }
  return actual;
}

export function validateUpstream(lock, docs) {
  const { contract, conformance, traceability, acceptance } = docs;

  if (contract?.schema !== lock.acceptedMtsVersion || contract?.status !== "accepted" || contract?.accepted !== true) {
    throw new Error("pinned MTS contract is not the accepted requested version");
  }
  if (conformance?.contract !== lock.acceptedMtsVersion || conformance?.status !== "accepted" || conformance?.accepted !== true) {
    throw new Error("pinned MTS conformance does not match the accepted contract");
  }
  if (traceability?.accepted !== true ||
      traceability?.contract !== lock.artifacts.contract.path ||
      traceability?.conformance !== lock.artifacts.conformance.path) {
    throw new Error("traceability does not bind the pinned contract/conformance");
  }
  if (acceptance?.decision !== "ACCEPT_MTS_V0_13" ||
      acceptance?.versionDecision?.acceptedVersion !== lock.acceptedMtsVersion ||
      acceptance?.current?.contract !== lock.artifacts.contract.path ||
      acceptance?.current?.conformance !== lock.artifacts.conformance.path ||
      acceptance?.acceptance?.cutoverPerformed !== true) {
    throw new Error("acceptance artifact does not authorize the pinned MTS version");
  }

  const laws = contract.requiredSemanticLaws;
  if (!laws || typeof laws !== "object") {
    throw new Error("requiredSemanticLaws missing");
  }
  for (const id of REQUIRED_LAWS) {
    if (typeof laws[id] !== "string" || laws[id].length === 0) {
      throw new Error(`required semantic law missing: ${id}`);
    }
    if (!traceability?.invariants?.[id]) {
      throw new Error(`traceability missing for semantic law: ${id}`);
    }
  }
  if (!Array.isArray(conformance.requiredPositiveVectors) ||
      !Array.isArray(conformance.requiredNegativeVectors)) {
    throw new Error("portable conformance vector inventory missing");
  }
}

export function buildProjection(lock, docs) {
  validateLock(lock);
  validateUpstream(lock, docs);

  const { contract, conformance, traceability, acceptance } = docs;
  const traceProjection = {};
  for (const id of REQUIRED_LAWS) {
    const item = traceability.invariants[id];
    traceProjection[id] = {
      contractPointer: item.contractPointer,
      positiveVectors: item.positive?.requiredPositiveVectors ?? [],
      negativeVectors: item.negative?.requiredNegativeVectors ?? [],
    };
  }

  return {
    schema: "amemory-mts-requirements-projection/v0.1",
    generated: true,
    generatedFrom: {
      repository: lock.repository,
      commit: lock.acceptedCommit,
      mtsVersion: lock.acceptedMtsVersion,
      artifacts: lock.artifacts,
    },
    acceptance: {
      decision: acceptance.decision,
      acceptedVersion: acceptance.versionDecision.acceptedVersion,
      current: acceptance.current,
      contractAccepted: contract.accepted,
      conformanceAccepted: conformance.accepted,
      conformanceCoverageState: conformance.coverageState,
      downstreamRepinAllowed: acceptance.acceptance.downstreamRepinAllowed,
      fullSelfHostedSystemClaimed: acceptance.acceptance.fullSelfHostedSystemClaimed,
    },
    foundation: {
      bootstrapBasis: contract.bootstrapBasis,
      contextAuthority: contract.contextAuthority,
      canonicalTopologyClass: contract.canonicalTopologyClass,
      representationBoundary: contract.representationBoundary,
      transportIdentity: contract.transportIdentity,
      materializationAuthority: contract.materializationAuthority,
      admissibleSemanticLinks: contract.admissibleSemanticLinks,
      rootOrigin: contract.rootOrigin,
      formalGrounding: contract.formalGrounding,
      physicalBoundary: contract.physicalBoundary,
    },
    laws: contract.requiredSemanticLaws,
    conformance: {
      requiredPositiveVectors: conformance.requiredPositiveVectors,
      requiredNegativeVectors: conformance.requiredNegativeVectors,
    },
    traceability: traceProjection,
    veto: acceptance.veto,
    explicitlyDeferred: contract.explicitlyDeferred,
    nonBlockingPostAcceptanceResearch: acceptance.nonBlockingPostAcceptanceResearch,
    researchPointers: [
      {
        repository: "netkeep80/anum_docs",
        issue: 1558,
        normative: false,
        topic: "full executable A-network dynamics for A-memory",
      },
      {
        repository: "netkeep80/anum_docs",
        issue: 1332,
        normative: false,
        topic: "MTS Aset <-> Doublets Aset representation boundary",
      },
    ],
  };
}

export function assertProjectionMatches(expected, actualText) {
  const expectedText = stableJson(expected);
  if (actualText !== expectedText) {
    throw new Error("committed MTS requirements projection differs from deterministic upstream projection");
  }
}

async function fetchArtifact(lock, name) {
  const artifact = lock.artifacts[name];
  const url = `https://raw.githubusercontent.com/${lock.repository}/${lock.acceptedCommit}/${artifact.path}`;
  const response = await fetch(url, { redirect: "error" });
  if (!response.ok) {
    throw new Error(`failed to fetch ${name}: HTTP ${response.status}`);
  }
  const bytes = Buffer.from(await response.arrayBuffer());
  verifyBlobSha(bytes, artifact.blobSha, name);
  return JSON.parse(bytes.toString("utf8"));
}

async function main() {
  const mode = process.argv.includes("--write") ? "write" : "check";
  const lockPath = "contracts/upstream/mts-v0.13.lock.json";
  const lock = JSON.parse(await readFile(lockPath, "utf8"));
  validateLock(lock);

  const docs = {};
  for (const name of ["contract", "conformance", "traceability", "acceptance"]) {
    docs[name] = await fetchArtifact(lock, name);
  }

  const projection = buildProjection(lock, docs);
  const target = lock.generatedProjection;

  if (mode === "write") {
    await writeFile(target, stableJson(projection));
    process.stdout.write(`updated ${target}\n`);
    return;
  }

  const actual = await readFile(target, "utf8");
  assertProjectionMatches(projection, actual);
  process.stdout.write("MTS_UPSTREAM_IMPORT=GREEN\n");
}

const invokedAsScript =
  process.argv[1] &&
  import.meta.url === pathToFileURL(process.argv[1]).href;

if (invokedAsScript) {
  main().catch((error) => {
    console.error(error?.stack ?? error);
    process.exitCode = 1;
  });
}
