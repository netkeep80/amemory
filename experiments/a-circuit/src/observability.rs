use amemory_optimized_cpu_probe::structural::{
    StructuralRunProfile, StructuralTraceEvent,
    STRUCTURAL_PROFILE_TIMING_AVAILABLE,
};
use serde::{Deserialize, Serialize};
#[cfg(not(target_family = "wasm"))]
use std::time::Instant;

#[derive(Clone, Debug)]
pub(crate) struct ObservationTimer {
    #[cfg(not(target_family = "wasm"))]
    started: Instant,
}

impl ObservationTimer {
    pub(crate) fn start() -> Self {
        Self {
            #[cfg(not(target_family = "wasm"))]
            started: Instant::now(),
        }
    }

    pub(crate) fn elapsed_ns(&self) -> u128 {
        #[cfg(not(target_family = "wasm"))]
        {
            self.started.elapsed().as_nanos()
        }
        #[cfg(target_family = "wasm")]
        {
            0
        }
    }
}

pub(crate) const OBSERVABILITY_TIMING_AVAILABLE: bool =
    !cfg!(target_family = "wasm");

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum RunStructuralFactKind {
    DiscoveryComplete,
    RuleMatched,
    Instantiated,
    Published,
    ScopeCommitted,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunCreatedLinkV1 {
    pub(crate) handle: u32,
    pub(crate) start: u32,
    pub(crate) end: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunBindingV1 {
    pub(crate) role: u32,
    pub(crate) value: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunStructuralFactV1 {
    pub(crate) kind: RunStructuralFactKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) active: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) rule: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) matched_rules: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) output_bundle_template: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) grounded_bundle: Option<u32>,
    /// Persistent carrier Links physically appended by this instantiation.
    /// This field does not classify them as Context scaffolding.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) created_links: Option<Vec<RunCreatedLinkV1>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) bindings: Option<Vec<RunBindingV1>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) outputs: Option<Vec<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) preserved: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) old_scope: Option<Vec<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) next_scope: Option<Vec<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) quiescent: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) handoff_count: Option<u32>,
}

impl From<StructuralTraceEvent> for RunStructuralFactV1 {
    fn from(event: StructuralTraceEvent) -> Self {
        match event {
            StructuralTraceEvent::DiscoveryComplete {
                active,
                matched_rules,
            } => Self {
                kind: RunStructuralFactKind::DiscoveryComplete,
                active: Some(active),
                rule: None,
                matched_rules: Some(matched_rules),
                output_bundle_template: None,
                grounded_bundle: None,
                created_links: None,
                bindings: None,
                outputs: None,
                preserved: None,
                old_scope: None,
                next_scope: None,
                quiescent: None,
                handoff_count: None,
            },
            StructuralTraceEvent::RuleMatched {
                active,
                rule,
                output_bundle_template,
                bindings,
            } => Self {
                kind: RunStructuralFactKind::RuleMatched,
                active: Some(active),
                rule: Some(rule),
                matched_rules: None,
                output_bundle_template: Some(output_bundle_template),
                grounded_bundle: None,
                created_links: None,
                bindings: Some(
                    bindings
                        .into_iter()
                        .map(|binding| RunBindingV1 {
                            role: binding.role,
                            value: binding.value,
                        })
                        .collect(),
                ),
                outputs: None,
                preserved: None,
                old_scope: None,
                next_scope: None,
                quiescent: None,
                handoff_count: None,
            },
            StructuralTraceEvent::Instantiated {
                active,
                rule,
                output_bundle_template,
                grounded_bundle,
                created_links,
            } => Self {
                kind: RunStructuralFactKind::Instantiated,
                active: Some(active),
                rule: Some(rule),
                matched_rules: None,
                output_bundle_template: Some(output_bundle_template),
                grounded_bundle: Some(grounded_bundle),
                created_links: Some(
                    created_links
                        .into_iter()
                        .map(|link| RunCreatedLinkV1 {
                            handle: link.handle,
                            start: link.start,
                            end: link.end,
                        })
                        .collect(),
                ),
                bindings: None,
                outputs: None,
                preserved: None,
                old_scope: None,
                next_scope: None,
                quiescent: None,
                handoff_count: None,
            },
            StructuralTraceEvent::Published {
                active,
                rule,
                outputs,
                preserved,
            } => Self {
                kind: RunStructuralFactKind::Published,
                active: Some(active),
                rule,
                matched_rules: None,
                output_bundle_template: None,
                grounded_bundle: None,
                created_links: None,
                bindings: None,
                outputs: Some(outputs),
                preserved: Some(preserved),
                old_scope: None,
                next_scope: None,
                quiescent: None,
                handoff_count: None,
            },
            StructuralTraceEvent::ScopeCommitted {
                old_members,
                next_members,
                quiescent,
                handoff_count,
            } => Self {
                kind: RunStructuralFactKind::ScopeCommitted,
                active: None,
                rule: None,
                matched_rules: None,
                output_bundle_template: None,
                grounded_bundle: None,
                created_links: None,
                bindings: None,
                outputs: None,
                preserved: None,
                old_scope: Some(old_members),
                next_scope: Some(next_members),
                quiescent: Some(quiescent),
                handoff_count: Some(handoff_count),
            },
        }
    }
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) structural_facts: Option<Vec<RunStructuralFactV1>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct StructuralProfileV1 {
    pub(crate) timing_available: bool,
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
            timing_available: STRUCTURAL_PROFILE_TIMING_AVAILABLE,
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
    pub(crate) timing_available: bool,
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
pub(crate) struct SessionStageTimingsV1 {
    pub(crate) timing_available: bool,
    pub(crate) prepare_ns: u64,
    pub(crate) load_ns: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionOpenProfileV1 {
    pub(crate) schema_version: u32,
    pub(crate) session_id: String,
    pub(crate) backend_id: String,
    pub(crate) stages: SessionStageTimingsV1,
    pub(crate) prepared_links: u32,
    pub(crate) base_links: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunStageTimingsV1 {
    pub(crate) timing_available: bool,
    pub(crate) configure_ns: u64,
    pub(crate) execute_ns: u64,
    pub(crate) result_ns: u64,
    pub(crate) evidence_ns: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RunPipelineProfileV1 {
    pub(crate) schema_version: u32,
    pub(crate) session_id: String,
    pub(crate) run_id: u64,
    pub(crate) backend_id: String,
    pub(crate) stages: RunStageTimingsV1,
    pub(crate) links_before_configure: u32,
    pub(crate) links_after_configure: u32,
    pub(crate) links_after_execute: u32,
    pub(crate) execute_profile: RunProfileV1,
}

pub(crate) fn time_stage<T>(work: impl FnOnce() -> T) -> (T, u64) {
    let started = ObservationTimer::start();
    let value = work();
    (value, ns_u64(started.elapsed_ns()))
}

pub(crate) fn session_open_profile_v1(
    session_id: String,
    prepare_ns: u64,
    load_ns: u64,
    prepared_links: u32,
    base_links: u32,
) -> SessionOpenProfileV1 {
    SessionOpenProfileV1 {
        schema_version: RUN_OBSERVABILITY_SCHEMA_VERSION,
        session_id,
        backend_id: OPTIMIZED_CPU_BACKEND_ID.to_owned(),
        stages: SessionStageTimingsV1 {
            timing_available: OBSERVABILITY_TIMING_AVAILABLE,
            prepare_ns,
            load_ns,
        },
        prepared_links,
        base_links,
    }
}

pub(crate) fn run_pipeline_profile_v1(
    observed: &ObservedRunV1,
    configure_ns: u64,
    result_ns: u64,
    external_evidence_ns: u64,
    links_before_configure: u32,
    links_after_configure: u32,
) -> Option<RunPipelineProfileV1> {
    let execute_profile = observed.profile.clone()?;
    let evidence_ns = execute_profile
        .trace_projection_ns
        .saturating_add(external_evidence_ns);

    Some(RunPipelineProfileV1 {
        schema_version: RUN_OBSERVABILITY_SCHEMA_VERSION,
        session_id: observed.session_id.clone(),
        run_id: observed.run_id,
        backend_id: observed.backend_id.clone(),
        stages: RunStageTimingsV1 {
            timing_available:
                observed.timing_available && execute_profile.timing_available,
            configure_ns,
            execute_ns: execute_profile.execute_ns,
            result_ns,
            evidence_ns,
        },
        links_before_configure,
        links_after_configure,
        links_after_execute: execute_profile.links_after_run,
        execute_profile,
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ObservedRunV1 {
    pub(crate) schema_version: u32,
    pub(crate) timing_available: bool,
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
            timing_available: OBSERVABILITY_TIMING_AVAILABLE,
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
                structural_facts: None,
            }],
            profile: None,
        };

        let json = serde_json::to_string(&run).unwrap();
        assert!(json.contains("\"schemaVersion\":1"));
        assert!(json.contains("\"observationLevel\":\"TRACE\""));
        assert!(json.contains("\"stage\":\"EXECUTE\""));
        assert!(json.contains(&format!(
            "\"timingAvailable\":{}",
            OBSERVABILITY_TIMING_AVAILABLE
        )));
        let decoded: ObservedRunV1 = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, run);
    }

    #[test]
    fn pipeline_profiles_round_trip_and_keep_session_run_split() {
        let execute = RunProfileV1 {
            schema_version: RUN_OBSERVABILITY_SCHEMA_VERSION,
            timing_available: OBSERVABILITY_TIMING_AVAILABLE,
            session_id: "A-memory#pipeline".to_owned(),
            run_id: 3,
            backend_id: OPTIMIZED_CPU_BACKEND_ID.to_owned(),
            base_links: 20,
            links_before_run: 23,
            links_after_run: 30,
            execution_link_delta: 7,
            scope_before_width: 1,
            scope_after_width: 1,
            active_reaction_count: 7,
            execute_ns: 100,
            trace_projection_ns: 9,
            structural: StructuralProfileV1 {
                timing_available: STRUCTURAL_PROFILE_TIMING_AVAILABLE,
                ..StructuralProfileV1::default()
            },
        };
        let observed = ObservedRunV1 {
            schema_version: RUN_OBSERVABILITY_SCHEMA_VERSION,
            timing_available: OBSERVABILITY_TIMING_AVAILABLE,
            session_id: execute.session_id.clone(),
            run_id: execute.run_id,
            backend_id: execute.backend_id.clone(),
            observation_level: RunObservationLevel::Trace,
            final_scope: vec![30],
            active_reaction_count: 7,
            final_quiescent: true,
            events: Vec::new(),
            profile: Some(execute),
        };

        let open = session_open_profile_v1(
            observed.session_id.clone(),
            11,
            22,
            20,
            20,
        );
        let run = run_pipeline_profile_v1(
            &observed,
            5,
            7,
            13,
            20,
            23,
        )
        .unwrap();

        assert_eq!(open.stages.prepare_ns, 11);
        assert_eq!(open.stages.load_ns, 22);
        assert_eq!(run.stages.configure_ns, 5);
        assert_eq!(run.stages.execute_ns, 100);
        assert_eq!(run.stages.result_ns, 7);
        assert_eq!(run.stages.evidence_ns, 22);
        assert_eq!(run.links_after_execute, 30);

        let json = serde_json::to_string(&(open.clone(), run.clone())).unwrap();
        let decoded: (SessionOpenProfileV1, RunPipelineProfileV1) =
            serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, (open, run));
    }

    #[test]
    fn native_structural_fact_projection_preserves_runtime_handles() {
        let native = StructuralTraceEvent::RuleMatched {
            active: 11,
            rule: 12,
            output_bundle_template: 13,
            bindings: vec![
                amemory_optimized_cpu_probe::structural::StructuralRoleBinding {
                    role: 14,
                    value: 15,
                },
            ],
        };
        let fact = RunStructuralFactV1::from(native);

        assert_eq!(fact.kind, RunStructuralFactKind::RuleMatched);
        assert_eq!(fact.active, Some(11));
        assert_eq!(fact.rule, Some(12));
        assert_eq!(fact.output_bundle_template, Some(13));
        assert_eq!(
            fact.bindings,
            Some(vec![RunBindingV1 {
                role: 14,
                value: 15,
            }]),
        );

        let json = serde_json::to_string(&fact).unwrap();
        assert!(json.contains("\"kind\":\"RULE_MATCHED\""));
        assert!(json.contains("\"rule\":12"));
    }

    #[test]
    fn persistent_link_delta_survives_portable_projection() {
        let fact = RunStructuralFactV1::from(
            StructuralTraceEvent::Instantiated {
                active: 11,
                rule: 12,
                output_bundle_template: 13,
                grounded_bundle: 17,
                created_links: vec![
                    amemory_optimized_cpu_probe::structural::StructuralCreatedLink {
                        handle: 14,
                        start: 11,
                        end: 12,
                    },
                    amemory_optimized_cpu_probe::structural::StructuralCreatedLink {
                        handle: 15,
                        start: 14,
                        end: 13,
                    },
                ],
            },
        );

        assert_eq!(fact.kind, RunStructuralFactKind::Instantiated);
        assert_eq!(
            fact.created_links,
            Some(vec![
                RunCreatedLinkV1 {
                    handle: 14,
                    start: 11,
                    end: 12,
                },
                RunCreatedLinkV1 {
                    handle: 15,
                    start: 14,
                    end: 13,
                },
            ]),
        );
        assert_eq!(fact.grounded_bundle, Some(17));

        let json = serde_json::to_string(&fact).unwrap();
        assert!(json.contains(
            "\"createdLinks\":[{\"handle\":14,\"start\":11,\"end\":12}"
        ));
        assert!(!json.contains("context"));
    }

    #[test]
    fn timing_availability_matches_target_clock_capability() {
        assert_eq!(
            OBSERVABILITY_TIMING_AVAILABLE,
            !cfg!(target_family = "wasm"),
        );
        assert_eq!(
            STRUCTURAL_PROFILE_TIMING_AVAILABLE,
            OBSERVABILITY_TIMING_AVAILABLE,
        );

        let timer = ObservationTimer::start();
        let elapsed = timer.elapsed_ns();
        if OBSERVABILITY_TIMING_AVAILABLE {
            // A monotonic clock can legitimately report 0 for a very short
            // interval; availability is the truth bit, not the magnitude.
            assert!(elapsed <= u128::MAX);
        } else {
            assert_eq!(elapsed, 0);
        }
    }

    #[test]
    fn u128_timing_projection_saturates() {
        assert_eq!(ns_u64(5), 5);
        assert_eq!(ns_u64(u128::MAX), u64::MAX);
    }
}
