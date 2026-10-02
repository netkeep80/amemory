use serde::{Deserialize, Serialize};

pub(crate) const SESSION_CONTRACT_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum RuntimeBackendV1 {
    OptimizedCpu,
    Webgpu,
    Linksdb,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum SessionStateV1 {
    Open,
    Configured,
    Running,
    Quiescent,
    Failed,
    Closed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum CapabilitySupportV1 {
    Supported,
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionCapabilitiesV1 {
    pub(crate) schema_version: u32,
    pub(crate) persistent_session: CapabilitySupportV1,
    pub(crate) reconfigure_without_reload: CapabilitySupportV1,
    pub(crate) step: CapabilitySupportV1,
    pub(crate) run_to_quiescence: CapabilitySupportV1,
    pub(crate) snapshot: CapabilitySupportV1,
    pub(crate) profile: CapabilitySupportV1,
    pub(crate) trace: CapabilitySupportV1,
    pub(crate) explicit_close: CapabilitySupportV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionIdentityV1 {
    pub(crate) memory_instance_id: String,
    pub(crate) session_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionSnapshotV1 {
    pub(crate) schema_version: u32,
    pub(crate) backend: RuntimeBackendV1,
    pub(crate) identity: SessionIdentityV1,
    pub(crate) state: SessionStateV1,
    pub(crate) capabilities: SessionCapabilitiesV1,
    pub(crate) base_link_count: u32,
    pub(crate) current_link_count: u32,
    pub(crate) scope_width: u32,
}

/// Physical backends implement this lifecycle/status boundary.
///
/// The trait deliberately exposes no generalized-MP operation and no storage
/// primitive. Semantic execution remains owned by each conforming backend
/// executor; this boundary only normalizes identity, lifecycle state and
/// capability/status facts.
pub(crate) trait RuntimeSessionV1 {
    fn runtime_backend_v1(&self) -> RuntimeBackendV1;
    fn runtime_identity_v1(&self) -> SessionIdentityV1;
    fn runtime_state_v1(&self) -> SessionStateV1;
    fn runtime_capabilities_v1(&self) -> SessionCapabilitiesV1;
    fn runtime_base_link_count_v1(&self) -> u32;
    fn runtime_current_link_count_v1(&self) -> u32;
    fn runtime_scope_width_v1(&self) -> u32;

    fn runtime_snapshot_v1(&self) -> SessionSnapshotV1 {
        SessionSnapshotV1 {
            schema_version: SESSION_CONTRACT_SCHEMA_VERSION,
            backend: self.runtime_backend_v1(),
            identity: self.runtime_identity_v1(),
            state: self.runtime_state_v1(),
            capabilities: self.runtime_capabilities_v1(),
            base_link_count: self.runtime_base_link_count_v1(),
            current_link_count: self.runtime_current_link_count_v1(),
            scope_width: self.runtime_scope_width_v1(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_support_is_explicit_machine_data() {
        assert_eq!(
            serde_json::to_string(&CapabilitySupportV1::Supported).unwrap(),
            "\"SUPPORTED\"",
        );
        assert_eq!(
            serde_json::to_string(&CapabilitySupportV1::Unsupported).unwrap(),
            "\"UNSUPPORTED\"",
        );
    }
}
