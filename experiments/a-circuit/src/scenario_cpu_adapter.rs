use super::{
    arithmetic_effect_n::{
        configure_arithmetic32_session,
        prepare_arithmetic32_session_program,
        project_arithmetic32_session_result, web_prove_arithmetic,
    },
    logic_effect_n::{
        configure_logic32_session, prepare_logic32_session_program,
        project_logic32_session_result, web_prove_logic,
    },
    memory_n::{
        configure_radix_memory_session,
        prepare_radix_memory_session_program,
        project_radix_memory_session_result, web_prove_radix_memory,
    },
    mux_n::{
        configure_mux1_session, prepare_mux1_session_program,
        project_mux1_session_result, web_prove_mux1,
    },
    mul32_n::{
        configure_mul32_session, prepare_mul32_session_program,
        project_mul32_session_result, web_prove_mul32,
    },
    proof_n::{
        ProofRuntimeSession, WebProofLoadStage, WebProofPrepareStage,
    },
    scenario::ScenarioProgramProfileV1,
    shift32_n::{
        configure_shift32_session, prepare_shift32_session_program,
        project_shift32_session_result, web_prove_shift32,
    },
};
use amemory_optimized_cpu_probe::Handle;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

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
type FreshInstanceFn = fn(
    &BTreeMap<String, Value>,
) -> Result<ScenarioNormalizedResultV1, String>;
type ScalarOracleFn =
    fn(&BTreeMap<String, Value>) -> Result<BTreeMap<String, Value>, String>;

enum CpuScenarioImplementation {
    Direct {
        prepare: PrepareFn,
        configure: ConfigureFn,
        project: ProjectFn,
        fresh_instance: FreshInstanceFn,
        scalar_oracle: ScalarOracleFn,
    },
    Logic32 {
        op: u32,
    },
    Arithmetic32 {
        op: u32,
    },
    Shift32 {
        op: u32,
    },
}

pub(crate) struct CpuScenarioAdapter {
    profile_id: &'static str,
    family: &'static str,
    program_id: &'static str,
    aset_source: &'static str,
    implementation: CpuScenarioImplementation,
}

impl CpuScenarioAdapter {
    pub(crate) fn canonical_program_profile(&self) -> ScenarioProgramProfileV1 {
        ScenarioProgramProfileV1 {
            profile_id: self.profile_id.to_owned(),
            family: self.family.to_owned(),
            program_id: self.program_id.to_owned(),
            aset_source: self.aset_source.to_owned(),
        }
    }

    pub(crate) fn prepare(&self) -> Option<WebProofPrepareStage> {
        match self.implementation {
            CpuScenarioImplementation::Direct { prepare, .. } => prepare(),
            CpuScenarioImplementation::Logic32 { op } => {
                prepare_logic32_session_program(op)
            }
            CpuScenarioImplementation::Arithmetic32 { .. } => {
                prepare_arithmetic32_session_program()
            }
            CpuScenarioImplementation::Shift32 { .. } => {
                prepare_shift32_session_program()
            }
        }
    }

    pub(crate) fn configure(
        &self,
        session: &mut ProofRuntimeSession,
        load: &WebProofLoadStage,
        inputs: &BTreeMap<String, Value>,
    ) -> Result<ConfiguredRun, String> {
        match self.implementation {
            CpuScenarioImplementation::Direct { configure, .. } => {
                configure(session, load, inputs)
            }
            CpuScenarioImplementation::Logic32 { op } => {
                configure_logic32_from_inputs(op, session, load, inputs)
            }
            CpuScenarioImplementation::Arithmetic32 { op } => {
                configure_arithmetic32_from_inputs(
                    op, session, load, inputs,
                )
            }
            CpuScenarioImplementation::Shift32 { op } => {
                configure_shift32_from_inputs(op, session, load, inputs)
            }
        }
    }

    pub(crate) fn project(
        &self,
        session: &ProofRuntimeSession,
        load: &WebProofLoadStage,
    ) -> Result<ScenarioNormalizedResultV1, String> {
        match self.implementation {
            CpuScenarioImplementation::Direct { project, .. } => {
                project(session, load)
            }
            CpuScenarioImplementation::Logic32 { .. } => {
                project_logic32_result(session, load)
            }
            CpuScenarioImplementation::Arithmetic32 { .. } => {
                project_arithmetic32_result(session, load)
            }
            CpuScenarioImplementation::Shift32 { .. } => {
                project_shift32_result(session, load)
            }
        }
    }

    pub(crate) fn fresh_instance(
        &self,
        inputs: &BTreeMap<String, Value>,
    ) -> Result<ScenarioNormalizedResultV1, String> {
        match self.implementation {
            CpuScenarioImplementation::Direct {
                fresh_instance, ..
            } => fresh_instance(inputs),
            CpuScenarioImplementation::Logic32 { op } => {
                fresh_instance_logic32_result(op, inputs)
            }
            CpuScenarioImplementation::Arithmetic32 { op } => {
                fresh_instance_arithmetic32_result(op, inputs)
            }
            CpuScenarioImplementation::Shift32 { op } => {
                fresh_instance_shift32_result(op, inputs)
            }
        }
    }

    pub(crate) fn scalar_oracle(
        &self,
        inputs: &BTreeMap<String, Value>,
    ) -> Result<BTreeMap<String, Value>, String> {
        match self.implementation {
            CpuScenarioImplementation::Direct {
                scalar_oracle, ..
            } => scalar_oracle(inputs),
            CpuScenarioImplementation::Logic32 { op } => {
                scalar_logic32_result(op, inputs)
            }
            CpuScenarioImplementation::Arithmetic32 { op } => {
                scalar_arithmetic32_result(op, inputs)
            }
            CpuScenarioImplementation::Shift32 { op } => {
                scalar_shift32_result(op, inputs)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CpuScenarioAdapterResolutionErrorV1 {
    UnsupportedProgramProfile {
        profile_id: String,
    },
    ProgramProfileMismatch {
        requested: ScenarioProgramProfileV1,
        canonical: ScenarioProgramProfileV1,
    },
}

pub(crate) fn resolve_cpu_scenario_adapter(
    requested: &ScenarioProgramProfileV1,
) -> Result<&'static CpuScenarioAdapter, CpuScenarioAdapterResolutionErrorV1> {
    let adapter = CPU_SCENARIO_ADAPTERS
        .iter()
        .find(|adapter| adapter.profile_id == requested.profile_id)
        .ok_or_else(|| {
            CpuScenarioAdapterResolutionErrorV1::UnsupportedProgramProfile {
                profile_id: requested.profile_id.clone(),
            }
        })?;

    let canonical = adapter.canonical_program_profile();
    if &canonical != requested {
        return Err(
            CpuScenarioAdapterResolutionErrorV1::ProgramProfileMismatch {
                requested: requested.clone(),
                canonical,
            },
        );
    }
    Ok(adapter)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ConfiguredRun {
    pub(crate) initial: Handle,
    pub(crate) links_before: u32,
    pub(crate) links_after: u32,
}

const CPU_SCENARIO_ADAPTERS: &[CpuScenarioAdapter] = &[
    CpuScenarioAdapter {
        profile_id: "a-circuit:mux1",
        family: "mux",
        program_id: "mux1",
        aset_source: "builtin:a-circuit/mux1",
        implementation: CpuScenarioImplementation::Direct {
            prepare: prepare_mux1_session_program,
            configure: configure_mux1_from_inputs,
            project: project_mux1_result,
            fresh_instance: fresh_instance_mux1_result,
            scalar_oracle: scalar_mux1_result,
        },
    },
    CpuScenarioAdapter {
        profile_id: "a-circuit:logic-and32",
        family: "logic-effect",
        program_id: "and32",
        aset_source: "builtin:a-circuit/logic-effect/and32",
        implementation: CpuScenarioImplementation::Logic32 { op: 1 },
    },
    CpuScenarioAdapter {
        profile_id: "a-circuit:logic-or32",
        family: "logic-effect",
        program_id: "or32",
        aset_source: "builtin:a-circuit/logic-effect/or32",
        implementation: CpuScenarioImplementation::Logic32 { op: 2 },
    },
    CpuScenarioAdapter {
        profile_id: "a-circuit:logic-xor32",
        family: "logic-effect",
        program_id: "xor32",
        aset_source: "builtin:a-circuit/logic-effect/xor32",
        implementation: CpuScenarioImplementation::Logic32 { op: 3 },
    },
    CpuScenarioAdapter {
        profile_id: "a-circuit:logic-not32",
        family: "logic-effect",
        program_id: "not32",
        aset_source: "builtin:a-circuit/logic-effect/not32",
        implementation: CpuScenarioImplementation::Logic32 { op: 4 },
    },
    CpuScenarioAdapter {
        profile_id: "a-circuit:logic-test32",
        family: "logic-effect",
        program_id: "test32",
        aset_source: "builtin:a-circuit/logic-effect/test32",
        implementation: CpuScenarioImplementation::Logic32 { op: 5 },
    },
    CpuScenarioAdapter {
        profile_id: "a-circuit:arithmetic-add32",
        family: "arithmetic-effect",
        program_id: "add32",
        aset_source: "builtin:a-circuit/arithmetic-effect/add32",
        implementation: CpuScenarioImplementation::Arithmetic32 { op: 6 },
    },
    CpuScenarioAdapter {
        profile_id: "a-circuit:arithmetic-adc32",
        family: "arithmetic-effect",
        program_id: "adc32",
        aset_source: "builtin:a-circuit/arithmetic-effect/adc32",
        implementation: CpuScenarioImplementation::Arithmetic32 { op: 7 },
    },
    CpuScenarioAdapter {
        profile_id: "a-circuit:arithmetic-sub32",
        family: "arithmetic-effect",
        program_id: "sub32",
        aset_source: "builtin:a-circuit/arithmetic-effect/sub32",
        implementation: CpuScenarioImplementation::Arithmetic32 { op: 8 },
    },
    CpuScenarioAdapter {
        profile_id: "a-circuit:arithmetic-sbb32",
        family: "arithmetic-effect",
        program_id: "sbb32",
        aset_source: "builtin:a-circuit/arithmetic-effect/sbb32",
        implementation: CpuScenarioImplementation::Arithmetic32 { op: 9 },
    },
    CpuScenarioAdapter {
        profile_id: "a-circuit:arithmetic-cmp32",
        family: "arithmetic-effect",
        program_id: "cmp32",
        aset_source: "builtin:a-circuit/arithmetic-effect/cmp32",
        implementation: CpuScenarioImplementation::Arithmetic32 { op: 10 },
    },
    CpuScenarioAdapter {
        profile_id: "a-circuit:shift-shl32",
        family: "shift-effect",
        program_id: "shl32",
        aset_source: "builtin:a-circuit/shift32/shl",
        implementation: CpuScenarioImplementation::Shift32 { op: 13 },
    },
    CpuScenarioAdapter {
        profile_id: "a-circuit:shift-shr32",
        family: "shift-effect",
        program_id: "shr32",
        aset_source: "builtin:a-circuit/shift32/shr",
        implementation: CpuScenarioImplementation::Shift32 { op: 14 },
    },
    CpuScenarioAdapter {
        profile_id: "a-circuit:shift-sar32",
        family: "shift-effect",
        program_id: "sar32",
        aset_source: "builtin:a-circuit/shift32/sar",
        implementation: CpuScenarioImplementation::Shift32 { op: 15 },
    },
    CpuScenarioAdapter {
        profile_id: "a-circuit:mul32",
        family: "mul",
        program_id: "mul32",
        aset_source: "builtin:a-circuit/mul32",
        implementation: CpuScenarioImplementation::Direct {
            prepare: prepare_mul32_session_program,
            configure: configure_mul32_from_inputs,
            project: project_mul32_result,
            fresh_instance: fresh_instance_mul32_result,
            scalar_oracle: scalar_mul32_result,
        },
    },
    CpuScenarioAdapter {
        profile_id: "a-circuit:memory-radix8",
        family: "memory",
        program_id: "radix-page8",
        aset_source: "builtin:a-circuit/memory-radix8",
        implementation: CpuScenarioImplementation::Direct {
            prepare: prepare_radix_memory_session_program,
            configure: configure_radix_memory_from_inputs,
            project: project_radix_memory_result,
            fresh_instance: fresh_instance_radix_memory_result,
            scalar_oracle: scalar_radix_memory_result,
        },
    },
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScenarioNormalizedResultV1 {
    pub(crate) fields: BTreeMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) result_recursive_wire: Option<String>,
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

fn fresh_instance_mux1_result(
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

fn wide64_normalized(
    lo: u32,
    hi: u32,
    result_recursive_wire: String,
) -> ScenarioNormalizedResultV1 {
    let mut fields = BTreeMap::new();
    fields.insert("lo".to_owned(), canonical_word32(lo));
    fields.insert("hi".to_owned(), canonical_word32(hi));
    ScenarioNormalizedResultV1 {
        fields,
        result_recursive_wire: Some(result_recursive_wire),
    }
}

fn logic32_inputs(
    op: u32,
    inputs: &BTreeMap<String, Value>,
) -> Result<(u32, u32), String> {
    let a = word32_input(inputs, "A")?;
    let b = if op == 4 {
        0
    } else {
        word32_input(inputs, "B")?
    };
    Ok((a, b))
}

fn configure_logic32_from_inputs(
    op: u32,
    session: &mut ProofRuntimeSession,
    load: &WebProofLoadStage,
    inputs: &BTreeMap<String, Value>,
) -> Result<ConfiguredRun, String> {
    let (a, b) = logic32_inputs(op, inputs)?;
    let (initial, before, after) =
        configure_logic32_session(session, load, op, a, b)
            .ok_or_else(|| format!("Logic32 op {op} configuration failed"))?;
    Ok(ConfiguredRun {
        initial,
        links_before: before as u32,
        links_after: after as u32,
    })
}

fn project_logic32_result(
    session: &ProofRuntimeSession,
    load: &WebProofLoadStage,
) -> Result<ScenarioNormalizedResultV1, String> {
    let projected = project_logic32_session_result(session, load)
        .ok_or_else(|| "Logic32 result projection failed".to_owned())?;
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

fn fresh_instance_logic32_result(
    op: u32,
    inputs: &BTreeMap<String, Value>,
) -> Result<ScenarioNormalizedResultV1, String> {
    let (a, b) = logic32_inputs(op, inputs)?;
    let proof = web_prove_logic(op, a, b)
        .ok_or_else(|| format!("fresh Logic32 op {op} oracle failed"))?;
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

fn arithmetic32_inputs(
    op: u32,
    inputs: &BTreeMap<String, Value>,
) -> Result<(u32, u32, u32), String> {
    let a = word32_input(inputs, "A")?;
    let b = word32_input(inputs, "B")?;
    let input_flag = if matches!(op, 7 | 9) {
        bit_input(inputs, "CF")? as u32
    } else {
        0
    };
    Ok((a, b, input_flag))
}

fn configure_arithmetic32_from_inputs(
    op: u32,
    session: &mut ProofRuntimeSession,
    load: &WebProofLoadStage,
    inputs: &BTreeMap<String, Value>,
) -> Result<ConfiguredRun, String> {
    let (a, b, input_flag) = arithmetic32_inputs(op, inputs)?;
    let (initial, before, after) =
        configure_arithmetic32_session(
            session, load, op, a, b, input_flag,
        )
        .ok_or_else(|| {
            format!("Arithmetic32 op {op} configuration failed")
        })?;
    Ok(ConfiguredRun {
        initial,
        links_before: before as u32,
        links_after: after as u32,
    })
}

fn project_arithmetic32_result(
    session: &ProofRuntimeSession,
    load: &WebProofLoadStage,
) -> Result<ScenarioNormalizedResultV1, String> {
    let projected = project_arithmetic32_session_result(session, load)
        .ok_or_else(|| "Arithmetic32 result projection failed".to_owned())?;
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

fn fresh_instance_arithmetic32_result(
    op: u32,
    inputs: &BTreeMap<String, Value>,
) -> Result<ScenarioNormalizedResultV1, String> {
    let (a, b, input_flag) = arithmetic32_inputs(op, inputs)?;
    let proof = web_prove_arithmetic(op, a, b, input_flag)
        .ok_or_else(|| {
            format!("fresh Arithmetic32 op {op} oracle failed")
        })?;
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

fn count8_input(
    inputs: &BTreeMap<String, Value>,
    key: &str,
) -> Result<u8, String> {
    let value = inputs
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("missing COUNT8 input {key}"))?;
    u8::try_from(value)
        .map_err(|_| format!("COUNT8 input {key} outside 0..255"))
}

fn u8_input(
    inputs: &BTreeMap<String, Value>,
    key: &str,
) -> Result<u8, String> {
    let value = inputs
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("missing U8 input {key}"))?;
    u8::try_from(value)
        .map_err(|_| format!("U8 input {key} outside 0..255"))
}

fn shift32_inputs(
    inputs: &BTreeMap<String, Value>,
) -> Result<(u32, u8), String> {
    Ok((
        word32_input(inputs, "VALUE")?,
        count8_input(inputs, "COUNT")?,
    ))
}

fn configure_shift32_from_inputs(
    op: u32,
    session: &mut ProofRuntimeSession,
    load: &WebProofLoadStage,
    inputs: &BTreeMap<String, Value>,
) -> Result<ConfiguredRun, String> {
    let (value, count) = shift32_inputs(inputs)?;
    let (initial, before, after) =
        configure_shift32_session(session, load, op, value, count)
            .ok_or_else(|| format!("Shift32 op {op} configuration failed"))?;
    Ok(ConfiguredRun {
        initial,
        links_before: before as u32,
        links_after: after as u32,
    })
}

fn project_shift32_result(
    session: &ProofRuntimeSession,
    load: &WebProofLoadStage,
) -> Result<ScenarioNormalizedResultV1, String> {
    let projected = project_shift32_session_result(session, load)
        .ok_or_else(|| "Shift32 result projection failed".to_owned())?;
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

fn fresh_instance_shift32_result(
    op: u32,
    inputs: &BTreeMap<String, Value>,
) -> Result<ScenarioNormalizedResultV1, String> {
    let (value, count) = shift32_inputs(inputs)?;
    let proof = web_prove_shift32(op, value, u32::from(count))
        .ok_or_else(|| format!("fresh Shift32 op {op} oracle failed"))?;
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

fn configure_mul32_from_inputs(
    session: &mut ProofRuntimeSession,
    load: &WebProofLoadStage,
    inputs: &BTreeMap<String, Value>,
) -> Result<ConfiguredRun, String> {
    let a = word32_input(inputs, "A")?;
    let b = word32_input(inputs, "B")?;
    let (initial, before, after) =
        configure_mul32_session(session, load, a, b)
            .ok_or_else(|| "MUL32 configuration failed".to_owned())?;
    Ok(ConfiguredRun {
        initial,
        links_before: before as u32,
        links_after: after as u32,
    })
}

fn project_mul32_result(
    session: &ProofRuntimeSession,
    load: &WebProofLoadStage,
) -> Result<ScenarioNormalizedResultV1, String> {
    let projected = project_mul32_session_result(session, load)
        .ok_or_else(|| "MUL32 result projection failed".to_owned())?;
    Ok(wide64_normalized(
        projected.lo,
        projected.hi,
        projected.result_recursive_wire,
    ))
}

fn fresh_instance_mul32_result(
    inputs: &BTreeMap<String, Value>,
) -> Result<ScenarioNormalizedResultV1, String> {
    let a = word32_input(inputs, "A")?;
    let b = word32_input(inputs, "B")?;
    let proof = web_prove_mul32(a, b)
        .ok_or_else(|| "fresh MUL32 oracle failed".to_owned())?;
    Ok(wide64_normalized(
        proof.outcome.lo,
        proof.outcome.hi,
        proof.proof.result.result_anum,
    ))
}

fn radix_memory_normalized(
    before: u32,
    after: u32,
    old_after: u32,
    result_recursive_wire: Option<String>,
) -> ScenarioNormalizedResultV1 {
    let mut fields = BTreeMap::new();
    fields.insert("before".to_owned(), Value::from(before));
    fields.insert("after".to_owned(), Value::from(after));
    fields.insert("oldAfter".to_owned(), Value::from(old_after));
    ScenarioNormalizedResultV1 {
        fields,
        result_recursive_wire,
    }
}

fn configure_radix_memory_from_inputs(
    session: &mut ProofRuntimeSession,
    load: &WebProofLoadStage,
    inputs: &BTreeMap<String, Value>,
) -> Result<ConfiguredRun, String> {
    let offset = u8_input(inputs, "OFFSET")?;
    let value = u8_input(inputs, "VALUE")?;
    let (initial, before, after) =
        configure_radix_memory_session(
            session,
            load,
            offset,
            value,
        )
        .ok_or_else(|| "M6A radix-memory configuration failed".to_owned())?;
    Ok(ConfiguredRun {
        initial,
        links_before: before as u32,
        links_after: after as u32,
    })
}

fn project_radix_memory_result(
    session: &ProofRuntimeSession,
    load: &WebProofLoadStage,
) -> Result<ScenarioNormalizedResultV1, String> {
    let projected =
        project_radix_memory_session_result(session, load)
            .ok_or_else(|| {
                "M6A radix-memory result projection failed".to_owned()
            })?;
    Ok(radix_memory_normalized(
        projected.before_value,
        projected.after_value,
        projected.old_after_value,
        Some(projected.result_recursive_wire),
    ))
}

fn fresh_instance_radix_memory_result(
    inputs: &BTreeMap<String, Value>,
) -> Result<ScenarioNormalizedResultV1, String> {
    let offset = u8_input(inputs, "OFFSET")?;
    let value = u8_input(inputs, "VALUE")?;
    let proof = web_prove_radix_memory(offset, value)
        .ok_or_else(|| "fresh M6A radix-memory witness failed".to_owned())?;
    Ok(radix_memory_normalized(
        proof.outcome.before_value,
        proof.outcome.after_value,
        proof.outcome.old_after_value,
        Some(proof.proof.result.result_anum),
    ))
}

fn scalar_radix_memory_result(
    inputs: &BTreeMap<String, Value>,
) -> Result<BTreeMap<String, Value>, String> {
    let _offset = u8_input(inputs, "OFFSET")?;
    let value = u8_input(inputs, "VALUE")?;
    Ok(radix_memory_normalized(
        0,
        u32::from(value),
        0,
        None,
    )
    .fields)
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


const SCALAR_CF: u32 = 1 << 0;
const SCALAR_PF: u32 = 1 << 2;
const SCALAR_AF: u32 = 1 << 4;
const SCALAR_ZF: u32 = 1 << 6;
const SCALAR_SF: u32 = 1 << 7;
const SCALAR_OF: u32 = 1 << 11;
const SCALAR_STATUS_FLAGS: u32 =
    SCALAR_CF | SCALAR_PF | SCALAR_AF | SCALAR_ZF | SCALAR_SF | SCALAR_OF;

fn scalar_effect32_fields(
    value: u32,
    writeback: u8,
    defined_mask: u32,
    value_mask: u32,
    undefined_mask: u32,
    preserve_mask: u32,
) -> BTreeMap<String, Value> {
    let mut fields = BTreeMap::new();
    fields.insert("value".to_owned(), canonical_word32(value));
    fields.insert("writeback".to_owned(), Value::from(writeback));
    fields.insert("definedMask".to_owned(), canonical_word32(defined_mask));
    fields.insert("valueMask".to_owned(), canonical_word32(value_mask));
    fields.insert(
        "undefinedMask".to_owned(),
        canonical_word32(undefined_mask),
    );
    fields.insert("preserveMask".to_owned(), canonical_word32(preserve_mask));
    fields
}

fn scalar_status_value_mask(
    cf: bool,
    pf: bool,
    af: bool,
    zf: bool,
    sf: bool,
    of: bool,
) -> u32 {
    let mut mask = 0u32;
    for (flag, set) in [
        (SCALAR_CF, cf),
        (SCALAR_PF, pf),
        (SCALAR_AF, af),
        (SCALAR_ZF, zf),
        (SCALAR_SF, sf),
        (SCALAR_OF, of),
    ] {
        if set {
            mask |= flag;
        }
    }
    mask
}

fn scalar_even_parity_low_byte(value: u32) -> bool {
    (value as u8).count_ones() % 2 == 0
}

fn scalar_mux1_result(
    inputs: &BTreeMap<String, Value>,
) -> Result<BTreeMap<String, Value>, String> {
    let select = bit_input(inputs, "S")?;
    let a = bit_input(inputs, "A")?;
    let b = bit_input(inputs, "B")?;
    let mut fields = BTreeMap::new();
    fields.insert(
        "value".to_owned(),
        Value::from((if select == 0 { a } else { b }) as u32),
    );
    Ok(fields)
}

fn scalar_logic32_result(
    op: u32,
    inputs: &BTreeMap<String, Value>,
) -> Result<BTreeMap<String, Value>, String> {
    let (a, b) = logic32_inputs(op, inputs)?;
    if op == 4 {
        return Ok(scalar_effect32_fields(
            !a,
            1,
            0,
            0,
            0,
            SCALAR_STATUS_FLAGS,
        ));
    }

    let (value, writeback) = match op {
        1 => (a & b, 1),
        2 => (a | b, 1),
        3 => (a ^ b, 1),
        5 => (a & b, 0),
        _ => return Err(format!("unsupported Logic32 op {op}")),
    };
    let defined =
        SCALAR_CF | SCALAR_PF | SCALAR_ZF | SCALAR_SF | SCALAR_OF;
    let value_mask = scalar_status_value_mask(
        false,
        scalar_even_parity_low_byte(value),
        false,
        value == 0,
        value >> 31 != 0,
        false,
    ) & defined;
    Ok(scalar_effect32_fields(
        value, writeback, defined, value_mask, SCALAR_AF, 0,
    ))
}

fn scalar_arithmetic32_result(
    op: u32,
    inputs: &BTreeMap<String, Value>,
) -> Result<BTreeMap<String, Value>, String> {
    let (a, b, input_flag) = arithmetic32_inputs(op, inputs)?;
    let x = input_flag;
    let (mode, writeback) = match op {
        6 | 7 => (0u8, 1u8),
        8 | 9 => (1u8, 1u8),
        10 => (1u8, 0u8),
        _ => return Err(format!("unsupported Arithmetic32 op {op}")),
    };

    let (value, cf, af, of) = if mode == 0 {
        let total = u64::from(a) + u64::from(b) + u64::from(x);
        let signed =
            i64::from(a as i32) + i64::from(b as i32) + i64::from(x);
        (
            total as u32,
            total > u64::from(u32::MAX),
            (a & 0x0f) + (b & 0x0f) + x > 0x0f,
            signed > i64::from(i32::MAX)
                || signed < i64::from(i32::MIN),
        )
    } else {
        let subtrahend = u64::from(b) + u64::from(x);
        let signed =
            i64::from(a as i32) - i64::from(b as i32) - i64::from(x);
        (
            a.wrapping_sub(b).wrapping_sub(x),
            u64::from(a) < subtrahend,
            (a & 0x0f) < (b & 0x0f) + x,
            signed > i64::from(i32::MAX)
                || signed < i64::from(i32::MIN),
        )
    };

    let value_mask = scalar_status_value_mask(
        cf,
        scalar_even_parity_low_byte(value),
        af,
        value == 0,
        value >> 31 != 0,
        of,
    );
    Ok(scalar_effect32_fields(
        value,
        writeback,
        SCALAR_STATUS_FLAGS,
        value_mask,
        0,
        0,
    ))
}

fn scalar_shift32_result(
    op: u32,
    inputs: &BTreeMap<String, Value>,
) -> Result<BTreeMap<String, Value>, String> {
    let (value, count) = shift32_inputs(inputs)?;
    let masked = u32::from(count & 31);
    if masked == 0 {
        return Ok(scalar_effect32_fields(
            value, 1, 0, 0, 0, SCALAR_STATUS_FLAGS,
        ));
    }

    let (result, cf, of) = match op {
        13 => {
            let result = value.wrapping_shl(masked);
            let cf = ((value >> (32 - masked)) & 1) != 0;
            let of = masked == 1
                && ((result >> 31 != 0) ^ cf);
            (result, cf, of)
        }
        14 => {
            let result = value >> masked;
            let cf = ((value >> (masked - 1)) & 1) != 0;
            let of = masked == 1 && (value >> 31 != 0);
            (result, cf, of)
        }
        15 => {
            let result = ((value as i32) >> masked) as u32;
            let cf = ((value >> (masked - 1)) & 1) != 0;
            (result, cf, false)
        }
        _ => return Err(format!("unsupported Shift32 op {op}")),
    };

    let pf = scalar_even_parity_low_byte(result);
    let zf = result == 0;
    let sf = result >> 31 != 0;

    let mut defined = SCALAR_CF | SCALAR_PF | SCALAR_ZF | SCALAR_SF;
    let mut undefined = SCALAR_AF;
    if masked == 1 {
        defined |= SCALAR_OF;
    } else {
        undefined |= SCALAR_OF;
    }
    let value_mask =
        scalar_status_value_mask(cf, pf, false, zf, sf, of) & defined;
    Ok(scalar_effect32_fields(
        result, 1, defined, value_mask, undefined, 0,
    ))
}

fn scalar_mul32_result(
    inputs: &BTreeMap<String, Value>,
) -> Result<BTreeMap<String, Value>, String> {
    let a = word32_input(inputs, "A")?;
    let b = word32_input(inputs, "B")?;
    let product = u64::from(a) * u64::from(b);
    let mut fields = BTreeMap::new();
    fields.insert("lo".to_owned(), canonical_word32(product as u32));
    fields.insert("hi".to_owned(), canonical_word32((product >> 32) as u32));
    Ok(fields)
}
