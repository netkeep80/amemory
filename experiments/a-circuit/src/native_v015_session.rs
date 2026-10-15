//! A1-2 G2: ownership-bound, non-executable native v0.15 Session.
//! Reuses existing SessionIdentityV1 and SessionStateV1, not the legacy
//! OptimizedStructuralEngine. No semantic host inputs or Γ execution.
//! Scope is accepted compact AND with G1's controlled Unix checkout.
//! Compile-fail external admission and semantic-control boundary:
//! ```compile_fail
//! let _ = amemory_optimized_cpu_probe::semantic_anet_loader::SourceReplayVerified::authenticate;
//! ```
//! ```compile_fail
//! use amemory_optimized_cpu_probe::semantic_anet_loader::UpstreamReplayAnchor;
//! ```
//! ```compile_fail
//! let _ = amemory_a_circuit::native_v015_session::NativeV015Session::set_interpreter;
//! ```
//! ```compile_fail
//! let _ = amemory_a_circuit::native_v015_session::NativeV015Session::set_theory;
//! ```
//! ```compile_fail
//! let _ = amemory_a_circuit::native_v015_session::NativeV015Session::set_context;
//! ```
//! ```compile_fail
//! let _ = amemory_a_circuit::native_v015_session::NativeV015Session::set_current;
//! ```
//! ```compile_fail
//! let _ = amemory_a_circuit::native_v015_session::NativeV015Session::set_semantic_current;
//! ```
//! ```compile_fail
//! let _ = amemory_a_circuit::native_v015_session::NativeV015Session::set_semantic_scope;
//! ```
//! ```compile_fail
//! let _ = amemory_a_circuit::native_v015_session::NativeV015Session::dispatch_rule_kind;
//! ```
//! ```compile_fail
//! let _ = amemory_a_circuit::native_v015_session::NativeV015Session::set_external_grounder;
//! ```
use super::native_v015_role_bundle::inspect as inspect_role_bundles_structurally;
pub use super::native_v015_role_bundle::{RoleBundleCandidate, RoleBundleInspection};
use super::runtime_session::{next_cpu_session_id, CpuMemoryInstance};
use super::session_contract::{SessionIdentityV1, SessionStateV1};
use amemory_optimized_cpu_probe::{
    semantic_anet_loader::{LoadedSemanticAnet, SemanticLoadError, SourceReplayVerified},
    Handle,
};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeV015Error {
    TrustOrLoad(SemanticLoadError),
    InvalidBinding,
    InvalidRoleBundleTopology,
    Closed,
    GammaNotImplemented,
}
impl From<SemanticLoadError> for NativeV015Error {
    fn from(error: SemanticLoadError) -> Self {
        Self::TrustOrLoad(error)
    }
}

/// Read-only: physical handles are diagnostics, NOT cross-store identities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeV015Observation {
    pub session_id: String,
    pub memory_id: String,
    pub store_instance_id: u32,
    pub carrier_epoch: u64,
    pub state: &'static str,
    pub gamma_implemented: bool,
    pub trusted_source_path: String,
    pub trusted_source_sha256: String,
    pub trusted_source_revision: String,
    pub semantic_content_sha256: String,
    pub positive_members: Vec<Handle>,
    pub canonical_members: Vec<String>,
    pub physical_carrier: Vec<(Handle, Handle)>,
}

/// Memory and the immutable admitted ANet are held as one owned value.
/// No Clone, no public mutation/rebind, no legacy fallback.
#[derive(Debug)]
pub struct NativeV015Session {
    identity: SessionIdentityV1,
    memory: CpuMemoryInstance,
    admitted: LoadedSemanticAnet,
    state: SessionStateV1,
}
impl NativeV015Session {
    /// Deployment supplies checkout from its own trusted configuration.
    /// Never forward a checkout path controlled by the untrusted request.
    pub fn open_trusted_compact_and(
        candidate: &str,
        deployment_owned_checkout: &Path,
    ) -> Result<Self, NativeV015Error> {
        let verified =
            SourceReplayVerified::from_pinned_compact_and_checkout(deployment_owned_checkout)?;
        Self::from_verified(CpuMemoryInstance::new(), candidate, &verified)
    }

    // Private admission path, used by real-source replay tests with noisy Stores.
    fn from_verified(
        mut memory: CpuMemoryInstance,
        candidate: &str,
        verified: &SourceReplayVerified,
    ) -> Result<Self, NativeV015Error> {
        let loaded = memory
            .store
            .load_verified_semantic_anet(candidate, verified)?;
        Self::bind_loaded(memory, loaded)
    }

    fn bind_loaded(
        memory: CpuMemoryInstance,
        admitted: LoadedSemanticAnet,
    ) -> Result<Self, NativeV015Error> {
        if !admitted.belongs_to(&memory.store) {
            return Err(NativeV015Error::InvalidBinding);
        }
        Ok(Self {
            identity: SessionIdentityV1 {
                session_id: next_cpu_session_id(),
                memory_instance_id: memory.id.clone(),
            },
            memory,
            admitted,
            state: SessionStateV1::Open,
        })
    }

    fn require_binding(&self) -> Result<(), NativeV015Error> {
        if self.state == SessionStateV1::Closed {
            return Err(NativeV015Error::Closed);
        }
        if !self.admitted.belongs_to(&self.memory.store) {
            return Err(NativeV015Error::InvalidBinding);
        }
        Ok(())
    }

    pub fn observe(&self) -> Result<NativeV015Observation, NativeV015Error> {
        self.require_binding()?;
        let mut positive_members = Vec::new();
        let mut canonical_members = Vec::new();
        for &member in self.admitted.members() {
            if !self.admitted.is_member(&self.memory.store, member) {
                return Err(NativeV015Error::InvalidBinding);
            }
            canonical_members.push(
                self.memory
                    .store
                    .export_direct_recursive_wire(member)
                    .map_err(|_| NativeV015Error::InvalidBinding)?,
            );
            positive_members.push(member);
        }
        canonical_members.sort_unstable();
        let source = self.admitted.source();
        Ok(NativeV015Observation {
            session_id: self.identity.session_id.clone(),
            memory_id: self.identity.memory_instance_id.clone(),
            store_instance_id: self.admitted.store_instance_id(),
            carrier_epoch: self.admitted.carrier_epoch(),
            state: "OPEN",
            gamma_implemented: false,
            trusted_source_path: source.path().to_owned(),
            trusted_source_sha256: source.bytes_sha256().to_owned(),
            trusted_source_revision: source.revision().to_owned(),
            semantic_content_sha256: self.admitted.content_sha256().to_owned(),
            positive_members,
            canonical_members,
            physical_carrier: self.memory.store.export_packed_duplets(),
        })
    }

    /// Source-backed, read-only STRUCTURAL candidates only. No active Theory
    /// or executable Rule authority is selected; A9b is research-only.
    /// Returns canonical structural wires, never transferable physical handles.
    pub fn inspect_role_bundles(&self) -> Result<RoleBundleInspection, NativeV015Error> {
        self.require_binding()?;
        for &member in self.admitted.members() {
            if !self.admitted.is_member(&self.memory.store, member) {
                return Err(NativeV015Error::InvalidBinding);
            }
        }
        inspect_role_bundles_structurally(&self.memory.store, self.admitted.members())
            .map_err(|_| NativeV015Error::InvalidRoleBundleTopology)
    }

    pub fn is_member(&self, handle: Handle) -> Result<bool, NativeV015Error> {
        self.require_binding()?;
        Ok(self.admitted.is_member(&self.memory.store, handle))
    }

    pub fn is_closed(&self) -> bool {
        self.state == SessionStateV1::Closed
    }

    pub fn close(&mut self) -> Result<(), NativeV015Error> {
        self.require_binding()?;
        self.state = SessionStateV1::Closed;
        Ok(())
    }

    // Explicitly no Γ, fake QUIESCENT, zero-step success, or legacy executor.
    pub fn step(&mut self) -> Result<(), NativeV015Error> {
        self.require_binding()?;
        Err(NativeV015Error::GammaNotImplemented)
    }
    pub fn run(&mut self) -> Result<(), NativeV015Error> {
        self.require_binding()?;
        Err(NativeV015Error::GammaNotImplemented)
    }
    pub fn run_to_quiescence(&mut self) -> Result<(), NativeV015Error> {
        self.require_binding()?;
        Err(NativeV015Error::GammaNotImplemented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use amemory_optimized_cpu_probe::{OptimizedLinkStore, ROOT_HANDLE};

    #[test]
    #[ignore = "real pinned upstream anum_docs source replay required"]
    fn a1_g2_native_bound_session_real_g1_n1_n15() {
        let checkout = std::path::PathBuf::from(
            std::env::var("A1_TRUSTED_ANUM_CHECKOUT").expect("trusted checkout"),
        );
        let candidate = std::fs::read_to_string(
            std::env::var("MTS_V015_SNAPSHOT_INPUT").expect("accepted source"),
        )
        .unwrap();
        let forged = std::fs::read_to_string(
            std::env::var("MTS_V015_TAMPERED_SNAPSHOT_INPUT").expect("SHA-recomputed forgery"),
        )
        .unwrap();
        // N1: absence of G1 means no Session.
        assert!(matches!(
            NativeV015Session::open_trusted_compact_and(
                &candidate,
                Path::new("/missing-g2-trust-checkout")
            ),
            Err(NativeV015Error::TrustOrLoad(_))
        ));
        let mut a = NativeV015Session::open_trusted_compact_and(&candidate, &checkout).unwrap();
        let first = a.observe().unwrap();
        assert_eq!(first.positive_members.len(), 4);
        assert!(!first.gamma_implemented);
        assert_eq!(first.state, "OPEN");
        assert_ne!(first.session_id, first.memory_id);
        let verified = SourceReplayVerified::from_pinned_compact_and_checkout(&checkout).unwrap();
        // N2/N3: attacker image with a freshly recomputed digest cannot mint.
        assert!(matches!(
            NativeV015Session::from_verified(CpuMemoryInstance::new(), &forged, &verified),
            Err(NativeV015Error::TrustOrLoad(_))
        ));
        let mut noisy = CpuMemoryInstance::new();
        let noise = noisy.store.ensure_pair(ROOT_HANDLE, ROOT_HANDLE).unwrap();
        noisy.store.ensure_start_self_closed(noise).unwrap();
        let mut b = NativeV015Session::from_verified(noisy, &candidate, &verified).unwrap();
        let second = b.observe().unwrap();
        // A1-3 G1: source-backed compact AND; zero RoleBundle arity only.
        let first_roles = a.inspect_role_bundles().unwrap();
        let second_roles = b.inspect_role_bundles().unwrap();
        assert_eq!(first_roles, second_roles); // canonical wires, not u32
        assert_eq!(first_roles.candidates.len(), 4);
        assert!(first_roles.unclassified_positive_members.is_empty());
        assert!(first_roles.malformed_sequence_members.is_empty());
        assert!(first_roles.ambiguous_members.is_empty());
        assert!(first_roles
            .candidates
            .iter()
            .all(|r| { r.roles.is_empty() && r.output_templates.len() == 1 }));
        assert_eq!(
            first_roles
                .candidates
                .iter()
                .map(|r| r.positive_admission.clone())
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            first.positive_members.len()
        );
        assert_ne!(first.session_id, second.session_id);
        assert_ne!(first.memory_id, second.memory_id);
        assert_ne!(first.store_instance_id, second.store_instance_id);
        assert_ne!(first.positive_members, second.positive_members);
        assert_eq!(first.canonical_members, second.canonical_members);
        assert_eq!(
            first.semantic_content_sha256,
            second.semantic_content_sha256
        );
        assert_eq!(first.trusted_source_sha256, second.trusted_source_sha256);
        // N4: the receipt belonging to A cannot be bound to B.
        let mut source = CpuMemoryInstance::new();
        let foreign = source
            .store
            .load_verified_semantic_anet(&candidate, &verified)
            .unwrap();
        assert!(matches!(
            NativeV015Session::bind_loaded(CpuMemoryInstance::new(), foreign),
            Err(NativeV015Error::InvalidBinding)
        ));
        // N5: a foreign store's handle cannot transfer authority.
        let foreign_member = second.positive_members[0];
        assert!(!a.admitted.is_member(&b.memory.store, foreign_member));
        // N6: physical existence/closure is not semantic membership.
        let nonmember = a
            .admitted
            .physical_coordinates()
            .iter()
            .copied()
            .find(|h| !a.admitted.members().contains(h))
            .unwrap();
        assert!(!a.is_member(nonmember).unwrap());
        // N10/N11/N12/N13: no Γ and no implicit legacy fallback or mutation.
        let carrier = a.memory.store.export_packed_duplets();
        let indices = a.memory.store.export_packed_incidence_index_image();
        assert_eq!(a.step(), Err(NativeV015Error::GammaNotImplemented));
        assert_eq!(a.run(), Err(NativeV015Error::GammaNotImplemented));
        assert_eq!(
            a.run_to_quiescence(),
            Err(NativeV015Error::GammaNotImplemented)
        );
        assert_eq!(a.observe().unwrap(), first);
        assert_eq!(a.memory.store.export_packed_duplets(), carrier);
        assert_eq!(
            a.memory.store.export_packed_incidence_index_image(),
            indices
        );
        // N15 observation is read-only.
        assert_eq!(a.observe().unwrap(), a.observe().unwrap());
        // N7/N8: replacing carrier invalidates bound generation.
        a.memory
            .store
            .load_packed_duplets(&[(ROOT_HANDLE, ROOT_HANDLE)])
            .unwrap();
        assert_eq!(a.observe(), Err(NativeV015Error::InvalidBinding));
        assert_eq!(
            a.inspect_role_bundles(),
            Err(NativeV015Error::InvalidBinding)
        );
        assert_eq!(a.step(), Err(NativeV015Error::InvalidBinding));
        assert_eq!(a.run(), Err(NativeV015Error::InvalidBinding));
        // N9: failed admission never produces an observable Session.
        let mut limited = CpuMemoryInstance::new();
        limited.store = OptimizedLinkStore::with_max_links(2);
        assert!(matches!(
            NativeV015Session::from_verified(limited, &candidate, &verified),
            Err(NativeV015Error::TrustOrLoad(_))
        ));
        // N14: no resurrection after Close.
        b.close().unwrap();
        assert!(b.is_closed());
        assert_eq!(b.observe(), Err(NativeV015Error::Closed));
        assert_eq!(b.inspect_role_bundles(), Err(NativeV015Error::Closed));
        assert_eq!(b.step(), Err(NativeV015Error::Closed));
        assert_eq!(b.run(), Err(NativeV015Error::Closed));
        assert_eq!(b.close(), Err(NativeV015Error::Closed));
        println!("A1_G2_NATIVE_SESSION_REAL_G1_N1_N15=PASS");
    }
}
