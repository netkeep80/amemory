use crate::{
    arithmetic_effect_n::web_run_arithmetic,
    logic_effect_n::web_run_logic,
    mul32_n::web_run_mul32,
    mul_effect_n::web_run_mul_effect,
    mux_n::{web_prove_mux1, web_run_mux32},
    rotate32_n::web_run_rotate32,
    rotate_carry32_n::web_run_rotate_carry32,
    shift32_n::web_run_shift32,
    unary_arith_n::web_run_unary32,
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

static LAST_PROOF_JSON: Mutex<String> = Mutex::new(String::new());

fn set_last_proof_json(value: String) {
    let mut guard = LAST_PROOF_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *guard = value;
}

fn clear_last_proof_json() {
    set_last_proof_json(String::new());
}

fn execute(op: u32, a: u32, b: u32, input_flag: u32) -> Option<LabOutcome> {
    clear_last_proof_json();
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
        let out = web_run_mux32(input_flag, a, b)?;
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
        let proof_json = serde_json::to_string(&proof).ok()?;
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
        set_last_proof_json(proof_json);
        return Some(out);
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
        let out = web_run_mul32(a, b);
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
        let out = web_run_mul_effect(a, b);
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
pub extern "C" fn amemory_i386_lab_proof_available() -> u32 {
    let guard = LAST_PROOF_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    u32::from(!guard.is_empty())
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_proof_json_len() -> u32 {
    let guard = LAST_PROOF_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.len() as u32
}

#[no_mangle]
pub extern "C" fn amemory_i386_lab_proof_json_byte(index: u32) -> u32 {
    let guard = LAST_PROOF_JSON
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

        let add = execute(6, u32::MAX, 1, 0).unwrap();
        assert_eq!(add.value, 0);
        assert_eq!(add.value_mask & FLAG_CF, FLAG_CF);
        assert_eq!(add.value_mask & FLAG_ZF, FLAG_ZF);
        assert_eq!(add.steady_link_delta, 0);

        let cmp = execute(10, 7, 9, 0).unwrap();
        assert_eq!(cmp.writeback, 0);
        assert_eq!(cmp.steady_link_delta, 0);

        let mux = execute(11, 0xaaaa_aaaa, 0x5555_5555, 1).unwrap();
        assert_eq!(mux.value, 0x5555_5555);
        assert_eq!(mux.preserve_mask, STATUS_FLAGS);
        assert_eq!(mux.steady_link_delta, 0);

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
        assert_eq!(proof_mux1.value, 1);
        assert_eq!(amemory_i386_lab_proof_available(), 1);
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

        let shl = execute(13, 0x8000_0001, 1, 0).unwrap();
        assert_eq!(shl.value, 2);
        assert_eq!(shl.value_mask & FLAG_CF, FLAG_CF);
        assert_eq!(shl.steady_link_delta, 0);

        let shl_alias = execute(13, 0x8000_0001, 33, 0).unwrap();
        assert_eq!(shl_alias.value, shl.value);
        assert_eq!(shl_alias.value_mask, shl.value_mask);

        let ror = execute(17, 1, 1, 0).unwrap();
        assert_eq!(ror.value, 0x8000_0000);
        assert_eq!(ror.steady_link_delta, 0);

        let rcl = execute(18, 0x8000_0000, 1, 0).unwrap();
        assert_eq!(rcl.quiescent, 1);
        assert_eq!(rcl.steady_link_delta, 0);

        let inc = execute(20, 0x7fff_ffff, 0, 0).unwrap();
        assert_eq!(inc.value, 0x8000_0000);
        assert_eq!(inc.preserve_mask & FLAG_CF, FLAG_CF);

        let neg = execute(22, 1, 0, 0).unwrap();
        assert_eq!(neg.value, u32::MAX);
        assert_eq!(neg.value_mask & FLAG_CF, FLAG_CF);

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
