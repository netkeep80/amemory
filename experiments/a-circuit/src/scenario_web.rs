use super::{
    scenario::{
        parse_and_validate_manifest_v1, ScenarioBackendV1,
        ScenarioValidationErrorV1,
    },
    scenario_registry::{
        load_preset_registry_v1, preset_manifest_source_by_index_v1,
        ScenarioPresetRegistryErrorV1,
    },
    scenario_runner::{
        run_scenario_manifest_v1, ScenarioExecutionReportV1,
        ScenarioRunnerErrorV1,
    },
};
use serde::{Deserialize, Serialize};
use std::{
    io::{self, Write},
    sync::Mutex,
};

const SCENARIO_TRANSPORT_SCHEMA_VERSION: u32 = 1;
const MAX_SCENARIO_MANIFEST_BYTES: usize = 1024 * 1024;
const MAX_SCENARIO_REPORT_BYTES: usize = 8 * 1024 * 1024;
const MAX_SCENARIO_ERROR_BYTES: usize = 512 * 1024;
const SCENARIO_LIMITS_JSON: &str =
    "{\"schemaVersion\":1,\"maxManifestBytes\":1048576,\"maxSingleReportBytes\":8388608,\"maxErrorBytes\":524288,\"maxRetainedReports\":1,\"maxRetainedErrors\":1,\"retentionMode\":\"LATEST\",\"liveSessionRetained\":false}";
const FALLBACK_ERROR_LIMIT_JSON: &str =
    "{\"code\":\"ERROR_LIMIT\",\"schemaVersion\":1,\"maxBytes\":524288}";

static SCENARIO_MANIFEST_BYTES: Mutex<Vec<u8>> = Mutex::new(Vec::new());
static SCENARIO_REPORT_JSON: Mutex<String> = Mutex::new(String::new());
static SCENARIO_ERROR_JSON: Mutex<String> = Mutex::new(String::new());
static SCENARIO_PRESET_REGISTRY_JSON: Mutex<String> = Mutex::new(String::new());
static SCENARIO_PRESET_MANIFEST_JSON: Mutex<String> = Mutex::new(String::new());

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScenarioTransportLimitsV1 {
    schema_version: u32,
    max_manifest_bytes: u32,
    max_single_report_bytes: u32,
    max_error_bytes: u32,
    max_retained_reports: u32,
    max_retained_errors: u32,
    retention_mode: String,
    live_session_retained: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum BoundedJsonError {
    Limit,
    Serialize(String),
    Utf8(String),
}

struct BoundedJsonWriter {
    bytes: Vec<u8>,
    max_bytes: usize,
    limit_exceeded: bool,
}

impl BoundedJsonWriter {
    fn new(max_bytes: usize) -> Self {
        Self {
            bytes: Vec::new(),
            max_bytes,
            limit_exceeded: false,
        }
    }
}

impl Write for BoundedJsonWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let Some(next_len) = self.bytes.len().checked_add(buffer.len()) else {
            self.limit_exceeded = true;
            return Err(io::Error::other("JSON byte limit exceeded"));
        };
        if next_len > self.max_bytes {
            self.limit_exceeded = true;
            return Err(io::Error::other("JSON byte limit exceeded"));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn serialize_bounded<T: Serialize>(
    value: &T,
    max_bytes: usize,
) -> Result<String, BoundedJsonError> {
    let mut writer = BoundedJsonWriter::new(max_bytes);
    let result = serde_json::to_writer(&mut writer, value);
    if writer.limit_exceeded {
        return Err(BoundedJsonError::Limit);
    }
    result.map_err(|error| BoundedJsonError::Serialize(error.to_string()))?;
    String::from_utf8(writer.bytes)
        .map_err(|error| BoundedJsonError::Utf8(error.to_string()))
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "SCREAMING_SNAKE_CASE")]
enum ScenarioTransportErrorV1 {
    ManifestLength {
        schema_version: u32,
        requested: u32,
        available: u32,
        max_bytes: u32,
    },
    ManifestUtf8 {
        schema_version: u32,
        message: String,
    },
    Validation {
        schema_version: u32,
        errors: Vec<ScenarioValidationErrorV1>,
    },
    BackendCode {
        schema_version: u32,
        backend_code: u32,
    },
    Runner {
        schema_version: u32,
        error: ScenarioRunnerErrorV1,
    },
    PresetRegistry {
        schema_version: u32,
        error: ScenarioPresetRegistryErrorV1,
    },
    ReportLimit {
        schema_version: u32,
        max_bytes: u32,
    },
    ErrorLimit {
        schema_version: u32,
        max_bytes: u32,
    },
    Serialize {
        schema_version: u32,
        message: String,
    },
}

fn backend_from_code(code: u32) -> Option<ScenarioBackendV1> {
    match code {
        0 => Some(ScenarioBackendV1::OptimizedCpu),
        1 => Some(ScenarioBackendV1::Webgpu),
        2 => Some(ScenarioBackendV1::Linksdb),
        _ => None,
    }
}

fn clear_report_and_error() {
    *SCENARIO_REPORT_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = String::new();
    *SCENARIO_ERROR_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = String::new();
}

fn store_error(error: ScenarioTransportErrorV1) {
    let json = match serialize_bounded(&error, MAX_SCENARIO_ERROR_BYTES) {
        Ok(json) => json,
        Err(BoundedJsonError::Limit) => {
            let limit = ScenarioTransportErrorV1::ErrorLimit {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
                max_bytes: MAX_SCENARIO_ERROR_BYTES as u32,
            };
            serialize_bounded(&limit, MAX_SCENARIO_ERROR_BYTES)
                .unwrap_or_else(|_| FALLBACK_ERROR_LIMIT_JSON.to_owned())
        }
        Err(error) => {
            let serialization = ScenarioTransportErrorV1::Serialize {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
                message: format!("{error:?}"),
            };
            serialize_bounded(&serialization, MAX_SCENARIO_ERROR_BYTES)
                .unwrap_or_else(|_| FALLBACK_ERROR_LIMIT_JSON.to_owned())
        }
    };
    *SCENARIO_ERROR_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = json;
}

fn serialize_report_bounded(
    report: &ScenarioExecutionReportV1,
    max_bytes: usize,
) -> Result<String, ScenarioTransportErrorV1> {
    serialize_bounded(report, max_bytes).map_err(|error| match error {
        BoundedJsonError::Limit => ScenarioTransportErrorV1::ReportLimit {
            schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
            max_bytes: max_bytes.min(u32::MAX as usize) as u32,
        },
        error => ScenarioTransportErrorV1::Serialize {
            schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
            message: format!("{error:?}"),
        },
    })
}

fn store_report_bounded(
    report: &ScenarioExecutionReportV1,
    max_bytes: usize,
) -> Result<(), ScenarioTransportErrorV1> {
    let json = serialize_report_bounded(report, max_bytes)?;
    *SCENARIO_REPORT_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = json;
    Ok(())
}

fn execute_manifest_bytes(
    length: u32,
    backend_code: u32,
) -> Result<ScenarioExecutionReportV1, ScenarioTransportErrorV1> {
    let backend =
        backend_from_code(backend_code).ok_or(ScenarioTransportErrorV1::BackendCode {
            schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
            backend_code,
        })?;

    let bytes = {
        let manifest = SCENARIO_MANIFEST_BYTES
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let requested = length as usize;
        if requested > manifest.len() || requested > MAX_SCENARIO_MANIFEST_BYTES {
            return Err(ScenarioTransportErrorV1::ManifestLength {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
                requested: length,
                available: manifest.len().min(u32::MAX as usize) as u32,
                max_bytes: MAX_SCENARIO_MANIFEST_BYTES as u32,
            });
        }
        manifest[..requested].to_vec()
    };

    let source = std::str::from_utf8(&bytes).map_err(|error| {
        ScenarioTransportErrorV1::ManifestUtf8 {
            schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
            message: error.to_string(),
        }
    })?;

    let manifest = parse_and_validate_manifest_v1(source).map_err(|errors| {
        ScenarioTransportErrorV1::Validation {
            schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
            errors,
        }
    })?;

    run_scenario_manifest_v1(&manifest, backend).map_err(|error| {
        ScenarioTransportErrorV1::Runner {
            schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
            error,
        }
    })
}

#[no_mangle]
pub extern "C" fn amemory_scenario_manifest_clear() {
    *SCENARIO_MANIFEST_BYTES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Vec::new();
    clear_report_and_error();
}

#[no_mangle]
pub extern "C" fn amemory_scenario_manifest_set_byte(
    index: u32,
    byte: u32,
) -> u32 {
    let index = index as usize;
    if index >= MAX_SCENARIO_MANIFEST_BYTES || byte > u8::MAX as u32 {
        return 0;
    }

    let mut manifest = SCENARIO_MANIFEST_BYTES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if manifest.len() <= index {
        manifest.resize(index + 1, 0);
    }
    manifest[index] = byte as u8;
    1
}

#[no_mangle]
pub extern "C" fn amemory_scenario_execute_json(
    length: u32,
    backend_code: u32,
) -> u32 {
    clear_report_and_error();

    match execute_manifest_bytes(length, backend_code) {
        Ok(report) => match store_report_bounded(
            &report,
            MAX_SCENARIO_REPORT_BYTES,
        ) {
            Ok(()) => 1,
            Err(error) => {
                store_error(error);
                0
            }
        },
        Err(error) => {
            store_error(error);
            0
        }
    }
}

#[no_mangle]
pub extern "C" fn amemory_scenario_transport_clear_output() {
    clear_report_and_error();
}

#[no_mangle]
pub extern "C" fn amemory_scenario_transport_limits_available() -> u32 {
    1
}

#[no_mangle]
pub extern "C" fn amemory_scenario_transport_limits_json_len() -> u32 {
    SCENARIO_LIMITS_JSON.len() as u32
}

#[no_mangle]
pub extern "C" fn amemory_scenario_transport_limits_json_ptr() -> u32 {
    SCENARIO_LIMITS_JSON.as_ptr() as usize as u32
}

#[no_mangle]
pub extern "C" fn amemory_scenario_transport_limits_json_byte(
    index: u32,
) -> u32 {
    SCENARIO_LIMITS_JSON
        .as_bytes()
        .get(index as usize)
        .copied()
        .map(u32::from)
        .unwrap_or(u32::MAX)
}

fn clear_preset_buffers() {
    *SCENARIO_PRESET_REGISTRY_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = String::new();
    *SCENARIO_PRESET_MANIFEST_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = String::new();
}

#[no_mangle]
pub extern "C" fn amemory_scenario_preset_registry_refresh() -> u32 {
    clear_preset_buffers();
    clear_report_and_error();

    let registry = match load_preset_registry_v1() {
        Ok(registry) => registry,
        Err(error) => {
            store_error(ScenarioTransportErrorV1::PresetRegistry {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
                error,
            });
            return 0;
        }
    };

    match serialize_bounded(&registry, MAX_SCENARIO_REPORT_BYTES) {
        Ok(json) => {
            *SCENARIO_PRESET_REGISTRY_JSON
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = json;
            1
        }
        Err(BoundedJsonError::Limit) => {
            store_error(ScenarioTransportErrorV1::ReportLimit {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
                max_bytes: MAX_SCENARIO_REPORT_BYTES as u32,
            });
            0
        }
        Err(error) => {
            store_error(ScenarioTransportErrorV1::Serialize {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
                message: format!("{error:?}"),
            });
            0
        }
    }
}

#[no_mangle]
pub extern "C" fn amemory_scenario_preset_registry_available() -> u32 {
    string_available(&SCENARIO_PRESET_REGISTRY_JSON)
}

#[no_mangle]
pub extern "C" fn amemory_scenario_preset_registry_json_len() -> u32 {
    string_len(&SCENARIO_PRESET_REGISTRY_JSON)
}

#[no_mangle]
pub extern "C" fn amemory_scenario_preset_registry_json_ptr() -> u32 {
    string_ptr(&SCENARIO_PRESET_REGISTRY_JSON)
}

#[no_mangle]
pub extern "C" fn amemory_scenario_preset_registry_json_byte(
    index: u32,
) -> u32 {
    string_byte(&SCENARIO_PRESET_REGISTRY_JSON, index)
}

#[no_mangle]
pub extern "C" fn amemory_scenario_preset_manifest_load(
    index: u32,
) -> u32 {
    *SCENARIO_PRESET_MANIFEST_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = String::new();
    clear_report_and_error();

    let source = match preset_manifest_source_by_index_v1(index) {
        Ok(source) => source,
        Err(error) => {
            store_error(ScenarioTransportErrorV1::PresetRegistry {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
                error,
            });
            return 0;
        }
    };

    if source.len() > MAX_SCENARIO_MANIFEST_BYTES {
        store_error(ScenarioTransportErrorV1::ManifestLength {
            schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
            requested: source.len().min(u32::MAX as usize) as u32,
            available: source.len().min(u32::MAX as usize) as u32,
            max_bytes: MAX_SCENARIO_MANIFEST_BYTES as u32,
        });
        return 0;
    }

    *SCENARIO_PRESET_MANIFEST_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = source.to_owned();
    1
}

#[no_mangle]
pub extern "C" fn amemory_scenario_preset_manifest_available() -> u32 {
    string_available(&SCENARIO_PRESET_MANIFEST_JSON)
}

#[no_mangle]
pub extern "C" fn amemory_scenario_preset_manifest_json_len() -> u32 {
    string_len(&SCENARIO_PRESET_MANIFEST_JSON)
}

#[no_mangle]
pub extern "C" fn amemory_scenario_preset_manifest_json_ptr() -> u32 {
    string_ptr(&SCENARIO_PRESET_MANIFEST_JSON)
}

#[no_mangle]
pub extern "C" fn amemory_scenario_preset_manifest_json_byte(
    index: u32,
) -> u32 {
    string_byte(&SCENARIO_PRESET_MANIFEST_JSON, index)
}

fn string_available(buffer: &Mutex<String>) -> u32 {
    let guard = buffer
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    u32::from(!guard.is_empty())
}

fn string_len(buffer: &Mutex<String>) -> u32 {
    buffer
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .len()
        .min(u32::MAX as usize) as u32
}

fn string_ptr(buffer: &Mutex<String>) -> u32 {
    buffer
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_ptr() as usize as u32
}

fn string_byte(buffer: &Mutex<String>, index: u32) -> u32 {
    buffer
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .as_bytes()
        .get(index as usize)
        .copied()
        .map(u32::from)
        .unwrap_or(u32::MAX)
}

#[no_mangle]
pub extern "C" fn amemory_scenario_report_available() -> u32 {
    string_available(&SCENARIO_REPORT_JSON)
}

#[no_mangle]
pub extern "C" fn amemory_scenario_report_json_len() -> u32 {
    string_len(&SCENARIO_REPORT_JSON)
}

#[no_mangle]
pub extern "C" fn amemory_scenario_report_json_ptr() -> u32 {
    string_ptr(&SCENARIO_REPORT_JSON)
}

#[no_mangle]
pub extern "C" fn amemory_scenario_report_json_byte(index: u32) -> u32 {
    string_byte(&SCENARIO_REPORT_JSON, index)
}

#[no_mangle]
pub extern "C" fn amemory_scenario_error_available() -> u32 {
    string_available(&SCENARIO_ERROR_JSON)
}

#[no_mangle]
pub extern "C" fn amemory_scenario_error_json_len() -> u32 {
    string_len(&SCENARIO_ERROR_JSON)
}

#[no_mangle]
pub extern "C" fn amemory_scenario_error_json_ptr() -> u32 {
    string_ptr(&SCENARIO_ERROR_JSON)
}

#[no_mangle]
pub extern "C" fn amemory_scenario_error_json_byte(index: u32) -> u32 {
    string_byte(&SCENARIO_ERROR_JSON, index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenario_registry::ScenarioPresetRegistryV1;

    const MUX1_LIFECYCLE: &str =
        include_str!("../scenarios/mux1-lifecycle-v1.json");

    fn load_manifest(source: &str) {
        amemory_scenario_manifest_clear();
        for (index, byte) in source.bytes().enumerate() {
            assert_eq!(
                amemory_scenario_manifest_set_byte(index as u32, byte as u32),
                1
            );
        }
    }

    fn read_buffer(
        len: extern "C" fn() -> u32,
        byte: extern "C" fn(u32) -> u32,
    ) -> String {
        let mut output = Vec::new();
        for index in 0..len() {
            let value = byte(index);
            assert!(value <= u8::MAX as u32);
            output.push(value as u8);
        }
        String::from_utf8(output).unwrap()
    }

    #[test]
    fn preset_registry_transport_derives_summary_and_exact_manifest() {
        clear_preset_buffers();
        clear_report_and_error();

        assert_eq!(amemory_scenario_preset_registry_refresh(), 1);
        assert_eq!(amemory_scenario_preset_registry_available(), 1);
        assert_eq!(amemory_scenario_error_available(), 0);

        let registry_json = read_buffer(
            amemory_scenario_preset_registry_json_len,
            amemory_scenario_preset_registry_json_byte,
        );
        let registry: ScenarioPresetRegistryV1 =
            serde_json::from_str(&registry_json).unwrap();
        assert_eq!(registry.entries.len(), 3);
        assert_eq!(registry.entries[0].scenario_id, "mux1-lifecycle");
        assert_eq!(registry.entries[1].scenario_id, "xor32-lifecycle");
        assert_eq!(
            registry.entries[1].program_profile_id,
            "a-circuit:logic-xor32"
        );
        assert_eq!(registry.entries[2].scenario_id, "add32-lifecycle");
        assert_eq!(
            registry.entries[2].program_profile_id,
            "a-circuit:arithmetic-add32"
        );

        assert_eq!(amemory_scenario_preset_manifest_load(0), 1);
        let manifest_json = read_buffer(
            amemory_scenario_preset_manifest_json_len,
            amemory_scenario_preset_manifest_json_byte,
        );
        assert_eq!(
            manifest_json,
            include_str!("../scenarios/mux1-lifecycle-v1.json")
        );

        assert_eq!(amemory_scenario_preset_manifest_load(99), 0);
        assert_eq!(amemory_scenario_preset_manifest_available(), 0);
        let error_json = read_buffer(
            amemory_scenario_error_json_len,
            amemory_scenario_error_json_byte,
        );
        assert!(error_json.contains("\"code\":\"PRESET_REGISTRY\""));
        assert!(error_json.contains("PRESET_INDEX_OUT_OF_RANGE"));
    }

    #[test]
    fn transport_limits_json_matches_bounded_latest_policy() {
        let limits: ScenarioTransportLimitsV1 =
            serde_json::from_str(SCENARIO_LIMITS_JSON).unwrap();
        assert_eq!(limits.schema_version, SCENARIO_TRANSPORT_SCHEMA_VERSION);
        assert_eq!(
            limits.max_manifest_bytes,
            MAX_SCENARIO_MANIFEST_BYTES as u32
        );
        assert_eq!(
            limits.max_single_report_bytes,
            MAX_SCENARIO_REPORT_BYTES as u32
        );
        assert_eq!(limits.max_error_bytes, MAX_SCENARIO_ERROR_BYTES as u32);
        assert_eq!(limits.max_retained_reports, 1);
        assert_eq!(limits.max_retained_errors, 1);
        assert_eq!(limits.retention_mode, "LATEST");
        assert!(!limits.live_session_retained);
    }

    #[test]
    fn bounded_report_overflow_is_explicit_and_never_partially_stored() {
        load_manifest(MUX1_LIFECYCLE);
        let report = execute_manifest_bytes(
            MUX1_LIFECYCLE.len() as u32,
            0,
        )
        .unwrap();

        clear_report_and_error();
        let error = store_report_bounded(&report, 64).unwrap_err();
        assert!(matches!(
            error,
            ScenarioTransportErrorV1::ReportLimit {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
                max_bytes: 64,
            }
        ));
        assert_eq!(amemory_scenario_report_available(), 0);

        store_error(error);
        assert_eq!(amemory_scenario_error_available(), 1);
        let error_json = read_buffer(
            amemory_scenario_error_json_len,
            amemory_scenario_error_json_byte,
        );
        assert!(error_json.contains("\"code\":\"REPORT_LIMIT\""));
        assert!(!error_json.contains("structuralFacts"));
    }

    #[test]
    fn oversized_error_is_replaced_by_small_structured_error_limit() {
        clear_report_and_error();
        store_error(ScenarioTransportErrorV1::Serialize {
            schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
            message: "x".repeat(MAX_SCENARIO_ERROR_BYTES + 1024),
        });

        assert_eq!(amemory_scenario_report_available(), 0);
        assert_eq!(amemory_scenario_error_available(), 1);
        assert!(
            amemory_scenario_error_json_len()
                < MAX_SCENARIO_ERROR_BYTES as u32
        );
        let json = read_buffer(
            amemory_scenario_error_json_len,
            amemory_scenario_error_json_byte,
        );
        assert!(json.contains("\"code\":\"ERROR_LIMIT\""));
        let _: serde_json::Value = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn output_clear_preserves_manifest_for_semantically_equal_rerun() {
        load_manifest(MUX1_LIFECYCLE);
        assert_eq!(
            amemory_scenario_execute_json(
                MUX1_LIFECYCLE.len() as u32,
                0,
            ),
            1
        );
        let first_json = read_buffer(
            amemory_scenario_report_json_len,
            amemory_scenario_report_json_byte,
        );
        let first: ScenarioExecutionReportV1 =
            serde_json::from_str(&first_json).unwrap();

        amemory_scenario_transport_clear_output();
        assert_eq!(amemory_scenario_report_available(), 0);
        assert_eq!(amemory_scenario_error_available(), 0);

        // Output cleanup does not clear the manifest input buffer.
        assert_eq!(
            amemory_scenario_execute_json(
                MUX1_LIFECYCLE.len() as u32,
                0,
            ),
            1
        );
        let second_json = read_buffer(
            amemory_scenario_report_json_len,
            amemory_scenario_report_json_byte,
        );
        let second: ScenarioExecutionReportV1 =
            serde_json::from_str(&second_json).unwrap();

        assert!(first.overall_pass && second.overall_pass);
        assert_eq!(first.runs.len(), second.runs.len());
        for (left, right) in first.runs.iter().zip(&second.runs) {
            assert_eq!(left.inputs, right.inputs);
            assert_eq!(left.result, right.result);
            assert_eq!(left.assertion_results, right.assertion_results);
            assert_eq!(left.observed.active_reaction_count, 7);
            assert_eq!(right.observed.active_reaction_count, 7);
        }

        amemory_scenario_manifest_clear();
        assert_eq!(amemory_scenario_report_available(), 0);
        assert_eq!(amemory_scenario_error_available(), 0);
        assert_eq!(
            amemory_scenario_execute_json(
                MUX1_LIFECYCLE.len() as u32,
                0,
            ),
            0
        );
        let error_json = read_buffer(
            amemory_scenario_error_json_len,
            amemory_scenario_error_json_byte,
        );
        assert!(error_json.contains("\"code\":\"MANIFEST_LENGTH\""));
    }

    #[test]
    fn browser_transport_exports_real_persistent_scenario_report() {
        load_manifest(MUX1_LIFECYCLE);

        assert_eq!(
            amemory_scenario_execute_json(
                MUX1_LIFECYCLE.len() as u32,
                0,
            ),
            1
        );
        assert_eq!(amemory_scenario_report_available(), 1);
        assert_eq!(amemory_scenario_error_available(), 0);

        let json = read_buffer(
            amemory_scenario_report_json_len,
            amemory_scenario_report_json_byte,
        );
        let report: ScenarioExecutionReportV1 =
            serde_json::from_str(&json).unwrap();

        assert_eq!(report.scenario_id, "mux1-lifecycle");
        assert_eq!(report.runs.len(), 4);
        assert!(report.overall_pass);
        assert_eq!(
            report.session_open_profile.session_id,
            report.session_id
        );
        assert!(report
            .runs
            .iter()
            .all(|run| run.observed.session_id == report.session_id));
        assert_eq!(
            report.runs[3].inputs,
            report.runs[0].inputs,
            "fourth run must return to the first manifest configuration"
        );
        assert!(
            report.runs[3].configuration_reused,
            "return-to-first must reuse canonical configuration Links"
        );

        let session_run_ids = report
            .runs
            .iter()
            .map(|run| run.session_run_id)
            .collect::<Vec<_>>();
        assert_eq!(session_run_ids, vec![1, 2, 3, 4]);
    }

    #[test]
    fn browser_transport_reports_unsupported_backend_without_fallback() {
        load_manifest(MUX1_LIFECYCLE);

        assert_eq!(
            amemory_scenario_execute_json(
                MUX1_LIFECYCLE.len() as u32,
                1,
            ),
            0
        );
        assert_eq!(amemory_scenario_report_available(), 0);
        assert_eq!(amemory_scenario_error_available(), 1);

        let json = read_buffer(
            amemory_scenario_error_json_len,
            amemory_scenario_error_json_byte,
        );
        let error: ScenarioTransportErrorV1 =
            serde_json::from_str(&json).unwrap();

        assert!(matches!(
            error,
            ScenarioTransportErrorV1::Runner {
                error: ScenarioRunnerErrorV1::UnsupportedBackend {
                    backend: ScenarioBackendV1::Webgpu,
                },
                ..
            }
        ));
    }

    #[test]
    fn browser_transport_rejects_invalid_backend_code_and_manifest_length() {
        load_manifest(MUX1_LIFECYCLE);
        assert_eq!(
            amemory_scenario_execute_json(
                MUX1_LIFECYCLE.len() as u32,
                99,
            ),
            0
        );
        let backend_error = read_buffer(
            amemory_scenario_error_json_len,
            amemory_scenario_error_json_byte,
        );
        assert!(backend_error.contains("\"code\":\"BACKEND_CODE\""));

        assert_eq!(
            amemory_scenario_execute_json(
                (MUX1_LIFECYCLE.len() + 1) as u32,
                0,
            ),
            0
        );
        let length_error = read_buffer(
            amemory_scenario_error_json_len,
            amemory_scenario_error_json_byte,
        );
        assert!(length_error.contains("\"code\":\"MANIFEST_LENGTH\""));
    }
}
