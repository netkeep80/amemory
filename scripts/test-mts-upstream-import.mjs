import assert from "node:assert/strict";
import {
  assertProjectionMatches,
  buildProjection,
  gitBlobSha,
  stableJson,
  validateExecutionProfile,
  validateLock,
  validateUpstream,
  verifyBlobSha,
} from "./sync-mts-requirements.mjs";

function expectThrow(fn, pattern) {
  assert.throws(fn, pattern);
}

const artifact = { path: "x.json", blobSha: "0".repeat(40) };
const baseLock = {
  schema: "amemory-upstream-mts-lock/v0.2",
  normative: true,
  repository: "netkeep80/anum_docs",
  acceptedMtsVersion: "mts-contract/v0.13",
  acceptedCommit: "1".repeat(40),
  floatingRefsAllowed: false,
  artifacts: {
    contract: artifact,
    conformance: artifact,
    traceability: artifact,
    acceptance: artifact,
  },
  executionProfile: {
    repository: "netkeep80/anum_docs",
    commit: "2".repeat(40),
    path: "profiles/amemory-execution-profile.json",
    blobSha: "3".repeat(40),
    schema: "mts-amemory-execution-profile/v0.1",
    id: "minimal-portable-amemory-execution",
    profileVersion: "0.1.0",
  },
  generatedProjection: "projection.json",
};

const laws = Object.fromEntries(
  Array.from({ length: 13 }, (_, i) => [`L${i + 1}`, `law ${i + 1}`]),
);
const invariants = Object.fromEntries(
  Object.keys(laws).map((id) => [
    id,
    {
      contractPointer: `/requiredSemanticLaws/${id}`,
      positive: { requiredPositiveVectors: [`p-${id}`] },
      negative: { requiredNegativeVectors: [`n-${id}`] },
    },
  ]),
);

function validProfile() {
  return {
    schema: "mts-amemory-execution-profile/v0.1",
    id: "minimal-portable-amemory-execution",
    profileVersion: "0.1.0",
    status: "current-post-v0.13-profile",
    foundation: {
      mtsVersion: "v0.13",
      acceptedFoundationMutated: false,
    },
    classification: {
      V013_SUFFICIENT: false,
      V013_PLUS_EXECUTION_PROFILE: true,
      SEMANTIC_EXTENSION_REQUIRED: false,
    },
    authority: {
      machineReadableProjection: true,
      machineReadableProjectionIsCompetingSemanticOwner: false,
    },
    scheduling: {
      physicalScheduleIsSemanticAuthority: false,
      threadOrderIsSemanticAuthority: false,
      workGroupOrderIsSemanticAuthority: false,
      gpuLaneOrderIsSemanticAuthority: false,
      allocationOrderIsSemanticAuthority: false,
      normalizedSemanticResultMustBeScheduleIndependent: true,
    },
    substrate: {
      localHandleIsSemanticIdentity: false,
      allocationOrderIsSemanticAuthority: false,
      concreteMemoryApiIsMtsOntology: false,
      doubletsLayoutIsMtsOntology: false,
      arrayLayoutIsMtsOntology: false,
      gpuLayoutIsMtsOntology: false,
    },
    portableLaws: Array.from(
      { length: 17 },
      (_, i) => `P${String(i + 1).padStart(2, "0")}_LAW`,
    ),
    consumerRequirements: {
      declareImplementedProfileId: true,
      declareCurrentScopeMechanism: true,
      declareOldPhysicalLinkRetentionPolicy: true,
      declareBackendSubstrateBoundary: true,
      schedulingChoicesMustBeDocumentedAsNonSemantic: true,
      normalizedBackendEquivalenceTestsRequired: true,
      cpuAcceleratorDifferentialEvidenceExpected: true,
    },
  };
}

function validDocs() {
  return {
    contract: {
      schema: "mts-contract/v0.13",
      status: "accepted",
      accepted: true,
      requiredSemanticLaws: { ...laws },
      bootstrapBasis: {},
      contextAuthority: {},
      canonicalTopologyClass: {},
      representationBoundary: {},
      transportIdentity: {},
      materializationAuthority: {},
      admissibleSemanticLinks: {},
      rootOrigin: {},
      formalGrounding: {},
      physicalBoundary: {},
      explicitlyDeferred: {},
    },
    conformance: {
      contract: "mts-contract/v0.13",
      status: "accepted",
      accepted: true,
      coverageState: "complete",
      requiredPositiveVectors: ["p"],
      requiredNegativeVectors: ["n"],
    },
    traceability: {
      accepted: true,
      contract: "x.json",
      conformance: "x.json",
      invariants,
    },
    acceptance: {
      decision: "ACCEPT_MTS_V0_13",
      versionDecision: { acceptedVersion: "mts-contract/v0.13" },
      current: { contract: "x.json", conformance: "x.json" },
      acceptance: {
        cutoverPerformed: true,
        downstreamRepinAllowed: true,
        fullSelfHostedSystemClaimed: false,
      },
      veto: {},
      nonBlockingPostAcceptanceResearch: [],
    },
    executionProfile: validProfile(),
  };
}

// Positive controls.
validateLock(baseLock);
validateExecutionProfile(baseLock, validProfile());
validateUpstream(baseLock, validDocs());
const projection = buildProjection(baseLock, validDocs());
assert.equal(projection.schema, "amemory-mts-requirements-projection/v0.2");
assert.equal(
  projection.generatedFrom.foundation.commit,
  baseLock.acceptedCommit,
);
assert.equal(
  projection.generatedFrom.executionProfile.commit,
  baseLock.executionProfile.commit,
);
assert.equal(
  projection.executionProfile.id,
  "minimal-portable-amemory-execution",
);
assertProjectionMatches(projection, stableJson(projection));

// Negative: floating accepted foundation ref instead of immutable commit.
expectThrow(
  () => validateLock({ ...baseLock, acceptedCommit: "main" }),
  /acceptedCommit must be an exact 40-hex Git commit/,
);

// Negative: floating execution-profile ref instead of immutable commit.
expectThrow(
  () =>
    validateLock({
      ...baseLock,
      executionProfile: { ...baseLock.executionProfile, commit: "main" },
    }),
  /executionProfile.commit must be an exact 40-hex Git commit/,
);

// Negative: execution profile must remain independently pinned to same upstream repository.
expectThrow(
  () =>
    validateLock({
      ...baseLock,
      executionProfile: {
        ...baseLock.executionProfile,
        repository: "example/not-authority",
      },
    }),
  /executionProfile must use the pinned upstream repository/,
);

// Negative: wrong upstream blob SHA.
const bytes = Buffer.from("{}");
expectThrow(
  () => verifyBlobSha(bytes, "0".repeat(40), "negative-test"),
  /blob SHA mismatch/,
);
assert.equal(gitBlobSha(bytes).length, 40);

// Negative: manual projection modification.
expectThrow(
  () =>
    assertProjectionMatches(
      projection,
      stableJson({ ...projection, generated: false }),
    ),
  /differs from deterministic upstream projection/,
);

// Negative: missing required accepted MTS semantic law.
{
  const docs = validDocs();
  delete docs.contract.requiredSemanticLaws.L7;
  expectThrow(
    () => validateUpstream(baseLock, docs),
    /required semantic law missing: L7/,
  );
}

// Negative: candidate/unaccepted foundation artifact.
{
  const docs = validDocs();
  docs.contract.accepted = false;
  expectThrow(
    () => validateUpstream(baseLock, docs),
    /not the accepted requested version/,
  );
}

// Negative: foundation contract/conformance mismatch.
{
  const docs = validDocs();
  docs.conformance.contract = "mts-contract/v0.12";
  expectThrow(
    () => validateUpstream(baseLock, docs),
    /does not match the accepted contract/,
  );
}

// Negative: profile identity drift.
{
  const profile = validProfile();
  profile.id = "different-profile";
  expectThrow(
    () => validateExecutionProfile(baseLock, profile),
    /identity does not match exact pin/,
  );
}

// Negative: profile must not retroactively mutate accepted v0.13.
{
  const profile = validProfile();
  profile.foundation.acceptedFoundationMutated = true;
  expectThrow(
    () => validateExecutionProfile(baseLock, profile),
    /does not preserve accepted MTS v0.13/,
  );
}

// Negative: D7 classification is exact.
{
  const profile = validProfile();
  profile.classification.SEMANTIC_EXTENSION_REQUIRED = true;
  expectThrow(
    () => validateExecutionProfile(baseLock, profile),
    /classification mismatch/,
  );
}

// Negative: all P01-P17 portable laws are mandatory.
{
  const profile = validProfile();
  profile.portableLaws = profile.portableLaws.slice(0, 16);
  expectThrow(
    () => validateExecutionProfile(baseLock, profile),
    /exactly 17 portable laws/,
  );
}

// Negative: thread/GPU scheduling cannot become semantic authority.
{
  const profile = validProfile();
  profile.scheduling.gpuLaneOrderIsSemanticAuthority = true;
  expectThrow(
    () => validateExecutionProfile(baseLock, profile),
    /scheduling veto mismatch: gpuLaneOrderIsSemanticAuthority/,
  );
}

// Negative: backend layout cannot become MTS ontology.
{
  const profile = validProfile();
  profile.substrate.gpuLayoutIsMtsOntology = true;
  expectThrow(
    () => validateExecutionProfile(baseLock, profile),
    /substrate veto mismatch: gpuLayoutIsMtsOntology/,
  );
}

// Negative: consumer D7 declarations are mandatory.
{
  const profile = validProfile();
  profile.consumerRequirements.declareCurrentScopeMechanism = false;
  expectThrow(
    () => validateExecutionProfile(baseLock, profile),
    /consumer requirement missing: declareCurrentScopeMechanism/,
  );
}

console.log("MTS_AND_AMEMORY_PROFILE_IMPORT_NEGATIVE_WITNESSES=GREEN");
