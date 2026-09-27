use super::{
    flag_patch::{
        install_alu_effect_result_tag, set_flag_action,
        undefined_flag_action, FlagPatchSchema,
    },
    full_adder::{
        call, define_bundle_rule, index_rule_for, Fixture as FullFixture,
    },
    logic_n::{GateSet, LogicProgram},
    proof_n::{
        execute_to_quiescence, identical_rerun, load_runtime, loaded_handle,
        prepare_stage, semantic_source, theory_admissions, visual_snapshot,
        ProofRuntimeMemory, WebProofResultStage, WebStructuralProof,
    },
};
use amemory_optimized_cpu_probe::{
    structural::{
        materialize_exact_sequence, read_exact_sequence, StructuralRunProfile,
    },
    Handle, OptimizedLinkStore, ROOT_HANDLE,
};

const WIDTH: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ShiftKind {
    Shl,
    Shr,
    Sar,
}

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
        // Dedicated M4 SHIFT32 namespace.
        for pole in [c, o, o, c, c, o, c, o, o, o, c, c] {
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
struct Shift32Program {
    shl: Handle,
    sal: Handle,
    shr: Handle,
    sar: Handle,
    raw_tag: Handle,
    result_tag: Handle,
    schema: FlagPatchSchema,
    gates: GateSet,
    links_after_build: usize,
}

fn function_for(
    program: &Shift32Program,
    kind: ShiftKind,
) -> Handle {
    match kind {
        ShiftKind::Shl => program.shl,
        ShiftKind::Shr => program.shr,
        ShiftKind::Sar => program.sar,
    }
}

fn output_template(
    f: &FullFixture,
    input: &[Handle],
    kind: ShiftKind,
    count: usize,
) -> (Vec<Handle>, Handle) {
    assert_eq!(input.len(), WIDTH);
    assert!((1..WIDTH).contains(&count));

    match kind {
        ShiftKind::Shl => {
            let mut out = Vec::with_capacity(WIDTH);
            out.extend(std::iter::repeat(f.zero).take(count));
            out.extend_from_slice(&input[..WIDTH - count]);
            (out, input[WIDTH - count])
        }
        ShiftKind::Shr => {
            let mut out = Vec::with_capacity(WIDTH);
            out.extend_from_slice(&input[count..]);
            out.extend(std::iter::repeat(f.zero).take(count));
            (out, input[count - 1])
        }
        ShiftKind::Sar => {
            let sign = input[WIDTH - 1];
            let mut out = Vec::with_capacity(WIDTH);
            out.extend_from_slice(&input[count..]);
            out.extend(std::iter::repeat(sign).take(count));
            (out, input[count - 1])
        }
    }
}

fn install_raw_rule(
    f: &mut FullFixture,
    anchors: &mut AnchorGen,
    function: Handle,
    kind: ShiftKind,
    masked_count: usize,
    raw_tag: Handle,
    result_tag: Handle,
) {
    let k = anchors.next(&mut f.store);
    let input_bits = anchors.roles(&mut f.store, WIDTH);
    let count_hi = anchors.roles(&mut f.store, 3);

    let mut count_bits = Vec::with_capacity(8);
    for bit in 0..5 {
        count_bits.push(if (masked_count >> bit) & 1 == 1 {
            f.one
        } else {
            f.zero
        });
    }
    count_bits.extend_from_slice(&count_hi);

    let input_word =
        materialize_exact_sequence(&mut f.store, &input_bits).unwrap();
    let count_word =
        materialize_exact_sequence(&mut f.store, &count_bits).unwrap();
    let args = materialize_exact_sequence(
        &mut f.store,
        &[input_word, count_word],
    )
    .unwrap();

    let invocation = call(&mut f.store, f.apply, function, args);
    let before = f.store.ensure_pair(k, invocation).unwrap();

    let after = if masked_count == 0 {
        let payload = materialize_exact_sequence(
            &mut f.store,
            &[f.one, input_word, ROOT_HANDLE],
        )
        .unwrap();
        let envelope =
            f.store.ensure_pair(result_tag, payload).unwrap();
        f.store.ensure_pair(k, envelope).unwrap()
    } else {
        let (out_bits, cf) =
            output_template(f, &input_bits, kind, masked_count);
        let output_word =
            materialize_exact_sequence(&mut f.store, &out_bits).unwrap();
        let is_one = if masked_count == 1 { f.one } else { f.zero };
        let original_msb = input_bits[WIDTH - 1];
        let payload = materialize_exact_sequence(
            &mut f.store,
            &[output_word, cf, function, is_one, original_msb],
        )
        .unwrap();
        let envelope =
            f.store.ensure_pair(raw_tag, payload).unwrap();
        f.store.ensure_pair(k, envelope).unwrap()
    };

    let mut roles = Vec::with_capacity(1 + WIDTH + 3);
    roles.push(k);
    roles.extend_from_slice(&input_bits);
    roles.extend_from_slice(&count_hi);

    let (_, admission) = define_bundle_rule(
        &mut f.store,
        f.theory,
        &roles,
        before,
        &[after],
    );
    index_rule_for(&mut f.store, &[f.o], admission);
}

fn final_effect(
    f: &mut FullFixture,
    schema: FlagPatchSchema,
    result_tag: Handle,
    k: Handle,
    bits: &[Handle],
    cf: Handle,
    pf: Handle,
    zf: Handle,
    of_action: Handle,
) -> Handle {
    let word = materialize_exact_sequence(&mut f.store, bits).unwrap();

    let set_cf =
        set_flag_action(&mut f.store, schema, schema.cf, cf);
    let set_pf =
        set_flag_action(&mut f.store, schema, schema.pf, pf);
    let undef_af =
        undefined_flag_action(&mut f.store, schema, schema.af);
    let set_zf =
        set_flag_action(&mut f.store, schema, schema.zf, zf);
    let set_sf = set_flag_action(
        &mut f.store,
        schema,
        schema.sf,
        bits[WIDTH - 1],
    );

    let patch = materialize_exact_sequence(
        &mut f.store,
        &[set_cf, set_pf, undef_af, set_zf, set_sf, of_action],
    )
    .unwrap();
    let payload =
        materialize_exact_sequence(&mut f.store, &[f.one, word, patch])
            .unwrap();
    let envelope =
        f.store.ensure_pair(result_tag, payload).unwrap();
    f.store.ensure_pair(k, envelope).unwrap()
}

impl Shift32Program {
    fn install(f: &mut FullFixture) -> Self {
        // Reuse already-proven structural bit gates. The word-level logic rules
        // installed by this component are not used by SHIFT32 itself.
        let logic = LogicProgram::install(f, WIDTH);
        let gates = logic.gates;

        let seed0 = f
            .store
            .ensure_pair(logic.word_binary, logic.result_tag)
            .unwrap();
        let mut anchors =
            AnchorGen::new(&mut f.store, seed0, f.o, f.c);

        let shl_left = anchors.next(&mut f.store);
        let shl_right = anchors.next(&mut f.store);
        let shl = f.store.ensure_pair(shl_left, shl_right).unwrap();
        let sal = shl;

        let shr_left = anchors.next(&mut f.store);
        let shr_right = anchors.next(&mut f.store);
        let shr = f.store.ensure_pair(shr_left, shr_right).unwrap();

        let sar_left = anchors.next(&mut f.store);
        let sar_right = anchors.next(&mut f.store);
        let sar = f.store.ensure_pair(sar_left, sar_right).unwrap();

        let raw_left = anchors.next(&mut f.store);
        let raw_right = anchors.next(&mut f.store);
        let raw_tag = f.store.ensure_pair(raw_left, raw_right).unwrap();

        let shared_seed = f.store.ensure_pair(f.k, f.full).unwrap();
        let schema =
            FlagPatchSchema::install(&mut f.store, shared_seed, f.o, f.c);
        let result_tag = install_alu_effect_result_tag(
            &mut f.store,
            shared_seed,
            f.o,
            f.c,
        );

        // Runtime Count8 masking is encoded structurally:
        // 32 generic patterns per function constrain c0..c4 while c5..c7 are
        // roles. No host runtime "count & 31" occurs.
        for (kind, function) in [
            (ShiftKind::Shl, shl),
            (ShiftKind::Shr, shr),
            (ShiftKind::Sar, sar),
        ] {
            for masked_count in 0..32usize {
                install_raw_rule(
                    f,
                    &mut anchors,
                    function,
                    kind,
                    masked_count,
                    raw_tag,
                    result_tag,
                );
            }
        }

        let zf_not_tag = anchors.next(&mut f.store);
        let pf_not_tag = anchors.next(&mut f.store);
        let shl_of_tag = anchors.next(&mut f.store);

        let mut zf_tags = Vec::with_capacity(WIDTH - 1);
        for _ in 0..(WIDTH - 1) {
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

        // SHIFT32_RAW -> begin ZF OR reduction.
        {
            let k = anchors.next(&mut f.store);
            let bits = anchors.roles(&mut f.store, WIDTH);
            let cf = anchors.next(&mut f.store);
            let kind = anchors.next(&mut f.store);
            let is_one = anchors.next(&mut f.store);
            let original_msb = anchors.next(&mut f.store);

            let word =
                materialize_exact_sequence(&mut f.store, &bits).unwrap();
            let raw_payload = materialize_exact_sequence(
                &mut f.store,
                &[word, cf, kind, is_one, original_msb],
            )
            .unwrap();
            let raw_envelope =
                f.store.ensure_pair(raw_tag, raw_payload).unwrap();
            let before = f.store.ensure_pair(k, raw_envelope).unwrap();

            let mut state = Vec::with_capacity(WIDTH + 5);
            state.push(k);
            state.push(cf);
            state.push(kind);
            state.push(is_one);
            state.push(original_msb);
            state.extend_from_slice(&bits);

            let caller =
                stage_frame(&mut f.store, zf_tags[0], &state);
            let or_call =
                binary_call(f, gates.or2, bits[0], bits[1]);
            let after = f.store.ensure_pair(caller, or_call).unwrap();

            let mut roles = Vec::with_capacity(WIDTH + 5);
            roles.push(k);
            roles.push(cf);
            roles.push(kind);
            roles.push(is_one);
            roles.push(original_msb);
            roles.extend_from_slice(&bits);

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[raw_tag], admission);
        }

        // OR-reduce 32 result bits, then NOT -> ZF.
        for i in 0..(WIDTH - 1) {
            let k = anchors.next(&mut f.store);
            let cf = anchors.next(&mut f.store);
            let kind = anchors.next(&mut f.store);
            let is_one = anchors.next(&mut f.store);
            let original_msb = anchors.next(&mut f.store);
            let bits = anchors.roles(&mut f.store, WIDTH);
            let acc = anchors.next(&mut f.store);

            let mut state = Vec::with_capacity(WIDTH + 5);
            state.push(k);
            state.push(cf);
            state.push(kind);
            state.push(is_one);
            state.push(original_msb);
            state.extend_from_slice(&bits);

            let caller =
                stage_frame(&mut f.store, zf_tags[i], &state);
            let acc_result =
                materialize_exact_sequence(&mut f.store, &[acc]).unwrap();
            let before =
                f.store.ensure_pair(caller, acc_result).unwrap();

            let after = if i + 1 < WIDTH - 1 {
                let next_caller =
                    stage_frame(&mut f.store, zf_tags[i + 1], &state);
                let next_call = binary_call(
                    f,
                    gates.or2,
                    acc,
                    bits[i + 2],
                );
                f.store.ensure_pair(next_caller, next_call).unwrap()
            } else {
                let next_caller =
                    stage_frame(&mut f.store, zf_not_tag, &state);
                let next_call =
                    unary_call(f, gates.not1, acc);
                f.store.ensure_pair(next_caller, next_call).unwrap()
            };

            let mut roles = state.clone();
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

        // ZF -> begin parity XOR reduction over low byte.
        {
            let k = anchors.next(&mut f.store);
            let cf = anchors.next(&mut f.store);
            let kind = anchors.next(&mut f.store);
            let is_one = anchors.next(&mut f.store);
            let original_msb = anchors.next(&mut f.store);
            let bits = anchors.roles(&mut f.store, WIDTH);
            let zf = anchors.next(&mut f.store);

            let mut state = Vec::with_capacity(WIDTH + 5);
            state.push(k);
            state.push(cf);
            state.push(kind);
            state.push(is_one);
            state.push(original_msb);
            state.extend_from_slice(&bits);

            let caller =
                stage_frame(&mut f.store, zf_not_tag, &state);
            let zf_result =
                materialize_exact_sequence(&mut f.store, &[zf]).unwrap();
            let before =
                f.store.ensure_pair(caller, zf_result).unwrap();

            let mut parity_state = state.clone();
            parity_state.push(zf);
            let next_caller =
                stage_frame(&mut f.store, pf_tags[0], &parity_state);
            let xor_call =
                binary_call(f, gates.xor2, bits[0], bits[1]);
            let after =
                f.store.ensure_pair(next_caller, xor_call).unwrap();

            let mut roles = state;
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
                &gates.bit_outputs,
                admission,
            );
        }

        // Low-byte XOR reduction, then NOT -> even PF.
        for i in 0..7 {
            let k = anchors.next(&mut f.store);
            let cf = anchors.next(&mut f.store);
            let kind = anchors.next(&mut f.store);
            let is_one = anchors.next(&mut f.store);
            let original_msb = anchors.next(&mut f.store);
            let bits = anchors.roles(&mut f.store, WIDTH);
            let zf = anchors.next(&mut f.store);
            let acc = anchors.next(&mut f.store);

            let mut state = Vec::with_capacity(WIDTH + 6);
            state.push(k);
            state.push(cf);
            state.push(kind);
            state.push(is_one);
            state.push(original_msb);
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
                    gates.xor2,
                    acc,
                    bits[i + 2],
                );
                f.store.ensure_pair(next_caller, next_call).unwrap()
            } else {
                let next_caller =
                    stage_frame(&mut f.store, pf_not_tag, &state);
                let next_call =
                    unary_call(f, gates.not1, acc);
                f.store.ensure_pair(next_caller, next_call).unwrap()
            };

            let mut roles = state.clone();
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

        // masked count > 1: OF is explicitly undefined, all kinds.
        {
            let k = anchors.next(&mut f.store);
            let cf = anchors.next(&mut f.store);
            let kind = anchors.next(&mut f.store);
            let original_msb = anchors.next(&mut f.store);
            let bits = anchors.roles(&mut f.store, WIDTH);
            let zf = anchors.next(&mut f.store);
            let pf = anchors.next(&mut f.store);

            let mut state = Vec::with_capacity(WIDTH + 6);
            state.push(k);
            state.push(cf);
            state.push(kind);
            state.push(f.zero); // is_one = false
            state.push(original_msb);
            state.extend_from_slice(&bits);
            state.push(zf);

            let caller =
                stage_frame(&mut f.store, pf_not_tag, &state);
            let pf_result =
                materialize_exact_sequence(&mut f.store, &[pf]).unwrap();
            let before =
                f.store.ensure_pair(caller, pf_result).unwrap();

            let undef_of =
                undefined_flag_action(&mut f.store, schema, schema.of);
            let after = final_effect(
                f,
                schema,
                result_tag,
                k,
                &bits,
                cf,
                pf,
                zf,
                undef_of,
            );

            let mut roles = Vec::with_capacity(WIDTH + 6);
            roles.push(k);
            roles.push(cf);
            roles.push(kind);
            roles.push(original_msb);
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
                &gates.bit_outputs,
                admission,
            );
        }

        // SHR count=1: OF = original MSB.
        {
            let k = anchors.next(&mut f.store);
            let cf = anchors.next(&mut f.store);
            let original_msb = anchors.next(&mut f.store);
            let bits = anchors.roles(&mut f.store, WIDTH);
            let zf = anchors.next(&mut f.store);
            let pf = anchors.next(&mut f.store);

            let mut state = Vec::with_capacity(WIDTH + 6);
            state.push(k);
            state.push(cf);
            state.push(shr);
            state.push(f.one);
            state.push(original_msb);
            state.extend_from_slice(&bits);
            state.push(zf);

            let caller =
                stage_frame(&mut f.store, pf_not_tag, &state);
            let pf_result =
                materialize_exact_sequence(&mut f.store, &[pf]).unwrap();
            let before =
                f.store.ensure_pair(caller, pf_result).unwrap();

            let set_of = set_flag_action(
                &mut f.store,
                schema,
                schema.of,
                original_msb,
            );
            let after = final_effect(
                f,
                schema,
                result_tag,
                k,
                &bits,
                cf,
                pf,
                zf,
                set_of,
            );

            let mut roles = Vec::with_capacity(WIDTH + 5);
            roles.push(k);
            roles.push(cf);
            roles.push(original_msb);
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
                &gates.bit_outputs,
                admission,
            );
        }

        // SAR count=1: OF = 0.
        {
            let k = anchors.next(&mut f.store);
            let cf = anchors.next(&mut f.store);
            let original_msb = anchors.next(&mut f.store);
            let bits = anchors.roles(&mut f.store, WIDTH);
            let zf = anchors.next(&mut f.store);
            let pf = anchors.next(&mut f.store);

            let mut state = Vec::with_capacity(WIDTH + 6);
            state.push(k);
            state.push(cf);
            state.push(sar);
            state.push(f.one);
            state.push(original_msb);
            state.extend_from_slice(&bits);
            state.push(zf);

            let caller =
                stage_frame(&mut f.store, pf_not_tag, &state);
            let pf_result =
                materialize_exact_sequence(&mut f.store, &[pf]).unwrap();
            let before =
                f.store.ensure_pair(caller, pf_result).unwrap();

            let set_of =
                set_flag_action(&mut f.store, schema, schema.of, f.zero);
            let after = final_effect(
                f,
                schema,
                result_tag,
                k,
                &bits,
                cf,
                pf,
                zf,
                set_of,
            );

            let mut roles = Vec::with_capacity(WIDTH + 5);
            roles.push(k);
            roles.push(cf);
            roles.push(original_msb);
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
                &gates.bit_outputs,
                admission,
            );
        }

        // SHL/SAL count=1: OF = MSB(result) XOR CF.
        {
            let k = anchors.next(&mut f.store);
            let cf = anchors.next(&mut f.store);
            let original_msb = anchors.next(&mut f.store);
            let bits = anchors.roles(&mut f.store, WIDTH);
            let zf = anchors.next(&mut f.store);
            let pf = anchors.next(&mut f.store);

            let mut state = Vec::with_capacity(WIDTH + 6);
            state.push(k);
            state.push(cf);
            state.push(shl);
            state.push(f.one);
            state.push(original_msb);
            state.extend_from_slice(&bits);
            state.push(zf);

            let caller =
                stage_frame(&mut f.store, pf_not_tag, &state);
            let pf_result =
                materialize_exact_sequence(&mut f.store, &[pf]).unwrap();
            let before =
                f.store.ensure_pair(caller, pf_result).unwrap();

            let next_caller = stage_frame(
                &mut f.store,
                shl_of_tag,
                &[k, cf, zf, pf]
                    .into_iter()
                    .chain(bits.iter().copied())
                    .collect::<Vec<_>>(),
            );
            let of_call =
                binary_call(f, gates.xor2, bits[WIDTH - 1], cf);
            let after =
                f.store.ensure_pair(next_caller, of_call).unwrap();

            let mut roles = Vec::with_capacity(WIDTH + 5);
            roles.push(k);
            roles.push(cf);
            roles.push(original_msb);
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
                &gates.bit_outputs,
                admission,
            );
        }

        // SHL count=1 OF gate result -> final effect.
        {
            let k = anchors.next(&mut f.store);
            let cf = anchors.next(&mut f.store);
            let zf = anchors.next(&mut f.store);
            let pf = anchors.next(&mut f.store);
            let bits = anchors.roles(&mut f.store, WIDTH);
            let of = anchors.next(&mut f.store);

            let mut state = Vec::with_capacity(WIDTH + 4);
            state.push(k);
            state.push(cf);
            state.push(zf);
            state.push(pf);
            state.extend_from_slice(&bits);

            let caller =
                stage_frame(&mut f.store, shl_of_tag, &state);
            let of_result =
                materialize_exact_sequence(&mut f.store, &[of]).unwrap();
            let before =
                f.store.ensure_pair(caller, of_result).unwrap();

            let set_of =
                set_flag_action(&mut f.store, schema, schema.of, of);
            let after = final_effect(
                f,
                schema,
                result_tag,
                k,
                &bits,
                cf,
                pf,
                zf,
                set_of,
            );

            let mut roles = state;
            roles.push(of);

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
            shl,
            sal,
            shr,
            sar,
            raw_tag,
            result_tag,
            schema,
            gates,
            links_after_build: f.store.link_count(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FlagState {
    Preserve,
    Set(u8),
    Undefined,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EffectOutcome {
    writeback: u8,
    value: u32,
    cf: FlagState,
    pf: FlagState,
    af: FlagState,
    zf: FlagState,
    sf: FlagState,
    of: FlagState,
    reactions: usize,
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

fn decode_action(
    f: &FullFixture,
    schema: FlagPatchSchema,
    action: Handle,
    expected_flag: Handle,
) -> FlagState {
    let (tag, payload) = f.store.poles(action).unwrap();
    let values = read_exact_sequence(&f.store, payload).unwrap();

    if tag == schema.set_tag {
        assert_eq!(values.len(), 2);
        assert_eq!(values[0], expected_flag);
        FlagState::Set(decode_bit(f, values[1]))
    } else {
        assert_eq!(tag, schema.undefined_tag);
        assert_eq!(values, vec![expected_flag]);
        FlagState::Undefined
    }
}

fn decode_effect(
    f: &FullFixture,
    program: &Shift32Program,
    reactions: usize,
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
    let value = decode_word(f, values[1]);
    let patch = read_exact_sequence(&f.store, values[2]).unwrap();

    if patch.is_empty() {
        return EffectOutcome {
            writeback,
            value,
            cf: FlagState::Preserve,
            pf: FlagState::Preserve,
            af: FlagState::Preserve,
            zf: FlagState::Preserve,
            sf: FlagState::Preserve,
            of: FlagState::Preserve,
            reactions,
        };
    }

    assert_eq!(patch.len(), 6);
    let s = program.schema;

    EffectOutcome {
        writeback,
        value,
        cf: decode_action(f, s, patch[0], s.cf),
        pf: decode_action(f, s, patch[1], s.pf),
        af: decode_action(f, s, patch[2], s.af),
        zf: decode_action(f, s, patch[3], s.zf),
        sf: decode_action(f, s, patch[4], s.sf),
        of: decode_action(f, s, patch[5], s.of),
        reactions,
    }
}

fn run_shift(
    f: &mut FullFixture,
    program: &Shift32Program,
    function: Handle,
    value: u32,
    count: u8,
) -> EffectOutcome {
    let word_bits = bit_handles(f, WIDTH, value);
    let count_bits = bit_handles(f, 8, u32::from(count));
    let word =
        materialize_exact_sequence(&mut f.store, &word_bits).unwrap();
    let count_word =
        materialize_exact_sequence(&mut f.store, &count_bits).unwrap();
    let args = materialize_exact_sequence(
        &mut f.store,
        &[word, count_word],
    )
    .unwrap();
    let invocation =
        call(&mut f.store, f.apply, function, args);
    let initial = f.store.ensure_pair(f.k, invocation).unwrap();

    f.engine.set_current(&f.store, &[initial]).unwrap();

    let mut reactions = 0usize;
    loop {
        let result = f.engine.run(&mut f.store).unwrap();
        if result.quiescent {
            assert_eq!(result.raw_rule_matches, 0);
            assert_eq!(result.handoff_count, 0);
            break;
        }

        reactions += 1;
        assert_eq!(result.raw_rule_matches, 1, "reaction {reactions}");
        assert_eq!(result.transitioned_members, 1, "reaction {reactions}");
        assert_eq!(result.handoff_count, 1, "reaction {reactions}");
        assert_eq!(result.next_members.len(), 1, "reaction {reactions}");
        assert!(reactions <= 90, "SHIFT32 failed to quiesce");
    }

    decode_effect(f, program, reactions)
}


fn run_shift_profiled(
    f: &mut FullFixture,
    program: &Shift32Program,
    function: Handle,
    value: u32,
    count: u8,
) -> (EffectOutcome, StructuralRunProfile) {
    let word_bits = bit_handles(f, WIDTH, value);
    let count_bits = bit_handles(f, 8, u32::from(count));
    let word = materialize_exact_sequence(&mut f.store, &word_bits).unwrap();
    let count_word = materialize_exact_sequence(&mut f.store, &count_bits).unwrap();
    let args = materialize_exact_sequence(&mut f.store, &[word, count_word]).unwrap();
    let invocation = call(&mut f.store, f.apply, function, args);
    let initial = f.store.ensure_pair(f.k, invocation).unwrap();

    f.engine.set_current(&f.store, &[initial]).unwrap();

    let mut reactions = 0usize;
    let mut profile = StructuralRunProfile::default();
    loop {
        let (result, step_profile) =
            f.engine.run_profiled(&mut f.store).unwrap();
        profile.accumulate(&step_profile);
        if result.quiescent {
            assert_eq!(result.raw_rule_matches, 0);
            assert_eq!(result.handoff_count, 0);
            break;
        }

        reactions += 1;
        assert_eq!(result.raw_rule_matches, 1, "profiled reaction {reactions}");
        assert_eq!(result.transitioned_members, 1, "profiled reaction {reactions}");
        assert_eq!(result.handoff_count, 1, "profiled reaction {reactions}");
        assert_eq!(result.next_members.len(), 1, "profiled reaction {reactions}");
        assert!(reactions <= 90, "profiled SHIFT32 failed to quiesce");
    }

    (decode_effect(f, program, reactions), profile)
}

fn expected(
    program: &Shift32Program,
    function: Handle,
    value: u32,
    count: u8,
) -> EffectOutcome {
    let masked = usize::from(count & 31);

    if masked == 0 {
        return EffectOutcome {
            writeback: 1,
            value,
            cf: FlagState::Preserve,
            pf: FlagState::Preserve,
            af: FlagState::Preserve,
            zf: FlagState::Preserve,
            sf: FlagState::Preserve,
            of: FlagState::Preserve,
            reactions: 1,
        };
    }

    let (result, cf) = if function == program.shl {
        (
            value.wrapping_shl(masked as u32),
            ((value >> (WIDTH - masked)) & 1) as u8,
        )
    } else if function == program.shr {
        (
            value >> masked,
            ((value >> (masked - 1)) & 1) as u8,
        )
    } else {
        assert_eq!(function, program.sar);
        (
            ((value as i32) >> masked) as u32,
            ((value >> (masked - 1)) & 1) as u8,
        )
    };

    let of = if masked == 1 {
        if function == program.shl {
            FlagState::Set(
                (((result >> 31) & 1) as u8) ^ cf,
            )
        } else if function == program.shr {
            FlagState::Set(((value >> 31) & 1) as u8)
        } else {
            FlagState::Set(0)
        }
    } else {
        FlagState::Undefined
    };

    EffectOutcome {
        writeback: 1,
        value: result,
        cf: FlagState::Set(cf),
        pf: FlagState::Set(u8::from(
            (result as u8).count_ones() % 2 == 0,
        )),
        af: FlagState::Undefined,
        zf: FlagState::Set(u8::from(result == 0)),
        sf: FlagState::Set(((result >> 31) & 1) as u8),
        of,
        reactions: if masked == 1 && function == program.shl {
            84
        } else {
            82
        },
    }
}

fn required_counts() -> [u8; 16] {
    [
        0, 1, 2, 7, 8, 15, 16, 30, 31,
        32, 33, 63, 64, 65, 95, 255,
    ]
}

fn values() -> Vec<u32> {
    let mut out = vec![
        0,
        1,
        0x8000_0000,
        u32::MAX,
        0xaaaa_aaaa,
        0x5555_5555,
        0x8000_0001,
        0x7fff_ffff,
        0xa5a5_5a5a,
    ];

    let mut z = 0x9e37_79b9u32;
    for _ in 0..5 {
        z = z.wrapping_mul(1664525).wrapping_add(1013904223);
        out.push(z);
    }

    out.sort_unstable();
    out.dedup();
    out
}


#[derive(Clone, Debug)]
pub(crate) struct WebShiftProofExecution {
    pub(crate) outcome: WebShiftOutcome,
    pub(crate) proof: WebStructuralProof,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WebShiftOutcome {
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

fn web_flag_masks(states: [(u32, FlagState); 6]) -> (u32, u32, u32, u32) {
    let mut defined = 0u32;
    let mut values = 0u32;
    let mut undefined = 0u32;
    let mut preserve = 0u32;
    for (mask, state) in states {
        match state {
            FlagState::Set(bit) => {
                defined |= mask;
                if bit != 0 { values |= mask; }
            }
            FlagState::Undefined => undefined |= mask,
            FlagState::Preserve => preserve |= mask,
        }
    }
    (defined, values, undefined, preserve)
}

fn runtime_shift_bit(
    memory: &ProofRuntimeMemory,
    value: Handle,
    zero: Handle,
    one: Handle,
) -> Option<u8> {
    if value == one {
        Some(1)
    } else if value == zero {
        Some(0)
    } else {
        let _ = memory;
        None
    }
}

fn runtime_shift_word(
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
        value |= u32::from(runtime_shift_bit(memory, bit, zero, one)?) << index;
    }
    Some(value)
}

fn runtime_shift_action(
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
        Some(FlagState::Set(runtime_shift_bit(
            memory, values[1], zero, one,
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
fn decode_runtime_shift_effect(
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

    let writeback = runtime_shift_bit(memory, values[0], zero, one)?;
    let value = runtime_shift_word(memory, values[1], zero, one)?;
    let patch = read_exact_sequence(&memory.store, values[2]).ok()?;

    let (cf, pf, af, zf, sf, of) = if patch.is_empty() {
        (
            FlagState::Preserve,
            FlagState::Preserve,
            FlagState::Preserve,
            FlagState::Preserve,
            FlagState::Preserve,
            FlagState::Preserve,
        )
    } else {
        if patch.len() != 6 {
            return None;
        }
        (
            runtime_shift_action(
                memory, patch[0], set_tag, undefined_tag, cf_flag, zero, one,
            )?,
            runtime_shift_action(
                memory, patch[1], set_tag, undefined_tag, pf_flag, zero, one,
            )?,
            runtime_shift_action(
                memory, patch[2], set_tag, undefined_tag, af_flag, zero, one,
            )?,
            runtime_shift_action(
                memory, patch[3], set_tag, undefined_tag, zf_flag, zero, one,
            )?,
            runtime_shift_action(
                memory, patch[4], set_tag, undefined_tag, sf_flag, zero, one,
            )?,
            runtime_shift_action(
                memory, patch[5], set_tag, undefined_tag, of_flag, zero, one,
            )?,
        )
    };

    Some((
        EffectOutcome {
            writeback,
            value,
            cf,
            pf,
            af,
            zf,
            sf,
            of,
            reactions,
        },
        payload,
    ))
}

pub(crate) fn web_prove_shift32(
    op: u32,
    value: u32,
    count: u32,
) -> Option<WebShiftProofExecution> {
    if count > u8::MAX as u32 {
        return None;
    }

    let mut compiler = FullFixture::new();
    let program = Shift32Program::install(&mut compiler);
    let (block, function) = match op {
        13 => ("SHL32", program.shl),
        14 => ("SHR32", program.shr),
        15 => ("SAR32", program.sar),
        _ => return None,
    };
    let links_after_build = program.links_after_build as u32;

    let value_bits = bit_handles(&compiler, WIDTH, value);
    let count_bits = bit_handles(&compiler, 8, count);
    let value_word =
        materialize_exact_sequence(&mut compiler.store, &value_bits).ok()?;
    let count_word =
        materialize_exact_sequence(&mut compiler.store, &count_bits).ok()?;
    let args = materialize_exact_sequence(
        &mut compiler.store,
        &[value_word, count_word],
    )
    .ok()?;
    let invocation =
        call(&mut compiler.store, compiler.apply, function, args);
    let initial = compiler
        .store
        .ensure_pair(compiler.k, invocation)
        .ok()?;

    let prepared_roots = vec![
        semantic_source(&compiler.store, "function.shift.selected", function),
        semantic_source(&compiler.store, "function.shift.shl", program.shl),
        semantic_source(&compiler.store, "function.shift.shr", program.shr),
        semantic_source(&compiler.store, "function.shift.sar", program.sar),
        semantic_source(&compiler.store, "data.value.word", value_word),
        semantic_source(&compiler.store, "data.count.word", count_word),
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
    let undefined_tag =
        loaded_handle(&load, "result.flag.undefined_tag")?;
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
        92,
    )?;
    if engine.current().len() != 1 {
        return None;
    }

    let final_link = engine.current()[0];
    let (actual, payload) = decode_runtime_shift_effect(
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
    let oracle = expected(&program, function, value, count as u8);
    if execute.active_reaction_count != actual.reactions as u32 {
        return None;
    }

    let result_anum = memory.store.export_anum(final_link).ok()?;
    let result_sequence_anum = memory.store.export_anum(payload).ok()?;
    let links_after_first = memory.store.link_count() as u32;
    let identical_rerun_link_delta = identical_rerun(
        &mut memory,
        &mut engine,
        initial,
        &result_anum,
        92,
    )?;
    let visual_links = visual_snapshot(&memory, &load.semantic_roots);

    let result = WebProofResultStage {
        memory_instance_id: memory.id.clone(),
        result_anum,
        result_sequence_anum,
        decoded_value: actual.value,
        oracle_value: oracle.value,
        oracle_matches: actual == oracle,
        links_final: memory.store.link_count() as u32,
        identical_rerun_link_delta,
        visual_links,
    };
    let proof = WebStructuralProof {
        schema_version: 2,
        block: block.to_owned(),
        prepare,
        load,
        execute,
        result,
    };

    let (defined_mask, value_mask, undefined_mask, preserve_mask) =
        web_flag_masks([
            (WEB_CF, actual.cf),
            (WEB_PF, actual.pf),
            (WEB_AF, actual.af),
            (WEB_ZF, actual.zf),
            (WEB_SF, actual.sf),
            (WEB_OF, actual.of),
        ]);
    let outcome = WebShiftOutcome {
        value: actual.value,
        writeback: actual.writeback,
        defined_mask,
        value_mask,
        undefined_mask,
        preserve_mask,
        reactions: actual.reactions as u32,
        links_after_build,
        links_after_first,
        steady_link_delta: proof.result.identical_rerun_link_delta,
        quiescent: u8::from(proof.execute.final_quiescent),
    };

    Some(WebShiftProofExecution { outcome, proof })
}

pub(crate) fn web_run_shift32(
    op: u32,
    value: u32,
    count: u32,
) -> Option<WebShiftOutcome> {
    if count > u8::MAX as u32 {
        return None;
    }

    let mut f = FullFixture::new();
    let p = Shift32Program::install(&mut f);
    let function = match op {
        13 => p.shl,
        14 => p.shr,
        15 => p.sar,
        _ => return None,
    };
    let links_after_build = f.store.link_count() as u32;

    let first = run_shift(&mut f, &p, function, value, count as u8);
    let links_after_first = f.store.link_count() as u32;
    let second = run_shift(&mut f, &p, function, value, count as u8);
    assert_eq!(second, first, "web SHIFT32 repeat changed result");
    let links_after_second = f.store.link_count() as u32;

    let (defined_mask, value_mask, undefined_mask, preserve_mask) =
        web_flag_masks([
            (WEB_CF, first.cf),
            (WEB_PF, first.pf),
            (WEB_AF, first.af),
            (WEB_ZF, first.zf),
            (WEB_SF, first.sf),
            (WEB_OF, first.of),
        ]);

    Some(WebShiftOutcome {
        value: first.value,
        writeback: first.writeback,
        defined_mask,
        value_mask,
        undefined_mask,
        preserve_mask,
        reactions: first.reactions as u32,
        links_after_build,
        links_after_first,
        steady_link_delta: links_after_second - links_after_first,
        quiescent: 1,
    })
}


#[test]
#[ignore = "informational Structural Rule profiler; no acceptance timing threshold"]
fn m4_shift32_profile_candidate_selectivity() {
    let mut f = FullFixture::new();
    let program = Shift32Program::install(&mut f);
    let value = 0x9234_5679u32;
    let count = 31u8;

    for (name, function) in [
        ("SHL", program.shl),
        ("SHR", program.shr),
        ("SAR", program.sar),
    ] {
        let expected_value = expected(&program, function, value, count);
        let (actual, profile) =
            run_shift_profiled(&mut f, &program, function, value, count);
        assert_eq!(actual, expected_value);

        println!(
            "SHIFT32_PROFILE op={} count={} reactions={} program_links={} total_ns={} discovery_ns={} role_decode_ns={} unification_ns={} instantiation_ns={} publication_ns={} trigger_candidates={} pre_unify_rejects={} role_decodes={} decoded_roles={} unify_attempts={} unify_successes={} contains_nodes={} unify_nodes={} instantiate_nodes={} constructor_attempts={} canonical_hits={} new_links={} publication_outputs={}",
            name,
            count,
            actual.reactions,
            program.links_after_build,
            profile.total_ns,
            profile.discovery_ns,
            profile.role_decode_ns,
            profile.unification_ns,
            profile.instantiation_ns,
            profile.publication_ns,
            profile.trigger_incidence_candidates,
            profile.candidates_rejected_before_unification,
            profile.role_dictionary_decodes,
            profile.decoded_roles,
            profile.unification_attempts,
            profile.unification_successes,
            profile.contains_role_nodes_visited,
            profile.unification_nodes_visited,
            profile.instantiation_nodes_visited,
            profile.instantiation_constructor_attempts,
            profile.instantiation_canonical_hits,
            profile.instantiation_new_links,
            profile.publication_outputs,
        );
    }

    println!("SHIFT32_PROFILE_NOTE=informational-only-no-performance-threshold");
}

#[test]
#[ignore = "heavy M4 SHIFT32 effect suite; mandatory release workflow"]
fn m4_shift32_required_counts_match_80386_profile() {
    let mut f = FullFixture::new();
    let program = Shift32Program::install(&mut f);
    assert_eq!(program.sal, program.shl);

    let sentinel_values = [
        0u32,
        0x8000_0001,
        0xa5a5_5a5a,
    ];

    for count in required_counts() {
        for value in sentinel_values {
            for function in [program.shl, program.shr, program.sar] {
                let actual =
                    run_shift(&mut f, &program, function, value, count);
                let expected =
                    expected(&program, function, value, count);
                assert_eq!(
                    actual, expected,
                    "value={value:#010x} count={count} function={function}"
                );
            }
        }
    }

    for count in [0u8, 1, 2, 31, 33] {
        for value in values() {
            for function in [program.shl, program.shr, program.sar] {
                let actual =
                    run_shift(&mut f, &program, function, value, count);
                let expected =
                    expected(&program, function, value, count);
                assert_eq!(
                    actual, expected,
                    "value={value:#010x} count={count} function={function}"
                );
            }
        }
    }

    println!(
        "M4_SHIFT32 rules={} program_links={}",
        3 * 32,
        program.links_after_build,
    );
}

#[test]
#[ignore = "heavy M4 SHIFT32 effect suite; mandatory release workflow"]
fn m4_shift32_count_masking_is_structural_and_steady() {
    let mut f = FullFixture::new();
    let program = Shift32Program::install(&mut f);
    let value = 0x9234_5679u32;

    for function in [program.shl, program.shr, program.sar] {
        let c0 = run_shift(&mut f, &program, function, value, 0);
        let c32 = run_shift(&mut f, &program, function, value, 32);
        let c64 = run_shift(&mut f, &program, function, value, 64);
        assert_eq!(c0, c32);
        assert_eq!(c0, c64);

        let c1 = run_shift(&mut f, &program, function, value, 1);
        let c33 = run_shift(&mut f, &program, function, value, 33);
        let c65 = run_shift(&mut f, &program, function, value, 65);
        assert_eq!(c1, c33);
        assert_eq!(c1, c65);

        let c31 = run_shift(&mut f, &program, function, value, 31);
        let c63 = run_shift(&mut f, &program, function, value, 63);
        let c95 = run_shift(&mut f, &program, function, value, 95);
        assert_eq!(c31, c63);
        assert_eq!(c31, c95);
    }

    let first =
        run_shift(&mut f, &program, program.sar, value, 255);
    let links = f.store.link_count();
    let second =
        run_shift(&mut f, &program, program.sar, value, 255);
    assert_eq!(second, first);
    assert_eq!(
        f.store.link_count(),
        links,
        "repeated identical SHIFT32 materialized new Links"
    );
}
