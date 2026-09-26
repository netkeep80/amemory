use super::{
    full_adder::{
        call, define_bundle_rule, index_rule_for, Fixture as FullFixture,
    },
    logic_n::{install_gate_basis, GateSet},
};
use amemory_optimized_cpu_probe::{
    structural::{materialize_exact_sequence, read_exact_sequence},
    Handle, OptimizedLinkStore, OptimizedStructuralEngine,
};
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::atomic::{AtomicU32, Ordering},
};

const WIDTH: usize = 32;
static NEXT_PROOF_MEMORY_ID: AtomicU32 = AtomicU32::new(1);

struct AnchorGen {
    current: Handle,
    flip: bool,
    o: Handle,
    c: Handle,
}

impl AnchorGen {
    fn new(
        store: &mut OptimizedLinkStore,
        mut seed: Handle,
        o: Handle,
        c: Handle,
    ) -> Self {
        for pole in [o, c, o, o, c, o, c, c, o, c, c, c, o] {
            seed = store.ensure_pair(seed, pole).unwrap();
        }
        Self {
            current: seed,
            flip: false,
            o,
            c,
        }
    }

    fn next(&mut self, store: &mut OptimizedLinkStore) -> Handle {
        let pole = if self.flip { self.c } else { self.o };
        self.flip = !self.flip;
        self.current = store.ensure_pair(self.current, pole).unwrap();
        self.current
    }

    fn roles(
        &mut self,
        store: &mut OptimizedLinkStore,
        count: usize,
    ) -> Vec<Handle> {
        (0..count).map(|_| self.next(store)).collect()
    }
}

fn stage_frame(
    store: &mut OptimizedLinkStore,
    tag: Handle,
    values: &[Handle],
) -> Handle {
    let payload = materialize_exact_sequence(store, values).unwrap();
    let descriptor = store.ensure_pair(tag, payload).unwrap();
    store.ensure_start_self_closed(descriptor).unwrap()
}

fn binary_call(
    f: &mut FullFixture,
    function: Handle,
    a: Handle,
    b: Handle,
) -> Handle {
    let args = materialize_exact_sequence(&mut f.store, &[a, b]).unwrap();
    call(&mut f.store, f.apply, function, args)
}

#[derive(Clone, Debug)]
struct MuxProgram {
    direct_mux1: Handle,
    mux1: Handle,
    mux32: Handle,
    bit_result_tag: Handle,
    word_result_tag: Handle,
    gates: GateSet,
    direct_steps: usize,
    mux1_steps: usize,
    mux32_steps: usize,
    links_after_build: usize,
}

impl MuxProgram {
    fn install(f: &mut FullFixture) -> Self {
        let gates = install_gate_basis(f);

        let seed0 = f.store.ensure_pair(f.k, gates.xor2).unwrap();
        let mut anchors =
            AnchorGen::new(&mut f.store, seed0, f.o, f.c);

        let direct_left = anchors.next(&mut f.store);
        let direct_right = anchors.next(&mut f.store);
        let direct_mux1 =
            f.store.ensure_pair(direct_left, direct_right).unwrap();

        let mux1_left = anchors.next(&mut f.store);
        let mux1_right = anchors.next(&mut f.store);
        let mux1 =
            f.store.ensure_pair(mux1_left, mux1_right).unwrap();

        let mux32_left = anchors.next(&mut f.store);
        let mux32_right = anchors.next(&mut f.store);
        let mux32 =
            f.store.ensure_pair(mux32_left, mux32_right).unwrap();

        let bit_tag_left = anchors.next(&mut f.store);
        let bit_tag_right = anchors.next(&mut f.store);
        let bit_result_tag =
            f.store.ensure_pair(bit_tag_left, bit_tag_right).unwrap();

        let word_tag_left = anchors.next(&mut f.store);
        let word_tag_right = anchors.next(&mut f.store);
        let word_result_tag =
            f.store.ensure_pair(word_tag_left, word_tag_right).unwrap();

        let xor_ab_tag = anchors.next(&mut f.store);
        let and_s_tag = anchors.next(&mut f.store);
        let xor_out_tag = anchors.next(&mut f.store);

        for (select, choose_b) in [(f.zero, false), (f.one, true)] {
            let k = anchors.next(&mut f.store);
            let a = anchors.next(&mut f.store);
            let b = anchors.next(&mut f.store);

            let args = materialize_exact_sequence(
                &mut f.store,
                &[select, a, b],
            )
            .unwrap();
            let invocation =
                call(&mut f.store, f.apply, direct_mux1, args);
            let before = f.store.ensure_pair(k, invocation).unwrap();

            let selected = if choose_b { b } else { a };
            let payload =
                materialize_exact_sequence(&mut f.store, &[selected])
                    .unwrap();
            let endpoint =
                f.store.ensure_pair(bit_result_tag, payload).unwrap();
            let after = f.store.ensure_pair(k, endpoint).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, a, b],
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        {
            let k = anchors.next(&mut f.store);
            let s = anchors.next(&mut f.store);
            let a = anchors.next(&mut f.store);
            let b = anchors.next(&mut f.store);

            let args =
                materialize_exact_sequence(&mut f.store, &[s, a, b])
                    .unwrap();
            let invocation =
                call(&mut f.store, f.apply, mux1, args);
            let before = f.store.ensure_pair(k, invocation).unwrap();

            let caller =
                stage_frame(&mut f.store, xor_ab_tag, &[k, s, a]);
            let xor_call =
                binary_call(f, gates.xor2, a, b);
            let after = f.store.ensure_pair(caller, xor_call).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, s, a, b],
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        {
            let k = anchors.next(&mut f.store);
            let s = anchors.next(&mut f.store);
            let a = anchors.next(&mut f.store);
            let x = anchors.next(&mut f.store);

            let caller =
                stage_frame(&mut f.store, xor_ab_tag, &[k, s, a]);
            let x_result =
                materialize_exact_sequence(&mut f.store, &[x]).unwrap();
            let before = f.store.ensure_pair(caller, x_result).unwrap();

            let next_caller =
                stage_frame(&mut f.store, and_s_tag, &[k, a]);
            let and_call =
                binary_call(f, gates.and2, s, x);
            let after =
                f.store.ensure_pair(next_caller, and_call).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, s, a, x],
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &gates.bit_outputs,
                admission,
            );
        }

        {
            let k = anchors.next(&mut f.store);
            let a = anchors.next(&mut f.store);
            let y = anchors.next(&mut f.store);

            let caller =
                stage_frame(&mut f.store, and_s_tag, &[k, a]);
            let y_result =
                materialize_exact_sequence(&mut f.store, &[y]).unwrap();
            let before = f.store.ensure_pair(caller, y_result).unwrap();

            let next_caller =
                stage_frame(&mut f.store, xor_out_tag, &[k]);
            let xor_call =
                binary_call(f, gates.xor2, a, y);
            let after =
                f.store.ensure_pair(next_caller, xor_call).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, a, y],
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &gates.bit_outputs,
                admission,
            );
        }

        {
            let k = anchors.next(&mut f.store);
            let out = anchors.next(&mut f.store);

            let caller =
                stage_frame(&mut f.store, xor_out_tag, &[k]);
            let out_result =
                materialize_exact_sequence(&mut f.store, &[out]).unwrap();
            let before =
                f.store.ensure_pair(caller, out_result).unwrap();

            let payload =
                materialize_exact_sequence(&mut f.store, &[out]).unwrap();
            let endpoint =
                f.store.ensure_pair(bit_result_tag, payload).unwrap();
            let after = f.store.ensure_pair(k, endpoint).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, out],
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &gates.bit_outputs,
                admission,
            );
        }

        let mut word_tags = Vec::with_capacity(WIDTH);
        for _ in 0..WIDTH {
            let left = anchors.next(&mut f.store);
            let right = anchors.next(&mut f.store);
            word_tags.push(f.store.ensure_pair(left, right).unwrap());
        }

        {
            let k = anchors.next(&mut f.store);
            let s = anchors.next(&mut f.store);
            let a_bits = anchors.roles(&mut f.store, WIDTH);
            let b_bits = anchors.roles(&mut f.store, WIDTH);

            let aword =
                materialize_exact_sequence(&mut f.store, &a_bits).unwrap();
            let bword =
                materialize_exact_sequence(&mut f.store, &b_bits).unwrap();
            let args =
                materialize_exact_sequence(&mut f.store, &[s, aword, bword])
                    .unwrap();
            let invocation =
                call(&mut f.store, f.apply, mux32, args);
            let before = f.store.ensure_pair(k, invocation).unwrap();

            let mut state = Vec::with_capacity(2 + 2 * (WIDTH - 1));
            state.push(k);
            state.push(s);
            state.extend_from_slice(&a_bits[1..]);
            state.extend_from_slice(&b_bits[1..]);

            let caller =
                stage_frame(&mut f.store, word_tags[0], &state);
            let mux_args = materialize_exact_sequence(
                &mut f.store,
                &[s, a_bits[0], b_bits[0]],
            )
            .unwrap();
            let mux_call =
                call(&mut f.store, f.apply, mux1, mux_args);
            let after = f.store.ensure_pair(caller, mux_call).unwrap();

            let mut roles = Vec::with_capacity(2 + 2 * WIDTH);
            roles.push(k);
            roles.push(s);
            roles.extend_from_slice(&a_bits);
            roles.extend_from_slice(&b_bits);

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        for i in 0..WIDTH {
            let k = anchors.next(&mut f.store);
            let s = anchors.next(&mut f.store);
            let remaining = WIDTH - i - 1;
            let a_rem = anchors.roles(&mut f.store, remaining);
            let b_rem = anchors.roles(&mut f.store, remaining);
            let previous = anchors.roles(&mut f.store, i);
            let out = anchors.next(&mut f.store);

            let mut state =
                Vec::with_capacity(2 + 2 * remaining + i);
            state.push(k);
            state.push(s);
            state.extend_from_slice(&a_rem);
            state.extend_from_slice(&b_rem);
            state.extend_from_slice(&previous);

            let caller =
                stage_frame(&mut f.store, word_tags[i], &state);
            let bit_payload =
                materialize_exact_sequence(&mut f.store, &[out]).unwrap();
            let bit_endpoint =
                f.store.ensure_pair(bit_result_tag, bit_payload).unwrap();
            let before =
                f.store.ensure_pair(caller, bit_endpoint).unwrap();

            let after = if i + 1 < WIDTH {
                let mut next_previous = previous.clone();
                next_previous.push(out);

                let mut next_state =
                    Vec::with_capacity(2 + 2 * (remaining - 1) + i + 1);
                next_state.push(k);
                next_state.push(s);
                next_state.extend_from_slice(&a_rem[1..]);
                next_state.extend_from_slice(&b_rem[1..]);
                next_state.extend_from_slice(&next_previous);

                let next_caller =
                    stage_frame(&mut f.store, word_tags[i + 1], &next_state);
                let mux_args = materialize_exact_sequence(
                    &mut f.store,
                    &[s, a_rem[0], b_rem[0]],
                )
                .unwrap();
                let mux_call =
                    call(&mut f.store, f.apply, mux1, mux_args);
                f.store.ensure_pair(next_caller, mux_call).unwrap()
            } else {
                let mut result_bits = previous.clone();
                result_bits.push(out);
                let word = materialize_exact_sequence(
                    &mut f.store,
                    &result_bits,
                )
                .unwrap();
                let payload =
                    materialize_exact_sequence(&mut f.store, &[word]).unwrap();
                let endpoint =
                    f.store.ensure_pair(word_result_tag, payload).unwrap();
                f.store.ensure_pair(k, endpoint).unwrap()
            };

            let mut roles =
                Vec::with_capacity(3 + 2 * remaining + i);
            roles.push(k);
            roles.push(s);
            roles.extend_from_slice(&a_rem);
            roles.extend_from_slice(&b_rem);
            roles.extend_from_slice(&previous);
            roles.push(out);

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &[bit_result_tag],
                admission,
            );
        }

        Self {
            direct_mux1,
            mux1,
            mux32,
            bit_result_tag,
            word_result_tag,
            gates,
            direct_steps: 1,
            mux1_steps: 7,
            mux32_steps: 1 + WIDTH * 8,
            links_after_build: f.store.link_count(),
        }
    }
}

fn decode_bit(f: &FullFixture, bit: Handle) -> u8 {
    if bit == f.one {
        1
    } else {
        assert_eq!(bit, f.zero);
        0
    }
}

fn run_exact_steps(
    f: &mut FullFixture,
    steps: usize,
    label: &str,
) {
    for step in 0..steps {
        let result = f.engine.run(&mut f.store).unwrap();
        assert!(
            !result.quiescent,
            "{label}: unexpected quiescence at {step}"
        );
        assert_eq!(result.raw_rule_matches, 1, "{label} step {step}");
        assert_eq!(result.transitioned_members, 1, "{label} step {step}");
        assert_eq!(result.handoff_count, 1, "{label} step {step}");
        assert_eq!(result.next_members.len(), 1, "{label} step {step}");
    }

    let stable = f.engine.current_bank();
    let quiescent = f.engine.run(&mut f.store).unwrap();
    assert!(quiescent.quiescent, "{label}: final quiescence");
    assert_eq!(quiescent.raw_rule_matches, 0);
    assert_eq!(quiescent.handoff_count, 0);
    assert_eq!(f.engine.current_bank(), stable);
}

fn run_mux1(
    f: &mut FullFixture,
    program: &MuxProgram,
    function: Handle,
    steps: usize,
    s: u8,
    a: u8,
    b: u8,
) -> u8 {
    let bits = [f.zero, f.one];
    let args = materialize_exact_sequence(
        &mut f.store,
        &[bits[s as usize], bits[a as usize], bits[b as usize]],
    )
    .unwrap();
    let invocation =
        call(&mut f.store, f.apply, function, args);
    let initial = f.store.ensure_pair(f.k, invocation).unwrap();

    f.engine.set_current(&f.store, &[initial]).unwrap();
    run_exact_steps(f, steps, "MUX1");

    let final_link = f.engine.current()[0];
    let (caller, endpoint) = f.store.poles(final_link).unwrap();
    assert_eq!(caller, f.k);
    let (tag, payload) = f.store.poles(endpoint).unwrap();
    assert_eq!(tag, program.bit_result_tag);
    let values = read_exact_sequence(&f.store, payload).unwrap();
    assert_eq!(values.len(), 1);
    decode_bit(f, values[0])
}

fn bit_handles(
    f: &FullFixture,
    value: u32,
) -> Vec<Handle> {
    (0..WIDTH)
        .map(|bit| {
            if (value >> bit) & 1 == 1 {
                f.one
            } else {
                f.zero
            }
        })
        .collect()
}

fn decode_word(
    f: &FullFixture,
    word: Handle,
) -> u32 {
    let bits = read_exact_sequence(&f.store, word).unwrap();
    assert_eq!(bits.len(), WIDTH);

    let mut value = 0u32;
    for (i, bit) in bits.into_iter().enumerate() {
        value |= u32::from(decode_bit(f, bit)) << i;
    }
    value
}

fn run_mux32(
    f: &mut FullFixture,
    program: &MuxProgram,
    s: u8,
    a: u32,
    b: u32,
) -> u32 {
    let bits = [f.zero, f.one];
    let a_bits = bit_handles(f, a);
    let b_bits = bit_handles(f, b);
    let aword =
        materialize_exact_sequence(&mut f.store, &a_bits).unwrap();
    let bword =
        materialize_exact_sequence(&mut f.store, &b_bits).unwrap();
    let args = materialize_exact_sequence(
        &mut f.store,
        &[bits[s as usize], aword, bword],
    )
    .unwrap();
    let invocation =
        call(&mut f.store, f.apply, program.mux32, args);
    let initial = f.store.ensure_pair(f.k, invocation).unwrap();

    f.engine.set_current(&f.store, &[initial]).unwrap();
    run_exact_steps(f, program.mux32_steps, "MUX32");

    let final_link = f.engine.current()[0];
    let (caller, endpoint) = f.store.poles(final_link).unwrap();
    assert_eq!(caller, f.k);
    let (tag, payload) = f.store.poles(endpoint).unwrap();
    assert_eq!(tag, program.word_result_tag);
    let values = read_exact_sequence(&f.store, payload).unwrap();
    assert_eq!(values.len(), 1);
    decode_word(f, values[0])
}

fn word_vectors() -> Vec<(u32, u32)> {
    let mut out = vec![
        (0, 0),
        (0, u32::MAX),
        (u32::MAX, 0),
        (u32::MAX, u32::MAX),
        (0xaaaa_aaaa, 0x5555_5555),
        (0x8000_0001, 0x7fff_fffe),
        (0x1234_5678, 0x9abc_def0),
    ];

    let mut z = 0x85eb_ca6bu32;
    for _ in 0..5 {
        z = z.wrapping_mul(1664525).wrapping_add(1013904223);
        let a = z;
        z = z.wrapping_mul(1664525).wrapping_add(1013904223);
        out.push((a, z));
    }

    out
}


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WebMuxOutcome {
    pub(crate) value: u32,
    pub(crate) reactions: u32,
    pub(crate) links_after_build: u32,
    pub(crate) links_after_first: u32,
    pub(crate) steady_link_delta: u32,
    pub(crate) quiescent: u8,
}

pub(crate) fn web_run_mux32(
    select: u32,
    a: u32,
    b: u32,
) -> Option<WebMuxOutcome> {
    if select > 1 {
        return None;
    }
    let mut f = FullFixture::new();
    let program = MuxProgram::install(&mut f);
    let links_after_build = f.store.link_count() as u32;
    let first = run_mux32(&mut f, &program, select as u8, a, b);
    let links_after_first = f.store.link_count() as u32;
    let second = run_mux32(&mut f, &program, select as u8, a, b);
    assert_eq!(second, first, "web MUX32 repeat changed result");
    let links_after_second = f.store.link_count() as u32;

    Some(WebMuxOutcome {
        value: first,
        reactions: program.mux32_steps as u32,
        links_after_build,
        links_after_first,
        steady_link_delta: links_after_second - links_after_first,
        quiescent: 1,
    })
}



#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WebMux1Outcome {
    pub(crate) value: u8,
    pub(crate) reactions: u32,
    pub(crate) links_after_build: u32,
    pub(crate) links_after_first: u32,
    pub(crate) steady_link_delta: u32,
    pub(crate) quiescent: u8,
}

pub(crate) fn web_run_mux1(
    select: u32,
    a: u32,
    b: u32,
) -> Option<WebMux1Outcome> {
    if select > 1 || a > 1 || b > 1 {
        return None;
    }

    let mut f = FullFixture::new();
    let program = MuxProgram::install(&mut f);
    let links_after_build = f.store.link_count() as u32;

    let first = run_mux1(
        &mut f,
        &program,
        program.mux1,
        program.mux1_steps,
        select as u8,
        a as u8,
        b as u8,
    );
    let links_after_first = f.store.link_count() as u32;

    let second = run_mux1(
        &mut f,
        &program,
        program.mux1,
        program.mux1_steps,
        select as u8,
        a as u8,
        b as u8,
    );
    assert_eq!(second, first, "web MUX1 repeat changed result");
    let links_after_second = f.store.link_count() as u32;

    Some(WebMux1Outcome {
        value: first,
        reactions: program.mux1_steps as u32,
        links_after_build,
        links_after_first,
        steady_link_delta: links_after_second - links_after_first,
        quiescent: 1,
    })
}



#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebProofPreparedRoot {
    pub(crate) role: String,
    pub(crate) source: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebProofLoadedRoot {
    pub(crate) role: String,
    pub(crate) source: String,
    pub(crate) local_handle: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebProofPrepareStage {
    pub(crate) compiler_label: String,
    pub(crate) runtime_memory_exists: bool,
    pub(crate) aset_anums: Vec<String>,
    pub(crate) semantic_roots: Vec<WebProofPreparedRoot>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebProofLoadStage {
    pub(crate) memory_instance_id: String,
    pub(crate) links_before_load: u32,
    pub(crate) links_after_load: u32,
    pub(crate) imported_anums: u32,
    pub(crate) portable_round_trip: bool,
    pub(crate) semantic_roots: Vec<WebProofLoadedRoot>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebProofReactionStep {
    pub(crate) memory_instance_id: String,
    pub(crate) step: u32,
    pub(crate) scope_before: Vec<String>,
    pub(crate) raw_rule_matches: u32,
    pub(crate) transitioned_members: u32,
    pub(crate) handoff_count: u32,
    pub(crate) scope_after: Vec<String>,
    pub(crate) links_after: u32,
    pub(crate) quiescent: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebProofVisualLink {
    pub(crate) key: String,
    pub(crate) start_key: String,
    pub(crate) end_key: String,
    pub(crate) local_handle: u32,
    pub(crate) anum: String,
    pub(crate) label: Option<String>,
    pub(crate) tags: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebProofExecuteStage {
    pub(crate) memory_instance_id: String,
    pub(crate) reactions: Vec<WebProofReactionStep>,
    pub(crate) active_reaction_count: u32,
    pub(crate) final_quiescent: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebProofResultStage {
    pub(crate) memory_instance_id: String,
    pub(crate) result_anum: String,
    pub(crate) result_sequence_anum: String,
    pub(crate) decoded_value: u8,
    pub(crate) oracle_value: u8,
    pub(crate) oracle_matches: bool,
    pub(crate) links_final: u32,
    pub(crate) visual_links: Vec<WebProofVisualLink>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebMux1Proof {
    pub(crate) schema_version: u32,
    pub(crate) block: String,
    pub(crate) prepare: WebProofPrepareStage,
    pub(crate) load: WebProofLoadStage,
    pub(crate) execute: WebProofExecuteStage,
    pub(crate) result: WebProofResultStage,
}

#[derive(Debug)]
struct ProofRuntimeMemory {
    id: String,
    store: OptimizedLinkStore,
}

fn export_all_anums(store: &OptimizedLinkStore) -> Vec<String> {
    (1..=store.link_count() as u32)
        .map(|handle| store.export_anum(handle).expect("proof export"))
        .collect()
}

fn export_scope(store: &OptimizedLinkStore, scope: &[Handle]) -> Vec<String> {
    scope
        .iter()
        .map(|handle| store.export_anum(*handle).expect("scope export"))
        .collect()
}

fn semantic_source(
    store: &OptimizedLinkStore,
    role: &str,
    handle: Handle,
) -> WebProofPreparedRoot {
    WebProofPreparedRoot {
        role: role.to_owned(),
        source: store.export_anum(handle).expect("semantic root export"),
    }
}

fn visual_snapshot(
    memory: &ProofRuntimeMemory,
    loaded_roots: &[WebProofLoadedRoot],
) -> Vec<WebProofVisualLink> {
    let mut roles_by_handle: HashMap<u32, Vec<String>> = HashMap::new();
    for root in loaded_roots {
        roles_by_handle
            .entry(root.local_handle)
            .or_default()
            .push(root.role.clone());
    }

    (1..=memory.store.link_count() as u32)
        .map(|handle| {
            let (start, end) = memory.store.poles(handle).expect("visual poles");
            let roles = roles_by_handle.get(&handle).cloned().unwrap_or_default();
            WebProofVisualLink {
                key: format!("{}:L{}", memory.id, handle),
                start_key: format!("{}:L{}", memory.id, start),
                end_key: format!("{}:L{}", memory.id, end),
                local_handle: handle,
                anum: memory.store.export_anum(handle).expect("visual Anum"),
                label: (!roles.is_empty()).then(|| roles.join(" + ")),
                tags: roles,
            }
        })
        .collect()
}

pub(crate) fn web_prove_mux1(
    select: u32,
    a: u32,
    b: u32,
) -> Option<WebMux1Proof> {
    if select > 1 || a > 1 || b > 1 {
        return None;
    }

    // Stage 1: compile a complete portable Aset before the runtime A-memory exists.
    // This compiler store is preparation state only; it is intentionally discarded
    // before execution. Runtime identity begins only below at ProofRuntimeMemory.
    let mut compiler = FullFixture::new();
    let program = MuxProgram::install(&mut compiler);
    let bits = [compiler.zero, compiler.one];
    let args = materialize_exact_sequence(
        &mut compiler.store,
        &[
            bits[select as usize],
            bits[a as usize],
            bits[b as usize],
        ],
    )
    .unwrap();
    let invocation = call(
        &mut compiler.store,
        compiler.apply,
        program.mux1,
        args,
    );
    let initial = compiler.store.ensure_pair(compiler.k, invocation).unwrap();

    let prepared_roots = vec![
        semantic_source(&compiler.store, "function.mux1", program.mux1),
        semantic_source(&compiler.store, "data.select", bits[select as usize]),
        semantic_source(&compiler.store, "data.a", bits[a as usize]),
        semantic_source(&compiler.store, "data.b", bits[b as usize]),
        semantic_source(&compiler.store, "data.zero", compiler.zero),
        semantic_source(&compiler.store, "data.one", compiler.one),
        semantic_source(&compiler.store, "execution.interpreter", compiler.interpreter),
        semantic_source(&compiler.store, "execution.theory", compiler.theory),
        semantic_source(&compiler.store, "execution.apply", compiler.apply),
        semantic_source(&compiler.store, "invocation.args", args),
        semantic_source(&compiler.store, "invocation.call", invocation),
        semantic_source(&compiler.store, "scope.initial", initial),
        semantic_source(&compiler.store, "context.caller", compiler.k),
        semantic_source(&compiler.store, "result.tag", program.bit_result_tag),
    ];
    let prepared_anums = export_all_anums(&compiler.store);

    let prepare = WebProofPrepareStage {
        compiler_label: "portable Aset compiler/preparation state (not runtime A-memory)".to_owned(),
        runtime_memory_exists: false,
        aset_anums: prepared_anums.clone(),
        semantic_roots: prepared_roots.clone(),
    };

    // Stage 2: exactly one runtime A-memory is created. Every following stage
    // keeps and mutates this one ProofRuntimeMemory.store instance.
    let memory_number = NEXT_PROOF_MEMORY_ID.fetch_add(1, Ordering::SeqCst);
    let mut memory = ProofRuntimeMemory {
        id: format!("A-memory#{}", memory_number),
        store: OptimizedLinkStore::new(),
    };
    let links_before_load = memory.store.link_count() as u32;
    for source in &prepared_anums {
        memory.store.import_anum(source).ok()?;
    }
    let links_after_load = memory.store.link_count() as u32;

    let mut loaded_roots = Vec::with_capacity(prepared_roots.len());
    let mut portable_round_trip = true;
    for root in &prepared_roots {
        let before = memory.store.link_count();
        let handle = memory.store.import_anum(&root.source).ok()?;
        // Semantic-root resolution must reuse already loaded topology.
        if memory.store.link_count() != before {
            return None;
        }
        if memory.store.export_anum(handle).ok().as_deref() != Some(root.source.as_str()) {
            portable_round_trip = false;
        }
        loaded_roots.push(WebProofLoadedRoot {
            role: root.role.clone(),
            source: root.source.clone(),
            local_handle: handle,
        });
    }

    let find = |role: &str| -> Option<Handle> {
        loaded_roots
            .iter()
            .find(|root| root.role == role)
            .map(|root| root.local_handle)
    };
    let interpreter = find("execution.interpreter")?;
    let initial = find("scope.initial")?;
    let caller = find("context.caller")?;
    let result_tag = find("result.tag")?;
    let zero = find("data.zero")?;
    let one = find("data.one")?;

    let load = WebProofLoadStage {
        memory_instance_id: memory.id.clone(),
        links_before_load,
        links_after_load,
        imported_anums: prepared_anums.len() as u32,
        portable_round_trip,
        semantic_roots: loaded_roots.clone(),
    };

    // Stage 3: execute against the exact same runtime store.
    let mut engine = OptimizedStructuralEngine::new(32);
    engine.set_interpreter(&memory.store, interpreter).ok()?;
    engine.set_current(&memory.store, &[initial]).ok()?;

    let mut reactions = Vec::new();
    for step in 0..64u32 {
        let scope_before = export_scope(&memory.store, engine.current());
        let reaction = engine.run(&mut memory.store).ok()?;
        let scope_after = export_scope(&memory.store, engine.current());
        let quiescent = reaction.quiescent;
        reactions.push(WebProofReactionStep {
            memory_instance_id: memory.id.clone(),
            step,
            scope_before,
            raw_rule_matches: reaction.raw_rule_matches,
            transitioned_members: reaction.transitioned_members,
            handoff_count: reaction.handoff_count,
            scope_after,
            links_after: memory.store.link_count() as u32,
            quiescent,
        });
        if quiescent {
            break;
        }
    }
    if !reactions.last().map(|step| step.quiescent).unwrap_or(false) {
        return None;
    }
    let active_reaction_count =
        reactions.iter().filter(|step| !step.quiescent).count() as u32;

    // Stage 4: result is decoded from this same memory, then visual topology is
    // projected directly from this same store. Host MUX arithmetic is oracle only.
    if engine.current().len() != 1 {
        return None;
    }
    let final_link = engine.current()[0];
    let (final_caller, endpoint) = memory.store.poles(final_link).ok()?;
    if final_caller != caller {
        return None;
    }
    let (tag, payload) = memory.store.poles(endpoint).ok()?;
    if tag != result_tag {
        return None;
    }
    let values = read_exact_sequence(&memory.store, payload).ok()?;
    if values.len() != 1 {
        return None;
    }
    let decoded_value = if values[0] == one {
        1
    } else if values[0] == zero {
        0
    } else {
        return None;
    };
    let oracle_value = if select == 0 { a as u8 } else { b as u8 };
    let result_anum = memory.store.export_anum(final_link).ok()?;
    let result_sequence_anum = memory.store.export_anum(payload).ok()?;
    let visual_links = visual_snapshot(&memory, &loaded_roots);

    let execute = WebProofExecuteStage {
        memory_instance_id: memory.id.clone(),
        reactions,
        active_reaction_count,
        final_quiescent: true,
    };
    let result = WebProofResultStage {
        memory_instance_id: memory.id.clone(),
        result_anum,
        result_sequence_anum,
        decoded_value,
        oracle_value,
        oracle_matches: decoded_value == oracle_value,
        links_final: memory.store.link_count() as u32,
        visual_links,
    };

    Some(WebMux1Proof {
        schema_version: 1,
        block: "MUX1".to_owned(),
        prepare,
        load,
        execute,
        result,
    })
}

#[test]
fn web_mux1_proof_uses_one_runtime_memory_for_all_eight_cases() {
    for select in 0..=1 {
        for a in 0..=1 {
            for b in 0..=1 {
                let proof = web_prove_mux1(select, a, b).expect("MUX1 proof");
                assert!(!proof.prepare.runtime_memory_exists);
                assert!(proof.load.portable_round_trip);
                assert_eq!(proof.load.links_before_load, 1);
                assert!(proof.load.links_after_load > proof.load.links_before_load);
                assert_eq!(proof.execute.active_reaction_count, 7);
                assert!(proof.execute.final_quiescent);
                assert!(proof.result.oracle_matches);

                let id = &proof.load.memory_instance_id;
                assert_eq!(&proof.execute.memory_instance_id, id);
                assert_eq!(&proof.result.memory_instance_id, id);
                assert!(proof.execute.reactions.iter().all(|step| &step.memory_instance_id == id));

                let mut previous = proof.load.links_after_load;
                for step in &proof.execute.reactions {
                    assert!(step.links_after >= previous);
                    previous = step.links_after;
                }
                assert_eq!(proof.result.links_final, previous);

                let keys = proof.result.visual_links
                    .iter()
                    .map(|link| link.key.as_str())
                    .collect::<std::collections::HashSet<_>>();
                for link in &proof.result.visual_links {
                    assert!(keys.contains(link.start_key.as_str()));
                    assert!(keys.contains(link.end_key.as_str()));
                    assert!(link.key.starts_with(id));
                }
            }
        }
    }
}


#[test]
fn m1_mux1_direct_and_composed_all_rows() {
    let mut f = FullFixture::new();
    let program = MuxProgram::install(&mut f);

    assert_eq!(program.direct_steps, 1);
    assert_eq!(program.mux1_steps, 7);

    for s in 0u8..=1 {
        for a in 0u8..=1 {
            for b in 0u8..=1 {
                let direct = run_mux1(
                    &mut f,
                    &program,
                    program.direct_mux1,
                    program.direct_steps,
                    s,
                    a,
                    b,
                );
                let composed = run_mux1(
                    &mut f,
                    &program,
                    program.mux1,
                    program.mux1_steps,
                    s,
                    a,
                    b,
                );
                let expected = if s == 0 { a } else { b };
                assert_eq!(direct, expected);
                assert_eq!(composed, expected);
                assert_eq!(direct, composed);
            }
        }
    }
}

#[test]
#[ignore = "heavy composed MUX32 suite; mandatory release workflow"]
fn m1_mux32_composed_selects_canonical_word() {
    let mut f = FullFixture::new();
    let program = MuxProgram::install(&mut f);

    assert_eq!(program.mux32_steps, 257);

    let vectors = word_vectors();
    for &(a, b) in &vectors {
        let select_a = run_mux32(&mut f, &program, 0, a, b);
        let select_b = run_mux32(&mut f, &program, 1, a, b);
        assert_eq!(select_a, a);
        assert_eq!(select_b, b);
    }

    println!(
        "M1_MUX32 vectors={} reactions={} program_links={} xor={} and={}",
        vectors.len() * 2,
        program.mux32_steps,
        program.links_after_build,
        program.gates.xor2,
        program.gates.and2,
    );
}

#[test]
#[ignore = "heavy composed MUX32 suite; mandatory release workflow"]
fn m1_mux32_steady_state_has_zero_link_growth() {
    let mut f = FullFixture::new();
    let program = MuxProgram::install(&mut f);
    let a = 0x1357_9bdfu32;
    let b = 0x2468_ace0u32;

    let first = run_mux32(&mut f, &program, 1, a, b);
    assert_eq!(first, b);

    let links = f.store.link_count();
    let second = run_mux32(&mut f, &program, 1, a, b);
    assert_eq!(second, first);
    assert_eq!(
        f.store.link_count(),
        links,
        "repeated identical MUX32 materialized new Links"
    );
}
