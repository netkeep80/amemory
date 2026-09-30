use super::{
    flag_patch::{
        install_alu_effect_result_tag, set_flag_action, FlagPatchSchema,
    },
    flags_n::FlaggedArithmeticProgram,
    full_adder::{
        call, define_bundle_rule, index_rule_for, Fixture as FullFixture,
    },
    proof_n::{
        execute_session_to_quiescence, execute_to_quiescence,
        identical_rerun, load_runtime, load_runtime_session, loaded_handle,
        prepare_stage, semantic_source, theory_admissions, visual_snapshot,
        ProofRuntimeMemory, ProofRuntimeSession, WebProofLoadStage,
        WebProofPrepareStage, WebProofResultStage, WebStructuralProof,
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


#[derive(Clone, Copy, Debug)]
pub(crate) struct PreparedArithmeticEffectCall {
    pub(crate) invocation: Handle,
    pub(crate) function: Handle,
    pub(crate) result_tag: Handle,
    pub(crate) active_steps: usize,
}

pub(crate) fn prepare_arithmetic_effect_call(
    f: &mut FullFixture,
    a: u32,
    b: u32,
    x: u8,
    mode: u8,
    writeback: u8,
) -> Option<PreparedArithmeticEffectCall> {
    if x > 1 || mode > 1 || writeback > 1 {
        return None;
    }

    let program = ArithmeticEffectProgram::install(f, 32);
    let a_bits = bit_handles(f, 32, a);
    let b_bits = bit_handles(f, 32, b);
    let aword =
        materialize_exact_sequence(&mut f.store, &a_bits).ok()?;
    let bword =
        materialize_exact_sequence(&mut f.store, &b_bits).ok()?;
    let xh = if x == 0 { f.zero } else { f.one };
    let mh = if mode == 0 { f.zero } else { f.one };
    let wbh = if writeback == 0 { f.zero } else { f.one };
    let args = materialize_exact_sequence(
        &mut f.store,
        &[aword, bword, xh, mh, wbh],
    )
    .ok()?;
    let invocation =
        call(&mut f.store, f.apply, program.effect, args);

    Some(PreparedArithmeticEffectCall {
        invocation,
        function: program.effect,
        result_tag: program.result_tag,
        active_steps: program.active_steps,
    })
}

#[derive(Clone, Debug)]
pub(crate) struct WebArithmeticProofExecution {
    pub(crate) outcome: WebArithmeticOutcome,
    pub(crate) proof: WebStructuralProof,
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

fn runtime_bit(
    memory: &ProofRuntimeMemory,
    bit: Handle,
    zero: Handle,
    one: Handle,
) -> Option<u8> {
    let _ = memory;
    if bit == one {
        Some(1)
    } else if bit == zero {
        Some(0)
    } else {
        None
    }
}

fn runtime_word(
    memory: &ProofRuntimeMemory,
    width: usize,
    word: Handle,
    zero: Handle,
    one: Handle,
) -> Option<u32> {
    let bits = read_exact_sequence(&memory.store, word).ok()?;
    if bits.len() != width {
        return None;
    }
    let mut value = 0u32;
    for (index, bit) in bits.into_iter().enumerate() {
        value |= u32::from(runtime_bit(memory, bit, zero, one)?) << index;
    }
    Some(value)
}

fn runtime_set(
    memory: &ProofRuntimeMemory,
    action: Handle,
    set_tag: Handle,
    expected_flag: Handle,
    zero: Handle,
    one: Handle,
) -> Option<u8> {
    let (tag, payload) = memory.store.poles(action).ok()?;
    if tag != set_tag {
        return None;
    }
    let values = read_exact_sequence(&memory.store, payload).ok()?;
    if values.len() != 2 || values[0] != expected_flag {
        return None;
    }
    runtime_bit(memory, values[1], zero, one)
}

#[allow(clippy::too_many_arguments)]
fn decode_runtime_arithmetic_effect(
    memory: &ProofRuntimeMemory,
    final_link: Handle,
    caller: Handle,
    result_tag: Handle,
    zero: Handle,
    one: Handle,
    set_tag: Handle,
    cf_flag: Handle,
    pf_flag: Handle,
    af_flag: Handle,
    zf_flag: Handle,
    sf_flag: Handle,
    of_flag: Handle,
    width: usize,
) -> Option<(EffectOutcome, Handle)> {
    let (final_caller, endpoint) = memory.store.poles(final_link).ok()?;
    if final_caller != caller {
        return None;
    }
    let (tag, payload) = memory.store.poles(endpoint).ok()?;
    if tag != result_tag {
        return None;
    }
    let values = read_exact_sequence(&memory.store, payload).ok()?;
    if values.len() != 3 {
        return None;
    }

    let writeback = runtime_bit(memory, values[0], zero, one)?;
    let value = runtime_word(memory, width, values[1], zero, one)?;
    let patch = read_exact_sequence(&memory.store, values[2]).ok()?;
    if patch.len() != 6 {
        return None;
    }

    Some((
        EffectOutcome {
            writeback,
            value,
            cf: runtime_set(memory, patch[0], set_tag, cf_flag, zero, one)?,
            pf: runtime_set(memory, patch[1], set_tag, pf_flag, zero, one)?,
            af: runtime_set(memory, patch[2], set_tag, af_flag, zero, one)?,
            zf: runtime_set(memory, patch[3], set_tag, zf_flag, zero, one)?,
            sf: runtime_set(memory, patch[4], set_tag, sf_flag, zero, one)?,
            of: runtime_set(memory, patch[5], set_tag, of_flag, zero, one)?,
        },
        payload,
    ))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Arithmetic32SessionProjection {
    pub(crate) value: u32,
    pub(crate) writeback: u8,
    pub(crate) defined_mask: u32,
    pub(crate) value_mask: u32,
    pub(crate) undefined_mask: u32,
    pub(crate) preserve_mask: u32,
    pub(crate) result_recursive_wire: String,
}

pub(crate) type Add32SessionProjection = Arithmetic32SessionProjection;

pub(crate) fn prepare_arithmetic32_session_program(
) -> Option<WebProofPrepareStage> {
    let mut compiler = FullFixture::new();
    let program = ArithmeticEffectProgram::install(&mut compiler, 32);

    // One static arithmetic Aset supports ADD/ADC/SUB/SBB/CMP.
    // Concrete A/B, carry/borrow input, mode, writeback, invocation and
    // initial Scope are runtime CONFIGURE data after LOAD.
    let prepared_roots = vec![
        semantic_source(
            &compiler.store,
            "function.effect.arithmetic",
            program.effect,
        ),
        semantic_source(
            &compiler.store,
            "function.flagged_arithmetic",
            program.flagged.flagged,
        ),
        semantic_source(
            &compiler.store,
            "result.flagged_tag",
            program.flagged.result_tag,
        ),
        semantic_source(
            &compiler.store,
            "data.bit.zero",
            compiler.zero,
        ),
        semantic_source(
            &compiler.store,
            "data.bit.one",
            compiler.one,
        ),
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
        semantic_source(
            &compiler.store,
            "context.caller",
            compiler.k,
        ),
        semantic_source(
            &compiler.store,
            "result.tag",
            program.result_tag,
        ),
        semantic_source(
            &compiler.store,
            "result.flag.set_tag",
            program.schema.set_tag,
        ),
        semantic_source(
            &compiler.store,
            "result.flag.cf",
            program.schema.cf,
        ),
        semantic_source(
            &compiler.store,
            "result.flag.pf",
            program.schema.pf,
        ),
        semantic_source(
            &compiler.store,
            "result.flag.af",
            program.schema.af,
        ),
        semantic_source(
            &compiler.store,
            "result.flag.zf",
            program.schema.zf,
        ),
        semantic_source(
            &compiler.store,
            "result.flag.sf",
            program.schema.sf,
        ),
        semantic_source(
            &compiler.store,
            "result.flag.of",
            program.schema.of,
        ),
    ];
    let admissions =
        theory_admissions(&compiler.store, compiler.theory)?;
    Some(prepare_stage(
        &compiler.store,
        prepared_roots,
        admissions,
    ))
}

fn arithmetic32_control(
    op: u32,
    input_flag: u32,
) -> Option<(u8, u8, u8)> {
    if input_flag > 1 {
        return None;
    }
    match op {
        6 => Some((0, 0, 1)),
        7 => Some((input_flag as u8, 0, 1)),
        8 => Some((0, 1, 1)),
        9 => Some((input_flag as u8, 1, 1)),
        10 => Some((0, 1, 0)),
        _ => None,
    }
}

pub(crate) fn configure_arithmetic32_session(
    session: &mut ProofRuntimeSession,
    load: &WebProofLoadStage,
    op: u32,
    a: u32,
    b: u32,
    input_flag: u32,
) -> Option<(Handle, usize, usize)> {
    let (x, mode, writeback) =
        arithmetic32_control(op, input_flag)?;
    let effect =
        loaded_handle(load, "function.effect.arithmetic")?;
    let apply = loaded_handle(load, "execution.apply")?;
    let caller = loaded_handle(load, "context.caller")?;
    let zero = loaded_handle(load, "data.bit.zero")?;
    let one = loaded_handle(load, "data.bit.one")?;

    let word = |store: &mut OptimizedLinkStore, value: u32| {
        let bits = (0..32)
            .map(|bit| {
                if (value >> bit) & 1 == 1 { one } else { zero }
            })
            .collect::<Vec<_>>();
        materialize_exact_sequence(store, &bits).ok()
    };
    let bit = |value: u8| if value == 0 { zero } else { one };

    let before = session.memory.store.link_count();
    let aword = word(&mut session.memory.store, a)?;
    let bword = word(&mut session.memory.store, b)?;
    let args = materialize_exact_sequence(
        &mut session.memory.store,
        &[aword, bword, bit(x), bit(mode), bit(writeback)],
    )
    .ok()?;
    let invocation =
        call(&mut session.memory.store, apply, effect, args);
    let initial = session
        .memory
        .store
        .ensure_pair(caller, invocation)
        .ok()?;
    let after = session.memory.store.link_count();

    Some((initial, before, after))
}

pub(crate) fn project_arithmetic32_session_result(
    session: &ProofRuntimeSession,
    load: &WebProofLoadStage,
) -> Option<Arithmetic32SessionProjection> {
    if session.engine.current().len() != 1 {
        return None;
    }

    let caller = loaded_handle(load, "context.caller")?;
    let result_tag = loaded_handle(load, "result.tag")?;
    let zero = loaded_handle(load, "data.bit.zero")?;
    let one = loaded_handle(load, "data.bit.one")?;
    let set_tag = loaded_handle(load, "result.flag.set_tag")?;
    let cf_flag = loaded_handle(load, "result.flag.cf")?;
    let pf_flag = loaded_handle(load, "result.flag.pf")?;
    let af_flag = loaded_handle(load, "result.flag.af")?;
    let zf_flag = loaded_handle(load, "result.flag.zf")?;
    let sf_flag = loaded_handle(load, "result.flag.sf")?;
    let of_flag = loaded_handle(load, "result.flag.of")?;
    let final_link = session.engine.current()[0];

    let (outcome, _) = decode_runtime_arithmetic_effect(
        &session.memory,
        final_link,
        caller,
        result_tag,
        zero,
        one,
        set_tag,
        cf_flag,
        pf_flag,
        af_flag,
        zf_flag,
        sf_flag,
        of_flag,
        32,
    )?;
    let (defined_mask, value_mask) =
        web_arithmetic_masks(outcome);

    Some(Arithmetic32SessionProjection {
        value: outcome.value,
        writeback: outcome.writeback,
        defined_mask,
        value_mask,
        undefined_mask: 0,
        preserve_mask: 0,
        result_recursive_wire: session
            .memory
            .store
            .export_anum(final_link)
            .ok()?,
    })
}

// Compatibility facade: ADD32 owns no separate persistent executor.
pub(crate) fn prepare_add32_session_program(
) -> Option<WebProofPrepareStage> {
    prepare_arithmetic32_session_program()
}

pub(crate) fn configure_add32_session(
    session: &mut ProofRuntimeSession,
    load: &WebProofLoadStage,
    a: u32,
    b: u32,
) -> Option<(Handle, usize, usize)> {
    configure_arithmetic32_session(session, load, 6, a, b, 0)
}

pub(crate) fn project_add32_session_result(
    session: &ProofRuntimeSession,
    load: &WebProofLoadStage,
) -> Option<Add32SessionProjection> {
    project_arithmetic32_session_result(session, load)
}

pub(crate) fn web_prove_arithmetic(
    op: u32,
    a: u32,
    b: u32,
    input_flag: u32,
) -> Option<WebArithmeticProofExecution> {
    if input_flag > 1 {
        return None;
    }

    let (block, x, mode, writeback) = match op {
        6 => ("ADD32", 0u8, 0u8, 1u8),
        7 => ("ADC32", input_flag as u8, 0u8, 1u8),
        8 => ("SUB32", 0u8, 1u8, 1u8),
        9 => ("SBB32", input_flag as u8, 1u8, 1u8),
        10 => ("CMP32", 0u8, 1u8, 0u8),
        _ => return None,
    };

    let mut compiler = FullFixture::new();
    let program = ArithmeticEffectProgram::install(&mut compiler, 32);
    let links_after_build = program.links_after_build as u32;

    let a_bits = bit_handles(&compiler, 32, a);
    let b_bits = bit_handles(&compiler, 32, b);
    let aword =
        materialize_exact_sequence(&mut compiler.store, &a_bits).ok()?;
    let bword =
        materialize_exact_sequence(&mut compiler.store, &b_bits).ok()?;
    let xh = if x == 0 { compiler.zero } else { compiler.one };
    let mh = if mode == 0 { compiler.zero } else { compiler.one };
    let wbh = if writeback == 0 { compiler.zero } else { compiler.one };

    let args = materialize_exact_sequence(
        &mut compiler.store,
        &[aword, bword, xh, mh, wbh],
    )
    .ok()?;
    let invocation =
        call(&mut compiler.store, compiler.apply, program.effect, args);
    let initial = compiler
        .store
        .ensure_pair(compiler.k, invocation)
        .ok()?;

    let prepared_roots = vec![
        semantic_source(
            &compiler.store,
            "function.effect.arithmetic",
            program.effect,
        ),
        semantic_source(
            &compiler.store,
            "function.flagged_arithmetic",
            program.flagged.flagged,
        ),
        semantic_source(
            &compiler.store,
            "result.flagged_tag",
            program.flagged.result_tag,
        ),
        semantic_source(&compiler.store, "data.a.word", aword),
        semantic_source(&compiler.store, "data.b.word", bword),
        semantic_source(&compiler.store, "data.x", xh),
        semantic_source(&compiler.store, "data.mode", mh),
        semantic_source(&compiler.store, "data.writeback", wbh),
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
        semantic_source(&compiler.store, "result.tag", program.result_tag),
        semantic_source(
            &compiler.store,
            "result.flag.set_tag",
            program.schema.set_tag,
        ),
        semantic_source(&compiler.store, "result.flag.cf", program.schema.cf),
        semantic_source(&compiler.store, "result.flag.pf", program.schema.pf),
        semantic_source(&compiler.store, "result.flag.af", program.schema.af),
        semantic_source(&compiler.store, "result.flag.zf", program.schema.zf),
        semantic_source(&compiler.store, "result.flag.sf", program.schema.sf),
        semantic_source(&compiler.store, "result.flag.of", program.schema.of),
    ];

    let admissions =
        theory_admissions(&compiler.store, compiler.theory)?;
    let prepare =
        prepare_stage(&compiler.store, prepared_roots, admissions);

    let (mut memory, load) = load_runtime(&prepare)?;
    let interpreter = loaded_handle(&load, "execution.interpreter")?;
    let initial = loaded_handle(&load, "scope.initial")?;
    let caller = loaded_handle(&load, "context.caller")?;
    let result_tag = loaded_handle(&load, "result.tag")?;
    let zero = loaded_handle(&load, "data.bit.zero")?;
    let one = loaded_handle(&load, "data.bit.one")?;
    let set_tag = loaded_handle(&load, "result.flag.set_tag")?;
    let cf_flag = loaded_handle(&load, "result.flag.cf")?;
    let pf_flag = loaded_handle(&load, "result.flag.pf")?;
    let af_flag = loaded_handle(&load, "result.flag.af")?;
    let zf_flag = loaded_handle(&load, "result.flag.zf")?;
    let sf_flag = loaded_handle(&load, "result.flag.sf")?;
    let of_flag = loaded_handle(&load, "result.flag.of")?;

    let (mut engine, execute) = execute_to_quiescence(
        &mut memory,
        interpreter,
        initial,
        32,
        program.active_steps as u32 + 2,
    )?;
    if execute.active_reaction_count != program.active_steps as u32
        || engine.current().len() != 1
    {
        return None;
    }

    let final_link = engine.current()[0];
    let (actual, payload) = decode_runtime_arithmetic_effect(
        &memory,
        final_link,
        caller,
        result_tag,
        zero,
        one,
        set_tag,
        cf_flag,
        pf_flag,
        af_flag,
        zf_flag,
        sf_flag,
        of_flag,
        32,
    )?;
    let oracle = expected(32, a, b, x, mode, writeback);

    let result_anum = memory.store.export_anum(final_link).ok()?;
    let result_sequence_anum = memory.store.export_anum(payload).ok()?;
    let links_after_first = memory.store.link_count() as u32;

    let identical_rerun_link_delta = identical_rerun(
        &mut memory,
        &mut engine,
        initial,
        &result_anum,
        program.active_steps as u32 + 2,
    )?;
    let visual_links =
        visual_snapshot(&memory, &load.semantic_roots);

    let result = WebProofResultStage {
        memory_instance_id: memory.id.clone(),
        result_anum,
        result_sequence_anum,
        decoded_value: actual.value,
        decoded_value_hi: None,
        oracle_value: oracle.value,
        oracle_value_hi: None,
        oracle_matches: actual == oracle,
        links_final: memory.store.link_count() as u32,
        identical_rerun_link_delta,
        visual_links,
    };

    let proof = WebStructuralProof {
        schema_version: 4,
        block: block.to_owned(),
        prepare,
        load,
        execute,
        result,
    };

    let (defined_mask, value_mask) = web_arithmetic_masks(actual);
    let outcome = WebArithmeticOutcome {
        value: actual.value,
        writeback: actual.writeback,
        defined_mask,
        value_mask,
        undefined_mask: 0,
        preserve_mask: 0,
        reactions: proof.execute.active_reaction_count,
        links_after_build,
        links_after_first,
        steady_link_delta: proof.result.identical_rerun_link_delta,
        quiescent: u8::from(proof.execute.final_quiescent),
    };

    Some(WebArithmeticProofExecution { outcome, proof })
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
fn persistent_arithmetic32_session_four_run_lifecycle_all_ops() {
    let run_vectors = [
        (0xffff_ffffu32, 0x0000_0000u32, 1u32),
        (0x7fff_ffffu32, 0x0000_0001u32, 0u32),
        (0x8000_0000u32, 0x0000_0001u32, 1u32),
        (0xffff_ffffu32, 0x0000_0000u32, 1u32),
    ];

    for op in 6u32..=10 {
        let prepare = prepare_arithmetic32_session_program()
            .expect("prepare Arithmetic32");
        let prepared_roles = prepare
            .semantic_roots
            .iter()
            .map(|root| root.role.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        for runtime_role in [
            "data.a.word",
            "data.b.word",
            "data.x",
            "data.mode",
            "data.writeback",
            "invocation.args",
            "invocation.call",
            "scope.initial",
        ] {
            assert!(
                !prepared_roles.contains(runtime_role),
                "op {op}: PREPARE leaked runtime role {runtime_role}",
            );
        }

        let (mut session, load) =
            load_runtime_session(&prepare, 128).expect("load Arithmetic32");
        let memory_id = session.memory.id.clone();
        let base_link_count = session.base_link_count;
        let base_carrier =
            session.memory.store.export_packed_duplets();

        let mut first_projection = None;
        for (index, (a, b, input_flag)) in
            run_vectors.into_iter().enumerate()
        {
            let (initial, before, after) =
                configure_arithmetic32_session(
                    &mut session,
                    &load,
                    op,
                    a,
                    b,
                    input_flag,
                )
                .expect("configure Arithmetic32");
            assert_eq!(session.memory.id, memory_id);
            assert!(before >= base_link_count);
            assert!(after >= before);

            if index == 3 {
                assert_eq!(
                    after, before,
                    "op {op}: return-to-first input must reuse canonical configuration Links",
                );
            }

            let execute = execute_session_to_quiescence(
                &mut session,
                initial,
                1024,
            )
            .expect("execute Arithmetic32");
            assert!(execute.final_quiescent);
            assert_eq!(execute.active_reaction_count, 609);
            assert_eq!(session.memory.id, memory_id);
            assert_eq!(session.engine.current().len(), 1);

            let projected =
                project_arithmetic32_session_result(&session, &load)
                    .expect("project Arithmetic32");
            let (x, mode, writeback) =
                arithmetic32_control(op, input_flag).unwrap();
            let expected =
                expected(32, a, b, x, mode, writeback);
            let (expected_defined, expected_values) =
                web_arithmetic_masks(expected);

            assert_eq!(projected.value, expected.value, "op {op} run {index}");
            assert_eq!(
                projected.writeback,
                expected.writeback,
                "op {op} run {index}",
            );
            assert_eq!(projected.defined_mask, expected_defined);
            assert_eq!(projected.value_mask, expected_values);
            assert_eq!(projected.undefined_mask, 0);
            assert_eq!(projected.preserve_mask, 0);

            let carrier = session.memory.store.export_packed_duplets();
            assert_eq!(
                &carrier[..base_link_count],
                base_carrier.as_slice(),
                "op {op}: loaded base prefix changed",
            );

            if index == 0 {
                first_projection = Some(projected.clone());
            } else if index == 3 {
                assert_eq!(
                    Some(projected),
                    first_projection,
                    "op {op}: return-to-first input changed semantic Result",
                );
            }
        }
    }
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
