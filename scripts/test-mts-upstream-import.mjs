import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
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

// D7 repository-level convergence guards.
{
  const contract = JSON.parse(
    readFileSync("contracts/amemory-contract-v0.1.json", "utf8"),
  );
  const conformance = JSON.parse(
    readFileSync("contracts/amemory-conformance-v0.1.json", "utf8"),
  );
  const projection = JSON.parse(
    readFileSync("contracts/upstream/mts-v0.13-requirements.json", "utf8"),
  );
  const readme = readFileSync("README.md", "utf8");

  assert.equal(
    contract.normativeAuthority.acceptedFoundationCommit,
    "440caf09558d4ff5cfda11805cb3ef97b48d1ad5",
  );
  assert.equal(
    contract.normativeAuthority.executionProfile.id,
    "minimal-portable-amemory-execution",
  );
  assert.equal(
    contract.normativeAuthority.executionProfile.profileVersion,
    "0.1.0",
  );
  assert.equal(
    contract.normativeAuthority.executionProfile.commit,
    "af4e3dadbb9857fba7239a58f79ed2da59bc5c42",
  );
  assert.equal(
    contract.executionProfileBoundary.fullReactionProfileConformanceClaimed,
    false,
  );

  const requiredD7Fields = [
    "implementedProfileId",
    "implementedProfileVersion",
    "profilePinCommit",
    "currentScopeMechanism",
    "oldPhysicalLinkRetentionPolicy",
    "backendSubstrateBoundary",
    "nonSemanticSchedulingChoices",
    "normalizedSemanticEquivalence",
    "differentialEvidence",
  ];
  for (const field of requiredD7Fields) {
    assert(
      contract.backendDeclaration.requiredFields.includes(field),
      `D7 backend declaration field missing: ${field}`,
    );
  }

  assert.equal(
    projection.executionProfile.id,
    "minimal-portable-amemory-execution",
  );
  assert.equal(projection.executionProfile.profileVersion, "0.1.0");
  assert.equal(
    projection.generatedFrom.foundation.commit,
    "440caf09558d4ff5cfda11805cb3ef97b48d1ad5",
  );
  assert.equal(
    projection.generatedFrom.executionProfile.commit,
    "af4e3dadbb9857fba7239a58f79ed2da59bc5c42",
  );

  assert.equal(
    conformance.normativeExecutionProfile.id,
    "minimal-portable-amemory-execution",
  );
  assert.equal(conformance.normativeExecutionProfile.profileVersion, "0.1.0");
  assert.equal(conformance.normativeExecutionProfile.portableLawCount, 17);

  const backends = new Map(
    conformance.backendMatrix.map((backend) => [backend.backendId, backend]),
  );
  for (const backendId of [
    "rust-reference-cpu-wasm",
    "browser-webgpu",
    "future-accelerator-family",
  ]) {
    assert(backends.has(backendId), `D7 backend missing: ${backendId}`);
  }

  for (const backendId of [
    "rust-reference-cpu-wasm",
    "browser-webgpu",
  ]) {
    const backend = backends.get(backendId);
    assert.equal(
      backend.implementedProfileId,
      "minimal-portable-amemory-execution",
    );
    assert.equal(backend.implementedProfileVersion, "0.1.0");
    assert.equal(
      backend.profilePinCommit,
      "af4e3dadbb9857fba7239a58f79ed2da59bc5c42",
    );
    assert.equal(backend.fullProfileConformance, true);
    assert.match(backend.currentScopeMechanism, /^R1 IMPLEMENTED:/);
    assert(backend.backendSubstrateBoundary.length > 0);
    assert(backend.nonSemanticSchedulingChoices.length > 0);
    assert(backend.normalizedSemanticEquivalence.length > 0);
    assert(backend.differentialEvidence.length > 0);

    for (const id of [
      "P01","P02","P03","P04","P05","P06","P08","P11","P12","P15",
    ]) {
      assert.equal(
        backend.profileLawCoverage[id],
        "r1-green-real-browser-differential",
        `${backendId} R1 GREEN coverage mismatch for ${id}`,
      );
    }

    for (const id of ["P07","P13"]) {
      assert.equal(
        backend.profileLawCoverage[id],
        "r2-green-real-browser-differential",
        `${backendId} R2 GREEN coverage mismatch for ${id}`,
      );
    }
    for (const id of ["P09","P10"]) {
      assert.equal(
        backend.profileLawCoverage[id],
        "r3-green-real-browser-differential",
        `${backendId} R3 GREEN coverage mismatch for ${id}`,
      );
    }
    assert.equal(
      backend.profileLawCoverage.P14,
      "r4-green-real-browser-differential",
      `${backendId} R4 GREEN coverage mismatch for P14`,
    );
    for (const id of ["P16","P17"]) {
      assert.equal(
        backend.profileLawCoverage[id],
        "r5-green-real-browser-differential",
        `${backendId} R5 GREEN coverage mismatch for ${id}`,
      );
    }
  }

  const future = backends.get("future-accelerator-family");
  assert.equal(future.implementationStatus, "planned");
  assert.equal(future.fullProfileConformance, false);

  const c045 = conformance.mandatoryVectors.find(
    (vector) =>
      vector.id === "AM-C045-cpu-webgpu-full-reaction-profile-differential",
  );
  assert.equal(c045?.status, "green");
  assert.deepEqual(c045?.evidence?.backends, ["rust-reference-cpu-wasm", "browser-webgpu"]);
  assert.equal(c045?.evidence?.realBrowserDifferential, true);
  assert.equal(c045?.evidence?.normalizedReactionDifferential, "PASS");
  assert.equal(c045?.evidence?.negativeMismatchControls, "PASS across R1-R5");
  assert.equal(c045?.evidence?.allPortableLawsCovered, true);
  assert.equal(c045?.evidence?.fullReactionProfileConformance, true);
  assert.deepEqual(
    Object.keys(c045?.evidence?.portableLawCoverage ?? {}).sort(),
    Array.from({ length: 17 }, (_, i) => `P${String(i + 1).padStart(2, "0")}`),
  );
  assert.deepEqual(c045?.evidence?.completedSlices, [
    "AM-C046-r1-one-reaction-cpu-webgpu",
    "AM-C047-r2-no-admitted-relation-quiescence",
    "AM-C048-r3-zero-and-duplicate-convergence",
    "AM-C049-r4-theory-admission-tplus1",
    "AM-C050-r5-recurrence-and-end-not-halt",
  ]);

  const c046 = conformance.mandatoryVectors.find(
    (vector) => vector.id === "AM-C046-r1-one-reaction-cpu-webgpu",
  );
  assert.equal(c046?.status, "green");
  assert.equal(c046?.evidence?.mergedMainSha, "50ef0a36dc7b190730f2c2bea38ed8dd70f0117c");
  assert.equal(c046?.evidence?.observed?.normalizedDifferential, "PASS");
  assert.equal(c046?.evidence?.observed?.theorySnapshotIsolation, "PASS");
  assert.equal(c046?.evidence?.observed?.oldScopeRetained, "PASS");
  assert.equal(c046?.evidence?.fullReactionProfileConformance, false);

  const c047 = conformance.mandatoryVectors.find(
    (vector) => vector.id === "AM-C047-r2-no-admitted-relation-quiescence",
  );
  assert.equal(c047?.status, "green");
  assert.equal(c047?.evidence?.mergedMainSha, "e02b3be9e28dfaba445d121e920aea2636bd1996");
  assert.equal(c047?.evidence?.observed?.normalizedDifferential, "PASS");
  assert.deepEqual(c047?.evidence?.observed?.quiescent, { cpu: true, gpu: true });
  assert.deepEqual(c047?.evidence?.observed?.invalidFailureQuiescent, { cpu: false, gpu: false });
  assert.deepEqual(c047?.evidence?.coveredPortableLaws, ["P07", "P13"]);
  assert.equal(c047?.evidence?.fullReactionProfileConformance, false);

  const c048 = conformance.mandatoryVectors.find(
    (vector) => vector.id === "AM-C048-r3-zero-and-duplicate-convergence",
  );
  assert.equal(c048?.status, "green");
  assert.equal(c048?.evidence?.mergedMainSha, "9bba6f1478c5fa49963f173c6a634c52d8e2db5b");
  assert.equal(c048?.evidence?.observed?.normalizedDifferential, "PASS");
  assert.deepEqual(c048?.evidence?.observed?.zero?.scopeCpu, []);
  assert.deepEqual(c048?.evidence?.observed?.zero?.scopeGpu, []);
  assert.deepEqual(c048?.evidence?.observed?.zero?.quiescent, { cpu: false, gpu: false });
  assert.deepEqual(c048?.evidence?.observed?.mixed?.scopeCpu, ["19816898"]);
  assert.deepEqual(c048?.evidence?.observed?.mixed?.scopeGpu, ["19816898"]);
  assert.equal(c048?.evidence?.observed?.mixed?.duplicateConvergence, "PASS single canonical 19816898");
  assert.deepEqual(c048?.evidence?.coveredPortableLaws, ["P09", "P10"]);
  assert.equal(c048?.evidence?.fullReactionProfileConformance, false);

  const c049 = conformance.mandatoryVectors.find(
    (vector) => vector.id === "AM-C049-r4-theory-admission-tplus1",
  );
  assert.equal(c049?.status, "green");
  assert.equal(c049?.evidence?.mergedMainSha, "302734eb5c26d18b3f1a1abea7007250eec78407");
  assert.equal(c049?.evidence?.observed?.sameReactionIsolation, "PASS");
  assert.equal(c049?.evidence?.observed?.nextReactionVisibility, "PASS");
  assert.equal(c049?.evidence?.observed?.staleSnapshotControl, "PASS");
  assert.equal(c049?.evidence?.observed?.normalizedTrajectoryDifferential, "PASS");
  assert.deepEqual(c049?.evidence?.coveredPortableLaws, ["P14"]);
  assert.deepEqual(c049?.evidence?.doesNotClosePortableLaws, ["P16", "P17"]);
  assert.equal(c049?.evidence?.fullReactionProfileConformance, false);

  const c050 = conformance.mandatoryVectors.find(
    (vector) => vector.id === "AM-C050-r5-recurrence-and-end-not-halt",
  );
  assert.equal(c050?.status, "green");
  assert.equal(c050?.evidence?.mergedMainSha, "df1c119709121e27fcb672fb5aeccc17f5b4391c");
  assert.equal(c050?.evidence?.observed?.recurrence, "PASS S0=S2=S4 and S1=S3");
  assert.equal(c050?.evidence?.observed?.endStructure, "PASS C=68 is END");
  assert.equal(c050?.evidence?.observed?.endContinuation, "PASS K->C -> K->A remains active");
  assert.equal(c050?.evidence?.observed?.boundedReturn, "PASS");
  assert.equal(c050?.evidence?.observed?.normalizedTrajectoryDifferential, "PASS");
  assert.deepEqual(c050?.evidence?.coveredPortableLaws, ["P16", "P17"]);
  assert.deepEqual(c050?.evidence?.doesNotClosePortableLaws, []);
  assert.equal(c050?.evidence?.fullReactionProfileConformance, false);

  assert.match(readme, /minimal-portable-amemory-execution/);
  assert.match(readme, /FULL_REACTION_PROFILE_CONFORMANCE = TRUE/);
  assert.match(readme, /R1|reaction/);
}

console.log("D7_BACKEND_PROFILE_DECLARATIONS=GREEN");

