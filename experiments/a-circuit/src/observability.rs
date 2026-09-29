use amemory_optimized_cpu_probe::structural::StructuralRunProfile;
use serde::{Deserialize, Serialize};

pub(crate) const RUN_OBSERVABILITY_SCHEMA_VERSION: u32 = 1;
pub(crate) const OPTIMIZED_CPU_BACKEND_ID: &str = "optimized-cpu";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum RunObservationLevel {
    Off,
    Profile,
    Trace,
    Full,
}

impl RunObservationLevel {
    pub(crate) fn profiles(self) -> bool {
        !matches!(self, Self::Off)
    }

    pub(crate) fn traces(self) -> bool {
        matches!(self, Self::Trace | Self::Full)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum RunStage {
    Prepare,
    Load,
    Configure,
    Execute,
    Result,
    Evidence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum RunEventKind {
    ExecuteBegin,
    ReactionEnd,
    Quiescence,
    RunEnd,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunEventV1 {
    pub(crate) schema_version: u32,
    pub(crate) session_id: String,
    pub(crate) run_id: u64,
    pub(crate) backend_id: String,
    pub(crate) sequence: u32,
    pub(crate) elapsed_ns: u64,
    pub(crate) stage: RunStage,
    pub(crate) kind: RunEventKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) reaction_index: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) scope_before: Option<Vec<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) scope_after: Option<Vec<u32>>,
    pub(crate) links_after: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) raw_rule_matches: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) transitioned_members: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) handoff_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) quiescent: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StructuralProfileV1 {
    pub(crate) trigger_incidence_candidates: u64,
    pub(crate) candidates_rejected_before_unification: u64,
    pub(crate) role_dictionary_decodes: u64,
    pub(crate) decoded_roles: u64,
    pub(crate) unification_attempts: u64,
    pub(crate) unification_successes: u64,
    pub(crate) contains_role_nodes_visited: u64,
    pub(crate) unification_nodes_visited: u64,
    pub(crate) instantiation_nodes_visited: u64,
    pub(crate) instantiation_constructor_attempts: u64,
    pub(crate) instantiation_canonical_hits: u64,
    pub(crate) instantiation_new_links: u64,
    pub(crate) publication_outputs: u64,
    pub(crate) rule_metadata_cache_hits: u64,
    pub(crate) rule_metadata_cache_misses: u64,
    pub(crate) grounded_path_checks: u64,
    pub(crate) grounded_path_rejects: u64,
    pub(crate) discovery_ns: u64,
    pub(crate) role_decode_ns: u64,
    pub(crate) unification_ns: u64,
    pub(crate) instantiation_ns: u64,
    pub(crate) publication_ns: u64,
    pub(crate) total_ns: u64,
}

impl From<&StructuralRunProfile> for StructuralProfileV1 {
    fn from(value: &StructuralRunProfile) -> Self {
        Self {
            trigger_incidence_candidates: value.trigger_incidence_candidates,
            candidates_rejected_before_unification:
                value.candidates_rejected_before_unification,
            role_dictionary_decodes: value.role_dictionary_decodes,
            decoded_roles: value.decoded_roles,
            unification_attempts: value.unification_attempts,
            unification_successes: value.unification_successes,
            contains_role_nodes_visited: value.contains_role_nodes_visited,
            unification_nodes_visited: value.unification_nodes_visited,
            instantiation_nodes_visited: value.instantiation_nodes_visited,
            instantiation_constructor_attempts:
                value.instantiation_constructor_attempts,
            instantiation_canonical_hits: value.instantiation_canonical_hits,
            instantiation_new_links: value.instantiation_new_links,
            publication_outputs: value.publication_outputs,
            rule_metadata_cache_hits: value.rule_metadata_cache_hits,
            rule_metadata_cache_misses: value.rule_metadata_cache_misses,
            grounded_path_checks: value.grounded_path_checks,
            grounded_path_rejects: value.grounded_path_rejects,
            discovery_ns: ns_u64(value.discovery_ns),
            role_decode_ns: ns_u64(value.role_decode_ns),
            unification_ns: ns_u64(value.unification_ns),
            instantiation_ns: ns_u64(value.instantiation_ns),
            publication_ns: ns_u64(value.publication_ns),
            total_ns: ns_u64(value.total_ns),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunProfileV1 {
    pub(crate) schema_version: u32,
    pub(crate) session_id: String,
    pub(crate) run_id: u64,
    pub(crate) backend_id: String,
    pub(crate) base_links: u32,
    pub(crate) links_before_run: u32,
    pub(crate) links_after_run: u32,
    pub(crate) execution_link_delta: u32,
    pub(crate) scope_before_width: u32,
    pub(crate) scope_after_width: u32,
    pub(crate) active_reaction_count: u32,
    pub(crate) execute_ns: u64,
    pub(crate) trace_projection_ns: u64,
    pub(crate) structural: StructuralProfileV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ObservedRunV1 {
    pub(crate) schema_version: u32,
    pub(crate) session_id: String,
    pub(crate) run_id: u64,
    pub(crate) backend_id: String,
    pub(crate) observation_level: RunObservationLevel,
    pub(crate) final_scope: Vec<u32>,
    pub(crate) active_reaction_count: u32,
    pub(crate) final_quiescent: bool,
    pub(crate) events: Vec<RunEventV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) profile: Option<RunProfileV1>,
}

pub(crate) fn ns_u64(value: u128) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observed_run_v1_json_round_trip_is_versioned() {
        let run = ObservedRunV1 {
            schema_version: RUN_OBSERVABILITY_SCHEMA_VERSION,
            session_id: "A-memory#test".to_owned(),
            run_id: 7,
            backend_id: OPTIMIZED_CPU_BACKEND_ID.to_owned(),
            observation_level: RunObservationLevel::Trace,
            final_scope: vec![11, 12],
            active_reaction_count: 2,
            final_quiescent: true,
            events: vec![RunEventV1 {
                schema_version: RUN_OBSERVABILITY_SCHEMA_VERSION,
                session_id: "A-memory#test".to_owned(),
                run_id: 7,
                backend_id: OPTIMIZED_CPU_BACKEND_ID.to_owned(),
                sequence: 0,
                elapsed_ns: 1,
                stage: RunStage::Execute,
                kind: RunEventKind::ExecuteBegin,
                reaction_index: None,
                scope_before: Some(vec![10]),
                scope_after: None,
                links_after: 12,
                raw_rule_matches: None,
                transitioned_members: None,
                handoff_count: None,
                quiescent: None,
            }],
            profile: None,
        };

        let json = serde_json::to_string(&run).unwrap();
        assert!(json.contains("\"schemaVersion\":1"));
        assert!(json.contains("\"observationLevel\":\"TRACE\""));
        assert!(json.contains("\"stage\":\"EXECUTE\""));
        let decoded: ObservedRunV1 = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, run);
    }

    #[test]
    fn u128_timing_projection_saturates() {
        assert_eq!(ns_u64(5), 5);
        assert_eq!(ns_u64(u128::MAX), u64::MAX);
    }
}
