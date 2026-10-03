use super::session_contract::{
    CapabilitySupportV1, RuntimeBackendV1, RuntimeSessionV1,
    SessionCapabilitiesV1, SessionIdentityV1, SessionStateV1,
    SESSION_CONTRACT_SCHEMA_VERSION,
};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_LINKSDB_MEMORY_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_LINKSDB_SESSION_ID: AtomicU64 = AtomicU64::new(1);

/// Minimal physical-store boundary for the LinksDB adapter.
///
/// This trait intentionally contains no generalized-MP operation. A concrete
/// Doublets implementation may satisfy physical storage/index operations later,
/// while semantic execution remains governed by the common A-memory contract.
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

/// First real #272 consumer for the future LinksDB physical backend.
///
/// This Session is deliberately storage/lifecycle-only. It proves identity,
/// state, snapshot and fail-closed capability reporting before any Doublets
/// dependency or backend-local semantic executor is introduced.
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
}
