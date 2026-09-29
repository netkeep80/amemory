use crate::{
    architectural_state_n::{
        web_prove_architectural_state_add,
        web_prove_architectural_state_add_ecx,
        web_prove_architectural_state_mul,
    },
    arithmetic_effect_n::{web_prove_arithmetic, web_run_arithmetic},
    logic_effect_n::{web_prove_logic, web_run_logic},
    memory_n::web_prove_radix_memory,
    memory32_n::web_prove_memory32,
    memory_word_n::{web_prove_word16_memory, web_prove_word32_memory},
    instruction_fetch_n::web_prove_instruction_fetch,
    stack_n::web_prove_stack_roundtrip,
    mul32_n::web_prove_mul32,
    mul_effect_n::web_prove_mul_effect,
    mux_n::{web_prove_mux1, web_prove_mux32},
    rotate32_n::{web_prove_rotate32, web_run_rotate32},
    rotate_carry32_n::{web_prove_rotate_carry32, web_run_rotate_carry32},
    shift32_n::{web_prove_shift32, web_run_shift32},
    unary_arith_n::{web_prove_unary32, web_run_unary32},
    proof_n::{packed_gpu_carrier_words, WebStructuralProof},
};
use serde::Serialize;
use std::sync::Mutex;

const FLAG_CF: u32 = 1 << 0;
const FLAG_PF: u32 = 1 << 2;
const FLAG_AF: u32 = 1 << 4;
const FLAG_ZF: u32 = 1 << 6;
const FLAG_SF: u32 = 1 << 7;
const FLAG_OF: u32 = 1 << 11;
const STATUS_FLAGS: u32 = FLAG_CF | FLAG_PF | FLAG_AF | FLAG_ZF | FLAG_SF | FLAG_OF;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct LabOutcome {
    value: u32,
    value_hi: u32,
    writeback: u32,
    defined_mask: u32,
    value_mask: u32,
    undefined_mask: u32,
    preserve_mask: u32,
    reactions: u32,
    links_after_build: u32,
    links_after_first: u32,
    steady_link_delta: u32,
    quiescent: u32,
}

const LAB_RESULT_SCHEMA_VERSION: u32 = 1;
const LAB_RESULT_REPRESENTATION_ID: &str = "amemory-i386-lab-result-json";
const LAB_RESULT_REPRESENTATION_VERSION: &str = "0.1.0";
const DEFAULT_LAB_INSTANCE_ID: u32 = 0;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LabOperation {
    opcode: u32,
    a: u32,
    b: u32,
    input_flag: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LabResultEnvelope {
    schema_version: u32,
    representation_id: &'static str,
    representation_version: &'static str,
    instance_id: u32,
    witness_kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    operation: Option<LabOperation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    outcome: Option<LabOutcome>,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload: Option<serde_json::Value>,
    compact_proof_available: bool,
}

#[cfg(test)]
static LAST_PROOF_JSON: Mutex<String> = Mutex::new(String::new());

const MAX_LAB_INSTANCES: usize = 4;

#[derive(Default)]
struct LabInstanceState {
    active: bool,
    result_json: String,
    compact_proof_json: String,
    gpu_carrier_words: Vec<u32>,
}

impl LabInstanceState {
    fn active() -> Self {
        Self {
            active: true,
            result_json: String::new(),
            compact_proof_json: String::new(),
            gpu_carrier_words: Vec::new(),
        }
    }
}

static LAB_RUNTIME: Mutex<Vec<LabInstanceState>> = Mutex::new(Vec::new());

fn ensure_default_instance(runtime: &mut Vec<LabInstanceState>) {
    if runtime.is_empty() {
        runtime.push(LabInstanceState::active());
    }
}

fn with_lab_instance<T>(
    instance_id: u32,
    f: impl FnOnce(&LabInstanceState) -> T,
) -> Option<T> {
    let mut runtime = LAB_RUNTIME
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    ensure_default_instance(&mut runtime);
    let state = runtime.get(instance_id as usize)?;
    if !state.active {
        return None;
    }
    Some(f(state))
}

fn with_lab_instance_mut<T>(
    instance_id: u32,
    f: impl FnOnce(&mut LabInstanceState) -> T,
) -> Option<T> {
    let mut runtime = LAB_RUNTIME
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    ensure_default_instance(&mut runtime);
    let state = runtime.get_mut(instance_id as usize)?;
    if !state.active {
        return None;
    }
    Some(f(state))
}

fn lab_instance_active(instance_id: u32) -> bool {
    with_lab_instance(instance_id, |_| ()).is_some()
}

fn set_compact_proof_for_instance(
    instance_id: u32,
    proof: &WebStructuralProof,
) -> Option<()> {
    let compact = proof.compact()?;
    let compact_json = serde_json::to_string(&compact).ok()?;

    #[cfg(test)]
    {
        let proof_json = serde_json::to_string(proof).ok()?;
        let mut proof_guard = LAST_PROOF_JSON
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *proof_guard = proof_json;
    }

    with_lab_instance_mut(instance_id, |state| {
        state.compact_proof_json = compact_json;
    })
}

fn set_last_compact_proof(proof: &WebStructuralProof) -> Option<()> {
    set_compact_proof_for_instance(DEFAULT_LAB_INSTANCE_ID, proof)
}

fn clear_compact_proof_for_instance(instance_id: u32) {
    #[cfg(test)]
    if instance_id == DEFAULT_LAB_INSTANCE_ID {
        let mut proof_guard = LAST_PROOF_JSON
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        proof_guard.clear();
    }

    let _ = with_lab_instance_mut(instance_id, |state| {
        state.compact_proof_json.clear();
        state.gpu_carrier_words.clear();
    });
}

fn clear_last_compact_proof() {
    clear_compact_proof_for_instance(DEFAULT_LAB_INSTANCE_ID);
}

fn clear_result_for_instance(instance_id: u32) {
    let _ = with_lab_instance_mut(instance_id, |state| {
        state.result_json.clear();
    });
}

fn set_result_for_instance(
    instance_id: u32,
    op: u32,
    a: u32,
    b: u32,
    input_flag: u32,
    outcome: LabOutcome,
) -> Option<()> {
    let compact_proof_available =
        with_lab_instance(instance_id, |state| !state.compact_proof_json.is_empty())?;
    let envelope = LabResultEnvelope {
        schema_version: LAB_RESULT_SCHEMA_VERSION,
        representation_id: LAB_RESULT_REPRESENTATION_ID,
        representation_version: LAB_RESULT_REPRESENTATION_VERSION,
        instance_id,
        witness_kind: "registry-block",
        operation: Some(LabOperation {
            opcode: op,
            a,
            b,
            input_flag,
        }),
        outcome: Some(outcome),
        payload: None,
        compact_proof_available,
    };
    let json = serde_json::to_string(&envelope).ok()?;
    with_lab_instance_mut(instance_id, |state| {
        state.result_json = json;
    })
}

fn set_witness_result_for_instance(
    instance_id: u32,
    witness_kind: &'static str,
    payload: serde_json::Value,
) -> Option<()> {
    let compact_proof_available =
        with_lab_instance(instance_id, |state| !state.compact_proof_json.is_empty())?;
    let envelope = LabResultEnvelope {
        schema_version: LAB_RESULT_SCHEMA_VERSION,
        representation_id: LAB_RESULT_REPRESENTATION_ID,
        representation_version: LAB_RESULT_REPRESENTATION_VERSION,
        instance_id,
        witness_kind,
        operation: None,
        outcome: None,
        payload: Some(payload),
        compact_proof_available,
    };
    let json = serde_json::to_string(&envelope).ok()?;
    with_lab_instance_mut(instance_id, |state| {
        state.result_json = json;
    })
}

fn set_default_witness_result(
    witness_kind: &'static str,
    payload: serde_json::Value,
) -> Option<()> {
    set_witness_result_for_instance(DEFAULT_LAB_INSTANCE_ID, witness_kind, payload)
}

fn execute_for_instance(
    instance_id: u32,
    op: u32,
    a: u32,
    b: u32,
    input_flag: u32,
) -> Option<LabOutcome> {
    clear_compact_proof_for_instance(instance_id);

    if (1..=5).contains(&op) {
        let execution = web_prove_logic(op, a, b)?;
        set_compact_proof_for_instance(instance_id, &execution.proof)?;
        let out = execution.outcome;
        return Some(LabOutcome {
            value: out.value,
            value_hi: 0,
            writeback: u32::from(out.writeback),
            defined_mask: out.defined_mask,
            value_mask: out.value_mask,
            undefined_mask: out.undefined_mask,
            preserve_mask: out.preserve_mask,
            reactions: out.reactions,
            links_after_build: out.links_after_build,
            links_after_first: out.links_after_first,
            steady_link_delta: out.steady_link_delta,
            quiescent: u32::from(out.quiescent),
        });
    }

    if let Some(out) = web_run_logic(op, a, b) {
        return Some(LabOutcome {
            value: out.value,
            value_hi: 0,
            writeback: u32::from(out.writeback),
            defined_mask: out.defined_mask,
            value_mask: out.value_mask,
            undefined_mask: out.undefined_mask,
            preserve_mask: out.preserve_mask,
            reactions: out.reactions,
            links_after_build: out.links_after_build,
            links_after_first: out.links_after_first,
            steady_link_delta: out.steady_link_delta,
            quiescent: u32::from(out.quiescent),
        });
    }

    if (6..=10).contains(&op) {
        let execution = web_prove_arithmetic(op, a, b, input_flag)?;
        set_compact_proof_for_instance(instance_id, &execution.proof)?;
        let out = execution.outcome;
        return Some(LabOutcome {
            value: out.value,
            value_hi: 0,
            writeback: u32::from(out.writeback),
            defined_mask: out.defined_mask,
            value_mask: out.value_mask,
            undefined_mask: out.undefined_mask,
            preserve_mask: out.preserve_mask,
            reactions: out.reactions,
            links_after_build: out.links_after_build,
            links_after_first: out.links_after_first,
            steady_link_delta: out.steady_link_delta,
            quiescent: u32::from(out.quiescent),
        });
    }

    if let Some(out) = web_run_arithmetic(op, a, b, input_flag) {
        return Some(LabOutcome {
            value: out.value,
            value_hi: 0,
            writeback: u32::from(out.writeback),
            defined_mask: out.defined_mask,
            value_mask: out.value_mask,
            undefined_mask: out.undefined_mask,
            preserve_mask: out.preserve_mask,
            reactions: out.reactions,
            links_after_build: out.links_after_build,
            links_after_first: out.links_after_first,
            steady_link_delta: out.steady_link_delta,
            quiescent: u32::from(out.quiescent),
        });
    }

    if op == 11 {
        let execution = web_prove_mux32(input_flag, a, b)?;
        set_compact_proof_for_instance(instance_id, &execution.proof)?;
        let out = execution.outcome;
        return Some(LabOutcome {
            value: out.value,
            value_hi: 0,
            writeback: 1,
            defined_mask: 0,
            value_mask: 0,
            undefined_mask: 0,
            preserve_mask: STATUS_FLAGS,
            reactions: out.reactions,
            links_after_build: out.links_after_build,
            links_after_first: out.links_after_first,
            steady_link_delta: out.steady_link_delta,
            quiescent: u32::from(out.quiescent),
        });
    }

    if op == 12 {
        let proof = web_prove_mux1(input_flag, a, b)?;
        set_compact_proof_for_instance(instance_id, &proof)?;
        let out = LabOutcome {
            value: u32::from(proof.result.decoded_value),
            value_hi: 0,
            writeback: 1,
            defined_mask: 0,
            value_mask: 0,
            undefined_mask: 0,
            preserve_mask: STATUS_FLAGS,
            reactions: proof.execute.active_reaction_count,
            links_after_build: proof.load.links_after_load,
            links_after_first: proof.result.links_final,
            steady_link_delta: proof.result.identical_rerun_link_delta,
            quiescent: u32::from(proof.execute.final_quiescent),
        };
        return Some(out);
    }


    if (13..=15).contains(&op) {
        let execution = web_prove_shift32(op, a, b)?;
        set_compact_proof_for_instance(instance_id, &execution.proof)?;
        let out = execution.outcome;
        return Some(LabOutcome {
            value: out.value,
            value_hi: 0,
            writeback: u32::from(out.writeback),
            defined_mask: out.defined_mask,
            value_mask: out.value_mask,
            undefined_mask: out.undefined_mask,
            preserve_mask: out.preserve_mask,
            reactions: out.reactions,
            links_after_build: out.links_after_build,
            links_after_first: out.links_after_first,
            steady_link_delta: out.steady_link_delta,
            quiescent: u32::from(out.quiescent),
        });
    }

    if let Some(out) = web_run_shift32(op, a, b) {
        return Some(LabOutcome {
            value: out.value,
            value_hi: 0,
            writeback: u32::from(out.writeback),
            defined_mask: out.defined_mask,
            value_mask: out.value_mask,
            undefined_mask: out.undefined_mask,
            preserve_mask: out.preserve_mask,
            reactions: out.reactions,
            links_after_build: out.links_after_build,
            links_after_first: out.links_after_first,
            steady_link_delta: out.steady_link_delta,
            quiescent: u32::from(out.quiescent),
        });
    }

    if (16..=17).contains(&op) {
        let execution = web_prove_rotate32(op, a, b)?;
        set_compact_proof_for_instance(instance_id, &execution.proof)?;
        let out = execution.outcome;
        return Some(LabOutcome {
            value: out.value,
            value_hi: 0,
            writeback: u32::from(out.writeback),
            defined_mask: out.defined_mask,
            value_mask: out.value_mask,
            undefined_mask: out.undefined_mask,
            preserve_mask: out.preserve_mask,
            reactions: out.reactions,
            links_after_build: out.links_after_build,
            links_after_first: out.links_after_first,
            steady_link_delta: out.steady_link_delta,
            quiescent: u32::from(out.quiescent),
        });
    }

    if let Some(out) = web_run_rotate32(op, a, b) {
        return Some(LabOutcome {
            value: out.value,
            value_hi: 0,
            writeback: u32::from(out.writeback),
            defined_mask: out.defined_mask,
            value_mask: out.value_mask,
            undefined_mask: out.undefined_mask,
            preserve_mask: out.preserve_mask,
            reactions: out.reactions,
            links_after_build: out.links_after_build,
            links_after_first: out.links_after_first,
            steady_link_delta: out.steady_link_delta,
            quiescent: u32::from(out.quiescent),
        });
    }

    if (18..=19).contains(&op) {
        let execution = web_prove_rotate_carry32(op, a, b, input_flag)?;
        set_compact_proof_for_instance(instance_id, &execution.proof)?;
        let out = execution.outcome;
        return Some(LabOutcome {
            value: out.value,
            value_hi: 0,
            writeback: u32::from(out.writeback),
            defined_mask: out.defined_mask,
            value_mask: out.value_mask,
            undefined_mask: out.undefined_mask,
            preserve_mask: out.preserve_mask,
            reactions: out.reactions,
            links_after_build: out.links_after_build,
            links_after_first: out.links_after_first,
            steady_link_delta: out.steady_link_delta,
            quiescent: u32::from(out.quiescent),
        });
    }

    if let Some(out) = web_run_rotate_carry32(op, a, b, input_flag) {
        return Some(LabOutcome {
            value: out.value,
            value_hi: 0,
            writeback: u32::from(out.writeback),
            defined_mask: out.defined_mask,
            value_mask: out.value_mask,
            undefined_mask: out.undefined_mask,
            preserve_mask: out.preserve_mask,
            reactions: out.reactions,
            links_after_build: out.links_after_build,
            links_after_first: out.links_after_first,
            steady_link_delta: out.steady_link_delta,
            quiescent: u32::from(out.quiescent),
        });
    }

    if (20..=22).contains(&op) {
        let execution = web_prove_unary32(op, a)?;
        set_compact_proof_for_instance(instance_id, &execution.proof)?;
        let out = execution.outcome;
        return Some(LabOutcome {
            value: out.value,
            value_hi: 0,
            writeback: u32::from(out.writeback),
            defined_mask: out.defined_mask,
            value_mask: out.value_mask,
            undefined_mask: out.undefined_mask,
            preserve_mask: out.preserve_mask,
            reactions: out.reactions,
            links_after_build: out.links_after_build,
            links_after_first: out.links_after_first,
            steady_link_delta: out.steady_link_delta,
            quiescent: u32::from(out.quiescent),
        });
    }

    if let Some(out) = web_run_unary32(op, a) {
        return Some(LabOutcome {
            value: out.value,
            value_hi: 0,
            writeback: u32::from(out.writeback),
            defined_mask: out.defined_mask,
            value_mask: out.value_mask,
            undefined_mask: out.undefined_mask,
            preserve_mask: out.preserve_mask,
            reactions: out.reactions,
            links_after_build: out.links_after_build,
            links_after_first: out.links_after_first,
            steady_link_delta: out.steady_link_delta,
            quiescent: u32::from(out.quiescent),
        });
    }

    if op == 23 {
        let execution = web_prove_mul32(a, b)?;
        set_compact_proof_for_instance(instance_id, &execution.proof)?;
        let out = execution.outcome;
        return Some(LabOutcome {
            value: out.lo,
            value_hi: out.hi,
            writeback: 1,
            defined_mask: 0,
            value_mask: 0,
            undefined_mask: 0,
            preserve_mask: STATUS_FLAGS,
            reactions: out.reactions,
            links_after_build: out.links_after_build,
            links_after_first: out.links_after_first,
            steady_link_delta: out.steady_link_delta,
            quiescent: u32::from(out.quiescent),
        });
    }

    if op == 24 {
        let execution = web_prove_mul_effect(a, b)?;
        set_compact_proof_for_instance(instance_id, &execution.proof)?;
        let out = execution.outcome;
        return Some(LabOutcome {
            value: out.lo,
            value_hi: out.hi,
            writeback: 1,
            defined_mask: out.defined_mask,
            value_mask: out.value_mask,
            undefined_mask: out.undefined_mask,
            preserve_mask: out.preserve_mask,
            reactions: out.reactions,
            links_after_build: out.links_after_build,
            links_after_first: out.links_after_first,
            steady_link_delta: out.steady_link_delta,
            quiescent: u32::from(out.quiescent),
        });
    }

    None
}

fn execute(op: u32, a: u32, b: u32, input_flag: u32) -> Option<LabOutcome> {
    execute_for_instance(DEFAULT_LAB_INSTANCE_ID, op, a, b, input_flag)
}

#[no_mangle]
pub extern "C" fn amemory_i386_state_probe() -> u32 {
    0x0000_050a
}

fn state_witness_payload(
    block: &'static str,
    out: &crate::architectural_state_n::WebArchitecturalStateOutcome,
) -> serde_json::Value {
    serde_json::json!({
        "block": block,
        "before": {
            "eax": out.eax_before,
            "ebx": out.ebx_before,
            "ecx": out.ecx_before,
            "edx": out.edx_before,
            "esi": out.esi_before,
            "edi": out.edi_before,
            "ebp": out.ebp_before,
            "esp": out.esp_before,
            "eip": out.eip_before
        },
        "after": {
            "eax": out.eax_after,
            "ebx": out.ebx_after,
            "ecx": out.ecx_after,
            "edx": out.edx_after,
            "esi": out.esi_after,
            "edi": out.edi_after,
            "ebp": out.ebp_after,
            "esp": out.esp_after,
            "eip": out.eip_after
        },
        "flags": {
            "defined": out.flags_defined_mask,
            "flagValues": out.flags_value_mask,
            "undefined": out.flags_undefined_mask,
            "preserve": 0
        },
        "reactions": out.reactions,
        "oldStateRetained": u32::from(out.old_state_retained),
        "atomicScope": u32::from(out.atomic_scope),
        "steadyDelta": out.steady_link_delta,
        "quiescent": u32::from(out.quiescent)
    })
}

#[no_mangle]
pub extern "C" fn amemory_i386_state_run_add32() -> u32 {
    clear_last_compact_proof();
    clear_result_for_instance(DEFAULT_LAB_INSTANCE_ID);
    let Some(execution) = web_prove_architectural_state_add() else {
        return 0;
    };
    if set_last_compact_proof(&execution.proof).is_none() {
        return 0;
    }
    let out = execution.outcome;
    if set_default_witness_result(
        "architectural-state",
        state_witness_payload("M5A_STATE_ADD32", &out),
    ).is_none() {
        clear_last_compact_proof();
        return 0;
    }
    1
}

#[no_mangle]
pub extern "C" fn amemory_i386_state_wide_probe() -> u32 {
    0x0000_050b
}

#[no_mangle]
pub extern "C" fn amemory_i386_state_run_mul32() -> u32 {
    clear_last_compact_proof();
    clear_result_for_instance(DEFAULT_LAB_INSTANCE_ID);
    let Some(execution) = web_prove_architectural_state_mul() else {
        return 0;
    };
    if set_last_compact_proof(&execution.proof).is_none() {
        return 0;
    }
    let out = execution.outcome;
    if set_default_witness_result(
        "architectural-state",
        state_witness_payload("M5B_STATE_MUL32", &out),
    ).is_none() {
        clear_last_compact_proof();
        return 0;
    }
    1
}

#[no_mangle]
pub extern "C" fn amemory_i386_state_full_probe() -> u32 {
    0x0000_050c
}

#[no_mangle]
pub extern "C" fn amemory_i386_state_run_add_ecx() -> u32 {
    clear_last_compact_proof();
    clear_result_for_instance(DEFAULT_LAB_INSTANCE_ID);
    let Some(execution) = web_prove_architectural_state_add_ecx() else {
        return 0;
    };
    if set_last_compact_proof(&execution.proof).is_none() {
        return 0;
    }
    let out = execution.outcome;
    if set_default_witness_result(
        "architectural-state",
        state_witness_payload("M5C_STATE_ADD_ECX", &out),
    ).is_none() {
        clear_last_compact_proof();
        return 0;
    }
    1
}

#[no_mangle]
pub extern "C" fn amemory_i386_memory_probe() -> u32 {
    0x0000_060a
}

#[no_mangle]
pub extern "C" fn amemory_i386_memory_run(
    offset: u32,
    value: u32,
) -> u32 {
    if offset > 0xff || value > 0xff {
        return 0;
    }
    clear_last_compact_proof();
    clear_result_for_instance(DEFAULT_LAB_INSTANCE_ID);
    let Some(execution) =
        web_prove_radix_memory(offset as u8, value as u8)
    else {
        return 0;
    };
    if set_last_compact_proof(&execution.proof).is_none() {
        return 0;
    }
    let out = execution.outcome;
    if set_default_witness_result(
        "memory-radix",
        serde_json::json!({
            "block": "M6A_RADIX_PAGE",
            "width": 8,
            "address": out.offset,
            "offset": out.offset,
            "write": out.write_value,
            "before": out.before_value,
            "after": out.after_value,
            "oldAfter": out.old_after_value,
            "oldRoot": out.old_root_ref,
            "newRoot": out.new_root_ref,
            "reactions": out.reactions,
            "linksAfterLoad": out.links_after_load,
            "linksFinal": out.links_final,
            "steadyDelta": out.steady_link_delta,
            "atomicScope": 1,
            "crossesPage": 0,
            "quiescent": u32::from(out.quiescent)
        }),
    ).is_none() {
        clear_last_compact_proof();
        return 0;
    }
    1
}

#[no_mangle]
pub extern "C" fn amemory_i386_memory32_probe() -> u32 {
    0x0000_060b
}

#[no_mangle]
pub extern "C" fn amemory_i386_memory32_run(
    address: u32,
    value: u32,
) -> u32 {
    if value > 0xff {
        return 0;
    }
    clear_last_compact_proof();
    clear_result_for_instance(DEFAULT_LAB_INSTANCE_ID);
    let Some(execution) =
        web_prove_memory32(address, value as u8)
    else {
        return 0;
    };
    if set_last_compact_proof(&execution.proof).is_none() {
        return 0;
    }
    let out = execution.outcome;
    if set_default_witness_result(
        "memory-byte",
        serde_json::json!({
            "block": "M6B_MEMORY32",
            "width": 8,
            "address": out.address,
            "page24": out.page24,
            "offset8": out.offset8,
            "write": out.write_value,
            "before": out.before_value,
            "after": out.after_value,
            "oldAfter": out.old_after_value,
            "oldRoot": out.old_root_ref,
            "newRoot": out.new_root_ref,
            "reactions": out.reactions,
            "linksAfterLoad": out.links_after_load,
            "linksFinal": out.links_final,
            "steadyDelta": out.steady_link_delta,
            "atomicScope": 1,
            "crossesPage": 0,
            "quiescent": u32::from(out.quiescent)
        }),
    ).is_none() {
        clear_last_compact_proof();
        return 0;
    }
    1
}

#[no_mangle]
pub extern "C" fn amemory_i386_word_memory_probe() -> u32 {
    0x0000_060c
}

#[no_mangle]
pub extern "C" fn amemory_i386_word_memory_run(
    width: u32,
    address: u32,
    value: u32,
) -> u32 {
    if width == 16 && value > 0xffff {
        return 0;
    }
    clear_last_compact_proof();
    clear_result_for_instance(DEFAULT_LAB_INSTANCE_ID);
    let execution = match width {
        16 => web_prove_word16_memory(address, value),
        32 => web_prove_word32_memory(address, value),
        _ => None,
    };
    let Some(execution) = execution else {
        return 0;
    };
    if set_last_compact_proof(&execution.proof).is_none() {
        return 0;
    }
    let out = execution.outcome;
    let block = if out.width == 16 {
        "M6C_MEMORY_WORD16"
    } else {
        "M6C_MEMORY_WORD32"
    };
    if set_default_witness_result(
        "memory-word",
        serde_json::json!({
            "block": block,
            "width": out.width,
            "address": out.address,
            "write": out.write_value,
            "before": out.before_value,
            "after": out.after_value,
            "oldAfter": out.old_after_value,
            "oldRoot": out.old_root_ref,
            "newRoot": out.new_root_ref,
            "reactions": out.reactions,
            "linksAfterLoad": out.links_after_load,
            "linksFinal": out.links_final,
            "steadyDelta": out.steady_link_delta,
            "atomicScope": u32::from(out.atomic_scope),
            "crossesPage": u32::from(out.crosses_page),
            "quiescent": u32::from(out.quiescent)
        }),
    ).is_none() {
        clear_last_compact_proof();
        return 0;
    }
    1
}

#[no_mangle]
pub extern "C" fn amemory_i386_fetch_probe() -> u32 {
    0x0000_060d
}

#[no_mangle]
pub extern "C" fn amemory_i386_fetch_run(
    eip: u32,
    instruction_byte: u32,
    seed_write: u32,
) -> u32 {
    if instruction_byte > 0xff || seed_write > 1 {
        return 0;
    }
    clear_last_compact_proof();
    clear_result_for_instance(DEFAULT_LAB_INSTANCE_ID);
    let Some(execution) = web_prove_instruction_fetch(
        eip,
        instruction_byte as u8,
        seed_write == 1,
    ) else {
        return 0;
    };
    if set_last_compact_proof(&execution.proof).is_none() {
        return 0;
    }
    let out = execution.outcome;
    let block = if out.seeded_write != 0 {
        "M6D2_FETCH_SEEDED"
    } else {
        "M6D2_FETCH_ZERO"
    };
    if set_default_witness_result(
        "instruction-fetch",
        serde_json::json!({
            "block": block,
            "eipBefore": out.eip_before,
            "eipAfter": out.eip_after,
            "byte": out.fetched_byte,
            "initialRoot": out.initial_memory_root_ref,
            "finalRoot": out.final_memory_root_ref,
            "seededWrite": u32::from(out.seeded_write),
            "statePreserved": u32::from(out.state_preserved),
            "oldStateRetained": u32::from(out.old_state_retained),
            "atomicScope": u32::from(out.atomic_scope),
            "reactions": out.reactions,
            "linksAfterLoad": out.links_after_load,
            "linksFinal": out.links_final,
            "steadyDelta": out.steady_link_delta,
            "quiescent": u32::from(out.quiescent)
        }),
    ).is_none() {
        clear_last_compact_proof();
        return 0;
    }
    1
}

#[no_mangle]
pub extern "C" fn amemory_i386_stack_probe() -> u32 {
    0x0000_060e
}

#[no_mangle]
pub extern "C" fn amemory_i386_stack_run(
    esp: u32,
    value: u32,
) -> u32 {
    clear_last_compact_proof();
    clear_result_for_instance(DEFAULT_LAB_INSTANCE_ID);
    let Some(execution) = web_prove_stack_roundtrip(esp, value) else {
        return 0;
    };
    if set_last_compact_proof(&execution.proof).is_none() {
        return 0;
    }
    let out = execution.outcome;
    if set_default_witness_result(
        "stack-roundtrip",
        serde_json::json!({
            "block": "M6D3_STACK_ROUNDTRIP",
            "espBefore": out.esp_before,
            "espAfter": out.esp_after,
            "value": out.value,
            "initialRoot": out.initial_memory_root_ref,
            "finalRoot": out.final_memory_root_ref,
            "statePreserved": u32::from(out.state_preserved),
            "oldStateRetained": u32::from(out.old_state_retained),
            "oldMemoryRetained": u32::from(out.old_memory_retained),
            "atomicScope": u32::from(out.atomic_scope),
            "reactions": out.reactions,
            "linksAfterLoad": out.links_after_load,
            "linksFinal": out.links_final,
            "steadyDelta": out.steady_link_delta,
            "quiescent": u32::from(out.quiescent)
        }),
    ).is_none() {
        clear_last_compact_proof();
        return 0;
    }
    1
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_instance_gpu_carrier_prepare(
    instance_id: u32,
) -> u32 {
    if !lab_instance_active(instance_id) {
        return 0;
    }
    clear_result_for_instance(instance_id);
    clear_compact_proof_for_instance(instance_id);

    let Some(proof) = web_prove_mux1(1, 0, 1) else {
        return 0;
    };
    let Some(words) = packed_gpu_carrier_words(&proof) else {
        return 0;
    };
    let Ok(word_len) = u32::try_from(words.len()) else {
        return 0;
    };
    let link_count = proof.prepare.compiled_links;
    let block = proof.block.clone();

    if set_compact_proof_for_instance(instance_id, &proof).is_none() {
        return 0;
    }
    if with_lab_instance_mut(instance_id, |state| {
        state.gpu_carrier_words = words;
    })
    .is_none()
    {
        clear_compact_proof_for_instance(instance_id);
        return 0;
    }

    if set_witness_result_for_instance(
        instance_id,
        "gpu-carrier",
        serde_json::json!({
            "block": block,
            "linkCount": link_count,
            "wordLength": word_len,
            "rootHandle": 1
        }),
    )
    .is_none()
    {
        clear_compact_proof_for_instance(instance_id);
        return 0;
    }

    1
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_instance_gpu_carrier_available(
    instance_id: u32,
) -> u32 {
    with_lab_instance(instance_id, |state| {
        u32::from(!state.gpu_carrier_words.is_empty())
    })
    .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_instance_gpu_carrier_word_len(
    instance_id: u32,
) -> u32 {
    with_lab_instance(instance_id, |state| {
        u32::try_from(state.gpu_carrier_words.len()).unwrap_or(0)
    })
    .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_instance_gpu_carrier_words_ptr(
    instance_id: u32,
) -> u32 {
    with_lab_instance(instance_id, |state| {
        state.gpu_carrier_words.as_ptr() as usize as u32
    })
    .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_instance_gpu_carrier_word(
    instance_id: u32,
    index: u32,
) -> u32 {
    with_lab_instance(instance_id, |state| {
        state
            .gpu_carrier_words
            .get(index as usize)
            .copied()
            .unwrap_or(u32::MAX)
    })
    .unwrap_or(u32::MAX)
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_gpu_carrier_prepare() -> u32 {
    amemory_i386_lab_instance_gpu_carrier_prepare(DEFAULT_LAB_INSTANCE_ID)
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_gpu_carrier_available() -> u32 {
    amemory_i386_lab_instance_gpu_carrier_available(DEFAULT_LAB_INSTANCE_ID)
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_gpu_carrier_word_len() -> u32 {
    amemory_i386_lab_instance_gpu_carrier_word_len(DEFAULT_LAB_INSTANCE_ID)
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_gpu_carrier_words_ptr() -> u32 {
    amemory_i386_lab_instance_gpu_carrier_words_ptr(DEFAULT_LAB_INSTANCE_ID)
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_gpu_carrier_word(index: u32) -> u32 {
    amemory_i386_lab_instance_gpu_carrier_word(
        DEFAULT_LAB_INSTANCE_ID,
        index,
    )
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_probe() -> u32 {
    0x0000_0386
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_supports(op: u32) -> u32 {
    u32::from((1..=24).contains(&op))
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_instance_capacity() -> u32 {
    MAX_LAB_INSTANCES as u32
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_instance_create() -> u32 {
    let mut runtime = LAB_RUNTIME
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    ensure_default_instance(&mut runtime);

    for index in 1..runtime.len() {
        if !runtime[index].active {
            runtime[index] = LabInstanceState::active();
            return index as u32;
        }
    }
    if runtime.len() >= MAX_LAB_INSTANCES {
        return u32::MAX;
    }
    runtime.push(LabInstanceState::active());
    (runtime.len() - 1) as u32
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_instance_destroy(instance_id: u32) -> u32 {
    if instance_id == DEFAULT_LAB_INSTANCE_ID {
        return 0;
    }
    let mut runtime = LAB_RUNTIME
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    ensure_default_instance(&mut runtime);
    let Some(state) = runtime.get_mut(instance_id as usize) else {
        return 0;
    };
    if !state.active {
        return 0;
    }
    *state = LabInstanceState::default();
    1
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_instance_run(
    instance_id: u32,
    op: u32,
    a: u32,
    b: u32,
    input_flag: u32,
) -> u32 {
    if !lab_instance_active(instance_id) {
        return 0;
    }
    clear_result_for_instance(instance_id);
    let Some(out) = execute_for_instance(instance_id, op, a, b, input_flag) else {
        return 0;
    };
    if set_result_for_instance(instance_id, op, a, b, input_flag, out).is_none() {
        clear_compact_proof_for_instance(instance_id);
        return 0;
    }

    1
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_run(
    op: u32,
    a: u32,
    b: u32,
    input_flag: u32,
) -> u32 {
    amemory_i386_lab_instance_run(DEFAULT_LAB_INSTANCE_ID, op, a, b, input_flag)
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_instance_result_available(instance_id: u32) -> u32 {
    with_lab_instance(instance_id, |state| u32::from(!state.result_json.is_empty()))
        .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_instance_result_json_len(instance_id: u32) -> u32 {
    with_lab_instance(instance_id, |state| state.result_json.len() as u32)
        .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_instance_result_json_ptr(instance_id: u32) -> u32 {
    with_lab_instance(instance_id, |state| state.result_json.as_ptr() as usize as u32)
        .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_instance_result_json_byte(
    instance_id: u32,
    index: u32,
) -> u32 {
    with_lab_instance(instance_id, |state| {
        state
            .result_json
            .as_bytes()
            .get(index as usize)
            .copied()
            .map(u32::from)
            .unwrap_or(u32::MAX)
    })
    .unwrap_or(u32::MAX)
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_result_available() -> u32 {
    amemory_i386_lab_instance_result_available(DEFAULT_LAB_INSTANCE_ID)
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_result_json_len() -> u32 {
    amemory_i386_lab_instance_result_json_len(DEFAULT_LAB_INSTANCE_ID)
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_result_json_ptr() -> u32 {
    amemory_i386_lab_instance_result_json_ptr(DEFAULT_LAB_INSTANCE_ID)
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_result_json_byte(index: u32) -> u32 {
    amemory_i386_lab_instance_result_json_byte(DEFAULT_LAB_INSTANCE_ID, index)
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_instance_compact_proof_available(
    instance_id: u32,
) -> u32 {
    with_lab_instance(instance_id, |state| {
        u32::from(!state.compact_proof_json.is_empty())
    })
    .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_instance_compact_proof_json_len(
    instance_id: u32,
) -> u32 {
    with_lab_instance(instance_id, |state| state.compact_proof_json.len() as u32)
        .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_instance_compact_proof_json_ptr(
    instance_id: u32,
) -> u32 {
    with_lab_instance(instance_id, |state| {
        state.compact_proof_json.as_ptr() as usize as u32
    })
    .unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_instance_compact_proof_json_byte(
    instance_id: u32,
    index: u32,
) -> u32 {
    with_lab_instance(instance_id, |state| {
        state
            .compact_proof_json
            .as_bytes()
            .get(index as usize)
            .copied()
            .map(u32::from)
            .unwrap_or(u32::MAX)
    })
    .unwrap_or(u32::MAX)
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_compact_proof_available() -> u32 {
    amemory_i386_lab_instance_compact_proof_available(DEFAULT_LAB_INSTANCE_ID)
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_compact_proof_json_len() -> u32 {
    amemory_i386_lab_instance_compact_proof_json_len(DEFAULT_LAB_INSTANCE_ID)
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_compact_proof_json_ptr() -> u32 {
    amemory_i386_lab_instance_compact_proof_json_ptr(DEFAULT_LAB_INSTANCE_ID)
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_compact_proof_json_byte(index: u32) -> u32 {
    amemory_i386_lab_instance_compact_proof_json_byte(DEFAULT_LAB_INSTANCE_ID, index)
}

#[cfg(test)]
fn test_last_proof_available() -> u32 {
    let guard = LAST_PROOF_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    u32::from(!guard.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "heavy web-lab structural integration; mandatory dedicated release workflow"]
    fn web_lab_executes_real_logic_arithmetic_and_mux_blocks() {
        let and = execute(1, 0xf0f0_1234, 0x0ff0_ffff, 0).unwrap();
        assert_eq!(and.value, 0x00f0_1234);
        assert_eq!(and.writeback, 1);
        assert_eq!(and.undefined_mask & FLAG_AF, FLAG_AF);
        assert_eq!(and.steady_link_delta, 0);
        assert_eq!(and.quiescent, 1);

        for (op, a, b, expected, expected_writeback, block) in [
            (1u32, 0xaaaa_aaaau32, 0x0f0f_0f0fu32, 0x0a0a_0a0au32, 1u32, "AND32"),
            (2u32, 0xaaaa_0000u32, 0x0000_5555u32, 0xaaaa_5555u32, 1u32, "OR32"),
            (3u32, 0xffff_0000u32, 0x0f0f_0f0fu32, 0xf0f0_0f0fu32, 1u32, "XOR32"),
            (4u32, 0x1234_5678u32, 0u32, !0x1234_5678u32, 1u32, "NOT32"),
            (5u32, 0xf0f0_1234u32, 0x0ff0_ffffu32, 0x00f0_1234u32, 0u32, "TEST32"),
        ] {
            let out = execute(op, a, b, 0).unwrap();
            assert_eq!(out.value, expected, "{block}");
            assert_eq!(out.writeback, expected_writeback, "{block}");
            assert_eq!(out.steady_link_delta, 0, "{block}");
            assert_eq!(test_last_proof_available(), 1, "{block}");

            let proof_json = {
                let guard = LAST_PROOF_JSON
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                guard.clone()
            };
            let proof: serde_json::Value =
                serde_json::from_str(&proof_json).unwrap();
            assert_eq!(proof["block"], block);
            let memory_id =
                proof["load"]["memoryInstanceId"].as_str().unwrap();
            assert_eq!(
                proof["execute"]["memoryInstanceId"].as_str().unwrap(),
                memory_id
            );
            assert_eq!(
                proof["result"]["memoryInstanceId"].as_str().unwrap(),
                memory_id
            );
            assert_eq!(proof["prepare"]["runtimeMemoryExists"], false);
            assert_eq!(proof["load"]["linksBeforeLoad"], 1);
            assert_eq!(proof["load"]["carrierRoundTrip"], true);
            assert_eq!(proof["execute"]["finalQuiescent"], true);
            assert_eq!(proof["result"]["oracleMatches"], true);
            assert_eq!(
                proof["result"]["decodedValue"].as_u64().unwrap() as u32,
                out.value
            );
            assert_eq!(proof["result"]["identicalRerunLinkDelta"], 0);
            assert_eq!(
                proof["result"]["visualLinks"].as_array().unwrap().len() as u64,
                proof["result"]["linksFinal"].as_u64().unwrap()
            );

            let roles = proof["prepare"]["semanticRoots"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|root| root["role"].as_str())
                .collect::<std::collections::HashSet<_>>();
            for required in [
                "data.a.word",
                "data.bit.zero",
                "data.bit.one",
                "execution.interpreter",
                "execution.theory",
                "execution.apply",
                "invocation.call",
                "scope.initial",
                "result.tag",
            ] {
                assert!(roles.contains(required), "{block}: missing {required}");
            }
            if op == 4 {
                assert!(roles.contains("function.word_not"));
                assert!(roles.contains("function.gate.not1"));
            } else {
                assert!(roles.contains("function.word_binary"));
                assert!(roles.iter().any(|role| role.starts_with("function.gate.")));
                assert!(roles.contains("data.b.word"));
                assert!(roles.contains("data.writeback"));

                let roots = proof["prepare"]["semanticRoots"].as_array().unwrap();
                let source_for = |role: &str| {
                    roots
                        .iter()
                        .find(|root| root["role"] == role)
                        .and_then(|root| root["source"].as_str())
                        .unwrap()
                };
                let expected_bit_role =
                    if expected_writeback == 0 { "data.bit.zero" } else { "data.bit.one" };
                assert_eq!(
                    source_for("data.writeback"),
                    source_for(expected_bit_role),
                    "{block}: structural writeback bit mismatch"
                );
            }
        }

        for (op, a, b, input_flag, expected, expected_writeback, block, expected_x, expected_mode) in [
            (6u32, u32::MAX, 1u32, 0u32, 0u32, 1u32, "ADD32", 0u32, 0u32),
            (7u32, u32::MAX, 0u32, 1u32, 0u32, 1u32, "ADC32", 1u32, 0u32),
            (8u32, 0u32, 1u32, 0u32, u32::MAX, 1u32, "SUB32", 0u32, 1u32),
            (9u32, 0u32, 0u32, 1u32, u32::MAX, 1u32, "SBB32", 1u32, 1u32),
            (10u32, 7u32, 9u32, 0u32, 0xffff_fffeu32, 0u32, "CMP32", 0u32, 1u32),
        ] {
            let out = execute(op, a, b, input_flag).unwrap();
            assert_eq!(out.value, expected, "{block}");
            assert_eq!(out.writeback, expected_writeback, "{block}");
            assert_eq!(out.steady_link_delta, 0, "{block}");
            assert_eq!(out.quiescent, 1, "{block}");
            assert_eq!(test_last_proof_available(), 1, "{block}");

            let proof_json = {
                let guard = LAST_PROOF_JSON
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                guard.clone()
            };
            let proof: serde_json::Value =
                serde_json::from_str(&proof_json).unwrap();
            assert_eq!(proof["block"], block);
            let memory_id =
                proof["load"]["memoryInstanceId"].as_str().unwrap();
            assert_eq!(
                proof["execute"]["memoryInstanceId"].as_str().unwrap(),
                memory_id
            );
            assert_eq!(
                proof["result"]["memoryInstanceId"].as_str().unwrap(),
                memory_id
            );
            assert_eq!(proof["load"]["carrierRoundTrip"], true);
            assert_eq!(proof["execute"]["finalQuiescent"], true);
            assert_eq!(proof["result"]["oracleMatches"], true);
            assert_eq!(
                proof["result"]["decodedValue"].as_u64().unwrap() as u32,
                out.value
            );
            assert_eq!(proof["result"]["identicalRerunLinkDelta"], 0);
            assert_eq!(
                proof["result"]["visualLinks"].as_array().unwrap().len() as u64,
                proof["result"]["linksFinal"].as_u64().unwrap()
            );

            let roots = proof["prepare"]["semanticRoots"].as_array().unwrap();
            let source_for = |role: &str| {
                roots
                    .iter()
                    .find(|root| root["role"] == role)
                    .and_then(|root| root["source"].as_str())
                    .unwrap()
            };
            for required in [
                "function.effect.arithmetic",
                "function.flagged_arithmetic",
                "data.a.word",
                "data.b.word",
                "data.x",
                "data.mode",
                "data.writeback",
                "execution.interpreter",
                "execution.theory",
                "execution.apply",
                "invocation.call",
                "scope.initial",
                "result.tag",
            ] {
                assert!(
                    roots.iter().any(|root| root["role"] == required),
                    "{block}: missing {required}"
                );
            }

            let bit_role = |value: u32| {
                if value == 0 { "data.bit.zero" } else { "data.bit.one" }
            };
            assert_eq!(
                source_for("data.x"),
                source_for(bit_role(expected_x)),
                "{block}: X selector mismatch"
            );
            assert_eq!(
                source_for("data.mode"),
                source_for(bit_role(expected_mode)),
                "{block}: Mode selector mismatch"
            );
            assert_eq!(
                source_for("data.writeback"),
                source_for(bit_role(expected_writeback)),
                "{block}: WriteBack selector mismatch"
            );
        }

        let mux = execute(11, 0xaaaa_aaaa, 0x5555_5555, 1).unwrap();
        assert_eq!(mux.value, 0x5555_5555);
        assert_eq!(mux.preserve_mask, STATUS_FLAGS);
        assert_eq!(mux.reactions, 257);
        assert_eq!(mux.steady_link_delta, 0);
        assert_eq!(test_last_proof_available(), 1);

        let mux32_proof_json = {
            let guard = LAST_PROOF_JSON
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.clone()
        };
        let mux32_proof: serde_json::Value =
            serde_json::from_str(&mux32_proof_json).unwrap();
        assert_eq!(mux32_proof["block"], "MUX32");
        let mux32_memory_id =
            mux32_proof["load"]["memoryInstanceId"].as_str().unwrap();
        assert_eq!(
            mux32_proof["execute"]["memoryInstanceId"].as_str().unwrap(),
            mux32_memory_id
        );
        assert_eq!(
            mux32_proof["result"]["memoryInstanceId"].as_str().unwrap(),
            mux32_memory_id
        );
        assert_eq!(mux32_proof["load"]["carrierRoundTrip"], true);
        assert_eq!(mux32_proof["execute"]["activeReactionCount"], 257);
        assert_eq!(mux32_proof["execute"]["finalQuiescent"], true);
        assert_eq!(mux32_proof["result"]["oracleMatches"], true);
        assert_eq!(mux32_proof["result"]["identicalRerunLinkDelta"], 0);
        assert_eq!(
            mux32_proof["result"]["visualLinks"].as_array().unwrap().len() as u64,
            mux32_proof["result"]["linksFinal"].as_u64().unwrap()
        );

        let mux32_roots = mux32_proof["prepare"]["semanticRoots"]
            .as_array()
            .unwrap();
        let mux32_source_for = |role: &str| {
            mux32_roots
                .iter()
                .find(|root| root["role"] == role)
                .and_then(|root| root["source"].as_str())
                .unwrap()
        };
        for required in [
            "function.mux32",
            "function.mux1",
            "function.gate.xor2",
            "function.gate.and2",
            "data.select",
            "data.a.word",
            "data.b.word",
            "execution.interpreter",
            "execution.theory",
            "execution.apply",
            "invocation.call",
            "scope.initial",
            "result.word_tag",
        ] {
            assert!(
                mux32_roots.iter().any(|root| root["role"] == required),
                "MUX32 missing {required}"
            );
        }
        assert_eq!(
            mux32_source_for("data.select"),
            mux32_source_for("data.bit.one")
        );

        for select in 0u32..=1 {
            for a in 0u32..=1 {
                for b in 0u32..=1 {
                    let mux1 = execute(12, a, b, select).unwrap();
                    assert_eq!(mux1.value, if select == 0 { a } else { b });
                    assert_eq!(mux1.reactions, 7);
                    assert_eq!(mux1.steady_link_delta, 0);
                }
            }
        }

        let proof_mux1 = execute(12, 1, 0, 1).unwrap();
        assert_eq!(proof_mux1.value, 0);
        assert_eq!(test_last_proof_available(), 1);
        let proof_json = {
            let guard = LAST_PROOF_JSON
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            guard.clone()
        };
        let proof: serde_json::Value = serde_json::from_str(&proof_json).unwrap();
        let memory_id = proof["load"]["memoryInstanceId"].as_str().unwrap();
        assert_eq!(proof["execute"]["memoryInstanceId"].as_str().unwrap(), memory_id);
        assert_eq!(proof["result"]["memoryInstanceId"].as_str().unwrap(), memory_id);
        assert_eq!(proof["prepare"]["runtimeMemoryExists"], false);
        assert_eq!(proof["result"]["oracleMatches"], true);
        assert_eq!(proof["result"]["identicalRerunLinkDelta"], 0);

        let mut shift_count_sources = Vec::new();
        for (op, value, count, expected, block) in [
            (13u32, 0x8000_0001u32, 1u32, 0x0000_0002u32, "SHL32"),
            (14u32, 0x8000_0001u32, 1u32, 0x4000_0000u32, "SHR32"),
            (15u32, 0x8000_0001u32, 1u32, 0xc000_0000u32, "SAR32"),
            (13u32, 0x8000_0001u32, 32u32, 0x8000_0001u32, "SHL32"),
            (13u32, 0x8000_0001u32, 33u32, 0x0000_0002u32, "SHL32"),
        ] {
            let out = execute(op, value, count, 0).unwrap();
            assert_eq!(out.value, expected, "{block} count={count}");
            assert_eq!(out.writeback, 1, "{block}");
            assert_eq!(out.steady_link_delta, 0, "{block}");
            assert_eq!(out.quiescent, 1, "{block}");
            assert_eq!(test_last_proof_available(), 1, "{block}");

            let proof_json = {
                let guard = LAST_PROOF_JSON
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                guard.clone()
            };
            let proof: serde_json::Value =
                serde_json::from_str(&proof_json).unwrap();
            assert_eq!(proof["block"], block);
            let memory_id =
                proof["load"]["memoryInstanceId"].as_str().unwrap();
            assert_eq!(
                proof["execute"]["memoryInstanceId"].as_str().unwrap(),
                memory_id
            );
            assert_eq!(
                proof["result"]["memoryInstanceId"].as_str().unwrap(),
                memory_id
            );
            assert_eq!(proof["load"]["carrierRoundTrip"], true);
            assert_eq!(proof["execute"]["finalQuiescent"], true);
            assert_eq!(proof["result"]["oracleMatches"], true);
            assert_eq!(proof["result"]["identicalRerunLinkDelta"], 0);
            assert_eq!(
                proof["result"]["decodedValue"].as_u64().unwrap() as u32,
                out.value
            );
            assert_eq!(
                proof["result"]["visualLinks"].as_array().unwrap().len() as u64,
                proof["result"]["linksFinal"].as_u64().unwrap()
            );

            let roots = proof["prepare"]["semanticRoots"].as_array().unwrap();
            let source_for = |role: &str| {
                roots
                    .iter()
                    .find(|root| root["role"] == role)
                    .and_then(|root| root["source"].as_str())
                    .unwrap()
            };
            for required in [
                "function.shift.selected",
                "function.shift.shl",
                "function.shift.shr",
                "function.shift.sar",
                "data.value.word",
                "data.count.word",
                "execution.interpreter",
                "execution.theory",
                "execution.apply",
                "invocation.call",
                "scope.initial",
                "result.tag",
                "result.flag.set_tag",
                "result.flag.undefined_tag",
            ] {
                assert!(
                    roots.iter().any(|root| root["role"] == required),
                    "{block}: missing {required}"
                );
            }
            let selected_role = match op {
                13 => "function.shift.shl",
                14 => "function.shift.shr",
                15 => "function.shift.sar",
                _ => unreachable!(),
            };
            assert_eq!(
                source_for("function.shift.selected"),
                source_for(selected_role),
                "{block}: selected structural function mismatch"
            );

            if op == 13 && count == 32 {
                assert_eq!(out.defined_mask, 0, "masked-zero SHL32 must define no status flags");
                assert_eq!(out.undefined_mask, 0, "masked-zero SHL32 must undefine no status flags");
                assert_eq!(out.preserve_mask, STATUS_FLAGS, "masked-zero SHL32 must preserve all status flags");
                assert_eq!(out.reactions, 1, "masked-zero SHL32 must use the one-step structural rule");
            }

            if op == 13 && (count == 1 || count == 33) {
                shift_count_sources.push((
                    count,
                    source_for("data.count.word").to_owned(),
                    out.value,
                    out.defined_mask,
                    out.value_mask,
                    out.undefined_mask,
                    out.preserve_mask,
                    out.reactions,
                ));
            }
        }

        assert_eq!(shift_count_sources.len(), 2);
        shift_count_sources.sort_by_key(|entry| entry.0);
        let count1 = &shift_count_sources[0];
        let count33 = &shift_count_sources[1];
        assert_eq!(count1.0, 1);
        assert_eq!(count33.0, 33);
        assert_ne!(
            count1.1, count33.1,
            "Count8 Anums must differ before structural masking"
        );
        assert_eq!(
            (&count1.2, &count1.3, &count1.4, &count1.5, &count1.6, &count1.7),
            (&count33.2, &count33.3, &count33.4, &count33.5, &count33.6, &count33.7),
            "count 1 and 33 must converge only through structural Count8 masking"
        );

        let mut rotate_aliases = Vec::new();
        for (op, value, count, expected, block) in [
            (16u32, 0x8000_0001u32, 1u32, 0x0000_0003u32, "ROL32"),
            (17u32, 0x0000_0001u32, 1u32, 0x8000_0000u32, "ROR32"),
            (16u32, 0x8000_0001u32, 2u32, 0x0000_0006u32, "ROL32"),
            (16u32, 0x8000_0001u32, 32u32, 0x8000_0001u32, "ROL32"),
            (16u32, 0x8000_0001u32, 33u32, 0x0000_0003u32, "ROL32"),
        ] {
            let out = execute(op, value, count, 0).unwrap();
            assert_eq!(out.value, expected, "{block} count={count}");
            assert_eq!(out.writeback, 1, "{block}");
            assert_eq!(out.steady_link_delta, 0, "{block}");
            assert_eq!(out.quiescent, 1, "{block}");
            assert_eq!(test_last_proof_available(), 1, "{block}");

            let proof_json = {
                let guard = LAST_PROOF_JSON
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                guard.clone()
            };
            let proof: serde_json::Value =
                serde_json::from_str(&proof_json).unwrap();
            assert_eq!(proof["block"], block);
            let memory_id =
                proof["load"]["memoryInstanceId"].as_str().unwrap();
            assert_eq!(
                proof["execute"]["memoryInstanceId"].as_str().unwrap(),
                memory_id
            );
            assert_eq!(
                proof["result"]["memoryInstanceId"].as_str().unwrap(),
                memory_id
            );
            assert_eq!(proof["load"]["carrierRoundTrip"], true);
            assert_eq!(proof["execute"]["finalQuiescent"], true);
            assert_eq!(proof["result"]["oracleMatches"], true);
            assert_eq!(proof["result"]["identicalRerunLinkDelta"], 0);
            assert_eq!(
                proof["result"]["decodedValue"].as_u64().unwrap() as u32,
                out.value
            );

            let roots = proof["prepare"]["semanticRoots"].as_array().unwrap();
            let source_for = |role: &str| {
                roots
                    .iter()
                    .find(|root| root["role"] == role)
                    .and_then(|root| root["source"].as_str())
                    .unwrap()
            };
            for required in [
                "function.rotate.selected",
                "function.rotate.rol",
                "function.rotate.ror",
                "function.gate.xor2",
                "data.value.word",
                "data.count.word",
                "execution.interpreter",
                "execution.theory",
                "execution.apply",
                "invocation.call",
                "scope.initial",
                "result.tag",
                "result.flag.set_tag",
                "result.flag.undefined_tag",
            ] {
                assert!(
                    roots.iter().any(|root| root["role"] == required),
                    "{block}: missing {required}"
                );
            }
            let selected_role =
                if op == 16 { "function.rotate.rol" } else { "function.rotate.ror" };
            assert_eq!(
                source_for("function.rotate.selected"),
                source_for(selected_role),
                "{block}: selected structural function mismatch"
            );

            if count == 1 {
                assert_eq!(out.defined_mask, FLAG_CF | FLAG_OF, "{block}");
                assert_eq!(out.undefined_mask, 0, "{block}");
            } else if count == 2 {
                assert_eq!(out.defined_mask, FLAG_CF, "{block}");
                assert_eq!(out.undefined_mask, FLAG_OF, "{block}");
            } else if count == 32 {
                assert_eq!(out.defined_mask, 0, "{block}");
                assert_eq!(out.undefined_mask, 0, "{block}");
                assert_eq!(out.preserve_mask, STATUS_FLAGS, "{block}");
                assert_eq!(out.reactions, 1, "{block}");
            }

            if op == 16 && (count == 1 || count == 33) {
                rotate_aliases.push((
                    count,
                    source_for("data.count.word").to_owned(),
                    out.value,
                    out.defined_mask,
                    out.value_mask,
                    out.undefined_mask,
                    out.preserve_mask,
                    out.reactions,
                ));
            }
        }
        rotate_aliases.sort_by_key(|entry| entry.0);
        assert_eq!(rotate_aliases.len(), 2);
        assert_ne!(rotate_aliases[0].1, rotate_aliases[1].1);
        assert_eq!(
            (&rotate_aliases[0].2, &rotate_aliases[0].3, &rotate_aliases[0].4,
             &rotate_aliases[0].5, &rotate_aliases[0].6, &rotate_aliases[0].7),
            (&rotate_aliases[1].2, &rotate_aliases[1].3, &rotate_aliases[1].4,
             &rotate_aliases[1].5, &rotate_aliases[1].6, &rotate_aliases[1].7),
            "ROL32 count 1/33 must converge only through structural masking"
        );

        let mut carry_aliases = Vec::new();
        for (op, value, count, cf_in, expected, block) in [
            (18u32, 0x8000_0000u32, 1u32, 1u32, 0x0000_0001u32, "RCL32"),
            (19u32, 0x0000_0001u32, 1u32, 0u32, 0x0000_0000u32, "RCR32"),
            (18u32, 0x8000_0000u32, 2u32, 1u32, 0x0000_0003u32, "RCL32"),
            (18u32, 0x8000_0000u32, 32u32, 1u32, 0x8000_0000u32, "RCL32"),
            (18u32, 0x8000_0000u32, 33u32, 1u32, 0x0000_0001u32, "RCL32"),
        ] {
            let out = execute(op, value, count, cf_in).unwrap();
            assert_eq!(out.value, expected, "{block} count={count}");
            assert_eq!(out.writeback, 1, "{block}");
            assert_eq!(out.steady_link_delta, 0, "{block}");
            assert_eq!(out.quiescent, 1, "{block}");
            assert_eq!(test_last_proof_available(), 1, "{block}");

            let proof_json = {
                let guard = LAST_PROOF_JSON
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                guard.clone()
            };
            let proof: serde_json::Value =
                serde_json::from_str(&proof_json).unwrap();
            assert_eq!(proof["block"], block);
            assert_eq!(proof["load"]["carrierRoundTrip"], true);
            assert_eq!(proof["execute"]["finalQuiescent"], true);
            assert_eq!(proof["result"]["oracleMatches"], true);
            assert_eq!(proof["result"]["identicalRerunLinkDelta"], 0);

            let roots = proof["prepare"]["semanticRoots"].as_array().unwrap();
            let source_for = |role: &str| {
                roots
                    .iter()
                    .find(|root| root["role"] == role)
                    .and_then(|root| root["source"].as_str())
                    .unwrap()
            };
            for required in [
                "function.rotate_carry.selected",
                "function.rotate_carry.rcl",
                "function.rotate_carry.rcr",
                "function.gate.xor2",
                "data.value.word",
                "data.count.word",
                "data.cf_in",
                "data.bit.zero",
                "data.bit.one",
                "execution.interpreter",
                "execution.theory",
                "execution.apply",
                "invocation.call",
                "scope.initial",
                "result.tag",
            ] {
                assert!(
                    roots.iter().any(|root| root["role"] == required),
                    "{block}: missing {required}"
                );
            }
            let selected_role =
                if op == 18 { "function.rotate_carry.rcl" } else { "function.rotate_carry.rcr" };
            assert_eq!(
                source_for("function.rotate_carry.selected"),
                source_for(selected_role),
                "{block}: selected structural function mismatch"
            );
            assert_eq!(
                source_for("data.cf_in"),
                source_for(if cf_in == 0 { "data.bit.zero" } else { "data.bit.one" }),
                "{block}: CF-in is not the structural input bit"
            );

            if count == 1 {
                assert_eq!(out.defined_mask, FLAG_CF | FLAG_OF, "{block}");
                assert_eq!(out.undefined_mask, 0, "{block}");
            } else if count == 2 {
                assert_eq!(out.defined_mask, FLAG_CF, "{block}");
                assert_eq!(out.undefined_mask, FLAG_OF, "{block}");
            } else if count == 32 {
                assert_eq!(out.defined_mask, 0, "{block}");
                assert_eq!(out.undefined_mask, 0, "{block}");
                assert_eq!(out.preserve_mask, STATUS_FLAGS, "{block}");
                assert_eq!(out.reactions, 1, "{block}");
            }

            if op == 18 && cf_in == 1 && (count == 1 || count == 33) {
                carry_aliases.push((
                    count,
                    source_for("data.count.word").to_owned(),
                    out.value,
                    out.defined_mask,
                    out.value_mask,
                    out.undefined_mask,
                    out.preserve_mask,
                    out.reactions,
                ));
            }
        }
        carry_aliases.sort_by_key(|entry| entry.0);
        assert_eq!(carry_aliases.len(), 2);
        assert_ne!(carry_aliases[0].1, carry_aliases[1].1);
        assert_eq!(
            (&carry_aliases[0].2, &carry_aliases[0].3, &carry_aliases[0].4,
             &carry_aliases[0].5, &carry_aliases[0].6, &carry_aliases[0].7),
            (&carry_aliases[1].2, &carry_aliases[1].3, &carry_aliases[1].4,
             &carry_aliases[1].5, &carry_aliases[1].6, &carry_aliases[1].7),
            "RCL32 count 1/33 must converge only through structural masking"
        );

        for (op, value, expected, block, cf_defined, expected_cf_set) in [
            (20u32, 0x7fff_ffffu32, 0x8000_0000u32, "INC32", false, false),
            (21u32, 0x8000_0000u32, 0x7fff_ffffu32, "DEC32", false, false),
            (22u32, 1u32, u32::MAX, "NEG32", true, true),
            (22u32, 0u32, 0u32, "NEG32", true, false),
        ] {
            let out = execute(op, value, 0, 0).unwrap();
            assert_eq!(out.value, expected, "{block}");
            assert_eq!(out.writeback, 1, "{block}");
            assert_eq!(out.undefined_mask, 0, "{block}");
            assert_eq!(out.steady_link_delta, 0, "{block}");
            assert_eq!(out.quiescent, 1, "{block}");
            assert_eq!(test_last_proof_available(), 1, "{block}");

            if cf_defined {
                assert_eq!(out.defined_mask & FLAG_CF, FLAG_CF, "{block}");
                assert_eq!(out.preserve_mask & FLAG_CF, 0, "{block}");
                assert_eq!(
                    out.value_mask & FLAG_CF,
                    if expected_cf_set { FLAG_CF } else { 0 },
                    "{block}"
                );
            } else {
                assert_eq!(out.defined_mask & FLAG_CF, 0, "{block}");
                assert_eq!(out.preserve_mask & FLAG_CF, FLAG_CF, "{block}");
            }

            let proof_json = {
                let guard = LAST_PROOF_JSON
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                guard.clone()
            };
            let proof: serde_json::Value =
                serde_json::from_str(&proof_json).unwrap();
            assert_eq!(proof["block"], block);
            let memory_id =
                proof["load"]["memoryInstanceId"].as_str().unwrap();
            assert_eq!(
                proof["execute"]["memoryInstanceId"].as_str().unwrap(),
                memory_id
            );
            assert_eq!(
                proof["result"]["memoryInstanceId"].as_str().unwrap(),
                memory_id
            );
            assert!(
                proof["execute"]["reactions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|step| step["memoryInstanceId"] == memory_id)
            );
            assert_eq!(proof["load"]["carrierRoundTrip"], true);
            assert_eq!(proof["execute"]["finalQuiescent"], true);
            assert_eq!(proof["result"]["oracleMatches"], true);
            assert_eq!(proof["result"]["identicalRerunLinkDelta"], 0);
            assert_eq!(
                proof["result"]["decodedValue"].as_u64().unwrap() as u32,
                out.value
            );
            assert_eq!(
                proof["result"]["visualLinks"].as_array().unwrap().len() as u64,
                proof["result"]["linksFinal"].as_u64().unwrap()
            );

            let roots = proof["prepare"]["semanticRoots"].as_array().unwrap();
            let source_for = |role: &str| {
                roots
                    .iter()
                    .find(|root| root["role"] == role)
                    .and_then(|root| root["source"].as_str())
                    .unwrap()
            };
            for required in [
                "function.unary.selected",
                "function.unary.inc",
                "function.unary.dec",
                "function.unary.neg",
                "function.flagged_arithmetic",
                "result.flagged_tag",
                "data.value.word",
                "execution.interpreter",
                "execution.theory",
                "execution.apply",
                "invocation.call",
                "scope.initial",
                "result.tag",
                "result.flag.set_tag",
                "result.flag.cf",
                "result.flag.pf",
                "result.flag.af",
                "result.flag.zf",
                "result.flag.sf",
                "result.flag.of",
            ] {
                assert!(
                    roots.iter().any(|root| root["role"] == required),
                    "{block}: missing {required}"
                );
            }
            let selected_role = match op {
                20 => "function.unary.inc",
                21 => "function.unary.dec",
                22 => "function.unary.neg",
                _ => unreachable!(),
            };
            assert_eq!(
                source_for("function.unary.selected"),
                source_for(selected_role),
                "{block}: selected structural function mismatch"
            );
        }

        let raw_mul = execute(23, u32::MAX, 2, 0).unwrap();
        assert_eq!(raw_mul.value, 0xffff_fffe);
        assert_eq!(raw_mul.value_hi, 1);
        assert_eq!(raw_mul.reactions, 33 + 1038);
        assert_eq!(raw_mul.preserve_mask, STATUS_FLAGS);
        assert_eq!(raw_mul.steady_link_delta, 0);

        let mul = execute(24, u32::MAX, 2, 0).unwrap();
        assert_eq!(mul.value, 0xffff_fffe);
        assert_eq!(mul.value_hi, 1);
        assert_eq!(mul.reactions, 97 + 1038);
        assert_eq!(mul.defined_mask, FLAG_CF | FLAG_OF);
        assert_eq!(mul.value_mask, FLAG_CF | FLAG_OF);
        assert_eq!(mul.undefined_mask, FLAG_PF | FLAG_AF | FLAG_ZF | FLAG_SF);
        assert_eq!(mul.preserve_mask, 0);
        assert_eq!(mul.steady_link_delta, 0);
    }

    #[test]
    fn web_lab_rejects_unknown_op_and_invalid_select() {
        assert!(execute(99, 0, 0, 0).is_none());
        assert!(execute(11, 0, 0, 2).is_none());
        assert!(execute(12, 2, 0, 0).is_none());
        assert_eq!(amemory_i386_lab_supports(12), 1);
        assert_eq!(amemory_i386_lab_supports(22), 1);
        assert_eq!(amemory_i386_lab_supports(23), 1);
        assert_eq!(amemory_i386_lab_supports(24), 1);
        assert_eq!(amemory_i386_lab_supports(25), 0);
        assert!(execute(13, 0, 256, 0).is_none());
        assert!(execute(18, 0, 1, 2).is_none());
    }
}
