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
        // Width is encoded into the program's anchor namespace so multiple
        // generated widths can coexist in one semantic Memory if needed.
        for _ in 0..width {
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
struct RippleProgram {
    width: usize,
    add: Handle,
    active_steps: usize,
    links_after_build: usize,
}

impl RippleProgram {
    fn install(f: &mut FullFixture, width: usize) -> Self {
        assert!((1..=32).contains(&width));

        let seed = f.store.ensure_pair(f.full, f.k).unwrap();
        let mut anchors = AnchorGen::new(
            &mut f.store,
            seed,
            f.o,
            f.c,
            width,
        );

        let add_left = anchors.next(&mut f.store);
        let add_right = anchors.next(&mut f.store);
        let add = f.store.ensure_pair(add_left, add_right).unwrap();

        let mut stage_tags = Vec::with_capacity(width);
        for _ in 0..width {
            let left = anchors.next(&mut f.store);
            let right = anchors.next(&mut f.store);
            stage_tags.push(f.store.ensure_pair(left, right).unwrap());
        }

        let full_outputs = [
            materialize_exact_sequence(&mut f.store, &[f.zero, f.zero]).unwrap(),
            materialize_exact_sequence(&mut f.store, &[f.one, f.zero]).unwrap(),
            materialize_exact_sequence(&mut f.store, &[f.zero, f.one]).unwrap(),
            materialize_exact_sequence(&mut f.store, &[f.one, f.one]).unwrap(),
        ];

        // -------------------------------------------------------------
        // Generic ADD_N OPEN:
        //
        // K -> Call(ADD_N,[Aword,Bword,Cin])
        // =>
        // Stage0(K, remaining input positions)
        //   -> Call(FULL,[a0,b0,Cin])
        //
        // WordN itself is ExactSequence_R([b0..bN-1]), LSB first.
        // -------------------------------------------------------------
        {
            let k = anchors.next(&mut f.store);
            let a_roles = anchors.roles(&mut f.store, width);
            let b_roles = anchors.roles(&mut f.store, width);
            let cin = anchors.next(&mut f.store);

            let aword =
                materialize_exact_sequence(&mut f.store, &a_roles).unwrap();
            let bword =
                materialize_exact_sequence(&mut f.store, &b_roles).unwrap();
            let args =
                materialize_exact_sequence(&mut f.store, &[aword, bword, cin]).unwrap();

            let invocation = call(&mut f.store, f.apply, add, args);
            let before = f.store.ensure_pair(k, invocation).unwrap();

            let mut state = Vec::with_capacity(1 + 2 * (width - 1));
            state.push(k);
            state.extend_from_slice(&a_roles[1..]);
            state.extend_from_slice(&b_roles[1..]);

            let caller = stage_frame(
                &mut f.store,
                stage_tags[0],
                &state,
            );
            let fa_args =
                materialize_exact_sequence(
                    &mut f.store,
                    &[a_roles[0], b_roles[0], cin],
                )
                .unwrap();
            let fa_call = call(&mut f.store, f.apply, f.full, fa_args);
            let after = f.store.ensure_pair(caller, fa_call).unwrap();

            let mut roles = Vec::with_capacity(2 * width + 2);
            roles.push(k);
            roles.extend_from_slice(&a_roles);
            roles.extend_from_slice(&b_roles);
            roles.push(cin);

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        // -------------------------------------------------------------
        // One generated continuation per bit position.
        //
        // Every continuation is generic over runtime bit values. There is no
        // per-input Rule generation.
        // -------------------------------------------------------------
        for i in 0..width {
            let k = anchors.next(&mut f.store);
            let remaining = width - i - 1;
            let a_rem = anchors.roles(&mut f.store, remaining);
            let b_rem = anchors.roles(&mut f.store, remaining);
            let prev_sums = anchors.roles(&mut f.store, i);
            let sum_i = anchors.next(&mut f.store);
            let carry = anchors.next(&mut f.store);

            let mut before_state =
                Vec::with_capacity(1 + 2 * remaining + i);
            before_state.push(k);
            before_state.extend_from_slice(&a_rem);
            before_state.extend_from_slice(&b_rem);
            before_state.extend_from_slice(&prev_sums);

            let caller = stage_frame(
                &mut f.store,
                stage_tags[i],
                &before_state,
            );
            let fa_result =
                materialize_exact_sequence(&mut f.store, &[sum_i, carry]).unwrap();
            let before = f.store.ensure_pair(caller, fa_result).unwrap();

            let after = if i + 1 < width {
                let mut next_state =
                    Vec::with_capacity(1 + 2 * (remaining - 1) + i + 1);
                next_state.push(k);
                next_state.extend_from_slice(&a_rem[1..]);
                next_state.extend_from_slice(&b_rem[1..]);
                next_state.extend_from_slice(&prev_sums);
                next_state.push(sum_i);

                let next_caller = stage_frame(
                    &mut f.store,
                    stage_tags[i + 1],
                    &next_state,
                );
                let next_args =
                    materialize_exact_sequence(
                        &mut f.store,
                        &[a_rem[0], b_rem[0], carry],
                    )
                    .unwrap();
                let next_call =
                    call(&mut f.store, f.apply, f.full, next_args);
                f.store.ensure_pair(next_caller, next_call).unwrap()
            } else {
                let mut sums = prev_sums.clone();
                sums.push(sum_i);
                let sum_word =
                    materialize_exact_sequence(&mut f.store, &sums).unwrap();
                let final_result =
                    materialize_exact_sequence(&mut f.store, &[sum_word, carry])
                        .unwrap();
                f.store.ensure_pair(k, final_result).unwrap()
            };

            let mut roles =
                Vec::with_capacity(1 + 2 * remaining + i + 2);
            roles.push(k);
            roles.extend_from_slice(&a_rem);
            roles.extend_from_slice(&b_rem);
            roles.extend_from_slice(&prev_sums);
            roles.push(sum_i);
            roles.push(carry);

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &full_outputs, admission);
        }

        let active_steps = 1 + 14 * width;
        let links_after_build = f.store.link_count();

        Self {
            width,
            add,
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
    assert_eq!(bits.len(), width, "WordN arity");

    let mut value = 0u32;
    for (index, bit) in bits.into_iter().enumerate() {
        if bit == f.one {
            value |= 1u32 << index;
        } else {
            assert_eq!(bit, f.zero, "WordN bit must be canonical 0/1");
        }
    }
    value
}

fn run_add(
    f: &mut FullFixture,
    program: &RippleProgram,
    a: u32,
    b: u32,
    cin: u8,
) -> (u32, u8) {
    let m = mask(program.width);
    assert_eq!(a & !m, 0, "A outside declared width");
    assert_eq!(b & !m, 0, "B outside declared width");
    assert!(cin <= 1);

    let a_bits = bit_handles(f, program.width, a);
    let b_bits = bit_handles(f, program.width, b);
    let aword =
        materialize_exact_sequence(&mut f.store, &a_bits).unwrap();
    let bword =
        materialize_exact_sequence(&mut f.store, &b_bits).unwrap();
    let cin_handle = if cin == 0 { f.zero } else { f.one };

    let args =
        materialize_exact_sequence(
            &mut f.store,
            &[aword, bword, cin_handle],
        )
        .unwrap();
    let invocation = call(&mut f.store, f.apply, program.add, args);
    let initial = f.store.ensure_pair(f.k, invocation).unwrap();

    f.engine.set_current(&f.store, &[initial]).unwrap();

    for step in 0..program.active_steps {
        let reaction = f.engine.run(&mut f.store).unwrap();
        assert!(
            !reaction.quiescent,
            "N={} A={a} B={b} Cin={cin}: step {step}",
            program.width,
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

    let result_values = read_exact_sequence(&f.store, result).unwrap();
    assert_eq!(result_values.len(), 2);
    let sum = decode_word(f, program.width, result_values[0]);
    let cout = if result_values[1] == f.one {
        1
    } else {
        assert_eq!(result_values[1], f.zero);
        0
    };

    (sum, cout)
}

fn verify_case(
    f: &mut FullFixture,
    program: &RippleProgram,
    a: u32,
    b: u32,
    cin: u8,
) {
    let (sum, cout) = run_add(f, program, a, b, cin);
    let total = u64::from(a) + u64::from(b) + u64::from(cin);
    let expected_sum = (total & u64::from(mask(program.width))) as u32;
    let expected_cout = ((total >> program.width) & 1) as u8;

    assert_eq!(
        sum, expected_sum,
        "N={} A={a} B={b} Cin={cin} Sum",
        program.width
    );
    assert_eq!(
        cout, expected_cout,
        "N={} A={a} B={b} Cin={cin} Cout",
        program.width
    );
}

fn deterministic_vectors(width: usize) -> Vec<(u32, u32, u8)> {
    let m = mask(width);
    let alt_a = 0xaaaa_aaaa & m;
    let alt_5 = 0x5555_5555 & m;

    let mut out = vec![
        (0, 0, 0),
        (0, 0, 1),
        (1 & m, 1 & m, 0),
        (m, 0, 0),
        (m, 1 & m, 0),
        (m, 0, 1),
        (m, m, 0),
        (m, m, 1),
        (alt_a, alt_5, 0),
        (alt_a, alt_5, 1),
    ];

    for bit in 0..width {
        let p = 1u32 << bit;
        out.push((p, 1 & m, 0));
        out.push((p, p, 0));
    }

    let mut x = 0x9e37_79b9u32 ^ (width as u32);
    for i in 0..24u32 {
        x = x.wrapping_mul(1664525).wrapping_add(1013904223);
        let a = x & m;
        x = x.wrapping_mul(1664525).wrapping_add(1013904223);
        let b = x & m;
        out.push((a, b, (i & 1) as u8));
    }

    out.sort_unstable();
    out.dedup();
    out
}

#[test]
#[ignore = "heavy scale suite; mandatory release workflow"]
fn m3_scale_generic_n4_repeats_exhaustive_512_proof() {
    let mut f = FullFixture::new();
    let program = RippleProgram::install(&mut f, 4);

    assert_eq!(program.active_steps, 57);

    let mut cases = 0usize;
    for a in 0u32..16 {
        for b in 0u32..16 {
            for cin in 0u8..=1 {
                verify_case(&mut f, &program, a, b, cin);
                cases += 1;
            }
        }
    }

    assert_eq!(cases, 512);
    println!(
        "RIPPLE_SCALE width=4 cases=512 active_steps={} program_links={}",
        program.active_steps,
        program.links_after_build,
    );
}

#[test]
#[ignore = "heavy scale suite; mandatory release workflow"]
fn m3_scale_8_16_32_same_architecture() {
    for width in [8usize, 16, 32] {
        let mut f = FullFixture::new();
        let program = RippleProgram::install(&mut f, width);
        assert_eq!(program.active_steps, 1 + 14 * width);

        let vectors = deterministic_vectors(width);
        for &(a, b, cin) in &vectors {
            verify_case(&mut f, &program, a, b, cin);

            // Commutativity metamorphic witness for the same generated program.
            verify_case(&mut f, &program, b, a, cin);
        }

        println!(
            "RIPPLE_SCALE width={} cases={} active_steps={} program_links={}",
            width,
            vectors.len() * 2,
            program.active_steps,
            program.links_after_build,
        );
    }
}

#[test]
fn m3_scale_word_order_is_semantic_at_32_bits() {
    let mut f = FullFixture::new();
    let bits = bit_handles(&f, 32, 0x8000_0003);
    let word =
        materialize_exact_sequence(&mut f.store, &bits).unwrap();
    assert_eq!(decode_word(&f, 32, word), 0x8000_0003);

    let reversed = bits.iter().copied().rev().collect::<Vec<_>>();
    let reversed_word =
        materialize_exact_sequence(&mut f.store, &reversed).unwrap();
    assert_ne!(word, reversed_word);
    assert_eq!(decode_word(&f, 32, reversed_word), 0xc000_0001);

    // Use a non-palindromic pattern to prove positional direction too.
    let bits = bit_handles(&f, 32, 0x0000_0003);
    let word =
        materialize_exact_sequence(&mut f.store, &bits).unwrap();
    let reversed = bits.iter().copied().rev().collect::<Vec<_>>();
    let reversed_word =
        materialize_exact_sequence(&mut f.store, &reversed).unwrap();

    assert_eq!(decode_word(&f, 32, word), 0x0000_0003);
    assert_eq!(decode_word(&f, 32, reversed_word), 0xc000_0000);
}
