use super::{
    arithmetic_n::ArithmeticProgram,
    full_adder::{
        call, define_bundle_rule, index_rule_for, Fixture as FullFixture,
    },
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
        // Width-separated namespace, distinct from the arithmetic generator.
        seed = store.ensure_pair(seed, o).unwrap();
        seed = store.ensure_pair(seed, c).unwrap();
        seed = store.ensure_pair(seed, o).unwrap();
        for _ in 0..width {
            seed = store.ensure_pair(seed, o).unwrap();
            seed = store.ensure_pair(seed, c).unwrap();
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

#[derive(Clone, Copy)]
struct GateSet {
    xor2: Handle,
    or2: Handle,
    not1: Handle,
    bit_outputs: [Handle; 2],
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
    let xor_left = anchors.next(&mut f.store);
    let xor_right = anchors.next(&mut f.store);
    let xor2 = f.store.ensure_pair(xor_left, xor_right).unwrap();

    let or_left = anchors.next(&mut f.store);
    let or_right = anchors.next(&mut f.store);
    let or2 = f.store.ensure_pair(or_left, or_right).unwrap();

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
        xor2,
        bit_outputs,
        &[(0, 0, 0), (0, 1, 1), (1, 0, 1), (1, 1, 0)],
    );
    install_binary_gate(
        f,
        anchors,
        or2,
        bit_outputs,
        &[(0, 0, 0), (0, 1, 1), (1, 0, 1), (1, 1, 1)],
    );

    let bits = [f.zero, f.one];
    for (input, out) in [(0usize, 1usize), (1usize, 0usize)] {
        let caller = anchors.next(&mut f.store);
        let args =
            materialize_exact_sequence(&mut f.store, &[bits[input]]).unwrap();
        let invocation = call(&mut f.store, f.apply, not1, args);
        let before = f.store.ensure_pair(caller, invocation).unwrap();
        let after = f
            .store
            .ensure_pair(caller, bit_outputs[out])
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
        xor2,
        or2,
        not1,
        bit_outputs,
    }
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

fn unary_call(
    f: &mut FullFixture,
    function: Handle,
    value: Handle,
) -> Handle {
    let args = materialize_exact_sequence(&mut f.store, &[value]).unwrap();
    call(&mut f.store, f.apply, function, args)
}

#[derive(Clone, Debug)]
struct FlaggedArithmeticProgram {
    width: usize,
    flagged: Handle,
    result_tag: Handle,
    active_steps: usize,
    links_after_build: usize,
}

impl FlaggedArithmeticProgram {
    fn install(f: &mut FullFixture, width: usize) -> Self {
        assert!((8..=32).contains(&width));

        let arithmetic = ArithmeticProgram::install(f, width);
        let seed0 = f
            .store
            .ensure_pair(arithmetic.arithmetic, arithmetic.result_tag)
            .unwrap();
        let mut anchors = AnchorGen::new(
            &mut f.store,
            seed0,
            f.o,
            f.c,
            width,
        );

        let gates = install_gates(f, &mut anchors);

        let flagged_left = anchors.next(&mut f.store);
        let flagged_right = anchors.next(&mut f.store);
        let flagged =
            f.store.ensure_pair(flagged_left, flagged_right).unwrap();

        let result_left = anchors.next(&mut f.store);
        let result_right = anchors.next(&mut f.store);
        let result_tag =
            f.store.ensure_pair(result_left, result_right).unwrap();

        let arith_tag = anchors.next(&mut f.store);
        let af_tag = anchors.next(&mut f.store);
        let of_tag = anchors.next(&mut f.store);
        let zf_not_tag = anchors.next(&mut f.store);
        let pf_not_tag = anchors.next(&mut f.store);

        let mut zf_tags = Vec::with_capacity(width);
        for _ in 0..width {
            let left = anchors.next(&mut f.store);
            let right = anchors.next(&mut f.store);
            zf_tags.push(f.store.ensure_pair(left, right).unwrap());
        }

        let mut pf_tags = Vec::with_capacity(8);
        for _ in 0..8 {
            let left = anchors.next(&mut f.store);
            let right = anchors.next(&mut f.store);
            pf_tags.push(f.store.ensure_pair(left, right).unwrap());
        }

        // FLAGS OPEN:
        //
        // K -> Call(ARITH_FLAGS,[Aword,Bword,X,Mode])
        // =>
        // ArithFrame(K) -> Call(ARITH,[Aword,Bword,X,Mode])
        {
            let k = anchors.next(&mut f.store);
            let aword = anchors.next(&mut f.store);
            let bword = anchors.next(&mut f.store);
            let x = anchors.next(&mut f.store);
            let mode = anchors.next(&mut f.store);

            let args =
                materialize_exact_sequence(&mut f.store, &[aword, bword, x, mode])
                    .unwrap();
            let before_call =
                call(&mut f.store, f.apply, flagged, args);
            let before = f.store.ensure_pair(k, before_call).unwrap();

            let caller = stage_frame(&mut f.store, arith_tag, &[k]);
            let arith_call = call(
                &mut f.store,
                f.apply,
                arithmetic.arithmetic,
                args,
            );
            let after = f.store.ensure_pair(caller, arith_call).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, aword, bword, x, mode],
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        // Composable ARITH_RESULT payload -> AF XOR.
        {
            let k = anchors.next(&mut f.store);
            let result_bits = anchors.roles(&mut f.store, width);
            let cf = anchors.next(&mut f.store);
            let aux_raw = anchors.next(&mut f.store);
            let sign_in_raw = anchors.next(&mut f.store);
            let final_raw = anchors.next(&mut f.store);
            let mode = anchors.next(&mut f.store);

            let word =
                materialize_exact_sequence(&mut f.store, &result_bits).unwrap();
            let raw = materialize_exact_sequence(
                &mut f.store,
                &[word, cf, aux_raw, sign_in_raw, final_raw, mode],
            )
            .unwrap();
            let envelope =
                f.store.ensure_pair(arithmetic.result_tag, raw).unwrap();
            let caller = stage_frame(&mut f.store, arith_tag, &[k]);
            let before = f.store.ensure_pair(caller, envelope).unwrap();

            let mut state = Vec::with_capacity(width + 4);
            state.push(k);
            state.extend_from_slice(&result_bits);
            state.push(cf);
            state.push(sign_in_raw);
            state.push(final_raw);

            let next_caller = stage_frame(&mut f.store, af_tag, &state);
            let af_call = binary_call(f, gates.xor2, aux_raw, mode);
            let after = f.store.ensure_pair(next_caller, af_call).unwrap();

            let mut roles = Vec::with_capacity(width + 7);
            roles.push(k);
            roles.extend_from_slice(&result_bits);
            roles.push(cf);
            roles.push(aux_raw);
            roles.push(sign_in_raw);
            roles.push(final_raw);
            roles.push(mode);

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &[arithmetic.result_tag],
                admission,
            );
        }

        // AF -> OF XOR.
        {
            let k = anchors.next(&mut f.store);
            let result_bits = anchors.roles(&mut f.store, width);
            let cf = anchors.next(&mut f.store);
            let sign_in_raw = anchors.next(&mut f.store);
            let final_raw = anchors.next(&mut f.store);
            let af = anchors.next(&mut f.store);

            let mut state = Vec::with_capacity(width + 4);
            state.push(k);
            state.extend_from_slice(&result_bits);
            state.push(cf);
            state.push(sign_in_raw);
            state.push(final_raw);

            let caller = stage_frame(&mut f.store, af_tag, &state);
            let af_result =
                materialize_exact_sequence(&mut f.store, &[af]).unwrap();
            let before = f.store.ensure_pair(caller, af_result).unwrap();

            let mut next_state = Vec::with_capacity(width + 3);
            next_state.push(k);
            next_state.extend_from_slice(&result_bits);
            next_state.push(cf);
            next_state.push(af);

            let next_caller = stage_frame(&mut f.store, of_tag, &next_state);
            let of_call =
                binary_call(f, gates.xor2, sign_in_raw, final_raw);
            let after = f.store.ensure_pair(next_caller, of_call).unwrap();

            let mut roles = Vec::with_capacity(width + 6);
            roles.push(k);
            roles.extend_from_slice(&result_bits);
            roles.push(cf);
            roles.push(sign_in_raw);
            roles.push(final_raw);
            roles.push(af);

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &gates.bit_outputs, admission);
        }

        // OF -> ZF OR reduction, seeded with 0.
        {
            let k = anchors.next(&mut f.store);
            let result_bits = anchors.roles(&mut f.store, width);
            let cf = anchors.next(&mut f.store);
            let af = anchors.next(&mut f.store);
            let of = anchors.next(&mut f.store);

            let mut state = Vec::with_capacity(width + 3);
            state.push(k);
            state.extend_from_slice(&result_bits);
            state.push(cf);
            state.push(af);

            let caller = stage_frame(&mut f.store, of_tag, &state);
            let of_result =
                materialize_exact_sequence(&mut f.store, &[of]).unwrap();
            let before = f.store.ensure_pair(caller, of_result).unwrap();

            let mut reduce_state = Vec::with_capacity(width + 4);
            reduce_state.push(k);
            reduce_state.extend_from_slice(&result_bits);
            reduce_state.push(cf);
            reduce_state.push(af);
            reduce_state.push(of);

            let next_caller =
                stage_frame(&mut f.store, zf_tags[0], &reduce_state);
            let or_call =
                binary_call(f, gates.or2, f.zero, result_bits[0]);
            let after = f.store.ensure_pair(next_caller, or_call).unwrap();

            let mut roles = Vec::with_capacity(width + 4);
            roles.push(k);
            roles.extend_from_slice(&result_bits);
            roles.push(cf);
            roles.push(af);
            roles.push(of);

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &gates.bit_outputs, admission);
        }

        // ZF = NOT(OR-reduction(all result bits)).
        for i in 0..width {
            let k = anchors.next(&mut f.store);
            let result_bits = anchors.roles(&mut f.store, width);
            let cf = anchors.next(&mut f.store);
            let af = anchors.next(&mut f.store);
            let of = anchors.next(&mut f.store);
            let acc = anchors.next(&mut f.store);

            let mut state = Vec::with_capacity(width + 4);
            state.push(k);
            state.extend_from_slice(&result_bits);
            state.push(cf);
            state.push(af);
            state.push(of);

            let caller = stage_frame(&mut f.store, zf_tags[i], &state);
            let acc_result =
                materialize_exact_sequence(&mut f.store, &[acc]).unwrap();
            let before = f.store.ensure_pair(caller, acc_result).unwrap();

            let after = if i + 1 < width {
                let next_caller =
                    stage_frame(&mut f.store, zf_tags[i + 1], &state);
                let next_call = binary_call(
                    f,
                    gates.or2,
                    acc,
                    result_bits[i + 1],
                );
                f.store.ensure_pair(next_caller, next_call).unwrap()
            } else {
                let next_caller =
                    stage_frame(&mut f.store, zf_not_tag, &state);
                let next_call = unary_call(f, gates.not1, acc);
                f.store.ensure_pair(next_caller, next_call).unwrap()
            };

            let mut roles = Vec::with_capacity(width + 5);
            roles.push(k);
            roles.extend_from_slice(&result_bits);
            roles.push(cf);
            roles.push(af);
            roles.push(of);
            roles.push(acc);

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &gates.bit_outputs, admission);
        }

        // ZF NOT result -> PF XOR reduction of exactly low byte bits 0..7.
        {
            let k = anchors.next(&mut f.store);
            let result_bits = anchors.roles(&mut f.store, width);
            let cf = anchors.next(&mut f.store);
            let af = anchors.next(&mut f.store);
            let of = anchors.next(&mut f.store);
            let zf = anchors.next(&mut f.store);

            let mut state = Vec::with_capacity(width + 4);
            state.push(k);
            state.extend_from_slice(&result_bits);
            state.push(cf);
            state.push(af);
            state.push(of);

            let caller = stage_frame(&mut f.store, zf_not_tag, &state);
            let zf_result =
                materialize_exact_sequence(&mut f.store, &[zf]).unwrap();
            let before = f.store.ensure_pair(caller, zf_result).unwrap();

            let mut parity_state = Vec::with_capacity(width + 5);
            parity_state.push(k);
            parity_state.extend_from_slice(&result_bits);
            parity_state.push(cf);
            parity_state.push(af);
            parity_state.push(of);
            parity_state.push(zf);

            let next_caller =
                stage_frame(&mut f.store, pf_tags[0], &parity_state);
            let xor_call =
                binary_call(f, gates.xor2, f.zero, result_bits[0]);
            let after = f.store.ensure_pair(next_caller, xor_call).unwrap();

            let mut roles = Vec::with_capacity(width + 5);
            roles.push(k);
            roles.extend_from_slice(&result_bits);
            roles.push(cf);
            roles.push(af);
            roles.push(of);
            roles.push(zf);

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &gates.bit_outputs, admission);
        }

        // PF = NOT(XOR-reduction(result bits 0..7)).
        for i in 0..8 {
            let k = anchors.next(&mut f.store);
            let result_bits = anchors.roles(&mut f.store, width);
            let cf = anchors.next(&mut f.store);
            let af = anchors.next(&mut f.store);
            let of = anchors.next(&mut f.store);
            let zf = anchors.next(&mut f.store);
            let acc = anchors.next(&mut f.store);

            let mut state = Vec::with_capacity(width + 5);
            state.push(k);
            state.extend_from_slice(&result_bits);
            state.push(cf);
            state.push(af);
            state.push(of);
            state.push(zf);

            let caller = stage_frame(&mut f.store, pf_tags[i], &state);
            let acc_result =
                materialize_exact_sequence(&mut f.store, &[acc]).unwrap();
            let before = f.store.ensure_pair(caller, acc_result).unwrap();

            let after = if i + 1 < 8 {
                let next_caller =
                    stage_frame(&mut f.store, pf_tags[i + 1], &state);
                let next_call = binary_call(
                    f,
                    gates.xor2,
                    acc,
                    result_bits[i + 1],
                );
                f.store.ensure_pair(next_caller, next_call).unwrap()
            } else {
                let next_caller =
                    stage_frame(&mut f.store, pf_not_tag, &state);
                let next_call = unary_call(f, gates.not1, acc);
                f.store.ensure_pair(next_caller, next_call).unwrap()
            };

            let mut roles = Vec::with_capacity(width + 6);
            roles.push(k);
            roles.extend_from_slice(&result_bits);
            roles.push(cf);
            roles.push(af);
            roles.push(of);
            roles.push(zf);
            roles.push(acc);

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &gates.bit_outputs, admission);
        }

        // PF NOT result -> stable FLAGGED_RESULT envelope.
        {
            let k = anchors.next(&mut f.store);
            let result_bits = anchors.roles(&mut f.store, width);
            let cf = anchors.next(&mut f.store);
            let af = anchors.next(&mut f.store);
            let of = anchors.next(&mut f.store);
            let zf = anchors.next(&mut f.store);
            let pf = anchors.next(&mut f.store);

            let mut state = Vec::with_capacity(width + 5);
            state.push(k);
            state.extend_from_slice(&result_bits);
            state.push(cf);
            state.push(af);
            state.push(of);
            state.push(zf);

            let caller = stage_frame(&mut f.store, pf_not_tag, &state);
            let pf_result =
                materialize_exact_sequence(&mut f.store, &[pf]).unwrap();
            let before = f.store.ensure_pair(caller, pf_result).unwrap();

            let word =
                materialize_exact_sequence(&mut f.store, &result_bits).unwrap();
            let sf = result_bits[width - 1];
            let payload = materialize_exact_sequence(
                &mut f.store,
                &[word, cf, pf, af, zf, sf, of],
            )
            .unwrap();
            let envelope =
                f.store.ensure_pair(result_tag, payload).unwrap();
            let after = f.store.ensure_pair(k, envelope).unwrap();

            let mut roles = Vec::with_capacity(width + 6);
            roles.push(k);
            roles.extend_from_slice(&result_bits);
            roles.push(cf);
            roles.push(af);
            roles.push(of);
            roles.push(zf);
            roles.push(pf);

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &gates.bit_outputs, admission);
        }

        // Wrapper/gates/reductions around the already-proven shared arithmetic:
        // arithmetic = 5 + 16N
        // extra      = 26 + 2N
        // total      = 31 + 18N
        let active_steps = arithmetic.active_steps + 26 + 2 * width;
        assert_eq!(active_steps, 31 + 18 * width);

        Self {
            width,
            flagged,
            result_tag,
            active_steps,
            links_after_build: f.store.link_count(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FlagOutcome {
    value: u32,
    cf: u8,
    pf: u8,
    af: u8,
    zf: u8,
    sf: u8,
    of: u8,
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

fn decode_bit(f: &FullFixture, bit: Handle) -> u8 {
    if bit == f.one {
        1
    } else {
        assert_eq!(bit, f.zero);
        0
    }
}

fn decode_word(
    f: &FullFixture,
    width: usize,
    word: Handle,
) -> u32 {
    let bits = read_exact_sequence(&f.store, word).unwrap();
    assert_eq!(bits.len(), width);

    let mut value = 0u32;
    for (i, bit) in bits.into_iter().enumerate() {
        value |= u32::from(decode_bit(f, bit)) << i;
    }
    value
}

fn run_flagged(
    f: &mut FullFixture,
    program: &FlaggedArithmeticProgram,
    a: u32,
    b: u32,
    x: u8,
    mode: u8,
) -> FlagOutcome {
    let m = mask(program.width);
    assert_eq!(a & !m, 0);
    assert_eq!(b & !m, 0);
    assert!(x <= 1);
    assert!(mode <= 1);

    let a_bits = bit_handles(f, program.width, a);
    let b_bits = bit_handles(f, program.width, b);
    let aword =
        materialize_exact_sequence(&mut f.store, &a_bits).unwrap();
    let bword =
        materialize_exact_sequence(&mut f.store, &b_bits).unwrap();
    let xh = if x == 0 { f.zero } else { f.one };
    let mh = if mode == 0 { f.zero } else { f.one };
    let args = materialize_exact_sequence(
        &mut f.store,
        &[aword, bword, xh, mh],
    )
    .unwrap();

    let invocation =
        call(&mut f.store, f.apply, program.flagged, args);
    let initial = f.store.ensure_pair(f.k, invocation).unwrap();
    f.engine.set_current(&f.store, &[initial]).unwrap();

    for step in 0..program.active_steps {
        let reaction = f.engine.run(&mut f.store).unwrap();
        assert!(
            !reaction.quiescent,
            "FLAGS_N={} A={a} B={b} X={x} M={mode}: step {step}",
            program.width
        );
        assert_eq!(reaction.raw_rule_matches, 1, "step {step} matches");
        assert_eq!(reaction.transitioned_members, 1, "step {step} transitioned");
        assert_eq!(reaction.handoff_count, 1, "step {step} handoff");
        assert_eq!(reaction.next_members.len(), 1, "step {step} Scope");
    }

    let stable = f.engine.current_bank();
    let quiescent = f.engine.run(&mut f.store).unwrap();
    assert!(quiescent.quiescent);
    assert_eq!(quiescent.raw_rule_matches, 0);
    assert_eq!(quiescent.handoff_count, 0);
    assert_eq!(f.engine.current_bank(), stable);
    assert_eq!(f.engine.current().len(), 1);

    let final_link = f.engine.current()[0];
    let (caller, endpoint) = f.store.poles(final_link).unwrap();
    assert_eq!(caller, f.k);

    let (tag, payload) = f.store.poles(endpoint).unwrap();
    assert_eq!(tag, program.result_tag);
    let values = read_exact_sequence(&f.store, payload).unwrap();
    assert_eq!(values.len(), 7);

    FlagOutcome {
        value: decode_word(f, program.width, values[0]),
        cf: decode_bit(f, values[1]),
        pf: decode_bit(f, values[2]),
        af: decode_bit(f, values[3]),
        zf: decode_bit(f, values[4]),
        sf: decode_bit(f, values[5]),
        of: decode_bit(f, values[6]),
    }
}

fn signed_value(width: usize, value: u32) -> i64 {
    let sign = 1u64 << (width - 1);
    let modulus = 1i64 << width;
    let v = u64::from(value & mask(width));
    if v & sign == 0 {
        v as i64
    } else {
        v as i64 - modulus
    }
}

fn expected_flags(
    width: usize,
    a: u32,
    b: u32,
    x: u8,
    mode: u8,
) -> FlagOutcome {
    let m = mask(width);
    let wide_mask = u64::from(m);

    let (value, cf) = if mode == 0 {
        let total = u64::from(a) + u64::from(b) + u64::from(x);
        ((total & wide_mask) as u32, u8::from(total > wide_mask))
    } else {
        let subtrahend = u64::from(b) + u64::from(x);
        (
            a.wrapping_sub(b)
                .wrapping_sub(u32::from(x))
                & m,
            u8::from(u64::from(a) < subtrahend),
        )
    };

    let af = if mode == 0 {
        u8::from(
            u16::from((a & 0x0f) as u8)
                + u16::from((b & 0x0f) as u8)
                + u16::from(x)
                > 0x0f,
        )
    } else {
        u8::from(
            u16::from((a & 0x0f) as u8)
                < u16::from((b & 0x0f) as u8) + u16::from(x),
        )
    };

    let sa = signed_value(width, a);
    let sb = signed_value(width, b);
    let signed_math = if mode == 0 {
        sa + sb + i64::from(x)
    } else {
        sa - sb - i64::from(x)
    };
    let min = -(1i64 << (width - 1));
    let max = (1i64 << (width - 1)) - 1;
    let of = u8::from(signed_math < min || signed_math > max);

    FlagOutcome {
        value,
        cf,
        pf: u8::from((value as u8).count_ones() % 2 == 0),
        af,
        zf: u8::from(value == 0),
        sf: ((value >> (width - 1)) & 1) as u8,
        of,
    }
}

fn vectors(width: usize) -> Vec<(u32, u32, u8, u8)> {
    let m = mask(width);
    let sign = 1u32 << (width - 1);
    let max_pos = sign - 1;

    let mut out = vec![
        (0, 0, 0, 0),                 // zero + PF even
        (m, 0, 1, 0),                 // ADC carry to zero
        (0, 0, 1, 1),                 // SBB borrow
        (0x0f & m, 0, 1, 0),          // AF add
        (0x10 & m, 1, 0, 1),          // AF subtract
        (max_pos, 0, 1, 0),           // positive signed overflow
        (sign, 0, 1, 1),              // negative signed overflow
        (sign, 1, 0, 0),              // negative non-overflow
        (0x03 & m, 0, 0, 0),          // PF even
        (0x01 & m, 0, 0, 0),          // PF odd
        (0x55 & m, 0, 0, 0),          // four low-byte bits
        (0x7f & m, 0, 1, 0),
        (0x80 & m, 1, 0, 1),
        (m, m, 1, 0),
        (m, m, 1, 1),
    ];

    let mut z = 0x6d2b_79f5u32 ^ width as u32;
    for i in 0..12u32 {
        z = z.wrapping_mul(1664525).wrapping_add(1013904223);
        let a = z & m;
        z = z.wrapping_mul(1664525).wrapping_add(1013904223);
        let b = z & m;
        out.push((a, b, (i & 1) as u8, ((i >> 1) & 1) as u8));
    }

    out.sort_unstable();
    out.dedup();
    out
}

#[test]
#[ignore = "heavy structural EFLAGS suite; mandatory release workflow"]
fn m3_eflags_8_16_32_match_independent_oracle() {
    for width in [8usize, 16, 32] {
        let mut f = FullFixture::new();
        let program = FlaggedArithmeticProgram::install(&mut f, width);
        assert_eq!(program.active_steps, 31 + 18 * width);

        let cases = vectors(width);
        for &(a, b, x, mode) in &cases {
            let actual =
                run_flagged(&mut f, &program, a, b, x, mode);
            let expected = expected_flags(width, a, b, x, mode);
            assert_eq!(
                actual, expected,
                "width={width} A={a:#x} B={b:#x} X={x} M={mode}"
            );
        }

        println!(
            "EFLAGS width={} cases={} active_steps={} program_links={}",
            width,
            cases.len(),
            program.active_steps,
            program.links_after_build,
        );
    }
}

#[test]
#[ignore = "heavy structural EFLAGS suite; mandatory release workflow"]
fn m3_eflags_named_edge_witnesses_and_steady_state() {
    let mut f = FullFixture::new();
    let program = FlaggedArithmeticProgram::install(&mut f, 32);

    let zero = run_flagged(&mut f, &program, u32::MAX, 0, 1, 0);
    assert_eq!(zero, expected_flags(32, u32::MAX, 0, 1, 0));
    assert_eq!((zero.cf, zero.zf, zero.pf), (1, 1, 1));

    let borrow = run_flagged(&mut f, &program, 0, 0, 1, 1);
    assert_eq!(borrow, expected_flags(32, 0, 0, 1, 1));
    assert_eq!((borrow.cf, borrow.sf), (1, 1));

    let add_overflow =
        run_flagged(&mut f, &program, 0x7fff_ffff, 0, 1, 0);
    assert_eq!(add_overflow.of, 1);

    let sub_overflow =
        run_flagged(&mut f, &program, 0x8000_0000, 0, 1, 1);
    assert_eq!(sub_overflow.of, 1);

    let aux_add = run_flagged(&mut f, &program, 0x0f, 0, 1, 0);
    assert_eq!(aux_add.af, 1);

    let aux_sub = run_flagged(&mut f, &program, 0x10, 1, 0, 1);
    assert_eq!(aux_sub.af, 1);

    let parity_even = run_flagged(&mut f, &program, 0x03, 0, 0, 0);
    let parity_odd = run_flagged(&mut f, &program, 0x01, 0, 0, 0);
    assert_eq!(parity_even.pf, 1);
    assert_eq!(parity_odd.pf, 0);

    let first = run_flagged(
        &mut f,
        &program,
        0x1234_5678,
        0x9abc_def0,
        1,
        1,
    );
    assert_eq!(
        first,
        expected_flags(32, 0x1234_5678, 0x9abc_def0, 1, 1)
    );
    let links = f.store.link_count();
    let second = run_flagged(
        &mut f,
        &program,
        0x1234_5678,
        0x9abc_def0,
        1,
        1,
    );
    assert_eq!(second, first);
    assert_eq!(
        f.store.link_count(),
        links,
        "repeated identical flagged ARITH32 materialized new Links"
    );
}
