use super::session_contract::{
    CapabilitySupportV1, RuntimeBackendV1, RuntimeSessionV1,
    SessionCapabilitiesV1, SessionIdentityV1, SessionStateV1,
    SESSION_CONTRACT_SCHEMA_VERSION,
};
use std::sync::atomic::{AtomicU64, Ordering};

#[cfg(not(target_family = "wasm"))]
use amemory_optimized_cpu_probe::ROOT_HANDLE;
#[cfg(not(target_family = "wasm"))]
use doublets::Doublets;
#[cfg(not(target_family = "wasm"))]
use std::collections::HashMap;

static NEXT_LINKSDB_MEMORY_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_LINKSDB_SESSION_ID: AtomicU64 = AtomicU64::new(1);

/// Minimal physical-store boundary for the LinksDB adapter.
///
/// This trait intentionally contains no generalized-MP operation. A concrete
/// Doublets implementation satisfies physical storage/index operations only;
/// semantic execution remains governed by the common A-memory contract.
pub(crate) trait LinksDbPhysicalStoreV1 {
    fn link_count_v1(&self) -> u32;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LinksDbCapabilityV1 {
    Configure,
    Step,
    RunToQuiescence,
    Profile,
    Trace,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LinksDbSessionErrorV1 {
    UnsupportedCapability {
        capability: LinksDbCapabilityV1,
    },
    Closed,
}

/// First real #272 consumer for the LinksDB physical backend.
///
/// This Session is deliberately storage/lifecycle-only. It proves identity,
/// state, snapshot and fail-closed capability reporting before semantic
/// execution is connected to the physical adapter.
pub(crate) struct LinksDbSessionV1<S: LinksDbPhysicalStoreV1> {
    store: S,
    identity: SessionIdentityV1,
    state: SessionStateV1,
    base_link_count: u32,
}

impl<S: LinksDbPhysicalStoreV1> LinksDbSessionV1<S> {
    pub(crate) fn open(store: S) -> Self {
        let base_link_count = store.link_count_v1();
        Self {
            store,
            identity: SessionIdentityV1 {
                memory_instance_id: format!(
                    "LinksDB-memory#{}",
                    NEXT_LINKSDB_MEMORY_ID.fetch_add(1, Ordering::Relaxed),
                ),
                session_id: format!(
                    "LinksDB-session#{}",
                    NEXT_LINKSDB_SESSION_ID.fetch_add(1, Ordering::Relaxed),
                ),
            },
            state: SessionStateV1::Open,
            base_link_count,
        }
    }

    fn reject(
        &self,
        capability: LinksDbCapabilityV1,
    ) -> Result<(), LinksDbSessionErrorV1> {
        if self.state == SessionStateV1::Closed {
            Err(LinksDbSessionErrorV1::Closed)
        } else {
            Err(LinksDbSessionErrorV1::UnsupportedCapability { capability })
        }
    }

    pub(crate) fn configure(
        &mut self,
    ) -> Result<(), LinksDbSessionErrorV1> {
        self.reject(LinksDbCapabilityV1::Configure)
    }

    pub(crate) fn step(&mut self) -> Result<(), LinksDbSessionErrorV1> {
        self.reject(LinksDbCapabilityV1::Step)
    }

    pub(crate) fn run_to_quiescence(
        &mut self,
    ) -> Result<(), LinksDbSessionErrorV1> {
        self.reject(LinksDbCapabilityV1::RunToQuiescence)
    }

    pub(crate) fn profile(&self) -> Result<(), LinksDbSessionErrorV1> {
        self.reject(LinksDbCapabilityV1::Profile)
    }

    pub(crate) fn trace(&self) -> Result<(), LinksDbSessionErrorV1> {
        self.reject(LinksDbCapabilityV1::Trace)
    }

    pub(crate) fn close(&mut self) -> bool {
        if self.state == SessionStateV1::Closed {
            return false;
        }
        self.state = SessionStateV1::Closed;
        true
    }
}

impl<S: LinksDbPhysicalStoreV1> RuntimeSessionV1 for LinksDbSessionV1<S> {
    fn runtime_backend_v1(&self) -> RuntimeBackendV1 {
        RuntimeBackendV1::Linksdb
    }

    fn runtime_identity_v1(&self) -> SessionIdentityV1 {
        self.identity.clone()
    }

    fn runtime_state_v1(&self) -> SessionStateV1 {
        self.state
    }

    fn runtime_capabilities_v1(&self) -> SessionCapabilitiesV1 {
        SessionCapabilitiesV1 {
            schema_version: SESSION_CONTRACT_SCHEMA_VERSION,
            persistent_session: CapabilitySupportV1::Supported,
            reconfigure_without_reload: CapabilitySupportV1::Unsupported,
            step: CapabilitySupportV1::Unsupported,
            run_to_quiescence: CapabilitySupportV1::Unsupported,
            snapshot: CapabilitySupportV1::Supported,
            profile: CapabilitySupportV1::Unsupported,
            trace: CapabilitySupportV1::Unsupported,
            explicit_close: CapabilitySupportV1::Supported,
        }
    }

    fn runtime_base_link_count_v1(&self) -> u32 {
        self.base_link_count
    }

    fn runtime_current_link_count_v1(&self) -> u32 {
        self.store.link_count_v1()
    }

    fn runtime_scope_width_v1(&self) -> u32 {
        0
    }
}

#[cfg(not(target_family = "wasm"))]
type NativeDoubletsUnitStoreV1 = doublets::unit::Store<
    usize,
    doublets::mem::Global<doublets::unit::LinkPart<usize>>,
>;

#[cfg(not(target_family = "wasm"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PortableLinkKindV1 {
    Root,
    Pair,
}

#[cfg(not(target_family = "wasm"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PortablePhysicalLinkV1 {
    pub(crate) handle: u32,
    pub(crate) start: u32,
    pub(crate) end: u32,
    pub(crate) physical_index: usize,
}

#[cfg(not(target_family = "wasm"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PortablePhysicalRecordV1 {
    link: PortablePhysicalLinkV1,
    kind: PortableLinkKindV1,
}

#[cfg(not(target_family = "wasm"))]
#[derive(Debug)]
pub(crate) enum DoubletsPhysicalStoreErrorV1 {
    Upstream(doublets::Error<usize>),
    UnknownPortableHandle(u32),
    MissingPhysicalLink(usize),
    UnmappedPhysicalEndpoint(usize),
    PortableHandleExhausted,
    PortableIndexMismatch {
        start: u32,
        end: u32,
        expected: Option<u32>,
        observed: Vec<u32>,
    },
    PortableRecordMismatch(u32),
}

#[cfg(not(target_family = "wasm"))]
impl From<doublets::Error<usize>> for DoubletsPhysicalStoreErrorV1 {
    fn from(value: doublets::Error<usize>) -> Self {
        Self::Upstream(value)
    }
}

/// Native-only #280/L1 adapter over the exact-pinned Doublets unit store.
///
/// The portable A-memory identity table is intentionally separate from
/// Doublets-local indices. Doublets pair identity cannot be semantic authority:
/// a self-incidence record and an ordinary A-memory PAIR may have identical
/// raw physical poles while remaining distinct Links.
///
/// A physical-only marker is allocated before ROOT so tests cannot accidentally
/// rely on portable handle == Doublets index.
#[cfg(not(target_family = "wasm"))]
pub(crate) struct DoubletsPhysicalStoreV1 {
    store: NativeDoubletsUnitStoreV1,
    physical_metadata_marker: usize,
    portable_records: Vec<PortablePhysicalRecordV1>,
    physical_to_portable: HashMap<usize, u32>,
    canonical_pairs: HashMap<(u32, u32), u32>,
}

#[cfg(not(target_family = "wasm"))]
impl DoubletsPhysicalStoreV1 {
    pub(crate) fn open_empty() -> Result<Self, DoubletsPhysicalStoreErrorV1> {
        let mut store = doublets::unit::Store::<usize, _>::new(
            doublets::mem::Global::new(),
        )?;

        // This record belongs to the physical adapter, not to A-memory. Its
        // presence makes local Doublets indices observably non-portable.
        let physical_metadata_marker = store.create_point()?;
        let physical_root = store.create_point()?;

        debug_assert_eq!(ROOT_HANDLE, 1);
        let root = PortablePhysicalRecordV1 {
            link: PortablePhysicalLinkV1 {
                handle: ROOT_HANDLE,
                start: ROOT_HANDLE,
                end: ROOT_HANDLE,
                physical_index: physical_root,
            },
            kind: PortableLinkKindV1::Root,
        };

        let mut physical_to_portable = HashMap::new();
        physical_to_portable.insert(physical_root, ROOT_HANDLE);

        Ok(Self {
            store,
            physical_metadata_marker,
            portable_records: vec![root],
            physical_to_portable,
            canonical_pairs: HashMap::new(),
        })
    }

    fn portable_record(
        &self,
        handle: u32,
    ) -> Result<&PortablePhysicalRecordV1, DoubletsPhysicalStoreErrorV1> {
        let index = handle
            .checked_sub(1)
            .ok_or(DoubletsPhysicalStoreErrorV1::UnknownPortableHandle(handle))?;
        self.portable_records
            .get(index as usize)
            .ok_or(DoubletsPhysicalStoreErrorV1::UnknownPortableHandle(handle))
    }

    fn next_portable_handle(&self) -> Result<u32, DoubletsPhysicalStoreErrorV1> {
        u32::try_from(self.portable_records.len() + 1)
            .map_err(|_| DoubletsPhysicalStoreErrorV1::PortableHandleExhausted)
    }

    pub(crate) fn physical_index_v1(
        &self,
        handle: u32,
    ) -> Result<usize, DoubletsPhysicalStoreErrorV1> {
        Ok(self.portable_record(handle)?.link.physical_index)
    }

    pub(crate) fn physical_link_count_v1(&self) -> usize {
        self.store.count()
    }

    pub(crate) fn physical_metadata_marker_v1(&self) -> usize {
        self.physical_metadata_marker
    }

    /// Direct Doublets pair search is exposed only as a physical diagnostic.
    ///
    /// It is deliberately not used as A-memory canonical identity because, for
    /// example, physical ROOT and PAIR(ROOT, ROOT) have equal source/target
    /// poles in this encoding.
    pub(crate) fn raw_physical_search_v1(
        &self,
        start: u32,
        end: u32,
    ) -> Result<Option<usize>, DoubletsPhysicalStoreErrorV1> {
        let physical_start = self.physical_index_v1(start)?;
        let physical_end = self.physical_index_v1(end)?;
        Ok(self.store.search(physical_start, physical_end))
    }

    fn physical_pair_candidates_v1(
        &self,
        start: u32,
        end: u32,
    ) -> Result<Vec<u32>, DoubletsPhysicalStoreErrorV1> {
        let physical_start = self.physical_index_v1(start)?;
        let physical_end = self.physical_index_v1(end)?;
        let any = self.store.constants().any;
        let mut candidates = Vec::new();

        self.store.each_by(
            [any, physical_start, physical_end],
            |physical| {
                if let Some(portable) =
                    self.physical_to_portable.get(&physical.index).copied()
                {
                    if let Ok(record) = self.portable_record(portable) {
                        if record.kind == PortableLinkKindV1::Pair
                            && record.link.start == start
                            && record.link.end == end
                        {
                            candidates.push(portable);
                        }
                    }
                }
                doublets::data::Flow::Continue
            },
        );

        candidates.sort_unstable();
        candidates.dedup();
        Ok(candidates)
    }

    /// Portable ordinary-PAIR lookup backed by Doublets physical enumeration.
    ///
    /// The adapter-owned constructor map is cross-checked against the physical
    /// result. A mismatch fails closed instead of silently choosing a
    /// backend-local index.
    pub(crate) fn search_pair_v1(
        &self,
        start: u32,
        end: u32,
    ) -> Result<Option<u32>, DoubletsPhysicalStoreErrorV1> {
        // Validate portable endpoints even when the canonical map is empty.
        self.portable_record(start)?;
        self.portable_record(end)?;

        let expected = self.canonical_pairs.get(&(start, end)).copied();
        let observed = self.physical_pair_candidates_v1(start, end)?;
        let physically_unique = if observed.len() == 1 {
            observed.first().copied()
        } else {
            None
        };

        let consistent = match (expected, observed.as_slice()) {
            (None, []) => true,
            (Some(expected), [observed]) => expected == *observed,
            _ => false,
        };
        if !consistent {
            return Err(DoubletsPhysicalStoreErrorV1::PortableIndexMismatch {
                start,
                end,
                expected,
                observed,
            });
        }

        Ok(physically_unique)
    }

    /// Canonical A-memory ordinary PAIR constructor over Doublets storage.
    ///
    /// Bare Doublets duplicate storage is intentional here. Applying Doublets
    /// uniqueness resolution would incorrectly collapse distinct A-memory
    /// constructor records that happen to have equal physical poles.
    pub(crate) fn ensure_pair_v1(
        &mut self,
        start: u32,
        end: u32,
    ) -> Result<u32, DoubletsPhysicalStoreErrorV1> {
        if let Some(existing) = self.search_pair_v1(start, end)? {
            return Ok(existing);
        }

        let physical_start = self.physical_index_v1(start)?;
        let physical_end = self.physical_index_v1(end)?;
        let physical_index =
            self.store.create_link(physical_start, physical_end)?;
        let handle = self.next_portable_handle()?;

        let previous_physical =
            self.physical_to_portable.insert(physical_index, handle);
        debug_assert!(previous_physical.is_none());
        let previous_pair = self.canonical_pairs.insert((start, end), handle);
        debug_assert!(previous_pair.is_none());

        self.portable_records.push(PortablePhysicalRecordV1 {
            link: PortablePhysicalLinkV1 {
                handle,
                start,
                end,
                physical_index,
            },
            kind: PortableLinkKindV1::Pair,
        });

        Ok(handle)
    }

    pub(crate) fn read_v1(
        &self,
        handle: u32,
    ) -> Result<PortablePhysicalLinkV1, DoubletsPhysicalStoreErrorV1> {
        let record = *self.portable_record(handle)?;
        let physical = self
            .store
            .get_link(record.link.physical_index)
            .ok_or(DoubletsPhysicalStoreErrorV1::MissingPhysicalLink(
                record.link.physical_index,
            ))?;

        let start = self
            .physical_to_portable
            .get(&physical.source)
            .copied()
            .ok_or(DoubletsPhysicalStoreErrorV1::UnmappedPhysicalEndpoint(
                physical.source,
            ))?;
        let end = self
            .physical_to_portable
            .get(&physical.target)
            .copied()
            .ok_or(DoubletsPhysicalStoreErrorV1::UnmappedPhysicalEndpoint(
                physical.target,
            ))?;

        if start != record.link.start || end != record.link.end {
            return Err(DoubletsPhysicalStoreErrorV1::PortableRecordMismatch(
                handle,
            ));
        }

        Ok(record.link)
    }

    pub(crate) fn start_incidence_v1(
        &self,
        start: u32,
    ) -> Result<Vec<u32>, DoubletsPhysicalStoreErrorV1> {
        let physical_start = self.physical_index_v1(start)?;
        let any = self.store.constants().any;
        let mut incidence = Vec::new();

        self.store.each_by([any, physical_start, any], |physical| {
            if let Some(portable) =
                self.physical_to_portable.get(&physical.index).copied()
            {
                incidence.push(portable);
            }
            doublets::data::Flow::Continue
        });

        incidence.sort_unstable();
        incidence.dedup();
        Ok(incidence)
    }

    pub(crate) fn end_incidence_v1(
        &self,
        end: u32,
    ) -> Result<Vec<u32>, DoubletsPhysicalStoreErrorV1> {
        let physical_end = self.physical_index_v1(end)?;
        let any = self.store.constants().any;
        let mut incidence = Vec::new();

        self.store.each_by([any, any, physical_end], |physical| {
            if let Some(portable) =
                self.physical_to_portable.get(&physical.index).copied()
            {
                incidence.push(portable);
            }
            doublets::data::Flow::Continue
        });

        incidence.sort_unstable();
        incidence.dedup();
        Ok(incidence)
    }
}

#[cfg(not(target_family = "wasm"))]
impl LinksDbPhysicalStoreV1 for DoubletsPhysicalStoreV1 {
    fn link_count_v1(&self) -> u32 {
        debug_assert!(self.portable_records.len() <= u32::MAX as usize);
        self.portable_records.len() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    struct ProbeStore {
        links: u32,
    }

    impl LinksDbPhysicalStoreV1 for ProbeStore {
        fn link_count_v1(&self) -> u32 {
            self.links
        }
    }

    #[test]
    fn linksdb_session_is_a_truthful_neutral_runtime_consumer() {
        let session = LinksDbSessionV1::open(ProbeStore { links: 17 });
        let snapshot = session.runtime_snapshot_v1();

        assert_eq!(snapshot.backend, RuntimeBackendV1::Linksdb);
        assert_ne!(
            snapshot.identity.memory_instance_id,
            snapshot.identity.session_id,
        );
        assert_eq!(snapshot.state, SessionStateV1::Open);
        assert_eq!(snapshot.base_link_count, 17);
        assert_eq!(snapshot.current_link_count, 17);
        assert_eq!(snapshot.scope_width, 0);

        let capabilities = snapshot.capabilities;
        assert_eq!(
            capabilities.persistent_session,
            CapabilitySupportV1::Supported,
        );
        assert_eq!(capabilities.snapshot, CapabilitySupportV1::Supported);
        assert_eq!(
            capabilities.explicit_close,
            CapabilitySupportV1::Supported,
        );
        assert_eq!(
            capabilities.reconfigure_without_reload,
            CapabilitySupportV1::Unsupported,
        );
        assert_eq!(capabilities.step, CapabilitySupportV1::Unsupported);
        assert_eq!(
            capabilities.run_to_quiescence,
            CapabilitySupportV1::Unsupported,
        );
        assert_eq!(capabilities.profile, CapabilitySupportV1::Unsupported);
        assert_eq!(capabilities.trace, CapabilitySupportV1::Unsupported);
    }

    #[test]
    fn unsupported_linksdb_execution_fails_closed_without_state_change() {
        let mut session = LinksDbSessionV1::open(ProbeStore { links: 3 });

        assert_eq!(
            session.configure(),
            Err(LinksDbSessionErrorV1::UnsupportedCapability {
                capability: LinksDbCapabilityV1::Configure,
            }),
        );
        assert_eq!(
            session.step(),
            Err(LinksDbSessionErrorV1::UnsupportedCapability {
                capability: LinksDbCapabilityV1::Step,
            }),
        );
        assert_eq!(
            session.run_to_quiescence(),
            Err(LinksDbSessionErrorV1::UnsupportedCapability {
                capability: LinksDbCapabilityV1::RunToQuiescence,
            }),
        );
        assert_eq!(
            session.profile(),
            Err(LinksDbSessionErrorV1::UnsupportedCapability {
                capability: LinksDbCapabilityV1::Profile,
            }),
        );
        assert_eq!(
            session.trace(),
            Err(LinksDbSessionErrorV1::UnsupportedCapability {
                capability: LinksDbCapabilityV1::Trace,
            }),
        );
        assert_eq!(session.runtime_state_v1(), SessionStateV1::Open);
        assert_eq!(session.runtime_current_link_count_v1(), 3);
    }

    #[test]
    fn explicit_close_is_idempotent_and_blocks_further_operations() {
        let mut session = LinksDbSessionV1::open(ProbeStore { links: 5 });

        assert!(session.close());
        assert!(!session.close());
        assert_eq!(session.runtime_state_v1(), SessionStateV1::Closed);
        assert_eq!(session.step(), Err(LinksDbSessionErrorV1::Closed));
    }

    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn linksdb_doublets_portable_identity_is_independent_from_local_index() {
        let store = DoubletsPhysicalStoreV1::open_empty().unwrap();

        assert_eq!(store.link_count_v1(), 1);
        assert_eq!(store.physical_link_count_v1(), 2);
        assert_eq!(store.physical_metadata_marker_v1(), 1);
        assert_eq!(store.physical_index_v1(ROOT_HANDLE).unwrap(), 2);
        assert_ne!(
            store.physical_index_v1(ROOT_HANDLE).unwrap(),
            ROOT_HANDLE as usize,
        );

        let root = store.read_v1(ROOT_HANDLE).unwrap();
        assert_eq!(root.handle, ROOT_HANDLE);
        assert_eq!((root.start, root.end), (ROOT_HANDLE, ROOT_HANDLE));
    }

    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn linksdb_doublets_pair_identity_does_not_collapse_into_root() {
        let mut store = DoubletsPhysicalStoreV1::open_empty().unwrap();
        let physical_root = store.physical_index_v1(ROOT_HANDLE).unwrap();

        // Raw Doublets pair lookup sees ROOT because its physical poles are
        // (root, root). That result is not an A-memory ordinary PAIR.
        assert_eq!(
            store
                .raw_physical_search_v1(ROOT_HANDLE, ROOT_HANDLE)
                .unwrap(),
            Some(physical_root),
        );
        assert_eq!(
            store.search_pair_v1(ROOT_HANDLE, ROOT_HANDLE).unwrap(),
            None,
        );

        let pair = store
            .ensure_pair_v1(ROOT_HANDLE, ROOT_HANDLE)
            .unwrap();
        assert_eq!(pair, 2);
        assert_ne!(store.physical_index_v1(pair).unwrap(), physical_root);
        assert_eq!(
            store.search_pair_v1(ROOT_HANDLE, ROOT_HANDLE).unwrap(),
            Some(pair),
        );

        let link = store.read_v1(pair).unwrap();
        assert_eq!((link.start, link.end), (ROOT_HANDLE, ROOT_HANDLE));
    }

    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn linksdb_doublets_ensure_pair_is_idempotent_and_incidence_is_portable() {
        let mut store = DoubletsPhysicalStoreV1::open_empty().unwrap();

        let first = store
            .ensure_pair_v1(ROOT_HANDLE, ROOT_HANDLE)
            .unwrap();
        let physical_after_first = store.physical_link_count_v1();
        let portable_after_first = store.link_count_v1();

        let second = store
            .ensure_pair_v1(ROOT_HANDLE, ROOT_HANDLE)
            .unwrap();

        assert_eq!(first, second);
        assert_eq!(store.link_count_v1(), portable_after_first);
        assert_eq!(store.physical_link_count_v1(), physical_after_first);
        assert_eq!(
            store.start_incidence_v1(ROOT_HANDLE).unwrap(),
            vec![ROOT_HANDLE, first],
        );
        assert_eq!(
            store.end_incidence_v1(ROOT_HANDLE).unwrap(),
            vec![ROOT_HANDLE, first],
        );
    }

    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn linksdb_doublets_session_counts_portable_links_not_adapter_metadata() {
        let mut store = DoubletsPhysicalStoreV1::open_empty().unwrap();
        let pair = store
            .ensure_pair_v1(ROOT_HANDLE, ROOT_HANDLE)
            .unwrap();
        assert_eq!(pair, 2);
        assert_eq!(store.physical_link_count_v1(), 3);
        assert_eq!(store.link_count_v1(), 2);

        let session = LinksDbSessionV1::open(store);
        let snapshot = session.runtime_snapshot_v1();
        assert_eq!(snapshot.base_link_count, 2);
        assert_eq!(snapshot.current_link_count, 2);
        assert_eq!(snapshot.backend, RuntimeBackendV1::Linksdb);
        assert_eq!(
            snapshot.capabilities.step,
            CapabilitySupportV1::Unsupported,
        );
    }
}
