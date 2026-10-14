//! A1-1a: read-only v0.15 semantic ANet transport into the CPU Link substrate.
//!
//! SOURCE_REPLAY_VERIFIED is an upstream *independently authenticated* boundary,
//! not a status that Rust derives from a caller-supplied hash. The caller must
//! supply an attestation obtained from the pinned anum_docs source replay.
//! This component neither interprets ANets nor changes Gamma.

use crate::{Handle, OptimizedLinkStore, StoreError, ROOT_HANDLE};
use serde::Deserialize;
use std::collections::BTreeSet;

pub const SEMANTIC_ANET_SCHEMA: &str = "mts-v015-semantic-anet-transport/v0.1";
pub const MTS_ACCEPTED_REVISION: &str = "813a1569300b241a3b70eb282eecd8231db15e4b";
pub const TRANSPORT_IMPLEMENTATION_REVISION: &str = "63bd6624e0e030b26ca5a79bc676892ed1408594";
pub const COMPACT_AND_SOURCE_SHA256: &str =
    "4a5bc92e661ee1d4e0c2c5d51769805b5406651bb491345200d088c9ad23e5f3";
#[cfg(test)]
const COMPACT_AND_CONTENT_SHA256: &str =
    "d332f148d42562e8585330493c78a9aa2db3f76641eb660e9804008e5276639c";
const COMPILER: &str = "v015-direct-formal-json-source@813a1569300b241a3b70eb282eecd8231db15e4b";
const DENOTATION: &str = "denoteV015ResolvedSourceAnet@813a1569300b241a3b70eb282eecd8231db15e4b";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticLoadError {
    Malformed(String),
    UnsupportedVersion,
    MissingIndependentVerification,
    UntrustedSource,
    ReplayMismatch,
    InvalidTopology,
    InvalidMembership,
    Store(StoreError),
}

impl From<StoreError> for SemanticLoadError {
    fn from(value: StoreError) -> Self {
        Self::Store(value)
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SourceReceipt {
    format: String,
    path: String,
    revision: String,
    bytes_sha256: String,
    native_source_sha256: String,
    compiler: String,
    denotation: String,
}

impl SourceReceipt {
    pub fn format(&self) -> &str {
        &self.format
    }
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn bytes_sha256(&self) -> &str {
        &self.bytes_sha256
    }
    pub fn native_source_sha256(&self) -> &str {
        &self.native_source_sha256
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct CanonicalTopology {
    schema: String,
    root: u32,
    links: Vec<[u32; 2]>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct TransportSnapshot {
    schema: String,
    topology: CanonicalTopology,
    members: Vec<u32>,
    source: SourceReceipt,
    content_sha256: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ReplayEvidence {
    status: String,
    topology: CanonicalTopology,
    members: Vec<u32>,
    source: SourceReceipt,
    content_sha256: String,
}

/// An independent trust anchor from a verified *upstream execution*, not from
/// fields taken from the untrusted transport. Authentication of the upstream
/// checkout/execution channel is the orchestration caller's responsibility.
/// In particular, never populate expected_content_sha256 by reading the
/// untrusted transport being admitted.
pub(crate) struct UpstreamReplayAnchor<'a> {
    pub authenticated_transport_revision: &'a str,
    pub accepted_mts_revision: &'a str,
    pub expected_source_sha256: &'a str,
    pub expected_content_sha256: &'a str,
    pub expected_source_path: &'a str,
    /// Entire independently authenticated SOURCE_REPLAY_VERIFIED evidence.
    /// A matching hash string alone cannot authorize altered topology/members.
    pub authenticated_replay_json: &'a str,
}

/// A capability minted only after comparing separately delivered source-backed
/// replay evidence with an independently provisioned source/content anchor.
/// It is not deserializable as a caller-provided trusted status.
pub struct SourceReplayVerified {
    snapshot: TransportSnapshot,
}

impl SourceReplayVerified {
    pub(crate) fn authenticate(
        evidence_json: &str,
        anchor: &UpstreamReplayAnchor<'_>,
    ) -> Result<Self, SemanticLoadError> {
        if anchor.authenticated_transport_revision != TRANSPORT_IMPLEMENTATION_REVISION
            || anchor.accepted_mts_revision != MTS_ACCEPTED_REVISION
            || !hex64(anchor.expected_source_sha256)
            || !hex64(anchor.expected_content_sha256)
            || anchor.expected_source_path.is_empty()
        {
            return Err(SemanticLoadError::UntrustedSource);
        }
        let evidence: ReplayEvidence = serde_json::from_str(evidence_json)
            .map_err(|e| SemanticLoadError::Malformed(e.to_string()))?;
        if evidence.status != "SOURCE_REPLAY_VERIFIED" {
            return Err(SemanticLoadError::MissingIndependentVerification);
        }
        let independently_authenticated: ReplayEvidence =
            serde_json::from_str(anchor.authenticated_replay_json)
                .map_err(|_| SemanticLoadError::UntrustedSource)?;
        if independently_authenticated.status != "SOURCE_REPLAY_VERIFIED"
            || evidence != independently_authenticated
        {
            return Err(SemanticLoadError::UntrustedSource);
        }
        let snapshot = TransportSnapshot {
            schema: SEMANTIC_ANET_SCHEMA.to_owned(),
            topology: evidence.topology,
            members: evidence.members,
            source: evidence.source,
            content_sha256: evidence.content_sha256,
        };
        validate_snapshot(&snapshot)?;
        if snapshot.source.bytes_sha256 != anchor.expected_source_sha256
            || snapshot.content_sha256 != anchor.expected_content_sha256
            || snapshot.source.path != anchor.expected_source_path
        {
            return Err(SemanticLoadError::UntrustedSource);
        }
        Ok(Self { snapshot })
    }

    pub fn source(&self) -> &SourceReceipt {
        &self.snapshot.source
    }
}

fn hex64(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

fn validate_snapshot(s: &TransportSnapshot) -> Result<(), SemanticLoadError> {
    if s.schema != SEMANTIC_ANET_SCHEMA {
        return Err(SemanticLoadError::UnsupportedVersion);
    }
    let src = &s.source;
    if (src.format != "formal" && src.format != "json")
        || src.path.is_empty()
        || src.revision != MTS_ACCEPTED_REVISION
        || src.compiler != COMPILER
        || src.denotation != DENOTATION
        || !hex64(&src.bytes_sha256)
        || !hex64(&src.native_source_sha256)
        || !hex64(&s.content_sha256)
    {
        return Err(SemanticLoadError::UntrustedSource);
    }
    let links = &s.topology.links;
    if s.topology.schema != "mts-storage-topology/v0.1"
        || s.topology.root != 0
        || links.is_empty()
        || links[0] != [0, 0]
        || links.len() > u32::MAX as usize
    {
        return Err(SemanticLoadError::InvalidTopology);
    }
    let mut frontier = 1usize;
    while frontier < links.len() {
        let mut ready = Vec::new();
        for (i, &[a, b]) in links.iter().enumerate().skip(frontier) {
            if a as usize >= links.len()
                || b as usize >= links.len()
                || (a == i as u32 && b == i as u32)
            {
                return Err(SemanticLoadError::InvalidTopology);
            }
            let item = if a == i as u32 && (b as usize) < frontier {
                Some((0u8, b, 0u32, i))
            } else if b == i as u32 && (a as usize) < frontier {
                Some((1u8, a, 0u32, i))
            } else if (a as usize) < frontier && (b as usize) < frontier {
                Some((2u8, a, b, i))
            } else {
                None
            };
            if let Some(item) = item {
                ready.push(item);
            }
        }
        if ready.is_empty() {
            return Err(SemanticLoadError::InvalidTopology);
        }
        ready.sort_unstable();
        for (offset, item) in ready.iter().enumerate() {
            if item.3 != frontier + offset
                || (offset > 0
                    && item.0 == ready[offset - 1].0
                    && item.1 == ready[offset - 1].1
                    && item.2 == ready[offset - 1].2)
            {
                return Err(SemanticLoadError::InvalidTopology);
            }
        }
        frontier += ready.len();
    }
    if s.members.windows(2).any(|w| w[0] >= w[1])
        || s.members.iter().any(|&m| m as usize >= links.len())
    {
        return Err(SemanticLoadError::InvalidMembership);
    }
    // A physical dependency closure may contain non-members; no unrelated
    // topology may be injected into an otherwise valid source-backed export.
    let mut visited = vec![false; links.len()];
    visited[0] = true;
    let mut todo = s.members.clone();
    while let Some(coord) = todo.pop() {
        if visited[coord as usize] {
            continue;
        }
        visited[coord as usize] = true;
        for pole in links[coord as usize] {
            if pole != coord {
                todo.push(pole);
            }
        }
    }
    if visited.iter().any(|x| !x) {
        return Err(SemanticLoadError::InvalidTopology);
    }
    Ok(())
}

/// Owned, immutable positive membership for this load. There is deliberately
/// no API that infers membership from substrate incidence or mutates admission.
#[derive(Clone, Debug)]
pub struct LoadedSemanticAnet {
    coordinates: Vec<Handle>,
    positive_members: BTreeSet<Handle>,
    store_instance_id: u32,
    source: SourceReceipt,
    content_sha256: String,
}

impl LoadedSemanticAnet {
    /// A physical handle is valid only in the store that received the load.
    pub fn is_member(&self, store: &OptimizedLinkStore, handle: Handle) -> bool {
        self.store_instance_id == store.instance_id() && self.positive_members.contains(&handle)
    }
    pub fn members(&self) -> &BTreeSet<Handle> {
        &self.positive_members
    }
    pub fn physical_coordinates(&self) -> &[Handle] {
        &self.coordinates
    }
    pub fn source(&self) -> &SourceReceipt {
        &self.source
    }
    pub fn content_sha256(&self) -> &str {
        &self.content_sha256
    }
}

impl OptimizedLinkStore {
    /// Imports only if the untrusted snapshot is structurally identical to
    /// independently verified source replay evidence. No fallback authority.
    /// All substrate modifications are rolled back on any late failure.
    pub fn load_verified_semantic_anet(
        &mut self,
        snapshot_json: &str,
        verified: &SourceReplayVerified,
    ) -> Result<LoadedSemanticAnet, SemanticLoadError> {
        let snapshot: TransportSnapshot = serde_json::from_str(snapshot_json)
            .map_err(|e| SemanticLoadError::Malformed(e.to_string()))?;
        validate_snapshot(&snapshot)?;
        if snapshot != verified.snapshot {
            return Err(SemanticLoadError::ReplayMismatch);
        }
        let checkpoint = self.append_checkpoint();
        let imported = (|| {
            let mut coordinates = Vec::with_capacity(snapshot.topology.links.len());
            coordinates.push(ROOT_HANDLE);
            let mut unique = BTreeSet::from([ROOT_HANDLE]);
            for (coordinate, pair) in snapshot.topology.links.iter().enumerate().skip(1) {
                let [a, b] = *pair;
                let handle = if a as usize == coordinate {
                    self.ensure_start_self_closed(coordinates[b as usize])?
                } else if b as usize == coordinate {
                    self.ensure_end_self_closed(coordinates[a as usize])?
                } else {
                    self.ensure_pair(coordinates[a as usize], coordinates[b as usize])?
                };
                if !unique.insert(handle) {
                    return Err(SemanticLoadError::InvalidTopology);
                }
                coordinates.push(handle);
            }
            let positive_members = snapshot
                .members
                .iter()
                .map(|&c| coordinates[c as usize])
                .collect();
            Ok(LoadedSemanticAnet {
                coordinates,
                positive_members,
                store_instance_id: self.instance_id(),
                source: snapshot.source,
                content_sha256: snapshot.content_sha256,
            })
        })();
        if imported.is_err() {
            self.rollback_append(checkpoint);
        }
        imported
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    // Intentionally ignored in an ordinary workspace test: no silently
    // substituted hand-built fixture. Exact pinned upstream producer and
    // independent verifier are required to run this in GitHub CI.
    #[test]
    #[ignore = "requires independently source-replayed compact AND from pinned upstream"]
    fn a1_real_upstream_p1_p5_n1_n8() {
        let snapshot = std::fs::read_to_string(
            std::env::var("MTS_V015_SNAPSHOT_INPUT").expect("missing upstream fixture"),
        )
        .unwrap();
        let replay = std::fs::read_to_string(
            std::env::var("MTS_V015_VERIFIED_EVIDENCE").expect("missing verified upstream replay"),
        )
        .unwrap();
        // Set by the *trusted upstream verifier job*, not extracted by Rust from
        // the untrusted snapshot. An attacker controlling this variable would
        // cross the external provenance boundary: it must be protected.
        let digest = std::env::var("MTS_V015_EXPECTED_DIGEST")
            .expect("missing independently authenticated expected digest");
        assert_eq!(
            digest, COMPACT_AND_CONTENT_SHA256,
            "independent compact AND digest pin"
        );
        let anchor = UpstreamReplayAnchor {
            authenticated_transport_revision: TRANSPORT_IMPLEMENTATION_REVISION,
            accepted_mts_revision: MTS_ACCEPTED_REVISION,
            expected_source_sha256: COMPACT_AND_SOURCE_SHA256,
            expected_content_sha256: &digest,
            expected_source_path: "formal/v0.15/regression/compact-and.formal",
            authenticated_replay_json: &replay,
        };
        let verified = SourceReplayVerified::authenticate(&replay, &anchor).unwrap();
        // Independent source-backed replay, not the advertised hash, grants
        // authority. Keeping both digest strings intact cannot forge evidence.
        let mut forged_replay: Value = serde_json::from_str(&replay).unwrap();
        forged_replay["members"].as_array_mut().unwrap().pop();
        assert!(matches!(
            SourceReplayVerified::authenticate(&forged_replay.to_string(), &anchor),
            Err(SemanticLoadError::UntrustedSource)
        ));
        let mut forged_structure: Value = serde_json::from_str(&replay).unwrap();
        forged_structure["topology"]["links"][1] = json!([1, 0]);
        assert!(SourceReplayVerified::authenticate(&forged_structure.to_string(), &anchor).is_err());
        let original: Value = serde_json::from_str(&snapshot).unwrap();
        assert_eq!(original["contentSha256"], digest);
        let mut first_memory = OptimizedLinkStore::new();
        let first = first_memory
            .load_verified_semantic_anet(&snapshot, &verified)
            .unwrap();
        assert_eq!(first.members().len(), 4); // P1: real approved AND
        assert!(first.physical_coordinates().len() > first.members().len()); // P2
        for &link in first.physical_coordinates() {
            if !first.members().contains(&link) {
                assert!(!first.is_member(&first_memory, link)); // P3: positive != incidence
            }
        }
        let before = first_memory.export_packed_duplets();
        let repeat = first_memory
            .load_verified_semantic_anet(&snapshot, &verified)
            .unwrap();
        assert_eq!(before, first_memory.export_packed_duplets()); // P5 idempotent
        assert_eq!(first.members(), repeat.members());
        let mut second_memory = OptimizedLinkStore::new();
        let unrelated = second_memory.ensure_pair(ROOT_HANDLE, ROOT_HANDLE).unwrap();
        second_memory.ensure_start_self_closed(unrelated).unwrap();
        let second = second_memory
            .load_verified_semantic_anet(&snapshot, &verified)
            .unwrap();
        let canonical = |store: &OptimizedLinkStore, result: &LoadedSemanticAnet| {
            result
                .members()
                .iter()
                .map(|&link| store.export_direct_recursive_wire(link).unwrap())
                .collect::<BTreeSet<_>>()
        };
        assert_eq!(
            canonical(&first_memory, &first),
            canonical(&second_memory, &second)
        );
        assert_ne!(first.physical_coordinates(), second.physical_coordinates()); // P4

        // Physical handles belong to one Store; a witness from A cannot
        // accidentally admit a coincidentally numbered physical Link in B.
        let b_member = *second.members().iter().next().unwrap();
        assert!(!first.is_member(&second_memory, b_member));

        // Every negative must leave physical carrier + all indices unchanged.
        let reject = |value: &Value| {
            let mut store = first_memory.clone();
            let before_carrier = store.export_packed_duplets();
            let before_incidence = store.export_packed_incidence_index_image();
            assert!(store
                .load_verified_semantic_anet(&value.to_string(), &verified)
                .is_err());
            assert_eq!(before_carrier, store.export_packed_duplets());
            assert_eq!(
                before_incidence,
                store.export_packed_incidence_index_image()
            );
        };
        let mut bad = original.clone();
        bad["source"]["revision"] = json!("0000000000000000000000000000000000000000");
        reject(&bad); // N1: wrong accepted pin
        let false_anchor = UpstreamReplayAnchor {
            expected_source_sha256: "0000000000000000000000000000000000000000000000000000",
            ..anchor
        };
        assert!(SourceReplayVerified::authenticate(&replay, &false_anchor).is_err()); // N2
        bad = original.clone();
        bad["topology"]["links"][1] = json!([100000, 100000]);
        reject(&bad); // N3 malformed structural identity
        bad = original.clone();
        bad["members"][0] = json!(100000);
        reject(&bad); // N4 absent coordinate
        bad = original.clone();
        let duplicate = bad["members"][0].clone();
        bad["members"].as_array_mut().unwrap().insert(1, duplicate);
        reject(&bad); // N5 duplicate
        bad = original.clone();
        let members = bad["members"].as_array().unwrap().clone();
        let nonmember = (1..bad["topology"]["links"].as_array().unwrap().len())
            .find(|&n| !members.contains(&json!(n)))
            .unwrap();
        bad["members"]
            .as_array_mut()
            .unwrap()
            .push(json!(nonmember));
        bad["members"]
            .as_array_mut()
            .unwrap()
            .sort_by_key(|v| v.as_u64().unwrap());
        reject(&bad); // N6 physical nonmember cannot be promoted
        bad = original.clone();
        bad["members"].as_array_mut().unwrap().pop();
        bad["contentSha256"] =
            json!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        reject(&bad);
        // N7: this adversarial file has a correctly recalculated transport
        // digest; only the separately authenticated source replay blocks it.
        let forged = std::fs::read_to_string(
            std::env::var("MTS_V015_TAMPERED_SNAPSHOT_INPUT")
                .expect("missing independently recomputed N7 attack"),
        )
        .unwrap();
        let forged_json: Value = serde_json::from_str(&forged).unwrap();
        assert_ne!(forged_json["members"], original["members"]);
        assert_ne!(forged_json["contentSha256"], original["contentSha256"]);
        reject(&forged_json); // N7: recalculated valid transport digest is not authority
        bad = original.clone();
        bad.as_object_mut().unwrap().remove("members");
        reject(&bad); // N8: no implicit fallback from physical Links
        bad = original.clone();
        bad["interpreter"] = json!("host-selected");
        reject(&bad); // N8: no host interpreter injection
        let mut limited = OptimizedLinkStore::with_max_links(2);
        let limited_before = limited.export_packed_duplets();
        let limited_indexes = limited.export_packed_incidence_index_image();
        assert!(limited
            .load_verified_semantic_anet(&snapshot, &verified)
            .is_err());
        assert_eq!(limited_before, limited.export_packed_duplets()); // late rollback
        assert_eq!(
            limited_indexes,
            limited.export_packed_incidence_index_image()
        );
        println!("A1_COMPONENT_RUST_LOADER_P1_P5_N1_N8=PASS");
    }

    #[test]
    fn a1_rejects_unauthenticated_semantic_claim_without_source_replay() {
        let untrusted = r#"{"schema":"mts-v015-semantic-anet-transport/v0.1","topology":{"schema":"mts-storage-topology/v0.1","root":0,"links":[[0,0]]},"members":[],"source":{},"contentSha256":"00"}"#;
        let store = OptimizedLinkStore::new();
        let original = store.export_packed_duplets();
        // There is no API importing a snapshot without a verified capability.
        // A text label SOURCE_REPLAY_VERIFIED alone is not a capability.
        assert!(serde_json::from_str::<ReplayEvidence>(untrusted).is_err());
        assert_eq!(store.export_packed_duplets(), original);
        assert_eq!(store.link_count(), 1);
        assert_eq!(
            store.poles(ROOT_HANDLE).unwrap(),
            (ROOT_HANDLE, ROOT_HANDLE)
        );
    }
}
