# A1-R1: normative v0.15 source-to-RoleBundle crosswalk

Date: 2026-10-15. Owner: [amemory#455](https://github.com/netkeep80/amemory/issues/455). Status **R1 normative boundary documented; R2 independent executable verification pending**. This document does not authorize Γ or an additional canonical JSON.

## Authority

- Accepted `anum_docs@813a1569300b241a3b70eb282eecd8231db15e4b/contracts/mts-contract-v0.15.json`: V15-AUTH-01, V15-ONTO-01/03, V15-JSON-01/02/04, V15-CTX-01/02, V15-GAMMA-01, V15-STAGE-01/02, V15-REG-01/03.
- Accepted `profiles/mts-v015-meta-interpreter-kernel.json`. Source/denotation owner `anum_docs#2002`; approved native Rule design `#1987`; Gamma/currentness `#1988/#2001`; one upstream bootstrap compiler `#2006`.
- Author-approved two-role source: `formal/v0.15/regression/two-role-meta-rule.formal` SHA-256 `f68e8629b5d476f49366f3f59e65ef64d523d5b5f8964dcf918283144ba292f9`, JSON SHA-256 `7597ac64d98b6243b3ffdc1a9bb5b6022aacb5a30ad134399c8b06457b9adf56`. Approval `anum_docs#1188`, registered in `traceability/mts-v0.15.json`.
- Transport implementation `anum_docs@63bd6624e0e030b26ca5a79bc676892ed1408594`; schema `mts-v015-semantic-anet-transport/v0.1`.

## Normative transitions

| Transition | Input → output; owner | Authority/invariants | Existing evidence and outstanding gate |
|---|---|---|---|
| FORMAL/JSON source → native ANet | Exact approved bytes → one native source ANet; `anum_docs#1901/#2002`, `v015-direct-formal-source.ts`, `v015-direct-json-source.ts` | Accepted exact digest/revision, contextual names, FORMAL=JSON source topology | Upstream approved corpus; R2 byte pin/replay |
| Native source → DAS denotation | Resolved native source ANet → `denotation.members`; `anum_docs#2002`, `v015-source-anet.ts` | Source grammar: bare members contribute positives; names bind values; nested bundle contributes members of that context; physical incidence does NOT imply membership | Accepted source/model; R2 independent exact output |
| DAS → semantic ANet | `denotation.members` → extensional `M_t`; same owner | Link identity, duplicate convergence, strictly positive members distinct from closure Links | `exportV015SemanticSnapshot`; R2 independently verify |
| Semantic ANet → transport | Source + `M_t` → rooted canonical topology, sorted member coordinates, receipt and digest; `anum_docs#2006` | All structural dependencies retained, not automatically members; locally recomputed checksum is not trust | `ts/src/v015-semantic-snapshot.ts`; R2 negative mutations |
| Transport → trust | Exact source bytes + fresh independent compiler/denotation/replay → immutable `SOURCE_REPLAY_VERIFIED`; `anum_docs#2006`, pinned by `amemory#455` | Checkout HEAD and tracked files, both source hashes, source identity and exact denotation/topology equality | Compact AND production provider; existing upstream CLI is AND-specific, downstream common orchestration must reuse original compiler without modifying schema |
| Trust → `LoadedSemanticAnet` | Opaque receipt + byte-identical candidate → store-bound handles, positive-membership set; `amemory#455`, `semantic_anet_loader.rs` | Fail closed, append rollback, bound instance/epoch, no caller-minted admission | G1 compact AND; R2 nonzero-arity |
| Loaded semantic members → RoleBundle | Read-only `M_t` plus physical poles → structural candidates; `amemory#455`, `native_v015_role_bundle.rs` | Canonical wire identities; no host-selected Theory/Context, no executable authority | G1 component only; R2 real two-role positive members |

## Exact member / role / admission distinctions

- `r∈Roles(V,M_t) ⇔ r∈M_t ∧ START(r)=V`. In the accepted two-role source nested bare `X` and `Y` denote `V->X`, `V->Y`. `RoleX` and `RoleY` are definition names for those values; names themselves are NOT an additional role.
- `Theory->Rule∈M_t` is positive admission; `Rule=V->Body`; `Body=AntecedentTemplate->ExactSequence(OutputTemplates)`. An admitted structurally valid Rule need not be applicable to any current Context.
- Physical `V->Z` is not a role without positive membership. Physical `Theory->Rule` is not admission without positive membership. Intermediate `V`, `PairXY`, `Antecedent`, `MetaBody`, sequence cells and contextual names are not automatically semantic members.
- For compact AND, `Roles(V,M_t)=∅` is the same generic topology, not a distinct grounded executor.
- Explicitly separate `SOURCE_REPLAY_VERIFIED`, `ROLE_BUNDLE_STRUCTURALLY_VALID`, `RULE_ADMITTED`, `RULE_APPLICABLE` and `GAMMA_EXECUTED`. Source parity and structural inspection alone cannot establish the latter two.

## Currentness and next refinement obligations

- The accepted kernel requires Context, Theory and currentness from ordinary Links and reaction-start positive membership; gauge boundary `C=END(R)`. The concrete A9a witness family `E=C->Theory`, `E∈M_t`, `E->K∈M_t`, `K∈M_t`, `K.current∈M_t` is **test evidence for refinement**, not a new normative primitive.
- R1: no normative obstacle identified to replay/inspect the approved Rule as read-only Link/membership evidence.
- R2: accepted exact FORMAL/JSON, two fresh Memories, trusted v0.1 replay, byte/revision/transport falsifiers, nested anchor/admission/role perturbations, AND compatibility. This step does not claim production Γ.
- R3 (STOP): independently refine/implement Context/currentness and applicable Rule authority from the semantic ANet without host selected Theory; falsify ambiguity, malformed gauge and same-generation admission.
- R4 (STOP): one J0 Γ; S1 immutable reaction-start snapshot and full plan, S2 structural substitution and Link construction, complete staged successor and atomic publication, 0/1/N images, no read-own-writes; matcher/context/freeze-resume conformance and proofs.
