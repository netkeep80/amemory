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
    structural::{materialize_exact_sequence, read_exact_sequence},
    Handle, OptimizedLinkStore, ROOT_HANDLE,
};

const WIDTH: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RotateKind {
    Rol,
    Ror,
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
        for pole in [o, c, o, o, c, o, c, c, o, c, c, c, o] {
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

#[derive(Clone, Debug)]
struct Rotate32Program {
    rol: Handle,
    ror: Handle,
    result_tag: Handle,
    schema: FlagPatchSchema,
    gates: GateSet,
    links_after_build: usize,
}

fn rotate_template(
    input: &[Handle],
    kind: RotateKind,
    count: usize,
) -> Vec<Handle> {
    assert_eq!(input.len(), WIDTH);
    assert!((1..WIDTH).contains(&count));

    let mut out = Vec::with_capacity(WIDTH);
    match kind {
        RotateKind::Rol => {
            out.extend_from_slice(&input[WIDTH - count..]);
            out.extend_from_slice(&input[..WIDTH - count]);
        }
        RotateKind::Ror => {
            out.extend_from_slice(&input[count..]);
            out.extend_from_slice(&input[..count]);
        }
    }
    out
}

fn install_rule(
    f: &mut FullFixture,
    anchors: &mut AnchorGen,
    function: Handle,
    kind: RotateKind,
    masked_count: usize,
    of_tag: Handle,
    result_tag: Handle,
    schema: FlagPatchSchema,
    gates: GateSet,
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
        let out_bits =
            rotate_template(&input_bits, kind, masked_count);
        let output_word =
            materialize_exact_sequence(&mut f.store, &out_bits).unwrap();
        let cf = match kind {
            RotateKind::Rol => out_bits[0],
            RotateKind::Ror => out_bits[WIDTH - 1],
        };

        if masked_count == 1 {
            let mut state = Vec::with_capacity(WIDTH + 2);
            state.push(k);
            state.push(cf);
            state.extend_from_slice(&out_bits);

            let caller =
                stage_frame(&mut f.store, of_tag, &state);
            let (a, b) = match kind {
                RotateKind::Rol => (out_bits[WIDTH - 1], cf),
                RotateKind::Ror => (
                    out_bits[WIDTH - 1],
                    out_bits[WIDTH - 2],
                ),
            };
            let of_call = binary_call(f, gates.xor2, a, b);
            f.store.ensure_pair(caller, of_call).unwrap()
        } else {
            let set_cf =
                set_flag_action(&mut f.store, schema, schema.cf, cf);
            let undef_of =
                undefined_flag_action(&mut f.store, schema, schema.of);
            let patch = materialize_exact_sequence(
                &mut f.store,
                &[set_cf, undef_of],
            )
            .unwrap();
            let payload = materialize_exact_sequence(
                &mut f.store,
                &[f.one, output_word, patch],
            )
            .unwrap();
            let envelope =
                f.store.ensure_pair(result_tag, payload).unwrap();
            f.store.ensure_pair(k, envelope).unwrap()
        }
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

impl Rotate32Program {
    fn install(f: &mut FullFixture) -> Self {
        let logic = LogicProgram::install(f, WIDTH);
        let gates = logic.gates;

        let seed0 = f
            .store
            .ensure_pair(logic.word_not, logic.result_tag)
            .unwrap();
        let mut anchors =
            AnchorGen::new(&mut f.store, seed0, f.o, f.c);

        let rol_left = anchors.next(&mut f.store);
        let rol_right = anchors.next(&mut f.store);
        let rol = f.store.ensure_pair(rol_left, rol_right).unwrap();

        let ror_left = anchors.next(&mut f.store);
        let ror_right = anchors.next(&mut f.store);
        let ror = f.store.ensure_pair(ror_left, ror_right).unwrap();

        let of_left = anchors.next(&mut f.store);
        let of_right = anchors.next(&mut f.store);
        let of_tag = f.store.ensure_pair(of_left, of_right).unwrap();

        let shared_seed = f.store.ensure_pair(f.k, f.full).unwrap();
        let schema =
            FlagPatchSchema::install(&mut f.store, shared_seed, f.o, f.c);
        let result_tag = install_alu_effect_result_tag(
            &mut f.store,
            shared_seed,
            f.o,
            f.c,
        );

        for (kind, function) in [
            (RotateKind::Rol, rol),
            (RotateKind::Ror, ror),
        ] {
            for masked_count in 0..32usize {
                install_rule(
                    f,
                    &mut anchors,
                    function,
                    kind,
                    masked_count,
                    of_tag,
                    result_tag,
                    schema,
                    gates,
                );
            }
        }

        // count=1 XOR result -> final two-action FlagPatch.
        {
            let k = anchors.next(&mut f.store);
            let cf = anchors.next(&mut f.store);
            let bits = anchors.roles(&mut f.store, WIDTH);
            let of = anchors.next(&mut f.store);

            let mut state = Vec::with_capacity(WIDTH + 2);
            state.push(k);
            state.push(cf);
            state.extend_from_slice(&bits);

            let caller =
                stage_frame(&mut f.store, of_tag, &state);
            let of_result =
                materialize_exact_sequence(&mut f.store, &[of]).unwrap();
            let before =
                f.store.ensure_pair(caller, of_result).unwrap();

            let word =
                materialize_exact_sequence(&mut f.store, &bits).unwrap();
            let set_cf =
                set_flag_action(&mut f.store, schema, schema.cf, cf);
            let set_of =
                set_flag_action(&mut f.store, schema, schema.of, of);
            let patch = materialize_exact_sequence(
                &mut f.store,
                &[set_cf, set_of],
            )
            .unwrap();
            let payload = materialize_exact_sequence(
                &mut f.store,
                &[f.one, word, patch],
            )
            .unwrap();
            let envelope =
                f.store.ensure_pair(result_tag, payload).unwrap();
            let after = f.store.ensure_pair(k, envelope).unwrap();

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
            rol,
            ror,
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
    of: FlagState,
    patch_len: usize,
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

fn decode_word(f: &FullFixture, word: Handle) -> u32 {
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
    program: &Rotate32Program,
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
            of: FlagState::Preserve,
            patch_len: 0,
            reactions,
        };
    }

    assert_eq!(patch.len(), 2);
    let s = program.schema;
    EffectOutcome {
        writeback,
        value,
        cf: decode_action(f, s, patch[0], s.cf),
        of: decode_action(f, s, patch[1], s.of),
        patch_len: 2,
        reactions,
    }
}

fn run_rotate(
    f: &mut FullFixture,
    program: &Rotate32Program,
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
    let args =
        materialize_exact_sequence(&mut f.store, &[word, count_word])
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
        assert!(reactions <= 4, "ROTATE32 failed to quiesce");
    }

    decode_effect(f, program, reactions)
}

fn expected(
    program: &Rotate32Program,
    function: Handle,
    value: u32,
    count: u8,
) -> EffectOutcome {
    let masked = u32::from(count & 31);

    if masked == 0 {
        return EffectOutcome {
            writeback: 1,
            value,
            cf: FlagState::Preserve,
            of: FlagState::Preserve,
            patch_len: 0,
            reactions: 1,
        };
    }

    let result = if function == program.rol {
        value.rotate_left(masked)
    } else {
        assert_eq!(function, program.ror);
        value.rotate_right(masked)
    };

    let cf = if function == program.rol {
        (result & 1) as u8
    } else {
        ((result >> 31) & 1) as u8
    };

    let of = if masked == 1 {
        if function == program.rol {
            FlagState::Set(
                (((result >> 31) & 1) as u8) ^ cf,
            )
        } else {
            FlagState::Set(
                (((result >> 31) & 1) as u8)
                    ^ (((result >> 30) & 1) as u8),
            )
        }
    } else {
        FlagState::Undefined
    };

    EffectOutcome {
        writeback: 1,
        value: result,
        cf: FlagState::Set(cf),
        of,
        patch_len: 2,
        reactions: if masked == 1 { 3 } else { 1 },
    }
}

fn counts() -> [u8; 16] {
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

    let mut z = 0x243f_6a88u32;
    for _ in 0..6 {
        z = z.wrapping_mul(1664525).wrapping_add(1013904223);
        out.push(z);
    }

    out.sort_unstable();
    out.dedup();
    out
}


#[derive(Clone, Debug)]
pub(crate) struct WebRotateProofExecution {
    pub(crate) outcome: WebRotateOutcome,
    pub(crate) proof: WebStructuralProof,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WebRotateOutcome {
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
const WEB_OTHER_STATUS: u32 = WEB_PF | WEB_AF | WEB_ZF | WEB_SF;

fn web_rotate_state(mask: u32, state: FlagState, defined: &mut u32, values: &mut u32, undefined: &mut u32, preserve: &mut u32) {
    match state {
        FlagState::Set(bit) => {
            *defined |= mask;
            if bit != 0 { *values |= mask; }
        }
        FlagState::Undefined => *undefined |= mask,
        FlagState::Preserve => *preserve |= mask,
    }
}

fn runtime_rotate_bit(
    memory: &ProofRuntimeMemory,
    value: Handle,
    zero: Handle,
    one: Handle,
) -> Option<u8> {
    let _ = memory;
    if value == one {
        Some(1)
    } else if value == zero {
        Some(0)
    } else {
        None
    }
}

fn runtime_rotate_word(
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
        value |= u32::from(runtime_rotate_bit(memory, bit, zero, one)?) << index;
    }
    Some(value)
}

fn runtime_rotate_action(
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
        Some(FlagState::Set(runtime_rotate_bit(
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
fn decode_runtime_rotate_effect(
    memory: &ProofRuntimeMemory,
    final_link: Handle,
    caller: Handle,
    result_tag: Handle,
    zero: Handle,
    one: Handle,
    set_tag: Handle,
    undefined_tag: Handle,
    cf_flag: Handle,
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

    let writeback = runtime_rotate_bit(memory, values[0], zero, one)?;
    let value = runtime_rotate_word(memory, values[1], zero, one)?;
    let patch = read_exact_sequence(&memory.store, values[2]).ok()?;
    let (cf, of, patch_len) = if patch.is_empty() {
        (FlagState::Preserve, FlagState::Preserve, 0)
    } else {
        if patch.len() != 2 {
            return None;
        }
        (
            runtime_rotate_action(
                memory, patch[0], set_tag, undefined_tag, cf_flag, zero, one,
            )?,
            runtime_rotate_action(
                memory, patch[1], set_tag, undefined_tag, of_flag, zero, one,
            )?,
            2,
        )
    };

    Some((
        EffectOutcome {
            writeback,
            value,
            cf,
            of,
            patch_len,
            reactions,
        },
        payload,
    ))
}

pub(crate) fn web_prove_rotate32(
    op: u32,
    value: u32,
    count: u32,
) -> Option<WebRotateProofExecution> {
    if count > u8::MAX as u32 {
        return None;
    }

    let mut compiler = FullFixture::new();
    let program = Rotate32Program::install(&mut compiler);
    let (block, function) = match op {
        16 => ("ROL32", program.rol),
        17 => ("ROR32", program.ror),
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
        semantic_source(&compiler.store, "function.rotate.selected", function),
        semantic_source(&compiler.store, "function.rotate.rol", program.rol),
        semantic_source(&compiler.store, "function.rotate.ror", program.ror),
        semantic_source(
            &compiler.store,
            "function.gate.xor2",
            program.gates.xor2,
        ),
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
    let of_flag = loaded_handle(&load, "result.flag.of")?;

    let (mut engine, execute) = execute_to_quiescence(
        &mut memory,
        interpreter,
        initial,
        32,
        6,
    )?;
    if engine.current().len() != 1 {
        return None;
    }

    let final_link = engine.current()[0];
    let (actual, payload) = decode_runtime_rotate_effect(
        &memory,
        final_link,
        caller,
        result_tag,
        zero,
        one,
        set_tag,
        undefined_tag,
        cf_flag,
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
        6,
    )?;
    let visual_links = visual_snapshot(&memory, &load.semantic_roots);

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

    let mut defined_mask = 0u32;
    let mut value_mask = 0u32;
    let mut undefined_mask = 0u32;
    let mut preserve_mask = WEB_OTHER_STATUS;
    web_rotate_state(
        WEB_CF,
        actual.cf,
        &mut defined_mask,
        &mut value_mask,
        &mut undefined_mask,
        &mut preserve_mask,
    );
    web_rotate_state(
        WEB_OF,
        actual.of,
        &mut defined_mask,
        &mut value_mask,
        &mut undefined_mask,
        &mut preserve_mask,
    );

    let outcome = WebRotateOutcome {
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

    Some(WebRotateProofExecution { outcome, proof })
}

pub(crate) fn web_run_rotate32(
    op: u32,
    value: u32,
    count: u32,
) -> Option<WebRotateOutcome> {
    if count > u8::MAX as u32 {
        return None;
    }

    let mut f = FullFixture::new();
    let p = Rotate32Program::install(&mut f);
    let function = match op {
        16 => p.rol,
        17 => p.ror,
        _ => return None,
    };
    let links_after_build = f.store.link_count() as u32;

    let first = run_rotate(&mut f, &p, function, value, count as u8);
    let links_after_first = f.store.link_count() as u32;
    let second = run_rotate(&mut f, &p, function, value, count as u8);
    assert_eq!(second, first, "web ROTATE32 repeat changed result");
    let links_after_second = f.store.link_count() as u32;

    let mut defined_mask = 0u32;
    let mut value_mask = 0u32;
    let mut undefined_mask = 0u32;
    let mut preserve_mask = WEB_OTHER_STATUS;
    web_rotate_state(WEB_CF, first.cf, &mut defined_mask, &mut value_mask, &mut undefined_mask, &mut preserve_mask);
    web_rotate_state(WEB_OF, first.of, &mut defined_mask, &mut value_mask, &mut undefined_mask, &mut preserve_mask);

    Some(WebRotateOutcome {
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
#[ignore = "heavy M4 ROTATE32 suite; mandatory release workflow"]
fn m4_rotate32_count_masking_and_flags_match_80386() {
    let mut f = FullFixture::new();
    let program = Rotate32Program::install(&mut f);

    let sentinels = [0u32, 0x8000_0001, 0xa5a5_5a5a];
    for count in counts() {
        for value in sentinels {
            for function in [program.rol, program.ror] {
                let actual =
                    run_rotate(&mut f, &program, function, value, count);
                assert_eq!(
                    actual,
                    expected(&program, function, value, count),
                    "value={value:#010x} count={count} function={function}"
                );
            }
        }
    }

    for count in [0u8, 1, 2, 31, 33] {
        for value in values() {
            for function in [program.rol, program.ror] {
                let actual =
                    run_rotate(&mut f, &program, function, value, count);
                assert_eq!(
                    actual,
                    expected(&program, function, value, count),
                    "value={value:#010x} count={count} function={function}"
                );
            }
        }
    }

    println!(
        "M4_ROTATE32 rules={} program_links={}",
        2 * 32,
        program.links_after_build,
    );
}

#[test]
#[ignore = "heavy M4 ROTATE32 suite; mandatory release workflow"]
fn m4_rotate32_aliases_preserve_only_unaffected_flags_and_steady() {
    let mut f = FullFixture::new();
    let program = Rotate32Program::install(&mut f);
    let value = 0x9234_5679u32;

    for function in [program.rol, program.ror] {
        let c0 = run_rotate(&mut f, &program, function, value, 0);
        let c32 = run_rotate(&mut f, &program, function, value, 32);
        let c64 = run_rotate(&mut f, &program, function, value, 64);
        assert_eq!(c0, c32);
        assert_eq!(c0, c64);
        assert_eq!(c0.patch_len, 0);

        let c1 = run_rotate(&mut f, &program, function, value, 1);
        let c33 = run_rotate(&mut f, &program, function, value, 33);
        let c65 = run_rotate(&mut f, &program, function, value, 65);
        assert_eq!(c1, c33);
        assert_eq!(c1, c65);
        assert_eq!(c1.patch_len, 2);

        let c31 = run_rotate(&mut f, &program, function, value, 31);
        let c63 = run_rotate(&mut f, &program, function, value, 63);
        let c95 = run_rotate(&mut f, &program, function, value, 95);
        assert_eq!(c31, c63);
        assert_eq!(c31, c95);
        assert_eq!(c31.of, FlagState::Undefined);
    }

    let first =
        run_rotate(&mut f, &program, program.ror, value, 255);
    let links = f.store.link_count();
    let second =
        run_rotate(&mut f, &program, program.ror, value, 255);
    assert_eq!(second, first);
    assert_eq!(
        f.store.link_count(),
        links,
        "repeated identical ROTATE32 materialized new Links"
    );
}
