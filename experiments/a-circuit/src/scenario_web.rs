use super::{
    scenario::{
        parse_and_validate_manifest_v1, ScenarioBackendV1,
        ScenarioValidationErrorV1,
    },
    scenario_runner::{
        run_scenario_manifest_v1, ScenarioExecutionReportV1,
        ScenarioRunnerErrorV1,
    },
};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

const SCENARIO_TRANSPORT_SCHEMA_VERSION: u32 = 1;
const MAX_SCENARIO_MANIFEST_BYTES: usize = 1024 * 1024;

static SCENARIO_MANIFEST_BYTES: Mutex<Vec<u8>> = Mutex::new(Vec::new());
static SCENARIO_REPORT_JSON: Mutex<String> = Mutex::new(String::new());
static SCENARIO_ERROR_JSON: Mutex<String> = Mutex::new(String::new());

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
    SCENARIO_REPORT_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
    SCENARIO_ERROR_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
}

fn store_error(error: ScenarioTransportErrorV1) {
    let json = serde_json::to_string(&error)
        .unwrap_or_else(|serialization_error| {
            format!(
                "{{\"code\":\"SERIALIZE\",\"schemaVersion\":{},\"message\":{:?}}}",
                SCENARIO_TRANSPORT_SCHEMA_VERSION,
                serialization_error.to_string()
            )
        });
    *SCENARIO_ERROR_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = json;
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
    SCENARIO_MANIFEST_BYTES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clear();
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
        Ok(report) => match serde_json::to_string(&report) {
            Ok(json) => {
                *SCENARIO_REPORT_JSON
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = json;
                1
            }
            Err(error) => {
                store_error(ScenarioTransportErrorV1::Serialize {
                    schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
                    message: error.to_string(),
                });
                0
            }
        },
        Err(error) => {
            store_error(error);
            0
        }
    }
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
