use serde::{Deserialize, Serialize};

pub(crate) const SESSION_CONTRACT_SCHEMA_VERSION: u32 = 1;


pub(crate) const RUN_BUDGET_SCHEMA_VERSION: u32 = 3;
pub(crate) const RUN_BUDGET_ACCOUNTING_SCHEMA_VERSION: u32 = 2;
pub(crate) const DEFAULT_MAX_APPENDED_LINKS_PER_RUN: u32 = 1_000_000;
pub(crate) const DEFAULT_MAX_TOTAL_LINKS: u32 = 2_000_000;
pub(crate) const DEFAULT_MAX_SCOPE_WIDTH: u32 = 65_536;
pub(crate) const DEFAULT_MAX_MATCH_CANDIDATES: u64 = 1_000_000_000;
pub(crate) const DEFAULT_MAX_UNIFICATION_NODES: u64 = 1_000_000_000;
pub(crate) const DEFAULT_MAX_INSTANTIATION_NODES: u64 = 1_000_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunBudgetV1 {
    pub(crate) schema_version: u32,
    pub(crate) max_reactions: u32,
    pub(crate) max_appended_links: u32,
    pub(crate) max_total_links: u32,
    pub(crate) max_scope_width: u32,
    pub(crate) max_match_candidates: u64,
    pub(crate) max_unification_nodes: u64,
    pub(crate) max_instantiation_nodes: u64,
}

impl RunBudgetV1 {
    pub(crate) fn scenario_default(max_reactions: u32) -> Self {
        Self {
            schema_version: RUN_BUDGET_SCHEMA_VERSION,
            max_reactions,
            max_appended_links: DEFAULT_MAX_APPENDED_LINKS_PER_RUN,
            max_total_links: DEFAULT_MAX_TOTAL_LINKS,
            max_scope_width: DEFAULT_MAX_SCOPE_WIDTH,
            max_match_candidates: DEFAULT_MAX_MATCH_CANDIDATES,
            max_unification_nodes: DEFAULT_MAX_UNIFICATION_NODES,
            max_instantiation_nodes: DEFAULT_MAX_INSTANTIATION_NODES,
        }
    }

    pub(crate) fn resource_stop_reason(
        self,
        links_before_run: u32,
        links_now: u32,
        scope_width: u32,
    ) -> Option<RunStopReasonV1> {
        if links_now > self.max_total_links {
            return Some(RunStopReasonV1::TotalLinksBudgetExceeded);
        }
        if links_now.saturating_sub(links_before_run)
            > self.max_appended_links
        {
            return Some(RunStopReasonV1::AppendedLinksBudgetExceeded);
        }
        if scope_width > self.max_scope_width {
            return Some(RunStopReasonV1::ScopeWidthBudgetExceeded);
        }
        None
    }

    pub(crate) fn work_stop_reason(
        self,
        usage: RunWorkUsageV1,
    ) -> Option<RunStopReasonV1> {
        if usage.match_candidates > self.max_match_candidates {
            return Some(RunStopReasonV1::MatchWorkBudgetExceeded);
        }
        if usage.unification_nodes > self.max_unification_nodes {
            return Some(RunStopReasonV1::UnificationWorkBudgetExceeded);
        }
        if usage.instantiation_nodes > self.max_instantiation_nodes {
            return Some(RunStopReasonV1::InstantiationWorkBudgetExceeded);
        }
        None
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunWorkUsageV1 {
    pub(crate) match_candidates: u64,
    pub(crate) unification_nodes: u64,
    pub(crate) instantiation_nodes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum BackendResourceAccountingV1 {
    OptimizedCpuDenseCarrier {
        #[serde(rename = "denseCarrierAllocatedBytes")]
        dense_carrier_allocated_bytes: u64,
        #[serde(rename = "maxDenseCarrierBytes")]
        max_dense_carrier_bytes: u64,
        #[serde(rename = "fullResidentBytesAvailable")]
        full_resident_bytes_available: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunBudgetAccountingV1 {
    pub(crate) schema_version: u32,
    pub(crate) reactions_consumed: u32,
    pub(crate) max_reactions: u32,
    pub(crate) appended_links_consumed: u32,
    pub(crate) max_appended_links: u32,
    pub(crate) total_links: u32,
    pub(crate) max_total_links: u32,
    pub(crate) scope_width: u32,
    pub(crate) max_scope_width: u32,
    pub(crate) match_candidates: u64,
    pub(crate) max_match_candidates: u64,
    pub(crate) unification_nodes: u64,
    pub(crate) max_unification_nodes: u64,
    pub(crate) instantiation_nodes: u64,
    pub(crate) max_instantiation_nodes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) backend_resource: Option<BackendResourceAccountingV1>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum RunStopReasonV1 {
    Quiescent,
    ReactionBudgetExceeded,
    AppendedLinksBudgetExceeded,
    TotalLinksBudgetExceeded,
    ScopeWidthBudgetExceeded,
    MatchWorkBudgetExceeded,
    UnificationWorkBudgetExceeded,
    InstantiationWorkBudgetExceeded,
    CarrierBytesBudgetExceeded,
    EngineFailure,
    InvalidState,
}

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
    fn common_budget_does_not_claim_backend_physical_bytes() {
        let budget = RunBudgetV1::scenario_default(8);
        let json = serde_json::to_value(budget).unwrap();
        assert_eq!(budget.schema_version, RUN_BUDGET_SCHEMA_VERSION);
        assert!(json.get("maxDenseCarrierBytes").is_none());
        assert!(json.get("maxCarrierBytes").is_none());
    }

    #[test]
    fn backend_resource_accounting_is_explicitly_tagged() {
        let accounting = RunBudgetAccountingV1 {
            schema_version: RUN_BUDGET_ACCOUNTING_SCHEMA_VERSION,
            reactions_consumed: 0,
            max_reactions: 1,
            appended_links_consumed: 0,
            max_appended_links: 1,
            total_links: 1,
            max_total_links: 2,
            scope_width: 0,
            max_scope_width: 1,
            match_candidates: 0,
            max_match_candidates: 1,
            unification_nodes: 0,
            max_unification_nodes: 1,
            instantiation_nodes: 0,
            max_instantiation_nodes: 1,
            backend_resource: Some(
                BackendResourceAccountingV1::OptimizedCpuDenseCarrier {
                    dense_carrier_allocated_bytes: 64,
                    max_dense_carrier_bytes: 128,
                    full_resident_bytes_available: false,
                },
            ),
        };
        let json = serde_json::to_value(accounting).unwrap();
        assert_eq!(
            json["backendResource"]["kind"],
            "OPTIMIZED_CPU_DENSE_CARRIER",
        );
        assert_eq!(json["backendResource"]["denseCarrierAllocatedBytes"], 64);
        assert_eq!(json["backendResource"]["maxDenseCarrierBytes"], 128);
        assert_eq!(json["backendResource"]["fullResidentBytesAvailable"], false);
        assert!(json["backendResource"]
            .get("dense_carrier_allocated_bytes")
            .is_none());
    }

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
