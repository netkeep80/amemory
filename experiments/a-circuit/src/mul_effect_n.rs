use super::{
    flag_patch::{
        install_wide_alu_effect_result_tag, set_flag_action,
        undefined_flag_action, FlagPatchSchema,
    },
    full_adder::{
        call, define_bundle_rule, index_rule_for, Fixture as FullFixture,
    },
    logic_n::{install_gate_basis, GateSet},
    mul32_n::Mul32Program,
};
use amemory_optimized_cpu_probe::{
    structural::{materialize_exact_sequence, read_exact_sequence},
    Handle, OptimizedLinkStore,
};

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
        for pole in [c,o,o,c,o,c,c,c,o,o,c,c,o,c,o,o,c,o] {
            seed = store.ensure_pair(seed, pole).unwrap();
        }
        Self { current: seed, flip: false, o, c }
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
struct MulEffectProgram {
    effect: Handle,
    result_tag: Handle,
    schema: FlagPatchSchema,
    gates: GateSet,
    mul: Mul32Program,
    links_after_build: usize,
}

impl MulEffectProgram {
    fn install(f: &mut FullFixture) -> Self {
        let mul = Mul32Program::install(f);
        let gates = install_gate_basis(f);

        let seed = f.store.ensure_pair(mul.mul32, mul.result_tag).unwrap();
        let mut anchors = AnchorGen::new(&mut f.store, seed, f.o, f.c);

        let left = anchors.next(&mut f.store);
        let right = anchors.next(&mut f.store);
        let effect = f.store.ensure_pair(left, right).unwrap();

        let mul_return_tag = anchors.next(&mut f.store);

        let mut or_tags = Vec::with_capacity(WIDTH - 1);
        for _ in 0..(WIDTH - 1) {
            let left = anchors.next(&mut f.store);
            let right = anchors.next(&mut f.store);
            or_tags.push(f.store.ensure_pair(left, right).unwrap());
        }

        let shared_seed = f.store.ensure_pair(f.k, f.full).unwrap();
        let schema =
            FlagPatchSchema::install(&mut f.store, shared_seed, f.o, f.c);
        let result_tag = install_wide_alu_effect_result_tag(
            &mut f.store, shared_seed, f.o, f.c,
        );

        // EFFECT OPEN -> raw MUL32.
        {
            let k = anchors.next(&mut f.store);
            let aword = anchors.next(&mut f.store);
            let bword = anchors.next(&mut f.store);

            let args =
                materialize_exact_sequence(&mut f.store, &[aword, bword])
                    .unwrap();
            let before_call =
                call(&mut f.store, f.apply, effect, args);
            let before = f.store.ensure_pair(k, before_call).unwrap();

            let caller =
                stage_frame(&mut f.store, mul_return_tag, &[k]);
            let mul_call =
                call(&mut f.store, f.apply, mul.mul32, args);
            let after = f.store.ensure_pair(caller, mul_call).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, aword, bword],
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        // Raw Product64 -> begin OR reduction over HiWord32.
        {
            let k = anchors.next(&mut f.store);
            let lo = anchors.next(&mut f.store);
            let hi_bits = anchors.roles(&mut f.store, WIDTH);

            let hi =
                materialize_exact_sequence(&mut f.store, &hi_bits).unwrap();
            let wide =
                materialize_exact_sequence(&mut f.store, &[lo, hi]).unwrap();
            let mul_payload =
                materialize_exact_sequence(&mut f.store, &[wide]).unwrap();
            let mul_endpoint =
                f.store.ensure_pair(mul.result_tag, mul_payload).unwrap();

            let caller =
                stage_frame(&mut f.store, mul_return_tag, &[k]);
            let before =
                f.store.ensure_pair(caller, mul_endpoint).unwrap();

            let mut state = Vec::with_capacity(WIDTH + 2);
            state.push(k);
            state.push(lo);
            state.extend_from_slice(&hi_bits);

            let next_caller =
                stage_frame(&mut f.store, or_tags[0], &state);
            let or_call =
                binary_call(f, gates.or2, hi_bits[0], hi_bits[1]);
            let after =
                f.store.ensure_pair(next_caller, or_call).unwrap();

            let mut roles = Vec::with_capacity(WIDTH + 2);
            roles.push(k);
            roles.push(lo);
            roles.extend_from_slice(&hi_bits);

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[mul.result_tag], admission);
        }

        // OR-reduce the high word. The final accumulator is high_nonzero.
        for i in 0..(WIDTH - 1) {
            let k = anchors.next(&mut f.store);
            let lo = anchors.next(&mut f.store);
            let hi_bits = anchors.roles(&mut f.store, WIDTH);
            let acc = anchors.next(&mut f.store);

            let mut state = Vec::with_capacity(WIDTH + 2);
            state.push(k);
            state.push(lo);
            state.extend_from_slice(&hi_bits);

            let caller =
                stage_frame(&mut f.store, or_tags[i], &state);
            let acc_result =
                materialize_exact_sequence(&mut f.store, &[acc]).unwrap();
            let before =
                f.store.ensure_pair(caller, acc_result).unwrap();

            let after = if i + 1 < WIDTH - 1 {
                let next_caller =
                    stage_frame(&mut f.store, or_tags[i + 1], &state);
                let next_call = binary_call(
                    f,
                    gates.or2,
                    acc,
                    hi_bits[i + 2],
                );
                f.store.ensure_pair(next_caller, next_call).unwrap()
            } else {
                let hi =
                    materialize_exact_sequence(&mut f.store, &hi_bits).unwrap();
                let wide =
                    materialize_exact_sequence(&mut f.store, &[lo, hi]).unwrap();

                let set_cf =
                    set_flag_action(&mut f.store, schema, schema.cf, acc);
                let undef_pf =
                    undefined_flag_action(&mut f.store, schema, schema.pf);
                let undef_af =
                    undefined_flag_action(&mut f.store, schema, schema.af);
                let undef_zf =
                    undefined_flag_action(&mut f.store, schema, schema.zf);
                let undef_sf =
                    undefined_flag_action(&mut f.store, schema, schema.sf);
                let set_of =
                    set_flag_action(&mut f.store, schema, schema.of, acc);

                let patch = materialize_exact_sequence(
                    &mut f.store,
                    &[set_cf, undef_pf, undef_af, undef_zf, undef_sf, set_of],
                )
                .unwrap();
                let payload =
                    materialize_exact_sequence(&mut f.store, &[wide, patch])
                        .unwrap();
                let endpoint =
                    f.store.ensure_pair(result_tag, payload).unwrap();
                f.store.ensure_pair(k, endpoint).unwrap()
            };

            let mut roles = state;
            roles.push(acc);

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

        Self {
            effect,
            result_tag,
            schema,
            gates,
            mul,
            links_after_build: f.store.link_count(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FlagState {
    Set(u8),
    Undefined,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Outcome {
    product: u64,
    cf: FlagState,
    pf: FlagState,
    af: FlagState,
    zf: FlagState,
    sf: FlagState,
    of: FlagState,
    reactions: usize,
}

fn word_handle(f: &mut FullFixture, value: u32) -> Handle {
    let bits: Vec<Handle> = (0..WIDTH)
        .map(|i| if (value >> i) & 1 == 1 { f.one } else { f.zero })
        .collect();
    materialize_exact_sequence(&mut f.store, &bits).unwrap()
}

fn decode_bit(f: &FullFixture, bit: Handle) -> u8 {
    if bit == f.one {
        1
    } else {
        assert_eq!(bit, f.zero);
        0
    }
}

fn decode_word(f: &FullFixture, word: Handle) -> u32 {
    let bits = read_exact_sequence(&f.store, word).unwrap();
    assert_eq!(bits.len(), WIDTH);
    let mut value = 0u32;
    for (i, bit) in bits.into_iter().enumerate() {
        value |= u32::from(decode_bit(f, bit)) << i;
    }
    value
}

fn decode_wide(f: &FullFixture, wide: Handle) -> u64 {
    let halves = read_exact_sequence(&f.store, wide).unwrap();
    assert_eq!(halves.len(), 2);
    u64::from(decode_word(f, halves[0]))
        | (u64::from(decode_word(f, halves[1])) << 32)
}

fn decode_action(
    f: &FullFixture,
    schema: FlagPatchSchema,
    action: Handle,
    flag: Handle,
) -> FlagState {
    let (tag, payload) = f.store.poles(action).unwrap();
    let values = read_exact_sequence(&f.store, payload).unwrap();

    if tag == schema.set_tag {
        assert_eq!(values.len(), 2);
        assert_eq!(values[0], flag);
        FlagState::Set(decode_bit(f, values[1]))
    } else {
        assert_eq!(tag, schema.undefined_tag);
        assert_eq!(values, vec![flag]);
        FlagState::Undefined
    }
}

fn expected_steps(b: u32) -> usize {
    97 + 1038 * (b.count_ones() as usize)
}

fn run(
    f: &mut FullFixture,
    p: &MulEffectProgram,
    a: u32,
    b: u32,
) -> Outcome {
    let aword = word_handle(f, a);
    let bword = word_handle(f, b);
    let args =
        materialize_exact_sequence(&mut f.store, &[aword, bword]).unwrap();
    let invocation =
        call(&mut f.store, f.apply, p.effect, args);
    let initial = f.store.ensure_pair(f.k, invocation).unwrap();
    f.engine.set_current(&f.store, &[initial]).unwrap();

    let reactions = expected_steps(b);
    for step in 0..reactions {
        let r = f.engine.run(&mut f.store).unwrap();
        assert!(
            !r.quiescent,
            "MUL_EFFECT A={a:#x} B={b:#x}: quiescent at {step}/{reactions}"
        );
        assert_eq!(r.raw_rule_matches, 1, "step {step}");
        assert_eq!(r.transitioned_members, 1, "step {step}");
        assert_eq!(r.handoff_count, 1, "step {step}");
        assert_eq!(r.next_members.len(), 1, "step {step}");
    }

    let stable = f.engine.current_bank();
    let q = f.engine.run(&mut f.store).unwrap();
    assert!(q.quiescent);
    assert_eq!(q.raw_rule_matches, 0);
    assert_eq!(q.handoff_count, 0);
    assert_eq!(f.engine.current_bank(), stable);
    assert_eq!(f.engine.current().len(), 1);

    let (caller, endpoint) =
        f.store.poles(f.engine.current()[0]).unwrap();
    assert_eq!(caller, f.k);
    let (tag, payload) = f.store.poles(endpoint).unwrap();
    assert_eq!(tag, p.result_tag);
    let values = read_exact_sequence(&f.store, payload).unwrap();
    assert_eq!(values.len(), 2);

    let product = decode_wide(f, values[0]);
    let patch = read_exact_sequence(&f.store, values[1]).unwrap();
    assert_eq!(patch.len(), 6);

    let s = p.schema;
    Outcome {
        product,
        cf: decode_action(f, s, patch[0], s.cf),
        pf: decode_action(f, s, patch[1], s.pf),
        af: decode_action(f, s, patch[2], s.af),
        zf: decode_action(f, s, patch[3], s.zf),
        sf: decode_action(f, s, patch[4], s.sf),
        of: decode_action(f, s, patch[5], s.of),
        reactions,
    }
}

fn expected(a: u32, b: u32) -> Outcome {
    let product = u64::from(a) * u64::from(b);
    let overflow = u8::from((product >> 32) != 0);

    Outcome {
        product,
        cf: FlagState::Set(overflow),
        pf: FlagState::Undefined,
        af: FlagState::Undefined,
        zf: FlagState::Undefined,
        sf: FlagState::Undefined,
        of: FlagState::Set(overflow),
        reactions: expected_steps(b),
    }
}

#[test]
#[ignore = "heavy x86 MUL effect suite; dedicated workflow"]
fn m4_mul_effect_high_half_drives_cf_of() {
    let mut f = FullFixture::new();
    let p = MulEffectProgram::install(&mut f);

    let cases = [
        (0u32, 0xdead_beefu32),
        (0xffffu32, 0xffffu32),
        (u32::MAX, 2u32),
        (0x8000_0000u32, 2u32),
        (0x1234_5678u32, 0x0001_0001u32),
        (u32::MAX, u32::MAX),
    ];

    for (a, b) in cases {
        let actual = run(&mut f, &p, a, b);
        assert_eq!(actual, expected(a, b), "A={a:#x} B={b:#x}");
    }

    println!(
        "M4_MUL_EFFECT cases={} worst_reactions={} program_links={} mul_links={} or_gate={}",
        cases.len(),
        expected_steps(u32::MAX),
        p.links_after_build,
        p.mul.links_after_build,
        p.gates.or2,
    );
}

#[test]
#[ignore = "heavy x86 MUL effect suite; dedicated workflow"]
fn m4_mul_effect_steady_state_and_undefined_flags() {
    let mut f = FullFixture::new();
    let p = MulEffectProgram::install(&mut f);

    let a = 0x1357_9bdfu32;
    let b = 1u32;

    let first = run(&mut f, &p, a, b);
    assert_eq!(first, expected(a, b));
    assert_eq!(first.cf, FlagState::Set(0));
    assert_eq!(first.of, FlagState::Set(0));
    assert_eq!(first.pf, FlagState::Undefined);
    assert_eq!(first.af, FlagState::Undefined);
    assert_eq!(first.zf, FlagState::Undefined);
    assert_eq!(first.sf, FlagState::Undefined);

    let links = f.store.link_count();
    let second = run(&mut f, &p, a, b);
    assert_eq!(second, first);
    assert_eq!(
        f.store.link_count(),
        links,
        "repeated identical MUL effect materialized new Links"
    );
}
