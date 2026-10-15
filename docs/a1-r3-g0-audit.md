# A1-R3 G0 — strict native Context/Theory/currentness boundary (2026-10-16)

Status: **AUDITED / IMPLEMENTATION COMPONENT-ONLY / NO AUTHOR-APPROVED CONTEXT SOURCE**. Owner: [#455](https://github.com/netkeep80/amemory/issues/455). Accepted MTS v0.15: `anum_docs@813a1569300b241a3b70eb282eecd8231db15e4b`. Base A-memory: `c4aab5b5e61c7f30d188fff80aa0f676ef43ec23`, v0.182.0. No change to accepted normative source.

## Source-of-truth decisions

- Kernel `profiles/mts-v015-meta-interpreter-kernel.json`: `GAMMA_STRUCTURAL_ASET`, `STRUCTURAL_UNARY_J0`, `C=END(R)`, `POSITIVE_SEMANTIC_ANET_MEMBERSHIP`, `LINK_STRUCTURE_AND_REACTION_START_SEMANTIC_MEMBERSHIP_SNAPSHOT`; no host semantic pointers or engine-specific Rule dispatch.
- Contract `contracts/mts-contract-v0.15.json`; #1987/#1988/#2001/#2002; A5/A6/A8/A9a/A9b/A9c. A6/A8/A9a currentness witness `E=C->Theory`, `E ∈ M_t`, `E->K ∈ M_t`, `K ∈ M_t`, `K.current ∈ M_t`; `K=START(parent->current)`, ancestry ends at `E`. This is *accepted-kernel-compatible test-based refinement*, not an additional universal foundation axiom.
- Author-approved regression FORMAL/JSON source listing at `formal/v0.15/regression/`: compact AND/NOT/OR/XOR, grounded zero-role and two-role meta-rule. The last positively denotes `V->X`, `V->Y`, `Theory->MetaRule`, but **not** `E`, `E->K`, `K`, `K.current`. No approved full launch-state FORMAL/JSON source located in this accepted regression corpus. Do **not** synthesize a production source or bless synthetic test-state with `SourceReplayVerified`.
- R2 transport: `SourceReplayVerified` is opaque and pins immutable upstream FORMAL/JSON compiler/replay, SHA and source identity; `LoadedSemanticAnet` stores immutable `BTreeSet<Handle>` positive membership and `store_instance_id / carrier_epoch`. Physical coordinates are not semantic members.
- `OptimizedLinkStore` has read-only `poles`, incidence, packed-duplet and recursive-wire exports; `ensure_*` are **component fixture only**, forbidden during production authority discovery. No authority derives from newest/maximum Handle, adjacency, lack of descendants, `RoleDictionary`, text names or host-selected Theory.
- Native Session `require_binding` checks closed state and `LoadedSemanticAnet.belongs_to(&store)`. Existing `inspect_role_bundles_structurally` classifies positive admissions, concrete Rule topology, ExactSequence and roles as canonical recursive wires but does not select active Theory. `runtime_session` legacy optimized executor is outside R3; Session `step/run/run_to_quiescence` continue returning `GammaNotImplemented`.
- R3 uses one new read-only context-and-rule inspection over the bound `M_t`; it verifies all required positive witnesses, self-start-closed Context topology and ancestry, disambiguates conflicting Theory claims per Context, and filters candidate Rule admissions by the **recovered** Theory's canonical Link identity. Multi-context is legal. No matcher/applicability or Γ.
- Store identity/epoch is enforced by the caller Session; direct internal component helper takes a test-only positive set and is **not** an admission API. Results carry only portable canonical recursive wires, no cross-session handles.

## Trust and acceptance separation

| Gate | Evidence / boundary |
|---|---|
| R3_NORMATIVE_CROSSWALK | PASS scoped to accepted kernel and historical A8/A9a test refinement |
| R3_COMPONENT_INSPECTION | Requires actual Rust tests, negative falsifiers and deterministic/canonical replay across two physical allocations |
| R3_TRUSTED_CONTEXT_E2E | **BLOCKED_AUTHOR_SOURCE_APPROVAL**; accepted compact AND and two-role sources lack a full Context launch-state |
| R3_FULL_ACCEPTANCE | **NOT_YET_READY**, regardless of component CI |
| R4_MATCH / Γ / S1 / S2 / PUBLICATION | **NOT_IMPLEMENTED / NOT AUTHORIZED** |

## Minimal Author source approval request (no file added)

Request a separately Author-reviewed FORMAL and canonical JSON pair denoting, with exact approved bytes/sha, a rooted `C=END(R)`, named `Theory`, ordinary `E=C->Theory`, `K=START(E->Truth)` (or nested `START(ParentK->Truth)`), `E->K`, positive bare memberships for **E, E->K, K, Truth**, and `Theory->Rule` plus 0/2-role Rule and roles. Include two current contexts with distinct theories, historical non-current structures, and foreign/physical-only Rule admissions in negative fixtures. Preserve independent FORMAL/JSON source parity, native-source ANet, DAS denotation, trusted v0.1 transport replay, two fresh Store allocation orders and a reviewable exact signed source identity. **Do not create or register any new canonical JSON without Author approval.**
