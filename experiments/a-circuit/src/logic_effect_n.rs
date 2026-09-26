use super::{
    full_adder::{
        call, define_bundle_rule, index_rule_for, Fixture as FullFixture,
    },
    flag_patch::{
        install_alu_effect_result_tag, set_flag_action,
        undefined_flag_action, FlagPatchSchema,
    },
    logic_n::LogicProgram,
};
use amemory_optimized_cpu_probe::{
    structural::{materialize_exact_sequence, read_exact_sequence},
    Handle, OptimizedLinkStore, ROOT_HANDLE,
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
        // Separate namespace from WORD logic and M3 arithmetic/flags.
        seed = store.ensure_pair(seed, c).unwrap();
        seed = store.ensure_pair(seed, o).unwrap();
        seed = store.ensure_pair(seed, c).unwrap();
        seed = store.ensure_pair(seed, c).unwrap();
        for _ in 0..width {
            seed = store.ensure_pair(seed, o).unwrap();
            seed = store.ensure_pair(seed, c).unwrap();
            seed = store.ensure_pair(seed, o).unwrap();
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
    let args =
        materialize_exact_sequence(&mut f.store, &[value]).unwrap();
    call(&mut f.store, f.apply, function, args)
}

#[derive(Clone, Debug)]
struct LogicEffectProgram {
    width: usize,
    logic: LogicProgram,
    binary_effect: Handle,
    not_effect: Handle,
    result_tag: Handle,
    schema: FlagPatchSchema,
    binary_steps: usize,
    not_steps: usize,
    links_after_build: usize,
}

impl LogicEffectProgram {
    fn install(f: &mut FullFixture, width: usize) -> Self {
        assert!((8..=32).contains(&width));

        let logic = LogicProgram::install(f, width);
        let seed0 = f
            .store
            .ensure_pair(logic.word_binary, logic.result_tag)
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
        let binary_effect =
            f.store.ensure_pair(effect_left, effect_right).unwrap();

        let not_left = anchors.next(&mut f.store);
        let not_right = anchors.next(&mut f.store);
        let not_effect =
            f.store.ensure_pair(not_left, not_right).unwrap();

        let effect_seed = f.store.ensure_pair(f.k, f.full).unwrap();
        let result_tag = install_alu_effect_result_tag(
            &mut f.store,
            effect_seed,
            f.o,
            f.c,
        );

        // Shared component ABI. Calling this with the same FullFixture seed
        // yields the same structural FlagId / SET / UNDEFINED Links for
        // arithmetic and logical effect producers in one A-memory.
        let flag_seed = f.store.ensure_pair(f.k, f.full).unwrap();
        let schema = FlagPatchSchema::install(
            &mut f.store,
            flag_seed,
            f.o,
            f.c,
        );

        let binary_result_tag = anchors.next(&mut f.store);
        let not_result_tag = anchors.next(&mut f.store);
        let zf_not_tag = anchors.next(&mut f.store);
        let pf_not_tag = anchors.next(&mut f.store);

        let mut zf_tags = Vec::with_capacity(width - 1);
        for _ in 0..(width - 1) {
            let left = anchors.next(&mut f.store);
            let right = anchors.next(&mut f.store);
            zf_tags.push(f.store.ensure_pair(left, right).unwrap());
        }

        let mut pf_tags = Vec::with_capacity(7);
        for _ in 0..7 {
            let left = anchors.next(&mut f.store);
            let right = anchors.next(&mut f.store);
            pf_tags.push(f.store.ensure_pair(left, right).unwrap());
        }

        // -------------------------------------------------------------
        // Binary logical effect OPEN.
        //
        // K -> Call(LOGIC_EFFECT,[Gate,Aword,Bword,WriteBack])
        // =>
        // ResultFrame([K,WriteBack])
        //   -> Call(WORD_BIN,[Gate,Aword,Bword])
        //
        // TEST therefore uses the exact AND path with WriteBack=0.
        // -------------------------------------------------------------
        {
            let k = anchors.next(&mut f.store);
            let gate = anchors.next(&mut f.store);
            let aword = anchors.next(&mut f.store);
            let bword = anchors.next(&mut f.store);
            let writeback = anchors.next(&mut f.store);

            let args = materialize_exact_sequence(
                &mut f.store,
                &[gate, aword, bword, writeback],
            )
            .unwrap();
            let before_call =
                call(&mut f.store, f.apply, binary_effect, args);
            let before = f.store.ensure_pair(k, before_call).unwrap();

            let caller = stage_frame(
                &mut f.store,
                binary_result_tag,
                &[k, writeback],
            );
            let logic_args = materialize_exact_sequence(
                &mut f.store,
                &[gate, aword, bword],
            )
            .unwrap();
            let logic_call = call(
                &mut f.store,
                f.apply,
                logic.word_binary,
                logic_args,
            );
            let after = f.store.ensure_pair(caller, logic_call).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, gate, aword, bword, writeback],
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        // WORD_LOGIC_RESULT -> begin ZF OR reduction.
        {
            let k = anchors.next(&mut f.store);
            let writeback = anchors.next(&mut f.store);
            let bits = anchors.roles(&mut f.store, width);

            let word =
                materialize_exact_sequence(&mut f.store, &bits).unwrap();
            let logic_payload =
                materialize_exact_sequence(&mut f.store, &[word]).unwrap();
            let logic_envelope = f
                .store
                .ensure_pair(logic.result_tag, logic_payload)
                .unwrap();

            let caller = stage_frame(
                &mut f.store,
                binary_result_tag,
                &[k, writeback],
            );
            let before =
                f.store.ensure_pair(caller, logic_envelope).unwrap();

            let mut state = Vec::with_capacity(width + 2);
            state.push(k);
            state.push(writeback);
            state.extend_from_slice(&bits);

            let next_caller =
                stage_frame(&mut f.store, zf_tags[0], &state);
            let or_call = binary_call(
                f,
                logic.gates.or2,
                bits[0],
                bits[1],
            );
            let after = f.store.ensure_pair(next_caller, or_call).unwrap();

            let mut roles = Vec::with_capacity(width + 2);
            roles.push(k);
            roles.push(writeback);
            roles.extend_from_slice(&bits);

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &[logic.result_tag],
                admission,
            );
        }

        // OR-reduce all result bits. The first call combined bits 0 and 1.
        for i in 0..(width - 1) {
            let k = anchors.next(&mut f.store);
            let writeback = anchors.next(&mut f.store);
            let bits = anchors.roles(&mut f.store, width);
            let acc = anchors.next(&mut f.store);

            let mut state = Vec::with_capacity(width + 2);
            state.push(k);
            state.push(writeback);
            state.extend_from_slice(&bits);

            let caller =
                stage_frame(&mut f.store, zf_tags[i], &state);
            let acc_result =
                materialize_exact_sequence(&mut f.store, &[acc]).unwrap();
            let before =
                f.store.ensure_pair(caller, acc_result).unwrap();

            let after = if i + 1 < width - 1 {
                let next_caller =
                    stage_frame(&mut f.store, zf_tags[i + 1], &state);
                let next_call = binary_call(
                    f,
                    logic.gates.or2,
                    acc,
                    bits[i + 2],
                );
                f.store.ensure_pair(next_caller, next_call).unwrap()
            } else {
                let next_caller =
                    stage_frame(&mut f.store, zf_not_tag, &state);
                let next_call =
                    unary_call(f, logic.gates.not1, acc);
                f.store.ensure_pair(next_caller, next_call).unwrap()
            };

            let mut roles = Vec::with_capacity(width + 3);
            roles.push(k);
            roles.push(writeback);
            roles.extend_from_slice(&bits);
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
                &logic.gates.bit_outputs,
                admission,
            );
        }

        // ZF result -> begin low-byte parity XOR reduction.
        {
            let k = anchors.next(&mut f.store);
            let writeback = anchors.next(&mut f.store);
            let bits = anchors.roles(&mut f.store, width);
            let zf = anchors.next(&mut f.store);

            let mut state = Vec::with_capacity(width + 2);
            state.push(k);
            state.push(writeback);
            state.extend_from_slice(&bits);

            let caller =
                stage_frame(&mut f.store, zf_not_tag, &state);
            let zf_result =
                materialize_exact_sequence(&mut f.store, &[zf]).unwrap();
            let before =
                f.store.ensure_pair(caller, zf_result).unwrap();

            let mut parity_state = Vec::with_capacity(width + 3);
            parity_state.push(k);
            parity_state.push(writeback);
            parity_state.extend_from_slice(&bits);
            parity_state.push(zf);

            let next_caller =
                stage_frame(&mut f.store, pf_tags[0], &parity_state);
            let xor_call = binary_call(
                f,
                logic.gates.xor2,
                bits[0],
                bits[1],
            );
            let after = f.store.ensure_pair(next_caller, xor_call).unwrap();

            let mut roles = Vec::with_capacity(width + 3);
            roles.push(k);
            roles.push(writeback);
            roles.extend_from_slice(&bits);
            roles.push(zf);

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &logic.gates.bit_outputs,
                admission,
            );
        }

        // XOR-reduce exactly low byte bits 0..7.
        for i in 0..7 {
            let k = anchors.next(&mut f.store);
            let writeback = anchors.next(&mut f.store);
            let bits = anchors.roles(&mut f.store, width);
            let zf = anchors.next(&mut f.store);
            let acc = anchors.next(&mut f.store);

            let mut state = Vec::with_capacity(width + 3);
            state.push(k);
            state.push(writeback);
            state.extend_from_slice(&bits);
            state.push(zf);

            let caller =
                stage_frame(&mut f.store, pf_tags[i], &state);
            let acc_result =
                materialize_exact_sequence(&mut f.store, &[acc]).unwrap();
            let before =
                f.store.ensure_pair(caller, acc_result).unwrap();

            let after = if i + 1 < 7 {
                let next_caller =
                    stage_frame(&mut f.store, pf_tags[i + 1], &state);
                let next_call = binary_call(
                    f,
                    logic.gates.xor2,
                    acc,
                    bits[i + 2],
                );
                f.store.ensure_pair(next_caller, next_call).unwrap()
            } else {
                let next_caller =
                    stage_frame(&mut f.store, pf_not_tag, &state);
                let next_call =
                    unary_call(f, logic.gates.not1, acc);
                f.store.ensure_pair(next_caller, next_call).unwrap()
            };

            let mut roles = Vec::with_capacity(width + 4);
            roles.push(k);
            roles.push(writeback);
            roles.extend_from_slice(&bits);
            roles.push(zf);
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
                &logic.gates.bit_outputs,
                admission,
            );
        }

        // Even-parity result -> structural FlagPatch + effect result.
        {
            let k = anchors.next(&mut f.store);
            let writeback = anchors.next(&mut f.store);
            let bits = anchors.roles(&mut f.store, width);
            let zf = anchors.next(&mut f.store);
            let pf = anchors.next(&mut f.store);

            let mut state = Vec::with_capacity(width + 3);
            state.push(k);
            state.push(writeback);
            state.extend_from_slice(&bits);
            state.push(zf);

            let caller =
                stage_frame(&mut f.store, pf_not_tag, &state);
            let pf_result =
                materialize_exact_sequence(&mut f.store, &[pf]).unwrap();
            let before =
                f.store.ensure_pair(caller, pf_result).unwrap();

            let word =
                materialize_exact_sequence(&mut f.store, &bits).unwrap();

            let set_cf = set_flag_action(
                &mut f.store,
                schema,
                schema.cf,
                f.zero,
            );
            let set_pf = set_flag_action(
                &mut f.store,
                schema,
                schema.pf,
                pf,
            );
            let undef_af =
                undefined_flag_action(&mut f.store, schema, schema.af);
            let set_zf = set_flag_action(
                &mut f.store,
                schema,
                schema.zf,
                zf,
            );
            let set_sf = set_flag_action(
                &mut f.store,
                schema,
                schema.sf,
                bits[width - 1],
            );
            let set_of = set_flag_action(
                &mut f.store,
                schema,
                schema.of,
                f.zero,
            );

            let patch = materialize_exact_sequence(
                &mut f.store,
                &[set_cf, set_pf, undef_af, set_zf, set_sf, set_of],
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

            let mut roles = Vec::with_capacity(width + 4);
            roles.push(k);
            roles.push(writeback);
            roles.extend_from_slice(&bits);
            roles.push(zf);
            roles.push(pf);

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &logic.gates.bit_outputs,
                admission,
            );
        }

        // -------------------------------------------------------------
        // NOT effect: reuse WORD_NOT and emit an empty FlagPatch.
        // -------------------------------------------------------------
        {
            let k = anchors.next(&mut f.store);
            let aword = anchors.next(&mut f.store);

            let args =
                materialize_exact_sequence(&mut f.store, &[aword]).unwrap();
            let before_call =
                call(&mut f.store, f.apply, not_effect, args);
            let before = f.store.ensure_pair(k, before_call).unwrap();

            let caller =
                stage_frame(&mut f.store, not_result_tag, &[k]);
            let logic_args =
                materialize_exact_sequence(&mut f.store, &[aword]).unwrap();
            let logic_call =
                call(&mut f.store, f.apply, logic.word_not, logic_args);
            let after = f.store.ensure_pair(caller, logic_call).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, aword],
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        {
            let k = anchors.next(&mut f.store);
            let bits = anchors.roles(&mut f.store, width);

            let word =
                materialize_exact_sequence(&mut f.store, &bits).unwrap();
            let logic_payload =
                materialize_exact_sequence(&mut f.store, &[word]).unwrap();
            let logic_envelope = f
                .store
                .ensure_pair(logic.result_tag, logic_payload)
                .unwrap();

            let caller =
                stage_frame(&mut f.store, not_result_tag, &[k]);
            let before =
                f.store.ensure_pair(caller, logic_envelope).unwrap();

            // ExactSequence_R([]) is ROOT. Empty patch means every flag is
            // preserved, which is exactly x86 NOT behavior.
            let payload = materialize_exact_sequence(
                &mut f.store,
                &[f.one, word, ROOT_HANDLE],
            )
            .unwrap();
            let envelope =
                f.store.ensure_pair(result_tag, payload).unwrap();
            let after = f.store.ensure_pair(k, envelope).unwrap();

            let mut roles = Vec::with_capacity(width + 1);
            roles.push(k);
            roles.extend_from_slice(&bits);

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &[logic.result_tag],
                admission,
            );
        }

        let binary_steps = 19 + 4 * width;
        let not_steps = 3 + 2 * width;

        Self {
            width,
            logic,
            binary_effect,
            not_effect,
            result_tag,
            schema,
            binary_steps,
            not_steps,
            links_after_build: f.store.link_count(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EffectOutcome {
    writeback: u8,
    value: u32,
    cf: Option<u8>,
    pf: Option<u8>,
    af_undefined: bool,
    zf: Option<u8>,
    sf: Option<u8>,
    of: Option<u8>,
    patch_len: usize,
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

fn decode_set(
    f: &FullFixture,
    schema: FlagPatchSchema,
    action: Handle,
    expected_flag: Handle,
) -> u8 {
    let (tag, payload) = f.store.poles(action).unwrap();
    assert_eq!(tag, schema.set_tag);
    let values = read_exact_sequence(&f.store, payload).unwrap();
    assert_eq!(values.len(), 2);
    assert_eq!(values[0], expected_flag);
    decode_bit(f, values[1])
}

fn decode_undefined(
    f: &FullFixture,
    schema: FlagPatchSchema,
    action: Handle,
    expected_flag: Handle,
) {
    let (tag, payload) = f.store.poles(action).unwrap();
    assert_eq!(tag, schema.undefined_tag);
    let values = read_exact_sequence(&f.store, payload).unwrap();
    assert_eq!(values, vec![expected_flag]);
}

fn decode_effect(
    f: &FullFixture,
    program: &LogicEffectProgram,
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

    if patch.is_empty() {
        return EffectOutcome {
            writeback,
            value,
            cf: None,
            pf: None,
            af_undefined: false,
            zf: None,
            sf: None,
            of: None,
            patch_len: 0,
        };
    }

    assert_eq!(patch.len(), 6);
    let schema = program.schema;
    let cf = decode_set(f, schema, patch[0], schema.cf);
    let pf = decode_set(f, schema, patch[1], schema.pf);
    decode_undefined(f, schema, patch[2], schema.af);
    let zf = decode_set(f, schema, patch[3], schema.zf);
    let sf = decode_set(f, schema, patch[4], schema.sf);
    let of = decode_set(f, schema, patch[5], schema.of);

    EffectOutcome {
        writeback,
        value,
        cf: Some(cf),
        pf: Some(pf),
        af_undefined: true,
        zf: Some(zf),
        sf: Some(sf),
        of: Some(of),
        patch_len: 6,
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

fn run_binary_effect(
    f: &mut FullFixture,
    program: &LogicEffectProgram,
    gate: Handle,
    a: u32,
    b: u32,
    writeback: u8,
) -> EffectOutcome {
    let m = mask(program.width);
    assert_eq!(a & !m, 0);
    assert_eq!(b & !m, 0);
    assert!(writeback <= 1);

    let a_bits = bit_handles(f, program.width, a);
    let b_bits = bit_handles(f, program.width, b);
    let aword =
        materialize_exact_sequence(&mut f.store, &a_bits).unwrap();
    let bword =
        materialize_exact_sequence(&mut f.store, &b_bits).unwrap();
    let wbh = if writeback == 0 { f.zero } else { f.one };

    let args = materialize_exact_sequence(
        &mut f.store,
        &[gate, aword, bword, wbh],
    )
    .unwrap();
    let invocation =
        call(&mut f.store, f.apply, program.binary_effect, args);
    let initial = f.store.ensure_pair(f.k, invocation).unwrap();
    f.engine.set_current(&f.store, &[initial]).unwrap();

    run_steps(
        f,
        program.binary_steps,
        &format!("LOGIC_EFFECT_N={}", program.width),
    );
    decode_effect(f, program)
}

fn run_not_effect(
    f: &mut FullFixture,
    program: &LogicEffectProgram,
    a: u32,
) -> EffectOutcome {
    let m = mask(program.width);
    assert_eq!(a & !m, 0);

    let a_bits = bit_handles(f, program.width, a);
    let aword =
        materialize_exact_sequence(&mut f.store, &a_bits).unwrap();
    let args =
        materialize_exact_sequence(&mut f.store, &[aword]).unwrap();
    let invocation =
        call(&mut f.store, f.apply, program.not_effect, args);
    let initial = f.store.ensure_pair(f.k, invocation).unwrap();
    f.engine.set_current(&f.store, &[initial]).unwrap();

    run_steps(
        f,
        program.not_steps,
        &format!("NOT_EFFECT_N={}", program.width),
    );
    decode_effect(f, program)
}

fn expected_logic(value: u32, width: usize, writeback: u8) -> EffectOutcome {
    let m = mask(width);
    let value = value & m;
    EffectOutcome {
        writeback,
        value,
        cf: Some(0),
        pf: Some(u8::from((value as u8).count_ones() % 2 == 0)),
        af_undefined: true,
        zf: Some(u8::from(value == 0)),
        sf: Some(((value >> (width - 1)) & 1) as u8),
        of: Some(0),
        patch_len: 6,
    }
}

fn expected_not(value: u32, width: usize) -> EffectOutcome {
    EffectOutcome {
        writeback: 1,
        value: (!value) & mask(width),
        cf: None,
        pf: None,
        af_undefined: false,
        zf: None,
        sf: None,
        of: None,
        patch_len: 0,
    }
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
        (3, 1),
        (sign, sign),
        (0x80 & m, 0xff & m),
        (0x55 & m, 0x0f & m),
        (0xaaaa_aaaa & m, 0x5555_5555 & m),
    ];

    let mut z = 0xc2b2_ae35u32 ^ width as u32;
    for _ in 0..10 {
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


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WebLogicOutcome {
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

fn web_logic_masks(out: EffectOutcome) -> (u32, u32, u32, u32) {
    let mut defined = 0u32;
    let mut values = 0u32;
    for (mask, value) in [
        (WEB_CF, out.cf),
        (WEB_PF, out.pf),
        (WEB_ZF, out.zf),
        (WEB_SF, out.sf),
        (WEB_OF, out.of),
    ] {
        if let Some(bit) = value {
            defined |= mask;
            if bit != 0 {
                values |= mask;
            }
        }
    }
    let undefined = if out.af_undefined { WEB_AF } else { 0 };
    let preserve = WEB_STATUS_FLAGS & !(defined | undefined);
    (defined, values, undefined, preserve)
}

pub(crate) fn web_run_logic(op: u32, a: u32, b: u32) -> Option<WebLogicOutcome> {
    let mut f = FullFixture::new();
    let program = LogicEffectProgram::install(&mut f, 32);
    let links_after_build = f.store.link_count() as u32;

    let (first, reactions) = match op {
        1 => (
            run_binary_effect(&mut f, &program, program.logic.gates.and2, a, b, 1),
            program.binary_steps,
        ),
        2 => (
            run_binary_effect(&mut f, &program, program.logic.gates.or2, a, b, 1),
            program.binary_steps,
        ),
        3 => (
            run_binary_effect(&mut f, &program, program.logic.gates.xor2, a, b, 1),
            program.binary_steps,
        ),
        4 => (run_not_effect(&mut f, &program, a), program.not_steps),
        5 => (
            run_binary_effect(&mut f, &program, program.logic.gates.and2, a, b, 0),
            program.binary_steps,
        ),
        _ => return None,
    };
    let links_after_first = f.store.link_count() as u32;

    let second = match op {
        1 => run_binary_effect(&mut f, &program, program.logic.gates.and2, a, b, 1),
        2 => run_binary_effect(&mut f, &program, program.logic.gates.or2, a, b, 1),
        3 => run_binary_effect(&mut f, &program, program.logic.gates.xor2, a, b, 1),
        4 => run_not_effect(&mut f, &program, a),
        5 => run_binary_effect(&mut f, &program, program.logic.gates.and2, a, b, 0),
        _ => unreachable!(),
    };
    assert_eq!(second, first, "web logic repeat changed result");
    let links_after_second = f.store.link_count() as u32;
    let (defined_mask, value_mask, undefined_mask, preserve_mask) =
        web_logic_masks(first);

    Some(WebLogicOutcome {
        value: first.value,
        writeback: first.writeback,
        defined_mask,
        value_mask,
        undefined_mask,
        preserve_mask,
        reactions: reactions as u32,
        links_after_build,
        links_after_first,
        steady_link_delta: links_after_second - links_after_first,
        quiescent: 1,
    })
}


#[test]
#[ignore = "heavy M4 logical-effect suite; mandatory release workflow"]
fn m4_logic_effects_8_16_32_match_partial_flag_oracle() {
    for width in [8usize, 16, 32] {
        let mut f = FullFixture::new();
        let program = LogicEffectProgram::install(&mut f, width);
        assert_eq!(program.binary_steps, 19 + 4 * width);
        assert_eq!(program.not_steps, 3 + 2 * width);

        let cases = vectors(width);
        for &(a, b) in &cases {
            let and_out = run_binary_effect(
                &mut f,
                &program,
                program.logic.gates.and2,
                a,
                b,
                1,
            );
            assert_eq!(and_out, expected_logic(a & b, width, 1));

            let or_out = run_binary_effect(
                &mut f,
                &program,
                program.logic.gates.or2,
                a,
                b,
                1,
            );
            assert_eq!(or_out, expected_logic(a | b, width, 1));

            let xor_out = run_binary_effect(
                &mut f,
                &program,
                program.logic.gates.xor2,
                a,
                b,
                1,
            );
            assert_eq!(xor_out, expected_logic(a ^ b, width, 1));

            let test_out = run_binary_effect(
                &mut f,
                &program,
                program.logic.gates.and2,
                a,
                b,
                0,
            );
            assert_eq!(test_out, expected_logic(a & b, width, 0));

            let not_out = run_not_effect(&mut f, &program, a);
            assert_eq!(not_out, expected_not(a, width));
        }

        println!(
            "M4_LOGIC_EFFECT width={} cases={} binary_steps={} not_steps={} program_links={}",
            width,
            cases.len(),
            program.binary_steps,
            program.not_steps,
            program.links_after_build,
        );
    }
}

#[test]
#[ignore = "heavy M4 logical-effect suite; mandatory release workflow"]
fn m4_logic_effects_test_and_not_semantics_and_steady_state() {
    let mut f = FullFixture::new();
    let program = LogicEffectProgram::install(&mut f, 32);

    let a = 0x8000_0003u32;
    let b = 0xffff_0001u32;

    let and_out = run_binary_effect(
        &mut f,
        &program,
        program.logic.gates.and2,
        a,
        b,
        1,
    );
    let test_out = run_binary_effect(
        &mut f,
        &program,
        program.logic.gates.and2,
        a,
        b,
        0,
    );

    assert_eq!(and_out.value, test_out.value);
    assert_eq!(and_out.cf, test_out.cf);
    assert_eq!(and_out.pf, test_out.pf);
    assert_eq!(and_out.zf, test_out.zf);
    assert_eq!(and_out.sf, test_out.sf);
    assert_eq!(and_out.of, test_out.of);
    assert!(and_out.af_undefined && test_out.af_undefined);
    assert_eq!(and_out.writeback, 1);
    assert_eq!(test_out.writeback, 0);

    let not_out = run_not_effect(&mut f, &program, a);
    assert_eq!(not_out, expected_not(a, 32));
    assert_eq!(not_out.patch_len, 0);

    let first = run_binary_effect(
        &mut f,
        &program,
        program.logic.gates.xor2,
        0x1234_5678,
        0x9abc_def0,
        1,
    );
    let links = f.store.link_count();
    let second = run_binary_effect(
        &mut f,
        &program,
        program.logic.gates.xor2,
        0x1234_5678,
        0x9abc_def0,
        1,
    );
    assert_eq!(second, first);
    assert_eq!(
        f.store.link_count(),
        links,
        "repeated identical LOGIC_EFFECT32 materialized new Links"
    );
}
