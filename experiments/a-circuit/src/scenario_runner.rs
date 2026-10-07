use super::{
    observability::{
        project_cpu_session_step_v1, run_pipeline_profile_v1,
        session_open_profile_for_backend_v1, session_open_profile_v1,
        time_stage, CpuSessionReactionEvidenceV1, ObservationTimer,
        ObservedRunV1, RunObservationLevel, RunPipelineProfileV1,
        SessionOpenProfileV1, LINKSDB_BACKEND_ID,
    },
    proof_n::{
        execute_session_observed_to_quiescence,
        load_runtime_session, project_bounded_run_observation_v1,
        project_load_stage_from_structural_store_v1, WebProofLoadStage,
        WebProofPrepareStage,
    },
    runtime_session::{
        RunControllerV1, CpuRunResourceBudgetV1, CpuRuntimeSession,
    },
    session_contract::{
        BackendResourceAccountingV1, CapabilitySupportV1,
        RunBudgetAccountingV1, RunBudgetV1, RunStopReasonV1,
        RuntimeSessionV1, SessionCapabilitiesV1, SessionStateV1,
    },
    scenario::{
        validate_manifest_v1, ScenarioAssertionV1, ScenarioBackendV1,
        ScenarioExecutionModeV1, ScenarioManifestV1, ScenarioOraclePolicyV1,
        ScenarioProgramProfileV1, ScenarioRunV1, ScenarioValidationErrorV1,
    },
    scenario_cpu_adapter::{
        configure_mux1_store_from_inputs, project_mux1_store_result,
        resolve_cpu_scenario_adapter, CpuScenarioAdapter,
        CpuScenarioAdapterResolutionErrorV1, ScenarioNormalizedResultV1,
    },
};
#[cfg(test)]
use super::{
    logic_effect_n::prepare_logic32_session_program,
    mux_n::{configure_mux1_session, prepare_mux1_session_program},
};
#[cfg(not(target_family = "wasm"))]
use super::linksdb_session::{
    DoubletsPhysicalStoreV1, LinksDbSessionV1,
};
use amemory_optimized_cpu_probe::Handle;
#[cfg(not(target_family = "wasm"))]
use amemory_optimized_cpu_probe::{
    structural::StructuralReadV1, PackedCarrierImage,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub(crate) const SCENARIO_REPORT_SCHEMA_VERSION: u32 = 1;
const OPTIMIZED_CPU_BACKEND_ID: &str = "optimized-cpu";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioAssertionResultV1 {
    pub(crate) assertion_index: u32,
    pub(crate) kind: String,
    pub(crate) passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) expected: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) actual: Option<Value>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioRunReportV1 {
    pub(crate) manifest_run_id: String,
    pub(crate) session_run_id: u64,
    pub(crate) inputs: BTreeMap<String, Value>,
    pub(crate) configuration_reused: bool,
    pub(crate) links_before_configure: u32,
    pub(crate) links_after_configure: u32,
    pub(crate) result: ScenarioNormalizedResultV1,
    pub(crate) assertion_results: Vec<ScenarioAssertionResultV1>,
    // Compatibility alias for the historical fresh-instance comparison.
    // It is NOT an independent oracle; E3 must not present it as such.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) oracle_matches: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) fresh_instance_matches: Option<bool>,
    pub(crate) scalar_oracle_matches: bool,
    pub(crate) observed: ObservedRunV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) pipeline_profile: Option<RunPipelineProfileV1>,
}

#[derive(Clone, Debug)]
struct ScenarioActiveStepRunV1 {
    run: ScenarioRunV1,
    session_run_id: u64,
    controller: RunControllerV1,
    active_reaction_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioStepBeginV1 {
    pub(crate) schema_version: u32,
    pub(crate) manifest_run_id: String,
    pub(crate) session_run_id: u64,
    pub(crate) configuration_reused: bool,
    pub(crate) links_before_configure: u32,
    pub(crate) links_after_configure: u32,
    pub(crate) max_reactions: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioStepReportV1 {
    pub(crate) schema_version: u32,
    pub(crate) manifest_run_id: String,
    pub(crate) session_run_id: u64,
    pub(crate) evidence: CpuSessionReactionEvidenceV1,
    pub(crate) completed: bool,
    pub(crate) active_reaction_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) result: Option<ScenarioNormalizedResultV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) assertion_results: Vec<ScenarioAssertionResultV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) fresh_instance_matches: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) scalar_oracle_matches: Option<bool>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum ScenarioManifestFieldAuthorityV1 {
    Enforced,
    Advisory,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioManifestFieldSemanticsV1 {
    pub(crate) schema_version: u32,
    pub(crate) program_profile: ScenarioManifestFieldAuthorityV1,
    pub(crate) typed_inputs: ScenarioManifestFieldAuthorityV1,
    pub(crate) supported_backends: ScenarioManifestFieldAuthorityV1,
    pub(crate) execution_mode: ScenarioManifestFieldAuthorityV1,
    pub(crate) observation_level: ScenarioManifestFieldAuthorityV1,
    pub(crate) oracle_policy: ScenarioManifestFieldAuthorityV1,
    pub(crate) invariants: ScenarioManifestFieldAuthorityV1,
    pub(crate) profiling_policy: ScenarioManifestFieldAuthorityV1,
    pub(crate) visualization_profile: ScenarioManifestFieldAuthorityV1,
}

fn scenario_manifest_field_semantics_v1() -> ScenarioManifestFieldSemanticsV1 {
    ScenarioManifestFieldSemanticsV1 {
        schema_version: 1,
        program_profile: ScenarioManifestFieldAuthorityV1::Enforced,
        typed_inputs: ScenarioManifestFieldAuthorityV1::Enforced,
        supported_backends: ScenarioManifestFieldAuthorityV1::Enforced,
        execution_mode: ScenarioManifestFieldAuthorityV1::Enforced,
        observation_level: ScenarioManifestFieldAuthorityV1::Enforced,
        oracle_policy: ScenarioManifestFieldAuthorityV1::Enforced,
        invariants: ScenarioManifestFieldAuthorityV1::Advisory,
        profiling_policy: ScenarioManifestFieldAuthorityV1::Advisory,
        visualization_profile: ScenarioManifestFieldAuthorityV1::Advisory,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioProvenanceV1 {
    pub(crate) amemory_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) build_sha: Option<String>,
    pub(crate) scenario_version: String,
    pub(crate) program_profile_id: String,
    pub(crate) program_profile: ScenarioProgramProfileV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) program_fingerprint: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioExecutionReportV1 {
    pub(crate) schema_version: u32,
    pub(crate) scenario_id: String,
    pub(crate) scenario_version: String,
    pub(crate) program_profile: ScenarioProgramProfileV1,
    pub(crate) manifest_field_semantics: ScenarioManifestFieldSemanticsV1,
    pub(crate) backend: ScenarioBackendV1,
    pub(crate) session_id: String,
    pub(crate) session_open_profile: SessionOpenProfileV1,
    pub(crate) runs: Vec<ScenarioRunReportV1>,
    pub(crate) overall_pass: bool,
    pub(crate) provenance: ScenarioProvenanceV1,
}

pub(crate) const SCENARIO_DIFFERENTIAL_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioComparisonCheckV1 {
    pub(crate) path: String,
    pub(crate) matches: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) left: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) right: Option<Value>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum ScenarioDifferentialExclusionClassV1 {
    BackendIdentity,
    Timing,
    PhysicalResource,
    BackendLocalHandle,
    Provenance,
    LegacyAlias,
    EvidenceClass,
    TraceDetail,
    DerivedDuplicate,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioDifferentialExcludedFieldV1 {
    pub(crate) path: String,
    pub(crate) class: ScenarioDifferentialExclusionClassV1,
    pub(crate) reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioRunEvidenceSummaryV1 {
    pub(crate) manifest_run_id: String,
    pub(crate) assertion_results: Vec<ScenarioAssertionResultV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) fresh_instance_matches: Option<bool>,
    pub(crate) scalar_oracle_matches: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioEvidenceSummaryV1 {
    pub(crate) overall_pass: bool,
    pub(crate) runs: Vec<ScenarioRunEvidenceSummaryV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioDifferentialReportV1 {
    pub(crate) schema_version: u32,
    pub(crate) left_backend: ScenarioBackendV1,
    pub(crate) right_backend: ScenarioBackendV1,
    // Semantic/lifecycle correctness only. Never include backend physics here.
    pub(crate) portable_match: bool,
    pub(crate) portable_checks: Vec<ScenarioComparisonCheckV1>,
    // Shared implementation diagnostics. This is not semantic authority.
    pub(crate) diagnostic_match: bool,
    pub(crate) diagnostic_checks: Vec<ScenarioComparisonCheckV1>,
    pub(crate) excluded_fields: Vec<ScenarioDifferentialExcludedFieldV1>,
    pub(crate) left_evidence: ScenarioEvidenceSummaryV1,
    pub(crate) right_evidence: ScenarioEvidenceSummaryV1,
}

pub(crate) const SCENARIO_HOST_ENVELOPE_SCHEMA_VERSION: u32 = 1;
pub(crate) const SCENARIO_PORTABLE_DIFFERENTIAL_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioHostRunV1 {
    pub(crate) manifest_run_id: String,
    pub(crate) session_run_id: u64,
    pub(crate) inputs: BTreeMap<String, Value>,
    pub(crate) configuration_reused: bool,
    pub(crate) result: ScenarioNormalizedResultV1,
    pub(crate) active_reaction_count: u32,
    pub(crate) final_quiescent: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioHostEnvelopeV1 {
    pub(crate) schema_version: u32,
    pub(crate) backend: ScenarioBackendV1,
    pub(crate) program_fingerprint: String,
    pub(crate) runs: Vec<ScenarioHostRunV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioPortableRunV1 {
    pub(crate) manifest_run_id: String,
    pub(crate) session_run_id: u64,
    pub(crate) inputs: BTreeMap<String, Value>,
    pub(crate) configuration_reused: bool,
    pub(crate) result: ScenarioNormalizedResultV1,
    pub(crate) active_reaction_count: u32,
    pub(crate) final_quiescent: bool,
    pub(crate) assertion_results: Vec<ScenarioAssertionResultV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) fresh_instance_matches: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) scalar_oracle_matches: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioPortableReportV1 {
    pub(crate) schema_version: u32,
    pub(crate) scenario_id: String,
    pub(crate) scenario_version: String,
    pub(crate) program_profile: ScenarioProgramProfileV1,
    pub(crate) manifest_field_semantics: ScenarioManifestFieldSemanticsV1,
    pub(crate) backend: ScenarioBackendV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) program_fingerprint: Option<String>,
    pub(crate) runs: Vec<ScenarioPortableRunV1>,
    pub(crate) overall_pass: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioPortableRunEvidenceSummaryV1 {
    pub(crate) manifest_run_id: String,
    pub(crate) assertion_results: Vec<ScenarioAssertionResultV1>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) fresh_instance_matches: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) scalar_oracle_matches: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioPortableEvidenceSummaryV1 {
    pub(crate) overall_pass: bool,
    pub(crate) runs: Vec<ScenarioPortableRunEvidenceSummaryV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioPortableDifferentialReportV1 {
    pub(crate) schema_version: u32,
    pub(crate) left_backend: ScenarioBackendV1,
    pub(crate) right_backend: ScenarioBackendV1,
    pub(crate) portable_match: bool,
    pub(crate) portable_checks: Vec<ScenarioComparisonCheckV1>,
    pub(crate) left_evidence: ScenarioPortableEvidenceSummaryV1,
    pub(crate) right_evidence: ScenarioPortableEvidenceSummaryV1,
}

fn push_comparison_check_v1<T: PartialEq + Serialize>(
    checks: &mut Vec<ScenarioComparisonCheckV1>,
    path: impl Into<String>,
    left: &T,
    right: &T,
) {
    let matches = left == right;
    let (left_value, right_value) = if matches {
        (None, None)
    } else {
        (
            serde_json::to_value(left).ok(),
            serde_json::to_value(right).ok(),
        )
    };
    checks.push(ScenarioComparisonCheckV1 {
        path: path.into(),
        matches,
        left: left_value,
        right: right_value,
    });
}


pub(crate) fn scenario_portable_report_from_execution_v1(
    report: &ScenarioExecutionReportV1,
) -> ScenarioPortableReportV1 {
    ScenarioPortableReportV1 {
        schema_version: report.schema_version,
        scenario_id: report.scenario_id.clone(),
        scenario_version: report.scenario_version.clone(),
        program_profile: report.program_profile.clone(),
        manifest_field_semantics: report.manifest_field_semantics.clone(),
        backend: report.backend,
        program_fingerprint: report.provenance.program_fingerprint.clone(),
        runs: report
            .runs
            .iter()
            .map(|run| ScenarioPortableRunV1 {
                manifest_run_id: run.manifest_run_id.clone(),
                session_run_id: run.session_run_id,
                inputs: run.inputs.clone(),
                configuration_reused: run.configuration_reused,
                result: run.result.clone(),
                active_reaction_count: run.observed.active_reaction_count,
                final_quiescent: run.observed.final_quiescent,
                assertion_results: run.assertion_results.clone(),
                fresh_instance_matches: run.fresh_instance_matches,
                scalar_oracle_matches: Some(run.scalar_oracle_matches),
            })
            .collect(),
        overall_pass: report.overall_pass,
    }
}

pub(crate) fn scenario_portable_report_from_host_v1(
    manifest: &ScenarioManifestV1,
    host: &ScenarioHostEnvelopeV1,
) -> Result<ScenarioPortableReportV1, String> {
    if host.schema_version != SCENARIO_HOST_ENVELOPE_SCHEMA_VERSION {
        return Err(format!(
            "HOST_SCHEMA_VERSION: expected {}, got {}",
            SCENARIO_HOST_ENVELOPE_SCHEMA_VERSION,
            host.schema_version,
        ));
    }
    if host.backend != ScenarioBackendV1::Webgpu {
        return Err(format!(
            "HOST_BACKEND: external host envelope must be webgpu, got {:?}",
            host.backend,
        ));
    }
    if !manifest.supported_backends.contains(&host.backend) {
        return Err("HOST_BACKEND: manifest does not support webgpu".to_owned());
    }
    let fingerprint = host.program_fingerprint.as_str();
    let fingerprint_ok = fingerprint
        .strip_prefix("prepared-aset-fnv1a64-v1:")
        .map(|suffix| {
            suffix.len() == 16
                && suffix.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
        .unwrap_or(false);
    if !fingerprint_ok {
        return Err(
            "HOST_PROGRAM_FINGERPRINT: invalid prepared-Aset fingerprint"
                .to_owned(),
        );
    }
    if host.runs.len() != manifest.run_sequence.len() {
        return Err(format!(
            "HOST_RUN_COUNT: expected {}, got {}",
            manifest.run_sequence.len(),
            host.runs.len(),
        ));
    }

    let mut runs = Vec::with_capacity(host.runs.len());
    for (index, (expected, observed)) in manifest
        .run_sequence
        .iter()
        .zip(&host.runs)
        .enumerate()
    {
        if observed.manifest_run_id != expected.run_id {
            return Err(format!(
                "HOST_RUN_ID: run[{index}] expected {}, got {}",
                expected.run_id,
                observed.manifest_run_id,
            ));
        }
        let expected_session_run_id = (index as u64) + 1;
        if observed.session_run_id != expected_session_run_id {
            return Err(format!(
                "HOST_SESSION_RUN_ID: run[{index}] expected {}, got {}",
                expected_session_run_id,
                observed.session_run_id,
            ));
        }
        if observed.inputs != expected.inputs {
            return Err(format!(
                "HOST_INPUTS: run[{index}] inputs differ from manifest"
            ));
        }

        let assertion_results = evaluate_assertions_with_state(
            &expected.assertions,
            &observed.result,
            observed.final_quiescent,
            observed.active_reaction_count,
        );
        runs.push(ScenarioPortableRunV1 {
            manifest_run_id: observed.manifest_run_id.clone(),
            session_run_id: observed.session_run_id,
            inputs: observed.inputs.clone(),
            configuration_reused: observed.configuration_reused,
            result: observed.result.clone(),
            active_reaction_count: observed.active_reaction_count,
            final_quiescent: observed.final_quiescent,
            assertion_results,
            // D2 does not manufacture independent evidence that the browser
            // host did not actually run.
            fresh_instance_matches: None,
            scalar_oracle_matches: None,
        });
    }

    let overall_pass = runs.iter().all(|run| {
        run.assertion_results
            .iter()
            .all(|assertion| assertion.passed)
    });
    Ok(ScenarioPortableReportV1 {
        schema_version: SCENARIO_REPORT_SCHEMA_VERSION,
        scenario_id: manifest.scenario_id.clone(),
        scenario_version: manifest.scenario_version.clone(),
        program_profile: manifest.program_profile.clone(),
        manifest_field_semantics: scenario_manifest_field_semantics_v1(),
        backend: host.backend,
        program_fingerprint: Some(host.program_fingerprint.clone()),
        runs,
        overall_pass,
    })
}

fn scenario_portable_evidence_summary_v1(
    report: &ScenarioPortableReportV1,
) -> ScenarioPortableEvidenceSummaryV1 {
    ScenarioPortableEvidenceSummaryV1 {
        overall_pass: report.overall_pass,
        runs: report
            .runs
            .iter()
            .map(|run| ScenarioPortableRunEvidenceSummaryV1 {
                manifest_run_id: run.manifest_run_id.clone(),
                assertion_results: run.assertion_results.clone(),
                fresh_instance_matches: run.fresh_instance_matches,
                scalar_oracle_matches: run.scalar_oracle_matches,
            })
            .collect(),
    }
}

fn scenario_portable_checks_v1(
    left: &ScenarioPortableReportV1,
    right: &ScenarioPortableReportV1,
) -> Vec<ScenarioComparisonCheckV1> {
    let mut checks = Vec::new();
    push_comparison_check_v1(
        &mut checks,
        "schemaVersion",
        &left.schema_version,
        &right.schema_version,
    );
    push_comparison_check_v1(
        &mut checks,
        "scenarioId",
        &left.scenario_id,
        &right.scenario_id,
    );
    push_comparison_check_v1(
        &mut checks,
        "scenarioVersion",
        &left.scenario_version,
        &right.scenario_version,
    );
    push_comparison_check_v1(
        &mut checks,
        "programProfile",
        &left.program_profile,
        &right.program_profile,
    );
    push_comparison_check_v1(
        &mut checks,
        "manifestFieldSemantics",
        &left.manifest_field_semantics,
        &right.manifest_field_semantics,
    );
    push_comparison_check_v1(
        &mut checks,
        "provenance.programFingerprint",
        &left.program_fingerprint,
        &right.program_fingerprint,
    );
    push_comparison_check_v1(
        &mut checks,
        "runs.length",
        &left.runs.len(),
        &right.runs.len(),
    );

    for (index, (left_run, right_run)) in
        left.runs.iter().zip(&right.runs).enumerate()
    {
        let prefix = format!("runs[{index}]");
        push_comparison_check_v1(
            &mut checks,
            format!("{prefix}.manifestRunId"),
            &left_run.manifest_run_id,
            &right_run.manifest_run_id,
        );
        push_comparison_check_v1(
            &mut checks,
            format!("{prefix}.sessionRunId"),
            &left_run.session_run_id,
            &right_run.session_run_id,
        );
        push_comparison_check_v1(
            &mut checks,
            format!("{prefix}.inputs"),
            &left_run.inputs,
            &right_run.inputs,
        );
        push_comparison_check_v1(
            &mut checks,
            format!("{prefix}.configurationReused"),
            &left_run.configuration_reused,
            &right_run.configuration_reused,
        );
        push_comparison_check_v1(
            &mut checks,
            format!("{prefix}.result"),
            &left_run.result,
            &right_run.result,
        );
        push_comparison_check_v1(
            &mut checks,
            format!("{prefix}.observed.activeReactionCount"),
            &left_run.active_reaction_count,
            &right_run.active_reaction_count,
        );
        push_comparison_check_v1(
            &mut checks,
            format!("{prefix}.observed.finalQuiescent"),
            &left_run.final_quiescent,
            &right_run.final_quiescent,
        );
    }
    checks
}

pub(crate) fn compare_scenario_portable_reports_v1(
    left: &ScenarioPortableReportV1,
    right: &ScenarioPortableReportV1,
) -> ScenarioPortableDifferentialReportV1 {
    let portable_checks = scenario_portable_checks_v1(left, right);
    ScenarioPortableDifferentialReportV1 {
        schema_version: SCENARIO_PORTABLE_DIFFERENTIAL_SCHEMA_VERSION,
        left_backend: left.backend,
        right_backend: right.backend,
        portable_match: portable_checks.iter().all(|check| check.matches),
        portable_checks,
        left_evidence: scenario_portable_evidence_summary_v1(left),
        right_evidence: scenario_portable_evidence_summary_v1(right),
    }
}

fn scenario_evidence_summary_v1(
    report: &ScenarioExecutionReportV1,
) -> ScenarioEvidenceSummaryV1 {
    ScenarioEvidenceSummaryV1 {
        overall_pass: report.overall_pass,
        runs: report
            .runs
            .iter()
            .map(|run| ScenarioRunEvidenceSummaryV1 {
                manifest_run_id: run.manifest_run_id.clone(),
                assertion_results: run.assertion_results.clone(),
                fresh_instance_matches: run.fresh_instance_matches,
                scalar_oracle_matches: run.scalar_oracle_matches,
            })
            .collect(),
    }
}

fn scenario_differential_exclusions_v1(
) -> Vec<ScenarioDifferentialExcludedFieldV1> {
    use ScenarioDifferentialExclusionClassV1 as Class;

    [
        (
            "backend",
            Class::BackendIdentity,
            "backend identity is reported explicitly and is expected to differ",
        ),
        (
            "sessionId",
            Class::BackendIdentity,
            "Session identity is host-local",
        ),
        (
            "sessionOpenProfile.sessionId",
            Class::BackendIdentity,
            "Session identity is host-local",
        ),
        (
            "sessionOpenProfile.backendId",
            Class::BackendIdentity,
            "backend identity is host-local",
        ),
        (
            "sessionOpenProfile.stages",
            Class::Timing,
            "prepare/load wall-clock timing is not semantic equality",
        ),
        (
            "provenance.amemoryVersion",
            Class::Provenance,
            "build version identifies provenance, not Scenario semantics",
        ),
        (
            "provenance.buildSha",
            Class::Provenance,
            "build SHA identifies provenance, not Scenario semantics",
        ),
        (
            "runs[*].oracleMatches",
            Class::LegacyAlias,
            "historical fresh-instance compatibility alias is not independent evidence",
        ),
        (
            "runs[*].assertionResults",
            Class::EvidenceClass,
            "manifest assertions are reported separately from portable semantic equality",
        ),
        (
            "runs[*].freshInstanceMatches",
            Class::EvidenceClass,
            "fresh-instance lifecycle evidence is reported separately",
        ),
        (
            "runs[*].scalarOracleMatches",
            Class::EvidenceClass,
            "scalar oracle evidence is reported separately",
        ),
        (
            "runs[*].observed.sessionId",
            Class::BackendIdentity,
            "Session identity is host-local",
        ),
        (
            "runs[*].observed.backendId",
            Class::BackendIdentity,
            "backend identity is host-local",
        ),
        (
            "runs[*].observed.timingAvailable",
            Class::Timing,
            "timing availability is a host capability",
        ),
        (
            "runs[*].observed.finalScope",
            Class::BackendLocalHandle,
            "raw final Scope contains backend-local handles; compare only a future normalized structural projection",
        ),
        (
            "runs[*].observed.events",
            Class::TraceDetail,
            "raw trace events may contain backend-local handles and elapsed timing",
        ),
        (
            "runs[*].observed.profile.sessionId",
            Class::BackendIdentity,
            "Session identity is host-local",
        ),
        (
            "runs[*].observed.profile.backendId",
            Class::BackendIdentity,
            "backend identity is host-local",
        ),
        (
            "runs[*].observed.profile.timingAvailable",
            Class::Timing,
            "timing availability is a host capability",
        ),
        (
            "runs[*].observed.profile.executeNs",
            Class::Timing,
            "execute wall-clock timing is measured separately from correctness",
        ),
        (
            "runs[*].observed.profile.traceProjectionNs",
            Class::Timing,
            "trace projection timing is measured separately from correctness",
        ),
        (
            "runs[*].observed.profile.structural.*Ns",
            Class::Timing,
            "structural stage timings are measured separately from correctness",
        ),
        (
            "runs[*].observed.profile.denseCarrierAllocatedBytes",
            Class::PhysicalResource,
            "dense-carrier bytes are backend-local physical accounting",
        ),
        (
            "runs[*].observed.profile.maxDenseCarrierBytes",
            Class::PhysicalResource,
            "dense-carrier limits are backend-local physical accounting",
        ),
        (
            "runs[*].observed.profile.fullResidentBytesAvailable",
            Class::PhysicalResource,
            "resident-byte availability is a backend capability",
        ),
        (
            "runs[*].observed.profile.budgetAccounting.denseCarrierAllocatedBytes",
            Class::PhysicalResource,
            "dense-carrier bytes are backend-local physical accounting",
        ),
        (
            "runs[*].observed.profile.budgetAccounting.maxDenseCarrierBytes",
            Class::PhysicalResource,
            "dense-carrier limits are backend-local physical accounting",
        ),
        (
            "runs[*].observed.profile.budgetAccounting.fullResidentBytesAvailable",
            Class::PhysicalResource,
            "resident-byte availability is a backend capability",
        ),
        (
            "runs[*].pipelineProfile.sessionId",
            Class::BackendIdentity,
            "Session identity is host-local",
        ),
        (
            "runs[*].pipelineProfile.backendId",
            Class::BackendIdentity,
            "backend identity is host-local",
        ),
        (
            "runs[*].pipelineProfile.stages",
            Class::Timing,
            "pipeline stage timings are measured separately from correctness",
        ),
        (
            "runs[*].pipelineProfile.executeProfile",
            Class::DerivedDuplicate,
            "portable diagnostics are compared through observed.profile; this envelope also contains backend-local/timing fields",
        ),
    ]
    .into_iter()
    .map(|(path, class, reason)| ScenarioDifferentialExcludedFieldV1 {
        path: path.to_owned(),
        class,
        reason: reason.to_owned(),
    })
    .collect()
}

pub(crate) fn compare_scenario_execution_reports_v1(
    left: &ScenarioExecutionReportV1,
    right: &ScenarioExecutionReportV1,
) -> ScenarioDifferentialReportV1 {
    let left_portable = scenario_portable_report_from_execution_v1(left);
    let right_portable = scenario_portable_report_from_execution_v1(right);
    let mut portable_checks =
        scenario_portable_checks_v1(&left_portable, &right_portable);
    let mut diagnostic_checks = Vec::new();

    push_comparison_check_v1(
        &mut diagnostic_checks,
        "sessionOpenProfile.preparedLinks",
        &left.session_open_profile.prepared_links,
        &right.session_open_profile.prepared_links,
    );
    push_comparison_check_v1(
        &mut diagnostic_checks,
        "sessionOpenProfile.baseLinks",
        &left.session_open_profile.base_links,
        &right.session_open_profile.base_links,
    );

    for (index, (left_run, right_run)) in
        left.runs.iter().zip(&right.runs).enumerate()
    {
        let prefix = format!("runs[{index}]");
        push_comparison_check_v1(
            &mut diagnostic_checks,
            format!("{prefix}.linksBeforeConfigure"),
            &left_run.links_before_configure,
            &right_run.links_before_configure,
        );
        push_comparison_check_v1(
            &mut diagnostic_checks,
            format!("{prefix}.linksAfterConfigure"),
            &left_run.links_after_configure,
            &right_run.links_after_configure,
        );

        let left_profile_present = left_run.observed.profile.is_some();
        let right_profile_present = right_run.observed.profile.is_some();
        push_comparison_check_v1(
            &mut diagnostic_checks,
            format!("{prefix}.observed.profile.present"),
            &left_profile_present,
            &right_profile_present,
        );

        if let (Some(left_profile), Some(right_profile)) = (
            left_run.observed.profile.as_ref(),
            right_run.observed.profile.as_ref(),
        ) {
            push_comparison_check_v1(
                &mut diagnostic_checks,
                format!("{prefix}.observed.profile.baseLinks"),
                &left_profile.base_links,
                &right_profile.base_links,
            );
            push_comparison_check_v1(
                &mut diagnostic_checks,
                format!("{prefix}.observed.profile.linksBeforeRun"),
                &left_profile.links_before_run,
                &right_profile.links_before_run,
            );
            push_comparison_check_v1(
                &mut diagnostic_checks,
                format!("{prefix}.observed.profile.linksAfterRun"),
                &left_profile.links_after_run,
                &right_profile.links_after_run,
            );
            push_comparison_check_v1(
                &mut diagnostic_checks,
                format!("{prefix}.observed.profile.executionLinkDelta"),
                &left_profile.execution_link_delta,
                &right_profile.execution_link_delta,
            );
            push_comparison_check_v1(
                &mut diagnostic_checks,
                format!("{prefix}.observed.profile.scopeBeforeWidth"),
                &left_profile.scope_before_width,
                &right_profile.scope_before_width,
            );
            push_comparison_check_v1(
                &mut diagnostic_checks,
                format!("{prefix}.observed.profile.scopeAfterWidth"),
                &left_profile.scope_after_width,
                &right_profile.scope_after_width,
            );
            push_comparison_check_v1(
                &mut diagnostic_checks,
                format!("{prefix}.observed.profile.activeReactionCount"),
                &left_profile.active_reaction_count,
                &right_profile.active_reaction_count,
            );

            let mut left_structural = left_profile.structural.clone();
            let mut right_structural = right_profile.structural.clone();
            for profile in [&mut left_structural, &mut right_structural] {
                profile.timing_available = false;
                profile.discovery_ns = 0;
                profile.role_decode_ns = 0;
                profile.unification_ns = 0;
                profile.instantiation_ns = 0;
                profile.publication_ns = 0;
                profile.total_ns = 0;
            }
            push_comparison_check_v1(
                &mut diagnostic_checks,
                format!("{prefix}.observed.profile.structural.commonCounters"),
                &left_structural,
                &right_structural,
            );

            let left_budget_present = left_profile.budget_accounting.is_some();
            let right_budget_present = right_profile.budget_accounting.is_some();
            push_comparison_check_v1(
                &mut diagnostic_checks,
                format!("{prefix}.observed.profile.budgetAccounting.present"),
                &left_budget_present,
                &right_budget_present,
            );
            if let (Some(left_budget), Some(right_budget)) = (
                left_profile.budget_accounting.as_ref(),
                right_profile.budget_accounting.as_ref(),
            ) {
                let mut left_budget = left_budget.clone();
                let mut right_budget = right_budget.clone();
                for budget in [&mut left_budget, &mut right_budget] {
                    budget.dense_carrier_allocated_bytes = None;
                    budget.max_dense_carrier_bytes = None;
                    budget.full_resident_bytes_available = false;
                }
                push_comparison_check_v1(
                    &mut diagnostic_checks,
                    format!(
                        "{prefix}.observed.profile.budgetAccounting.commonCounters"
                    ),
                    &left_budget,
                    &right_budget,
                );
            }
        }

        let left_pipeline_present = left_run.pipeline_profile.is_some();
        let right_pipeline_present = right_run.pipeline_profile.is_some();
        push_comparison_check_v1(
            &mut diagnostic_checks,
            format!("{prefix}.pipelineProfile.present"),
            &left_pipeline_present,
            &right_pipeline_present,
        );
        if let (Some(left_pipeline), Some(right_pipeline)) = (
            left_run.pipeline_profile.as_ref(),
            right_run.pipeline_profile.as_ref(),
        ) {
            push_comparison_check_v1(
                &mut diagnostic_checks,
                format!("{prefix}.pipelineProfile.linksBeforeConfigure"),
                &left_pipeline.links_before_configure,
                &right_pipeline.links_before_configure,
            );
            push_comparison_check_v1(
                &mut diagnostic_checks,
                format!("{prefix}.pipelineProfile.linksAfterConfigure"),
                &left_pipeline.links_after_configure,
                &right_pipeline.links_after_configure,
            );
            push_comparison_check_v1(
                &mut diagnostic_checks,
                format!("{prefix}.pipelineProfile.linksAfterExecute"),
                &left_pipeline.links_after_execute,
                &right_pipeline.links_after_execute,
            );
        }
    }

    let portable_match =
        portable_checks.iter().all(|check| check.matches);
    let diagnostic_match =
        diagnostic_checks.iter().all(|check| check.matches);
    ScenarioDifferentialReportV1 {
        schema_version: SCENARIO_DIFFERENTIAL_SCHEMA_VERSION,
        left_backend: left.backend,
        right_backend: right.backend,
        portable_match,
        portable_checks,
        diagnostic_match,
        diagnostic_checks,
        excluded_fields: scenario_differential_exclusions_v1(),
        left_evidence: scenario_evidence_summary_v1(left),
        right_evidence: scenario_evidence_summary_v1(right),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum ScenarioRunnerErrorV1 {
    Validation {
        errors: Vec<ScenarioValidationErrorV1>,
    },
    UnsupportedBackend {
        backend: ScenarioBackendV1,
    },
    UnsupportedProgramProfile {
        profile_id: String,
    },
    ProgramProfileMismatch {
        requested: ScenarioProgramProfileV1,
        canonical: ScenarioProgramProfileV1,
    },
    UnsupportedExecutionMode {
        run_id: String,
        mode: ScenarioExecutionModeV1,
    },
    PrepareFailed {
        profile_id: String,
    },
    LoadFailed {
        profile_id: String,
    },
    ConfigureFailed {
        run_id: String,
        message: String,
    },
    ExecuteFailed {
        run_id: String,
        max_reactions: u32,
        stop_reason: RunStopReasonV1,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        budget_accounting: Option<RunBudgetAccountingV1>,
    },
    ProjectResultFailed {
        run_id: String,
        message: String,
    },
    ResultProjectionMutatedCarrier {
        run_id: String,
    },
    EvidenceProjectionMutatedCarrier {
        run_id: String,
    },
    LoadedBaseMutated {
        run_id: String,
    },
    OracleFailed {
        run_id: String,
        message: String,
    },
    StepRunAlreadyActive {
        run_id: String,
    },
    StepRunNotActive,
    StepControlFailed {
        run_id: String,
        message: String,
    },
}

const PREPARED_ASET_FINGERPRINT_ID: &str =
    "prepared-aset-fnv1a64-v1";
const FNV1A64_OFFSET: u64 = 0xcbf29ce484222325;
const FNV1A64_PRIME: u64 = 0x00000100000001b3;

fn fingerprint_mix_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in (bytes.len() as u64).to_le_bytes() {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(FNV1A64_PRIME);
    }
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FNV1A64_PRIME);
    }
}

fn fingerprint_mix_u32(hash: &mut u64, value: u32) {
    fingerprint_mix_bytes(hash, &value.to_le_bytes());
}

/// Deterministic structural identity for the exact static PREPARE image.
///
/// This is a versioned reproducibility fingerprint, not a cryptographic
/// signature. Source identity remains the exact build SHA in provenance.
pub(crate) fn prepared_aset_fingerprint_v1(
    prepare: &WebProofPrepareStage,
) -> String {
    let mut hash = FNV1A64_OFFSET;
    fingerprint_mix_bytes(
        &mut hash,
        PREPARED_ASET_FINGERPRINT_ID.as_bytes(),
    );
    fingerprint_mix_u32(&mut hash, prepare.compiled_links);

    for duplet in &prepare.carrier_duplets {
        fingerprint_mix_u32(&mut hash, duplet.start);
        fingerprint_mix_u32(&mut hash, duplet.end);
    }

    // Semantic root vector order is host presentation, not program identity.
    // Canonicalize the mapping before hashing it.
    let mut roots = prepare.semantic_roots.iter().collect::<Vec<_>>();
    roots.sort_by(|left, right| {
        left.role
            .cmp(&right.role)
            .then(left.carrier_ref.cmp(&right.carrier_ref))
    });
    for root in roots {
        fingerprint_mix_bytes(&mut hash, root.role.as_bytes());
        fingerprint_mix_u32(&mut hash, root.carrier_ref);
        fingerprint_mix_bytes(&mut hash, root.source.as_bytes());
    }

    let mut admissions = prepare.theory_admissions.iter().collect::<Vec<_>>();
    admissions.sort();
    for admission in admissions {
        fingerprint_mix_bytes(&mut hash, admission.as_bytes());
    }

    format!("{PREPARED_ASET_FINGERPRINT_ID}:{hash:016x}")
}

#[cfg(not(target_family = "wasm"))]
struct ScenarioLinksDbSessionV1 {
    manifest: ScenarioManifestV1,
    program_profile: ScenarioProgramProfileV1,
    adapter: &'static CpuScenarioAdapter,
    session: LinksDbSessionV1<DoubletsPhysicalStoreV1>,
    load: WebProofLoadStage,
    loaded_prefix: Vec<(Handle, Handle)>,
    session_open_profile: SessionOpenProfileV1,
    program_fingerprint: String,
    completed_runs: u64,
}

pub(crate) struct ScenarioCpuSessionV1 {
    manifest: ScenarioManifestV1,
    program_profile: ScenarioProgramProfileV1,
    adapter: &'static CpuScenarioAdapter,
    session: CpuRuntimeSession,
    load: WebProofLoadStage,
    loaded_link_count: usize,
    loaded_prefix: Vec<(Handle, Handle)>,
    pub(crate) session_open_profile: SessionOpenProfileV1,
    pub(crate) program_fingerprint: String,
    store_instance_id: String,
    engine_instance_id: String,
    completed_runs: u64,
    active_step_run: Option<ScenarioActiveStepRunV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioLiveSessionStatusV1 {
    pub(crate) schema_version: u32,
    pub(crate) backend: ScenarioBackendV1,
    pub(crate) session_id: String,
    pub(crate) memory_instance_id: String,
    pub(crate) runtime_state: SessionStateV1,
    pub(crate) runtime_capabilities: SessionCapabilitiesV1,
    pub(crate) store_instance_id: String,
    pub(crate) engine_instance_id: String,
    pub(crate) program_profile: ScenarioProgramProfileV1,
    pub(crate) manifest_field_semantics: ScenarioManifestFieldSemanticsV1,
    pub(crate) program_fingerprint: String,
    pub(crate) base_link_count: u32,
    pub(crate) current_link_count: u32,
    pub(crate) completed_runs: u64,
    pub(crate) prepare_count: u32,
    pub(crate) load_count: u32,
    pub(crate) session_open_profile: SessionOpenProfileV1,
}

impl ScenarioCpuSessionV1 {
    pub(crate) fn status_v1(&self) -> ScenarioLiveSessionStatusV1 {
        let runtime = self.session.runtime_snapshot_v1();
        ScenarioLiveSessionStatusV1 {
            schema_version: SCENARIO_REPORT_SCHEMA_VERSION,
            backend: ScenarioBackendV1::OptimizedCpu,
            session_id: runtime.identity.session_id,
            memory_instance_id: runtime.identity.memory_instance_id,
            runtime_state: runtime.state,
            runtime_capabilities: runtime.capabilities,
            store_instance_id: self.store_instance_id.clone(),
            engine_instance_id: self.engine_instance_id.clone(),
            program_profile: self.program_profile.clone(),
            manifest_field_semantics: scenario_manifest_field_semantics_v1(),
            program_fingerprint: self.program_fingerprint.clone(),
            base_link_count: runtime.base_link_count,
            current_link_count: runtime.current_link_count,
            completed_runs: self.completed_runs,
            prepare_count: 1,
            load_count: 1,
            session_open_profile: self.session_open_profile.clone(),
        }
    }
}

fn resolve_program_adapter_v1(
    requested: &ScenarioProgramProfileV1,
) -> Result<&'static CpuScenarioAdapter, ScenarioRunnerErrorV1> {
    resolve_cpu_scenario_adapter(requested).map_err(|error| match error {
        CpuScenarioAdapterResolutionErrorV1::UnsupportedProgramProfile {
            profile_id,
        } => ScenarioRunnerErrorV1::UnsupportedProgramProfile {
            profile_id,
        },
        CpuScenarioAdapterResolutionErrorV1::ProgramProfileMismatch {
            requested,
            canonical,
        } => ScenarioRunnerErrorV1::ProgramProfileMismatch {
            requested,
            canonical,
        },
    })
}

pub(crate) fn open_cpu_scenario_session_v1(
    manifest: &ScenarioManifestV1,
) -> Result<ScenarioCpuSessionV1, ScenarioRunnerErrorV1> {
    let validation = validate_manifest_v1(manifest);
    if !validation.is_empty() {
        return Err(ScenarioRunnerErrorV1::Validation {
            errors: validation,
        });
    }

    if !manifest
        .supported_backends
        .contains(&ScenarioBackendV1::OptimizedCpu)
    {
        return Err(ScenarioRunnerErrorV1::UnsupportedBackend {
            backend: ScenarioBackendV1::OptimizedCpu,
        });
    }

    // Resolve the complete descriptor before PREPARE. The adapter registry,
    // not caller-provided provenance text, is authority for the built-in
    // program that will actually be loaded.
    let adapter = resolve_program_adapter_v1(&manifest.program_profile)?;
    let program_profile = adapter.canonical_program_profile();

    let (prepare, prepare_ns) = time_stage(|| adapter.prepare());
    let prepare = prepare.ok_or_else(|| {
        ScenarioRunnerErrorV1::PrepareFailed {
            profile_id: manifest.program_profile.profile_id.clone(),
        }
    })?;
    let prepared_links = prepare.compiled_links;
    let program_fingerprint = prepared_aset_fingerprint_v1(&prepare);

    let (loaded, load_ns) =
        time_stage(|| load_runtime_session(&prepare, 32));
    let (session, load) = loaded.ok_or_else(|| {
        ScenarioRunnerErrorV1::LoadFailed {
            profile_id: manifest.program_profile.profile_id.clone(),
        }
    })?;

    let session_open_profile = session_open_profile_v1(
        session.id.clone(),
        prepare_ns,
        load_ns,
        prepared_links,
        session.base_link_count as u32,
    );
    let loaded_link_count = session.base_link_count;
    let loaded_prefix = session.memory.store.export_packed_duplets();
    let store_instance_id = session.memory.id.clone();
    let engine_instance_id = format!(
        "{}:optimized-structural-engine",
        session.memory.id
    );

    Ok(ScenarioCpuSessionV1 {
        manifest: manifest.clone(),
        program_profile,
        adapter,
        session,
        load,
        loaded_link_count,
        loaded_prefix,
        session_open_profile,
        program_fingerprint,
        store_instance_id,
        engine_instance_id,
        completed_runs: 0,
        active_step_run: None,
    })
}

#[cfg(not(target_family = "wasm"))]
fn open_linksdb_scenario_session_v1(
    manifest: &ScenarioManifestV1,
) -> Result<ScenarioLinksDbSessionV1, ScenarioRunnerErrorV1> {
    let validation = validate_manifest_v1(manifest);
    if !validation.is_empty() {
        return Err(ScenarioRunnerErrorV1::Validation {
            errors: validation,
        });
    }

    if !manifest
        .supported_backends
        .contains(&ScenarioBackendV1::Linksdb)
    {
        return Err(ScenarioRunnerErrorV1::UnsupportedBackend {
            backend: ScenarioBackendV1::Linksdb,
        });
    }

    let adapter = resolve_program_adapter_v1(&manifest.program_profile)?;
    let program_profile = adapter.canonical_program_profile();
    if program_profile.profile_id != "a-circuit:mux1" {
        return Err(ScenarioRunnerErrorV1::UnsupportedProgramProfile {
            profile_id: program_profile.profile_id,
        });
    }

    let (prepare, prepare_ns) = time_stage(|| adapter.prepare());
    let prepare = prepare.ok_or_else(|| {
        ScenarioRunnerErrorV1::PrepareFailed {
            profile_id: manifest.program_profile.profile_id.clone(),
        }
    })?;
    let prepared_links = prepare.compiled_links;
    let program_fingerprint = prepared_aset_fingerprint_v1(&prepare);
    let carrier = prepare
        .carrier_duplets
        .iter()
        .map(|duplet| (duplet.start, duplet.end))
        .collect::<Vec<_>>();
    let image = PackedCarrierImage::from_duplets(&carrier).map_err(|_| {
        ScenarioRunnerErrorV1::LoadFailed {
            profile_id: manifest.program_profile.profile_id.clone(),
        }
    })?;
    let interpreter = prepare
        .semantic_roots
        .iter()
        .find(|root| root.role == "execution.interpreter")
        .map(|root| root.carrier_ref)
        .ok_or_else(|| ScenarioRunnerErrorV1::LoadFailed {
            profile_id: manifest.program_profile.profile_id.clone(),
        })?;

    let (loaded, load_ns) = time_stage(|| {
        let store =
            DoubletsPhysicalStoreV1::from_packed_carrier_v1(&image).ok()?;
        let session =
            LinksDbSessionV1::open_structural(store, interpreter, 32).ok()?;
        let runtime = session.runtime_snapshot_v1();
        let load = project_load_stage_from_structural_store_v1(
            &prepare,
            session.physical_store_v1(),
            runtime.identity.memory_instance_id,
            1,
            runtime.base_link_count,
        )?;
        load.carrier_round_trip.then_some((session, load))
    });
    let (session, load) = loaded.ok_or_else(|| {
        ScenarioRunnerErrorV1::LoadFailed {
            profile_id: manifest.program_profile.profile_id.clone(),
        }
    })?;

    let runtime = session.runtime_snapshot_v1();
    let session_open_profile = session_open_profile_for_backend_v1(
        runtime.identity.session_id,
        LINKSDB_BACKEND_ID,
        prepare_ns,
        load_ns,
        prepared_links,
        runtime.base_link_count,
    );

    Ok(ScenarioLinksDbSessionV1 {
        manifest: manifest.clone(),
        program_profile,
        adapter,
        session,
        load,
        loaded_prefix: carrier,
        session_open_profile,
        program_fingerprint,
        completed_runs: 0,
    })
}

#[cfg(not(target_family = "wasm"))]
fn ensure_linksdb_session_configurable(
    live: &ScenarioLinksDbSessionV1,
    run_id: &str,
) -> Result<(), ScenarioRunnerErrorV1> {
    if matches!(
        live.session.runtime_state_v1(),
        SessionStateV1::Open | SessionStateV1::Quiescent
    ) {
        return Ok(());
    }

    Err(ScenarioRunnerErrorV1::StepControlFailed {
        run_id: run_id.to_owned(),
        message: format!(
            "LinksDB Session is not configurable in state {:?}",
            live.session.runtime_state_v1(),
        ),
    })
}

#[cfg(not(target_family = "wasm"))]
fn linksdb_loaded_prefix_matches_v1(
    live: &ScenarioLinksDbSessionV1,
) -> bool {
    live.loaded_prefix
        .iter()
        .enumerate()
        .all(|(index, &(start, end))| {
            let Ok(handle) = u32::try_from(index + 1) else {
                return false;
            };
            live.session
                .physical_store_v1()
                .poles(handle)
                .is_ok_and(|poles| poles == (start, end))
        })
}

fn ensure_cpu_session_configurable(
    live: &ScenarioCpuSessionV1,
    run_id: &str,
) -> Result<(), ScenarioRunnerErrorV1> {
    if matches!(
        live.session.runtime_state_v1(),
        SessionStateV1::Open
            | SessionStateV1::Quiescent
    ) {
        return Ok(());
    }

    Err(ScenarioRunnerErrorV1::StepControlFailed {
        run_id: run_id.to_owned(),
        message: format!(
            "Session is not configurable in state {:?}",
            live.session.runtime_state_v1(),
        ),
    })
}

pub(crate) fn run_cpu_scenario_session_once_v1(
    live: &mut ScenarioCpuSessionV1,
    run: &ScenarioRunV1,
) -> Result<ScenarioRunReportV1, ScenarioRunnerErrorV1> {
    if let Some(active) = live.active_step_run.as_ref() {
        return Err(ScenarioRunnerErrorV1::StepRunAlreadyActive {
            run_id: active.run.run_id.clone(),
        });
    }

    let mut validation_manifest = live.manifest.clone();
    validation_manifest.run_sequence = vec![run.clone()];
    let validation = validate_manifest_v1(&validation_manifest);
    if !validation.is_empty() {
        return Err(ScenarioRunnerErrorV1::Validation {
            errors: validation,
        });
    }

    if run.execution_mode != ScenarioExecutionModeV1::ToQuiescence {
        return Err(ScenarioRunnerErrorV1::UnsupportedExecutionMode {
            run_id: run.run_id.clone(),
            mode: run.execution_mode,
        });
    }
    ensure_cpu_session_configurable(live, &run.run_id)?;

    let adapter = live.adapter;
    let (configured, configure_ns) =
        time_stage(|| adapter.configure(
            &mut live.session,
            &live.load,
            &run.inputs,
        ));
    let configured = configured.map_err(|message| {
        ScenarioRunnerErrorV1::ConfigureFailed {
            run_id: run.run_id.clone(),
            message,
        }
    })?;

    let observed = execute_session_observed_to_quiescence(
        &mut live.session,
        configured.initial,
        run.max_reactions,
        live.manifest.observation_level,
    )
    .map_err(|stop| ScenarioRunnerErrorV1::ExecuteFailed {
        run_id: run.run_id.clone(),
        max_reactions: run.max_reactions,
        stop_reason: stop.stop_reason,
        budget_accounting: stop.budget_accounting,
    })?;

    let links_before_result = live.session.memory.store.link_count();
    let (projected, result_ns) =
        time_stage(|| adapter.project(&live.session, &live.load));
    let result = projected.map_err(|message| {
        ScenarioRunnerErrorV1::ProjectResultFailed {
            run_id: run.run_id.clone(),
            message,
        }
    })?;
    if live.session.memory.store.link_count() != links_before_result {
        return Err(
            ScenarioRunnerErrorV1::ResultProjectionMutatedCarrier {
                run_id: run.run_id.clone(),
            },
        );
    }

    let assertion_results =
        evaluate_assertions(&run.assertions, &result, &observed);

    // Independent scalar semantics see only typed Scenario inputs and
    // ordinary host integers. They do not call the structural executor,
    // inspect the Link Store, consume trace data, or reproduce recursive wire.
    let scalar_oracle =
        adapter.scalar_oracle(&run.inputs).map_err(|message| {
            ScenarioRunnerErrorV1::OracleFailed {
                run_id: run.run_id.clone(),
                message,
            }
        })?;
    let scalar_oracle_matches = scalar_oracle == result.fields;

    // A fresh instance of the same structural implementation is still useful
    // for detecting retained-session leakage, but is a separate evidence
    // class and must not be called an independent semantic oracle.
    let fresh_instance_matches = match live.manifest.oracle_policy {
        ScenarioOraclePolicyV1::FreshInstance => {
            let fresh =
                adapter.fresh_instance(&run.inputs).map_err(|message| {
                    ScenarioRunnerErrorV1::OracleFailed {
                        run_id: run.run_id.clone(),
                        message,
                    }
                })?;
            Some(fresh == result)
        }
        ScenarioOraclePolicyV1::None
        | ScenarioOraclePolicyV1::ExpectedAssertions => None,
    };
    let oracle_matches = fresh_instance_matches;

    let links_before_evidence = live.session.memory.store.link_count();
    let (_, external_evidence_ns) = time_stage(|| {
        serde_json::to_string(&(&observed, &result, &assertion_results))
            .expect("serializable scenario evidence")
    });
    if live.session.memory.store.link_count() != links_before_evidence {
        return Err(
            ScenarioRunnerErrorV1::EvidenceProjectionMutatedCarrier {
                run_id: run.run_id.clone(),
            },
        );
    }

    let pipeline_profile = run_pipeline_profile_v1(
        &observed,
        configure_ns,
        result_ns,
        external_evidence_ns,
        configured.links_before,
        configured.links_after,
    );

    let carrier = live.session.memory.store.export_packed_duplets();
    if carrier.len() < live.loaded_link_count
        || &carrier[..live.loaded_link_count]
            != live.loaded_prefix.as_slice()
    {
        return Err(ScenarioRunnerErrorV1::LoadedBaseMutated {
            run_id: run.run_id.clone(),
        });
    }

    let session_run_id = observed.run_id;
    let report = ScenarioRunReportV1 {
        manifest_run_id: run.run_id.clone(),
        session_run_id,
        inputs: run.inputs.clone(),
        configuration_reused:
            configured.links_after == configured.links_before,
        links_before_configure: configured.links_before,
        links_after_configure: configured.links_after,
        result,
        assertion_results,
        oracle_matches,
        fresh_instance_matches,
        scalar_oracle_matches,
        observed,
        pipeline_profile,
    };
    live.completed_runs = session_run_id;
    Ok(report)
}

#[cfg(not(target_family = "wasm"))]
fn fresh_linksdb_mux1_result_v1(
    manifest: &ScenarioManifestV1,
    run: &ScenarioRunV1,
) -> Result<ScenarioNormalizedResultV1, String> {
    let mut fresh_manifest = manifest.clone();
    fresh_manifest.oracle_policy = ScenarioOraclePolicyV1::None;
    fresh_manifest.run_sequence = vec![run.clone()];

    let report = run_linksdb_scenario_manifest_v1(&fresh_manifest)
        .map_err(|error| {
            format!("fresh LinksDB Scenario execution failed: {error:?}")
        })?;
    if report.backend != ScenarioBackendV1::Linksdb
        || report.runs.len() != 1
    {
        return Err(
            "fresh LinksDB Scenario used an unexpected backend/report shape"
                .to_owned(),
        );
    }

    Ok(report.runs[0].result.clone())
}

#[cfg(not(target_family = "wasm"))]
fn run_linksdb_scenario_session_once_v1(
    live: &mut ScenarioLinksDbSessionV1,
    run: &ScenarioRunV1,
) -> Result<ScenarioRunReportV1, ScenarioRunnerErrorV1> {
    let mut validation_manifest = live.manifest.clone();
    validation_manifest.run_sequence = vec![run.clone()];
    let validation = validate_manifest_v1(&validation_manifest);
    if !validation.is_empty() {
        return Err(ScenarioRunnerErrorV1::Validation {
            errors: validation,
        });
    }

    if run.execution_mode != ScenarioExecutionModeV1::ToQuiescence {
        return Err(ScenarioRunnerErrorV1::UnsupportedExecutionMode {
            run_id: run.run_id.clone(),
            mode: run.execution_mode,
        });
    }
    ensure_linksdb_session_configurable(live, &run.run_id)?;

    let (configured, configure_ns) = time_stage(|| {
        configure_mux1_store_from_inputs(
            live.session.physical_store_mut_v1(),
            &live.load,
            &run.inputs,
        )
    });
    let configured = configured.map_err(|message| {
        ScenarioRunnerErrorV1::ConfigureFailed {
            run_id: run.run_id.clone(),
            message,
        }
    })?;

    let identity = live.session.runtime_identity_v1();
    let base_links = live.session.runtime_base_link_count_v1();
    let run_started = ObservationTimer::start();
    let bounded = live
        .session
        .run_to_quiescence(
            configured.initial,
            RunBudgetV1::scenario_default(run.max_reactions),
            live.manifest.observation_level.runtime_trace_mode(),
        )
        .map_err(|stop_reason| ScenarioRunnerErrorV1::ExecuteFailed {
            run_id: run.run_id.clone(),
            max_reactions: run.max_reactions,
            stop_reason,
            budget_accounting: None,
        })?;
    if bounded.stop_reason != RunStopReasonV1::Quiescent {
        return Err(ScenarioRunnerErrorV1::ExecuteFailed {
            run_id: run.run_id.clone(),
            max_reactions: run.max_reactions,
            stop_reason: bounded.stop_reason,
            budget_accounting: Some(bounded.budget_accounting),
        });
    }

    let final_scope = live
        .session
        .current_scope_v1()
        .ok_or_else(|| ScenarioRunnerErrorV1::ProjectResultFailed {
            run_id: run.run_id.clone(),
            message: "LinksDB Session lost structural engine".to_owned(),
        })?
        .to_vec();
    let observed = project_bounded_run_observation_v1(
        identity.session_id,
        LINKSDB_BACKEND_ID,
        base_links,
        final_scope.clone(),
        live.manifest.observation_level,
        &run_started,
        bounded,
    );

    let links_before_result =
        live.session.runtime_current_link_count_v1();
    let (projected, result_ns) = time_stage(|| {
        project_mux1_store_result(
            live.session.physical_store_v1(),
            &final_scope,
            &live.load,
        )
    });
    let result = projected.map_err(|message| {
        ScenarioRunnerErrorV1::ProjectResultFailed {
            run_id: run.run_id.clone(),
            message,
        }
    })?;
    if live.session.runtime_current_link_count_v1()
        != links_before_result
    {
        return Err(
            ScenarioRunnerErrorV1::ResultProjectionMutatedCarrier {
                run_id: run.run_id.clone(),
            },
        );
    }

    let assertion_results =
        evaluate_assertions(&run.assertions, &result, &observed);

    let scalar_oracle =
        live.adapter.scalar_oracle(&run.inputs).map_err(|message| {
            ScenarioRunnerErrorV1::OracleFailed {
                run_id: run.run_id.clone(),
                message,
            }
        })?;
    let scalar_oracle_matches = scalar_oracle == result.fields;

    let fresh_instance_matches = match live.manifest.oracle_policy {
        ScenarioOraclePolicyV1::FreshInstance => {
            let fresh = fresh_linksdb_mux1_result_v1(
                &live.manifest,
                run,
            )
            .map_err(|message| ScenarioRunnerErrorV1::OracleFailed {
                run_id: run.run_id.clone(),
                message,
            })?;
            Some(fresh == result)
        }
        ScenarioOraclePolicyV1::None
        | ScenarioOraclePolicyV1::ExpectedAssertions => None,
    };
    let oracle_matches = fresh_instance_matches;

    let links_before_evidence =
        live.session.runtime_current_link_count_v1();
    let (_, external_evidence_ns) = time_stage(|| {
        serde_json::to_string(&(&observed, &result, &assertion_results))
            .expect("serializable LinksDB scenario evidence")
    });
    if live.session.runtime_current_link_count_v1()
        != links_before_evidence
    {
        return Err(
            ScenarioRunnerErrorV1::EvidenceProjectionMutatedCarrier {
                run_id: run.run_id.clone(),
            },
        );
    }

    let pipeline_profile = run_pipeline_profile_v1(
        &observed,
        configure_ns,
        result_ns,
        external_evidence_ns,
        configured.links_before,
        configured.links_after,
    );

    if !linksdb_loaded_prefix_matches_v1(live) {
        return Err(ScenarioRunnerErrorV1::LoadedBaseMutated {
            run_id: run.run_id.clone(),
        });
    }

    let session_run_id = observed.run_id;
    let report = ScenarioRunReportV1 {
        manifest_run_id: run.run_id.clone(),
        session_run_id,
        inputs: run.inputs.clone(),
        configuration_reused:
            configured.links_after == configured.links_before,
        links_before_configure: configured.links_before,
        links_after_configure: configured.links_after,
        result,
        assertion_results,
        oracle_matches,
        fresh_instance_matches,
        scalar_oracle_matches,
        observed,
        pipeline_profile,
    };
    live.completed_runs = session_run_id;
    Ok(report)
}

pub(crate) fn begin_cpu_scenario_step_run_v1(
    live: &mut ScenarioCpuSessionV1,
    run: &ScenarioRunV1,
) -> Result<ScenarioStepBeginV1, ScenarioRunnerErrorV1> {
    if let Some(active) = live.active_step_run.as_ref() {
        return Err(ScenarioRunnerErrorV1::StepRunAlreadyActive {
            run_id: active.run.run_id.clone(),
        });
    }

    let mut validation_manifest = live.manifest.clone();
    validation_manifest.run_sequence = vec![run.clone()];
    let validation = validate_manifest_v1(&validation_manifest);
    if !validation.is_empty() {
        return Err(ScenarioRunnerErrorV1::Validation {
            errors: validation,
        });
    }
    if run.execution_mode != ScenarioExecutionModeV1::Step {
        return Err(ScenarioRunnerErrorV1::UnsupportedExecutionMode {
            run_id: run.run_id.clone(),
            mode: run.execution_mode,
        });
    }
    ensure_cpu_session_configurable(live, &run.run_id)?;

    let configured = live.adapter.configure(
        &mut live.session,
        &live.load,
        &run.inputs,
    )
    .map_err(|message| ScenarioRunnerErrorV1::ConfigureFailed {
        run_id: run.run_id.clone(),
        message,
    })?;

    let controller = live
        .session
        .begin_budgeted_run(
            configured.initial,
            RunBudgetV1::scenario_default(run.max_reactions),
        )
        .map_err(|error| ScenarioRunnerErrorV1::StepControlFailed {
            run_id: run.run_id.clone(),
            message: format!("{error:?}"),
        })?;
    let session_run_id = controller.run_id();

    let begin = ScenarioStepBeginV1 {
        schema_version: SCENARIO_REPORT_SCHEMA_VERSION,
        manifest_run_id: run.run_id.clone(),
        session_run_id,
        configuration_reused:
            configured.links_after == configured.links_before,
        links_before_configure: configured.links_before,
        links_after_configure: configured.links_after,
        max_reactions: run.max_reactions,
    };
    live.active_step_run = Some(ScenarioActiveStepRunV1 {
        run: run.clone(),
        session_run_id,
        controller,
        active_reaction_count: 0,
    });
    Ok(begin)
}

pub(crate) fn step_cpu_scenario_session_v1(
    live: &mut ScenarioCpuSessionV1,
) -> Result<ScenarioStepReportV1, ScenarioRunnerErrorV1> {
    let mut active = live
        .active_step_run
        .take()
        .ok_or(ScenarioRunnerErrorV1::StepRunNotActive)?;

    let controlled = active
        .controller
        .next(
            &mut live.session,
            live.manifest.observation_level.runtime_trace_mode(),
        );

    if let Some(stop_reason) = controlled.stop_reason {
        if stop_reason != RunStopReasonV1::Quiescent {
            return Err(ScenarioRunnerErrorV1::ExecuteFailed {
                run_id: active.run.run_id,
                max_reactions: active.run.max_reactions,
                stop_reason,
                budget_accounting: Some(controlled.budget_accounting),
            });
        }
    }

    let step = controlled.step.ok_or_else(|| {
        ScenarioRunnerErrorV1::StepControlFailed {
            run_id: active.run.run_id.clone(),
            message: "runtime budget controller stopped without step evidence"
                .to_owned(),
        }
    })?;
    let projected = project_cpu_session_step_v1(
        step,
        live.manifest.observation_level,
    );
    if !projected.evidence.quiescent {
        active.active_reaction_count =
            active.active_reaction_count.saturating_add(1);
    }

    let completed =
        controlled.stop_reason == Some(RunStopReasonV1::Quiescent);
    let evidence = projected.evidence;
    if !completed {
        let report = ScenarioStepReportV1 {
            schema_version: SCENARIO_REPORT_SCHEMA_VERSION,
            manifest_run_id: active.run.run_id.clone(),
            session_run_id: active.session_run_id,
            evidence,
            completed: false,
            active_reaction_count: active.active_reaction_count,
            result: None,
            assertion_results: Vec::new(),
            fresh_instance_matches: None,
            scalar_oracle_matches: None,
        };
        live.active_step_run = Some(active);
        return Ok(report);
    }

    let finalized = (|| {
        let links_before_result = live.session.memory.store.link_count();
        let result = live.adapter.project(&live.session, &live.load)
            .map_err(|message| {
                ScenarioRunnerErrorV1::ProjectResultFailed {
                    run_id: active.run.run_id.clone(),
                    message,
                }
            })?;
        if live.session.memory.store.link_count() != links_before_result {
            return Err(
                ScenarioRunnerErrorV1::ResultProjectionMutatedCarrier {
                    run_id: active.run.run_id.clone(),
                },
            );
        }

        let assertion_results = evaluate_assertions_with_state(
            &active.run.assertions,
            &result,
            true,
            active.active_reaction_count,
        );

        let scalar_oracle =
            live.adapter.scalar_oracle(&active.run.inputs).map_err(
                |message| ScenarioRunnerErrorV1::OracleFailed {
                    run_id: active.run.run_id.clone(),
                    message,
                },
            )?;
        let scalar_oracle_matches = scalar_oracle == result.fields;

        let fresh_instance_matches = match live.manifest.oracle_policy {
            ScenarioOraclePolicyV1::FreshInstance => {
                let fresh = live.adapter.fresh_instance(
                    &active.run.inputs,
                )
                .map_err(|message| ScenarioRunnerErrorV1::OracleFailed {
                    run_id: active.run.run_id.clone(),
                    message,
                })?;
                Some(fresh == result)
            }
            ScenarioOraclePolicyV1::None
            | ScenarioOraclePolicyV1::ExpectedAssertions => None,
        };

        let carrier = live.session.memory.store.export_packed_duplets();
        if carrier.len() < live.loaded_link_count
            || &carrier[..live.loaded_link_count]
                != live.loaded_prefix.as_slice()
        {
            return Err(ScenarioRunnerErrorV1::LoadedBaseMutated {
                run_id: active.run.run_id.clone(),
            });
        }

        Ok(ScenarioStepReportV1 {
            schema_version: SCENARIO_REPORT_SCHEMA_VERSION,
            manifest_run_id: active.run.run_id.clone(),
            session_run_id: active.session_run_id,
            evidence,
            completed: true,
            active_reaction_count: active.active_reaction_count,
            result: Some(result),
            assertion_results,
            fresh_instance_matches,
            scalar_oracle_matches: Some(scalar_oracle_matches),
        })
    })();

    match finalized {
        Ok(report) => {
            live.completed_runs = active.session_run_id;
            Ok(report)
        }
        Err(error) => {
            live.session.fail_active_run();
            Err(error)
        }
    }
}

fn build_scenario_execution_report_v1(
    manifest: &ScenarioManifestV1,
    backend: ScenarioBackendV1,
    session_id: String,
    session_open_profile: SessionOpenProfileV1,
    program_profile: ScenarioProgramProfileV1,
    program_fingerprint: String,
    reports: Vec<ScenarioRunReportV1>,
) -> ScenarioExecutionReportV1 {
    let overall_pass = reports.iter().all(|run| {
        run.assertion_results
            .iter()
            .all(|assertion| assertion.passed)
            && run.oracle_matches != Some(false)
            && run.scalar_oracle_matches
    });

    ScenarioExecutionReportV1 {
        schema_version: SCENARIO_REPORT_SCHEMA_VERSION,
        scenario_id: manifest.scenario_id.clone(),
        scenario_version: manifest.scenario_version.clone(),
        program_profile: program_profile.clone(),
        manifest_field_semantics: scenario_manifest_field_semantics_v1(),
        backend,
        session_id,
        session_open_profile,
        runs: reports,
        overall_pass,
        provenance: ScenarioProvenanceV1 {
            amemory_version: include_str!("../../../VERSION")
                .trim()
                .to_owned(),
            build_sha: option_env!("AMEMORY_SOURCE_SHA")
                .or(option_env!("AMEMORY_BUILD_SHA"))
                .or(option_env!("GITHUB_SHA"))
                .map(str::to_owned),
            scenario_version: manifest.scenario_version.clone(),
            program_profile_id: program_profile.profile_id.clone(),
            program_profile,
            program_fingerprint: Some(program_fingerprint),
        },
    }
}

fn run_cpu_scenario_manifest_v1(
    manifest: &ScenarioManifestV1,
) -> Result<ScenarioExecutionReportV1, ScenarioRunnerErrorV1> {
    let mut live = open_cpu_scenario_session_v1(manifest)?;
    let session_id = live.session.runtime_identity_v1().session_id;
    let session_open_profile = live.session_open_profile.clone();
    let program_fingerprint = live.program_fingerprint.clone();
    let program_profile = live.program_profile.clone();
    let mut reports = Vec::with_capacity(manifest.run_sequence.len());

    for run in &manifest.run_sequence {
        reports.push(run_cpu_scenario_session_once_v1(
            &mut live,
            run,
        )?);
    }

    Ok(build_scenario_execution_report_v1(
        manifest,
        ScenarioBackendV1::OptimizedCpu,
        session_id,
        session_open_profile,
        program_profile,
        program_fingerprint,
        reports,
    ))
}

#[cfg(not(target_family = "wasm"))]
fn run_linksdb_scenario_manifest_v1(
    manifest: &ScenarioManifestV1,
) -> Result<ScenarioExecutionReportV1, ScenarioRunnerErrorV1> {
    let mut live = open_linksdb_scenario_session_v1(manifest)?;
    let session_id = live.session.runtime_identity_v1().session_id;
    let session_open_profile = live.session_open_profile.clone();
    let program_fingerprint = live.program_fingerprint.clone();
    let program_profile = live.program_profile.clone();
    let mut reports = Vec::with_capacity(manifest.run_sequence.len());

    for run in &manifest.run_sequence {
        reports.push(run_linksdb_scenario_session_once_v1(
            &mut live,
            run,
        )?);
    }

    Ok(build_scenario_execution_report_v1(
        manifest,
        ScenarioBackendV1::Linksdb,
        session_id,
        session_open_profile,
        program_profile,
        program_fingerprint,
        reports,
    ))
}

pub(crate) fn run_scenario_manifest_v1(
    manifest: &ScenarioManifestV1,
    backend: ScenarioBackendV1,
) -> Result<ScenarioExecutionReportV1, ScenarioRunnerErrorV1> {
    let validation = validate_manifest_v1(manifest);
    if !validation.is_empty() {
        return Err(ScenarioRunnerErrorV1::Validation {
            errors: validation,
        });
    }

    if !manifest.supported_backends.contains(&backend) {
        return Err(ScenarioRunnerErrorV1::UnsupportedBackend { backend });
    }

    match backend {
        ScenarioBackendV1::OptimizedCpu => {
            run_cpu_scenario_manifest_v1(manifest)
        }
        ScenarioBackendV1::Linksdb => {
            #[cfg(not(target_family = "wasm"))]
            {
                run_linksdb_scenario_manifest_v1(manifest)
            }
            #[cfg(target_family = "wasm")]
            {
                Err(ScenarioRunnerErrorV1::UnsupportedBackend { backend })
            }
        }
        ScenarioBackendV1::Webgpu => {
            // WebGPU is a real persistent browser adapter, but not a native
            // Rust Scenario host. Never silently substitute CPU.
            Err(ScenarioRunnerErrorV1::UnsupportedBackend { backend })
        }
    }
}

#[cfg(not(target_family = "wasm"))]
pub(crate) fn run_native_cpu_linksdb_differential_json_v1(
    source: &str,
) -> Result<String, String> {
    let manifest = super::scenario::parse_and_validate_manifest_v1(source)
        .map_err(|errors| {
            let detail = serde_json::to_string(&errors)
                .unwrap_or_else(|_| "manifest validation failed".to_owned());
            format!("MANIFEST_INVALID: {detail}")
        })?;

    let cpu = run_scenario_manifest_v1(
        &manifest,
        ScenarioBackendV1::OptimizedCpu,
    )
    .map_err(|error| {
        format!("OPTIMIZED_CPU_EXECUTION_FAILED: {error:?}")
    })?;

    let linksdb = run_scenario_manifest_v1(
        &manifest,
        ScenarioBackendV1::Linksdb,
    )
    .map_err(|error| {
        format!("LINKSDB_EXECUTION_FAILED: {error:?}")
    })?;

    let comparison =
        compare_scenario_execution_reports_v1(&cpu, &linksdb);
    serde_json::to_string_pretty(&comparison)
        .map_err(|error| format!("DIFFERENTIAL_SERIALIZE_FAILED: {error}"))
}

fn evaluate_assertions(
    assertions: &[ScenarioAssertionV1],
    result: &ScenarioNormalizedResultV1,
    observed: &ObservedRunV1,
) -> Vec<ScenarioAssertionResultV1> {
    evaluate_assertions_with_state(
        assertions,
        result,
        observed.final_quiescent,
        observed.active_reaction_count,
    )
}

fn evaluate_assertions_with_state(
    assertions: &[ScenarioAssertionV1],
    result: &ScenarioNormalizedResultV1,
    final_quiescent: bool,
    active_reaction_count: u32,
) -> Vec<ScenarioAssertionResultV1> {
    assertions
        .iter()
        .enumerate()
        .map(|(index, assertion)| match assertion {
            ScenarioAssertionV1::ResultFieldEquals { field, expected } => {
                let actual = result.fields.get(field).cloned();
                ScenarioAssertionResultV1 {
                    assertion_index: index as u32,
                    kind: "RESULT_FIELD_EQUALS".to_owned(),
                    passed: actual.as_ref() == Some(expected),
                    field: Some(field.clone()),
                    expected: Some(expected.clone()),
                    actual,
                }
            }
            ScenarioAssertionV1::QuiescentEquals { expected } => {
                let actual = Value::Bool(final_quiescent);
                ScenarioAssertionResultV1 {
                    assertion_index: index as u32,
                    kind: "QUIESCENT_EQUALS".to_owned(),
                    passed: final_quiescent == *expected,
                    field: None,
                    expected: Some(Value::Bool(*expected)),
                    actual: Some(actual),
                }
            }
            ScenarioAssertionV1::ReactionCountEquals { expected } => {
                let actual = Value::from(active_reaction_count);
                ScenarioAssertionResultV1 {
                    assertion_index: index as u32,
                    kind: "REACTION_COUNT_EQUALS".to_owned(),
                    passed: active_reaction_count == *expected,
                    field: None,
                    expected: Some(Value::from(*expected)),
                    actual: Some(actual),
                }
            }
        })
        .collect()
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::parse_and_validate_manifest_v1;

    const MUX1_LIFECYCLE: &str =
        include_str!("../scenarios/mux1-lifecycle-v1.json");
    const XOR32_LIFECYCLE: &str =
        include_str!("../scenarios/xor32-lifecycle-v1.json");
    const AND32_LIFECYCLE: &str =
        include_str!("../scenarios/and32-lifecycle-v1.json");
    const OR32_LIFECYCLE: &str =
        include_str!("../scenarios/or32-lifecycle-v1.json");
    const NOT32_LIFECYCLE: &str =
        include_str!("../scenarios/not32-lifecycle-v1.json");
    const TEST32_LIFECYCLE: &str =
        include_str!("../scenarios/test32-lifecycle-v1.json");
    const ADD32_LIFECYCLE: &str =
        include_str!("../scenarios/add32-lifecycle-v1.json");
    const ADC32_LIFECYCLE: &str =
        include_str!("../scenarios/adc32-lifecycle-v1.json");
    const SUB32_LIFECYCLE: &str =
        include_str!("../scenarios/sub32-lifecycle-v1.json");
    const SBB32_LIFECYCLE: &str =
        include_str!("../scenarios/sbb32-lifecycle-v1.json");
    const CMP32_LIFECYCLE: &str =
        include_str!("../scenarios/cmp32-lifecycle-v1.json");
    const SHL32_LIFECYCLE: &str =
        include_str!("../scenarios/shl32-lifecycle-v1.json");
    const SHR32_LIFECYCLE: &str =
        include_str!("../scenarios/shr32-lifecycle-v1.json");
    const SAR32_LIFECYCLE: &str =
        include_str!("../scenarios/sar32-lifecycle-v1.json");
    const ROL32_LIFECYCLE: &str =
        include_str!("../scenarios/rol32-lifecycle-v1.json");
    const ROR32_LIFECYCLE: &str =
        include_str!("../scenarios/ror32-lifecycle-v1.json");
    const RCL32_LIFECYCLE: &str =
        include_str!("../scenarios/rcl32-lifecycle-v1.json");
    const RCR32_LIFECYCLE: &str =
        include_str!("../scenarios/rcr32-lifecycle-v1.json");
    const INC32_LIFECYCLE: &str =
        include_str!("../scenarios/inc32-lifecycle-v1.json");
    const DEC32_LIFECYCLE: &str =
        include_str!("../scenarios/dec32-lifecycle-v1.json");
    const NEG32_LIFECYCLE: &str =
        include_str!("../scenarios/neg32-lifecycle-v1.json");
    const MUL_EFFECT32_LIFECYCLE: &str =
        include_str!("../scenarios/mul-effect32-lifecycle-v1.json");
    const MUX32_LIFECYCLE: &str =
        include_str!("../scenarios/mux32-lifecycle-v1.json");
    const MUL32_LIFECYCLE: &str =
        include_str!("../scenarios/mul32-lifecycle-v1.json");
    const RADIX_MEMORY8_LIFECYCLE: &str =
        include_str!("../scenarios/radix-memory8-lifecycle-v1.json");

    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn canonical_mux1_cpu_linksdb_portable_semantics_match() {
        let manifest =
            parse_and_validate_manifest_v1(MUX1_LIFECYCLE).unwrap();
        let cpu = run_scenario_manifest_v1(
            &manifest,
            ScenarioBackendV1::OptimizedCpu,
        )
        .unwrap();
        let linksdb = run_scenario_manifest_v1(
            &manifest,
            ScenarioBackendV1::Linksdb,
        )
        .unwrap();

        let comparison =
            compare_scenario_execution_reports_v1(&cpu, &linksdb);
        assert!(
            comparison.portable_match,
            "portable Scenario semantics diverged: {:?}",
            comparison
                .portable_checks
                .iter()
                .filter(|check| !check.matches)
                .collect::<Vec<_>>(),
        );
        assert!(
            comparison.diagnostic_match,
            "CPU/LinksDB common diagnostics diverged: {:?}",
            comparison
                .diagnostic_checks
                .iter()
                .filter(|check| !check.matches)
                .collect::<Vec<_>>(),
        );
        assert!(
            comparison.left_evidence.overall_pass
                && comparison.right_evidence.overall_pass
        );
        assert_eq!(
            comparison.left_evidence,
            comparison.right_evidence,
            "independent evidence classes must remain separately inspectable",
        );
        assert!(
            comparison.excluded_fields.iter().any(|field| {
                field.path == "runs[*].observed.finalScope"
                    && field.class
                        == ScenarioDifferentialExclusionClassV1::BackendLocalHandle
            }),
            "local Scope handles must be explicitly classified as non-portable",
        );
        assert!(
            comparison.excluded_fields.iter().any(|field| {
                field.path
                    == "runs[*].observed.profile.budgetAccounting.denseCarrierAllocatedBytes"
                    && field.class
                        == ScenarioDifferentialExclusionClassV1::PhysicalResource
            }),
            "backend-local resource metrics must be explicitly classified",
        );

        let json = serde_json::to_string(&comparison).unwrap();
        let decoded: ScenarioDifferentialReportV1 =
            serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, comparison);

        let mut evidence_only_difference = linksdb.clone();
        evidence_only_difference.runs[0].scalar_oracle_matches = false;
        evidence_only_difference.overall_pass = false;
        let evidence_comparison = compare_scenario_execution_reports_v1(
            &linksdb,
            &evidence_only_difference,
        );
        assert!(
            evidence_comparison.portable_match,
            "independent evidence must not silently become semantic equality"
        );
        assert_ne!(
            evidence_comparison.left_evidence,
            evidence_comparison.right_evidence,
        );
        assert!(evidence_comparison.diagnostic_match);

        let mut diagnostic_difference = linksdb.clone();
        diagnostic_difference.runs[0]
            .observed
            .profile
            .as_mut()
            .expect("canonical TRACE run must have profile")
            .structural
            .trigger_incidence_candidates += 1;
        let diagnostic_comparison = compare_scenario_execution_reports_v1(
            &linksdb,
            &diagnostic_difference,
        );
        assert!(
            diagnostic_comparison.portable_match,
            "implementation diagnostics must not become semantic authority"
        );
        assert!(!diagnostic_comparison.diagnostic_match);

        let mut semantic_difference = linksdb.clone();
        semantic_difference.runs[0]
            .result
            .fields
            .insert("d0MismatchProbe".to_owned(), Value::from(true));
        let semantic_comparison =
            compare_scenario_execution_reports_v1(&linksdb, &semantic_difference);
        assert!(!semantic_comparison.portable_match);
        let result_mismatch = semantic_comparison
            .portable_checks
            .iter()
            .find(|check| check.path == "runs[0].result")
            .expect("normalized Result must be a portable comparison field");
        assert!(!result_mismatch.matches);
        assert!(result_mismatch.left.is_some());
        assert!(result_mismatch.right.is_some());

        for (cpu_run, linksdb_run) in cpu.runs.iter().zip(&linksdb.runs) {
            let cpu_budget = cpu_run
                .observed
                .profile
                .as_ref()
                .expect("canonical TRACE run must have CPU profile")
                .budget_accounting
                .as_ref()
                .expect("CPU bounded accounting");
            let linksdb_budget = linksdb_run
                .observed
                .profile
                .as_ref()
                .expect("canonical TRACE run must have LinksDB profile")
                .budget_accounting
                .as_ref()
                .expect("LinksDB bounded accounting");

            assert!(
                cpu_budget.dense_carrier_allocated_bytes.is_some(),
                "optimized CPU must report its dense-carrier allocation"
            );
            assert!(cpu_budget.max_dense_carrier_bytes.is_some());
            assert_eq!(
                linksdb_budget.dense_carrier_allocated_bytes,
                None,
                "unavailable LinksDB dense-carrier bytes must stay absent, not zero"
            );
            assert_eq!(linksdb_budget.max_dense_carrier_bytes, None);
        }

        assert_eq!(
            cpu.runs
                .iter()
                .map(|run| run.configuration_reused)
                .collect::<Vec<_>>(),
            vec![false, false, false, true],
        );
        assert_eq!(
            linksdb.runs
                .iter()
                .map(|run| run.configuration_reused)
                .collect::<Vec<_>>(),
            vec![false, false, false, true],
        );
    }

    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn native_cpu_linksdb_differential_driver_serializes_d0_report() {
        let json =
            run_native_cpu_linksdb_differential_json_v1(MUX1_LIFECYCLE)
                .unwrap();
        let report: ScenarioDifferentialReportV1 =
            serde_json::from_str(&json).unwrap();

        assert_eq!(
            report.left_backend,
            ScenarioBackendV1::OptimizedCpu
        );
        assert_eq!(report.right_backend, ScenarioBackendV1::Linksdb);
        assert!(report.portable_match);
        assert!(report.diagnostic_match);
        assert!(
            report.left_evidence.overall_pass
                && report.right_evidence.overall_pass
        );
    }

    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn native_cpu_linksdb_differential_driver_fails_closed_when_linksdb_is_unsupported(
    ) {
        let error =
            run_native_cpu_linksdb_differential_json_v1(XOR32_LIFECYCLE)
                .unwrap_err();

        assert!(
            error.starts_with("LINKSDB_EXECUTION_FAILED:"),
            "unexpected error: {error}"
        );
        assert!(
            error.contains("UnsupportedBackend")
                || error.contains("UnsupportedProgramProfile"),
            "must expose the real unsupported LinksDB boundary: {error}"
        );
    }

    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn canonical_mux1_manifest_runs_end_to_end_on_linksdb_only_where_proven() {
        let manifest =
            parse_and_validate_manifest_v1(MUX1_LIFECYCLE).unwrap();
        assert!(
            manifest.supported_backends.contains(
                &ScenarioBackendV1::OptimizedCpu,
            )
        );
        assert!(
            manifest.supported_backends.contains(
                &ScenarioBackendV1::Webgpu,
            )
        );
        assert!(
            manifest.supported_backends.contains(
                &ScenarioBackendV1::Linksdb,
            )
        );

        let report =
            run_scenario_manifest_v1(
                &manifest,
                ScenarioBackendV1::Linksdb,
            )
            .unwrap();

        assert_eq!(report.backend, ScenarioBackendV1::Linksdb);
        assert!(report.overall_pass);
        assert_eq!(report.runs.len(), 4);
        assert_eq!(
            report.session_open_profile.backend_id,
            LINKSDB_BACKEND_ID,
        );
        assert!(report.provenance.program_fingerprint.is_some());

        let values = report
            .runs
            .iter()
            .map(|run| {
                run.result.fields["value"]
                    .as_u64()
                    .expect("MUX1 value")
            })
            .collect::<Vec<_>>();
        assert_eq!(values, vec![0, 1, 1, 0]);

        let run_ids = report
            .runs
            .iter()
            .map(|run| run.session_run_id)
            .collect::<Vec<_>>();
        assert_eq!(run_ids, vec![1, 2, 3, 4]);
        assert_eq!(
            report
                .runs
                .iter()
                .map(|run| run.configuration_reused)
                .collect::<Vec<_>>(),
            vec![false, false, false, true],
        );

        for run in &report.runs {
            assert_eq!(run.observed.session_id, report.session_id);
            assert_eq!(run.observed.backend_id, LINKSDB_BACKEND_ID);
            assert_eq!(run.observed.active_reaction_count, 7);
            assert!(run.observed.final_quiescent);
            assert_eq!(run.fresh_instance_matches, Some(true));
            assert_eq!(run.oracle_matches, Some(true));
            assert!(run.scalar_oracle_matches);
            assert!(run
                .assertion_results
                .iter()
                .all(|assertion| assertion.passed));

            let profile = run
                .observed
                .profile
                .as_ref()
                .expect("TRACE includes profile");
            assert_eq!(profile.dense_carrier_allocated_bytes, None);
            assert_eq!(profile.max_dense_carrier_bytes, None);
            assert!(!profile.full_resident_bytes_available);
            let accounting = profile
                .budget_accounting
                .as_ref()
                .expect("bounded accounting");
            assert_eq!(accounting.dense_carrier_allocated_bytes, None);
            assert_eq!(accounting.max_dense_carrier_bytes, None);
            assert!(run.pipeline_profile.is_some());
            assert!(run.result.result_recursive_wire.is_some());
        }

        assert_eq!(
            report.runs[0].result.result_recursive_wire,
            report.runs[3].result.result_recursive_wire,
            "return-to-first must reproduce the same portable result wire",
        );

        let mut unsupported =
            parse_and_validate_manifest_v1(XOR32_LIFECYCLE).unwrap();
        unsupported
            .supported_backends
            .push(ScenarioBackendV1::Linksdb);
        assert!(matches!(
            run_scenario_manifest_v1(
                &unsupported,
                ScenarioBackendV1::Linksdb,
            ),
            Err(
                ScenarioRunnerErrorV1::UnsupportedProgramProfile { .. }
            ),
        ));
    }

    #[test]
    fn prepared_aset_fingerprint_is_stable_and_program_specific() {
        let mux_a = prepare_mux1_session_program().unwrap();
        let mux_b = prepare_mux1_session_program().unwrap();
        let xor = prepare_logic32_session_program(3).unwrap();

        let mux_fingerprint = prepared_aset_fingerprint_v1(&mux_a);
        assert_eq!(
            mux_fingerprint,
            prepared_aset_fingerprint_v1(&mux_b),
            "same static program must have stable fingerprint",
        );
        assert_ne!(
            mux_fingerprint,
            prepared_aset_fingerprint_v1(&xor),
            "different prepared programs must not share the same fingerprint",
        );
        assert!(mux_fingerprint.starts_with(
            "prepared-aset-fnv1a64-v1:"
        ));
        assert_eq!(
            mux_fingerprint.len(),
            "prepared-aset-fnv1a64-v1:".len() + 16,
        );
    }

    #[test]
    fn built_in_program_descriptor_is_resolved_as_one_exact_registry_tuple() {
        let canonical =
            parse_and_validate_manifest_v1(MUX1_LIFECYCLE).unwrap();

        for (field, mutate) in [
            ("family", 0u8),
            ("programId", 1u8),
            ("asetSource", 2u8),
        ] {
            let mut manifest = canonical.clone();
            match mutate {
                0 => manifest.program_profile.family = "not-mux".to_owned(),
                1 => manifest.program_profile.program_id = "not-mux1".to_owned(),
                2 => manifest.program_profile.aset_source =
                    "audit:unknown-program".to_owned(),
                _ => unreachable!(),
            }

            let error = match open_cpu_scenario_session_v1(&manifest) {
                Err(error) => error,
                Ok(_) => panic!("{field} mismatch unexpectedly opened Session"),
            };
            match error {
                ScenarioRunnerErrorV1::ProgramProfileMismatch {
                    requested,
                    canonical: actual,
                } => {
                    assert_eq!(requested, manifest.program_profile);
                    assert_eq!(actual, canonical.program_profile);
                }
                other => panic!(
                    "{field} mismatch returned wrong error: {other:?}"
                ),
            }
        }

        let mut unknown = canonical;
        unknown.program_profile.profile_id =
            "a-circuit:does-not-exist".to_owned();
        assert!(matches!(
            open_cpu_scenario_session_v1(&unknown),
            Err(ScenarioRunnerErrorV1::UnsupportedProgramProfile { .. })
        ));
    }

    #[test]
    fn invalid_live_run_is_rejected_before_configure_without_link_delta() {
        let manifest =
            parse_and_validate_manifest_v1(MUX1_LIFECYCLE).unwrap();
        let mut live = open_cpu_scenario_session_v1(&manifest).unwrap();
        let before = live.status_v1();

        let mut invalid = manifest.run_sequence[0].clone();
        invalid.run_id = "invalid-prefix-input".to_owned();
        invalid
            .inputs
            .insert("S".to_owned(), Value::String("12garbage".to_owned()));

        let error =
            run_cpu_scenario_session_once_v1(&mut live, &invalid).unwrap_err();
        assert!(matches!(error, ScenarioRunnerErrorV1::Validation { .. }));

        let after = live.status_v1();
        assert_eq!(after.session_id, before.session_id);
        assert_eq!(after.current_link_count, before.current_link_count);
        assert_eq!(after.completed_runs, before.completed_runs);
        assert_eq!(after.prepare_count, 1);
        assert_eq!(after.load_count, 1);
    }

    #[test]
    fn retained_cpu_session_runs_separate_mux1_actions_without_reload() {
        let manifest =
            parse_and_validate_manifest_v1(MUX1_LIFECYCLE).unwrap();
        let mut live = open_cpu_scenario_session_v1(&manifest).unwrap();
        let opened = live.status_v1();

        let first = run_cpu_scenario_session_once_v1(
            &mut live,
            &manifest.run_sequence[0],
        )
        .unwrap();
        let second = run_cpu_scenario_session_once_v1(
            &mut live,
            &manifest.run_sequence[1],
        )
        .unwrap();

        let mut return_run = manifest.run_sequence[0].clone();
        return_run.run_id = "manual-return-to-first".to_owned();
        let returned = run_cpu_scenario_session_once_v1(
            &mut live,
            &return_run,
        )
        .unwrap();
        let after = live.status_v1();

        assert_eq!(
            vec![
                first.session_run_id,
                second.session_run_id,
                returned.session_run_id,
            ],
            vec![1, 2, 3],
        );
        assert_eq!(opened.session_id, after.session_id);
        assert_eq!(opened.memory_instance_id, after.memory_instance_id);
        assert_ne!(opened.session_id, opened.memory_instance_id);
        assert_eq!(opened.runtime_state, SessionStateV1::Open);
        assert_eq!(after.runtime_state, SessionStateV1::Quiescent);
        assert_eq!(
            opened.runtime_capabilities.step,
            CapabilitySupportV1::Supported,
        );
        assert_eq!(
            opened.runtime_capabilities.explicit_close,
            CapabilitySupportV1::Unsupported,
        );
        assert_eq!(opened.store_instance_id, after.store_instance_id);
        assert_eq!(opened.engine_instance_id, after.engine_instance_id);
        assert_eq!(opened.base_link_count, after.base_link_count);
        assert_eq!(opened.program_fingerprint, after.program_fingerprint);
        assert_eq!(opened.program_profile, manifest.program_profile);
        assert_eq!(after.program_profile, manifest.program_profile);
        assert_eq!(
            opened.manifest_field_semantics.invariants,
            ScenarioManifestFieldAuthorityV1::Advisory
        );
        assert_eq!(
            opened.manifest_field_semantics.profiling_policy,
            ScenarioManifestFieldAuthorityV1::Advisory
        );
        assert_eq!(
            opened.manifest_field_semantics.visualization_profile,
            ScenarioManifestFieldAuthorityV1::Advisory
        );
        assert_eq!(
            opened.manifest_field_semantics.program_profile,
            ScenarioManifestFieldAuthorityV1::Enforced
        );
        assert_eq!(opened.prepare_count, 1);
        assert_eq!(opened.load_count, 1);
        assert_eq!(after.prepare_count, 1);
        assert_eq!(after.load_count, 1);
        assert_eq!(after.completed_runs, 3);
        assert_eq!(first.observed.session_id, opened.session_id);
        assert_eq!(second.observed.session_id, opened.session_id);
        assert_eq!(returned.observed.session_id, opened.session_id);
        assert_eq!(first.result, returned.result);
        assert!(
            returned.configuration_reused,
            "return-to-first must reuse canonical configuration Links",
        );
        assert!(
            after.current_link_count >= after.base_link_count,
            "CONFIGURE/EXECUTE may append runtime Links but must retain base",
        );
    }

    fn bounded_runtime_first_run(
        source: &str,
        observation_level: RunObservationLevel,
    ) -> (
        ScenarioNormalizedResultV1,
        Vec<(Handle, Handle)>,
        Vec<CpuSessionReactionEvidenceV1>,
        ScenarioLiveSessionStatusV1,
        ScenarioLiveSessionStatusV1,
    ) {
        let manifest = parse_and_validate_manifest_v1(source).unwrap();
        let run = manifest.run_sequence[0].clone();
        let mut live = open_cpu_scenario_session_v1(&manifest).unwrap();
        let opened = live.status_v1();

        let configured = live.adapter.configure(
            &mut live.session,
            &live.load,
            &run.inputs,
        )
        .unwrap();
        let bounded = live
            .session
            .run_to_quiescence(
                configured.initial,
                RunBudgetV1::scenario_default(run.max_reactions),
                observation_level.runtime_trace_mode(),
            )
            .unwrap();
        assert_eq!(bounded.run_id, 1);
        assert_eq!(
            bounded.stop_reason,
            RunStopReasonV1::Quiescent,
        );
        assert_eq!(
            live.session.runtime_state_v1(),
            SessionStateV1::Quiescent,
        );

        let evidence = bounded
            .steps
            .into_iter()
            .map(|step| {
                project_cpu_session_step_v1(step, observation_level).evidence
            })
            .collect::<Vec<_>>();
        for (index, step) in evidence.iter().enumerate() {
            assert_eq!(step.session_id, opened.session_id);
            assert_eq!(step.run_id, bounded.run_id);
            assert_eq!(step.reaction_index, index as u32);
            assert!(
                step.links_after >= step.links_before,
                "a successful reaction must not shrink the canonical Store",
            );
            if let Some(previous) = index
                .checked_sub(1)
                .and_then(|previous| evidence.get(previous))
            {
                assert_eq!(
                    previous.scope_after,
                    step.scope_before,
                    "runtime Scope chain must be contiguous",
                );
            }
        }

        let result = live.adapter.project(&live.session, &live.load).unwrap();
        let carrier = live.session.memory.store.export_packed_duplets();
        let after = live.status_v1();
        (result, carrier, evidence, opened, after)
    }

    fn wrapped_first_run(
        source: &str,
        observation_level: RunObservationLevel,
    ) -> (
        ScenarioNormalizedResultV1,
        Vec<(Handle, Handle)>,
        ObservedRunV1,
        ScenarioLiveSessionStatusV1,
        ScenarioLiveSessionStatusV1,
    ) {
        let mut manifest = parse_and_validate_manifest_v1(source).unwrap();
        manifest.observation_level = observation_level;
        let run = manifest.run_sequence[0].clone();
        let mut live = open_cpu_scenario_session_v1(&manifest).unwrap();
        let opened = live.status_v1();
        let report =
            run_cpu_scenario_session_once_v1(&mut live, &run).unwrap();
        let carrier = live.session.memory.store.export_packed_duplets();
        let after = live.status_v1();
        (report.result, carrier, report.observed, opened, after)
    }

    #[test]
    fn runtime_bounded_steps_match_observed_to_quiescence() {
        for (source, active_reactions) in [
            (MUX1_LIFECYCLE, 7u32),
            (XOR32_LIFECYCLE, 147u32),
        ] {
            let (step_result, step_carrier, steps, step_opened, step_after) =
                bounded_runtime_first_run(source, RunObservationLevel::Trace);
            let (
                wrapped_result,
                wrapped_carrier,
                observed,
                wrapped_opened,
                wrapped_after,
            ) = wrapped_first_run(source, RunObservationLevel::Trace);

            assert_eq!(step_result, wrapped_result);
            assert_eq!(step_carrier, wrapped_carrier);
            assert_eq!(
                steps.len() as u32,
                active_reactions + 1,
                "terminal quiescent step is real reaction evidence",
            );
            assert_eq!(
                observed.active_reaction_count,
                active_reactions,
            );
            assert!(observed.final_quiescent);
            assert_eq!(
                steps.last().unwrap().scope_after,
                observed.final_scope,
            );
            assert!(steps.last().unwrap().quiescent);

            assert_eq!(
                step_opened.store_instance_id,
                step_after.store_instance_id,
            );
            assert_eq!(
                step_opened.engine_instance_id,
                step_after.engine_instance_id,
            );
            assert_eq!(
                wrapped_opened.store_instance_id,
                wrapped_after.store_instance_id,
            );
            assert_eq!(
                wrapped_opened.engine_instance_id,
                wrapped_after.engine_instance_id,
            );
        }
    }

    #[test]
    fn profile_projects_exact_dense_carrier_budget_boundary() {
        let mut manifest = parse_and_validate_manifest_v1(MUX1_LIFECYCLE).unwrap();
        manifest.run_sequence.truncate(1);
        manifest.observation_level = RunObservationLevel::Profile;
        let run = manifest.run_sequence[0].clone();
        let mut live = open_cpu_scenario_session_v1(&manifest).unwrap();

        let report = run_cpu_scenario_session_once_v1(&mut live, &run).unwrap();
        let profile = report
            .observed
            .profile
            .as_ref()
            .expect("PROFILE observation must project runtime resource evidence");

        assert_eq!(
            profile.dense_carrier_allocated_bytes,
            Some(
                live.session
                    .memory
                    .store
                    .dense_carrier_index_allocated_bytes(),
            ),
        );
        assert_eq!(
            profile.max_dense_carrier_bytes,
            Some(
                CpuRunResourceBudgetV1::default()
                    .max_dense_carrier_bytes,
            ),
        );
        assert!(!profile.full_resident_bytes_available);

        let accounting = profile
            .budget_accounting
            .as_ref()
            .expect("PROFILE must project authoritative runtime budget accounting");
        let budget = RunBudgetV1::scenario_default(run.max_reactions);
        assert_eq!(accounting.max_reactions, budget.max_reactions);
        assert_eq!(
            accounting.max_appended_links,
            budget.max_appended_links,
        );
        assert_eq!(accounting.max_total_links, budget.max_total_links);
        assert_eq!(accounting.max_scope_width, budget.max_scope_width);
        assert_eq!(
            accounting.max_match_candidates,
            budget.max_match_candidates,
        );
        assert_eq!(
            accounting.max_unification_nodes,
            budget.max_unification_nodes,
        );
        assert_eq!(
            accounting.max_instantiation_nodes,
            budget.max_instantiation_nodes,
        );
        assert_eq!(
            accounting.max_dense_carrier_bytes,
            Some(
                CpuRunResourceBudgetV1::default()
                    .max_dense_carrier_bytes,
            ),
        );
        assert_eq!(accounting.total_links, profile.links_after_run);
        assert_eq!(
            accounting.appended_links_consumed,
            profile.execution_link_delta,
        );
        assert_eq!(accounting.scope_width, profile.scope_after_width);
        assert_eq!(
            accounting.match_candidates,
            profile.structural.trigger_incidence_candidates,
        );
        assert_eq!(
            accounting.unification_nodes,
            profile.structural.unification_nodes_visited,
        );
        assert_eq!(
            accounting.instantiation_nodes,
            profile.structural.instantiation_nodes_visited,
        );
        assert_eq!(
            accounting.dense_carrier_allocated_bytes,
            profile.dense_carrier_allocated_bytes,
        );
        assert!(!accounting.full_resident_bytes_available);
    }

    #[test]
    fn observation_level_changes_evidence_not_step_semantics() {
        let mut reference: Option<(
            ScenarioNormalizedResultV1,
            Vec<(Handle, Handle)>,
            Vec<u32>,
            usize,
        )> = None;

        for level in [
            RunObservationLevel::Off,
            RunObservationLevel::Profile,
            RunObservationLevel::Trace,
            RunObservationLevel::Full,
        ] {
            let (result, carrier, steps, opened, after) =
                bounded_runtime_first_run(MUX1_LIFECYCLE, level);
            let final_scope = steps.last().unwrap().scope_after.clone();
            let semantic = (
                result,
                carrier,
                final_scope,
                steps.len(),
            );

            if let Some(expected) = &reference {
                assert_eq!(
                    &semantic, expected,
                    "observation level changed Session.step semantics",
                );
            } else {
                reference = Some(semantic);
            }

            assert_eq!(opened.session_id, after.session_id);
            assert_eq!(
                opened.store_instance_id,
                after.store_instance_id,
            );
            assert_eq!(
                opened.engine_instance_id,
                after.engine_instance_id,
            );

            if level.traces() {
                assert!(steps.iter().any(|step| {
                    step.structural_facts
                        .as_ref()
                        .is_some_and(|facts| !facts.is_empty())
                }));
            } else {
                assert!(
                    steps.iter().all(|step| step.structural_facts.is_none())
                );
            }
        }
    }

    #[test]
    fn canonical_mux1_manifest_runs_four_times_on_one_session() {
        let manifest =
            parse_and_validate_manifest_v1(MUX1_LIFECYCLE).unwrap();
        let report = run_scenario_manifest_v1(
            &manifest,
            ScenarioBackendV1::OptimizedCpu,
        )
        .unwrap();

        assert!(report.overall_pass);
        assert_eq!(report.runs.len(), 4);
        assert!(
            report
                .runs
                .iter()
                .all(|run| run.observed.session_id == report.session_id)
        );
        assert_eq!(
            report
                .runs
                .iter()
                .map(|run| run.session_run_id)
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 4],
        );
        assert!(
            report
                .runs
                .iter()
                .all(|run| run.observed.active_reaction_count == 7)
        );
        assert!(
            report
                .runs
                .iter()
                .all(|run| run.observed.final_quiescent)
        );
        assert!(
            report
                .runs
                .iter()
                .all(|run| run.oracle_matches == Some(true)
                && run.fresh_instance_matches == Some(true)
                && run.scalar_oracle_matches)
        );
        assert_eq!(report.runs[0].result, report.runs[3].result);
        assert!(
            report.runs[3].configuration_reused,
            "returning to first input must reuse canonical configuration Links",
        );

        let json = serde_json::to_string(&report).unwrap();
        let decoded: ScenarioExecutionReportV1 =
            serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, report);
        let expected_fingerprint = prepared_aset_fingerprint_v1(
            &prepare_mux1_session_program().unwrap(),
        );
        assert_eq!(
            report.provenance.program_fingerprint.as_deref(),
            Some(expected_fingerprint.as_str()),
        );
        assert_eq!(report.program_profile, manifest.program_profile);
        assert_eq!(report.provenance.program_profile, manifest.program_profile);
        assert_eq!(
            report.manifest_field_semantics.invariants,
            ScenarioManifestFieldAuthorityV1::Advisory
        );
    }

    #[test]
    fn canonical_logic32_family_manifests_run_four_times_on_one_session() {
        for (source, expected_reactions) in [
            (AND32_LIFECYCLE, 147u32),
            (OR32_LIFECYCLE, 147u32),
            (NOT32_LIFECYCLE, 67u32),
            (TEST32_LIFECYCLE, 147u32),
        ] {
            let manifest = parse_and_validate_manifest_v1(source).unwrap();
            let report = run_scenario_manifest_v1(
                &manifest,
                ScenarioBackendV1::OptimizedCpu,
            )
            .unwrap();

            assert!(report.overall_pass, "{}", manifest.scenario_id);
            assert_eq!(report.runs.len(), 4, "{}", manifest.scenario_id);
            assert!(report.runs.iter().all(|run| {
                run.observed.session_id == report.session_id
                    && run.observed.active_reaction_count == expected_reactions
                    && run.observed.final_quiescent
                    && run.oracle_matches == Some(true)
                    && run.fresh_instance_matches == Some(true)
                    && run.scalar_oracle_matches
            }), "{}", manifest.scenario_id);
            assert_eq!(
                report.runs[0].result,
                report.runs[3].result,
                "{} return-to-first result",
                manifest.scenario_id,
            );
            assert!(
                report.runs[3].configuration_reused,
                "{} return-to-first must reuse canonical Links",
                manifest.scenario_id,
            );
        }
    }

    #[test]
    fn canonical_xor32_manifest_runs_four_times_on_one_session() {
        let manifest =
            parse_and_validate_manifest_v1(XOR32_LIFECYCLE).unwrap();
        let report = run_scenario_manifest_v1(
            &manifest,
            ScenarioBackendV1::OptimizedCpu,
        )
        .unwrap();

        assert!(report.overall_pass);
        assert_eq!(report.runs.len(), 4);
        assert!(report.runs.iter().all(|run| {
            run.observed.session_id == report.session_id
        }));
        assert!(report.runs.iter().all(|run| {
            run.observed.active_reaction_count == 147
        }));
        assert!(report.runs.iter().all(|run| {
            run.observed.final_quiescent
        }));
        assert!(report.runs.iter().all(|run| {
            run.oracle_matches == Some(true)
                && run.fresh_instance_matches == Some(true)
                && run.scalar_oracle_matches
        }));
        assert_eq!(
            report.runs[2].result.fields.get("value"),
            Some(&Value::String("0x1d3b5687".to_owned())),
        );
        assert_eq!(report.runs[0].result, report.runs[3].result);
        assert!(
            report.runs[3].configuration_reused,
            "returning to first XOR32 inputs must reuse canonical Links",
        );
    }

    #[test]
    fn canonical_arithmetic32_family_manifests_run_four_times_on_one_session() {
        for source in [
            ADC32_LIFECYCLE,
            SUB32_LIFECYCLE,
            SBB32_LIFECYCLE,
            CMP32_LIFECYCLE,
        ] {
            let manifest = parse_and_validate_manifest_v1(source).unwrap();
            let report = run_scenario_manifest_v1(
                &manifest,
                ScenarioBackendV1::OptimizedCpu,
            )
            .unwrap();

            assert!(report.overall_pass, "{}", manifest.scenario_id);
            assert_eq!(report.runs.len(), 4, "{}", manifest.scenario_id);
            assert!(report.runs.iter().all(|run| {
                run.observed.session_id == report.session_id
                    && run.observed.active_reaction_count == 609
                    && run.observed.final_quiescent
                    && run.oracle_matches == Some(true)
                    && run.fresh_instance_matches == Some(true)
                    && run.scalar_oracle_matches
            }), "{}", manifest.scenario_id);
            assert_eq!(
                report.runs[0].result,
                report.runs[3].result,
                "{} return-to-first result",
                manifest.scenario_id,
            );
            assert!(
                report.runs[3].configuration_reused,
                "{} return-to-first must reuse canonical Links",
                manifest.scenario_id,
            );
        }
    }

    #[test]
    fn canonical_add32_manifest_runs_four_times_on_one_session() {
        let manifest =
            parse_and_validate_manifest_v1(ADD32_LIFECYCLE).unwrap();
        let report = run_scenario_manifest_v1(
            &manifest,
            ScenarioBackendV1::OptimizedCpu,
        )
        .unwrap();

        assert!(report.overall_pass);
        assert_eq!(report.runs.len(), 4);
        assert!(report.runs.iter().all(|run| {
            run.observed.session_id == report.session_id
        }));
        assert!(report.runs.iter().all(|run| {
            run.observed.active_reaction_count == 609
        }));
        assert!(report.runs.iter().all(|run| {
            run.observed.final_quiescent
        }));
        assert!(report.runs.iter().all(|run| {
            run.oracle_matches == Some(true)
                && run.fresh_instance_matches == Some(true)
                && run.scalar_oracle_matches
        }));
        assert_eq!(
            report.runs[1].result.fields.get("value"),
            Some(&Value::String("0x00000000".to_owned())),
        );
        assert_eq!(
            report.runs[2].result.fields.get("value"),
            Some(&Value::String("0x80000000".to_owned())),
        );
        assert_eq!(report.runs[0].result, report.runs[3].result);
        assert!(
            report.runs[3].configuration_reused,
            "returning to first ADD32 inputs must reuse canonical Links",
        );
    }

    #[test]
    fn canonical_shl32_manifest_proves_count_masking_in_one_session() {
        let manifest =
            parse_and_validate_manifest_v1(SHL32_LIFECYCLE).unwrap();
        let report = run_scenario_manifest_v1(
            &manifest,
            ScenarioBackendV1::OptimizedCpu,
        )
        .unwrap();

        assert!(report.overall_pass);
        assert_eq!(report.runs.len(), 5);
        assert!(report.runs.iter().all(|run| {
            run.observed.session_id == report.session_id
        }));
        assert_eq!(
            report
                .runs
                .iter()
                .map(|run| run.observed.active_reaction_count)
                .collect::<Vec<_>>(),
            vec![1, 84, 82, 84, 1],
        );
        assert!(report.runs.iter().all(|run| {
            run.observed.final_quiescent
                && run.oracle_matches == Some(true)
                && run.fresh_instance_matches == Some(true)
                && run.scalar_oracle_matches
        }));
        assert_eq!(
            report.runs[1].result,
            report.runs[3].result,
            "COUNT=33 must mask to COUNT=1 semantically",
        );
        assert!(
            !report.runs[3].configuration_reused,
            "COUNT=33 is a distinct structural input even though it masks to 1",
        );
        assert_eq!(report.runs[0].result, report.runs[4].result);
        assert!(
            report.runs[4].configuration_reused,
            "returning to COUNT=0 must reuse canonical configuration Links",
        );
    }

    #[test]
    fn canonical_shift32_family_manifests_share_one_session_model() {
        for source in [SHR32_LIFECYCLE, SAR32_LIFECYCLE] {
            let manifest = parse_and_validate_manifest_v1(source).unwrap();
            let report = run_scenario_manifest_v1(
                &manifest,
                ScenarioBackendV1::OptimizedCpu,
            )
            .unwrap();

            assert!(report.overall_pass, "{}", manifest.scenario_id);
            assert_eq!(report.runs.len(), 5, "{}", manifest.scenario_id);
            assert_eq!(
                report
                    .runs
                    .iter()
                    .map(|run| run.observed.active_reaction_count)
                    .collect::<Vec<_>>(),
                vec![1, 82, 1, 82, 1],
                "{} reaction profile",
                manifest.scenario_id,
            );
            assert!(report.runs.iter().all(|run| {
                run.observed.session_id == report.session_id
                    && run.observed.final_quiescent
                    && run.oracle_matches == Some(true)
                    && run.fresh_instance_matches == Some(true)
                    && run.scalar_oracle_matches
            }), "{}", manifest.scenario_id);

            assert_eq!(
                report.runs[0].result,
                report.runs[2].result,
                "{} COUNT=32 must mask to zero",
                manifest.scenario_id,
            );
            assert!(
                !report.runs[2].configuration_reused,
                "{} COUNT=32 is a distinct structural input",
                manifest.scenario_id,
            );
            assert_eq!(
                report.runs[1].result,
                report.runs[3].result,
                "{} COUNT=33 must mask to one",
                manifest.scenario_id,
            );
            assert!(
                !report.runs[3].configuration_reused,
                "{} COUNT=33 is a distinct structural input",
                manifest.scenario_id,
            );
            assert_eq!(
                report.runs[0].result,
                report.runs[4].result,
                "{} return-to-first result",
                manifest.scenario_id,
            );
            assert!(
                report.runs[4].configuration_reused,
                "{} return-to-first must reuse canonical Links",
                manifest.scenario_id,
            );
        }
    }

    #[test]
    fn canonical_rotate32_family_manifests_share_one_session_model() {
        for source in [ROL32_LIFECYCLE, ROR32_LIFECYCLE] {
            let manifest = parse_and_validate_manifest_v1(source).unwrap();
            let report = run_scenario_manifest_v1(
                &manifest,
                ScenarioBackendV1::OptimizedCpu,
            )
            .unwrap();

            assert!(report.overall_pass, "{}", manifest.scenario_id);
            assert_eq!(report.runs.len(), 6, "{}", manifest.scenario_id);
            assert_eq!(
                report
                    .runs
                    .iter()
                    .map(|run| run.observed.active_reaction_count)
                    .collect::<Vec<_>>(),
                vec![1, 3, 1, 1, 3, 1],
                "{} reaction profile",
                manifest.scenario_id,
            );
            assert!(report.runs.iter().all(|run| {
                run.observed.session_id == report.session_id
                    && run.observed.final_quiescent
                    && run.oracle_matches == Some(true)
                    && run.fresh_instance_matches == Some(true)
                    && run.scalar_oracle_matches
            }), "{}", manifest.scenario_id);

            assert_eq!(
                report.runs[0].result,
                report.runs[3].result,
                "{} COUNT=32 must mask to zero",
                manifest.scenario_id,
            );
            assert!(!report.runs[3].configuration_reused);
            assert_eq!(
                report.runs[1].result,
                report.runs[4].result,
                "{} COUNT=33 must mask to one",
                manifest.scenario_id,
            );
            assert!(!report.runs[4].configuration_reused);
            assert_eq!(report.runs[0].result, report.runs[5].result);
            assert!(
                report.runs[5].configuration_reused,
                "{} return-to-first must reuse canonical Links",
                manifest.scenario_id,
            );
        }
    }

    #[test]
    fn canonical_rotate_carry32_family_manifests_share_one_session_model() {
        for source in [RCL32_LIFECYCLE, RCR32_LIFECYCLE] {
            let manifest = parse_and_validate_manifest_v1(source).unwrap();
            let report = run_scenario_manifest_v1(
                &manifest,
                ScenarioBackendV1::OptimizedCpu,
            )
            .unwrap();

            assert!(report.overall_pass, "{}", manifest.scenario_id);
            assert_eq!(report.runs.len(), 7, "{}", manifest.scenario_id);
            assert_eq!(
                report
                    .runs
                    .iter()
                    .map(|run| run.observed.active_reaction_count)
                    .collect::<Vec<_>>(),
                vec![1, 3, 3, 1, 1, 3, 1],
                "{} reaction profile",
                manifest.scenario_id,
            );
            assert!(report.runs.iter().all(|run| {
                run.observed.session_id == report.session_id
                    && run.observed.final_quiescent
                    && run.oracle_matches == Some(true)
                    && run.fresh_instance_matches == Some(true)
                    && run.scalar_oracle_matches
            }), "{}", manifest.scenario_id);

            assert_eq!(report.runs[0].result, report.runs[4].result);
            assert!(!report.runs[4].configuration_reused);
            assert_eq!(report.runs[2].result, report.runs[5].result);
            assert!(!report.runs[5].configuration_reused);
            assert_eq!(report.runs[0].result, report.runs[6].result);
            assert!(
                report.runs[6].configuration_reused,
                "{} return-to-first must reuse canonical Links",
                manifest.scenario_id,
            );
        }
    }

    #[test]
    fn canonical_unary32_family_manifests_share_one_session_model() {
        for source in [
            INC32_LIFECYCLE,
            DEC32_LIFECYCLE,
            NEG32_LIFECYCLE,
        ] {
            let manifest = parse_and_validate_manifest_v1(source).unwrap();
            let report = run_scenario_manifest_v1(
                &manifest,
                ScenarioBackendV1::OptimizedCpu,
            )
            .unwrap();

            assert!(report.overall_pass, "{}", manifest.scenario_id);
            assert_eq!(report.runs.len(), 5, "{}", manifest.scenario_id);
            assert!(report.runs.iter().all(|run| {
                run.observed.session_id == report.session_id
                    && run.observed.active_reaction_count == 609
                    && run.observed.final_quiescent
                    && run.oracle_matches == Some(true)
                    && run.fresh_instance_matches == Some(true)
                    && run.scalar_oracle_matches
            }), "{}", manifest.scenario_id);

            assert_eq!(
                report.runs[0].result,
                report.runs[4].result,
                "{} return-to-first result",
                manifest.scenario_id,
            );
            assert!(
                report.runs[4].configuration_reused,
                "{} return-to-first must reuse canonical Links",
                manifest.scenario_id,
            );
        }
    }

    #[test]
    fn canonical_mul_effect32_manifest_proves_effect_in_one_session() {
        let manifest =
            parse_and_validate_manifest_v1(MUL_EFFECT32_LIFECYCLE).unwrap();
        let report = run_scenario_manifest_v1(
            &manifest,
            ScenarioBackendV1::OptimizedCpu,
        )
        .unwrap();

        assert!(report.overall_pass);
        assert_eq!(report.runs.len(), 5);
        assert_eq!(
            report
                .runs
                .iter()
                .map(|run| run.observed.active_reaction_count)
                .collect::<Vec<_>>(),
            vec![97, 1135, 1135, 1135, 97],
        );
        assert!(report.runs.iter().all(|run| {
            run.observed.session_id == report.session_id
                && run.observed.final_quiescent
                && run.oracle_matches == Some(true)
                && run.fresh_instance_matches == Some(true)
                && run.scalar_oracle_matches
        }));
        assert_eq!(
            report.runs[2].result.fields.get("hi"),
            Some(&Value::String("0x00000001".to_owned())),
        );
        assert_eq!(
            report.runs[2].result.fields.get("valueMask"),
            Some(&Value::String("0x00000801".to_owned())),
        );
        assert_eq!(
            report.runs[2].result.fields.get("undefinedMask"),
            Some(&Value::String("0x000000d4".to_owned())),
        );
        assert_eq!(report.runs[0].result, report.runs[4].result);
        assert!(
            report.runs[4].configuration_reused,
            "returning to first MUL-effect inputs must reuse canonical Links",
        );
    }

    #[test]
    fn canonical_mux32_manifest_completes_maintained_corpus() {
        let manifest =
            parse_and_validate_manifest_v1(MUX32_LIFECYCLE).unwrap();
        let report = run_scenario_manifest_v1(
            &manifest,
            ScenarioBackendV1::OptimizedCpu,
        )
        .unwrap();

        assert!(report.overall_pass);
        assert_eq!(report.runs.len(), 5);
        assert!(report.runs.iter().all(|run| {
            run.observed.session_id == report.session_id
                && run.observed.active_reaction_count == 257
                && run.observed.final_quiescent
                && run.oracle_matches == Some(true)
                && run.fresh_instance_matches == Some(true)
                && run.scalar_oracle_matches
        }));
        assert_eq!(
            report.runs[0].result.fields.get("value"),
            Some(&Value::String("0x00000000".to_owned())),
        );
        assert_eq!(
            report.runs[1].result.fields.get("value"),
            Some(&Value::String("0xffffffff".to_owned())),
        );
        assert_eq!(
            report.runs[2].result.fields.get("value"),
            Some(&Value::String("0x12345678".to_owned())),
        );
        assert_eq!(
            report.runs[3].result.fields.get("value"),
            Some(&Value::String("0x9abcdef0".to_owned())),
        );
        assert_eq!(report.runs[0].result, report.runs[4].result);
        assert!(
            report.runs[4].configuration_reused,
            "returning to first MUX32 inputs must reuse canonical Links",
        );
    }

    #[test]
    fn canonical_mul32_manifest_proves_wide64_in_one_session() {
        let manifest =
            parse_and_validate_manifest_v1(MUL32_LIFECYCLE).unwrap();
        let report = run_scenario_manifest_v1(
            &manifest,
            ScenarioBackendV1::OptimizedCpu,
        )
        .unwrap();

        assert!(report.overall_pass);
        assert_eq!(report.runs.len(), 4);
        assert!(report.runs.iter().all(|run| {
            run.observed.session_id == report.session_id
                && run.observed.final_quiescent
                && run.oracle_matches == Some(true)
                && run.fresh_instance_matches == Some(true)
                && run.scalar_oracle_matches
        }));
        assert_eq!(
            report
                .runs
                .iter()
                .map(|run| run.observed.active_reaction_count)
                .collect::<Vec<_>>(),
            vec![33, 1071, 1071, 33],
        );
        assert_eq!(
            report.runs[2].result.fields.get("lo"),
            Some(&Value::String("0x00000000".to_owned())),
        );
        assert_eq!(
            report.runs[2].result.fields.get("hi"),
            Some(&Value::String("0x00000001".to_owned())),
        );
        assert_eq!(report.runs[0].result, report.runs[3].result);
        assert!(
            report.runs[3].configuration_reused,
            "returning to first MUL32 inputs must reuse canonical Links",
        );
    }

    #[test]
    fn canonical_m6a_radix_memory_manifest_runs_through_scenario_session() {
        let manifest =
            parse_and_validate_manifest_v1(RADIX_MEMORY8_LIFECYCLE).unwrap();
        let report = run_scenario_manifest_v1(
            &manifest,
            ScenarioBackendV1::OptimizedCpu,
        )
        .unwrap();

        assert!(report.overall_pass);
        assert_eq!(report.runs.len(), 4);
        assert!(report.runs.iter().all(|run| {
            run.observed.session_id == report.session_id
                && run.observed.final_quiescent
                && run.observed.active_reaction_count > 40
                && run.oracle_matches == Some(true)
                && run.fresh_instance_matches == Some(true)
                && run.scalar_oracle_matches
        }));
        assert_eq!(
            report.runs[0].result.fields.get("after"),
            Some(&Value::from(0xabu32)),
        );
        assert_eq!(
            report.runs[0].result.fields.get("oldAfter"),
            Some(&Value::from(0u32)),
        );
        assert_eq!(report.runs[0].result, report.runs[3].result);
        assert!(
            report.runs[3].configuration_reused,
            "returning to first M6A inputs must reuse canonical Links",
        );
    }

    fn configured_mux1_for_budget_test(
    ) -> (CpuRuntimeSession, WebProofLoadStage, Handle) {
        let prepare = prepare_mux1_session_program().unwrap();
        let (mut session, load) =
            load_runtime_session(&prepare, 32).unwrap();
        let (initial, _, _) =
            configure_mux1_session(&mut session, &load, 1, 0, 1)
                .unwrap();
        (session, load, initial)
    }

    #[test]
    fn total_links_budget_stops_before_semantic_step() {
        let (mut session, _load, initial) =
            configured_mux1_for_budget_test();
        let current_links = session.memory.store.link_count() as u32;
        let budget = RunBudgetV1 {
            max_total_links: current_links.saturating_sub(1),
            ..RunBudgetV1::scenario_default(64)
        };
        let mut controller =
            session.begin_budgeted_run(initial, budget).unwrap();
        let stopped =
            controller.next(
                &mut session,
                RunObservationLevel::Off.runtime_trace_mode(),
            );
        assert!(stopped.step.is_none());
        assert_eq!(
            stopped.stop_reason,
            Some(RunStopReasonV1::TotalLinksBudgetExceeded),
        );
        assert_eq!(session.runtime_state_v1(), SessionStateV1::Failed);
    }

    #[test]
    fn scope_width_budget_stops_before_semantic_step() {
        let (mut session, _load, initial) =
            configured_mux1_for_budget_test();
        let budget = RunBudgetV1 {
            max_scope_width: 0,
            ..RunBudgetV1::scenario_default(64)
        };
        let mut controller =
            session.begin_budgeted_run(initial, budget).unwrap();
        let stopped =
            controller.next(
                &mut session,
                RunObservationLevel::Off.runtime_trace_mode(),
            );
        assert!(stopped.step.is_none());
        assert_eq!(
            stopped.stop_reason,
            Some(RunStopReasonV1::ScopeWidthBudgetExceeded),
        );
        assert_eq!(session.runtime_state_v1(), SessionStateV1::Failed);
    }

    #[test]
    fn appended_links_budget_stops_after_bounded_semantic_step() {
        let (mut session, _load, initial) =
            configured_mux1_for_budget_test();
        let budget = RunBudgetV1 {
            max_appended_links: 0,
            ..RunBudgetV1::scenario_default(64)
        };
        let stopped = session
            .run_to_quiescence(
                initial,
                budget,
                RunObservationLevel::Off.runtime_trace_mode(),
            )
            .unwrap();
        assert!(!stopped.steps.is_empty());
        assert_eq!(
            stopped.stop_reason,
            RunStopReasonV1::AppendedLinksBudgetExceeded,
        );
        assert_eq!(session.runtime_state_v1(), SessionStateV1::Failed);
    }

    #[test]
    fn dense_carrier_byte_budget_stops_after_atomic_growth() {
        let (mut session, _load, initial) =
            configured_mux1_for_budget_test();

        // Fill exactly the current measured Vec slack with unrelated canonical
        // START forms. This changes no configured current/theory handles and
        // makes the next appended Link force a real dense-buffer allocation.
        let spare = session
            .memory
            .store
            .dense_carrier_index_min_spare_links();
        let latest = session.memory.store.link_count() as Handle;
        let mut source = session
            .memory
            .store
            .export_anum(latest)
            .unwrap();
        let target_links = session
            .memory
            .store
            .link_count()
            .saturating_add(spare);
        while session.memory.store.link_count() < target_links {
            source.insert(0, '9');
            let before_links = session.memory.store.link_count();
            session.memory.store.import_anum(&source).unwrap();
            let after_links = session.memory.store.link_count();
            assert!(
                after_links == before_links || after_links == before_links + 1,
                "one nested START import may append at most one new outer form",
            );
        }
        assert_eq!(
            session
                .memory
                .store
                .dense_carrier_index_min_spare_links(),
            0,
        );

        let before = session
            .memory
            .store
            .dense_carrier_index_allocated_bytes();
        let budget = RunBudgetV1::scenario_default(64);
        let stopped = session
            .run_to_quiescence_with_resource_budget(
                initial,
                budget,
                CpuRunResourceBudgetV1 {
                    max_dense_carrier_bytes: before,
                },
                RunObservationLevel::Off.runtime_trace_mode(),
            )
            .unwrap();

        assert!(!stopped.steps.is_empty());
        assert_eq!(
            stopped.stop_reason,
            RunStopReasonV1::CarrierBytesBudgetExceeded,
        );
        match stopped.budget_accounting.backend_resource {
            Some(
                BackendResourceAccountingV1::OptimizedCpuDenseCarrier {
                    dense_carrier_allocated_bytes,
                    max_dense_carrier_bytes,
                    full_resident_bytes_available,
                },
            ) => {
                assert!(dense_carrier_allocated_bytes > before);
                assert_eq!(max_dense_carrier_bytes, before);
                assert!(!full_resident_bytes_available);
            }
            None => panic!(
                "optimized CPU run lost dense-carrier resource accounting"
            ),
        }
        assert_eq!(session.runtime_state_v1(), SessionStateV1::Failed);
    }

    #[test]
    fn match_work_budget_stops_after_atomic_reaction_with_observation_off() {
        let (mut session, _load, initial) =
            configured_mux1_for_budget_test();
        let budget = RunBudgetV1 {
            max_match_candidates: 0,
            ..RunBudgetV1::scenario_default(64)
        };
        let stopped = session
            .run_to_quiescence(
                initial,
                budget,
                RunObservationLevel::Off.runtime_trace_mode(),
            )
            .unwrap();

        assert!(!stopped.steps.is_empty());
        assert!(
            stopped.steps[0]
                .structural_profile
                .trigger_incidence_candidates
                > 0,
        );
        assert_eq!(
            stopped.stop_reason,
            RunStopReasonV1::MatchWorkBudgetExceeded,
        );
        assert_eq!(stopped.budget_accounting.max_match_candidates, 0);
        assert_eq!(
            stopped.budget_accounting.match_candidates,
            stopped.steps[0]
                .structural_profile
                .trigger_incidence_candidates,
        );
        assert!(stopped.budget_accounting.match_candidates > 0);
        assert_eq!(session.runtime_state_v1(), SessionStateV1::Failed);
    }

    #[test]
    fn unification_work_budget_stops_from_authoritative_profile() {
        let (mut session, _load, initial) =
            configured_mux1_for_budget_test();
        let budget = RunBudgetV1 {
            max_unification_nodes: 0,
            ..RunBudgetV1::scenario_default(64)
        };
        let stopped = session
            .run_to_quiescence(
                initial,
                budget,
                RunObservationLevel::Off.runtime_trace_mode(),
            )
            .unwrap();

        assert!(!stopped.steps.is_empty());
        assert!(
            stopped.steps[0]
                .structural_profile
                .unification_nodes_visited
                > 0,
        );
        assert_eq!(
            stopped.stop_reason,
            RunStopReasonV1::UnificationWorkBudgetExceeded,
        );
        assert_eq!(stopped.budget_accounting.max_unification_nodes, 0);
        assert_eq!(
            stopped.budget_accounting.unification_nodes,
            stopped.steps[0]
                .structural_profile
                .unification_nodes_visited,
        );
        assert!(stopped.budget_accounting.unification_nodes > 0);
        assert_eq!(session.runtime_state_v1(), SessionStateV1::Failed);
    }

    #[test]
    fn instantiation_work_budget_stops_from_authoritative_profile() {
        let (mut session, _load, initial) =
            configured_mux1_for_budget_test();
        let budget = RunBudgetV1 {
            max_instantiation_nodes: 0,
            ..RunBudgetV1::scenario_default(64)
        };
        let stopped = session
            .run_to_quiescence(
                initial,
                budget,
                RunObservationLevel::Off.runtime_trace_mode(),
            )
            .unwrap();

        assert!(!stopped.steps.is_empty());
        assert!(
            stopped.steps[0]
                .structural_profile
                .instantiation_nodes_visited
                > 0,
        );
        assert_eq!(
            stopped.stop_reason,
            RunStopReasonV1::InstantiationWorkBudgetExceeded,
        );
        assert_eq!(stopped.budget_accounting.max_instantiation_nodes, 0);
        assert_eq!(
            stopped.budget_accounting.instantiation_nodes,
            stopped.steps[0]
                .structural_profile
                .instantiation_nodes_visited,
        );
        assert!(stopped.budget_accounting.instantiation_nodes > 0);
        assert_eq!(session.runtime_state_v1(), SessionStateV1::Failed);
    }

    #[test]
    fn reaction_budget_is_safety_only_and_never_partial_success() {
        let mut manifest =
            parse_and_validate_manifest_v1(XOR32_LIFECYCLE).unwrap();
        manifest.run_sequence.truncate(1);
        manifest.run_sequence[0].max_reactions = 1;

        let error = run_scenario_manifest_v1(
            &manifest,
            ScenarioBackendV1::OptimizedCpu,
        )
        .unwrap_err();

        match error {
            ScenarioRunnerErrorV1::ExecuteFailed {
                run_id,
                max_reactions,
                stop_reason,
                budget_accounting: Some(accounting),
            } => {
                assert_eq!(run_id, "run-1-zero-ones");
                assert_eq!(max_reactions, 1);
                assert_eq!(
                    stop_reason,
                    RunStopReasonV1::ReactionBudgetExceeded,
                );
                assert_eq!(accounting.reactions_consumed, 1);
                assert_eq!(accounting.max_reactions, 1);
                assert!(accounting.total_links > 0);
                assert!(accounting.max_total_links >= accounting.total_links);
                assert!(matches!(
                    accounting.backend_resource,
                    Some(
                        BackendResourceAccountingV1::OptimizedCpuDenseCarrier {
                            full_resident_bytes_available: false,
                            ..
                        },
                    )
                ));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn wrong_assertion_does_not_drive_execution() {
        let mut manifest =
            parse_and_validate_manifest_v1(MUX1_LIFECYCLE).unwrap();
        manifest.run_sequence.truncate(1);
        manifest.run_sequence[0].assertions[0] =
            ScenarioAssertionV1::ResultFieldEquals {
                field: "value".to_owned(),
                expected: Value::from(1u32),
            };

        let report = run_scenario_manifest_v1(
            &manifest,
            ScenarioBackendV1::OptimizedCpu,
        )
        .unwrap();

        assert!(!report.overall_pass);
        assert_eq!(
            report.runs[0].result.fields.get("value"),
            Some(&Value::from(0u32)),
            "wrong expected value must not alter semantic result",
        );
        assert!(!report.runs[0].assertion_results[0].passed);
        assert_eq!(report.runs[0].oracle_matches, Some(true));
        assert_eq!(report.runs[0].fresh_instance_matches, Some(true));
        assert!(report.runs[0].scalar_oracle_matches);
    }

    #[test]
    fn unsupported_backend_fails_without_cpu_fallback() {
        let manifest =
            parse_and_validate_manifest_v1(MUX1_LIFECYCLE).unwrap();

        let error = run_scenario_manifest_v1(
            &manifest,
            ScenarioBackendV1::Webgpu,
        )
        .unwrap_err();

        assert_eq!(
            error,
            ScenarioRunnerErrorV1::UnsupportedBackend {
                backend: ScenarioBackendV1::Webgpu,
            },
        );
    }
}
