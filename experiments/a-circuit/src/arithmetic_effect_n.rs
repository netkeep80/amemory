use super::{
    flag_patch::{
        install_alu_effect_result_tag, set_flag_action, FlagPatchSchema,
    },
    flags_n::FlaggedArithmeticProgram,
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
        seed = store.ensure_pair(seed, c).unwrap();
        seed = store.ensure_pair(seed, c).unwrap();
        seed = store.ensure_pair(seed, o).unwrap();
        seed = store.ensure_pair(seed, c).unwrap();
        seed = store.ensure_pair(seed, o).unwrap();
        for _ in 0..width {
            seed = store.ensure_pair(seed, o).unwrap();
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
struct ArithmeticEffectProgram {
    width: usize,
    effect: Handle,
    result_tag: Handle,
    schema: FlagPatchSchema,
    flagged: FlaggedArithmeticProgram,
    active_steps: usize,
    links_after_build: usize,
}

impl ArithmeticEffectProgram {
    fn install(f: &mut FullFixture, width: usize) -> Self {
        assert!((8..=32).contains(&width));

        let flagged = FlaggedArithmeticProgram::install(f, width);
        let seed0 = f
            .store
            .ensure_pair(flagged.flagged, flagged.result_tag)
            .unwrap();
        let mut anchors = AnchorGen::new(
            &mut f.store,
            seed0,
            f.o,
            f.c,
            width,
        );

        let effect_left = anchors.next(&mut f.store);
        let effect_right = anchors.next(&mut f.store);
        let effect = f.store.ensure_pair(effect_left, effect_right).unwrap();

        let result_frame_tag = anchors.next(&mut f.store);

        let shared_seed = f.store.ensure_pair(f.k, f.full).unwrap();
        let schema =
            FlagPatchSchema::install(&mut f.store, shared_seed, f.o, f.c);
        let result_tag =
            install_alu_effect_result_tag(&mut f.store, shared_seed, f.o, f.c);

        // ARITH_EFFECT OPEN:
        //
        // K -> Call(EFFECT,[Aword,Bword,X,Mode,WriteBack])
        // =>
        // ResultFrame([K,WriteBack])
        //   -> Call(FLAGGED_ARITH,[Aword,Bword,X,Mode])
        {
            let k = anchors.next(&mut f.store);
            let aword = anchors.next(&mut f.store);
            let bword = anchors.next(&mut f.store);
            let x = anchors.next(&mut f.store);
            let mode = anchors.next(&mut f.store);
            let writeback = anchors.next(&mut f.store);

            let args = materialize_exact_sequence(
                &mut f.store,
                &[aword, bword, x, mode, writeback],
            )
            .unwrap();
            let before_call =
                call(&mut f.store, f.apply, effect, args);
            let before = f.store.ensure_pair(k, before_call).unwrap();

            let caller = stage_frame(
                &mut f.store,
                result_frame_tag,
                &[k, writeback],
            );
            let flagged_args = materialize_exact_sequence(
                &mut f.store,
                &[aword, bword, x, mode],
            )
            .unwrap();
            let flagged_call = call(
                &mut f.store,
                f.apply,
                flagged.flagged,
                flagged_args,
            );
            let after = f.store.ensure_pair(caller, flagged_call).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, aword, bword, x, mode, writeback],
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        // FLAGGED_RESULT -> shared ALU_EFFECT_RESULT.
        {
            let k = anchors.next(&mut f.store);
            let writeback = anchors.next(&mut f.store);
            let word = anchors.next(&mut f.store);
            let cf = anchors.next(&mut f.store);
            let pf = anchors.next(&mut f.store);
            let af = anchors.next(&mut f.store);
            let zf = anchors.next(&mut f.store);
            let sf = anchors.next(&mut f.store);
            let of = anchors.next(&mut f.store);

            let flagged_payload = materialize_exact_sequence(
                &mut f.store,
                &[word, cf, pf, af, zf, sf, of],
            )
            .unwrap();
            let flagged_envelope = f
                .store
                .ensure_pair(flagged.result_tag, flagged_payload)
                .unwrap();

            let caller = stage_frame(
                &mut f.store,
                result_frame_tag,
                &[k, writeback],
            );
            let before =
                f.store.ensure_pair(caller, flagged_envelope).unwrap();

            let set_cf =
                set_flag_action(&mut f.store, schema, schema.cf, cf);
            let set_pf =
                set_flag_action(&mut f.store, schema, schema.pf, pf);
            let set_af =
                set_flag_action(&mut f.store, schema, schema.af, af);
            let set_zf =
                set_flag_action(&mut f.store, schema, schema.zf, zf);
            let set_sf =
                set_flag_action(&mut f.store, schema, schema.sf, sf);
            let set_of =
                set_flag_action(&mut f.store, schema, schema.of, of);

            let patch = materialize_exact_sequence(
                &mut f.store,
                &[set_cf, set_pf, set_af, set_zf, set_sf, set_of],
            )
            .unwrap();
            let payload = materialize_exact_sequence(
                &mut f.store,
                &[writeback, word, patch],
            )
            .unwrap();
            let envelope =
                f.store.ensure_pair(result_tag, payload).unwrap();
            let after = f.store.ensure_pair(k, envelope).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, writeback, word, cf, pf, af, zf, sf, of],
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &[flagged.result_tag],
                admission,
            );
        }

        let active_steps = flagged.active_steps + 2;

        Self {
            width,
            effect,
            result_tag,
            schema,
            flagged,
            active_steps,
            links_after_build: f.store.link_count(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EffectOutcome {
    writeback: u8,
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

fn decode_bit(f: &FullFixture, value: Handle) -> u8 {
    if value == f.one {
        1
    } else {
        assert_eq!(value, f.zero);
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

fn decode_set(
    f: &FullFixture,
    schema: FlagPatchSchema,
    action: Handle,
    flag: Handle,
) -> u8 {
    let (tag, payload) = f.store.poles(action).unwrap();
    assert_eq!(tag, schema.set_tag);
    let values = read_exact_sequence(&f.store, payload).unwrap();
    assert_eq!(values.len(), 2);
    assert_eq!(values[0], flag);
    decode_bit(f, values[1])
}

fn decode_effect(
    f: &FullFixture,
    program: &ArithmeticEffectProgram,
) -> EffectOutcome {
    assert_eq!(f.engine.current().len(), 1);
    let final_link = f.engine.current()[0];
    let (caller, endpoint) = f.store.poles(final_link).unwrap();
    assert_eq!(caller, f.k);

    let (tag, payload) = f.store.poles(endpoint).unwrap();
    assert_eq!(tag, program.result_tag);
    let values = read_exact_sequence(&f.store, payload).unwrap();
    assert_eq!(values.len(), 3);

    let writeback = decode_bit(f, values[0]);
    let value = decode_word(f, program.width, values[1]);
    let patch = read_exact_sequence(&f.store, values[2]).unwrap();
    assert_eq!(patch.len(), 6);

    let s = program.schema;
    EffectOutcome {
        writeback,
        value,
        cf: decode_set(f, s, patch[0], s.cf),
        pf: decode_set(f, s, patch[1], s.pf),
        af: decode_set(f, s, patch[2], s.af),
        zf: decode_set(f, s, patch[3], s.zf),
        sf: decode_set(f, s, patch[4], s.sf),
        of: decode_set(f, s, patch[5], s.of),
    }
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
        assert_eq!(reaction.handoff_count, 1, "{label} step {step} handoff");
        assert_eq!(reaction.next_members.len(), 1, "{label} step {step} Scope");
    }

    let stable = f.engine.current_bank();
    let quiescent = f.engine.run(&mut f.store).unwrap();
    assert!(quiescent.quiescent, "{label}: final quiescence");
    assert_eq!(quiescent.raw_rule_matches, 0);
    assert_eq!(quiescent.handoff_count, 0);
    assert_eq!(f.engine.current_bank(), stable);
}

fn run_effect(
    f: &mut FullFixture,
    program: &ArithmeticEffectProgram,
    a: u32,
    b: u32,
    x: u8,
    mode: u8,
    writeback: u8,
) -> EffectOutcome {
    let m = mask(program.width);
    assert_eq!(a & !m, 0);
    assert_eq!(b & !m, 0);
    assert!(x <= 1 && mode <= 1 && writeback <= 1);

    let a_bits = bit_handles(f, program.width, a);
    let b_bits = bit_handles(f, program.width, b);
    let aword =
        materialize_exact_sequence(&mut f.store, &a_bits).unwrap();
    let bword =
        materialize_exact_sequence(&mut f.store, &b_bits).unwrap();
    let xh = if x == 0 { f.zero } else { f.one };
    let mh = if mode == 0 { f.zero } else { f.one };
    let wbh = if writeback == 0 { f.zero } else { f.one };

    let args = materialize_exact_sequence(
        &mut f.store,
        &[aword, bword, xh, mh, wbh],
    )
    .unwrap();
    let invocation =
        call(&mut f.store, f.apply, program.effect, args);
    let initial = f.store.ensure_pair(f.k, invocation).unwrap();
    f.engine.set_current(&f.store, &[initial]).unwrap();

    run_steps(
        f,
        program.active_steps,
        &format!("ARITH_EFFECT_N={}", program.width),
    );
    decode_effect(f, program)
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

fn expected(
    width: usize,
    a: u32,
    b: u32,
    x: u8,
    mode: u8,
    writeback: u8,
) -> EffectOutcome {
    let m = mask(width);
    let wide_mask = u64::from(m);

    let (value, cf) = if mode == 0 {
        let total = u64::from(a) + u64::from(b) + u64::from(x);
        ((total & wide_mask) as u32, u8::from(total > wide_mask))
    } else {
        let subtrahend = u64::from(b) + u64::from(x);
        (
            a.wrapping_sub(b).wrapping_sub(u32::from(x)) & m,
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

    EffectOutcome {
        writeback,
        value,
        cf,
        pf: u8::from((value as u8).count_ones() % 2 == 0),
        af,
        zf: u8::from(value == 0),
        sf: ((value >> (width - 1)) & 1) as u8,
        of: u8::from(signed_math < min || signed_math > max),
    }
}

fn vectors(width: usize) -> Vec<(u32, u32, u8, u8)> {
    let m = mask(width);
    let sign = 1u32 << (width - 1);
    let max_pos = sign - 1;
    let mut out = vec![
        (0, 0, 0, 0),
        (m, 0, 1, 0),
        (0, 0, 1, 1),
        (0x0f & m, 0, 1, 0),
        (0x10 & m, 1, 0, 1),
        (max_pos, 0, 1, 0),
        (sign, 0, 1, 1),
        (0x55 & m, 0xaa & m, 0, 0),
    ];

    let mut z = 0x27d4_eb2du32 ^ width as u32;
    for i in 0..10u32 {
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


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WebArithmeticOutcome {
    pub(crate) value: u32,
    pub(crate) writeback: u8,
    pub(crate) defined_mask: u32,
    pub(crate) value_mask: u32,
    pub(crate) undefined_mask: u32,
    pub(crate) preserve_mask: u32,
    pub(crate) reactions: u32,
    pub(crate) links_after_build: u32,
    pub(crate) links_after_first: u32,
    pub(crate) steady_link_delta: u32,
    pub(crate) quiescent: u8,
}

const WEB_CF: u32 = 1 << 0;
const WEB_PF: u32 = 1 << 2;
const WEB_AF: u32 = 1 << 4;
const WEB_ZF: u32 = 1 << 6;
const WEB_SF: u32 = 1 << 7;
const WEB_OF: u32 = 1 << 11;
const WEB_STATUS_FLAGS: u32 = WEB_CF | WEB_PF | WEB_AF | WEB_ZF | WEB_SF | WEB_OF;

fn web_arithmetic_masks(out: EffectOutcome) -> (u32, u32) {
    let mut values = 0u32;
    for (mask, bit) in [
        (WEB_CF, out.cf),
        (WEB_PF, out.pf),
        (WEB_AF, out.af),
        (WEB_ZF, out.zf),
        (WEB_SF, out.sf),
        (WEB_OF, out.of),
    ] {
        if bit != 0 {
            values |= mask;
        }
    }
    (WEB_STATUS_FLAGS, values)
}

pub(crate) fn web_run_arithmetic(
    op: u32,
    a: u32,
    b: u32,
    input_flag: u32,
) -> Option<WebArithmeticOutcome> {
    if input_flag > 1 {
        return None;
    }
    let mut f = FullFixture::new();
    let program = ArithmeticEffectProgram::install(&mut f, 32);
    let links_after_build = f.store.link_count() as u32;

    let (x, mode, writeback) = match op {
        6 => (0u8, 0u8, 1u8),                 // ADD
        7 => (input_flag as u8, 0u8, 1u8),   // ADC
        8 => (0u8, 1u8, 1u8),                 // SUB
        9 => (input_flag as u8, 1u8, 1u8),   // SBB
        10 => (0u8, 1u8, 0u8),                // CMP
        _ => return None,
    };

    let first = run_effect(&mut f, &program, a, b, x, mode, writeback);
    let links_after_first = f.store.link_count() as u32;
    let second = run_effect(&mut f, &program, a, b, x, mode, writeback);
    assert_eq!(second, first, "web arithmetic repeat changed result");
    let links_after_second = f.store.link_count() as u32;
    let (defined_mask, value_mask) = web_arithmetic_masks(first);

    Some(WebArithmeticOutcome {
        value: first.value,
        writeback: first.writeback,
        defined_mask,
        value_mask,
        undefined_mask: 0,
        preserve_mask: 0,
        reactions: program.active_steps as u32,
        links_after_build,
        links_after_first,
        steady_link_delta: links_after_second - links_after_first,
        quiescent: 1,
    })
}


#[test]
#[ignore = "heavy M4 arithmetic-effect suite; mandatory release workflow"]
fn m4_arith_effect_8_16_32_matches_oracle() {
    for width in [8usize, 16, 32] {
        let mut f = FullFixture::new();
        let program = ArithmeticEffectProgram::install(&mut f, width);
        assert_eq!(program.active_steps, 33 + 18 * width);

        let cases = vectors(width);
        for &(a, b, x, mode) in &cases {
            let actual =
                run_effect(&mut f, &program, a, b, x, mode, 1);
            assert_eq!(
                actual,
                expected(width, a, b, x, mode, 1),
                "width={width} A={a:#x} B={b:#x} X={x} M={mode}"
            );
        }

        println!(
            "M4_ARITH_EFFECT width={} cases={} active_steps={} program_links={} flagged_links={}",
            width,
            cases.len(),
            program.active_steps,
            program.links_after_build,
            program.flagged.links_after_build,
        );
    }
}

#[test]
#[ignore = "heavy M4 arithmetic-effect suite; mandatory release workflow"]
fn m4_arith_effect_cmp_is_sub_without_writeback_and_steady_state() {
    let mut f = FullFixture::new();
    let program = ArithmeticEffectProgram::install(&mut f, 32);

    let a = 0x8000_0000u32;
    let b = 1u32;

    let sub = run_effect(&mut f, &program, a, b, 0, 1, 1);
    let cmp = run_effect(&mut f, &program, a, b, 0, 1, 0);

    assert_eq!(sub.value, cmp.value);
    assert_eq!(sub.cf, cmp.cf);
    assert_eq!(sub.pf, cmp.pf);
    assert_eq!(sub.af, cmp.af);
    assert_eq!(sub.zf, cmp.zf);
    assert_eq!(sub.sf, cmp.sf);
    assert_eq!(sub.of, cmp.of);
    assert_eq!(sub.writeback, 1);
    assert_eq!(cmp.writeback, 0);
    assert_eq!(cmp, expected(32, a, b, 0, 1, 0));

    let first = run_effect(
        &mut f,
        &program,
        0x1234_5678,
        0x9abc_def0,
        1,
        1,
        1,
    );
    let links = f.store.link_count();
    let second = run_effect(
        &mut f,
        &program,
        0x1234_5678,
        0x9abc_def0,
        1,
        1,
        1,
    );
    assert_eq!(second, first);
    assert_eq!(
        f.store.link_count(),
        links,
        "repeated identical ARITH_EFFECT32 materialized new Links"
    );
}
