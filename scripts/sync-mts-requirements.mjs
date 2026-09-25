import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";

export const REQUIRED_LAWS = Array.from({ length: 13 }, (_, i) => `L${i + 1}`);
export const REQUIRED_PROFILE_LAWS = Array.from(
  { length: 17 },
  (_, i) => `P${String(i + 1).padStart(2, "0")}_`,
);

export function stableJson(value) {
  return JSON.stringify(value, null, 2) + "\n";
}

function exactCommit(value, label) {
  if (!/^[0-9a-f]{40}$/.test(value ?? "")) {
    throw new Error(`${label} must be an exact 40-hex Git commit`);
  }
}

function exactBlob(value, label) {
  if (!/^[0-9a-f]{40}$/.test(value ?? "")) {
    throw new Error(`${label} must be an exact 40-hex Git blob SHA`);
  }
}

export function validateLock(lock) {
  if (lock?.schema !== "amemory-upstream-mts-lock/v0.2") {
    throw new Error("unexpected upstream lock schema");
  }
  if (lock.floatingRefsAllowed !== false) {
    throw new Error("floating upstream refs must be forbidden");
  }
  exactCommit(lock.acceptedCommit, "acceptedCommit");
  if (!lock.repository || !lock.acceptedMtsVersion) {
    throw new Error("repository and acceptedMtsVersion are required");
  }

  for (const name of ["contract", "conformance", "traceability", "acceptance"]) {
    const artifact = lock.artifacts?.[name];
    if (!artifact?.path) {
      throw new Error(`invalid pinned artifact path: ${name}`);
    }
    exactBlob(artifact.blobSha, `${name} blobSha`);
  }

  const profile = lock.executionProfile;
  if (!profile || profile.repository !== lock.repository) {
    throw new Error("executionProfile must use the pinned upstream repository");
  }
  exactCommit(profile.commit, "executionProfile.commit");
  exactBlob(profile.blobSha, "executionProfile.blobSha");
  if (
    !profile.path ||
    !profile.schema ||
    !profile.id ||
    !profile.profileVersion
  ) {
    throw new Error("executionProfile pin is incomplete");
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
    throw new Error(
      `${label} blob SHA mismatch: expected ${expected}, got ${actual}`,
    );
  }
  return actual;
}

export function validateExecutionProfile(lock, profile) {
  const pin = lock.executionProfile;

  if (
    profile?.schema !== pin.schema ||
    profile?.id !== pin.id ||
    profile?.profileVersion !== pin.profileVersion
  ) {
    throw new Error("execution profile identity does not match exact pin");
  }
  if (profile.status !== "current-post-v0.13-profile") {
    throw new Error("execution profile is not the current post-v0.13 profile");
  }
  if (
    profile.foundation?.mtsVersion !== "v0.13" ||
    profile.foundation?.acceptedFoundationMutated !== false
  ) {
    throw new Error("execution profile does not preserve accepted MTS v0.13");
  }
  if (
    profile.classification?.V013_SUFFICIENT !== false ||
    profile.classification?.V013_PLUS_EXECUTION_PROFILE !== true ||
    profile.classification?.SEMANTIC_EXTENSION_REQUIRED !== false
  ) {
    throw new Error("execution profile classification mismatch");
  }
  if (
    profile.authority?.machineReadableProjection !== true ||
    profile.authority?.machineReadableProjectionIsCompetingSemanticOwner !== false
  ) {
    throw new Error("execution profile authority boundary mismatch");
  }

  if (!Array.isArray(profile.portableLaws) || profile.portableLaws.length !== 17) {
    throw new Error("execution profile must expose exactly 17 portable laws");
  }
  for (const prefix of REQUIRED_PROFILE_LAWS) {
    if (!profile.portableLaws.some((law) => law.startsWith(prefix))) {
      throw new Error(`execution profile portable law missing: ${prefix}`);
    }
  }

  const consumer = profile.consumerRequirements ?? {};
  for (const key of [
    "declareImplementedProfileId",
    "declareCurrentScopeMechanism",
    "declareOldPhysicalLinkRetentionPolicy",
    "declareBackendSubstrateBoundary",
    "schedulingChoicesMustBeDocumentedAsNonSemantic",
    "normalizedBackendEquivalenceTestsRequired",
    "cpuAcceleratorDifferentialEvidenceExpected",
  ]) {
    if (consumer[key] !== true) {
      throw new Error(`execution profile consumer requirement missing: ${key}`);
    }
  }

  const scheduling = profile.scheduling ?? {};
  for (const key of [
    "physicalScheduleIsSemanticAuthority",
    "threadOrderIsSemanticAuthority",
    "workGroupOrderIsSemanticAuthority",
    "gpuLaneOrderIsSemanticAuthority",
    "allocationOrderIsSemanticAuthority",
  ]) {
    if (scheduling[key] !== false) {
      throw new Error(`execution profile scheduling veto mismatch: ${key}`);
    }
  }
  if (scheduling.normalizedSemanticResultMustBeScheduleIndependent !== true) {
    throw new Error("execution profile must require normalized schedule independence");
  }

  const substrate = profile.substrate ?? {};
  for (const key of [
    "localHandleIsSemanticIdentity",
    "allocationOrderIsSemanticAuthority",
    "concreteMemoryApiIsMtsOntology",
    "doubletsLayoutIsMtsOntology",
    "arrayLayoutIsMtsOntology",
    "gpuLayoutIsMtsOntology",
  ]) {
    if (substrate[key] !== false) {
      throw new Error(`execution profile substrate veto mismatch: ${key}`);
    }
  }
}

export function validateUpstream(lock, docs) {
  const { contract, conformance, traceability, acceptance, executionProfile } = docs;

  if (
    contract?.schema !== lock.acceptedMtsVersion ||
    contract?.status !== "accepted" ||
    contract?.accepted !== true
  ) {
    throw new Error("pinned MTS contract is not the accepted requested version");
  }
  if (
    conformance?.contract !== lock.acceptedMtsVersion ||
    conformance?.status !== "accepted" ||
    conformance?.accepted !== true
  ) {
    throw new Error("pinned MTS conformance does not match the accepted contract");
  }
  if (
    traceability?.accepted !== true ||
    traceability?.contract !== lock.artifacts.contract.path ||
    traceability?.conformance !== lock.artifacts.conformance.path
  ) {
    throw new Error("traceability does not bind the pinned contract/conformance");
  }
  if (
    acceptance?.decision !== "ACCEPT_MTS_V0_13" ||
    acceptance?.versionDecision?.acceptedVersion !== lock.acceptedMtsVersion ||
    acceptance?.current?.contract !== lock.artifacts.contract.path ||
    acceptance?.current?.conformance !== lock.artifacts.conformance.path ||
    acceptance?.acceptance?.cutoverPerformed !== true
  ) {
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
  if (
    !Array.isArray(conformance.requiredPositiveVectors) ||
    !Array.isArray(conformance.requiredNegativeVectors)
  ) {
    throw new Error("portable conformance vector inventory missing");
  }

  validateExecutionProfile(lock, executionProfile);
}

export function buildProjection(lock, docs) {
  validateLock(lock);
  validateUpstream(lock, docs);

  const { contract, conformance, traceability, acceptance, executionProfile } = docs;
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
    schema: "amemory-mts-requirements-projection/v0.2",
    generated: true,
    generatedFrom: {
      foundation: {
        repository: lock.repository,
        commit: lock.acceptedCommit,
        mtsVersion: lock.acceptedMtsVersion,
        artifacts: lock.artifacts,
      },
      executionProfile: {
        repository: lock.executionProfile.repository,
        commit: lock.executionProfile.commit,
        path: lock.executionProfile.path,
        blobSha: lock.executionProfile.blobSha,
      },
    },
    acceptance: {
      decision: acceptance.decision,
      acceptedVersion: acceptance.versionDecision.acceptedVersion,
      current: acceptance.current,
      contractAccepted: contract.accepted,
      conformanceAccepted: conformance.accepted,
      conformanceCoverageState: conformance.coverageState,
      downstreamRepinAllowed: acceptance.acceptance.downstreamRepinAllowed,
      fullSelfHostedSystemClaimed:
        acceptance.acceptance.fullSelfHostedSystemClaimed,
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
    executionProfile,
    veto: acceptance.veto,
    explicitlyDeferred: contract.explicitlyDeferred,
    nonBlockingPostAcceptanceResearch:
      acceptance.nonBlockingPostAcceptanceResearch,
    researchPointers: [
      {
        repository: "netkeep80/anum_docs",
        issue: 1558,
        normative: false,
        topic: "research history for the separately pinned A-memory execution profile",
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
    throw new Error(
      "committed MTS/profile requirements projection differs from deterministic upstream projection",
    );
  }
}

async function fetchPinnedJson(repository, commit, artifact, label) {
  const url =
    `https://raw.githubusercontent.com/${repository}/${commit}/${artifact.path}`;
  const response = await fetch(url, { redirect: "error" });
  if (!response.ok) {
    throw new Error(`failed to fetch ${label}: HTTP ${response.status}`);
  }
  const bytes = Buffer.from(await response.arrayBuffer());
  verifyBlobSha(bytes, artifact.blobSha, label);
  return JSON.parse(bytes.toString("utf8"));
}

async function main() {
  const mode = process.argv.includes("--write") ? "write" : "check";
  const lockPath = "contracts/upstream/mts-v0.13.lock.json";
  const lock = JSON.parse(await readFile(lockPath, "utf8"));
  validateLock(lock);

  const docs = {};
  for (const name of ["contract", "conformance", "traceability", "acceptance"]) {
    docs[name] = await fetchPinnedJson(
      lock.repository,
      lock.acceptedCommit,
      lock.artifacts[name],
      name,
    );
  }
  docs.executionProfile = await fetchPinnedJson(
    lock.executionProfile.repository,
    lock.executionProfile.commit,
    {
      path: lock.executionProfile.path,
      blobSha: lock.executionProfile.blobSha,
    },
    "executionProfile",
  );

  const projection = buildProjection(lock, docs);
  const target = lock.generatedProjection;

  if (mode === "write") {
    await writeFile(target, stableJson(projection));
    process.stdout.write(`updated ${target}\n`);
    return;
  }

  const actual = await readFile(target, "utf8");
  assertProjectionMatches(projection, actual);
  process.stdout.write("MTS_AND_AMEMORY_PROFILE_UPSTREAM_IMPORT=GREEN\n");
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
