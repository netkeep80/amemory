use super::observability::RunObservationLevel;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const SCENARIO_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioManifestV1 {
    pub(crate) schema_version: u32,
    pub(crate) scenario_id: String,
    pub(crate) scenario_version: String,
    pub(crate) title: String,
    pub(crate) description: String,
    pub(crate) program_profile: ScenarioProgramProfileV1,
    pub(crate) input_schema: Vec<ScenarioInputSpecV1>,
    pub(crate) initial_inputs: BTreeMap<String, Value>,
    pub(crate) run_sequence: Vec<ScenarioRunV1>,
    pub(crate) supported_backends: Vec<ScenarioBackendV1>,
    pub(crate) oracle_policy: ScenarioOraclePolicyV1,
    pub(crate) invariants: Vec<String>,
    pub(crate) observation_level: RunObservationLevel,
    pub(crate) profiling_policy: ScenarioProfilingPolicyV1,
    pub(crate) visualization_profile: ScenarioVisualizationProfileV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioProgramProfileV1 {
    pub(crate) profile_id: String,
    pub(crate) family: String,
    pub(crate) program_id: String,
    pub(crate) aset_source: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum ScenarioInputTypeV1 {
    Bit,
    Word32,
    Count8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioInputSpecV1 {
    pub(crate) key: String,
    #[serde(rename = "type")]
    pub(crate) input_type: ScenarioInputTypeV1,
    pub(crate) description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) default: Option<Value>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum ScenarioExecutionModeV1 {
    Step,
    ToQuiescence,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioRunV1 {
    pub(crate) run_id: String,
    pub(crate) inputs: BTreeMap<String, Value>,
    pub(crate) execution_mode: ScenarioExecutionModeV1,
    #[serde(default)]
    pub(crate) assertions: Vec<ScenarioAssertionV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum ScenarioAssertionV1 {
    ResultFieldEquals {
        field: String,
        expected: Value,
    },
    QuiescentEquals {
        expected: bool,
    },
    ReactionCountEquals {
        expected: u32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ScenarioBackendV1 {
    OptimizedCpu,
    Webgpu,
    Linksdb,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum ScenarioOraclePolicyV1 {
    None,
    FreshInstance,
    ExpectedAssertions,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum ScenarioProfilingPolicyV1 {
    Off,
    Stages,
    Full,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ScenarioVisualizationProfileV1 {
    Simple,
    Engineering,
    Proof,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioValidationErrorV1 {
    pub(crate) code: String,
    pub(crate) path: String,
    pub(crate) message: String,
}

pub(crate) fn parse_and_validate_manifest_v1(
    source: &str,
) -> Result<ScenarioManifestV1, Vec<ScenarioValidationErrorV1>> {
    let manifest: ScenarioManifestV1 = serde_json::from_str(source).map_err(|error| {
        vec![ScenarioValidationErrorV1 {
            code: "JSON_PARSE".to_owned(),
            path: "$".to_owned(),
            message: error.to_string(),
        }]
    })?;
    let errors = validate_manifest_v1(&manifest);
    if errors.is_empty() {
        Ok(manifest)
    } else {
        Err(errors)
    }
}

pub(crate) fn validate_manifest_v1(
    manifest: &ScenarioManifestV1,
) -> Vec<ScenarioValidationErrorV1> {
    let mut errors = Vec::new();

    if manifest.schema_version != SCENARIO_SCHEMA_VERSION {
        push_error(
            &mut errors,
            "SCHEMA_VERSION",
            "$.schemaVersion",
            format!(
                "expected schemaVersion {}, got {}",
                SCENARIO_SCHEMA_VERSION, manifest.schema_version
            ),
        );
    }

    require_non_empty(
        &mut errors,
        "SCENARIO_ID",
        "$.scenarioId",
        &manifest.scenario_id,
    );
    require_non_empty(
        &mut errors,
        "SCENARIO_VERSION",
        "$.scenarioVersion",
        &manifest.scenario_version,
    );
    require_non_empty(&mut errors, "TITLE", "$.title", &manifest.title);
    require_non_empty(
        &mut errors,
        "PROGRAM_PROFILE",
        "$.programProfile.profileId",
        &manifest.program_profile.profile_id,
    );
    require_non_empty(
        &mut errors,
        "PROGRAM_FAMILY",
        "$.programProfile.family",
        &manifest.program_profile.family,
    );
    require_non_empty(
        &mut errors,
        "PROGRAM_ID",
        "$.programProfile.programId",
        &manifest.program_profile.program_id,
    );
    require_non_empty(
        &mut errors,
        "ASET_SOURCE",
        "$.programProfile.asetSource",
        &manifest.program_profile.aset_source,
    );

    if manifest.input_schema.is_empty() {
        push_error(
            &mut errors,
            "INPUT_SCHEMA_EMPTY",
            "$.inputSchema",
            "inputSchema must contain at least one input".to_owned(),
        );
    }

    let mut input_keys = BTreeSet::new();
    let mut input_types = BTreeMap::new();
    for (index, input) in manifest.input_schema.iter().enumerate() {
        let path = format!("$.inputSchema[{index}].key");
        require_non_empty(&mut errors, "INPUT_KEY", &path, &input.key);
        if !input_keys.insert(input.key.clone()) {
            push_error(
                &mut errors,
                "INPUT_KEY_DUPLICATE",
                &path,
                format!("duplicate input key {}", input.key),
            );
        }
        input_types.insert(input.key.clone(), input.input_type);
        if let Some(default) = &input.default {
            validate_value(
                &mut errors,
                &format!("$.inputSchema[{index}].default"),
                input.input_type,
                default,
            );
        }
    }

    validate_input_object(
        &mut errors,
        "$.initialInputs",
        &manifest.initial_inputs,
        &input_types,
    );

    if manifest.run_sequence.is_empty() {
        push_error(
            &mut errors,
            "RUN_SEQUENCE_EMPTY",
            "$.runSequence",
            "runSequence must contain at least one run".to_owned(),
        );
    }

    let mut run_ids = BTreeSet::new();
    for (index, run) in manifest.run_sequence.iter().enumerate() {
        let base = format!("$.runSequence[{index}]");
        require_non_empty(
            &mut errors,
            "RUN_ID",
            &format!("{base}.runId"),
            &run.run_id,
        );
        if !run_ids.insert(run.run_id.clone()) {
            push_error(
                &mut errors,
                "RUN_ID_DUPLICATE",
                &format!("{base}.runId"),
                format!("duplicate run id {}", run.run_id),
            );
        }

        validate_input_object(
            &mut errors,
            &format!("{base}.inputs"),
            &run.inputs,
            &input_types,
        );

        for (assertion_index, assertion) in run.assertions.iter().enumerate() {
            if let ScenarioAssertionV1::ResultFieldEquals { field, .. } = assertion {
                require_non_empty(
                    &mut errors,
                    "ASSERTION_FIELD",
                    &format!("{base}.assertions[{assertion_index}].field"),
                    field,
                );
            }
        }
    }

    if manifest.supported_backends.is_empty() {
        push_error(
            &mut errors,
            "BACKENDS_EMPTY",
            "$.supportedBackends",
            "supportedBackends must contain at least one backend".to_owned(),
        );
    } else {
        let mut seen = BTreeSet::new();
        for (index, backend) in manifest.supported_backends.iter().enumerate() {
            let key = format!("{backend:?}");
            if !seen.insert(key) {
                push_error(
                    &mut errors,
                    "BACKEND_DUPLICATE",
                    &format!("$.supportedBackends[{index}]"),
                    "duplicate backend".to_owned(),
                );
            }
        }
    }

    let mut invariants = BTreeSet::new();
    for (index, invariant) in manifest.invariants.iter().enumerate() {
        require_non_empty(
            &mut errors,
            "INVARIANT_EMPTY",
            &format!("$.invariants[{index}]"),
            invariant,
        );
        if !invariants.insert(invariant.clone()) {
            push_error(
                &mut errors,
                "INVARIANT_DUPLICATE",
                &format!("$.invariants[{index}]"),
                format!("duplicate invariant {invariant}"),
            );
        }
    }

    errors
}

fn validate_input_object(
    errors: &mut Vec<ScenarioValidationErrorV1>,
    path: &str,
    values: &BTreeMap<String, Value>,
    input_types: &BTreeMap<String, ScenarioInputTypeV1>,
) {
    for key in input_types.keys() {
        if !values.contains_key(key) {
            push_error(
                errors,
                "INPUT_MISSING",
                path,
                format!("missing input {key}"),
            );
        }
    }
    for (key, value) in values {
        let Some(input_type) = input_types.get(key) else {
            push_error(
                errors,
                "INPUT_UNKNOWN",
                &format!("{path}.{key}"),
                format!("unknown input {key}"),
            );
            continue;
        };
        validate_value(
            errors,
            &format!("{path}.{key}"),
            *input_type,
            value,
        );
    }
}

fn validate_value(
    errors: &mut Vec<ScenarioValidationErrorV1>,
    path: &str,
    input_type: ScenarioInputTypeV1,
    value: &Value,
) {
    let valid = match input_type {
        ScenarioInputTypeV1::Bit => value.as_u64().is_some_and(|value| value <= 1),
        ScenarioInputTypeV1::Count8 => {
            value.as_u64().is_some_and(|value| value <= u8::MAX as u64)
        }
        ScenarioInputTypeV1::Word32 => match value {
            Value::Number(number) => number.as_u64().is_some_and(|value| value <= u32::MAX as u64),
            Value::String(text) => is_canonical_word32_hex(text),
            _ => false,
        },
    };

    if !valid {
        push_error(
            errors,
            "INPUT_VALUE",
            path,
            format!("value does not match {input_type:?}"),
        );
    }
}

fn is_canonical_word32_hex(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[0] == b'0'
        && bytes[1] == b'x'
        && bytes[2..]
            .iter()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn require_non_empty(
    errors: &mut Vec<ScenarioValidationErrorV1>,
    code: &str,
    path: &str,
    value: &str,
) {
    if value.trim().is_empty() {
        push_error(
            errors,
            code,
            path,
            "value must not be empty".to_owned(),
        );
    }
}

fn push_error(
    errors: &mut Vec<ScenarioValidationErrorV1>,
    code: &str,
    path: &str,
    message: String,
) {
    errors.push(ScenarioValidationErrorV1 {
        code: code.to_owned(),
        path: path.to_owned(),
        message,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const MUX1_LIFECYCLE: &str =
        include_str!("../scenarios/mux1-lifecycle-v1.json");

    #[test]
    fn mux1_lifecycle_manifest_is_valid_and_round_trips() {
        let manifest = parse_and_validate_manifest_v1(MUX1_LIFECYCLE)
            .expect("valid MUX1 lifecycle scenario");

        assert_eq!(manifest.schema_version, SCENARIO_SCHEMA_VERSION);
        assert_eq!(manifest.scenario_id, "mux1-lifecycle");
        assert_eq!(manifest.run_sequence.len(), 4);
        assert_eq!(
            manifest.supported_backends,
            vec![ScenarioBackendV1::OptimizedCpu]
        );
        assert_eq!(
            manifest.run_sequence[0].inputs,
            manifest.run_sequence[3].inputs,
            "fourth run must intentionally return to the first configuration",
        );

        let json = serde_json::to_string_pretty(&manifest).unwrap();
        let decoded = parse_and_validate_manifest_v1(&json).unwrap();
        assert_eq!(decoded, manifest);
    }

    #[test]
    fn validator_reports_duplicate_missing_and_invalid_input() {
        let mut manifest =
            parse_and_validate_manifest_v1(MUX1_LIFECYCLE).unwrap();

        manifest.input_schema.push(manifest.input_schema[0].clone());
        manifest.run_sequence[0].inputs.remove("S");
        manifest.run_sequence[1]
            .inputs
            .insert("A".to_owned(), Value::from(2u64));

        let errors = validate_manifest_v1(&manifest);
        let codes = errors
            .iter()
            .map(|error| error.code.as_str())
            .collect::<Vec<_>>();

        assert!(codes.contains(&"INPUT_KEY_DUPLICATE"));
        assert!(codes.contains(&"INPUT_MISSING"));
        assert!(codes.contains(&"INPUT_VALUE"));
    }

    #[test]
    fn word32_hex_validation_is_canonical() {
        assert!(is_canonical_word32_hex("0xdeadbeef"));
        assert!(is_canonical_word32_hex("0x00000000"));
        assert!(!is_canonical_word32_hex("0xDEADBEEF"));
        assert!(!is_canonical_word32_hex("deadbeef"));
        assert!(!is_canonical_word32_hex("0x1"));
    }
}
