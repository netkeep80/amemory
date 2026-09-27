use crate::{
    arithmetic_effect_n::{web_prove_arithmetic, web_run_arithmetic},
    logic_effect_n::{web_prove_logic, web_run_logic},
    mul32_n::web_run_mul32,
    mul_effect_n::web_run_mul_effect,
    mux_n::{web_prove_mux1, web_prove_mux32},
    rotate32_n::{web_prove_rotate32, web_run_rotate32},
    rotate_carry32_n::{web_prove_rotate_carry32, web_run_rotate_carry32},
    shift32_n::{web_prove_shift32, web_run_shift32},
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

    if (1..=5).contains(&op) {
        let execution = web_prove_logic(op, a, b)?;
        let proof_json = serde_json::to_string(&execution.proof).ok()?;
        let out = execution.outcome;
        set_last_proof_json(proof_json);
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
        let proof_json = serde_json::to_string(&execution.proof).ok()?;
        let out = execution.outcome;
        set_last_proof_json(proof_json);
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
        let proof_json = serde_json::to_string(&execution.proof).ok()?;
        let out = execution.outcome;
        set_last_proof_json(proof_json);
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


    if (13..=15).contains(&op) {
        let execution = web_prove_shift32(op, a, b)?;
        let proof_json = serde_json::to_string(&execution.proof).ok()?;
        let out = execution.outcome;
        set_last_proof_json(proof_json);
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
        let proof_json = serde_json::to_string(&execution.proof).ok()?;
        let out = execution.outcome;
        set_last_proof_json(proof_json);
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
        let proof_json = serde_json::to_string(&execution.proof).ok()?;
        let out = execution.outcome;
        set_last_proof_json(proof_json);
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
pub extern "C" fn amemory_i386_lab_proof_json_ptr() -> u32 {
    let guard = LAST_PROOF_JSON
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    guard.as_ptr() as usize as u32
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
            assert_eq!(amemory_i386_lab_proof_available(), 1, "{block}");

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
            assert_eq!(amemory_i386_lab_proof_available(), 1, "{block}");

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
        assert_eq!(amemory_i386_lab_proof_available(), 1);

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
            assert_eq!(amemory_i386_lab_proof_available(), 1, "{block}");

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
