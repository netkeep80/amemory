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
    proof_n::{
        execute_to_quiescence, identical_rerun, load_runtime, loaded_handle,
        prepare_stage, semantic_source, theory_admissions, visual_snapshot,
        ProofRuntimeMemory, WebProofResultStage, WebStructuralProof,
    },
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


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WebMulEffectOutcome {
    pub(crate) lo: u32,
    pub(crate) hi: u32,
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

fn web_mul_effect_flag(
    mask: u32,
    state: FlagState,
    defined: &mut u32,
    values: &mut u32,
    undefined: &mut u32,
) {
    match state {
        FlagState::Set(bit) => {
            *defined |= mask;
            if bit != 0 {
                *values |= mask;
            }
        }
        FlagState::Undefined => *undefined |= mask,
    }
}


#[derive(Clone, Debug)]
pub(crate) struct WebMulEffectProofExecution {
    pub(crate) outcome: WebMulEffectOutcome,
    pub(crate) proof: WebStructuralProof,
}

fn runtime_mul_effect_bit(
    value: Handle,
    zero: Handle,
    one: Handle,
) -> Option<u8> {
    if value == one {
        Some(1)
    } else if value == zero {
        Some(0)
    } else {
        None
    }
}

fn runtime_mul_effect_word(
    memory: &ProofRuntimeMemory,
    word: Handle,
    zero: Handle,
    one: Handle,
) -> Option<u32> {
    let bits = read_exact_sequence(&memory.store, word).ok()?;
    if bits.len() != WIDTH {
        return None;
    }
    let mut value = 0u32;
    for (index, bit) in bits.into_iter().enumerate() {
        value |=
            u32::from(runtime_mul_effect_bit(bit, zero, one)?) << index;
    }
    Some(value)
}

fn runtime_mul_effect_wide(
    memory: &ProofRuntimeMemory,
    wide: Handle,
    zero: Handle,
    one: Handle,
) -> Option<u64> {
    let halves = read_exact_sequence(&memory.store, wide).ok()?;
    if halves.len() != 2 {
        return None;
    }
    let lo = runtime_mul_effect_word(memory, halves[0], zero, one)?;
    let hi = runtime_mul_effect_word(memory, halves[1], zero, one)?;
    Some(u64::from(lo) | (u64::from(hi) << 32))
}

fn runtime_mul_effect_action(
    memory: &ProofRuntimeMemory,
    action: Handle,
    set_tag: Handle,
    undefined_tag: Handle,
    expected_flag: Handle,
    zero: Handle,
    one: Handle,
) -> Option<FlagState> {
    let (tag, payload) = memory.store.poles(action).ok()?;
    let values = read_exact_sequence(&memory.store, payload).ok()?;

    if tag == set_tag {
        if values.len() != 2 || values[0] != expected_flag {
            return None;
        }
        Some(FlagState::Set(runtime_mul_effect_bit(
            values[1], zero, one,
        )?))
    } else if tag == undefined_tag {
        if values != vec![expected_flag] {
            return None;
        }
        Some(FlagState::Undefined)
    } else {
        None
    }
}

#[allow(clippy::too_many_arguments)]
fn decode_runtime_mul_effect(
    memory: &ProofRuntimeMemory,
    final_link: Handle,
    caller: Handle,
    result_tag: Handle,
    zero: Handle,
    one: Handle,
    set_tag: Handle,
    undefined_tag: Handle,
    cf_flag: Handle,
    pf_flag: Handle,
    af_flag: Handle,
    zf_flag: Handle,
    sf_flag: Handle,
    of_flag: Handle,
    reactions: usize,
) -> Option<(Outcome, Handle)> {
    let (final_caller, endpoint) = memory.store.poles(final_link).ok()?;
    if final_caller != caller {
        return None;
    }
    let (tag, payload) = memory.store.poles(endpoint).ok()?;
    if tag != result_tag {
        return None;
    }

    let values = read_exact_sequence(&memory.store, payload).ok()?;
    if values.len() != 2 {
        return None;
    }
    let product =
        runtime_mul_effect_wide(memory, values[0], zero, one)?;
    let patch = read_exact_sequence(&memory.store, values[1]).ok()?;
    if patch.len() != 6 {
        return None;
    }

    Some((
        Outcome {
            product,
            cf: runtime_mul_effect_action(
                memory, patch[0], set_tag, undefined_tag, cf_flag, zero, one,
            )?,
            pf: runtime_mul_effect_action(
                memory, patch[1], set_tag, undefined_tag, pf_flag, zero, one,
            )?,
            af: runtime_mul_effect_action(
                memory, patch[2], set_tag, undefined_tag, af_flag, zero, one,
            )?,
            zf: runtime_mul_effect_action(
                memory, patch[3], set_tag, undefined_tag, zf_flag, zero, one,
            )?,
            sf: runtime_mul_effect_action(
                memory, patch[4], set_tag, undefined_tag, sf_flag, zero, one,
            )?,
            of: runtime_mul_effect_action(
                memory, patch[5], set_tag, undefined_tag, of_flag, zero, one,
            )?,
            reactions,
        },
        payload,
    ))
}

pub(crate) fn web_prove_mul_effect(
    a: u32,
    b: u32,
) -> Option<WebMulEffectProofExecution> {
    let mut compiler = FullFixture::new();
    let program = MulEffectProgram::install(&mut compiler);
    let links_after_build = program.links_after_build as u32;

    let aword = word_handle(&mut compiler, a);
    let bword = word_handle(&mut compiler, b);
    let args =
        materialize_exact_sequence(&mut compiler.store, &[aword, bword]).ok()?;
    let invocation =
        call(&mut compiler.store, compiler.apply, program.effect, args);
    let initial =
        compiler.store.ensure_pair(compiler.k, invocation).ok()?;

    let prepared_roots = vec![
        semantic_source(
            &compiler.store,
            "function.effect.mul",
            program.effect,
        ),
        semantic_source(
            &compiler.store,
            "function.dependency.mul32",
            program.mul.mul32,
        ),
        semantic_source(
            &compiler.store,
            "function.dependency.or2",
            program.gates.or2,
        ),
        semantic_source(
            &compiler.store,
            "result.dependency.mul32_tag",
            program.mul.result_tag,
        ),
        semantic_source(&compiler.store, "data.a.word", aword),
        semantic_source(&compiler.store, "data.b.word", bword),
        semantic_source(&compiler.store, "data.bit.zero", compiler.zero),
        semantic_source(&compiler.store, "data.bit.one", compiler.one),
        semantic_source(
            &compiler.store,
            "execution.interpreter",
            compiler.interpreter,
        ),
        semantic_source(&compiler.store, "execution.theory", compiler.theory),
        semantic_source(&compiler.store, "execution.apply", compiler.apply),
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
        semantic_source(
            &compiler.store,
            "result.flag.undefined_tag",
            program.schema.undefined_tag,
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

    let admissions = theory_admissions(&compiler.store, compiler.theory)?;
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
    let undefined_tag =
        loaded_handle(&load, "result.flag.undefined_tag")?;
    let cf_flag = loaded_handle(&load, "result.flag.cf")?;
    let pf_flag = loaded_handle(&load, "result.flag.pf")?;
    let af_flag = loaded_handle(&load, "result.flag.af")?;
    let zf_flag = loaded_handle(&load, "result.flag.zf")?;
    let sf_flag = loaded_handle(&load, "result.flag.sf")?;
    let of_flag = loaded_handle(&load, "result.flag.of")?;

    let expected_reactions = expected_steps(b) as u32;
    let max_steps = expected_reactions + 2;
    let (mut engine, execute) = execute_to_quiescence(
        &mut memory,
        interpreter,
        initial,
        32,
        max_steps,
    )?;
    if execute.active_reaction_count != expected_reactions
        || engine.current().len() != 1
    {
        return None;
    }

    let final_link = engine.current()[0];
    let (actual, payload) = decode_runtime_mul_effect(
        &memory,
        final_link,
        caller,
        result_tag,
        zero,
        one,
        set_tag,
        undefined_tag,
        cf_flag,
        pf_flag,
        af_flag,
        zf_flag,
        sf_flag,
        of_flag,
        execute.active_reaction_count as usize,
    )?;
    let oracle = expected(a, b);

    let result_anum = memory.store.export_anum(final_link).ok()?;
    let result_sequence_anum = memory.store.export_anum(payload).ok()?;
    let links_after_first = memory.store.link_count() as u32;
    let identical_rerun_link_delta = identical_rerun(
        &mut memory,
        &mut engine,
        initial,
        &result_anum,
        max_steps,
    )?;
    let visual_links = visual_snapshot(&memory, &load.semantic_roots);

    let result = WebProofResultStage {
        memory_instance_id: memory.id.clone(),
        result_anum,
        result_sequence_anum,
        decoded_value: actual.product as u32,
        decoded_value_hi: Some((actual.product >> 32) as u32),
        oracle_value: oracle.product as u32,
        oracle_value_hi: Some((oracle.product >> 32) as u32),
        oracle_matches: actual == oracle,
        links_final: memory.store.link_count() as u32,
        identical_rerun_link_delta,
        visual_links,
    };
    let proof = WebStructuralProof {
        schema_version: 3,
        block: "x86 MUL32 effect".to_owned(),
        prepare,
        load,
        execute,
        result,
    };

    let mut defined_mask = 0u32;
    let mut value_mask = 0u32;
    let mut undefined_mask = 0u32;
    web_mul_effect_flag(
        WEB_CF, actual.cf, &mut defined_mask, &mut value_mask,
        &mut undefined_mask,
    );
    web_mul_effect_flag(
        WEB_PF, actual.pf, &mut defined_mask, &mut value_mask,
        &mut undefined_mask,
    );
    web_mul_effect_flag(
        WEB_AF, actual.af, &mut defined_mask, &mut value_mask,
        &mut undefined_mask,
    );
    web_mul_effect_flag(
        WEB_ZF, actual.zf, &mut defined_mask, &mut value_mask,
        &mut undefined_mask,
    );
    web_mul_effect_flag(
        WEB_SF, actual.sf, &mut defined_mask, &mut value_mask,
        &mut undefined_mask,
    );
    web_mul_effect_flag(
        WEB_OF, actual.of, &mut defined_mask, &mut value_mask,
        &mut undefined_mask,
    );

    let outcome = WebMulEffectOutcome {
        lo: actual.product as u32,
        hi: (actual.product >> 32) as u32,
        defined_mask,
        value_mask,
        undefined_mask,
        preserve_mask: 0,
        reactions: expected_reactions,
        links_after_build,
        links_after_first,
        steady_link_delta: proof.result.identical_rerun_link_delta,
        quiescent: u8::from(proof.execute.final_quiescent),
    };

    Some(WebMulEffectProofExecution { outcome, proof })
}

pub(crate) fn web_run_mul_effect(
    a: u32,
    b: u32,
) -> WebMulEffectOutcome {
    let mut f = FullFixture::new();
    let p = MulEffectProgram::install(&mut f);
    let links_after_build = f.store.link_count() as u32;

    let first = run(&mut f, &p, a, b);
    let links_after_first = f.store.link_count() as u32;
    let second = run(&mut f, &p, a, b);
    assert_eq!(second, first, "web MUL effect repeat changed result");
    let links_after_second = f.store.link_count() as u32;

    let mut defined_mask = 0u32;
    let mut value_mask = 0u32;
    let mut undefined_mask = 0u32;
    web_mul_effect_flag(WEB_CF, first.cf, &mut defined_mask, &mut value_mask, &mut undefined_mask);
    web_mul_effect_flag(WEB_PF, first.pf, &mut defined_mask, &mut value_mask, &mut undefined_mask);
    web_mul_effect_flag(WEB_AF, first.af, &mut defined_mask, &mut value_mask, &mut undefined_mask);
    web_mul_effect_flag(WEB_ZF, first.zf, &mut defined_mask, &mut value_mask, &mut undefined_mask);
    web_mul_effect_flag(WEB_SF, first.sf, &mut defined_mask, &mut value_mask, &mut undefined_mask);
    web_mul_effect_flag(WEB_OF, first.of, &mut defined_mask, &mut value_mask, &mut undefined_mask);

    WebMulEffectOutcome {
        lo: first.product as u32,
        hi: (first.product >> 32) as u32,
        defined_mask,
        value_mask,
        undefined_mask,
        preserve_mask: 0,
        reactions: first.reactions as u32,
        links_after_build,
        links_after_first,
        steady_link_delta: links_after_second - links_after_first,
        quiescent: 1,
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
