use crate::{
    architectural_state_n::{
        web_prove_architectural_state_add,
        web_prove_architectural_state_add_ecx,
        web_prove_architectural_state_mul,
    },
    arithmetic_effect_n::{web_prove_arithmetic, web_run_arithmetic},
    logic_effect_n::{web_prove_logic, web_run_logic},
    memory_n::web_prove_radix_memory,
    mul32_n::web_prove_mul32,
    mul_effect_n::web_prove_mul_effect,
    mux_n::{web_prove_mux1, web_prove_mux32},
    rotate32_n::{web_prove_rotate32, web_run_rotate32},
    rotate_carry32_n::{web_prove_rotate_carry32, web_run_rotate_carry32},
    shift32_n::{web_prove_shift32, web_run_shift32},
    unary_arith_n::{web_prove_unary32, web_run_unary32},
    proof_n::WebStructuralProof,
};
use std::sync::Mutex;

const FLAG_CF: u32 = 1 << 0;
const FLAG_PF: u32 = 1 << 2;
const FLAG_AF: u32 = 1 << 4;
const FLAG_ZF: u32 = 1 << 6;
const FLAG_SF: u32 = 1 << 7;
const FLAG_OF: u32 = 1 << 11;
const STATUS_FLAGS: u32 = FLAG_CF | FLAG_PF | FLAG_AF | FLAG_ZF | FLAG_SF | FLAG_OF;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

#[cfg(test)]
static LAST_PROOF_JSON: Mutex<String> = Mutex::new(String::new());
static LAST_COMPACT_PROOF_JSON: Mutex<String> = Mutex::new(String::new());

fn set_last_compact_proof(proof: &WebStructuralProof) -> Option<()> {
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

    let mut compact_guard = LAST_COMPACT_PROOF_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *compact_guard = compact_json;
    Some(())
}

fn clear_last_compact_proof() {
    #[cfg(test)]
    {
        let mut proof_guard = LAST_PROOF_JSON
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        proof_guard.clear();
    }

    let mut compact_guard = LAST_COMPACT_PROOF_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    compact_guard.clear();
}

fn execute(op: u32, a: u32, b: u32, input_flag: u32) -> Option<LabOutcome> {
    clear_last_compact_proof();

    if (1..=5).contains(&op) {
        let execution = web_prove_logic(op, a, b)?;
        set_last_compact_proof(&execution.proof)?;
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
        set_last_compact_proof(&execution.proof)?;
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
        set_last_compact_proof(&execution.proof)?;
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
        set_last_compact_proof(&proof)?;
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
        set_last_compact_proof(&execution.proof)?;
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
        set_last_compact_proof(&execution.proof)?;
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
        set_last_compact_proof(&execution.proof)?;
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
        set_last_compact_proof(&execution.proof)?;
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
        set_last_compact_proof(&execution.proof)?;
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
        set_last_compact_proof(&execution.proof)?;
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

static mut LAST_VALUE: u32 = 0;
static mut LAST_VALUE_HI: u32 = 0;
static mut LAST_WRITEBACK: u32 = 0;
static mut LAST_DEFINED_MASK: u32 = 0;
static mut LAST_VALUE_MASK: u32 = 0;
static mut LAST_UNDEFINED_MASK: u32 = 0;
static mut LAST_PRESERVE_MASK: u32 = 0;
static mut LAST_REACTIONS: u32 = 0;
static mut LAST_LINKS_AFTER_BUILD: u32 = 0;
static mut LAST_LINKS_AFTER_FIRST: u32 = 0;
static mut LAST_STEADY_LINK_DELTA: u32 = 0;
static mut LAST_QUIESCENT: u32 = 0;

static mut M5A_EAX_BEFORE: u32 = 0;
static mut M5A_EBX_BEFORE: u32 = 0;
static mut M5A_EDX_BEFORE: u32 = 0;
static mut M5A_ECX_BEFORE: u32 = 0;
static mut M5A_ESI_BEFORE: u32 = 0;
static mut M5A_EDI_BEFORE: u32 = 0;
static mut M5A_EBP_BEFORE: u32 = 0;
static mut M5A_ESP_BEFORE: u32 = 0;
static mut M5A_EIP_BEFORE: u32 = 0;
static mut M5A_EAX_AFTER: u32 = 0;
static mut M5A_EBX_AFTER: u32 = 0;
static mut M5A_EDX_AFTER: u32 = 0;
static mut M5A_ECX_AFTER: u32 = 0;
static mut M5A_ESI_AFTER: u32 = 0;
static mut M5A_EDI_AFTER: u32 = 0;
static mut M5A_EBP_AFTER: u32 = 0;
static mut M5A_ESP_AFTER: u32 = 0;
static mut M5A_EIP_AFTER: u32 = 0;
static mut M5A_FLAGS_DEFINED_MASK: u32 = 0;
static mut M5A_FLAGS_VALUE_MASK: u32 = 0;
static mut M5A_FLAGS_UNDEFINED_MASK: u32 = 0;
static mut M5A_REACTIONS: u32 = 0;
static mut M5A_OLD_STATE_RETAINED: u32 = 0;
static mut M5A_ATOMIC_SCOPE: u32 = 0;
static mut M5A_STEADY_LINK_DELTA: u32 = 0;
static mut M5A_QUIESCENT: u32 = 0;

#[no_mangle]
pub extern "C" fn amemory_i386_state_probe() -> u32 {
    0x0000_050a
}

fn store_state_outcome(
    out: crate::architectural_state_n::WebArchitecturalStateOutcome,
) {
    unsafe {
        M5A_EAX_BEFORE = out.eax_before;
        M5A_EBX_BEFORE = out.ebx_before;
        M5A_EDX_BEFORE = out.edx_before;
        M5A_ECX_BEFORE = out.ecx_before;
        M5A_ESI_BEFORE = out.esi_before;
        M5A_EDI_BEFORE = out.edi_before;
        M5A_EBP_BEFORE = out.ebp_before;
        M5A_ESP_BEFORE = out.esp_before;
        M5A_EIP_BEFORE = out.eip_before;
        M5A_EAX_AFTER = out.eax_after;
        M5A_EBX_AFTER = out.ebx_after;
        M5A_EDX_AFTER = out.edx_after;
        M5A_ECX_AFTER = out.ecx_after;
        M5A_ESI_AFTER = out.esi_after;
        M5A_EDI_AFTER = out.edi_after;
        M5A_EBP_AFTER = out.ebp_after;
        M5A_ESP_AFTER = out.esp_after;
        M5A_EIP_AFTER = out.eip_after;
        M5A_FLAGS_DEFINED_MASK = out.flags_defined_mask;
        M5A_FLAGS_VALUE_MASK = out.flags_value_mask;
        M5A_FLAGS_UNDEFINED_MASK = out.flags_undefined_mask;
        M5A_REACTIONS = out.reactions;
        M5A_OLD_STATE_RETAINED = u32::from(out.old_state_retained);
        M5A_ATOMIC_SCOPE = u32::from(out.atomic_scope);
        M5A_STEADY_LINK_DELTA = out.steady_link_delta;
        M5A_QUIESCENT = u32::from(out.quiescent);
    }
}

#[no_mangle]
pub extern "C" fn amemory_i386_state_run_add32() -> u32 {
    clear_last_compact_proof();
    let Some(execution) = web_prove_architectural_state_add() else {
        return 0;
    };
    if set_last_compact_proof(&execution.proof).is_none() {
        return 0;
    }
    store_state_outcome(execution.outcome);
    1
}

#[no_mangle]
pub extern "C" fn amemory_i386_state_wide_probe() -> u32 {
    0x0000_050b
}

#[no_mangle]
pub extern "C" fn amemory_i386_state_run_mul32() -> u32 {
    clear_last_compact_proof();
    let Some(execution) = web_prove_architectural_state_mul() else {
        return 0;
    };
    if set_last_compact_proof(&execution.proof).is_none() {
        return 0;
    }
    store_state_outcome(execution.outcome);
    1
}

#[no_mangle]
pub extern "C" fn amemory_i386_state_full_probe() -> u32 {
    0x0000_050c
}

#[no_mangle]
pub extern "C" fn amemory_i386_state_run_add_ecx() -> u32 {
    clear_last_compact_proof();
    let Some(execution) = web_prove_architectural_state_add_ecx() else {
        return 0;
    };
    if set_last_compact_proof(&execution.proof).is_none() {
        return 0;
    }
    store_state_outcome(execution.outcome);
    1
}

#[no_mangle]
pub extern "C" fn amemory_i386_state_eax_before() -> u32 {
    unsafe { M5A_EAX_BEFORE }
}
#[no_mangle]
pub extern "C" fn amemory_i386_state_ebx_before() -> u32 {
    unsafe { M5A_EBX_BEFORE }
}
#[no_mangle]
pub extern "C" fn amemory_i386_state_edx_before() -> u32 {
    unsafe { M5A_EDX_BEFORE }
}
#[no_mangle]
pub extern "C" fn amemory_i386_state_ecx_before() -> u32 {
    unsafe { M5A_ECX_BEFORE }
}
#[no_mangle]
pub extern "C" fn amemory_i386_state_esi_before() -> u32 {
    unsafe { M5A_ESI_BEFORE }
}
#[no_mangle]
pub extern "C" fn amemory_i386_state_edi_before() -> u32 {
    unsafe { M5A_EDI_BEFORE }
}
#[no_mangle]
pub extern "C" fn amemory_i386_state_ebp_before() -> u32 {
    unsafe { M5A_EBP_BEFORE }
}
#[no_mangle]
pub extern "C" fn amemory_i386_state_esp_before() -> u32 {
    unsafe { M5A_ESP_BEFORE }
}
#[no_mangle]
pub extern "C" fn amemory_i386_state_eip_before() -> u32 {
    unsafe { M5A_EIP_BEFORE }
}
#[no_mangle]
pub extern "C" fn amemory_i386_state_eax_after() -> u32 {
    unsafe { M5A_EAX_AFTER }
}
#[no_mangle]
pub extern "C" fn amemory_i386_state_ebx_after() -> u32 {
    unsafe { M5A_EBX_AFTER }
}
#[no_mangle]
pub extern "C" fn amemory_i386_state_edx_after() -> u32 {
    unsafe { M5A_EDX_AFTER }
}
#[no_mangle]
pub extern "C" fn amemory_i386_state_ecx_after() -> u32 {
    unsafe { M5A_ECX_AFTER }
}
#[no_mangle]
pub extern "C" fn amemory_i386_state_esi_after() -> u32 {
    unsafe { M5A_ESI_AFTER }
}
#[no_mangle]
pub extern "C" fn amemory_i386_state_edi_after() -> u32 {
    unsafe { M5A_EDI_AFTER }
}
#[no_mangle]
pub extern "C" fn amemory_i386_state_ebp_after() -> u32 {
    unsafe { M5A_EBP_AFTER }
}
#[no_mangle]
pub extern "C" fn amemory_i386_state_esp_after() -> u32 {
    unsafe { M5A_ESP_AFTER }
}
#[no_mangle]
pub extern "C" fn amemory_i386_state_eip_after() -> u32 {
    unsafe { M5A_EIP_AFTER }
}
#[no_mangle]
pub extern "C" fn amemory_i386_state_flags_defined_mask() -> u32 {
    unsafe { M5A_FLAGS_DEFINED_MASK }
}
#[no_mangle]
pub extern "C" fn amemory_i386_state_flags_value_mask() -> u32 {
    unsafe { M5A_FLAGS_VALUE_MASK }
}
#[no_mangle]
pub extern "C" fn amemory_i386_state_flags_undefined_mask() -> u32 {
    unsafe { M5A_FLAGS_UNDEFINED_MASK }
}
#[no_mangle]
pub extern "C" fn amemory_i386_state_reactions() -> u32 {
    unsafe { M5A_REACTIONS }
}
#[no_mangle]
pub extern "C" fn amemory_i386_state_old_state_retained() -> u32 {
    unsafe { M5A_OLD_STATE_RETAINED }
}
#[no_mangle]
pub extern "C" fn amemory_i386_state_atomic_scope() -> u32 {
    unsafe { M5A_ATOMIC_SCOPE }
}
#[no_mangle]
pub extern "C" fn amemory_i386_state_steady_link_delta() -> u32 {
    unsafe { M5A_STEADY_LINK_DELTA }
}
#[no_mangle]
pub extern "C" fn amemory_i386_state_quiescent() -> u32 {
    unsafe { M5A_QUIESCENT }
}

static mut M6A_OFFSET: u32 = 0;
static mut M6A_WRITE_VALUE: u32 = 0;
static mut M6A_BEFORE_VALUE: u32 = 0;
static mut M6A_AFTER_VALUE: u32 = 0;
static mut M6A_OLD_AFTER_VALUE: u32 = 0;
static mut M6A_OLD_ROOT_REF: u32 = 0;
static mut M6A_NEW_ROOT_REF: u32 = 0;
static mut M6A_REACTIONS: u32 = 0;
static mut M6A_LINKS_AFTER_LOAD: u32 = 0;
static mut M6A_LINKS_FINAL: u32 = 0;
static mut M6A_STEADY_LINK_DELTA: u32 = 0;
static mut M6A_QUIESCENT: u32 = 0;

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
    let Some(execution) =
        web_prove_radix_memory(offset as u8, value as u8)
    else {
        return 0;
    };
    if set_last_compact_proof(&execution.proof).is_none() {
        return 0;
    }
    let out = execution.outcome;
    unsafe {
        M6A_OFFSET = out.offset;
        M6A_WRITE_VALUE = out.write_value;
        M6A_BEFORE_VALUE = out.before_value;
        M6A_AFTER_VALUE = out.after_value;
        M6A_OLD_AFTER_VALUE = out.old_after_value;
        M6A_OLD_ROOT_REF = out.old_root_ref;
        M6A_NEW_ROOT_REF = out.new_root_ref;
        M6A_REACTIONS = out.reactions;
        M6A_LINKS_AFTER_LOAD = out.links_after_load;
        M6A_LINKS_FINAL = out.links_final;
        M6A_STEADY_LINK_DELTA = out.steady_link_delta;
        M6A_QUIESCENT = u32::from(out.quiescent);
    }
    1
}

#[no_mangle]
pub extern "C" fn amemory_i386_memory_offset() -> u32 {
    unsafe { M6A_OFFSET }
}
#[no_mangle]
pub extern "C" fn amemory_i386_memory_write_value() -> u32 {
    unsafe { M6A_WRITE_VALUE }
}
#[no_mangle]
pub extern "C" fn amemory_i386_memory_before_value() -> u32 {
    unsafe { M6A_BEFORE_VALUE }
}
#[no_mangle]
pub extern "C" fn amemory_i386_memory_after_value() -> u32 {
    unsafe { M6A_AFTER_VALUE }
}
#[no_mangle]
pub extern "C" fn amemory_i386_memory_old_after_value() -> u32 {
    unsafe { M6A_OLD_AFTER_VALUE }
}
#[no_mangle]
pub extern "C" fn amemory_i386_memory_old_root_ref() -> u32 {
    unsafe { M6A_OLD_ROOT_REF }
}
#[no_mangle]
pub extern "C" fn amemory_i386_memory_new_root_ref() -> u32 {
    unsafe { M6A_NEW_ROOT_REF }
}
#[no_mangle]
pub extern "C" fn amemory_i386_memory_reactions() -> u32 {
    unsafe { M6A_REACTIONS }
}
#[no_mangle]
pub extern "C" fn amemory_i386_memory_links_after_load() -> u32 {
    unsafe { M6A_LINKS_AFTER_LOAD }
}
#[no_mangle]
pub extern "C" fn amemory_i386_memory_links_final() -> u32 {
    unsafe { M6A_LINKS_FINAL }
}
#[no_mangle]
pub extern "C" fn amemory_i386_memory_steady_link_delta() -> u32 {
    unsafe { M6A_STEADY_LINK_DELTA }
}
#[no_mangle]
pub extern "C" fn amemory_i386_memory_quiescent() -> u32 {
    unsafe { M6A_QUIESCENT }
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
pub extern "C" fn amemory_i386_lab_run(
    op: u32,
    a: u32,
    b: u32,
    input_flag: u32,
) -> u32 {
    let Some(out) = execute(op, a, b, input_flag) else {
        return 0;
    };

    unsafe {
        LAST_VALUE = out.value;
        LAST_VALUE_HI = out.value_hi;
        LAST_WRITEBACK = out.writeback;
        LAST_DEFINED_MASK = out.defined_mask;
        LAST_VALUE_MASK = out.value_mask;
        LAST_UNDEFINED_MASK = out.undefined_mask;
        LAST_PRESERVE_MASK = out.preserve_mask;
        LAST_REACTIONS = out.reactions;
        LAST_LINKS_AFTER_BUILD = out.links_after_build;
        LAST_LINKS_AFTER_FIRST = out.links_after_first;
        LAST_STEADY_LINK_DELTA = out.steady_link_delta;
        LAST_QUIESCENT = out.quiescent;
    }
    1
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_value() -> u32 { unsafe { LAST_VALUE } }
#[no_mangle]
pub extern "C" fn amemory_i386_lab_value_hi() -> u32 { unsafe { LAST_VALUE_HI } }
#[no_mangle]
pub extern "C" fn amemory_i386_lab_writeback() -> u32 { unsafe { LAST_WRITEBACK } }
#[no_mangle]
pub extern "C" fn amemory_i386_lab_defined_mask() -> u32 { unsafe { LAST_DEFINED_MASK } }
#[no_mangle]
pub extern "C" fn amemory_i386_lab_value_mask() -> u32 { unsafe { LAST_VALUE_MASK } }
#[no_mangle]
pub extern "C" fn amemory_i386_lab_undefined_mask() -> u32 { unsafe { LAST_UNDEFINED_MASK } }
#[no_mangle]
pub extern "C" fn amemory_i386_lab_preserve_mask() -> u32 { unsafe { LAST_PRESERVE_MASK } }
#[no_mangle]
pub extern "C" fn amemory_i386_lab_reactions() -> u32 { unsafe { LAST_REACTIONS } }
#[no_mangle]
pub extern "C" fn amemory_i386_lab_links_after_build() -> u32 { unsafe { LAST_LINKS_AFTER_BUILD } }
#[no_mangle]
pub extern "C" fn amemory_i386_lab_links_after_first() -> u32 { unsafe { LAST_LINKS_AFTER_FIRST } }
#[no_mangle]
pub extern "C" fn amemory_i386_lab_steady_link_delta() -> u32 { unsafe { LAST_STEADY_LINK_DELTA } }
#[no_mangle]
pub extern "C" fn amemory_i386_lab_quiescent() -> u32 { unsafe { LAST_QUIESCENT } }

#[no_mangle]
pub extern "C" fn amemory_i386_lab_compact_proof_available() -> u32 {
    let guard = LAST_COMPACT_PROOF_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    u32::from(!guard.is_empty())
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_compact_proof_json_len() -> u32 {
    let guard = LAST_COMPACT_PROOF_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.len() as u32
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_compact_proof_json_ptr() -> u32 {
    let guard = LAST_COMPACT_PROOF_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.as_ptr() as usize as u32
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_compact_proof_json_byte(
    index: u32,
) -> u32 {
    let guard = LAST_COMPACT_PROOF_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard
        .as_bytes()
        .get(index as usize)
        .copied()
        .map(u32::from)
        .unwrap_or(u32::MAX)
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
