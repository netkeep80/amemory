import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";

export const REQUIRED_V013_LAWS = Array.from({ length: 13 }, (_, i) => `L${i + 1}`);
export const REQUIRED_V014_LAWS = Array.from({ length: 14 }, (_, i) => `V14-L${i + 1}`);
export const REQUIRED_LAWS = REQUIRED_V013_LAWS;
export const KNOWN_LOCK_PATHS = [
  "contracts/upstream/mts-v0.13.lock.json",
  "contracts/upstream/mts-v0.14.lock.json",
  "contracts/upstream/mts-v0.15.lock.json",
];

function foundationVersionTag(acceptedMtsVersion) {
  const match = /^mts-contract\/(v\d+\.\d+)$/.exec(acceptedMtsVersion ?? "");
  if (!match) throw new Error(`unsupported accepted MTS version: ${acceptedMtsVersion}`);
  return match[1];
}

function expectedAcceptanceDecision(acceptedMtsVersion) {
  return `ACCEPT_MTS_${foundationVersionTag(acceptedMtsVersion).replace(".", "_").toUpperCase()}`;
}

function requiredLawIds(lock) {
  if (lock.acceptedMtsVersion === "mts-contract/v0.13") return REQUIRED_V013_LAWS;
  if (lock.acceptedMtsVersion === "mts-contract/v0.14") return REQUIRED_V014_LAWS;
  throw new Error(`unsupported accepted MTS version: ${lock.acceptedMtsVersion}`);
}
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
  const v015 = lock?.acceptedMtsVersion === "mts-contract/v0.15";
  const expectedSchema = v015
    ? "amemory-upstream-mts-lock/v0.3"
    : "amemory-upstream-mts-lock/v0.2";
  if (lock?.schema !== expectedSchema) {
    throw new Error("unexpected upstream lock schema");
  }
  if (lock.floatingRefsAllowed !== false) {
    throw new Error("floating upstream refs must be forbidden");
  }
  exactCommit(lock.acceptedCommit, "acceptedCommit");
  if (!lock.repository || !lock.acceptedMtsVersion) {
    throw new Error("repository and acceptedMtsVersion are required");
  }

  const artifactNames = v015
    ? ["contract", "conformance", "traceability", "acceptance", "requirements"]
    : ["contract", "conformance", "traceability", "acceptance"];
  for (const name of artifactNames) {
    const artifact = lock.artifacts?.[name];
    if (!artifact?.path) {
      throw new Error(`invalid pinned artifact path: ${name}`);
    }
    exactBlob(artifact.blobSha, `${name} blobSha`);
  }

  if (v015) {
    const kernel = lock.executionKernel;
    if (!kernel || kernel.repository !== lock.repository) {
      throw new Error("executionKernel must use the pinned upstream repository");
    }
    exactCommit(kernel.commit, "executionKernel.commit");
    exactBlob(kernel.blobSha, "executionKernel.blobSha");
    if (
      kernel.commit !== lock.acceptedCommit ||
      !kernel.path ||
      !kernel.schema ||
      !kernel.id ||
      !kernel.status
    ) {
      throw new Error("executionKernel pin is incomplete or not co-pinned with accepted v0.15");
    }
    const frozen = lock.frozenCompatibilityBackend;
    if (
      frozen?.repository !== "netkeep80/amemory" ||
      frozen?.commit !== "832daa89f15fd0f3b7b40819b6d3670c7fd57e7d" ||
      frozen?.version !== "0.175.0" ||
      frozen?.role !== "COMPATIBILITY_CONFORMANCE_ONLY" ||
      frozen?.semanticAuthority !== false
    ) {
      throw new Error("frozen compatibility backend boundary mismatch");
    }
    return;
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

export function validateV015ExecutionKernel(lock, kernel) {
  const pin = lock.executionKernel;
  if (
    kernel?.schema !== pin.schema ||
    kernel?.id !== pin.id ||
    kernel?.status !== pin.status ||
    kernel?.acceptedBaseline !== "MTS v0.15" ||
    kernel?.semanticAuthority?.versionAccepted !== true
  ) {
    throw new Error("v0.15 execution kernel identity/acceptance mismatch");
  }
  if (
    kernel.command?.repeatedCommandCount !== 1 ||
    kernel.command?.localReaction !== "STRUCTURAL_UNARY_J0" ||
    kernel.command?.externalSemanticGrounderAllowed !== false ||
    kernel.command?.programSpecificDispatchAllowed !== false
  ) {
    throw new Error("v0.15 execution kernel command boundary mismatch");
  }
  for (const key of [
    "physicalLinkExistenceAlone",
    "hostNameAuthority",
    "externalCurrentPointer",
    "externalScopePointer",
    "externalSelectedTheoryPointer",
    "externalProgramCounter",
  ]) {
    if (kernel.authority?.[key] !== false) {
      throw new Error(`v0.15 execution kernel authority veto mismatch: ${key}`);
    }
  }
  if (
    kernel.frozenBackend?.repository !== lock.frozenCompatibilityBackend.repository ||
    kernel.frozenBackend?.main !== lock.frozenCompatibilityBackend.commit ||
    kernel.frozenBackend?.version !== lock.frozenCompatibilityBackend.version ||
    kernel.frozenBackend?.role !== "COMPATIBILITY_CONFORMANCE_ONLY" ||
    kernel.frozenBackend?.semanticAuthority !== false
  ) {
    throw new Error("v0.15 kernel frozen compatibility boundary mismatch");
  }
}

function sameStringSet(left, right) {
  return JSON.stringify([...left].sort()) === JSON.stringify([...right].sort());
}

function validateV015Upstream(lock, docs) {
  const {
    contract,
    conformance,
    traceability,
    acceptance,
    requirements,
    executionKernel,
  } = docs;

  if (
    contract?.schema !== lock.acceptedMtsVersion ||
    contract?.status !== "accepted" ||
    contract?.accepted !== true
  ) {
    throw new Error("pinned MTS v0.15 contract is not accepted");
  }
  if (
    conformance?.contract !== lock.acceptedMtsVersion ||
    conformance?.status !== "accepted" ||
    conformance?.accepted !== true ||
    conformance?.coverageState !== "complete"
  ) {
    throw new Error("pinned MTS v0.15 conformance mismatch");
  }
  if (
    traceability?.mtsVersion !== "v0.15" ||
    traceability?.status !== "accepted" ||
    traceability?.accepted !== true ||
    traceability?.contract !== lock.artifacts.contract.path ||
    traceability?.conformance !== lock.artifacts.conformance.path ||
    traceability?.acceptance !== lock.artifacts.acceptance.path ||
    traceability?.authorDecision?.decision !== "ACCEPT_MTS_V0_15"
  ) {
    throw new Error("v0.15 traceability acceptance binding mismatch");
  }
  if (
    acceptance?.decision !== "ACCEPT_MTS_V0_15" ||
    acceptance?.versionDecision?.acceptedVersion !== lock.acceptedMtsVersion ||
    acceptance?.current?.contract !== lock.artifacts.contract.path ||
    acceptance?.current?.conformance !== lock.artifacts.conformance.path ||
    acceptance?.acceptance?.cutoverPerformed !== true ||
    acceptance?.acceptance?.downstreamRepinAllowed !== true ||
    acceptance?.acceptance?.amemorySemanticPatchUsed !== false
  ) {
    throw new Error("v0.15 acceptance artifact does not authorize downstream repin");
  }
  if (
    requirements?.mtsVersion !== "v0.15" ||
    requirements?.status !== "accepted" ||
    requirements?.accepted !== true ||
    requirements?.contract !== lock.artifacts.contract.path ||
    requirements?.traceability !== lock.artifacts.traceability.path ||
    !Array.isArray(requirements?.requirements) ||
    requirements.requirements.length !== 48
  ) {
    throw new Error("v0.15 requirements registry mismatch");
  }

  const lawIds = Object.keys(contract.requiredSemanticLaws ?? {});
  const traceIds = Object.keys(traceability.requirements ?? {});
  const registryIds = requirements.requirements.map((item) => item.id);
  if (
    lawIds.length !== 48 ||
    !sameStringSet(lawIds, traceIds) ||
    !sameStringSet(lawIds, registryIds)
  ) {
    throw new Error("v0.15 law/traceability/requirements identity mismatch");
  }
  if (
    requirements.requirements.some(
      (item) => item.mandatory !== true || item.state === "OPEN",
    )
  ) {
    throw new Error("v0.15 mandatory requirement unresolved");
  }
  if (traceability.approvedJsonCorpus?.entries?.length !== 6) {
    throw new Error("v0.15 approved executable corpus must contain exactly six artifacts");
  }
  if (
    contract.semanticAuthority?.kernel !== lock.executionKernel.path ||
    contract.semanticAuthority?.programSpecificHostSemanticInjectionAllowed !== false ||
    contract.semanticAuthority?.jsonSpecificSemanticPathAllowed !== false ||
    contract.semanticAuthority?.specialBooleanRuntimeAllowed !== false
  ) {
    throw new Error("v0.15 contract semantic authority boundary mismatch");
  }
  if (
    contract.implementation?.frozenCompatibilityBackend !==
      `netkeep80/amemory@${lock.frozenCompatibilityBackend.commit}/${lock.frozenCompatibilityBackend.version}`
  ) {
    throw new Error("v0.15 contract frozen compatibility backend pin mismatch");
  }
  if (
    !Array.isArray(conformance.requiredPositiveVectors) ||
    !Array.isArray(conformance.requiredNegativeVectors) ||
    !Array.isArray(conformance.requiredExecutableGates)
  ) {
    throw new Error("v0.15 conformance inventory missing");
  }

  validateV015ExecutionKernel(lock, executionKernel);
}

export function validateUpstream(lock, docs) {
  if (lock?.acceptedMtsVersion === "mts-contract/v0.15") {
    return validateV015Upstream(lock, docs);
  }
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
    acceptance?.decision !== expectedAcceptanceDecision(lock.acceptedMtsVersion) ||
    acceptance?.versionDecision?.acceptedVersion !== lock.acceptedMtsVersion ||
    acceptance?.current?.contract !== lock.artifacts.contract.path ||
    acceptance?.current?.conformance !== lock.artifacts.conformance.path ||
    acceptance?.acceptance?.cutoverPerformed !== true ||
    acceptance?.acceptance?.downstreamRepinAllowed !== true
  ) {
    throw new Error("acceptance artifact does not authorize the pinned MTS version");
  }

  const laws = contract.requiredSemanticLaws;
  if (!laws || typeof laws !== "object") {
    throw new Error("requiredSemanticLaws missing");
  }
  for (const id of requiredLawIds(lock)) {
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

function buildV013Projection(lock, docs) {
  validateLock(lock);
  validateUpstream(lock, docs);

  const { contract, conformance, traceability, acceptance, executionProfile } = docs;
  const traceProjection = {};
  for (const id of REQUIRED_V013_LAWS) {
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


function buildV014Projection(lock, docs) {
  validateLock(lock);
  validateUpstream(lock, docs);

  const { contract, conformance, traceability, acceptance, executionProfile } = docs;
  const traceProjection = {};
  for (const id of REQUIRED_V014_LAWS) {
    const item = traceability.invariants[id];
    traceProjection[id] = {
      status: item.status,
      contractPointer: item.contractPointer,
      positiveVectors: item.positive?.requiredPositiveVectors ?? [],
      negativeVectors: item.negative?.requiredNegativeVectors ?? [],
      requiredExecutableGates: item.requiredExecutableGates ?? [],
    };
  }

  const acceptedFoundationVersion = foundationVersionTag(lock.acceptedMtsVersion);
  const profileFoundationVersion = executionProfile.foundation?.mtsVersion ?? null;

  return {
    schema: "amemory-mts-requirements-projection/v0.3",
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
      previousAcceptedVersion: acceptance.versionDecision.previousAcceptedVersion,
      current: acceptance.current,
      contractAccepted: contract.accepted,
      conformanceAccepted: conformance.accepted,
      conformanceCoverageState: conformance.coverageState,
      downstreamRepinAllowed: acceptance.acceptance.downstreamRepinAllowed,
      singleLiveSemanticRuntime: acceptance.acceptance.singleLiveSemanticRuntime,
    },
    authoritySplit: {
      acceptedFoundation: {
        version: acceptedFoundationVersion,
        contract: lock.acceptedMtsVersion,
        commit: lock.acceptedCommit,
      },
      executionProfile: {
        id: executionProfile.id,
        profileVersion: executionProfile.profileVersion,
        status: executionProfile.status,
        foundationMtsVersion: profileFoundationVersion,
        commit: lock.executionProfile.commit,
      },
      sameFoundationVersion: acceptedFoundationVersion === profileFoundationVersion,
      executionProfileMayBeRelabeledAsAcceptedFoundation: false,
      classification:
        acceptedFoundationVersion === profileFoundationVersion
          ? "FOUNDATION_AND_PROFILE_SAME_VERSION"
          : "INDEPENDENT_EXECUTION_PROFILE_PRE_DATES_ACCEPTED_FOUNDATION",
    },
    foundation: {
      inheritedFoundation: contract.inheritedFoundation,
      foundationOrientation: contract.foundationOrientation,
      recursiveAlphabet: contract.recursiveAlphabet,
      recursivePrefixCodec: contract.recursivePrefixCodec,
      ostensiveFormalNotation: contract.ostensiveFormalNotation,
      sequenceSemantics: contract.sequenceSemantics,
      reactionResultBasis: contract.reactionResultBasis,
      linkImmutabilityAndRewrite: contract.linkImmutabilityAndRewrite,
      generalizedMpNonRegression: contract.generalizedMpNonRegression,
      executionProfileNonRegression: contract.executionProfileNonRegression,
      representationLayers: contract.representationLayers,
    },
    laws: contract.requiredSemanticLaws,
    conformance: {
      requiredPositiveVectors: conformance.requiredPositiveVectors,
      requiredNegativeVectors: conformance.requiredNegativeVectors,
    },
    traceability: traceProjection,
    executionProfile,
    veto: acceptance.veto,
    explicitlyDeferred: {
      multiplicationScope: contract.multiplicationScope,
    },
    postAcceptanceWork: acceptance.postAcceptanceWork ?? [],
    researchPointers: [
      {
        repository: "netkeep80/anum_docs",
        issue: 1558,
        normative: false,
        topic: "research history and authority of the independently pinned A-memory execution profile",
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

function buildV015Projection(lock, docs) {
  validateLock(lock);
  validateUpstream(lock, docs);

  const {
    contract,
    conformance,
    traceability,
    acceptance,
    requirements,
    executionKernel,
  } = docs;
  const stateCounts = {};
  for (const item of requirements.requirements) {
    stateCounts[item.state] = (stateCounts[item.state] ?? 0) + 1;
  }
  const criticalIds = [
    "V15-AUTH-01",
    "V15-CLOSE-03",
    "V15-FRESH-01",
    "V15-FRESH-02",
    "V15-FRESH-03",
    "V15-AND-01",
    "V15-AND-02",
    "V15-AND-03",
    "V15-AND-04",
    "V15-CTX-01",
    "V15-CTX-02",
    "V15-GAMMA-01",
    "V15-REG-01",
    "V15-REG-02",
    "V15-READY-03",
  ];

  return {
    schema: "amemory-mts-requirements-projection/v0.4",
    generated: true,
    generatedFrom: {
      foundation: {
        repository: lock.repository,
        commit: lock.acceptedCommit,
        mtsVersion: lock.acceptedMtsVersion,
        artifacts: lock.artifacts,
      },
      executionKernel: {
        repository: lock.executionKernel.repository,
        commit: lock.executionKernel.commit,
        path: lock.executionKernel.path,
        blobSha: lock.executionKernel.blobSha,
      },
    },
    acceptance: {
      decision: acceptance.decision,
      acceptedVersion: acceptance.versionDecision.acceptedVersion,
      previousAcceptedVersion: acceptance.versionDecision.previousAcceptedVersion,
      current: acceptance.current,
      contractAccepted: contract.accepted,
      conformanceAccepted: conformance.accepted,
      conformanceCoverageState: conformance.coverageState,
      downstreamRepinAllowed: acceptance.acceptance.downstreamRepinAllowed,
      singleLiveSemanticRuntime: acceptance.acceptance.singleLiveSemanticRuntime,
      amemorySemanticPatchUsed: acceptance.acceptance.amemorySemanticPatchUsed,
    },
    authoritySplit: {
      acceptedFoundation: {
        version: "v0.15",
        contract: lock.acceptedMtsVersion,
        commit: lock.acceptedCommit,
      },
      executionKernel: {
        id: executionKernel.id,
        schema: executionKernel.schema,
        status: executionKernel.status,
        acceptedBaseline: executionKernel.acceptedBaseline,
        commit: lock.executionKernel.commit,
      },
      frozenCompatibilityBackend: lock.frozenCompatibilityBackend,
      runtimeMigration: {
        state: "A0_A1_PENDING",
        primaryIssue: 455,
        a0Issue: 492,
        productionV015ConformanceClaimed: false,
      },
    },
    semanticAuthority: contract.semanticAuthority,
    executionKernel: {
      command: executionKernel.command,
      phases: executionKernel.phases,
      authority: executionKernel.authority,
      context: executionKernel.context,
      causalLaws: executionKernel.causalLaws,
      bootstrapBoundary: executionKernel.bootstrapBoundary,
    },
    laws: contract.requiredSemanticLaws,
    conformance: {
      requiredPositiveVectors: conformance.requiredPositiveVectors,
      requiredNegativeVectors: conformance.requiredNegativeVectors,
      requiredExecutableGates: conformance.requiredExecutableGates,
    },
    requirements: {
      registryPath: lock.artifacts.requirements.path,
      total: requirements.requirements.length,
      stateCounts,
      currentAccepted: requirements.currentAccepted,
      states: Object.fromEntries(
        Object.entries(traceability.requirements).map(([id, item]) => [
          id,
          item.state,
        ]),
      ),
      criticalExecutionRequirements: Object.fromEntries(
        criticalIds.map((id) => [id, traceability.requirements[id]]),
      ),
    },
    approvedExecutableCorpus: {
      count: traceability.approvedJsonCorpus.entries.length,
      entries: traceability.approvedJsonCorpus.entries.map((entry) => ({
        id: entry.id,
        formalSourceArtifact: entry.formalSourceArtifact,
        formalSourceDigest: entry.formalSourceDigest,
        canonicalJsonArtifact: entry.canonicalJsonArtifact,
        canonicalJsonDigest: entry.canonicalJsonDigest,
        semanticEntry: entry.semanticEntry,
        expectedRecursiveRepresentation: entry.expectedRecursiveRepresentation,
        expectedRecursiveDigest: entry.expectedRecursiveDigest,
      })),
    },
    veto: acceptance.veto,
    postAcceptanceWork: acceptance.postAcceptanceWork,
  };
}

export function buildProjection(lock, docs) {
  if (lock?.acceptedMtsVersion === "mts-contract/v0.13") {
    return buildV013Projection(lock, docs);
  }
  if (lock?.acceptedMtsVersion === "mts-contract/v0.14") {
    return buildV014Projection(lock, docs);
  }
  if (lock?.acceptedMtsVersion === "mts-contract/v0.15") {
    return buildV015Projection(lock, docs);
  }
  throw new Error(`unsupported accepted MTS version: ${lock?.acceptedMtsVersion}`);
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

async function loadPinnedDocs(lock) {
  const docs = {};
  const v015 = lock.acceptedMtsVersion === "mts-contract/v0.15";
  const artifactNames = v015
    ? ["contract", "conformance", "traceability", "acceptance", "requirements"]
    : ["contract", "conformance", "traceability", "acceptance"];
  for (const name of artifactNames) {
    docs[name] = await fetchPinnedJson(
      lock.repository,
      lock.acceptedCommit,
      lock.artifacts[name],
      name,
    );
  }
  if (v015) {
    docs.executionKernel = await fetchPinnedJson(
      lock.executionKernel.repository,
      lock.executionKernel.commit,
      {
        path: lock.executionKernel.path,
        blobSha: lock.executionKernel.blobSha,
      },
      "executionKernel",
    );
  } else {
    docs.executionProfile = await fetchPinnedJson(
      lock.executionProfile.repository,
      lock.executionProfile.commit,
      {
        path: lock.executionProfile.path,
        blobSha: lock.executionProfile.blobSha,
      },
      "executionProfile",
    );
  }
  return docs;
}

async function processLock(lockPath, mode) {
  const lock = JSON.parse(await readFile(lockPath, "utf8"));
  validateLock(lock);
  const docs = await loadPinnedDocs(lock);
  const projection = buildProjection(lock, docs);
  const target = lock.generatedProjection;

  if (mode === "write") {
    await writeFile(target, stableJson(projection));
    process.stdout.write(`updated ${target}\n`);
    return;
  }

  const actual = await readFile(target, "utf8");
  assertProjectionMatches(projection, actual);
  process.stdout.write(`MTS_UPSTREAM_IMPORT_GREEN=${lock.acceptedMtsVersion}\n`);
}

async function main() {
  const mode = process.argv.includes("--write") ? "write" : "check";
  for (const lockPath of KNOWN_LOCK_PATHS) {
    await processLock(lockPath, mode);
  }
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
