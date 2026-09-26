use super::{
    full_adder::{
        call, define_bundle_rule, index_rule_for, Fixture as FullFixture,
    },
    ripple_n::{run_add, RippleProgram},
    subtractor_n::{run_sub, RippleSubProgram},
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
        // Distinct from both existing ripple namespaces.
        seed = store.ensure_pair(seed, c).unwrap();
        seed = store.ensure_pair(seed, o).unwrap();
        for _ in 0..width {
            seed = store.ensure_pair(seed, c).unwrap();
            seed = store.ensure_pair(seed, o).unwrap();
        }
        seed = store.ensure_pair(seed, c).unwrap();

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

#[derive(Clone, Debug)]
pub(crate) struct ArithmeticProgram {
    pub(crate) width: usize,
    pub(crate) arithmetic: Handle,
    pub(crate) result_tag: Handle,
    pub(crate) active_steps: usize,
    pub(crate) links_after_build: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ArithmeticOutcome {
    value: u32,
    status: u8,
    aux_raw: u8,
    sign_in_raw: u8,
    final_raw: u8,
    mode: u8,
}

impl ArithmeticProgram {
    pub(crate) fn install(f: &mut FullFixture, width: usize) -> Self {
        assert!((4..=32).contains(&width));

        let seed0 = f.store.ensure_pair(f.k, f.full).unwrap();
        let mut anchors = AnchorGen::new(
            &mut f.store,
            seed0,
            f.o,
            f.c,
            width,
        );

        // Standalone structural XOR2. As with unary NOT, the scalar result is
        // returned inside a one-position ExactSequence so it cannot collide
        // with unrelated generic templates in the mixed Theory.
        let xor_left = anchors.next(&mut f.store);
        let xor_right = anchors.next(&mut f.store);
        let xor2 = f.store.ensure_pair(xor_left, xor_right).unwrap();

        let xor_outputs = [
            materialize_exact_sequence(&mut f.store, &[f.zero]).unwrap(),
            materialize_exact_sequence(&mut f.store, &[f.one]).unwrap(),
        ];

        for (a, b, output) in [
            (f.zero, f.zero, f.zero),
            (f.zero, f.one, f.one),
            (f.one, f.zero, f.one),
            (f.one, f.one, f.zero),
        ] {
            let caller = anchors.next(&mut f.store);
            let args =
                materialize_exact_sequence(&mut f.store, &[a, b]).unwrap();
            let invocation = call(&mut f.store, f.apply, xor2, args);
            let before = f.store.ensure_pair(caller, invocation).unwrap();
            let output_index = if output == f.zero { 0 } else { 1 };
            let after = f
                .store
                .ensure_pair(caller, xor_outputs[output_index])
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

        let arith_left = anchors.next(&mut f.store);
        let arith_right = anchors.next(&mut f.store);
        let arithmetic =
            f.store.ensure_pair(arith_left, arith_right).unwrap();

        // Stable component-result envelope tag. A large ExactSequence payload
        // is self-starting, so using it directly as the endpoint would make
        // trigger discovery depend on the entire concrete result value. The
        // stable tag keeps the payload fully structural while allowing later
        // components (EFLAGS/ALU) to index one generic continuation.
        let result_left = anchors.next(&mut f.store);
        let result_right = anchors.next(&mut f.store);
        let result_tag =
            f.store.ensure_pair(result_left, result_right).unwrap();

        let c0_tag = anchors.next(&mut f.store);
        let status_tag = anchors.next(&mut f.store);

        let mut b_tags = Vec::with_capacity(width);
        let mut fa_tags = Vec::with_capacity(width);
        for _ in 0..width {
            let b_left = anchors.next(&mut f.store);
            let b_right = anchors.next(&mut f.store);
            b_tags.push(f.store.ensure_pair(b_left, b_right).unwrap());

            let fa_left = anchors.next(&mut f.store);
            let fa_right = anchors.next(&mut f.store);
            fa_tags.push(f.store.ensure_pair(fa_left, fa_right).unwrap());
        }

        let full_outputs = [
            materialize_exact_sequence(&mut f.store, &[f.zero, f.zero]).unwrap(),
            materialize_exact_sequence(&mut f.store, &[f.one, f.zero]).unwrap(),
            materialize_exact_sequence(&mut f.store, &[f.zero, f.one]).unwrap(),
            materialize_exact_sequence(&mut f.store, &[f.one, f.one]).unwrap(),
        ];

        // OPEN:
        //
        // K -> Call(ARITH,[Aword,Bword,X,Mode])
        // =>
        // C0Frame([K,A...,B...,Mode]) -> XOR2(X,Mode)
        //
        // Mode 0: X is ADD/ADC Cin.
        // Mode 1: X is SUB/SBB Bin, so XOR with one gives the two's-complement
        // carry-in NOT(Bin).
        {
            let k = anchors.next(&mut f.store);
            let a_roles = anchors.roles(&mut f.store, width);
            let b_roles = anchors.roles(&mut f.store, width);
            let x = anchors.next(&mut f.store);
            let mode = anchors.next(&mut f.store);

            let aword =
                materialize_exact_sequence(&mut f.store, &a_roles).unwrap();
            let bword =
                materialize_exact_sequence(&mut f.store, &b_roles).unwrap();
            let args = materialize_exact_sequence(
                &mut f.store,
                &[aword, bword, x, mode],
            )
            .unwrap();
            let invocation =
                call(&mut f.store, f.apply, arithmetic, args);
            let before = f.store.ensure_pair(k, invocation).unwrap();

            let mut state = Vec::with_capacity(2 * width + 2);
            state.push(k);
            state.extend_from_slice(&a_roles);
            state.extend_from_slice(&b_roles);
            state.push(mode);

            let caller = stage_frame(&mut f.store, c0_tag, &state);
            let xor_args =
                materialize_exact_sequence(&mut f.store, &[x, mode]).unwrap();
            let xor_call = call(&mut f.store, f.apply, xor2, xor_args);
            let after = f.store.ensure_pair(caller, xor_call).unwrap();

            let mut roles = Vec::with_capacity(2 * width + 3);
            roles.push(k);
            roles.extend_from_slice(&a_roles);
            roles.extend_from_slice(&b_roles);
            roles.push(x);
            roles.push(mode);

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        // C0 return -> first B XOR.
        {
            let k = anchors.next(&mut f.store);
            let a_roles = anchors.roles(&mut f.store, width);
            let b_roles = anchors.roles(&mut f.store, width);
            let mode = anchors.next(&mut f.store);
            let carry = anchors.next(&mut f.store);

            let mut before_state = Vec::with_capacity(2 * width + 2);
            before_state.push(k);
            before_state.extend_from_slice(&a_roles);
            before_state.extend_from_slice(&b_roles);
            before_state.push(mode);
            let caller = stage_frame(&mut f.store, c0_tag, &before_state);

            let carry_result =
                materialize_exact_sequence(&mut f.store, &[carry]).unwrap();
            let before = f.store.ensure_pair(caller, carry_result).unwrap();

            // Consistent per-bit state:
            // [K, A_remaining..., B_after_current..., Result_so_far...,
            //  CarryIn, Mode, AuxRaw, SignInRaw]
            let mut state =
                Vec::with_capacity(1 + width + (width - 1) + 4);
            state.push(k);
            state.extend_from_slice(&a_roles);
            state.extend_from_slice(&b_roles[1..]);
            state.push(carry);
            state.push(mode);
            state.push(f.zero);
            state.push(f.zero);

            let next_caller =
                stage_frame(&mut f.store, b_tags[0], &state);
            let xor_args = materialize_exact_sequence(
                &mut f.store,
                &[b_roles[0], mode],
            )
            .unwrap();
            let xor_call = call(&mut f.store, f.apply, xor2, xor_args);
            let after = f.store.ensure_pair(next_caller, xor_call).unwrap();

            let mut roles = Vec::with_capacity(2 * width + 3);
            roles.push(k);
            roles.extend_from_slice(&a_roles);
            roles.extend_from_slice(&b_roles);
            roles.push(mode);
            roles.push(carry);

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &xor_outputs, admission);
        }

        for i in 0..width {
            let remaining_a = width - i;
            let remaining_b_after = width - i - 1;

            // B_eff return -> Full Adder.
            {
                let k = anchors.next(&mut f.store);
                let a_rem = anchors.roles(&mut f.store, remaining_a);
                let b_after =
                    anchors.roles(&mut f.store, remaining_b_after);
                let prev = anchors.roles(&mut f.store, i);
                let carry = anchors.next(&mut f.store);
                let mode = anchors.next(&mut f.store);
                let aux = anchors.next(&mut f.store);
                let sign_in = anchors.next(&mut f.store);
                let b_eff = anchors.next(&mut f.store);

                let mut state = Vec::with_capacity(
                    1 + remaining_a + remaining_b_after + i + 4,
                );
                state.push(k);
                state.extend_from_slice(&a_rem);
                state.extend_from_slice(&b_after);
                state.extend_from_slice(&prev);
                state.push(carry);
                state.push(mode);
                state.push(aux);
                state.push(sign_in);

                let caller =
                    stage_frame(&mut f.store, b_tags[i], &state);
                let xor_result =
                    materialize_exact_sequence(&mut f.store, &[b_eff]).unwrap();
                let before =
                    f.store.ensure_pair(caller, xor_result).unwrap();

                let fa_caller =
                    stage_frame(&mut f.store, fa_tags[i], &state);
                let fa_args = materialize_exact_sequence(
                    &mut f.store,
                    &[a_rem[0], b_eff, carry],
                )
                .unwrap();
                let fa_call =
                    call(&mut f.store, f.apply, f.full, fa_args);
                let after =
                    f.store.ensure_pair(fa_caller, fa_call).unwrap();

                let mut roles = Vec::with_capacity(state.len() + 1);
                roles.push(k);
                roles.extend_from_slice(&a_rem);
                roles.extend_from_slice(&b_after);
                roles.extend_from_slice(&prev);
                roles.push(carry);
                roles.push(mode);
                roles.push(aux);
                roles.push(sign_in);
                roles.push(b_eff);

                let (_, admission) = define_bundle_rule(
                    &mut f.store,
                    f.theory,
                    &roles,
                    before,
                    &[after],
                );
                index_rule_for(&mut f.store, &xor_outputs, admission);
            }

            // Full Adder result -> next B XOR or final status XOR.
            {
                let k = anchors.next(&mut f.store);
                let a_rem = anchors.roles(&mut f.store, remaining_a);
                let b_after =
                    anchors.roles(&mut f.store, remaining_b_after);
                let prev = anchors.roles(&mut f.store, i);
                let carry = anchors.next(&mut f.store);
                let mode = anchors.next(&mut f.store);
                let aux = anchors.next(&mut f.store);
                let sign_in = anchors.next(&mut f.store);
                let result_i = anchors.next(&mut f.store);
                let carry_next = anchors.next(&mut f.store);

                let mut state = Vec::with_capacity(
                    1 + remaining_a + remaining_b_after + i + 4,
                );
                state.push(k);
                state.extend_from_slice(&a_rem);
                state.extend_from_slice(&b_after);
                state.extend_from_slice(&prev);
                state.push(carry);
                state.push(mode);
                state.push(aux);
                state.push(sign_in);

                let caller =
                    stage_frame(&mut f.store, fa_tags[i], &state);
                let fa_result = materialize_exact_sequence(
                    &mut f.store,
                    &[result_i, carry_next],
                )
                .unwrap();
                let before = f.store.ensure_pair(caller, fa_result).unwrap();

                let next_aux = if i == 3 { carry_next } else { aux };
                let next_sign = if i + 1 == width - 1 {
                    carry_next
                } else {
                    sign_in
                };

                let mut next_results = prev.clone();
                next_results.push(result_i);

                let after = if i + 1 < width {
                    let mut next_state = Vec::with_capacity(
                        1 + (remaining_a - 1)
                            + (remaining_b_after - 1)
                            + (i + 1)
                            + 4,
                    );
                    next_state.push(k);
                    next_state.extend_from_slice(&a_rem[1..]);
                    next_state.extend_from_slice(&b_after[1..]);
                    next_state.extend_from_slice(&next_results);
                    next_state.push(carry_next);
                    next_state.push(mode);
                    next_state.push(next_aux);
                    next_state.push(next_sign);

                    let next_caller =
                        stage_frame(&mut f.store, b_tags[i + 1], &next_state);
                    let xor_args = materialize_exact_sequence(
                        &mut f.store,
                        &[b_after[0], mode],
                    )
                    .unwrap();
                    let xor_call =
                        call(&mut f.store, f.apply, xor2, xor_args);
                    f.store.ensure_pair(next_caller, xor_call).unwrap()
                } else {
                    let mut final_state =
                        Vec::with_capacity(1 + width + 4);
                    final_state.push(k);
                    final_state.extend_from_slice(&next_results);
                    final_state.push(mode);
                    final_state.push(next_aux);
                    final_state.push(next_sign);
                    final_state.push(carry_next);

                    let status_caller =
                        stage_frame(&mut f.store, status_tag, &final_state);
                    let xor_args = materialize_exact_sequence(
                        &mut f.store,
                        &[carry_next, mode],
                    )
                    .unwrap();
                    let xor_call =
                        call(&mut f.store, f.apply, xor2, xor_args);
                    f.store.ensure_pair(status_caller, xor_call).unwrap()
                };

                let mut roles = Vec::with_capacity(state.len() + 2);
                roles.push(k);
                roles.extend_from_slice(&a_rem);
                roles.extend_from_slice(&b_after);
                roles.extend_from_slice(&prev);
                roles.push(carry);
                roles.push(mode);
                roles.push(aux);
                roles.push(sign_in);
                roles.push(result_i);
                roles.push(carry_next);

                let (_, admission) = define_bundle_rule(
                    &mut f.store,
                    f.theory,
                    &roles,
                    before,
                    &[after],
                );
                index_rule_for(&mut f.store, &full_outputs, admission);
            }
        }

        // StatusOut = C[N] XOR Mode.
        {
            let k = anchors.next(&mut f.store);
            let results = anchors.roles(&mut f.store, width);
            let mode = anchors.next(&mut f.store);
            let aux = anchors.next(&mut f.store);
            let sign_in = anchors.next(&mut f.store);
            let final_raw = anchors.next(&mut f.store);
            let status = anchors.next(&mut f.store);

            let mut state = Vec::with_capacity(1 + width + 4);
            state.push(k);
            state.extend_from_slice(&results);
            state.push(mode);
            state.push(aux);
            state.push(sign_in);
            state.push(final_raw);

            let caller =
                stage_frame(&mut f.store, status_tag, &state);
            let status_result =
                materialize_exact_sequence(&mut f.store, &[status]).unwrap();
            let before =
                f.store.ensure_pair(caller, status_result).unwrap();

            let word =
                materialize_exact_sequence(&mut f.store, &results).unwrap();

            // Raw arithmetic outcome intentionally exposes carry taps needed
            // by the next EFLAGS stage:
            // [Word, CF/Borrow, C4_raw, Csign_in_raw, Cfinal_raw, Mode]
            let outcome = materialize_exact_sequence(
                &mut f.store,
                &[word, status, aux, sign_in, final_raw, mode],
            )
            .unwrap();
            let envelope =
                f.store.ensure_pair(result_tag, outcome).unwrap();
            let after = f.store.ensure_pair(k, envelope).unwrap();

            let mut roles = Vec::with_capacity(width + 6);
            roles.push(k);
            roles.extend_from_slice(&results);
            roles.push(mode);
            roles.push(aux);
            roles.push(sign_in);
            roles.push(final_raw);
            roles.push(status);

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &xor_outputs, admission);
        }

        let active_steps = 5 + 16 * width;
        let links_after_build = f.store.link_count();

        Self {
            width,
            arithmetic,
            result_tag,
            active_steps,
            links_after_build,
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

fn decode_bit(f: &FullFixture, bit: Handle) -> u8 {
    if bit == f.one {
        1
    } else {
        assert_eq!(bit, f.zero);
        0
    }
}

pub(crate) fn run_arithmetic(
    f: &mut FullFixture,
    program: &ArithmeticProgram,
    a: u32,
    b: u32,
    x: u8,
    mode: u8,
) -> ArithmeticOutcome {
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
        call(&mut f.store, f.apply, program.arithmetic, args);
    let initial = f.store.ensure_pair(f.k, invocation).unwrap();

    f.engine.set_current(&f.store, &[initial]).unwrap();

    for step in 0..program.active_steps {
        let reaction = f.engine.run(&mut f.store).unwrap();
        assert!(
            !reaction.quiescent,
            "ARITH_N={} A={a} B={b} X={x} M={mode}: step {step}",
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
    let (caller, result) = f.store.poles(final_link).unwrap();
    assert_eq!(caller, f.k);

    let (result_tag, payload) = f.store.poles(result).unwrap();
    assert_eq!(result_tag, program.result_tag);
    let values = read_exact_sequence(&f.store, payload).unwrap();
    assert_eq!(values.len(), 6);

    ArithmeticOutcome {
        value: decode_word(f, program.width, values[0]),
        status: decode_bit(f, values[1]),
        aux_raw: decode_bit(f, values[2]),
        sign_in_raw: decode_bit(f, values[3]),
        final_raw: decode_bit(f, values[4]),
        mode: decode_bit(f, values[5]),
    }
}

fn expected(
    width: usize,
    a: u32,
    b: u32,
    x: u8,
    mode: u8,
) -> ArithmeticOutcome {
    let m = mask(width);
    let effective_b = if mode == 0 { b } else { b ^ m };
    let mut carry = x ^ mode;
    let mut value = 0u32;
    let mut aux_raw = 0u8;
    let mut sign_in_raw = 0u8;

    for bit in 0..width {
        if bit == width - 1 {
            sign_in_raw = carry;
        }

        let av = ((a >> bit) & 1) as u8;
        let bv = ((effective_b >> bit) & 1) as u8;
        let sum = av ^ bv ^ carry;
        let next =
            (av & bv) | (av & carry) | (bv & carry);
        value |= u32::from(sum) << bit;
        carry = next;

        if bit == 3 {
            aux_raw = carry;
        }
    }

    ArithmeticOutcome {
        value,
        status: carry ^ mode,
        aux_raw,
        sign_in_raw,
        final_raw: carry,
        mode,
    }
}

fn compact_vectors(width: usize) -> Vec<(u32, u32, u8)> {
    let m = mask(width);
    let mut out = vec![
        (0, 0, 0),
        (0, 0, 1),
        (0, 1 & m, 0),
        (m, 0, 0),
        (m, 1 & m, 0),
        (m, m, 1),
        (0xaaaa_aaaa & m, 0x5555_5555 & m, 0),
        (0x5555_5555 & m, 0xaaaa_aaaa & m, 1),
    ];

    for bit in [0usize, width / 2, width - 1] {
        let p = 1u32 << bit;
        out.push((p, 1 & m, 0));
        out.push((0, p, 1));
    }

    let mut z = 0xd1b5_4a32u32 ^ (width as u32);
    for i in 0..12u32 {
        z = z.wrapping_mul(1664525).wrapping_add(1013904223);
        let a = z & m;
        z = z.wrapping_mul(1664525).wrapping_add(1013904223);
        let b = z & m;
        out.push((a, b, (i & 1) as u8));
    }

    out.sort_unstable();
    out.dedup();
    out
}

#[test]
#[ignore = "heavy shared arithmetic differential; mandatory release workflow"]
fn m3_arith_shared_n4_exhaustive_differential_1024() {
    let mut shared_f = FullFixture::new();
    let shared = ArithmeticProgram::install(&mut shared_f, 4);
    assert_eq!(shared.active_steps, 69);

    let mut add_f = FullFixture::new();
    let add = RippleProgram::install(&mut add_f, 4);

    let mut sub_f = FullFixture::new();
    let sub = RippleSubProgram::install(&mut sub_f, 4);

    let mut cases = 0usize;
    for a in 0u32..16 {
        for b in 0u32..16 {
            for x in 0u8..=1 {
                for mode in 0u8..=1 {
                    let actual =
                        run_arithmetic(&mut shared_f, &shared, a, b, x, mode);
                    assert_eq!(actual, expected(4, a, b, x, mode));

                    let legacy = if mode == 0 {
                        run_add(&mut add_f, &add, a, b, x)
                    } else {
                        run_sub(&mut sub_f, &sub, a, b, x)
                    };
                    assert_eq!(
                        (actual.value, actual.status),
                        legacy,
                        "legacy differential A={a} B={b} X={x} M={mode}"
                    );
                    cases += 1;
                }
            }
        }
    }

    assert_eq!(cases, 1024);
    println!(
        "ARITH_SHARED width=4 cases=1024 active_steps={} program_links={}",
        shared.active_steps,
        shared.links_after_build,
    );
}

#[test]
#[ignore = "heavy shared arithmetic differential; mandatory release workflow"]
fn m3_arith_shared_8_16_32_differential() {
    for width in [8usize, 16, 32] {
        let mut shared_f = FullFixture::new();
        let shared = ArithmeticProgram::install(&mut shared_f, width);
        assert_eq!(shared.active_steps, 5 + 16 * width);

        let mut add_f = FullFixture::new();
        let add = RippleProgram::install(&mut add_f, width);

        let mut sub_f = FullFixture::new();
        let sub = RippleSubProgram::install(&mut sub_f, width);

        let vectors = compact_vectors(width);
        let mut cases = 0usize;

        for &(a, b, x) in &vectors {
            for mode in 0u8..=1 {
                let actual =
                    run_arithmetic(&mut shared_f, &shared, a, b, x, mode);
                assert_eq!(actual, expected(width, a, b, x, mode));

                let legacy = if mode == 0 {
                    run_add(&mut add_f, &add, a, b, x)
                } else {
                    run_sub(&mut sub_f, &sub, a, b, x)
                };
                assert_eq!((actual.value, actual.status), legacy);
                cases += 1;
            }
        }

        println!(
            "ARITH_SHARED width={} cases={} active_steps={} program_links={}",
            width,
            cases,
            shared.active_steps,
            shared.links_after_build,
        );
    }
}

#[test]
#[ignore = "heavy shared arithmetic differential; mandatory release workflow"]
fn m3_arith_shared_taps_and_steady_state() {
    let mut f = FullFixture::new();
    let program = ArithmeticProgram::install(&mut f, 32);

    // ADC carry.
    let add =
        run_arithmetic(&mut f, &program, u32::MAX, 0, 1, 0);
    assert_eq!(add.value, 0);
    assert_eq!(add.status, 1);
    assert_eq!(add.mode, 0);

    // SBB borrow.
    let sub = run_arithmetic(&mut f, &program, 0, 0, 1, 1);
    assert_eq!(sub.value, u32::MAX);
    assert_eq!(sub.status, 1);
    assert_eq!(sub.mode, 1);

    // A case with known sign-boundary overflow behavior; raw taps remain
    // explicit structural outputs for the next EFLAGS witness.
    let overflow =
        run_arithmetic(&mut f, &program, 0x7fff_ffff, 0, 1, 0);
    assert_eq!(overflow, expected(32, 0x7fff_ffff, 0, 1, 0));
    assert_eq!(overflow.sign_in_raw ^ overflow.final_raw, 1);

    let first =
        run_arithmetic(&mut f, &program, 0x1234_5678, 0x9abc_def0, 1, 1);
    assert_eq!(first, expected(32, 0x1234_5678, 0x9abc_def0, 1, 1));
    let links = f.store.link_count();
    let second =
        run_arithmetic(&mut f, &program, 0x1234_5678, 0x9abc_def0, 1, 1);
    assert_eq!(second, first);
    assert_eq!(
        f.store.link_count(),
        links,
        "repeated identical ARITH32 materialized new Links"
    );
}
