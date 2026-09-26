use super::full_adder::{
    call, define_bundle_rule, index_rule_for, Fixture as FullFixture,
};
use amemory_optimized_cpu_probe::{
    structural::{materialize_exact_sequence, read_exact_sequence},
    Handle, OptimizedLinkStore,
};

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
        width: usize,
    ) -> Self {
        // Width-separated namespace, distinct from M3 arithmetic/flags.
        seed = store.ensure_pair(seed, o).unwrap();
        seed = store.ensure_pair(seed, o).unwrap();
        seed = store.ensure_pair(seed, c).unwrap();
        for _ in 0..width {
            seed = store.ensure_pair(seed, c).unwrap();
            seed = store.ensure_pair(seed, c).unwrap();
            seed = store.ensure_pair(seed, o).unwrap();
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

#[derive(Clone, Copy, Debug)]
pub(crate) struct GateSet {
    pub(crate) and2: Handle,
    pub(crate) or2: Handle,
    pub(crate) xor2: Handle,
    pub(crate) not1: Handle,
    pub(crate) bit_outputs: [Handle; 2],
}

fn install_binary_gate(
    f: &mut FullFixture,
    anchors: &mut AnchorGen,
    function: Handle,
    bit_outputs: [Handle; 2],
    rows: &[(u8, u8, u8)],
) {
    let bits = [f.zero, f.one];

    for &(a, b, out) in rows {
        let caller = anchors.next(&mut f.store);
        let args = materialize_exact_sequence(
            &mut f.store,
            &[bits[a as usize], bits[b as usize]],
        )
        .unwrap();
        let invocation = call(&mut f.store, f.apply, function, args);
        let before = f.store.ensure_pair(caller, invocation).unwrap();
        let after = f
            .store
            .ensure_pair(caller, bit_outputs[out as usize])
            .unwrap();

        let (_, admission) = define_bundle_rule(
            &mut f.store,
            f.theory,
            &[caller],
            before,
            &[after],
        );
        index_rule_for(&mut f.store, &[f.o], admission);
    }
}

fn install_gates(
    f: &mut FullFixture,
    anchors: &mut AnchorGen,
) -> GateSet {
    let and_left = anchors.next(&mut f.store);
    let and_right = anchors.next(&mut f.store);
    let and2 = f.store.ensure_pair(and_left, and_right).unwrap();

    let or_left = anchors.next(&mut f.store);
    let or_right = anchors.next(&mut f.store);
    let or2 = f.store.ensure_pair(or_left, or_right).unwrap();

    let xor_left = anchors.next(&mut f.store);
    let xor_right = anchors.next(&mut f.store);
    let xor2 = f.store.ensure_pair(xor_left, xor_right).unwrap();

    let not_left = anchors.next(&mut f.store);
    let not_right = anchors.next(&mut f.store);
    let not1 = f.store.ensure_pair(not_left, not_right).unwrap();

    let bit_outputs = [
        materialize_exact_sequence(&mut f.store, &[f.zero]).unwrap(),
        materialize_exact_sequence(&mut f.store, &[f.one]).unwrap(),
    ];

    install_binary_gate(
        f,
        anchors,
        and2,
        bit_outputs,
        &[(0, 0, 0), (0, 1, 0), (1, 0, 0), (1, 1, 1)],
    );
    install_binary_gate(
        f,
        anchors,
        or2,
        bit_outputs,
        &[(0, 0, 0), (0, 1, 1), (1, 0, 1), (1, 1, 1)],
    );
    install_binary_gate(
        f,
        anchors,
        xor2,
        bit_outputs,
        &[(0, 0, 0), (0, 1, 1), (1, 0, 1), (1, 1, 0)],
    );

    let bits = [f.zero, f.one];
    for (input, output) in [(0usize, 1usize), (1usize, 0usize)] {
        let caller = anchors.next(&mut f.store);
        let args =
            materialize_exact_sequence(&mut f.store, &[bits[input]]).unwrap();
        let invocation = call(&mut f.store, f.apply, not1, args);
        let before = f.store.ensure_pair(caller, invocation).unwrap();
        let after = f
            .store
            .ensure_pair(caller, bit_outputs[output])
            .unwrap();

        let (_, admission) = define_bundle_rule(
            &mut f.store,
            f.theory,
            &[caller],
            before,
            &[after],
        );
        index_rule_for(&mut f.store, &[f.o], admission);
    }

    GateSet {
        and2,
        or2,
        xor2,
        not1,
        bit_outputs,
    }
}

#[derive(Clone, Debug)]
pub(crate) struct LogicProgram {
    pub(crate) width: usize,
    pub(crate) word_binary: Handle,
    pub(crate) word_not: Handle,
    pub(crate) result_tag: Handle,
    pub(crate) gates: GateSet,
    pub(crate) binary_steps: usize,
    pub(crate) unary_steps: usize,
    pub(crate) links_after_build: usize,
}

impl LogicProgram {
    pub(crate) fn install(f: &mut FullFixture, width: usize) -> Self {
        assert!((8..=32).contains(&width));

        let seed0 = f.store.ensure_pair(f.k, f.full).unwrap();
        let mut anchors = AnchorGen::new(
            &mut f.store,
            seed0,
            f.o,
            f.c,
            width,
        );

        let gates = install_gates(f, &mut anchors);

        let binary_left = anchors.next(&mut f.store);
        let binary_right = anchors.next(&mut f.store);
        let word_binary =
            f.store.ensure_pair(binary_left, binary_right).unwrap();

        let not_left = anchors.next(&mut f.store);
        let not_right = anchors.next(&mut f.store);
        let word_not =
            f.store.ensure_pair(not_left, not_right).unwrap();

        let result_left = anchors.next(&mut f.store);
        let result_right = anchors.next(&mut f.store);
        let result_tag =
            f.store.ensure_pair(result_left, result_right).unwrap();

        let mut binary_tags = Vec::with_capacity(width);
        let mut unary_tags = Vec::with_capacity(width);
        for _ in 0..width {
            let left = anchors.next(&mut f.store);
            let right = anchors.next(&mut f.store);
            binary_tags.push(f.store.ensure_pair(left, right).unwrap());

            let left = anchors.next(&mut f.store);
            let right = anchors.next(&mut f.store);
            unary_tags.push(f.store.ensure_pair(left, right).unwrap());
        }

        // -------------------------------------------------------------
        // Generic WORD_BIN OPEN.
        //
        // K -> Call(WORD_BIN,[Gate,Aword,Bword])
        // =>
        // Stage0([K,Gate,A_remaining,B_remaining])
        //   -> Call(Gate,[a0,b0])
        //
        // Gate is a role/value, so one controller executes AND2/OR2/XOR2
        // without host dispatch or an ALU opcode table.
        // -------------------------------------------------------------
        {
            let k = anchors.next(&mut f.store);
            let gate = anchors.next(&mut f.store);
            let a_roles = anchors.roles(&mut f.store, width);
            let b_roles = anchors.roles(&mut f.store, width);

            let aword =
                materialize_exact_sequence(&mut f.store, &a_roles).unwrap();
            let bword =
                materialize_exact_sequence(&mut f.store, &b_roles).unwrap();
            let args = materialize_exact_sequence(
                &mut f.store,
                &[gate, aword, bword],
            )
            .unwrap();
            let invocation =
                call(&mut f.store, f.apply, word_binary, args);
            let before = f.store.ensure_pair(k, invocation).unwrap();

            let mut state = Vec::with_capacity(2 * width - 1 + 2);
            state.push(k);
            state.push(gate);
            state.extend_from_slice(&a_roles[1..]);
            state.extend_from_slice(&b_roles[1..]);

            let caller =
                stage_frame(&mut f.store, binary_tags[0], &state);
            let gate_args = materialize_exact_sequence(
                &mut f.store,
                &[a_roles[0], b_roles[0]],
            )
            .unwrap();
            let gate_call =
                call(&mut f.store, f.apply, gate, gate_args);
            let after = f.store.ensure_pair(caller, gate_call).unwrap();

            let mut roles = Vec::with_capacity(2 * width + 2);
            roles.push(k);
            roles.push(gate);
            roles.extend_from_slice(&a_roles);
            roles.extend_from_slice(&b_roles);

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        // One generic continuation per bit position for all binary gates.
        for i in 0..width {
            let k = anchors.next(&mut f.store);
            let gate = anchors.next(&mut f.store);
            let remaining = width - i - 1;
            let a_rem = anchors.roles(&mut f.store, remaining);
            let b_rem = anchors.roles(&mut f.store, remaining);
            let previous = anchors.roles(&mut f.store, i);
            let out = anchors.next(&mut f.store);

            let mut state =
                Vec::with_capacity(2 + 2 * remaining + i);
            state.push(k);
            state.push(gate);
            state.extend_from_slice(&a_rem);
            state.extend_from_slice(&b_rem);
            state.extend_from_slice(&previous);

            let caller =
                stage_frame(&mut f.store, binary_tags[i], &state);
            let gate_result =
                materialize_exact_sequence(&mut f.store, &[out]).unwrap();
            let before =
                f.store.ensure_pair(caller, gate_result).unwrap();

            let after = if i + 1 < width {
                let mut next_previous = previous.clone();
                next_previous.push(out);

                let mut next_state =
                    Vec::with_capacity(2 + 2 * (remaining - 1) + i + 1);
                next_state.push(k);
                next_state.push(gate);
                next_state.extend_from_slice(&a_rem[1..]);
                next_state.extend_from_slice(&b_rem[1..]);
                next_state.extend_from_slice(&next_previous);

                let next_caller = stage_frame(
                    &mut f.store,
                    binary_tags[i + 1],
                    &next_state,
                );
                let next_args = materialize_exact_sequence(
                    &mut f.store,
                    &[a_rem[0], b_rem[0]],
                )
                .unwrap();
                let next_call =
                    call(&mut f.store, f.apply, gate, next_args);
                f.store.ensure_pair(next_caller, next_call).unwrap()
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
                let envelope =
                    f.store.ensure_pair(result_tag, payload).unwrap();
                f.store.ensure_pair(k, envelope).unwrap()
            };

            let mut roles =
                Vec::with_capacity(3 + 2 * remaining + i);
            roles.push(k);
            roles.push(gate);
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
                &gates.bit_outputs,
                admission,
            );
        }

        // -------------------------------------------------------------
        // Generic WORD_NOT OPEN.
        // -------------------------------------------------------------
        {
            let k = anchors.next(&mut f.store);
            let a_roles = anchors.roles(&mut f.store, width);

            let aword =
                materialize_exact_sequence(&mut f.store, &a_roles).unwrap();
            let args =
                materialize_exact_sequence(&mut f.store, &[aword]).unwrap();
            let invocation =
                call(&mut f.store, f.apply, word_not, args);
            let before = f.store.ensure_pair(k, invocation).unwrap();

            let mut state = Vec::with_capacity(width);
            state.push(k);
            state.extend_from_slice(&a_roles[1..]);

            let caller =
                stage_frame(&mut f.store, unary_tags[0], &state);
            let not_args = materialize_exact_sequence(
                &mut f.store,
                &[a_roles[0]],
            )
            .unwrap();
            let not_call =
                call(&mut f.store, f.apply, gates.not1, not_args);
            let after = f.store.ensure_pair(caller, not_call).unwrap();

            let mut roles = Vec::with_capacity(width + 1);
            roles.push(k);
            roles.extend_from_slice(&a_roles);

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        // One generic continuation per bit for WORD_NOT.
        for i in 0..width {
            let k = anchors.next(&mut f.store);
            let remaining = width - i - 1;
            let a_rem = anchors.roles(&mut f.store, remaining);
            let previous = anchors.roles(&mut f.store, i);
            let out = anchors.next(&mut f.store);

            let mut state = Vec::with_capacity(1 + remaining + i);
            state.push(k);
            state.extend_from_slice(&a_rem);
            state.extend_from_slice(&previous);

            let caller =
                stage_frame(&mut f.store, unary_tags[i], &state);
            let not_result =
                materialize_exact_sequence(&mut f.store, &[out]).unwrap();
            let before =
                f.store.ensure_pair(caller, not_result).unwrap();

            let after = if i + 1 < width {
                let mut next_previous = previous.clone();
                next_previous.push(out);

                let mut next_state =
                    Vec::with_capacity(1 + (remaining - 1) + i + 1);
                next_state.push(k);
                next_state.extend_from_slice(&a_rem[1..]);
                next_state.extend_from_slice(&next_previous);

                let next_caller = stage_frame(
                    &mut f.store,
                    unary_tags[i + 1],
                    &next_state,
                );
                let next_args =
                    materialize_exact_sequence(&mut f.store, &[a_rem[0]])
                        .unwrap();
                let next_call =
                    call(&mut f.store, f.apply, gates.not1, next_args);
                f.store.ensure_pair(next_caller, next_call).unwrap()
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
                let envelope =
                    f.store.ensure_pair(result_tag, payload).unwrap();
                f.store.ensure_pair(k, envelope).unwrap()
            };

            let mut roles = Vec::with_capacity(2 + remaining + i);
            roles.push(k);
            roles.extend_from_slice(&a_rem);
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
                &gates.bit_outputs,
                admission,
            );
        }

        let binary_steps = 1 + 2 * width;
        let unary_steps = 1 + 2 * width;

        Self {
            width,
            word_binary,
            word_not,
            result_tag,
            gates,
            binary_steps,
            unary_steps,
            links_after_build: f.store.link_count(),
        }
    }
}

fn mask(width: usize) -> u32 {
    if width == 32 {
        u32::MAX
    } else {
        (1u32 << width) - 1
    }
}

fn bit_handles(
    f: &FullFixture,
    width: usize,
    value: u32,
) -> Vec<Handle> {
    (0..width)
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
    width: usize,
    word: Handle,
) -> u32 {
    let bits = read_exact_sequence(&f.store, word).unwrap();
    assert_eq!(bits.len(), width);

    let mut value = 0u32;
    for (index, bit) in bits.into_iter().enumerate() {
        if bit == f.one {
            value |= 1u32 << index;
        } else {
            assert_eq!(bit, f.zero);
        }
    }
    value
}

fn decode_logic_result(
    f: &FullFixture,
    program: &LogicProgram,
) -> u32 {
    assert_eq!(f.engine.current().len(), 1);
    let final_link = f.engine.current()[0];
    let (caller, endpoint) = f.store.poles(final_link).unwrap();
    assert_eq!(caller, f.k);

    let (tag, payload) = f.store.poles(endpoint).unwrap();
    assert_eq!(tag, program.result_tag);
    let values = read_exact_sequence(&f.store, payload).unwrap();
    assert_eq!(values.len(), 1);

    decode_word(f, program.width, values[0])
}

fn run_steps(
    f: &mut FullFixture,
    steps: usize,
    label: &str,
) {
    for step in 0..steps {
        let reaction = f.engine.run(&mut f.store).unwrap();
        assert!(
            !reaction.quiescent,
            "{label}: unexpected quiescence at step {step}"
        );
        assert_eq!(reaction.raw_rule_matches, 1, "{label} step {step} matches");
        assert_eq!(
            reaction.transitioned_members,
            1,
            "{label} step {step} transitioned"
        );
        assert_eq!(
            reaction.handoff_count,
            1,
            "{label} step {step} handoff"
        );
        assert_eq!(
            reaction.next_members.len(),
            1,
            "{label} step {step} Scope"
        );
    }

    let stable = f.engine.current_bank();
    let quiescent = f.engine.run(&mut f.store).unwrap();
    assert!(quiescent.quiescent, "{label}: final quiescence");
    assert_eq!(quiescent.raw_rule_matches, 0);
    assert_eq!(quiescent.handoff_count, 0);
    assert_eq!(f.engine.current_bank(), stable);
}

fn run_binary(
    f: &mut FullFixture,
    program: &LogicProgram,
    gate: Handle,
    a: u32,
    b: u32,
) -> u32 {
    let m = mask(program.width);
    assert_eq!(a & !m, 0);
    assert_eq!(b & !m, 0);

    let a_bits = bit_handles(f, program.width, a);
    let b_bits = bit_handles(f, program.width, b);
    let aword =
        materialize_exact_sequence(&mut f.store, &a_bits).unwrap();
    let bword =
        materialize_exact_sequence(&mut f.store, &b_bits).unwrap();
    let args = materialize_exact_sequence(
        &mut f.store,
        &[gate, aword, bword],
    )
    .unwrap();

    let invocation =
        call(&mut f.store, f.apply, program.word_binary, args);
    let initial = f.store.ensure_pair(f.k, invocation).unwrap();
    f.engine.set_current(&f.store, &[initial]).unwrap();

    run_steps(
        f,
        program.binary_steps,
        &format!("WORD_BIN_N={}", program.width),
    );
    decode_logic_result(f, program)
}

fn run_not(
    f: &mut FullFixture,
    program: &LogicProgram,
    a: u32,
) -> u32 {
    let m = mask(program.width);
    assert_eq!(a & !m, 0);

    let a_bits = bit_handles(f, program.width, a);
    let aword =
        materialize_exact_sequence(&mut f.store, &a_bits).unwrap();
    let args =
        materialize_exact_sequence(&mut f.store, &[aword]).unwrap();

    let invocation =
        call(&mut f.store, f.apply, program.word_not, args);
    let initial = f.store.ensure_pair(f.k, invocation).unwrap();
    f.engine.set_current(&f.store, &[initial]).unwrap();

    run_steps(
        f,
        program.unary_steps,
        &format!("WORD_NOT_N={}", program.width),
    );
    decode_logic_result(f, program)
}

fn vectors(width: usize) -> Vec<(u32, u32)> {
    let m = mask(width);
    let sign = 1u32 << (width - 1);

    let mut out = vec![
        (0, 0),
        (0, m),
        (m, 0),
        (m, m),
        (1, 1),
        (sign, sign),
        (0xaaaa_aaaa & m, 0x5555_5555 & m),
        (0xf0f0_f0f0 & m, 0x0ff0_0ff0 & m),
        (0x8000_0003 & m, 0x0000_0001 & m),
    ];

    let mut z = 0xa341_316cu32 ^ width as u32;
    for _ in 0..16 {
        z = z.wrapping_mul(1664525).wrapping_add(1013904223);
        let a = z & m;
        z = z.wrapping_mul(1664525).wrapping_add(1013904223);
        let b = z & m;
        out.push((a, b));
    }

    out.sort_unstable();
    out.dedup();
    out
}

#[test]
#[ignore = "heavy M4 word-logic suite; mandatory release workflow"]
fn m4_logic_8_16_32_function_link_selects_binary_operation() {
    for width in [8usize, 16, 32] {
        let mut f = FullFixture::new();
        let program = LogicProgram::install(&mut f, width);

        assert_eq!(program.binary_steps, 1 + 2 * width);
        assert_eq!(program.unary_steps, 1 + 2 * width);

        let cases = vectors(width);
        for &(a, b) in &cases {
            let and_value =
                run_binary(&mut f, &program, program.gates.and2, a, b);
            let or_value =
                run_binary(&mut f, &program, program.gates.or2, a, b);
            let xor_value =
                run_binary(&mut f, &program, program.gates.xor2, a, b);
            let not_value = run_not(&mut f, &program, a);

            let m = mask(width);
            assert_eq!(and_value, (a & b) & m);
            assert_eq!(or_value, (a | b) & m);
            assert_eq!(xor_value, (a ^ b) & m);
            assert_eq!(not_value, (!a) & m);
        }

        println!(
            "M4_LOGIC width={} cases={} binary_steps={} unary_steps={} program_links={}",
            width,
            cases.len(),
            program.binary_steps,
            program.unary_steps,
            program.links_after_build,
        );
    }
}

#[test]
#[ignore = "heavy M4 word-logic suite; mandatory release workflow"]
fn m4_logic_test_reuses_and_value_and_steady_state_is_canonical() {
    let mut f = FullFixture::new();
    let program = LogicProgram::install(&mut f, 32);

    let a = 0x8000_0003u32;
    let b = 0xffff_0001u32;

    // TEST's condition value is the same structural AND result. Architectural
    // destination suppression and partial flag update are the next layer.
    let and_value =
        run_binary(&mut f, &program, program.gates.and2, a, b);
    assert_eq!(and_value, a & b);
    let test_condition_value = and_value;
    assert_eq!(test_condition_value, 0x8000_0001);

    let links = f.store.link_count();
    let repeated =
        run_binary(&mut f, &program, program.gates.and2, a, b);
    assert_eq!(repeated, and_value);
    assert_eq!(
        f.store.link_count(),
        links,
        "repeated identical WORD_BIN32 materialized new Links"
    );

    let not_first = run_not(&mut f, &program, a);
    assert_eq!(not_first, !a);
    let links = f.store.link_count();
    let not_second = run_not(&mut f, &program, a);
    assert_eq!(not_second, not_first);
    assert_eq!(
        f.store.link_count(),
        links,
        "repeated identical WORD_NOT32 materialized new Links"
    );
}
