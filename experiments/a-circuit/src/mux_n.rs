use super::{
    full_adder::{
        call, define_bundle_rule, index_rule_for, Fixture as FullFixture,
    },
    logic_n::{install_gate_basis, GateSet},
    proof_n::{
        execute_session_to_quiescence, execute_to_quiescence,
        identical_rerun, load_runtime, load_runtime_session, loaded_handle,
        prepare_stage, semantic_source, theory_admissions, visual_snapshot,
        WebMux1Proof, WebProofResultStage, WebStructuralProof,
    },
};
use amemory_optimized_cpu_probe::{
    structural::{
        define_structural_interpreter, define_structural_role_dictionary,
        materialize_exact_sequence, read_exact_sequence,
    },
    Handle, OptimizedLinkStore, ROOT_HANDLE,
};
use std::collections::HashSet;

const WIDTH: usize = 32;

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


#[derive(Clone, Debug)]
pub(crate) struct WebMux32ProofExecution {
    pub(crate) outcome: WebMuxOutcome,
    pub(crate) proof: WebStructuralProof,
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

pub(crate) fn web_prove_mux32(
    select: u32,
    a: u32,
    b: u32,
) -> Option<WebMux32ProofExecution> {
    if select > 1 {
        return None;
    }

    let mut compiler = FullFixture::new();
    let program = MuxProgram::install(&mut compiler);
    let links_after_build = program.links_after_build as u32;
    let bits = [compiler.zero, compiler.one];

    let a_bits = bit_handles(&compiler, a);
    let b_bits = bit_handles(&compiler, b);
    let aword =
        materialize_exact_sequence(&mut compiler.store, &a_bits).ok()?;
    let bword =
        materialize_exact_sequence(&mut compiler.store, &b_bits).ok()?;
    let select_handle = bits[select as usize];
    let args = materialize_exact_sequence(
        &mut compiler.store,
        &[select_handle, aword, bword],
    )
    .ok()?;
    let invocation =
        call(&mut compiler.store, compiler.apply, program.mux32, args);
    let initial = compiler
        .store
        .ensure_pair(compiler.k, invocation)
        .ok()?;

    let prepared_roots = vec![
        semantic_source(&compiler.store, "function.mux32", program.mux32),
        semantic_source(&compiler.store, "function.mux1", program.mux1),
        semantic_source(
            &compiler.store,
            "function.gate.xor2",
            program.gates.xor2,
        ),
        semantic_source(
            &compiler.store,
            "function.gate.and2",
            program.gates.and2,
        ),
        semantic_source(&compiler.store, "data.select", select_handle),
        semantic_source(&compiler.store, "data.a.word", aword),
        semantic_source(&compiler.store, "data.b.word", bword),
        semantic_source(&compiler.store, "data.bit.zero", compiler.zero),
        semantic_source(&compiler.store, "data.bit.one", compiler.one),
        semantic_source(
            &compiler.store,
            "execution.interpreter",
            compiler.interpreter,
        ),
        semantic_source(
            &compiler.store,
            "execution.theory",
            compiler.theory,
        ),
        semantic_source(
            &compiler.store,
            "execution.apply",
            compiler.apply,
        ),
        semantic_source(&compiler.store, "invocation.args", args),
        semantic_source(&compiler.store, "invocation.call", invocation),
        semantic_source(&compiler.store, "scope.initial", initial),
        semantic_source(&compiler.store, "context.caller", compiler.k),
        semantic_source(
            &compiler.store,
            "result.word_tag",
            program.word_result_tag,
        ),
        semantic_source(
            &compiler.store,
            "result.bit_tag",
            program.bit_result_tag,
        ),
    ];

    let admissions =
        theory_admissions(&compiler.store, compiler.theory)?;
    let prepare =
        prepare_stage(&compiler.store, prepared_roots, admissions);
    let (mut memory, load) = load_runtime(&prepare)?;

    let interpreter = loaded_handle(&load, "execution.interpreter")?;
    let initial = loaded_handle(&load, "scope.initial")?;
    let caller = loaded_handle(&load, "context.caller")?;
    let result_tag = loaded_handle(&load, "result.word_tag")?;
    let zero = loaded_handle(&load, "data.bit.zero")?;
    let one = loaded_handle(&load, "data.bit.one")?;

    let (mut engine, execute) = execute_to_quiescence(
        &mut memory,
        interpreter,
        initial,
        32,
        program.mux32_steps as u32 + 2,
    )?;
    if execute.active_reaction_count != program.mux32_steps as u32
        || engine.current().len() != 1
    {
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
    let word_bits = read_exact_sequence(&memory.store, values[0]).ok()?;
    if word_bits.len() != WIDTH {
        return None;
    }
    let mut decoded_value = 0u32;
    for (index, bit) in word_bits.into_iter().enumerate() {
        let value = if bit == one {
            1u32
        } else if bit == zero {
            0u32
        } else {
            return None;
        };
        decoded_value |= value << index;
    }
    let oracle_value = if select == 0 { a } else { b };

    let result_anum = memory.store.export_anum(final_link).ok()?;
    let result_sequence_anum = memory.store.export_anum(payload).ok()?;
    let links_after_first = memory.store.link_count() as u32;
    let identical_rerun_link_delta = identical_rerun(
        &mut memory,
        &mut engine,
        initial,
        &result_anum,
        program.mux32_steps as u32 + 2,
    )?;
    let visual_links =
        visual_snapshot(&memory, &load.semantic_roots);

    let result = WebProofResultStage {
        memory_instance_id: memory.id.clone(),
        result_anum,
        result_sequence_anum,
        decoded_value,
        decoded_value_hi: None,
        oracle_value,
        oracle_value_hi: None,
        oracle_matches: decoded_value == oracle_value,
        links_final: memory.store.link_count() as u32,
        identical_rerun_link_delta,
        visual_links,
    };
    let proof = WebStructuralProof {
        schema_version: 4,
        block: "MUX32".to_owned(),
        prepare,
        load,
        execute,
        result,
    };
    let outcome = WebMuxOutcome {
        value: decoded_value,
        reactions: proof.execute.active_reaction_count,
        links_after_build,
        links_after_first,
        steady_link_delta: proof.result.identical_rerun_link_delta,
        quiescent: u8::from(proof.execute.final_quiescent),
    };

    Some(WebMux32ProofExecution { outcome, proof })
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



#[derive(Debug)]
struct ProofMuxFixture {
    store: OptimizedLinkStore,
    mux1: Handle,
    xor2: Handle,
    and2: Handle,
    k: Handle,
    zero: Handle,
    one: Handle,
    apply: Handle,
    theory: Handle,
    interpreter: Handle,
    bit_outputs: [Handle; 2],
    bit_result_tag: Handle,
}

fn proof_binary_call(
    store: &mut OptimizedLinkStore,
    apply: Handle,
    function: Handle,
    a: Handle,
    b: Handle,
) -> Handle {
    let args = materialize_exact_sequence(store, &[a, b]).unwrap();
    call(store, apply, function, args)
}

fn proof_fresh_pair(
    store: &mut OptimizedLinkStore,
    anchors: &mut AnchorGen,
) -> Handle {
    let left = anchors.next(store);
    let right = anchors.next(store);
    store.ensure_pair(left, right).unwrap()
}

fn install_proof_gate(
    store: &mut OptimizedLinkStore,
    anchors: &mut AnchorGen,
    theory: Handle,
    apply: Handle,
    trigger: Handle,
    function: Handle,
    bits: [Handle; 2],
    bit_outputs: [Handle; 2],
    rows: &[(usize, usize, usize)],
) {
    for &(a, b, out) in rows {
        let caller = anchors.next(store);
        let args = materialize_exact_sequence(store, &[bits[a], bits[b]]).unwrap();
        let invocation = call(store, apply, function, args);
        let before = store.ensure_pair(caller, invocation).unwrap();
        let after = store.ensure_pair(caller, bit_outputs[out]).unwrap();

        let (_, admission) = define_bundle_rule(
            store,
            theory,
            &[caller],
            before,
            &[after],
        );
        index_rule_for(store, &[trigger], admission);
    }
}

fn build_proof_mux_fixture() -> ProofMuxFixture {
    let mut store = OptimizedLinkStore::new();

    let o = store.ensure_start_self_closed(ROOT_HANDLE).unwrap();
    let c = store.ensure_end_self_closed(ROOT_HANDLE).unwrap();
    let l = store.ensure_pair(o, c).unwrap();
    let u = store.ensure_pair(c, o).unwrap();

    // Small deterministic namespace dedicated to the portable MUX1 proof.
    let seed = store.ensure_pair(u, l).unwrap();
    let mut anchors = AnchorGen::new(&mut store, seed, o, c);

    let theory = proof_fresh_pair(&mut store, &mut anchors);
    let authority_dictionary =
        define_structural_role_dictionary(&mut store, &[]).unwrap();
    let grammar = proof_fresh_pair(&mut store, &mut anchors);
    let interpreter = define_structural_interpreter(
        &mut store,
        authority_dictionary,
        grammar,
        theory,
    )
    .unwrap();

    let xor2 = proof_fresh_pair(&mut store, &mut anchors);
    let and2 = proof_fresh_pair(&mut store, &mut anchors);
    let mux1 = proof_fresh_pair(&mut store, &mut anchors);
    let k = proof_fresh_pair(&mut store, &mut anchors);
    let xor_ab_tag = anchors.next(&mut store);
    let and_s_tag = anchors.next(&mut store);
    let xor_out_tag = anchors.next(&mut store);
    let bit_result_tag = proof_fresh_pair(&mut store, &mut anchors);

    let apply = o;
    let zero = u;
    let one = l;
    let bits = [zero, one];
    let bit_outputs = [
        materialize_exact_sequence(&mut store, &[zero]).unwrap(),
        materialize_exact_sequence(&mut store, &[one]).unwrap(),
    ];

    // Gate truth tables are themselves Structural Rules in Theory.
    install_proof_gate(
        &mut store,
        &mut anchors,
        theory,
        apply,
        o,
        xor2,
        bits,
        bit_outputs,
        &[(0, 0, 0), (0, 1, 1), (1, 0, 1), (1, 1, 0)],
    );
    install_proof_gate(
        &mut store,
        &mut anchors,
        theory,
        apply,
        o,
        and2,
        bits,
        bit_outputs,
        &[(0, 0, 0), (0, 1, 0), (1, 0, 0), (1, 1, 1)],
    );

    // MUX1 = A XOR ((A XOR B) AND S), expressed only as structural calls/rules.
    {
        let k_role = anchors.next(&mut store);
        let s_role = anchors.next(&mut store);
        let a_role = anchors.next(&mut store);
        let b_role = anchors.next(&mut store);

        let args =
            materialize_exact_sequence(&mut store, &[s_role, a_role, b_role]).unwrap();
        let invocation = call(&mut store, apply, mux1, args);
        let before = store.ensure_pair(k_role, invocation).unwrap();

        let caller = stage_frame(
            &mut store,
            xor_ab_tag,
            &[k_role, s_role, a_role],
        );
        let xor_call =
            proof_binary_call(&mut store, apply, xor2, a_role, b_role);
        let after = store.ensure_pair(caller, xor_call).unwrap();

        let (_, admission) = define_bundle_rule(
            &mut store,
            theory,
            &[k_role, s_role, a_role, b_role],
            before,
            &[after],
        );
        index_rule_for(&mut store, &[o], admission);
    }

    {
        let k_role = anchors.next(&mut store);
        let s_role = anchors.next(&mut store);
        let a_role = anchors.next(&mut store);
        let x_role = anchors.next(&mut store);

        let caller = stage_frame(
            &mut store,
            xor_ab_tag,
            &[k_role, s_role, a_role],
        );
        let x_result =
            materialize_exact_sequence(&mut store, &[x_role]).unwrap();
        let before = store.ensure_pair(caller, x_result).unwrap();

        let next_caller =
            stage_frame(&mut store, and_s_tag, &[k_role, a_role]);
        let and_call =
            proof_binary_call(&mut store, apply, and2, s_role, x_role);
        let after = store.ensure_pair(next_caller, and_call).unwrap();

        let (_, admission) = define_bundle_rule(
            &mut store,
            theory,
            &[k_role, s_role, a_role, x_role],
            before,
            &[after],
        );
        index_rule_for(&mut store, &bit_outputs, admission);
    }

    {
        let k_role = anchors.next(&mut store);
        let a_role = anchors.next(&mut store);
        let y_role = anchors.next(&mut store);

        let caller = stage_frame(&mut store, and_s_tag, &[k_role, a_role]);
        let y_result =
            materialize_exact_sequence(&mut store, &[y_role]).unwrap();
        let before = store.ensure_pair(caller, y_result).unwrap();

        let next_caller =
            stage_frame(&mut store, xor_out_tag, &[k_role]);
        let xor_call =
            proof_binary_call(&mut store, apply, xor2, a_role, y_role);
        let after = store.ensure_pair(next_caller, xor_call).unwrap();

        let (_, admission) = define_bundle_rule(
            &mut store,
            theory,
            &[k_role, a_role, y_role],
            before,
            &[after],
        );
        index_rule_for(&mut store, &bit_outputs, admission);
    }

    {
        let k_role = anchors.next(&mut store);
        let out_role = anchors.next(&mut store);

        let caller = stage_frame(&mut store, xor_out_tag, &[k_role]);
        let out_result =
            materialize_exact_sequence(&mut store, &[out_role]).unwrap();
        let before = store.ensure_pair(caller, out_result).unwrap();

        let payload =
            materialize_exact_sequence(&mut store, &[out_role]).unwrap();
        let endpoint = store.ensure_pair(bit_result_tag, payload).unwrap();
        let after = store.ensure_pair(k_role, endpoint).unwrap();

        let (_, admission) = define_bundle_rule(
            &mut store,
            theory,
            &[k_role, out_role],
            before,
            &[after],
        );
        index_rule_for(&mut store, &bit_outputs, admission);
    }

    ProofMuxFixture {
        store,
        mux1,
        xor2,
        and2,
        k,
        zero,
        one,
        apply,
        theory,
        interpreter,
        bit_outputs,
        bit_result_tag,
    }
}

pub(crate) fn web_prove_mux1(
    select: u32,
    a: u32,
    b: u32,
) -> Option<WebMux1Proof> {
    if select > 1 || a > 1 || b > 1 {
        return None;
    }

    // Stage 1: prepare a complete portable image. This compiler store is not
    // the runtime A-memory and is discarded as execution authority.
    let mut compiler = build_proof_mux_fixture();
    let bits = [compiler.zero, compiler.one];
    let args = materialize_exact_sequence(
        &mut compiler.store,
        &[
            bits[select as usize],
            bits[a as usize],
            bits[b as usize],
        ],
    )
    .ok()?;
    let invocation = call(
        &mut compiler.store,
        compiler.apply,
        compiler.mux1,
        args,
    );
    let initial = compiler
        .store
        .ensure_pair(compiler.k, invocation)
        .ok()?;

    let prepared_roots = vec![
        semantic_source(&compiler.store, "function.mux1", compiler.mux1),
        semantic_source(
            &compiler.store,
            "function.dependency.xor2",
            compiler.xor2,
        ),
        semantic_source(
            &compiler.store,
            "function.dependency.and2",
            compiler.and2,
        ),
        semantic_source(
            &compiler.store,
            "data.select",
            bits[select as usize],
        ),
        semantic_source(&compiler.store, "data.a", bits[a as usize]),
        semantic_source(&compiler.store, "data.b", bits[b as usize]),
        semantic_source(&compiler.store, "data.zero", compiler.zero),
        semantic_source(&compiler.store, "data.one", compiler.one),
        semantic_source(
            &compiler.store,
            "execution.interpreter",
            compiler.interpreter,
        ),
        semantic_source(
            &compiler.store,
            "execution.theory",
            compiler.theory,
        ),
        semantic_source(
            &compiler.store,
            "execution.apply",
            compiler.apply,
        ),
        semantic_source(&compiler.store, "invocation.args", args),
        semantic_source(
            &compiler.store,
            "invocation.call",
            invocation,
        ),
        semantic_source(&compiler.store, "scope.initial", initial),
        semantic_source(
            &compiler.store,
            "context.caller",
            compiler.k,
        ),
        semantic_source(
            &compiler.store,
            "result.tag",
            compiler.bit_result_tag,
        ),
        semantic_source(
            &compiler.store,
            "result.zero",
            compiler.bit_outputs[0],
        ),
        semantic_source(
            &compiler.store,
            "result.one",
            compiler.bit_outputs[1],
        ),
    ];

    let admissions =
        theory_admissions(&compiler.store, compiler.theory)?;
    let prepare = prepare_stage(
        &compiler.store,
        prepared_roots,
        admissions,
    );

    // Stage 2: create exactly one runtime A-memory and import the portable Aset.
    let (mut memory, load) = load_runtime(&prepare)?;

    let interpreter = loaded_handle(&load, "execution.interpreter")?;
    let initial = loaded_handle(&load, "scope.initial")?;
    let caller = loaded_handle(&load, "context.caller")?;
    let result_tag = loaded_handle(&load, "result.tag")?;
    let zero = loaded_handle(&load, "data.zero")?;
    let one = loaded_handle(&load, "data.one")?;

    // Stage 3: execute the generic structural engine in that same runtime memory.
    let (mut engine, execute) = execute_to_quiescence(
        &mut memory,
        interpreter,
        initial,
        32,
        64,
    )?;

    // Stage 4: decode the result from the same store. Host MUX arithmetic is
    // independent oracle only, never runtime authority.
    if engine.current().len() != 1 {
        return None;
    }
    let final_link = engine.current()[0];
    let (final_caller, endpoint) =
        memory.store.poles(final_link).ok()?;
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
    let oracle_value = if select == 0 { a } else { b };

    let result_anum = memory.store.export_anum(final_link).ok()?;
    let result_sequence_anum =
        memory.store.export_anum(payload).ok()?;

    let identical_rerun_link_delta = identical_rerun(
        &mut memory,
        &mut engine,
        initial,
        &result_anum,
        64,
    )?;

    let visual_links =
        visual_snapshot(&memory, &load.semantic_roots);

    let result = WebProofResultStage {
        memory_instance_id: memory.id.clone(),
        result_anum,
        result_sequence_anum,
        decoded_value,
        decoded_value_hi: None,
        oracle_value,
        oracle_value_hi: None,
        oracle_matches: decoded_value == oracle_value,
        links_final: memory.store.link_count() as u32,
        identical_rerun_link_delta,
        visual_links,
    };

    Some(WebStructuralProof {
        schema_version: 4,
        block: "MUX1".to_owned(),
        prepare,
        load,
        execute,
        result,
    })
}

#[test]
fn web_mux1_lifecycle_reconfigures_four_runs_in_one_loaded_memory() {
    // PREPARE the static program/Theory only. No concrete invocation/current
    // Link is part of the packed image: every input configuration below is
    // published after LOAD into this one runtime store.
    let compiler = build_proof_mux_fixture();
    let prepared_roots = vec![
        semantic_source(&compiler.store, "function.mux1", compiler.mux1),
        semantic_source(&compiler.store, "data.zero", compiler.zero),
        semantic_source(&compiler.store, "data.one", compiler.one),
        semantic_source(
            &compiler.store,
            "execution.interpreter",
            compiler.interpreter,
        ),
        semantic_source(
            &compiler.store,
            "execution.theory",
            compiler.theory,
        ),
        semantic_source(
            &compiler.store,
            "execution.apply",
            compiler.apply,
        ),
        semantic_source(&compiler.store, "context.caller", compiler.k),
        semantic_source(
            &compiler.store,
            "result.tag",
            compiler.bit_result_tag,
        ),
    ];
    let admissions =
        theory_admissions(&compiler.store, compiler.theory).unwrap();
    let prepare =
        prepare_stage(&compiler.store, prepared_roots, admissions);

    // LOAD exactly once and create one engine owned by the runtime Session.
    let (mut session, load) =
        load_runtime_session(&prepare, 32).unwrap();
    let runtime_id = session.memory.id.clone();
    let store_address =
        std::ptr::addr_of!(session.memory.store) as usize;
    let engine_address =
        std::ptr::addr_of!(session.engine) as usize;
    let loaded_link_count = session.base_link_count;
    let loaded_prefix = session.memory.store.export_packed_duplets();
    assert_eq!(loaded_prefix.len(), loaded_link_count);
    assert_eq!(
        loaded_link_count,
        load.links_after_load as usize,
        "Session base boundary must equal the one-time LOAD boundary",
    );

    let mux1 = loaded_handle(&load, "function.mux1").unwrap();
    let apply = loaded_handle(&load, "execution.apply").unwrap();
    let caller = loaded_handle(&load, "context.caller").unwrap();
    let result_tag = loaded_handle(&load, "result.tag").unwrap();
    let zero = loaded_handle(&load, "data.zero").unwrap();
    let one = loaded_handle(&load, "data.one").unwrap();
    let bits = [zero, one];

    let vectors = [
        (0usize, 0usize, 1usize, 0u32),
        (1usize, 0usize, 1usize, 1u32),
        (0usize, 1usize, 0usize, 1u32),
        (0usize, 0usize, 1usize, 0u32),
    ];
    let mut first_initial = None;
    let mut first_result_wire = None;

    for (run_index, &(select, a, b, expected)) in
        vectors.iter().enumerate()
    {
        assert_eq!(session.memory.id, runtime_id);
        assert_eq!(
            std::ptr::addr_of!(session.memory.store) as usize,
            store_address,
            "run {run_index} replaced the runtime store object",
        );
        assert_eq!(
            std::ptr::addr_of!(session.engine) as usize,
            engine_address,
            "run {run_index} replaced the persistent executor",
        );

        // CONFIGURE: publish this run's input as normal immutable Links in the
        // already-loaded A-memory. No old Link is rewritten.
        let before_config = session.memory.store.link_count();
        let args = materialize_exact_sequence(
            &mut session.memory.store,
            &[bits[select], bits[a], bits[b]],
        )
        .unwrap();
        let invocation =
            call(&mut session.memory.store, apply, mux1, args);
        let initial = session
            .memory
            .store
            .ensure_pair(caller, invocation)
            .unwrap();
        let after_config = session.memory.store.link_count();

        if run_index < 3 {
            assert!(
                after_config > before_config,
                "new input vector must publish runtime Links after LOAD",
            );
        } else {
            assert_eq!(
                Some(initial),
                first_initial,
                "returning to the first input must reuse its canonical Link",
            );
        }
        if run_index == 0 {
            first_initial = Some(initial);
        }

        // EXECUTE: common Session API owns the same engine and invokes only
        // the generic generalized-MP engine.run() loop until quiescence.
        let execute =
            execute_session_to_quiescence(&mut session, initial, 64).unwrap();
        assert!(execute.final_quiescent);
        assert_eq!(
            execute.active_reaction_count, 7,
            "run {run_index} changed the proven MUX1 reaction count",
        );
        assert_eq!(session.engine.current().len(), 1);

        // RESULT projection is read-only over the same runtime carrier.
        let final_link = session.engine.current()[0];
        let (final_caller, endpoint) =
            session.memory.store.poles(final_link).unwrap();
        assert_eq!(final_caller, caller);
        let (tag, payload) = session.memory.store.poles(endpoint).unwrap();
        assert_eq!(tag, result_tag);
        let values =
            read_exact_sequence(&session.memory.store, payload).unwrap();
        assert_eq!(values.len(), 1);
        let decoded = if values[0] == one {
            1
        } else if values[0] == zero {
            0
        } else {
            panic!("run {run_index} returned a non-bit MUX1 result");
        };
        assert_eq!(decoded, expected);

        // Fresh-instance execution is an oracle only. It cannot influence
        // this Session's input, matching or result.
        let fresh =
            web_prove_mux1(select as u32, a as u32, b as u32).unwrap();
        assert_ne!(fresh.load.memory_instance_id, runtime_id);
        assert_eq!(fresh.result.decoded_value, decoded);

        let carrier = session.memory.store.export_packed_duplets();
        assert_eq!(
            &carrier[..loaded_link_count],
            loaded_prefix.as_slice(),
            "run {run_index} mutated the originally loaded Aset prefix",
        );
        assert_eq!(session.memory.id, runtime_id);
        assert_eq!(
            std::ptr::addr_of!(session.memory.store) as usize,
            store_address,
            "run {run_index} replaced the runtime store object",
        );
        assert_eq!(
            std::ptr::addr_of!(session.engine) as usize,
            engine_address,
            "run {run_index} replaced the persistent executor",
        );

        let result_wire =
            session.memory.store.export_anum(final_link).unwrap();
        if run_index == 0 {
            first_result_wire = Some(result_wire);
        } else if run_index == 3 {
            assert_eq!(
                Some(result_wire),
                first_result_wire,
                "returning to the first configuration changed its semantic result",
            );
        }
    }
}

#[test]
fn web_mux1_proof_uses_one_runtime_memory_for_all_eight_cases() {
    for select in 0..=1 {
        for a in 0..=1 {
            for b in 0..=1 {
                let proof = web_prove_mux1(select, a, b).expect("MUX1 proof");
                assert!(!proof.prepare.runtime_memory_exists);
                assert!(proof.load.carrier_round_trip);
                assert!(!proof.prepare.theory_admissions.is_empty());
                assert_eq!(proof.load.links_before_load, 1);
                assert_eq!(proof.load.links_after_load, proof.prepare.compiled_links);
                assert!(proof.load.links_after_load > proof.load.links_before_load);
                assert_eq!(proof.execute.active_reaction_count, 7);
                assert!(proof.execute.final_quiescent);
                assert!(proof.result.oracle_matches);
                assert_eq!(proof.result.identical_rerun_link_delta, 0);

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
                assert_eq!(
                    proof.result.visual_links.len(),
                    proof.result.links_final as usize,
                    "visual snapshot must contain every runtime Link",
                );

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
