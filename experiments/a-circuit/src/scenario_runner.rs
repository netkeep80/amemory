use super::{
    arithmetic_effect_n::{
        configure_add32_session, prepare_add32_session_program,
        project_add32_session_result, web_prove_arithmetic,
    },
    logic_effect_n::{
        configure_xor32_session, prepare_xor32_session_program,
        project_xor32_session_result, web_prove_logic,
    },
    mux_n::{
        configure_mux1_session, prepare_mux1_session_program,
        project_mux1_session_result, web_prove_mux1,
    },
    observability::{
        run_pipeline_profile_v1, session_open_profile_v1, time_stage,
        ObservedRunV1, RunPipelineProfileV1, SessionOpenProfileV1,
    },
    proof_n::{
        execute_session_observed_to_quiescence, load_runtime_session,
        ProofRuntimeSession, WebProofLoadStage, WebProofPrepareStage,
    },
    scenario::{
        validate_manifest_v1, ScenarioAssertionV1, ScenarioBackendV1,
        ScenarioExecutionModeV1, ScenarioManifestV1, ScenarioOraclePolicyV1,
        ScenarioProgramProfileV1, ScenarioValidationErrorV1,
    },
};
use amemory_optimized_cpu_probe::Handle;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

pub(crate) const SCENARIO_REPORT_SCHEMA_VERSION: u32 = 1;
const OPTIMIZED_CPU_BACKEND_ID: &str = "optimized-cpu";

type PrepareFn = fn() -> Option<WebProofPrepareStage>;
type ConfigureFn = fn(
    &mut ProofRuntimeSession,
    &WebProofLoadStage,
    &BTreeMap<String, Value>,
) -> Result<ConfiguredRun, String>;
type ProjectFn = fn(
    &ProofRuntimeSession,
    &WebProofLoadStage,
) -> Result<ScenarioNormalizedResultV1, String>;
type OracleFn = fn(
    &BTreeMap<String, Value>,
) -> Result<ScenarioNormalizedResultV1, String>;

struct CpuScenarioAdapter {
    profile_id: &'static str,
    prepare: PrepareFn,
    configure: ConfigureFn,
    project: ProjectFn,
    oracle: OracleFn,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ConfiguredRun {
    initial: Handle,
    links_before: u32,
    links_after: u32,
}

const CPU_SCENARIO_ADAPTERS: &[CpuScenarioAdapter] = &[
    CpuScenarioAdapter {
        profile_id: "a-circuit:mux1",
        prepare: prepare_mux1_session_program,
        configure: configure_mux1_from_inputs,
        project: project_mux1_result,
        oracle: oracle_mux1_result,
    },
    CpuScenarioAdapter {
        profile_id: "a-circuit:logic-xor32",
        prepare: prepare_xor32_session_program,
        configure: configure_xor32_from_inputs,
        project: project_xor32_result,
        oracle: oracle_xor32_result,
    },
    CpuScenarioAdapter {
        profile_id: "a-circuit:arithmetic-add32",
        prepare: prepare_add32_session_program,
        configure: configure_add32_from_inputs,
        project: project_add32_result,
        oracle: oracle_add32_result,
    },
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioNormalizedResultV1 {
    pub(crate) fields: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) result_recursive_wire: Option<String>,
}

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) oracle_matches: Option<bool>,
    pub(crate) observed: ObservedRunV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) pipeline_profile: Option<RunPipelineProfileV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioProvenanceV1 {
    pub(crate) amemory_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) build_sha: Option<String>,
    pub(crate) scenario_version: String,
    pub(crate) program_profile_id: String,
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
    pub(crate) backend: ScenarioBackendV1,
    pub(crate) session_id: String,
    pub(crate) session_open_profile: SessionOpenProfileV1,
    pub(crate) runs: Vec<ScenarioRunReportV1>,
    pub(crate) overall_pass: bool,
    pub(crate) provenance: ScenarioProvenanceV1,
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

    if !manifest.supported_backends.contains(&backend)
        || backend != ScenarioBackendV1::OptimizedCpu
    {
        return Err(ScenarioRunnerErrorV1::UnsupportedBackend { backend });
    }

    let adapter = CPU_SCENARIO_ADAPTERS
        .iter()
        .find(|adapter| adapter.profile_id == manifest.program_profile.profile_id)
        .ok_or_else(|| ScenarioRunnerErrorV1::UnsupportedProgramProfile {
            profile_id: manifest.program_profile.profile_id.clone(),
        })?;

    let (prepare, prepare_ns) = time_stage(|| (adapter.prepare)());
    let prepare = prepare.ok_or_else(|| ScenarioRunnerErrorV1::PrepareFailed {
        profile_id: manifest.program_profile.profile_id.clone(),
    })?;
    let prepared_links = prepare.compiled_links;

    let (loaded, load_ns) =
        time_stage(|| load_runtime_session(&prepare, 32));
    let (mut session, load) = loaded.ok_or_else(|| {
        ScenarioRunnerErrorV1::LoadFailed {
            profile_id: manifest.program_profile.profile_id.clone(),
        }
    })?;

    let session_open_profile = session_open_profile_v1(
        session.memory.id.clone(),
        prepare_ns,
        load_ns,
        prepared_links,
        session.base_link_count as u32,
    );
    let session_id = session.memory.id.clone();
    let loaded_link_count = session.base_link_count;
    let loaded_prefix = session.memory.store.export_packed_duplets();
    let mut reports = Vec::with_capacity(manifest.run_sequence.len());

    for run in &manifest.run_sequence {
        if run.execution_mode != ScenarioExecutionModeV1::ToQuiescence {
            return Err(ScenarioRunnerErrorV1::UnsupportedExecutionMode {
                run_id: run.run_id.clone(),
                mode: run.execution_mode,
            });
        }

        // Adapter receives inputs only. Assertions/oracle expected values are
        // structurally unavailable to CONFIGURE and therefore cannot become
        // execution authority.
        let (configured, configure_ns) =
            time_stage(|| (adapter.configure)(
                &mut session,
                &load,
                &run.inputs,
            ));
        let configured = configured.map_err(|message| {
            ScenarioRunnerErrorV1::ConfigureFailed {
                run_id: run.run_id.clone(),
                message,
            }
        })?;

        let observed = execute_session_observed_to_quiescence(
            &mut session,
            configured.initial,
            run.max_reactions,
            manifest.observation_level,
        )
        .ok_or_else(|| ScenarioRunnerErrorV1::ExecuteFailed {
            run_id: run.run_id.clone(),
            max_reactions: run.max_reactions,
        })?;

        let links_before_result = session.memory.store.link_count();
        let (projected, result_ns) =
            time_stage(|| (adapter.project)(&session, &load));
        let result = projected.map_err(|message| {
            ScenarioRunnerErrorV1::ProjectResultFailed {
                run_id: run.run_id.clone(),
                message,
            }
        })?;
        if session.memory.store.link_count() != links_before_result {
            return Err(
                ScenarioRunnerErrorV1::ResultProjectionMutatedCarrier {
                    run_id: run.run_id.clone(),
                },
            );
        }

        // Assertions are evaluated only after semantic execution and RESULT
        // projection. Their expected values never enter CONFIGURE/EXECUTE.
        let assertion_results =
            evaluate_assertions(&run.assertions, &result, &observed);

        let oracle_matches = match manifest.oracle_policy {
            ScenarioOraclePolicyV1::FreshInstance => {
                let oracle = (adapter.oracle)(&run.inputs).map_err(|message| {
                    ScenarioRunnerErrorV1::OracleFailed {
                        run_id: run.run_id.clone(),
                        message,
                    }
                })?;
                Some(oracle == result)
            }
            ScenarioOraclePolicyV1::None
            | ScenarioOraclePolicyV1::ExpectedAssertions => None,
        };

        let links_before_evidence = session.memory.store.link_count();
        let (_, external_evidence_ns) = time_stage(|| {
            serde_json::to_string(&(&observed, &result, &assertion_results))
                .expect("serializable scenario evidence")
        });
        if session.memory.store.link_count() != links_before_evidence {
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

        let carrier = session.memory.store.export_packed_duplets();
        if carrier.len() < loaded_link_count
            || &carrier[..loaded_link_count] != loaded_prefix.as_slice()
        {
            return Err(ScenarioRunnerErrorV1::LoadedBaseMutated {
                run_id: run.run_id.clone(),
            });
        }

        reports.push(ScenarioRunReportV1 {
            manifest_run_id: run.run_id.clone(),
            session_run_id: observed.run_id,
            inputs: run.inputs.clone(),
            configuration_reused:
                configured.links_after == configured.links_before,
            links_before_configure: configured.links_before,
            links_after_configure: configured.links_after,
            result,
            assertion_results,
            oracle_matches,
            observed,
            pipeline_profile,
        });
    }

    let overall_pass = reports.iter().all(|run| {
        run.assertion_results
            .iter()
            .all(|assertion| assertion.passed)
            && run.oracle_matches != Some(false)
    });

    Ok(ScenarioExecutionReportV1 {
        schema_version: SCENARIO_REPORT_SCHEMA_VERSION,
        scenario_id: manifest.scenario_id.clone(),
        scenario_version: manifest.scenario_version.clone(),
        program_profile: manifest.program_profile.clone(),
        backend,
        session_id,
        session_open_profile,
        runs: reports,
        overall_pass,
        provenance: ScenarioProvenanceV1 {
            amemory_version: include_str!("../../../VERSION")
                .trim()
                .to_owned(),
            build_sha: option_env!("AMEMORY_BUILD_SHA")
                .or(option_env!("GITHUB_SHA"))
                .map(str::to_owned),
            scenario_version: manifest.scenario_version.clone(),
            program_profile_id: manifest.program_profile.profile_id.clone(),
            // Current prepare DTO has no stable cryptographic program digest.
            // Do not invent one; a later pipeline slice will expose it.
            program_fingerprint: None,
        },
    })
}

fn evaluate_assertions(
    assertions: &[ScenarioAssertionV1],
    result: &ScenarioNormalizedResultV1,
    observed: &ObservedRunV1,
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
                let actual = Value::Bool(observed.final_quiescent);
                ScenarioAssertionResultV1 {
                    assertion_index: index as u32,
                    kind: "QUIESCENT_EQUALS".to_owned(),
                    passed: observed.final_quiescent == *expected,
                    field: None,
                    expected: Some(Value::Bool(*expected)),
                    actual: Some(actual),
                }
            }
            ScenarioAssertionV1::ReactionCountEquals { expected } => {
                let actual = Value::from(observed.active_reaction_count);
                ScenarioAssertionResultV1 {
                    assertion_index: index as u32,
                    kind: "REACTION_COUNT_EQUALS".to_owned(),
                    passed: observed.active_reaction_count == *expected,
                    field: None,
                    expected: Some(Value::from(*expected)),
                    actual: Some(actual),
                }
            }
        })
        .collect()
}

fn configure_mux1_from_inputs(
    session: &mut ProofRuntimeSession,
    load: &WebProofLoadStage,
    inputs: &BTreeMap<String, Value>,
) -> Result<ConfiguredRun, String> {
    let select = bit_input(inputs, "S")?;
    let a = bit_input(inputs, "A")?;
    let b = bit_input(inputs, "B")?;
    let (initial, before, after) =
        configure_mux1_session(session, load, select, a, b)
            .ok_or_else(|| "MUX1 configuration failed".to_owned())?;
    Ok(ConfiguredRun {
        initial,
        links_before: before as u32,
        links_after: after as u32,
    })
}

fn project_mux1_result(
    session: &ProofRuntimeSession,
    load: &WebProofLoadStage,
) -> Result<ScenarioNormalizedResultV1, String> {
    let projected = project_mux1_session_result(session, load)
        .ok_or_else(|| "MUX1 result projection failed".to_owned())?;
    let mut fields = BTreeMap::new();
    fields.insert("value".to_owned(), Value::from(projected.value));
    Ok(ScenarioNormalizedResultV1 {
        fields,
        result_recursive_wire: Some(projected.result_recursive_wire),
    })
}

fn oracle_mux1_result(
    inputs: &BTreeMap<String, Value>,
) -> Result<ScenarioNormalizedResultV1, String> {
    let select = bit_input(inputs, "S")? as u32;
    let a = bit_input(inputs, "A")? as u32;
    let b = bit_input(inputs, "B")? as u32;
    let proof = web_prove_mux1(select, a, b)
        .ok_or_else(|| "fresh MUX1 oracle failed".to_owned())?;
    let mut fields = BTreeMap::new();
    fields.insert(
        "value".to_owned(),
        Value::from(proof.result.decoded_value),
    );
    Ok(ScenarioNormalizedResultV1 {
        fields,
        result_recursive_wire: Some(proof.result.result_anum),
    })
}

fn canonical_word32(value: u32) -> Value {
    Value::String(format!("0x{value:08x}"))
}

fn word32_input(
    inputs: &BTreeMap<String, Value>,
    key: &str,
) -> Result<u32, String> {
    let value = inputs
        .get(key)
        .ok_or_else(|| format!("missing WORD32 input {key}"))?;
    if let Some(number) = value.as_u64() {
        return u32::try_from(number)
            .map_err(|_| format!("WORD32 input {key} outside 0..2^32-1"));
    }
    let text = value
        .as_str()
        .ok_or_else(|| format!("WORD32 input {key} must be integer or canonical hex"))?;
    if text.len() != 10
        || !text.starts_with("0x")
        || !text[2..].bytes().all(|byte| {
            byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()
        })
    {
        return Err(format!(
            "WORD32 input {key} must be canonical lowercase 0x........"
        ));
    }
    u32::from_str_radix(&text[2..], 16)
        .map_err(|_| format!("WORD32 input {key} is invalid"))
}

fn effect32_normalized(
    value: u32,
    writeback: u8,
    defined_mask: u32,
    value_mask: u32,
    undefined_mask: u32,
    preserve_mask: u32,
    result_recursive_wire: String,
) -> ScenarioNormalizedResultV1 {
    let mut fields = BTreeMap::new();
    fields.insert("value".to_owned(), canonical_word32(value));
    fields.insert("writeback".to_owned(), Value::from(writeback));
    fields.insert(
        "definedMask".to_owned(),
        canonical_word32(defined_mask),
    );
    fields.insert(
        "valueMask".to_owned(),
        canonical_word32(value_mask),
    );
    fields.insert(
        "undefinedMask".to_owned(),
        canonical_word32(undefined_mask),
    );
    fields.insert(
        "preserveMask".to_owned(),
        canonical_word32(preserve_mask),
    );
    ScenarioNormalizedResultV1 {
        fields,
        result_recursive_wire: Some(result_recursive_wire),
    }
}

fn configure_xor32_from_inputs(
    session: &mut ProofRuntimeSession,
    load: &WebProofLoadStage,
    inputs: &BTreeMap<String, Value>,
) -> Result<ConfiguredRun, String> {
    let a = word32_input(inputs, "A")?;
    let b = word32_input(inputs, "B")?;
    let (initial, before, after) =
        configure_xor32_session(session, load, a, b)
            .ok_or_else(|| "XOR32 configuration failed".to_owned())?;
    Ok(ConfiguredRun {
        initial,
        links_before: before as u32,
        links_after: after as u32,
    })
}

fn project_xor32_result(
    session: &ProofRuntimeSession,
    load: &WebProofLoadStage,
) -> Result<ScenarioNormalizedResultV1, String> {
    let projected = project_xor32_session_result(session, load)
        .ok_or_else(|| "XOR32 result projection failed".to_owned())?;
    Ok(effect32_normalized(
        projected.value,
        projected.writeback,
        projected.defined_mask,
        projected.value_mask,
        projected.undefined_mask,
        projected.preserve_mask,
        projected.result_recursive_wire,
    ))
}

fn oracle_xor32_result(
    inputs: &BTreeMap<String, Value>,
) -> Result<ScenarioNormalizedResultV1, String> {
    let a = word32_input(inputs, "A")?;
    let b = word32_input(inputs, "B")?;
    let proof = web_prove_logic(3, a, b)
        .ok_or_else(|| "fresh XOR32 oracle failed".to_owned())?;
    Ok(effect32_normalized(
        proof.outcome.value,
        proof.outcome.writeback,
        proof.outcome.defined_mask,
        proof.outcome.value_mask,
        proof.outcome.undefined_mask,
        proof.outcome.preserve_mask,
        proof.proof.result.result_anum,
    ))
}

fn configure_add32_from_inputs(
    session: &mut ProofRuntimeSession,
    load: &WebProofLoadStage,
    inputs: &BTreeMap<String, Value>,
) -> Result<ConfiguredRun, String> {
    let a = word32_input(inputs, "A")?;
    let b = word32_input(inputs, "B")?;
    let (initial, before, after) =
        configure_add32_session(session, load, a, b)
            .ok_or_else(|| "ADD32 configuration failed".to_owned())?;
    Ok(ConfiguredRun {
        initial,
        links_before: before as u32,
        links_after: after as u32,
    })
}

fn project_add32_result(
    session: &ProofRuntimeSession,
    load: &WebProofLoadStage,
) -> Result<ScenarioNormalizedResultV1, String> {
    let projected = project_add32_session_result(session, load)
        .ok_or_else(|| "ADD32 result projection failed".to_owned())?;
    Ok(effect32_normalized(
        projected.value,
        projected.writeback,
        projected.defined_mask,
        projected.value_mask,
        projected.undefined_mask,
        projected.preserve_mask,
        projected.result_recursive_wire,
    ))
}

fn oracle_add32_result(
    inputs: &BTreeMap<String, Value>,
) -> Result<ScenarioNormalizedResultV1, String> {
    let a = word32_input(inputs, "A")?;
    let b = word32_input(inputs, "B")?;
    let proof = web_prove_arithmetic(6, a, b, 0)
        .ok_or_else(|| "fresh ADD32 oracle failed".to_owned())?;
    Ok(effect32_normalized(
        proof.outcome.value,
        proof.outcome.writeback,
        proof.outcome.defined_mask,
        proof.outcome.value_mask,
        proof.outcome.undefined_mask,
        proof.outcome.preserve_mask,
        proof.proof.result.result_anum,
    ))
}

fn bit_input(
    inputs: &BTreeMap<String, Value>,
    key: &str,
) -> Result<usize, String> {
    let value = inputs
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("missing BIT input {key}"))?;
    if value > 1 {
        return Err(format!("BIT input {key} outside 0..1"));
    }
    Ok(value as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario::parse_and_validate_manifest_v1;

    const MUX1_LIFECYCLE: &str =
        include_str!("../scenarios/mux1-lifecycle-v1.json");
    const XOR32_LIFECYCLE: &str =
        include_str!("../scenarios/xor32-lifecycle-v1.json");
    const ADD32_LIFECYCLE: &str =
        include_str!("../scenarios/add32-lifecycle-v1.json");

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
                .all(|run| run.oracle_matches == Some(true))
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

        assert_eq!(
            error,
            ScenarioRunnerErrorV1::ExecuteFailed {
                run_id: "run-1-zero-ones".to_owned(),
                max_reactions: 1,
            },
        );
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
    }

    #[test]
    fn unsupported_backend_fails_without_cpu_fallback() {
        let mut manifest =
            parse_and_validate_manifest_v1(MUX1_LIFECYCLE).unwrap();
        manifest.supported_backends.push(ScenarioBackendV1::Webgpu);

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
