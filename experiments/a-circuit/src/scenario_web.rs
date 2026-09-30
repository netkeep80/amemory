use super::{
    scenario::{
        parse_and_validate_manifest_v1, ScenarioBackendV1, ScenarioRunV1,
        ScenarioValidationErrorV1,
    },
    observability::RunPipelineProfileV1,
    scenario_registry::{
        load_preset_registry_v1, preset_manifest_source_by_index_v1,
        ScenarioPresetRegistryErrorV1,
    },
    scenario_runner::{
        begin_cpu_scenario_step_run_v1, open_cpu_scenario_session_v1,
        run_cpu_scenario_session_once_v1, run_scenario_manifest_v1,
        step_cpu_scenario_session_v1, ScenarioCpuSessionV1,
        ScenarioExecutionReportV1, ScenarioLiveSessionStatusV1,
        ScenarioRunReportV1, ScenarioRunnerErrorV1, ScenarioStepBeginV1,
        ScenarioStepReportV1,
    },
};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    io::{self, Write},
    sync::Mutex,
};

const SCENARIO_TRANSPORT_SCHEMA_VERSION: u32 = 1;
const MAX_SCENARIO_MANIFEST_BYTES: usize = 1024 * 1024;
const MAX_SCENARIO_REPORT_BYTES: usize = 8 * 1024 * 1024;
const MAX_SCENARIO_ERROR_BYTES: usize = 512 * 1024;
const MAX_LIVE_RETAINED_RUNS: u32 = 32;
const MAX_LIVE_RETAINED_EVENTS: u32 = 65_536;
const MAX_LIVE_RETAINED_BYTES: u32 = 16 * 1024 * 1024;
const MAX_LIVE_PROFILE_POINTS: u32 = 64;
const MAX_LIVE_EXPORT_BYTES: usize = 16 * 1024 * 1024;
const DEFAULT_LIVE_RETAINED_RUNS: u32 = 8;
const DEFAULT_LIVE_RETAINED_EVENTS: u32 = 8_192;
const DEFAULT_LIVE_RETAINED_BYTES: u32 = 16 * 1024 * 1024;
const SCENARIO_LIMITS_JSON: &str =
    "{\"schemaVersion\":1,\"maxManifestBytes\":1048576,\"maxSingleReportBytes\":8388608,\"maxErrorBytes\":524288,\"maxRetainedReports\":1,\"maxRetainedErrors\":1,\"retentionMode\":\"LATEST\",\"liveSessionRetained\":true,\"batchSessionRetained\":false,\"liveObserver\":{\"schemaVersion\":1,\"defaultRetentionMode\":\"RING\",\"supportedRetentionModes\":[\"LATEST\",\"RING\",\"EXPLICIT_EXPORT\"],\"maxRetainedRuns\":32,\"maxRetainedEvents\":65536,\"maxRetainedBytes\":16777216,\"maxSingleReportBytes\":8388608,\"maxProfilePoints\":64,\"maxExportBytes\":16777216}}";
const FALLBACK_ERROR_LIMIT_JSON: &str =
    "{\"code\":\"ERROR_LIMIT\",\"schemaVersion\":1,\"maxBytes\":524288}";

static SCENARIO_MANIFEST_BYTES: Mutex<Vec<u8>> = Mutex::new(Vec::new());
static SCENARIO_REPORT_JSON: Mutex<String> = Mutex::new(String::new());
static SCENARIO_ERROR_JSON: Mutex<String> = Mutex::new(String::new());
static SCENARIO_PRESET_REGISTRY_JSON: Mutex<String> = Mutex::new(String::new());
static SCENARIO_PRESET_MANIFEST_JSON: Mutex<String> = Mutex::new(String::new());
static SCENARIO_LIVE_SESSION: Mutex<Option<ScenarioCpuSessionV1>> =
    Mutex::new(None);
static SCENARIO_LIVE_RUN_BYTES: Mutex<Vec<u8>> = Mutex::new(Vec::new());
static SCENARIO_LIVE_OUTPUT_JSON: Mutex<String> =
    Mutex::new(String::new());
static SCENARIO_LIVE_HISTORY: Mutex<Option<ScenarioLiveObserverHistoryV1>> =
    Mutex::new(None);
static SCENARIO_LIVE_HISTORY_JSON: Mutex<String> =
    Mutex::new(String::new());
static SCENARIO_LIVE_EXPORT_JSON: Mutex<String> =
    Mutex::new(String::new());

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
    batch_session_retained: bool,
    live_observer: ScenarioLiveObserverLimitsV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScenarioLiveObserverLimitsV1 {
    schema_version: u32,
    default_retention_mode: LiveRetentionModeV1,
    supported_retention_modes: Vec<LiveRetentionModeV1>,
    max_retained_runs: u32,
    max_retained_events: u32,
    max_retained_bytes: u32,
    max_single_report_bytes: u32,
    max_profile_points: u32,
    max_export_bytes: u32,
}


#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum LiveRetentionModeV1 {
    Latest,
    Ring,
    ExplicitExport,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScenarioLiveHistoryPolicyV1 {
    schema_version: u32,
    retention_mode: LiveRetentionModeV1,
    max_retained_runs: u32,
    max_retained_events: u32,
    max_retained_bytes: u32,
    max_single_report_bytes: u32,
    max_profile_points: u32,
}

impl Default for ScenarioLiveHistoryPolicyV1 {
    fn default() -> Self {
        Self {
            schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
            retention_mode: LiveRetentionModeV1::Ring,
            max_retained_runs: DEFAULT_LIVE_RETAINED_RUNS,
            max_retained_events: DEFAULT_LIVE_RETAINED_EVENTS,
            max_retained_bytes: DEFAULT_LIVE_RETAINED_BYTES,
            max_single_report_bytes: MAX_SCENARIO_REPORT_BYTES as u32,
            max_profile_points: MAX_LIVE_PROFILE_POINTS,
        }
    }
}

#[derive(Clone, Debug)]
struct RetainedLiveRunV1 {
    report: ScenarioRunReportV1,
    event_count: u32,
    serialized_bytes: u32,
}

#[derive(Debug)]
struct ScenarioLiveObserverHistoryV1 {
    session_id: String,
    program_fingerprint: String,
    policy: ScenarioLiveHistoryPolicyV1,
    runs: VecDeque<RetainedLiveRunV1>,
    retained_events: u32,
    retained_bytes: u32,
    latest_report: Option<ScenarioRunReportV1>,
    latest_report_bytes: u32,
    profile_trend: VecDeque<RunPipelineProfileV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScenarioLiveHistoryStatusV1 {
    schema_version: u32,
    session_id: String,
    program_fingerprint: String,
    retention_mode: LiveRetentionModeV1,
    max_retained_runs: u32,
    max_retained_events: u32,
    max_retained_bytes: u32,
    max_single_report_bytes: u32,
    retained_runs: u32,
    retained_events: u32,
    retained_bytes: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    first_run_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_run_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    latest_run_id: Option<u64>,
    latest_report_bytes: u32,
    profile_points: u32,
    max_profile_points: u32,
    export_available: bool,
    profile_trend: Vec<RunPipelineProfileV1>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum ScenarioLiveHistoryExportKindV1 {
    SelectedRun,
    AvailableHistory,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScenarioLiveHistoryExportV1 {
    schema_version: u32,
    representation_id: String,
    representation_version: String,
    export_kind: ScenarioLiveHistoryExportKindV1,
    session_id: String,
    program_fingerprint: String,
    runs: Vec<ScenarioRunReportV1>,
}

impl ScenarioLiveObserverHistoryV1 {
    fn new(status: &ScenarioLiveSessionStatusV1) -> Self {
        Self {
            session_id: status.session_id.clone(),
            program_fingerprint: status.program_fingerprint.clone(),
            policy: ScenarioLiveHistoryPolicyV1::default(),
            runs: VecDeque::new(),
            retained_events: 0,
            retained_bytes: 0,
            latest_report: None,
            latest_report_bytes: 0,
            profile_trend: VecDeque::new(),
        }
    }

    fn status(&self, export_available: bool) -> ScenarioLiveHistoryStatusV1 {
        ScenarioLiveHistoryStatusV1 {
            schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
            session_id: self.session_id.clone(),
            program_fingerprint: self.program_fingerprint.clone(),
            retention_mode: self.policy.retention_mode,
            max_retained_runs: self.policy.max_retained_runs,
            max_retained_events: self.policy.max_retained_events,
            max_retained_bytes: self.policy.max_retained_bytes,
            max_single_report_bytes: self.policy.max_single_report_bytes,
            retained_runs: self.runs.len().min(u32::MAX as usize) as u32,
            retained_events: self.retained_events,
            retained_bytes: self.retained_bytes,
            first_run_id:
                self.runs.front().map(|entry| entry.report.session_run_id),
            last_run_id:
                self.runs.back().map(|entry| entry.report.session_run_id),
            latest_run_id:
                self.latest_report.as_ref().map(|report| report.session_run_id),
            latest_report_bytes: self.latest_report_bytes,
            profile_points:
                self.profile_trend.len().min(u32::MAX as usize) as u32,
            max_profile_points: self.policy.max_profile_points,
            export_available,
            profile_trend: self.profile_trend.iter().cloned().collect(),
        }
    }

    fn push_profile(&mut self, report: &ScenarioRunReportV1) {
        if let Some(profile) = report.pipeline_profile.clone() {
            self.profile_trend.push_back(profile);
            while self.profile_trend.len()
                > self.policy.max_profile_points as usize
            {
                self.profile_trend.pop_front();
            }
        }
    }

    fn remove_oldest(&mut self) {
        if let Some(entry) = self.runs.pop_front() {
            self.retained_events =
                self.retained_events.saturating_sub(entry.event_count);
            self.retained_bytes =
                self.retained_bytes.saturating_sub(entry.serialized_bytes);
        }
    }

    fn clear_raw(&mut self) {
        self.runs.clear();
        self.retained_events = 0;
        self.retained_bytes = 0;
    }

    fn clear_history(&mut self) {
        self.clear_raw();
        self.latest_report = None;
        self.latest_report_bytes = 0;
    }

    fn enforce_policy(&mut self) {
        match self.policy.retention_mode {
            LiveRetentionModeV1::Latest => {
                while self.runs.len() > 1 {
                    self.remove_oldest();
                }
            }
            LiveRetentionModeV1::Ring => {
                while self.runs.len()
                    > self.policy.max_retained_runs as usize
                    || self.retained_events > self.policy.max_retained_events
                    || self.retained_bytes > self.policy.max_retained_bytes
                {
                    self.remove_oldest();
                }
            }
            LiveRetentionModeV1::ExplicitExport => self.clear_raw(),
        }

        let latest_events = self
            .latest_report
            .as_ref()
            .map(|report| {
                report.observed.events.len().min(u32::MAX as usize) as u32
            })
            .unwrap_or(0);
        if self.latest_report_bytes > self.policy.max_retained_bytes
            || latest_events > self.policy.max_retained_events
        {
            self.latest_report = None;
            self.latest_report_bytes = 0;
        }

        while self.profile_trend.len()
            > self.policy.max_profile_points as usize
        {
            self.profile_trend.pop_front();
        }
    }

    fn set_policy(&mut self, policy: ScenarioLiveHistoryPolicyV1) {
        self.policy = policy;
        self.enforce_policy();
    }

    fn record_run(
        &mut self,
        report: &ScenarioRunReportV1,
    ) -> Result<(), ScenarioTransportErrorV1> {
        // PROFILE is observer-only and remains available even when a detailed
        // report cannot be retained under the raw-history policy.
        self.push_profile(report);

        let serialized = serialize_bounded(
            report,
            self.policy.max_single_report_bytes as usize,
        )
        .map_err(|error| match error {
            BoundedJsonError::Limit => {
                ScenarioTransportErrorV1::LiveHistoryLimit {
                    schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
                    limit_kind: "SINGLE_REPORT_BYTES".to_owned(),
                    requested: 0,
                    max: self.policy.max_single_report_bytes as u64,
                }
            }
            error => ScenarioTransportErrorV1::Serialize {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
                message: format!("{error:?}"),
            },
        })?;
        let serialized_bytes =
            serialized.len().min(u32::MAX as usize) as u32;
        let event_count =
            report.observed.events.len().min(u32::MAX as usize) as u32;

        if event_count > self.policy.max_retained_events {
            return Err(ScenarioTransportErrorV1::LiveHistoryLimit {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
                limit_kind: "RETAINED_EVENTS".to_owned(),
                requested: event_count as u64,
                max: self.policy.max_retained_events as u64,
            });
        }
        if serialized_bytes > self.policy.max_retained_bytes {
            return Err(ScenarioTransportErrorV1::LiveHistoryLimit {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
                limit_kind: "RETAINED_BYTES".to_owned(),
                requested: serialized_bytes as u64,
                max: self.policy.max_retained_bytes as u64,
            });
        }

        self.latest_report = Some(report.clone());
        self.latest_report_bytes = serialized_bytes;

        match self.policy.retention_mode {
            LiveRetentionModeV1::Latest => {
                self.clear_raw();
                self.runs.push_back(RetainedLiveRunV1 {
                    report: report.clone(),
                    event_count,
                    serialized_bytes,
                });
                self.retained_events = event_count;
                self.retained_bytes = serialized_bytes;
            }
            LiveRetentionModeV1::Ring => {
                self.runs.push_back(RetainedLiveRunV1 {
                    report: report.clone(),
                    event_count,
                    serialized_bytes,
                });
                self.retained_events =
                    self.retained_events.saturating_add(event_count);
                self.retained_bytes =
                    self.retained_bytes.saturating_add(serialized_bytes);
                self.enforce_policy();
            }
            LiveRetentionModeV1::ExplicitExport => {
                // Keep only the latest complete report for an explicit export
                // action. No automatic raw run history is retained.
                self.clear_raw();
            }
        }
        Ok(())
    }

    fn selected_run(&self, run_id: u64) -> Option<ScenarioRunReportV1> {
        self.runs
            .iter()
            .find(|entry| entry.report.session_run_id == run_id)
            .map(|entry| entry.report.clone())
            .or_else(|| {
                self.latest_report
                    .as_ref()
                    .filter(|report| report.session_run_id == run_id)
                    .cloned()
            })
    }

    fn available_runs(&self) -> Vec<ScenarioRunReportV1> {
        if self.runs.is_empty() {
            self.latest_report.iter().cloned().collect()
        } else {
            self.runs.iter().map(|entry| entry.report.clone()).collect()
        }
    }
}

fn live_retention_mode_from_code(code: u32) -> Option<LiveRetentionModeV1> {
    match code {
        0 => Some(LiveRetentionModeV1::Latest),
        1 => Some(LiveRetentionModeV1::Ring),
        2 => Some(LiveRetentionModeV1::ExplicitExport),
        _ => None,
    }
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
    LiveSessionAlreadyOpen {
        schema_version: u32,
        session_id: String,
    },
    LiveSessionNotOpen {
        schema_version: u32,
    },
    LiveRunLength {
        schema_version: u32,
        requested: u32,
        available: u32,
        max_bytes: u32,
    },
    LiveRunUtf8 {
        schema_version: u32,
        message: String,
    },
    LiveRunJson {
        schema_version: u32,
        message: String,
    },
    LiveHistoryPolicy {
        schema_version: u32,
        field: String,
        requested: u64,
        max: u64,
    },
    LiveHistoryLimit {
        schema_version: u32,
        limit_kind: String,
        requested: u64,
        max: u64,
    },
    LiveHistoryRunNotFound {
        schema_version: u32,
        session_run_id: u64,
    },
    LiveHistoryEmpty {
        schema_version: u32,
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

fn clear_error_only() {
    *SCENARIO_ERROR_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) =
        String::new();
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

fn parse_manifest_bytes(
    length: u32,
) -> Result<super::scenario::ScenarioManifestV1, ScenarioTransportErrorV1> {
    let bytes = {
        let manifest = SCENARIO_MANIFEST_BYTES
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let requested = length as usize;
        if requested > manifest.len()
            || requested > MAX_SCENARIO_MANIFEST_BYTES
        {
            return Err(ScenarioTransportErrorV1::ManifestLength {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
                requested: length,
                available:
                    manifest.len().min(u32::MAX as usize) as u32,
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

    parse_and_validate_manifest_v1(source).map_err(|errors| {
        ScenarioTransportErrorV1::Validation {
            schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
            errors,
        }
    })
}

fn execute_manifest_bytes(
    length: u32,
    backend_code: u32,
) -> Result<ScenarioExecutionReportV1, ScenarioTransportErrorV1> {
    let backend = backend_from_code(backend_code).ok_or(
        ScenarioTransportErrorV1::BackendCode {
            schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
            backend_code,
        },
    )?;
    let manifest = parse_manifest_bytes(length)?;

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

fn clear_live_history_buffers(clear_export: bool) {
    *SCENARIO_LIVE_HISTORY_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) =
        String::new();
    if clear_export {
        *SCENARIO_LIVE_EXPORT_JSON
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) =
            String::new();
    }
}

fn clear_live_output_and_error() {
    *SCENARIO_LIVE_OUTPUT_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) =
        String::new();
    *SCENARIO_ERROR_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) =
        String::new();
}

fn store_live_output<T: Serialize>(
    value: &T,
) -> Result<(), ScenarioTransportErrorV1> {
    let json = serialize_bounded(value, MAX_SCENARIO_REPORT_BYTES)
        .map_err(|error| match error {
            BoundedJsonError::Limit => {
                ScenarioTransportErrorV1::ReportLimit {
                    schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
                    max_bytes: MAX_SCENARIO_REPORT_BYTES as u32,
                }
            }
            error => ScenarioTransportErrorV1::Serialize {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
                message: format!("{error:?}"),
            },
        })?;
    *SCENARIO_LIVE_OUTPUT_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = json;
    Ok(())
}


fn store_live_history_json<T: Serialize>(
    value: &T,
) -> Result<(), ScenarioTransportErrorV1> {
    let json = serialize_bounded(value, MAX_SCENARIO_REPORT_BYTES)
        .map_err(|error| match error {
            BoundedJsonError::Limit => {
                ScenarioTransportErrorV1::ReportLimit {
                    schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
                    max_bytes: MAX_SCENARIO_REPORT_BYTES as u32,
                }
            }
            error => ScenarioTransportErrorV1::Serialize {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
                message: format!("{error:?}"),
            },
        })?;
    *SCENARIO_LIVE_HISTORY_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = json;
    Ok(())
}

fn store_live_history_export(
    value: &ScenarioLiveHistoryExportV1,
) -> Result<(), ScenarioTransportErrorV1> {
    let json = serialize_bounded(value, MAX_LIVE_EXPORT_BYTES)
        .map_err(|error| match error {
            BoundedJsonError::Limit => {
                ScenarioTransportErrorV1::LiveHistoryLimit {
                    schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
                    limit_kind: "EXPORT_BYTES".to_owned(),
                    requested: 0,
                    max: MAX_LIVE_EXPORT_BYTES as u64,
                }
            }
            error => ScenarioTransportErrorV1::Serialize {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
                message: format!("{error:?}"),
            },
        })?;
    *SCENARIO_LIVE_EXPORT_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = json;
    Ok(())
}

fn live_export_available() -> bool {
    !SCENARIO_LIVE_EXPORT_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .is_empty()
}

fn live_history_status() -> Result<ScenarioLiveHistoryStatusV1, ScenarioTransportErrorV1> {
    let export_available = live_export_available();
    let history = SCENARIO_LIVE_HISTORY
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(history) = history.as_ref() else {
        return Err(ScenarioTransportErrorV1::LiveSessionNotOpen {
            schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
        });
    };
    Ok(history.status(export_available))
}

fn validate_live_history_policy(
    mode_code: u32,
    max_retained_runs: u32,
    max_retained_events: u32,
    max_retained_bytes: u32,
) -> Result<ScenarioLiveHistoryPolicyV1, ScenarioTransportErrorV1> {
    let retention_mode = live_retention_mode_from_code(mode_code)
        .ok_or_else(|| ScenarioTransportErrorV1::LiveHistoryPolicy {
            schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
            field: "retentionMode".to_owned(),
            requested: mode_code as u64,
            max: 2,
        })?;

    for (field, requested, max) in [
        ("maxRetainedRuns", max_retained_runs, MAX_LIVE_RETAINED_RUNS),
        (
            "maxRetainedEvents",
            max_retained_events,
            MAX_LIVE_RETAINED_EVENTS,
        ),
        (
            "maxRetainedBytes",
            max_retained_bytes,
            MAX_LIVE_RETAINED_BYTES,
        ),
    ] {
        if requested == 0 || requested > max {
            return Err(ScenarioTransportErrorV1::LiveHistoryPolicy {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
                field: field.to_owned(),
                requested: requested as u64,
                max: max as u64,
            });
        }
    }

    Ok(ScenarioLiveHistoryPolicyV1 {
        schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
        retention_mode,
        max_retained_runs,
        max_retained_events,
        max_retained_bytes,
        max_single_report_bytes: MAX_SCENARIO_REPORT_BYTES as u32,
        max_profile_points: MAX_LIVE_PROFILE_POINTS,
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScenarioLiveRunEnvelopeV1 {
    schema_version: u32,
    status: ScenarioLiveSessionStatusV1,
    run: ScenarioRunReportV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScenarioLiveStepBeginEnvelopeV1 {
    schema_version: u32,
    status: ScenarioLiveSessionStatusV1,
    begin: ScenarioStepBeginV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScenarioLiveStepEnvelopeV1 {
    schema_version: u32,
    status: ScenarioLiveSessionStatusV1,
    step: ScenarioStepReportV1,
}

#[no_mangle]
pub extern "C" fn amemory_scenario_live_session_available() -> u32 {
    let slot = SCENARIO_LIVE_SESSION
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    u32::from(slot.is_some())
}

#[no_mangle]
pub extern "C" fn amemory_scenario_live_open_json(
    length: u32,
    backend_code: u32,
) -> u32 {
    clear_live_output_and_error();

    let backend = match backend_from_code(backend_code) {
        Some(backend) => backend,
        None => {
            store_error(ScenarioTransportErrorV1::BackendCode {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
                backend_code,
            });
            return 0;
        }
    };
    if backend != ScenarioBackendV1::OptimizedCpu {
        store_error(ScenarioTransportErrorV1::Runner {
            schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
            error: ScenarioRunnerErrorV1::UnsupportedBackend {
                backend,
            },
        });
        return 0;
    }

    let mut slot = SCENARIO_LIVE_SESSION
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(existing) = slot.as_ref() {
        store_error(
            ScenarioTransportErrorV1::LiveSessionAlreadyOpen {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
                session_id: existing.status_v1().session_id,
            },
        );
        return 0;
    }

    let manifest = match parse_manifest_bytes(length) {
        Ok(manifest) => manifest,
        Err(error) => {
            store_error(error);
            return 0;
        }
    };
    let session = match open_cpu_scenario_session_v1(&manifest) {
        Ok(session) => session,
        Err(error) => {
            store_error(ScenarioTransportErrorV1::Runner {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
                error,
            });
            return 0;
        }
    };
    let status = session.status_v1();
    if let Err(error) = store_live_output(&status) {
        store_error(error);
        return 0;
    }

    *SCENARIO_LIVE_HISTORY
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) =
        Some(ScenarioLiveObserverHistoryV1::new(&status));
    clear_live_history_buffers(true);
    *slot = Some(session);
    1
}

#[no_mangle]
pub extern "C" fn amemory_scenario_live_status_refresh() -> u32 {
    clear_live_output_and_error();
    let slot = SCENARIO_LIVE_SESSION
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(session) = slot.as_ref() else {
        store_error(ScenarioTransportErrorV1::LiveSessionNotOpen {
            schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
        });
        return 0;
    };
    match store_live_output(&session.status_v1()) {
        Ok(()) => 1,
        Err(error) => {
            store_error(error);
            0
        }
    }
}


#[no_mangle]
pub extern "C" fn amemory_scenario_live_history_policy_set(
    mode_code: u32,
    max_retained_runs: u32,
    max_retained_events: u32,
    max_retained_bytes: u32,
) -> u32 {
    clear_error_only();
    *SCENARIO_LIVE_HISTORY_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) =
        String::new();

    let policy = match validate_live_history_policy(
        mode_code,
        max_retained_runs,
        max_retained_events,
        max_retained_bytes,
    ) {
        Ok(policy) => policy,
        Err(error) => {
            store_error(error);
            return 0;
        }
    };

    {
        let mut history = SCENARIO_LIVE_HISTORY
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(history) = history.as_mut() else {
            store_error(ScenarioTransportErrorV1::LiveSessionNotOpen {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
            });
            return 0;
        };
        history.set_policy(policy);
    }

    match live_history_status()
        .and_then(|status| {
            store_live_history_json(&status)?;
            Ok(status)
        })
    {
        Ok(_) => 1,
        Err(error) => {
            store_error(error);
            0
        }
    }
}

#[no_mangle]
pub extern "C" fn amemory_scenario_live_history_status_refresh() -> u32 {
    clear_error_only();
    *SCENARIO_LIVE_HISTORY_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) =
        String::new();
    match live_history_status()
        .and_then(|status| {
            store_live_history_json(&status)?;
            Ok(status)
        })
    {
        Ok(_) => 1,
        Err(error) => {
            store_error(error);
            0
        }
    }
}

#[no_mangle]
pub extern "C" fn amemory_scenario_live_history_clear() -> u32 {
    clear_error_only();
    {
        let mut history = SCENARIO_LIVE_HISTORY
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(history) = history.as_mut() else {
            store_error(ScenarioTransportErrorV1::LiveSessionNotOpen {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
            });
            return 0;
        };
        history.clear_history();
    }
    *SCENARIO_LIVE_OUTPUT_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) =
        String::new();
    *SCENARIO_LIVE_HISTORY_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) =
        String::new();
    1
}

#[no_mangle]
pub extern "C" fn amemory_scenario_live_history_export_run(
    session_run_id: u32,
) -> u32 {
    clear_error_only();
    let export = {
        let history = SCENARIO_LIVE_HISTORY
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(history) = history.as_ref() else {
            store_error(ScenarioTransportErrorV1::LiveSessionNotOpen {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
            });
            return 0;
        };
        let Some(report) = history.selected_run(session_run_id as u64) else {
            store_error(ScenarioTransportErrorV1::LiveHistoryRunNotFound {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
                session_run_id: session_run_id as u64,
            });
            return 0;
        };
        ScenarioLiveHistoryExportV1 {
            schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
            representation_id:
                "amemory-live-observer-history-json".to_owned(),
            representation_version: "0.1.0".to_owned(),
            export_kind: ScenarioLiveHistoryExportKindV1::SelectedRun,
            session_id: history.session_id.clone(),
            program_fingerprint: history.program_fingerprint.clone(),
            runs: vec![report],
        }
    };

    match store_live_history_export(&export) {
        Ok(()) => 1,
        Err(error) => {
            store_error(error);
            0
        }
    }
}

#[no_mangle]
pub extern "C" fn amemory_scenario_live_history_export_available() -> u32 {
    clear_error_only();
    let export = {
        let history = SCENARIO_LIVE_HISTORY
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(history) = history.as_ref() else {
            store_error(ScenarioTransportErrorV1::LiveSessionNotOpen {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
            });
            return 0;
        };
        let runs = history.available_runs();
        if runs.is_empty() {
            store_error(ScenarioTransportErrorV1::LiveHistoryEmpty {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
            });
            return 0;
        }
        ScenarioLiveHistoryExportV1 {
            schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
            representation_id:
                "amemory-live-observer-history-json".to_owned(),
            representation_version: "0.1.0".to_owned(),
            export_kind:
                ScenarioLiveHistoryExportKindV1::AvailableHistory,
            session_id: history.session_id.clone(),
            program_fingerprint: history.program_fingerprint.clone(),
            runs,
        }
    };

    match store_live_history_export(&export) {
        Ok(()) => 1,
        Err(error) => {
            store_error(error);
            0
        }
    }
}

#[no_mangle]
pub extern "C" fn amemory_scenario_live_history_export_clear() {
    *SCENARIO_LIVE_EXPORT_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) =
        String::new();
}

#[no_mangle]
pub extern "C" fn amemory_scenario_live_history_available() -> u32 {
    string_available(&SCENARIO_LIVE_HISTORY_JSON)
}

#[no_mangle]
pub extern "C" fn amemory_scenario_live_history_json_len() -> u32 {
    string_len(&SCENARIO_LIVE_HISTORY_JSON)
}

#[no_mangle]
pub extern "C" fn amemory_scenario_live_history_json_ptr() -> u32 {
    string_ptr(&SCENARIO_LIVE_HISTORY_JSON)
}

#[no_mangle]
pub extern "C" fn amemory_scenario_live_history_json_byte(
    index: u32,
) -> u32 {
    string_byte(&SCENARIO_LIVE_HISTORY_JSON, index)
}

#[no_mangle]
pub extern "C" fn amemory_scenario_live_history_export_json_available(
) -> u32 {
    string_available(&SCENARIO_LIVE_EXPORT_JSON)
}

#[no_mangle]
pub extern "C" fn amemory_scenario_live_history_export_json_len() -> u32 {
    string_len(&SCENARIO_LIVE_EXPORT_JSON)
}

#[no_mangle]
pub extern "C" fn amemory_scenario_live_history_export_json_ptr() -> u32 {
    string_ptr(&SCENARIO_LIVE_EXPORT_JSON)
}

#[no_mangle]
pub extern "C" fn amemory_scenario_live_history_export_json_byte(
    index: u32,
) -> u32 {
    string_byte(&SCENARIO_LIVE_EXPORT_JSON, index)
}

#[no_mangle]
pub extern "C" fn amemory_scenario_live_run_clear() {
    *SCENARIO_LIVE_RUN_BYTES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Vec::new();
    clear_live_output_and_error();
}

#[no_mangle]
pub extern "C" fn amemory_scenario_live_run_set_byte(
    index: u32,
    byte: u32,
) -> u32 {
    let index = index as usize;
    if index >= MAX_SCENARIO_MANIFEST_BYTES
        || byte > u8::MAX as u32
    {
        return 0;
    }
    let mut run = SCENARIO_LIVE_RUN_BYTES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if run.len() <= index {
        run.resize(index + 1, 0);
    }
    run[index] = byte as u8;
    1
}

fn parse_live_run_bytes(
    length: u32,
) -> Result<ScenarioRunV1, ScenarioTransportErrorV1> {
    let bytes = {
        let run = SCENARIO_LIVE_RUN_BYTES
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let requested = length as usize;
        if requested > run.len()
            || requested > MAX_SCENARIO_MANIFEST_BYTES
        {
            return Err(ScenarioTransportErrorV1::LiveRunLength {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
                requested: length,
                available: run.len().min(u32::MAX as usize) as u32,
                max_bytes: MAX_SCENARIO_MANIFEST_BYTES as u32,
            });
        }
        run[..requested].to_vec()
    };
    let source = std::str::from_utf8(&bytes).map_err(|error| {
        ScenarioTransportErrorV1::LiveRunUtf8 {
            schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
            message: error.to_string(),
        }
    })?;
    serde_json::from_str(source).map_err(|error| {
        ScenarioTransportErrorV1::LiveRunJson {
            schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
            message: error.to_string(),
        }
    })
}

#[no_mangle]
pub extern "C" fn amemory_scenario_live_step_begin_json(
    length: u32,
) -> u32 {
    clear_live_output_and_error();
    let run = match parse_live_run_bytes(length) {
        Ok(run) => run,
        Err(error) => {
            store_error(error);
            return 0;
        }
    };

    let mut slot = SCENARIO_LIVE_SESSION
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(session) = slot.as_mut() else {
        store_error(ScenarioTransportErrorV1::LiveSessionNotOpen {
            schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
        });
        return 0;
    };

    let begin = match begin_cpu_scenario_step_run_v1(session, &run) {
        Ok(begin) => begin,
        Err(error) => {
            store_error(ScenarioTransportErrorV1::Runner {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
                error,
            });
            return 0;
        }
    };
    let envelope = ScenarioLiveStepBeginEnvelopeV1 {
        schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
        status: session.status_v1(),
        begin,
    };
    match store_live_output(&envelope) {
        Ok(()) => 1,
        Err(error) => {
            store_error(error);
            0
        }
    }
}

#[no_mangle]
pub extern "C" fn amemory_scenario_live_step_json() -> u32 {
    clear_live_output_and_error();

    let mut slot = SCENARIO_LIVE_SESSION
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(session) = slot.as_mut() else {
        store_error(ScenarioTransportErrorV1::LiveSessionNotOpen {
            schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
        });
        return 0;
    };

    let step = match step_cpu_scenario_session_v1(session) {
        Ok(step) => step,
        Err(error) => {
            store_error(ScenarioTransportErrorV1::Runner {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
                error,
            });
            return 0;
        }
    };
    let envelope = ScenarioLiveStepEnvelopeV1 {
        schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
        status: session.status_v1(),
        step,
    };
    match store_live_output(&envelope) {
        Ok(()) => 1,
        Err(error) => {
            store_error(error);
            0
        }
    }
}

#[no_mangle]
pub extern "C" fn amemory_scenario_live_execute_json(
    length: u32,
) -> u32 {
    clear_live_output_and_error();
    let run = match parse_live_run_bytes(length) {
        Ok(run) => run,
        Err(error) => {
            store_error(error);
            return 0;
        }
    };

    let mut slot = SCENARIO_LIVE_SESSION
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(session) = slot.as_mut() else {
        store_error(ScenarioTransportErrorV1::LiveSessionNotOpen {
            schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
        });
        return 0;
    };

    let report = match run_cpu_scenario_session_once_v1(session, &run) {
        Ok(report) => report,
        Err(error) => {
            store_error(ScenarioTransportErrorV1::Runner {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
                error,
            });
            return 0;
        }
    };
    let envelope = ScenarioLiveRunEnvelopeV1 {
        schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
        status: session.status_v1(),
        run: report.clone(),
    };

    // Serialize the current run completely before mutating observer-history
    // state. A detailed TRACE/FULL payload is never partially published.
    let live_json = match serialize_bounded(
        &envelope,
        MAX_SCENARIO_REPORT_BYTES,
    ) {
        Ok(json) => json,
        Err(BoundedJsonError::Limit) => {
            store_error(ScenarioTransportErrorV1::ReportLimit {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
                max_bytes: MAX_SCENARIO_REPORT_BYTES as u32,
            });
            return 0;
        }
        Err(error) => {
            store_error(ScenarioTransportErrorV1::Serialize {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
                message: format!("{error:?}"),
            });
            return 0;
        }
    };

    {
        let mut history = SCENARIO_LIVE_HISTORY
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(history) = history.as_mut() else {
            store_error(ScenarioTransportErrorV1::LiveSessionNotOpen {
                schema_version: SCENARIO_TRANSPORT_SCHEMA_VERSION,
            });
            return 0;
        };
        if let Err(error) = history.record_run(&report) {
            store_error(error);
            return 0;
        }
    }

    *SCENARIO_LIVE_OUTPUT_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = live_json;
    1
}

#[no_mangle]
pub extern "C" fn amemory_scenario_live_close() -> u32 {
    let mut slot = SCENARIO_LIVE_SESSION
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let existed = slot.take().is_some();
    *SCENARIO_LIVE_RUN_BYTES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Vec::new();
    *SCENARIO_LIVE_HISTORY
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    clear_live_history_buffers(true);
    clear_live_output_and_error();
    u32::from(existed)
}

#[no_mangle]
pub extern "C" fn amemory_scenario_live_output_available() -> u32 {
    string_available(&SCENARIO_LIVE_OUTPUT_JSON)
}

#[no_mangle]
pub extern "C" fn amemory_scenario_live_output_json_len() -> u32 {
    string_len(&SCENARIO_LIVE_OUTPUT_JSON)
}

#[no_mangle]
pub extern "C" fn amemory_scenario_live_output_json_ptr() -> u32 {
    string_ptr(&SCENARIO_LIVE_OUTPUT_JSON)
}

#[no_mangle]
pub extern "C" fn amemory_scenario_live_output_json_byte(
    index: u32,
) -> u32 {
    string_byte(&SCENARIO_LIVE_OUTPUT_JSON, index)
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
        assert_eq!(registry.entries.len(), 20);
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
        assert_eq!(registry.entries[3].scenario_id, "shl32-lifecycle");
        assert_eq!(
            registry.entries[3].program_profile_id,
            "a-circuit:shift-shl32"
        );
        assert_eq!(registry.entries[4].scenario_id, "mul32-lifecycle");
        assert_eq!(
            registry.entries[4].program_profile_id,
            "a-circuit:mul32"
        );
        assert_eq!(
            registry.entries[5].scenario_id,
            "radix-memory8-lifecycle"
        );
        assert_eq!(
            registry.entries[5].program_profile_id,
            "a-circuit:memory-radix8"
        );
        for (index, scenario_id, profile_id) in [
            (6usize, "and32-lifecycle", "a-circuit:logic-and32"),
            (7usize, "or32-lifecycle", "a-circuit:logic-or32"),
            (8usize, "not32-lifecycle", "a-circuit:logic-not32"),
            (9usize, "test32-lifecycle", "a-circuit:logic-test32"),
            (10usize, "adc32-lifecycle", "a-circuit:arithmetic-adc32"),
            (11usize, "sub32-lifecycle", "a-circuit:arithmetic-sub32"),
            (12usize, "sbb32-lifecycle", "a-circuit:arithmetic-sbb32"),
            (13usize, "cmp32-lifecycle", "a-circuit:arithmetic-cmp32"),
            (14usize, "shr32-lifecycle", "a-circuit:shift-shr32"),
            (15usize, "sar32-lifecycle", "a-circuit:shift-sar32"),
            (16usize, "rol32-lifecycle", "a-circuit:rotate-rol32"),
            (17usize, "ror32-lifecycle", "a-circuit:rotate-ror32"),
            (18usize, "rcl32-lifecycle", "a-circuit:rotate-carry-rcl32"),
            (19usize, "rcr32-lifecycle", "a-circuit:rotate-carry-rcr32"),
        ] {
            assert_eq!(registry.entries[index].scenario_id, scenario_id);
            assert_eq!(
                registry.entries[index].program_profile_id,
                profile_id
            );
        }

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
        assert!(limits.live_session_retained);
        assert!(!limits.batch_session_retained);
        assert_eq!(
            limits.live_observer.default_retention_mode,
            LiveRetentionModeV1::Ring
        );
        assert_eq!(
            limits.live_observer.supported_retention_modes,
            vec![
                LiveRetentionModeV1::Latest,
                LiveRetentionModeV1::Ring,
                LiveRetentionModeV1::ExplicitExport,
            ]
        );
        assert_eq!(
            limits.live_observer.max_retained_runs,
            MAX_LIVE_RETAINED_RUNS
        );
        assert_eq!(
            limits.live_observer.max_retained_events,
            MAX_LIVE_RETAINED_EVENTS
        );
        assert_eq!(
            limits.live_observer.max_retained_bytes,
            MAX_LIVE_RETAINED_BYTES
        );
        assert_eq!(
            limits.live_observer.max_profile_points,
            MAX_LIVE_PROFILE_POINTS
        );
        assert_eq!(
            limits.live_observer.max_export_bytes,
            MAX_LIVE_EXPORT_BYTES as u32
        );
    }

    #[test]
    fn live_history_policy_codes_and_bounds_are_explicit() {
        assert_eq!(
            live_retention_mode_from_code(0),
            Some(LiveRetentionModeV1::Latest)
        );
        assert_eq!(
            live_retention_mode_from_code(1),
            Some(LiveRetentionModeV1::Ring)
        );
        assert_eq!(
            live_retention_mode_from_code(2),
            Some(LiveRetentionModeV1::ExplicitExport)
        );
        assert_eq!(live_retention_mode_from_code(3), None);

        assert!(validate_live_history_policy(
            1,
            DEFAULT_LIVE_RETAINED_RUNS,
            DEFAULT_LIVE_RETAINED_EVENTS,
            DEFAULT_LIVE_RETAINED_BYTES,
        )
        .is_ok());
        assert!(matches!(
            validate_live_history_policy(
                1,
                MAX_LIVE_RETAINED_RUNS + 1,
                DEFAULT_LIVE_RETAINED_EVENTS,
                DEFAULT_LIVE_RETAINED_BYTES,
            ),
            Err(ScenarioTransportErrorV1::LiveHistoryPolicy { .. })
        ));
        assert!(matches!(
            validate_live_history_policy(
                1,
                DEFAULT_LIVE_RETAINED_RUNS,
                0,
                DEFAULT_LIVE_RETAINED_BYTES,
            ),
            Err(ScenarioTransportErrorV1::LiveHistoryPolicy { .. })
        ));
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
