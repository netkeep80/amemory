import assert from "node:assert/strict";
import {
  assertProjectionMatches,
  buildProjection,
  gitBlobSha,
  stableJson,
  validateLock,
  validateUpstream,
  verifyBlobSha,
} from "./sync-mts-requirements.mjs";

function expectThrow(fn, pattern) {
  assert.throws(fn, pattern);
}

const artifact = { path: "x.json", blobSha: "0".repeat(40) };
const baseLock = {
  schema: "amemory-upstream-mts-lock/v0.1",
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
  };
}

// Positive controls.
validateLock(baseLock);
validateUpstream(baseLock, validDocs());
const projection = buildProjection(baseLock, validDocs());
assertProjectionMatches(projection, stableJson(projection));

// Negative: floating ref instead of immutable commit.
expectThrow(
  () => validateLock({ ...baseLock, acceptedCommit: "main" }),
  /exact 40-hex Git commit/,
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
  () => assertProjectionMatches(projection, stableJson({ ...projection, generated: false })),
  /differs from deterministic upstream projection/,
);

// Negative: missing required semantic law.
{
  const docs = validDocs();
  delete docs.contract.requiredSemanticLaws.L7;
  expectThrow(() => validateUpstream(baseLock, docs), /required semantic law missing: L7/);
}

// Negative: candidate/unaccepted upstream artifact.
{
  const docs = validDocs();
  docs.contract.accepted = false;
  expectThrow(() => validateUpstream(baseLock, docs), /not the accepted requested version/);
}

// Negative: contract/conformance mismatch.
{
  const docs = validDocs();
  docs.conformance.contract = "mts-contract/v0.12";
  expectThrow(() => validateUpstream(baseLock, docs), /does not match the accepted contract/);
}

console.log("MTS_UPSTREAM_IMPORT_NEGATIVE_WITNESSES=GREEN");
