use super::{
    arithmetic_effect_n::prepare_arithmetic_effect_call,
    flag_patch::{
        install_alu_effect_result_tag, install_wide_alu_effect_result_tag,
        set_flag_action, undefined_flag_action, FlagPatchSchema,
    },
    mul_effect_n::prepare_mul_effect_call,
    full_adder::{
        define_bundle_rule, index_rule_for, Fixture as FullFixture,
    },
    proof_n::{
        execute_to_quiescence, identical_rerun, load_runtime, loaded_handle,
        prepare_stage, semantic_source, theory_admissions, visual_snapshot,
        WebProofResultStage, WebStructuralProof,
    },
};
use amemory_optimized_cpu_probe::{
    structural::{materialize_exact_sequence, read_exact_sequence},
    Handle, OptimizedLinkStore, ROOT_HANDLE,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PatchShape {
    FullSet6,
    Logic6,
    PreserveCf5,
    Empty,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ArchitecturalStateSchema {
    pub(crate) state_tag: Handle,
    pub(crate) apply_state: Handle,
    pub(crate) apply_wide_state: Handle,
    pub(crate) eax: Handle,
    pub(crate) ebx: Handle,
    pub(crate) edx: Handle,
    pub(crate) ecx: Handle,
    pub(crate) esi: Handle,
    pub(crate) edi: Handle,
    pub(crate) ebp: Handle,
    pub(crate) esp: Handle,
    pub(crate) eip: Handle,
    pub(crate) undefined: Handle,
    pub(crate) flags: FlagPatchSchema,
    pub(crate) effect_result_tag: Handle,
    pub(crate) wide_effect_result_tag: Handle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StateValue {
    eax: u32,
    ebx: u32,
    edx: u32,
    ecx: u32,
    esi: u32,
    edi: u32,
    ebp: u32,
    esp: u32,
    eip: u32,
    cf: Option<u8>,
    pf: Option<u8>,
    af: Option<u8>,
    zf: Option<u8>,
    sf: Option<u8>,
    of: Option<u8>,
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
        for pole in [c, o, o, c, c, o, c, o, c, c, o, o, c, o, o, c] {
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
}

fn binding(
    store: &mut OptimizedLinkStore,
    id: Handle,
    value: Handle,
) -> Handle {
    store.ensure_pair(id, value).unwrap()
}

fn state_link(
    store: &mut OptimizedLinkStore,
    schema: ArchitecturalStateSchema,
    eax: Handle,
    ebx: Handle,
    edx: Handle,
    cf: Handle,
    pf: Handle,
    af: Handle,
    zf: Handle,
    sf: Handle,
    of: Handle,
    ecx: Handle,
    esi: Handle,
    edi: Handle,
    ebp: Handle,
    esp: Handle,
    eip: Handle,
) -> Handle {
    // Stable M5 prefix MUST NOT be reordered:
    // [EAX, EBX, EDX, CF, PF, AF, ZF, SF, OF].
    // M5c extends the record append-only.
    let fields = [
        binding(store, schema.eax, eax),
        binding(store, schema.ebx, ebx),
        binding(store, schema.edx, edx),
        binding(store, schema.flags.cf, cf),
        binding(store, schema.flags.pf, pf),
        binding(store, schema.flags.af, af),
        binding(store, schema.flags.zf, zf),
        binding(store, schema.flags.sf, sf),
        binding(store, schema.flags.of, of),
        binding(store, schema.ecx, ecx),
        binding(store, schema.esi, esi),
        binding(store, schema.edi, edi),
        binding(store, schema.ebp, ebp),
        binding(store, schema.esp, esp),
        binding(store, schema.eip, eip),
    ];
    let payload = materialize_exact_sequence(store, &fields).unwrap();
    store.ensure_pair(schema.state_tag, payload).unwrap()
}

fn state_apply_frame(
    store: &mut OptimizedLinkStore,
    schema: ArchitecturalStateSchema,
    state: Handle,
    target: Handle,
) -> Handle {
    let payload =
        materialize_exact_sequence(store, &[state, target]).unwrap();
    let descriptor = store
        .ensure_pair(schema.apply_state, payload)
        .unwrap();
    store.ensure_start_self_closed(descriptor).unwrap()
}

fn state_apply_wide_frame(
    store: &mut OptimizedLinkStore,
    schema: ArchitecturalStateSchema,
    state: Handle,
    low_target: Handle,
    high_target: Handle,
) -> Handle {
    let payload = materialize_exact_sequence(
        store,
        &[state, low_target, high_target],
    )
    .unwrap();
    let descriptor = store
        .ensure_pair(schema.apply_wide_state, payload)
        .unwrap();
    store.ensure_start_self_closed(descriptor).unwrap()
}

fn set_action_template(
    store: &mut OptimizedLinkStore,
    schema: FlagPatchSchema,
    flag: Handle,
    value: Handle,
) -> Handle {
    set_flag_action(store, schema, flag, value)
}

fn effect_template(
    store: &mut OptimizedLinkStore,
    schema: ArchitecturalStateSchema,
    writeback: Handle,
    word: Handle,
    shape: PatchShape,
    cf: Handle,
    pf: Handle,
    af: Handle,
    zf: Handle,
    sf: Handle,
    of: Handle,
    zero: Handle,
) -> Handle {
    let patch = match shape {
        PatchShape::FullSet6 => {
            let actions = [
                set_action_template(store, schema.flags, schema.flags.cf, cf),
                set_action_template(store, schema.flags, schema.flags.pf, pf),
                set_action_template(store, schema.flags, schema.flags.af, af),
                set_action_template(store, schema.flags, schema.flags.zf, zf),
                set_action_template(store, schema.flags, schema.flags.sf, sf),
                set_action_template(store, schema.flags, schema.flags.of, of),
            ];
            materialize_exact_sequence(store, &actions).unwrap()
        }
        PatchShape::Logic6 => {
            let actions = [
                set_action_template(store, schema.flags, schema.flags.cf, zero),
                set_action_template(store, schema.flags, schema.flags.pf, pf),
                undefined_flag_action(store, schema.flags, schema.flags.af),
                set_action_template(store, schema.flags, schema.flags.zf, zf),
                set_action_template(store, schema.flags, schema.flags.sf, sf),
                set_action_template(store, schema.flags, schema.flags.of, zero),
            ];
            materialize_exact_sequence(store, &actions).unwrap()
        }
        PatchShape::PreserveCf5 => {
            let actions = [
                set_action_template(store, schema.flags, schema.flags.pf, pf),
                set_action_template(store, schema.flags, schema.flags.af, af),
                set_action_template(store, schema.flags, schema.flags.zf, zf),
                set_action_template(store, schema.flags, schema.flags.sf, sf),
                set_action_template(store, schema.flags, schema.flags.of, of),
            ];
            materialize_exact_sequence(store, &actions).unwrap()
        }
        PatchShape::Empty => ROOT_HANDLE,
    };

    let payload =
        materialize_exact_sequence(store, &[writeback, word, patch]).unwrap();
    store
        .ensure_pair(schema.effect_result_tag, payload)
        .unwrap()
}

#[allow(clippy::too_many_arguments)]
fn install_case(
    f: &mut FullFixture,
    anchors: &mut AnchorGen,
    schema: ArchitecturalStateSchema,
    target: Handle,
    writeback: Handle,
    shape: PatchShape,
) {
    let old_eax = anchors.next(&mut f.store);
    let old_ebx = anchors.next(&mut f.store);
    let old_edx = anchors.next(&mut f.store);
    let old_ecx = anchors.next(&mut f.store);
    let old_esi = anchors.next(&mut f.store);
    let old_edi = anchors.next(&mut f.store);
    let old_ebp = anchors.next(&mut f.store);
    let old_esp = anchors.next(&mut f.store);
    let old_eip = anchors.next(&mut f.store);
    let old_cf = anchors.next(&mut f.store);
    let old_pf = anchors.next(&mut f.store);
    let old_af = anchors.next(&mut f.store);
    let old_zf = anchors.next(&mut f.store);
    let old_sf = anchors.next(&mut f.store);
    let old_of = anchors.next(&mut f.store);
    let word = anchors.next(&mut f.store);
    let new_cf = anchors.next(&mut f.store);
    let new_pf = anchors.next(&mut f.store);
    let new_af = anchors.next(&mut f.store);
    let new_zf = anchors.next(&mut f.store);
    let new_sf = anchors.next(&mut f.store);
    let new_of = anchors.next(&mut f.store);

    let before_state = state_link(
        &mut f.store,
        schema,
        old_eax,
        old_ebx,
        old_edx,
        old_cf,
        old_pf,
        old_af,
        old_zf,
        old_sf,
        old_of,
        old_ecx,
        old_esi,
        old_edi,
        old_ebp,
        old_esp,
        old_eip,
    );
    let before_effect = effect_template(
        &mut f.store,
        schema,
        writeback,
        word,
        shape,
        new_cf,
        new_pf,
        new_af,
        new_zf,
        new_sf,
        new_of,
        f.zero,
    );
    // The state/target continuation frame is the caller propagated through
    // the existing ALU pipeline. A completed ALU effect therefore arrives as
    // exactly the same shape whether it was produced by a real ALU execution
    // or supplied directly by an isolated applier witness.
    let before_frame =
        state_apply_frame(&mut f.store, schema, before_state, target);
    let before =
        f.store.ensure_pair(before_frame, before_effect).unwrap();

    let write = writeback == f.one;
    let next_eax = if write && target == schema.eax {
        word
    } else {
        old_eax
    };
    let next_ebx = if write && target == schema.ebx {
        word
    } else {
        old_ebx
    };
    let next_edx = if write && target == schema.edx {
        word
    } else {
        old_edx
    };
    let next_ecx = if write && target == schema.ecx {
        word
    } else {
        old_ecx
    };
    let next_esi = if write && target == schema.esi {
        word
    } else {
        old_esi
    };
    let next_edi = if write && target == schema.edi {
        word
    } else {
        old_edi
    };
    let next_ebp = if write && target == schema.ebp {
        word
    } else {
        old_ebp
    };
    let next_esp = if write && target == schema.esp {
        word
    } else {
        old_esp
    };
    let (next_cf, next_pf, next_af, next_zf, next_sf, next_of) =
        match shape {
            PatchShape::FullSet6 => {
                (new_cf, new_pf, new_af, new_zf, new_sf, new_of)
            }
            PatchShape::Logic6 => {
                (f.zero, new_pf, schema.undefined, new_zf, new_sf, f.zero)
            }
            PatchShape::PreserveCf5 => {
                (old_cf, new_pf, new_af, new_zf, new_sf, new_of)
            }
            PatchShape::Empty => {
                (old_cf, old_pf, old_af, old_zf, old_sf, old_of)
            }
        };

    let after_state = state_link(
        &mut f.store,
        schema,
        next_eax,
        next_ebx,
        next_edx,
        next_cf,
        next_pf,
        next_af,
        next_zf,
        next_sf,
        next_of,
        next_ecx,
        next_esi,
        next_edi,
        next_ebp,
        next_esp,
        old_eip,
    );
    let after = f.store.ensure_pair(f.k, after_state).unwrap();

    let mut roles = vec![
        old_eax, old_ebx, old_edx, old_ecx, old_esi, old_edi,
        old_ebp, old_esp, old_eip, old_cf, old_pf, old_af, old_zf,
        old_sf, old_of, word,
    ];
    match shape {
        PatchShape::FullSet6 => {
            roles.extend_from_slice(&[
                new_cf, new_pf, new_af, new_zf, new_sf, new_of,
            ]);
        }
        PatchShape::Logic6 => {
            roles.extend_from_slice(&[new_pf, new_zf, new_sf]);
        }
        PatchShape::PreserveCf5 => {
            roles.extend_from_slice(&[
                new_pf, new_af, new_zf, new_sf, new_of,
            ]);
        }
        PatchShape::Empty => {}
    }

    let (_, admission) = define_bundle_rule(
        &mut f.store,
        f.theory,
        &roles,
        before,
        &[after],
    );
    index_rule_for(
        &mut f.store,
        &[schema.effect_result_tag],
        admission,
    );
}

fn install_wide_mul_case(
    f: &mut FullFixture,
    anchors: &mut AnchorGen,
    schema: ArchitecturalStateSchema,
    new_cf: Handle,
    new_of: Handle,
) {
    let old_eax = anchors.next(&mut f.store);
    let old_ebx = anchors.next(&mut f.store);
    let old_edx = anchors.next(&mut f.store);
    let old_ecx = anchors.next(&mut f.store);
    let old_esi = anchors.next(&mut f.store);
    let old_edi = anchors.next(&mut f.store);
    let old_ebp = anchors.next(&mut f.store);
    let old_esp = anchors.next(&mut f.store);
    let old_eip = anchors.next(&mut f.store);
    let old_cf = anchors.next(&mut f.store);
    let old_pf = anchors.next(&mut f.store);
    let old_af = anchors.next(&mut f.store);
    let old_zf = anchors.next(&mut f.store);
    let old_sf = anchors.next(&mut f.store);
    let old_of = anchors.next(&mut f.store);
    let lo = anchors.next(&mut f.store);
    let hi = anchors.next(&mut f.store);

    let before_state = state_link(
        &mut f.store,
        schema,
        old_eax,
        old_ebx,
        old_edx,
        old_cf,
        old_pf,
        old_af,
        old_zf,
        old_sf,
        old_of,
        old_ecx,
        old_esi,
        old_edi,
        old_ebp,
        old_esp,
        old_eip,
    );
    let wide =
        materialize_exact_sequence(&mut f.store, &[lo, hi]).unwrap();

    let set_cf =
        set_flag_action(&mut f.store, schema.flags, schema.flags.cf, new_cf);
    let undef_pf =
        undefined_flag_action(&mut f.store, schema.flags, schema.flags.pf);
    let undef_af =
        undefined_flag_action(&mut f.store, schema.flags, schema.flags.af);
    let undef_zf =
        undefined_flag_action(&mut f.store, schema.flags, schema.flags.zf);
    let undef_sf =
        undefined_flag_action(&mut f.store, schema.flags, schema.flags.sf);
    let set_of =
        set_flag_action(&mut f.store, schema.flags, schema.flags.of, new_of);
    let patch = materialize_exact_sequence(
        &mut f.store,
        &[set_cf, undef_pf, undef_af, undef_zf, undef_sf, set_of],
    )
    .unwrap();
    let payload =
        materialize_exact_sequence(&mut f.store, &[wide, patch]).unwrap();
    let effect = f
        .store
        .ensure_pair(schema.wide_effect_result_tag, payload)
        .unwrap();

    let frame = state_apply_wide_frame(
        &mut f.store,
        schema,
        before_state,
        schema.eax,
        schema.edx,
    );
    let before = f.store.ensure_pair(frame, effect).unwrap();

    let after_state = state_link(
        &mut f.store,
        schema,
        lo,
        old_ebx,
        hi,
        new_cf,
        schema.undefined,
        schema.undefined,
        schema.undefined,
        schema.undefined,
        new_of,
        old_ecx,
        old_esi,
        old_edi,
        old_ebp,
        old_esp,
        old_eip,
    );
    let after = f.store.ensure_pair(f.k, after_state).unwrap();

    let roles = [
        old_eax, old_ebx, old_edx, old_ecx, old_esi, old_edi,
        old_ebp, old_esp, old_eip, old_cf, old_pf, old_af, old_zf,
        old_sf, old_of, lo, hi,
    ];
    let (_, admission) = define_bundle_rule(
        &mut f.store,
        f.theory,
        &roles,
        before,
        &[after],
    );
    index_rule_for(
        &mut f.store,
        &[schema.wide_effect_result_tag],
        admission,
    );
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ArchitecturalStateProgram {
    pub(crate) schema: ArchitecturalStateSchema,
}

impl ArchitecturalStateProgram {
    pub(crate) fn install(f: &mut FullFixture) -> Self {
        let shared = f.store.ensure_pair(f.k, f.full).unwrap();
        let flags =
            FlagPatchSchema::install(&mut f.store, shared, f.o, f.c);
        let effect_result_tag =
            install_alu_effect_result_tag(&mut f.store, shared, f.o, f.c);
        let wide_effect_result_tag =
            install_wide_alu_effect_result_tag(&mut f.store, shared, f.o, f.c);

        let result_tags = f
            .store
            .ensure_pair(effect_result_tag, wide_effect_result_tag)
            .unwrap();
        let seed = f
            .store
            .ensure_pair(f.interpreter, result_tags)
            .unwrap();
        let mut anchors = AnchorGen::new(&mut f.store, seed, f.o, f.c);
        let state_tag = anchors.next(&mut f.store);
        let apply_state = anchors.next(&mut f.store);
        let apply_wide_state = anchors.next(&mut f.store);
        let eax = anchors.next(&mut f.store);
        let ebx = anchors.next(&mut f.store);
        let edx = anchors.next(&mut f.store);
        let ecx = anchors.next(&mut f.store);
        let esi = anchors.next(&mut f.store);
        let edi = anchors.next(&mut f.store);
        let ebp = anchors.next(&mut f.store);
        let esp = anchors.next(&mut f.store);
        let eip = anchors.next(&mut f.store);
        let undefined = anchors.next(&mut f.store);

        let schema = ArchitecturalStateSchema {
            state_tag,
            apply_state,
            apply_wide_state,
            eax,
            ebx,
            edx,
            ecx,
            esi,
            edi,
            ebp,
            esp,
            eip,
            undefined,
            flags,
            effect_result_tag,
            wide_effect_result_tag,
        };

        for target in [
            schema.eax,
            schema.ebx,
            schema.ecx,
            schema.edx,
            schema.esi,
            schema.edi,
            schema.ebp,
            schema.esp,
        ] {
            for writeback in [f.zero, f.one] {
                for shape in [
                    PatchShape::FullSet6,
                    PatchShape::Logic6,
                    PatchShape::PreserveCf5,
                    PatchShape::Empty,
                ] {
                    install_case(
                        f,
                        &mut anchors,
                        schema,
                        target,
                        writeback,
                        shape,
                    );
                }
            }
        }
        for new_cf in [f.zero, f.one] {
            for new_of in [f.zero, f.one] {
                install_wide_mul_case(
                    f,
                    &mut anchors,
                    schema,
                    new_cf,
                    new_of,
                );
            }
        }

        Self { schema }
    }
}

fn bit_handles(
    f: &FullFixture,
    value: u32,
) -> Vec<Handle> {
    (0..32)
        .map(|bit| {
            if (value >> bit) & 1 == 1 {
                f.one
            } else {
                f.zero
            }
        })
        .collect()
}

fn word(
    f: &mut FullFixture,
    value: u32,
) -> Handle {
    let bits = bit_handles(f, value);
    materialize_exact_sequence(&mut f.store, &bits).unwrap()
}

fn decode_word(
    f: &FullFixture,
    value: Handle,
) -> Option<u32> {
    let bits = read_exact_sequence(&f.store, value).ok()?;
    if bits.len() != 32 {
        return None;
    }
    let mut out = 0u32;
    for (index, bit) in bits.into_iter().enumerate() {
        if bit == f.one {
            out |= 1u32 << index;
        } else if bit != f.zero {
            return None;
        }
    }
    Some(out)
}

fn decode_flag(
    f: &FullFixture,
    schema: ArchitecturalStateSchema,
    value: Handle,
) -> Option<Option<u8>> {
    if value == f.zero {
        Some(Some(0))
    } else if value == f.one {
        Some(Some(1))
    } else if value == schema.undefined {
        Some(None)
    } else {
        None
    }
}

fn decode_binding(
    store: &OptimizedLinkStore,
    binding: Handle,
    expected_id: Handle,
) -> Option<Handle> {
    let (id, value) = store.poles(binding).ok()?;
    if id != expected_id {
        return None;
    }
    Some(value)
}

fn decode_state(
    f: &FullFixture,
    schema: ArchitecturalStateSchema,
    state: Handle,
) -> Option<StateValue> {
    let (tag, payload) = f.store.poles(state).ok()?;
    if tag != schema.state_tag {
        return None;
    }
    let fields = read_exact_sequence(&f.store, payload).ok()?;
    if fields.len() != 15 {
        return None;
    }

    let eax = decode_word(
        f,
        decode_binding(&f.store, fields[0], schema.eax)?,
    )?;
    let ebx = decode_word(
        f,
        decode_binding(&f.store, fields[1], schema.ebx)?,
    )?;
    let edx = decode_word(
        f,
        decode_binding(&f.store, fields[2], schema.edx)?,
    )?;
    let cf = decode_flag(
        f,
        schema,
        decode_binding(&f.store, fields[3], schema.flags.cf)?,
    )?;
    let pf = decode_flag(
        f,
        schema,
        decode_binding(&f.store, fields[4], schema.flags.pf)?,
    )?;
    let af = decode_flag(
        f,
        schema,
        decode_binding(&f.store, fields[5], schema.flags.af)?,
    )?;
    let zf = decode_flag(
        f,
        schema,
        decode_binding(&f.store, fields[6], schema.flags.zf)?,
    )?;
    let sf = decode_flag(
        f,
        schema,
        decode_binding(&f.store, fields[7], schema.flags.sf)?,
    )?;
    let of = decode_flag(
        f,
        schema,
        decode_binding(&f.store, fields[8], schema.flags.of)?,
    )?;
    let ecx = decode_word(
        f,
        decode_binding(&f.store, fields[9], schema.ecx)?,
    )?;
    let esi = decode_word(
        f,
        decode_binding(&f.store, fields[10], schema.esi)?,
    )?;
    let edi = decode_word(
        f,
        decode_binding(&f.store, fields[11], schema.edi)?,
    )?;
    let ebp = decode_word(
        f,
        decode_binding(&f.store, fields[12], schema.ebp)?,
    )?;
    let esp = decode_word(
        f,
        decode_binding(&f.store, fields[13], schema.esp)?,
    )?;
    let eip = decode_word(
        f,
        decode_binding(&f.store, fields[14], schema.eip)?,
    )?;

    Some(StateValue {
        eax,
        ebx,
        edx,
        ecx,
        esi,
        edi,
        ebp,
        esp,
        eip,
        cf,
        pf,
        af,
        zf,
        sf,
        of,
    })
}


fn word_in_store(
    store: &mut OptimizedLinkStore,
    zero: Handle,
    one: Handle,
    value: u32,
) -> Option<Handle> {
    let bits = (0..32)
        .map(|bit| if (value >> bit) & 1 == 1 { one } else { zero })
        .collect::<Vec<_>>();
    materialize_exact_sequence(store, &bits).ok()
}

fn decode_word_in_store(
    store: &OptimizedLinkStore,
    zero: Handle,
    one: Handle,
    value: Handle,
) -> Option<u32> {
    let bits = read_exact_sequence(store, value).ok()?;
    if bits.len() != 32 {
        return None;
    }
    let mut out = 0u32;
    for (index, bit) in bits.into_iter().enumerate() {
        if bit == one {
            out |= 1u32 << index;
        } else if bit != zero {
            return None;
        }
    }
    Some(out)
}

fn decode_flag_in_store(
    value: Handle,
    zero: Handle,
    one: Handle,
    undefined: Handle,
) -> Option<Option<u8>> {
    if value == zero {
        Some(Some(0))
    } else if value == one {
        Some(Some(1))
    } else if value == undefined {
        Some(None)
    } else {
        None
    }
}

fn decode_state_in_store(
    store: &OptimizedLinkStore,
    schema: ArchitecturalStateSchema,
    state: Handle,
    zero: Handle,
    one: Handle,
) -> Option<StateValue> {
    let (tag, payload) = store.poles(state).ok()?;
    if tag != schema.state_tag {
        return None;
    }
    let fields = read_exact_sequence(store, payload).ok()?;
    if fields.len() != 15 {
        return None;
    }

    let values = fields
        .into_iter()
        .zip([
            schema.eax,
            schema.ebx,
            schema.edx,
            schema.flags.cf,
            schema.flags.pf,
            schema.flags.af,
            schema.flags.zf,
            schema.flags.sf,
            schema.flags.of,
            schema.ecx,
            schema.esi,
            schema.edi,
            schema.ebp,
            schema.esp,
            schema.eip,
        ])
        .map(|(field, expected)| decode_binding(store, field, expected))
        .collect::<Option<Vec<_>>>()?;

    Some(StateValue {
        eax: decode_word_in_store(store, zero, one, values[0])?,
        ebx: decode_word_in_store(store, zero, one, values[1])?,
        edx: decode_word_in_store(store, zero, one, values[2])?,
        cf: decode_flag_in_store(values[3], zero, one, schema.undefined)?,
        pf: decode_flag_in_store(values[4], zero, one, schema.undefined)?,
        af: decode_flag_in_store(values[5], zero, one, schema.undefined)?,
        zf: decode_flag_in_store(values[6], zero, one, schema.undefined)?,
        sf: decode_flag_in_store(values[7], zero, one, schema.undefined)?,
        of: decode_flag_in_store(values[8], zero, one, schema.undefined)?,
        ecx: decode_word_in_store(store, zero, one, values[9])?,
        esi: decode_word_in_store(store, zero, one, values[10])?,
        edi: decode_word_in_store(store, zero, one, values[11])?,
        ebp: decode_word_in_store(store, zero, one, values[12])?,
        esp: decode_word_in_store(store, zero, one, values[13])?,
        eip: decode_word_in_store(store, zero, one, values[14])?,
    })
}

fn state_from_value(
    f: &mut FullFixture,
    schema: ArchitecturalStateSchema,
    value: StateValue,
) -> Option<Handle> {
    let eax = word_in_store(&mut f.store, f.zero, f.one, value.eax)?;
    let ebx = word_in_store(&mut f.store, f.zero, f.one, value.ebx)?;
    let edx = word_in_store(&mut f.store, f.zero, f.one, value.edx)?;
    let ecx = word_in_store(&mut f.store, f.zero, f.one, value.ecx)?;
    let esi = word_in_store(&mut f.store, f.zero, f.one, value.esi)?;
    let edi = word_in_store(&mut f.store, f.zero, f.one, value.edi)?;
    let ebp = word_in_store(&mut f.store, f.zero, f.one, value.ebp)?;
    let esp = word_in_store(&mut f.store, f.zero, f.one, value.esp)?;
    let eip = word_in_store(&mut f.store, f.zero, f.one, value.eip)?;
    let flag = |value: Option<u8>| -> Option<Handle> {
        match value {
            Some(0) => Some(f.zero),
            Some(1) => Some(f.one),
            None => Some(schema.undefined),
            _ => None,
        }
    };
    Some(state_link(
        &mut f.store,
        schema,
        eax,
        ebx,
        edx,
        flag(value.cf)?,
        flag(value.pf)?,
        flag(value.af)?,
        flag(value.zf)?,
        flag(value.sf)?,
        flag(value.of)?,
        ecx,
        esi,
        edi,
        ebp,
        esp,
        eip,
    ))
}

fn flags_to_masks(value: StateValue) -> (u32, u32, u32) {
    let mut defined = 0u32;
    let mut values = 0u32;
    let mut undefined = 0u32;
    for (mask, flag) in [
        (1u32 << 0, value.cf),
        (1u32 << 2, value.pf),
        (1u32 << 4, value.af),
        (1u32 << 6, value.zf),
        (1u32 << 7, value.sf),
        (1u32 << 11, value.of),
    ] {
        match flag {
            Some(bit) => {
                defined |= mask;
                if bit == 1 {
                    values |= mask;
                }
            }
            None => undefined |= mask,
        }
    }
    (defined, values, undefined)
}

#[derive(Clone, Debug)]
pub(crate) struct WebArchitecturalStateExecution {
    pub(crate) outcome: WebArchitecturalStateOutcome,
    pub(crate) proof: WebStructuralProof,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WebArchitecturalStateOutcome {
    pub(crate) eax_before: u32,
    pub(crate) ebx_before: u32,
    pub(crate) edx_before: u32,
    pub(crate) ecx_before: u32,
    pub(crate) esi_before: u32,
    pub(crate) edi_before: u32,
    pub(crate) ebp_before: u32,
    pub(crate) esp_before: u32,
    pub(crate) eip_before: u32,
    pub(crate) eax_after: u32,
    pub(crate) ebx_after: u32,
    pub(crate) edx_after: u32,
    pub(crate) ecx_after: u32,
    pub(crate) esi_after: u32,
    pub(crate) edi_after: u32,
    pub(crate) ebp_after: u32,
    pub(crate) esp_after: u32,
    pub(crate) eip_after: u32,
    pub(crate) flags_defined_mask: u32,
    pub(crate) flags_value_mask: u32,
    pub(crate) flags_undefined_mask: u32,
    pub(crate) reactions: u32,
    pub(crate) old_state_retained: u8,
    pub(crate) atomic_scope: u8,
    pub(crate) steady_link_delta: u32,
    pub(crate) quiescent: u8,
}

/// Real one-memory M5a integration witness:
///
/// State(EAX=0xffff_ffff, EBX=0x1122_3344)
///   -> actual structural ADD32 effect (+1)
///   -> atomic successor State(EAX=0, EBX preserved, x86 flags)
///
/// The host chooses fixture inputs and independently decodes/checks the final
/// state. It never constructs or injects the successor state.
pub(crate) fn web_prove_architectural_state_add(
) -> Option<WebArchitecturalStateExecution> {
    let mut compiler = FullFixture::new();

    let alu = prepare_arithmetic_effect_call(
        &mut compiler,
        0xffff_ffff,
        1,
        0,
        0,
        1,
    )?;
    let program = ArchitecturalStateProgram::install(&mut compiler);
    if alu.result_tag != program.schema.effect_result_tag {
        return None;
    }

    let before_value = StateValue {
        eax: 0xffff_ffff,
        ebx: 0x1122_3344,
        edx: 0x5566_7788,
        ecx: 0x0102_0304,
        esi: 0x1111_2222,
        edi: 0x3333_4444,
        ebp: 0x5555_6666,
        esp: 0x7777_8888,
        eip: 0x0040_1000,
        cf: Some(0),
        pf: Some(0),
        af: Some(0),
        zf: Some(0),
        sf: Some(1),
        of: Some(1),
    };
    let expected = StateValue {
        eax: 0,
        ebx: before_value.ebx,
        edx: before_value.edx,
        ecx: before_value.ecx,
        esi: before_value.esi,
        edi: before_value.edi,
        ebp: before_value.ebp,
        esp: before_value.esp,
        eip: before_value.eip,
        cf: Some(1),
        pf: Some(1),
        af: Some(1),
        zf: Some(1),
        sf: Some(0),
        of: Some(0),
    };

    let before_state =
        state_from_value(&mut compiler, program.schema, before_value)?;
    let frame = state_apply_frame(
        &mut compiler.store,
        program.schema,
        before_state,
        program.schema.eax,
    );
    // The real arithmetic program sees the continuation frame as its caller.
    // Its final generic rule therefore produces frame -> ALU_EFFECT_RESULT,
    // which is the exact trigger consumed by the state applier.
    let initial = compiler
        .store
        .ensure_pair(frame, alu.invocation)
        .ok()?;

    let prepared_roots = vec![
        semantic_source(
            &compiler.store,
            "function.effect.arithmetic",
            alu.function,
        ),
        semantic_source(
            &compiler.store,
            "state.schema.tag",
            program.schema.state_tag,
        ),
        semantic_source(
            &compiler.store,
            "state.apply.frame_tag",
            program.schema.apply_state,
        ),
        semantic_source(
            &compiler.store,
            "state.register.eax",
            program.schema.eax,
        ),
        semantic_source(
            &compiler.store,
            "state.register.ebx",
            program.schema.ebx,
        ),
        semantic_source(
            &compiler.store,
            "state.register.edx",
            program.schema.edx,
        ),
        semantic_source(
            &compiler.store,
            "state.register.ecx",
            program.schema.ecx,
        ),
        semantic_source(
            &compiler.store,
            "state.register.esi",
            program.schema.esi,
        ),
        semantic_source(
            &compiler.store,
            "state.register.edi",
            program.schema.edi,
        ),
        semantic_source(
            &compiler.store,
            "state.register.ebp",
            program.schema.ebp,
        ),
        semantic_source(
            &compiler.store,
            "state.register.esp",
            program.schema.esp,
        ),
        semantic_source(
            &compiler.store,
            "state.eip",
            program.schema.eip,
        ),
        semantic_source(
            &compiler.store,
            "state.flag.undefined",
            program.schema.undefined,
        ),
        semantic_source(
            &compiler.store,
            "state.before",
            before_state,
        ),
        semantic_source(
            &compiler.store,
            "state.target",
            program.schema.eax,
        ),
        semantic_source(
            &compiler.store,
            "state.continuation",
            frame,
        ),
        semantic_source(
            &compiler.store,
            "result.alu_effect_tag",
            program.schema.effect_result_tag,
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
            "scope.initial",
            initial,
        ),
        semantic_source(
            &compiler.store,
            "context.result",
            compiler.k,
        ),
    ];

    let admissions =
        theory_admissions(&compiler.store, compiler.theory)?;
    let prepare =
        prepare_stage(&compiler.store, prepared_roots, admissions);
    let (mut memory, load) = load_runtime(&prepare)?;

    let interpreter =
        loaded_handle(&load, "execution.interpreter")?;
    let initial = loaded_handle(&load, "scope.initial")?;
    let before_state =
        loaded_handle(&load, "state.before")?;
    let state_tag =
        loaded_handle(&load, "state.schema.tag")?;
    let apply_state =
        loaded_handle(&load, "state.apply.frame_tag")?;
    let eax = loaded_handle(&load, "state.register.eax")?;
    let ebx = loaded_handle(&load, "state.register.ebx")?;
    let edx = loaded_handle(&load, "state.register.edx")?;
    let undefined =
        loaded_handle(&load, "state.flag.undefined")?;
    let effect_result_tag =
        loaded_handle(&load, "result.alu_effect_tag")?;
    let zero = loaded_handle(&load, "data.bit.zero")?;
    let one = loaded_handle(&load, "data.bit.one")?;
    let result_context =
        loaded_handle(&load, "context.result")?;

    // Flag ids are canonical components of the same shared schema; locate
    // them by the loaded carrier refs from the compiler schema.
    let loaded = |handle: Handle| -> Option<Handle> {
        (handle >= 1 && handle <= load.links_after_load)
            .then_some(handle)
    };
    let wide_effect_result_tag =
        loaded(program.schema.wide_effect_result_tag)?;
    let runtime_schema = ArchitecturalStateSchema {
        state_tag,
        apply_state,
        apply_wide_state: loaded(program.schema.apply_wide_state)?,
        eax,
        ebx,
        edx,
        ecx: loaded(program.schema.ecx)?,
        esi: loaded(program.schema.esi)?,
        edi: loaded(program.schema.edi)?,
        ebp: loaded(program.schema.ebp)?,
        esp: loaded(program.schema.esp)?,
        eip: loaded(program.schema.eip)?,
        undefined,
        flags: FlagPatchSchema {
            set_tag: loaded(program.schema.flags.set_tag)?,
            undefined_tag: loaded(program.schema.flags.undefined_tag)?,
            cf: loaded(program.schema.flags.cf)?,
            pf: loaded(program.schema.flags.pf)?,
            af: loaded(program.schema.flags.af)?,
            zf: loaded(program.schema.flags.zf)?,
            sf: loaded(program.schema.flags.sf)?,
            of: loaded(program.schema.flags.of)?,
        },
        effect_result_tag,
        wide_effect_result_tag,
    };

    let max_steps = alu.active_steps as u32 + 3;
    let (mut engine, execute) = execute_to_quiescence(
        &mut memory,
        interpreter,
        initial,
        32,
        max_steps,
    )?;
    if execute.active_reaction_count != alu.active_steps as u32 + 1
        || engine.current().len() != 1
    {
        return None;
    }

    // Atomicity witness: every reaction keeps a one-member Scope. No
    // architectural state is published during the ALU pipeline; the final
    // active reaction publishes the complete successor in one handoff.
    let atomic_scope = execute.reactions.iter().all(|step| {
        step.scope_before.len() == 1 && step.scope_after.len() == 1
    });
    if !atomic_scope {
        return None;
    }

    let final_link = engine.current()[0];
    let (caller, successor) = memory.store.poles(final_link).ok()?;
    if caller != result_context {
        return None;
    }
    let actual = decode_state_in_store(
        &memory.store,
        runtime_schema,
        successor,
        zero,
        one,
    )?;
    if actual != expected || !memory.store.is_valid(before_state) {
        return None;
    }

    let (state_tag_check, result_sequence) =
        memory.store.poles(successor).ok()?;
    if state_tag_check != runtime_schema.state_tag {
        return None;
    }
    // The arbitrary final Link is recursive Link wire. The state payload is
    // a real ExactSequence and therefore legitimately supplies
    // resultSequenceAnum.
    let result_recursive_wire =
        memory.store.export_anum(final_link).ok()?;
    let result_sequence_anum =
        memory.store.export_anum(result_sequence).ok()?;
    let identical_rerun_link_delta = identical_rerun(
        &mut memory,
        &mut engine,
        initial,
        &result_recursive_wire,
        max_steps,
    )?;
    let visual_links = visual_snapshot(&memory, &load.semantic_roots);

    let result = WebProofResultStage {
        memory_instance_id: memory.id.clone(),
        result_anum: result_recursive_wire,
        result_sequence_anum,
        decoded_value: actual.eax,
        decoded_value_hi: Some(actual.ebx),
        oracle_value: expected.eax,
        oracle_value_hi: Some(expected.ebx),
        oracle_matches: actual == expected,
        links_final: memory.store.link_count() as u32,
        identical_rerun_link_delta,
        visual_links,
    };
    let proof = WebStructuralProof {
        schema_version: 4,
        block: "M5A_STATE_ADD32".to_owned(),
        prepare,
        load,
        execute,
        result,
    };

    let (defined, values, undefined_mask) = flags_to_masks(actual);
    Some(WebArchitecturalStateExecution {
        outcome: WebArchitecturalStateOutcome {
            eax_before: before_value.eax,
            ebx_before: before_value.ebx,
            edx_before: before_value.edx,
            ecx_before: before_value.ecx,
            esi_before: before_value.esi,
            edi_before: before_value.edi,
            ebp_before: before_value.ebp,
            esp_before: before_value.esp,
            eip_before: before_value.eip,
            eax_after: actual.eax,
            ebx_after: actual.ebx,
            edx_after: actual.edx,
            ecx_after: actual.ecx,
            esi_after: actual.esi,
            edi_after: actual.edi,
            ebp_after: actual.ebp,
            esp_after: actual.esp,
            eip_after: actual.eip,
            flags_defined_mask: defined,
            flags_value_mask: values,
            flags_undefined_mask: undefined_mask,
            reactions: proof.execute.active_reaction_count,
            old_state_retained: 1,
            atomic_scope: 1,
            steady_link_delta: proof.result.identical_rerun_link_delta,
            quiescent: u8::from(proof.execute.final_quiescent),
        },
        proof,
    })
}


/// Real one-memory M5b integration witness:
///
/// State(EAX=0xffff_ffff, EDX=old, EBX=preserved)
///   -> actual structural MUL effect (*2)
///   -> atomic successor State(EAX=0xffff_fffe, EDX=1, EBX preserved)
///
/// Operand fetch is intentionally outside M5b; this slice proves that the
/// existing wide ALU effect composes into one indivisible architectural-state
/// publication without host construction of the successor.
pub(crate) fn web_prove_architectural_state_mul(
) -> Option<WebArchitecturalStateExecution> {
    let mut compiler = FullFixture::new();

    let alu = prepare_mul_effect_call(
        &mut compiler,
        0xffff_ffff,
        2,
    )?;
    let program = ArchitecturalStateProgram::install(&mut compiler);
    if alu.result_tag != program.schema.wide_effect_result_tag {
        return None;
    }

    let before_value = StateValue {
        eax: 0xffff_ffff,
        ebx: 0x1122_3344,
        edx: 0xa5a5_5a5a,
        ecx: 0x0102_0304,
        esi: 0x1111_2222,
        edi: 0x3333_4444,
        ebp: 0x5555_6666,
        esp: 0x7777_8888,
        eip: 0x0040_1000,
        cf: Some(0),
        pf: Some(0),
        af: Some(1),
        zf: Some(1),
        sf: Some(1),
        of: Some(0),
    };
    let expected = StateValue {
        eax: 0xffff_fffe,
        ebx: before_value.ebx,
        edx: 0x0000_0001,
        ecx: before_value.ecx,
        esi: before_value.esi,
        edi: before_value.edi,
        ebp: before_value.ebp,
        esp: before_value.esp,
        eip: before_value.eip,
        cf: Some(1),
        pf: None,
        af: None,
        zf: None,
        sf: None,
        of: Some(1),
    };

    let before_state =
        state_from_value(&mut compiler, program.schema, before_value)?;
    let frame = state_apply_wide_frame(
        &mut compiler.store,
        program.schema,
        before_state,
        program.schema.eax,
        program.schema.edx,
    );
    let initial = compiler
        .store
        .ensure_pair(frame, alu.invocation)
        .ok()?;

    let prepared_roots = vec![
        semantic_source(
            &compiler.store,
            "function.effect.mul",
            alu.function,
        ),
        semantic_source(
            &compiler.store,
            "state.schema.tag",
            program.schema.state_tag,
        ),
        semantic_source(
            &compiler.store,
            "state.apply.wide_frame_tag",
            program.schema.apply_wide_state,
        ),
        semantic_source(
            &compiler.store,
            "state.register.eax",
            program.schema.eax,
        ),
        semantic_source(
            &compiler.store,
            "state.register.ebx",
            program.schema.ebx,
        ),
        semantic_source(
            &compiler.store,
            "state.register.edx",
            program.schema.edx,
        ),
        semantic_source(
            &compiler.store,
            "state.register.ecx",
            program.schema.ecx,
        ),
        semantic_source(
            &compiler.store,
            "state.register.esi",
            program.schema.esi,
        ),
        semantic_source(
            &compiler.store,
            "state.register.edi",
            program.schema.edi,
        ),
        semantic_source(
            &compiler.store,
            "state.register.ebp",
            program.schema.ebp,
        ),
        semantic_source(
            &compiler.store,
            "state.register.esp",
            program.schema.esp,
        ),
        semantic_source(
            &compiler.store,
            "state.eip",
            program.schema.eip,
        ),
        semantic_source(
            &compiler.store,
            "state.flag.undefined",
            program.schema.undefined,
        ),
        semantic_source(
            &compiler.store,
            "state.before",
            before_state,
        ),
        semantic_source(
            &compiler.store,
            "state.low_target",
            program.schema.eax,
        ),
        semantic_source(
            &compiler.store,
            "state.high_target",
            program.schema.edx,
        ),
        semantic_source(
            &compiler.store,
            "state.continuation",
            frame,
        ),
        semantic_source(
            &compiler.store,
            "result.wide_alu_effect_tag",
            program.schema.wide_effect_result_tag,
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
            "scope.initial",
            initial,
        ),
        semantic_source(
            &compiler.store,
            "context.result",
            compiler.k,
        ),
    ];

    let admissions =
        theory_admissions(&compiler.store, compiler.theory)?;
    let prepare =
        prepare_stage(&compiler.store, prepared_roots, admissions);
    let (mut memory, load) = load_runtime(&prepare)?;

    let interpreter =
        loaded_handle(&load, "execution.interpreter")?;
    let initial = loaded_handle(&load, "scope.initial")?;
    let before_state =
        loaded_handle(&load, "state.before")?;
    let result_context =
        loaded_handle(&load, "context.result")?;
    let zero = loaded_handle(&load, "data.bit.zero")?;
    let one = loaded_handle(&load, "data.bit.one")?;

    let loaded = |handle: Handle| -> Option<Handle> {
        (handle >= 1 && handle <= load.links_after_load)
            .then_some(handle)
    };
    let runtime_schema = ArchitecturalStateSchema {
        state_tag: loaded(program.schema.state_tag)?,
        apply_state: loaded(program.schema.apply_state)?,
        apply_wide_state: loaded(program.schema.apply_wide_state)?,
        eax: loaded(program.schema.eax)?,
        ebx: loaded(program.schema.ebx)?,
        edx: loaded(program.schema.edx)?,
        ecx: loaded(program.schema.ecx)?,
        esi: loaded(program.schema.esi)?,
        edi: loaded(program.schema.edi)?,
        ebp: loaded(program.schema.ebp)?,
        esp: loaded(program.schema.esp)?,
        eip: loaded(program.schema.eip)?,
        undefined: loaded(program.schema.undefined)?,
        flags: FlagPatchSchema {
            set_tag: loaded(program.schema.flags.set_tag)?,
            undefined_tag: loaded(program.schema.flags.undefined_tag)?,
            cf: loaded(program.schema.flags.cf)?,
            pf: loaded(program.schema.flags.pf)?,
            af: loaded(program.schema.flags.af)?,
            zf: loaded(program.schema.flags.zf)?,
            sf: loaded(program.schema.flags.sf)?,
            of: loaded(program.schema.flags.of)?,
        },
        effect_result_tag: loaded(program.schema.effect_result_tag)?,
        wide_effect_result_tag:
            loaded(program.schema.wide_effect_result_tag)?,
    };

    let max_steps = alu.active_steps as u32 + 3;
    let (mut engine, execute) = execute_to_quiescence(
        &mut memory,
        interpreter,
        initial,
        32,
        max_steps,
    )?;
    if execute.active_reaction_count != alu.active_steps as u32 + 1
        || engine.current().len() != 1
    {
        return None;
    }

    let atomic_scope = execute.reactions.iter().all(|step| {
        step.scope_before.len() == 1 && step.scope_after.len() == 1
    });
    if !atomic_scope {
        return None;
    }

    let final_link = engine.current()[0];
    let (caller, successor) = memory.store.poles(final_link).ok()?;
    if caller != result_context {
        return None;
    }
    let actual = decode_state_in_store(
        &memory.store,
        runtime_schema,
        successor,
        zero,
        one,
    )?;
    if actual != expected || !memory.store.is_valid(before_state) {
        return None;
    }

    let (state_tag_check, result_sequence) =
        memory.store.poles(successor).ok()?;
    if state_tag_check != runtime_schema.state_tag {
        return None;
    }
    let result_recursive_wire =
        memory.store.export_anum(final_link).ok()?;
    let result_sequence_anum =
        memory.store.export_anum(result_sequence).ok()?;
    let identical_rerun_link_delta = identical_rerun(
        &mut memory,
        &mut engine,
        initial,
        &result_recursive_wire,
        max_steps,
    )?;
    let visual_links = visual_snapshot(&memory, &load.semantic_roots);

    let result = WebProofResultStage {
        memory_instance_id: memory.id.clone(),
        result_anum: result_recursive_wire,
        result_sequence_anum,
        decoded_value: actual.eax,
        decoded_value_hi: Some(actual.edx),
        oracle_value: expected.eax,
        oracle_value_hi: Some(expected.edx),
        oracle_matches: actual == expected,
        links_final: memory.store.link_count() as u32,
        identical_rerun_link_delta,
        visual_links,
    };
    let proof = WebStructuralProof {
        schema_version: 4,
        block: "M5B_STATE_MUL32".to_owned(),
        prepare,
        load,
        execute,
        result,
    };

    let (defined, values, undefined_mask) = flags_to_masks(actual);
    Some(WebArchitecturalStateExecution {
        outcome: WebArchitecturalStateOutcome {
            eax_before: before_value.eax,
            ebx_before: before_value.ebx,
            edx_before: before_value.edx,
            ecx_before: before_value.ecx,
            esi_before: before_value.esi,
            edi_before: before_value.edi,
            ebp_before: before_value.ebp,
            esp_before: before_value.esp,
            eip_before: before_value.eip,
            eax_after: actual.eax,
            ebx_after: actual.ebx,
            edx_after: actual.edx,
            ecx_after: actual.ecx,
            esi_after: actual.esi,
            edi_after: actual.edi,
            ebp_after: actual.ebp,
            esp_after: actual.esp,
            eip_after: actual.eip,
            flags_defined_mask: defined,
            flags_value_mask: values,
            flags_undefined_mask: undefined_mask,
            reactions: proof.execute.active_reaction_count,
            old_state_retained: 1,
            atomic_scope: 1,
            steady_link_delta: proof.result.identical_rerun_link_delta,
            quiescent: u8::from(proof.execute.final_quiescent),
        },
        proof,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bit(f: &FullFixture, value: u8) -> Handle {
        match value {
            0 => f.zero,
            1 => f.one,
            _ => panic!("bit"),
        }
    }

    fn flag_value(
        f: &FullFixture,
        p: &ArchitecturalStateProgram,
        value: Option<u8>,
    ) -> Handle {
        value.map_or(p.schema.undefined, |v| bit(f, v))
    }

    fn make_state(
        f: &mut FullFixture,
        p: &ArchitecturalStateProgram,
        value: StateValue,
    ) -> Handle {
        let eax = word(f, value.eax);
        let ebx = word(f, value.ebx);
        let edx = word(f, value.edx);
        let ecx = word(f, value.ecx);
        let esi = word(f, value.esi);
        let edi = word(f, value.edi);
        let ebp = word(f, value.ebp);
        let esp = word(f, value.esp);
        let eip = word(f, value.eip);
        let cf = flag_value(f, p, value.cf);
        let pf = flag_value(f, p, value.pf);
        let af = flag_value(f, p, value.af);
        let zf = flag_value(f, p, value.zf);
        let sf = flag_value(f, p, value.sf);
        let of = flag_value(f, p, value.of);
        state_link(
            &mut f.store,
            p.schema,
            eax,
            ebx,
            edx,
            cf,
            pf,
            af,
            zf,
            sf,
            of,
            ecx,
            esi,
            edi,
            ebp,
            esp,
            eip,
        )
    }

    fn make_full_effect(
        f: &mut FullFixture,
        p: &ArchitecturalStateProgram,
        writeback: u8,
        value: u32,
        flags: [u8; 6],
    ) -> Handle {
        let writeback = bit(f, writeback);
        let word = word(f, value);
        let cf = bit(f, flags[0]);
        let pf = bit(f, flags[1]);
        let af = bit(f, flags[2]);
        let zf = bit(f, flags[3]);
        let sf = bit(f, flags[4]);
        let of = bit(f, flags[5]);
        effect_template(
            &mut f.store,
            p.schema,
            writeback,
            word,
            PatchShape::FullSet6,
            cf,
            pf,
            af,
            zf,
            sf,
            of,
            f.zero,
        )
    }

    fn make_logic_effect(
        f: &mut FullFixture,
        p: &ArchitecturalStateProgram,
        writeback: u8,
        value: u32,
        pf: u8,
        zf: u8,
        sf: u8,
    ) -> Handle {
        let writeback = bit(f, writeback);
        let word = word(f, value);
        let pf = bit(f, pf);
        let zf = bit(f, zf);
        let sf = bit(f, sf);
        effect_template(
            &mut f.store,
            p.schema,
            writeback,
            word,
            PatchShape::Logic6,
            f.zero,
            pf,
            f.zero,
            zf,
            sf,
            f.zero,
            f.zero,
        )
    }

    fn make_inc_effect(
        f: &mut FullFixture,
        p: &ArchitecturalStateProgram,
        value: u32,
        flags: [u8; 5],
    ) -> Handle {
        let word = word(f, value);
        let pf = bit(f, flags[0]);
        let af = bit(f, flags[1]);
        let zf = bit(f, flags[2]);
        let sf = bit(f, flags[3]);
        let of = bit(f, flags[4]);
        effect_template(
            &mut f.store,
            p.schema,
            f.one,
            word,
            PatchShape::PreserveCf5,
            f.zero,
            pf,
            af,
            zf,
            sf,
            of,
            f.zero,
        )
    }

    fn make_wide_mul_effect(
        f: &mut FullFixture,
        p: &ArchitecturalStateProgram,
        lo: u32,
        hi: u32,
        cf: u8,
        of: u8,
    ) -> Handle {
        let lo = word(f, lo);
        let hi = word(f, hi);
        let wide =
            materialize_exact_sequence(&mut f.store, &[lo, hi]).unwrap();
        let cf = bit(f, cf);
        let of = bit(f, of);
        let actions = [
            set_flag_action(
                &mut f.store,
                p.schema.flags,
                p.schema.flags.cf,
                cf,
            ),
            undefined_flag_action(
                &mut f.store,
                p.schema.flags,
                p.schema.flags.pf,
            ),
            undefined_flag_action(
                &mut f.store,
                p.schema.flags,
                p.schema.flags.af,
            ),
            undefined_flag_action(
                &mut f.store,
                p.schema.flags,
                p.schema.flags.zf,
            ),
            undefined_flag_action(
                &mut f.store,
                p.schema.flags,
                p.schema.flags.sf,
            ),
            set_flag_action(
                &mut f.store,
                p.schema.flags,
                p.schema.flags.of,
                of,
            ),
        ];
        let patch =
            materialize_exact_sequence(&mut f.store, &actions).unwrap();
        let payload =
            materialize_exact_sequence(&mut f.store, &[wide, patch]).unwrap();
        f.store
            .ensure_pair(p.schema.wide_effect_result_tag, payload)
            .unwrap()
    }

    fn wide_transition(
        f: &mut FullFixture,
        p: &ArchitecturalStateProgram,
        state: Handle,
        low_target: Handle,
        high_target: Handle,
        effect: Handle,
    ) -> Option<Handle> {
        let frame = state_apply_wide_frame(
            &mut f.store,
            p.schema,
            state,
            low_target,
            high_target,
        );
        let initial = f.store.ensure_pair(frame, effect).unwrap();
        f.engine.set_current(&f.store, &[initial]).unwrap();
        let reaction = f.engine.run(&mut f.store).unwrap();
        if reaction.quiescent {
            return None;
        }
        assert_eq!(reaction.raw_rule_matches, 1);
        assert_eq!(reaction.transitioned_members, 1);
        assert_eq!(reaction.handoff_count, 1);
        assert_eq!(reaction.next_members.len(), 1);
        let final_member = f.engine.current()[0];
        let (caller, successor) = f.store.poles(final_member).unwrap();
        assert_eq!(caller, f.k);
        assert!(f.store.is_valid(state));
        Some(successor)
    }

    fn transition(
        f: &mut FullFixture,
        p: &ArchitecturalStateProgram,
        state: Handle,
        target: Handle,
        effect: Handle,
    ) -> Option<Handle> {
        let frame =
            state_apply_frame(&mut f.store, p.schema, state, target);
        let initial = f.store.ensure_pair(frame, effect).unwrap();
        f.engine.set_current(&f.store, &[initial]).unwrap();

        let before_links = f.store.link_count();
        let reaction = f.engine.run(&mut f.store).unwrap();
        if reaction.quiescent {
            return None;
        }
        assert_eq!(reaction.raw_rule_matches, 1);
        assert_eq!(reaction.transitioned_members, 1);
        assert_eq!(reaction.handoff_count, 1);
        assert_eq!(reaction.next_members.len(), 1);

        let final_member = f.engine.current()[0];
        let (caller, successor) = f.store.poles(final_member).unwrap();
        assert_eq!(caller, f.k);
        assert!(decode_state(f, p.schema, successor).is_some());
        assert!(f.store.is_valid(state), "old state remains physically present");

        let after_first = f.store.link_count();
        assert!(after_first >= before_links);

        let stable = f.engine.current_bank();
        let quiescent = f.engine.run(&mut f.store).unwrap();
        assert!(quiescent.quiescent);
        assert_eq!(quiescent.raw_rule_matches, 0);
        assert_eq!(f.engine.current_bank(), stable);

        f.engine.set_current(&f.store, &[initial]).unwrap();
        let links_before_rerun = f.store.link_count();
        let rerun = f.engine.run(&mut f.store).unwrap();
        assert!(!rerun.quiescent);
        assert_eq!(rerun.raw_rule_matches, 1);
        assert_eq!(f.store.link_count(), links_before_rerun);

        Some(successor)
    }

    fn initial() -> StateValue {
        StateValue {
            eax: 0x1122_3344,
            ebx: 0xaabb_ccdd,
            edx: 0x5566_7788,
            ecx: 0x0102_0304,
            esi: 0x1111_2222,
            edi: 0x3333_4444,
            ebp: 0x5555_6666,
            esp: 0x7777_8888,
            eip: 0x0040_1000,
            cf: Some(1),
            pf: Some(0),
            af: Some(1),
            zf: Some(0),
            sf: Some(1),
            of: Some(0),
        }
    }

    #[test]
    fn m5c_single_effect_targets_all_gprs_but_not_eip() {
        let targets = |p: &ArchitecturalStateProgram| [
            p.schema.eax,
            p.schema.ebx,
            p.schema.ecx,
            p.schema.edx,
            p.schema.esi,
            p.schema.edi,
            p.schema.ebp,
            p.schema.esp,
        ];

        for target_index in 0..8 {
            let mut f = FullFixture::new();
            let p = ArchitecturalStateProgram::install(&mut f);
            let old_value = initial();
            let old = make_state(&mut f, &p, old_value);
            let effect = make_full_effect(
                &mut f,
                &p,
                1,
                0xdead_beef,
                [0, 1, 0, 1, 0, 1],
            );
            let target = targets(&p)[target_index];
            let next =
                transition(&mut f, &p, old, target, effect).unwrap();
            let actual = decode_state(&f, p.schema, next).unwrap();

            let expected_registers = [
                actual.eax,
                actual.ebx,
                actual.ecx,
                actual.edx,
                actual.esi,
                actual.edi,
                actual.ebp,
                actual.esp,
            ];
            let old_registers = [
                old_value.eax,
                old_value.ebx,
                old_value.ecx,
                old_value.edx,
                old_value.esi,
                old_value.edi,
                old_value.ebp,
                old_value.esp,
            ];
            for (index, value) in expected_registers.into_iter().enumerate() {
                assert_eq!(
                    value,
                    if index == target_index {
                        0xdead_beef
                    } else {
                        old_registers[index]
                    },
                    "target index {target_index}, register index {index}"
                );
            }
            assert_eq!(actual.eip, old_value.eip);
        }

        let mut f = FullFixture::new();
        let p = ArchitecturalStateProgram::install(&mut f);
        let old = make_state(&mut f, &p, initial());
        let effect = make_full_effect(
            &mut f,
            &p,
            1,
            0xdead_beef,
            [0, 1, 0, 1, 0, 1],
        );
        assert!(
            transition(&mut f, &p, old, p.schema.eip, effect).is_none(),
            "EIP is structural state but not an ordinary ALU destination"
        );
    }

    #[test]
    fn m5b_direct_wide_applier_updates_two_registers_in_one_state() {
        let mut f = FullFixture::new();
        let p = ArchitecturalStateProgram::install(&mut f);
        let old_value = initial();
        let old = make_state(&mut f, &p, old_value);
        let effect = make_wide_mul_effect(
            &mut f,
            &p,
            0xffff_fffe,
            0x0000_0001,
            1,
            1,
        );

        let next = wide_transition(
            &mut f,
            &p,
            old,
            p.schema.eax,
            p.schema.edx,
            effect,
        )
        .unwrap();
        assert_eq!(
            decode_state(&f, p.schema, next).unwrap(),
            StateValue {
                eax: 0xffff_fffe,
                ebx: old_value.ebx,
                edx: 0x0000_0001,
                ecx: old_value.ecx,
                esi: old_value.esi,
                edi: old_value.edi,
                ebp: old_value.ebp,
                esp: old_value.esp,
                eip: old_value.eip,
                cf: Some(1),
                pf: None,
                af: None,
                zf: None,
                sf: None,
                of: Some(1),
            }
        );
    }

    #[test]
    fn m5b_repeated_targets_and_malformed_patch_fail_closed() {
        let mut f = FullFixture::new();
        let p = ArchitecturalStateProgram::install(&mut f);
        let old = make_state(&mut f, &p, initial());
        let effect = make_wide_mul_effect(
            &mut f,
            &p,
            0xffff_fffe,
            1,
            1,
            1,
        );

        assert!(
            wide_transition(
                &mut f,
                &p,
                old,
                p.schema.eax,
                p.schema.eax,
                effect,
            )
            .is_none(),
            "repeated wide destination id must fail closed"
        );

        let lo = word(&mut f, 0xffff_fffe);
        let hi = word(&mut f, 1);
        let wide =
            materialize_exact_sequence(&mut f.store, &[lo, hi]).unwrap();
        let set_cf_zero = set_flag_action(
            &mut f.store,
            p.schema.flags,
            p.schema.flags.cf,
            f.zero,
        );
        let set_cf_one = set_flag_action(
            &mut f.store,
            p.schema.flags,
            p.schema.flags.cf,
            f.one,
        );
        let bad_patch =
            materialize_exact_sequence(
                &mut f.store,
                &[set_cf_zero, set_cf_one],
            )
            .unwrap();
        let bad_payload =
            materialize_exact_sequence(&mut f.store, &[wide, bad_patch])
                .unwrap();
        let bad_effect = f
            .store
            .ensure_pair(p.schema.wide_effect_result_tag, bad_payload)
            .unwrap();

        assert!(
            wide_transition(
                &mut f,
                &p,
                old,
                p.schema.eax,
                p.schema.edx,
                bad_effect,
            )
            .is_none(),
            "malformed/conflicting wide patch must fail closed"
        );
        let non_bit = f
            .store
            .ensure_pair(p.schema.eax, p.schema.edx)
            .unwrap();
        let bad_cf = set_flag_action(
            &mut f.store,
            p.schema.flags,
            p.schema.flags.cf,
            non_bit,
        );
        let undef_pf = undefined_flag_action(
            &mut f.store,
            p.schema.flags,
            p.schema.flags.pf,
        );
        let undef_af = undefined_flag_action(
            &mut f.store,
            p.schema.flags,
            p.schema.flags.af,
        );
        let undef_zf = undefined_flag_action(
            &mut f.store,
            p.schema.flags,
            p.schema.flags.zf,
        );
        let undef_sf = undefined_flag_action(
            &mut f.store,
            p.schema.flags,
            p.schema.flags.sf,
        );
        let set_of = set_flag_action(
            &mut f.store,
            p.schema.flags,
            p.schema.flags.of,
            f.one,
        );
        let bad_flag_patch = materialize_exact_sequence(
            &mut f.store,
            &[bad_cf, undef_pf, undef_af, undef_zf, undef_sf, set_of],
        )
        .unwrap();
        let bad_flag_payload = materialize_exact_sequence(
            &mut f.store,
            &[wide, bad_flag_patch],
        )
        .unwrap();
        let bad_flag_effect = f
            .store
            .ensure_pair(
                p.schema.wide_effect_result_tag,
                bad_flag_payload,
            )
            .unwrap();
        assert!(
            wide_transition(
                &mut f,
                &p,
                old,
                p.schema.eax,
                p.schema.edx,
                bad_flag_effect,
            )
            .is_none(),
            "non-bit CF/OF value must fail closed"
        );
    }

    #[test]
    fn m5b_real_mul_pipeline_updates_edx_eax_and_flags_atomically() {
        let execution = web_prove_architectural_state_mul().unwrap();
        assert_eq!(execution.outcome.eax_before, 0xffff_ffff);
        assert_eq!(execution.outcome.edx_before, 0xa5a5_5a5a);
        assert_eq!(execution.outcome.ebx_before, 0x1122_3344);
        assert_eq!(execution.outcome.ecx_before, 0x0102_0304);
        assert_eq!(execution.outcome.eip_before, 0x0040_1000);
        assert_eq!(execution.outcome.eax_after, 0xffff_fffe);
        assert_eq!(execution.outcome.edx_after, 0x0000_0001);
        assert_eq!(execution.outcome.ebx_after, 0x1122_3344);
        assert_eq!(execution.outcome.ecx_after, execution.outcome.ecx_before);
        assert_eq!(execution.outcome.eip_after, execution.outcome.eip_before);
        assert_eq!(execution.outcome.flags_defined_mask, 0x0000_0801);
        assert_eq!(execution.outcome.flags_value_mask, 0x0000_0801);
        assert_eq!(execution.outcome.flags_undefined_mask, 0x0000_00d4);
        assert_eq!(execution.outcome.old_state_retained, 1);
        assert_eq!(execution.outcome.atomic_scope, 1);
        assert_eq!(execution.outcome.steady_link_delta, 0);
        assert_eq!(execution.outcome.quiescent, 1);
        assert_eq!(execution.proof.block, "M5B_STATE_MUL32");
        assert!(execution.proof.result.oracle_matches);
        assert!(execution.proof.execute.active_reaction_count > 2);
        assert!(execution.proof.execute.reactions.iter().all(|step| {
            step.scope_before.len() == 1 && step.scope_after.len() == 1
        }));
    }

    #[test]
    fn m5a_real_add_pipeline_composes_into_atomic_state() {
        let execution = web_prove_architectural_state_add().unwrap();
        assert_eq!(execution.outcome.eax_before, 0xffff_ffff);
        assert_eq!(execution.outcome.eax_after, 0);
        assert_eq!(execution.outcome.ebx_before, 0x1122_3344);
        assert_eq!(execution.outcome.ebx_after, 0x1122_3344);
        assert_eq!(execution.outcome.edx_before, 0x5566_7788);
        assert_eq!(execution.outcome.edx_after, 0x5566_7788);
        assert_eq!(execution.outcome.ecx_before, 0x0102_0304);
        assert_eq!(execution.outcome.ecx_after, execution.outcome.ecx_before);
        assert_eq!(execution.outcome.eip_before, 0x0040_1000);
        assert_eq!(execution.outcome.eip_after, execution.outcome.eip_before);
        assert_eq!(execution.outcome.flags_defined_mask, 0x0000_08d5);
        assert_eq!(execution.outcome.flags_value_mask, 0x0000_0055);
        assert_eq!(execution.outcome.flags_undefined_mask, 0);
        assert_eq!(execution.outcome.old_state_retained, 1);
        assert_eq!(execution.outcome.atomic_scope, 1);
        assert_eq!(execution.outcome.steady_link_delta, 0);
        assert_eq!(execution.outcome.quiescent, 1);
        assert_eq!(execution.proof.block, "M5A_STATE_ADD32");
        assert!(execution.proof.result.oracle_matches);
        assert!(execution.proof.execute.active_reaction_count > 1);
        assert!(execution.proof.execute.reactions.iter().all(|step| {
            step.scope_before.len() == 1 && step.scope_after.len() == 1
        }));
    }

    #[test]
    fn m5a_add_like_writeback_updates_eax_and_flags_atomically() {
        let mut f = FullFixture::new();
        let p = ArchitecturalStateProgram::install(&mut f);
        let old = make_state(&mut f, &p, initial());
        let effect = make_full_effect(
            &mut f,
            &p,
            1,
            0x5566_7788,
            [0, 1, 0, 1, 0, 1],
        );

        let next = transition(&mut f, &p, old, p.schema.eax, effect).unwrap();
        assert_eq!(
            decode_state(&f, p.schema, next).unwrap(),
            StateValue {
                eax: 0x5566_7788,
                ebx: 0xaabb_ccdd,
                edx: 0x5566_7788,
                ecx: 0x0102_0304,
                esi: 0x1111_2222,
                edi: 0x3333_4444,
                ebp: 0x5555_6666,
                esp: 0x7777_8888,
                eip: 0x0040_1000,
                cf: Some(0),
                pf: Some(1),
                af: Some(0),
                zf: Some(1),
                sf: Some(0),
                of: Some(1),
            }
        );
    }

    #[test]
    fn m5a_cmp_like_no_writeback_preserves_registers_and_updates_flags() {
        let mut f = FullFixture::new();
        let p = ArchitecturalStateProgram::install(&mut f);
        let old_value = initial();
        let old = make_state(&mut f, &p, old_value);
        let effect = make_full_effect(
            &mut f,
            &p,
            0,
            0xdead_beef,
            [0, 1, 1, 1, 0, 1],
        );

        let next = transition(&mut f, &p, old, p.schema.eax, effect).unwrap();
        let actual = decode_state(&f, p.schema, next).unwrap();
        assert_eq!(actual.eax, old_value.eax);
        assert_eq!(actual.ebx, old_value.ebx);
        assert_eq!(
            [actual.cf, actual.pf, actual.af, actual.zf, actual.sf, actual.of],
            [Some(0), Some(1), Some(1), Some(1), Some(0), Some(1)],
        );
    }

    #[test]
    fn m5a_test_like_logic_patch_publishes_explicit_undefined_af() {
        let mut f = FullFixture::new();
        let p = ArchitecturalStateProgram::install(&mut f);
        let old_value = initial();
        let old = make_state(&mut f, &p, old_value);
        let effect =
            make_logic_effect(&mut f, &p, 0, 0, 1, 1, 0);

        let next = transition(&mut f, &p, old, p.schema.ebx, effect).unwrap();
        let actual = decode_state(&f, p.schema, next).unwrap();
        assert_eq!(actual.eax, old_value.eax);
        assert_eq!(actual.ebx, old_value.ebx);
        assert_eq!(actual.cf, Some(0));
        assert_eq!(actual.pf, Some(1));
        assert_eq!(actual.af, None);
        assert_eq!(actual.zf, Some(1));
        assert_eq!(actual.sf, Some(0));
        assert_eq!(actual.of, Some(0));
    }

    #[test]
    fn m5a_inc_like_patch_preserves_cf() {
        let mut f = FullFixture::new();
        let p = ArchitecturalStateProgram::install(&mut f);
        let old_value = initial();
        let old = make_state(&mut f, &p, old_value);
        let effect =
            make_inc_effect(&mut f, &p, 0x1122_3345, [1, 0, 0, 0, 0]);

        let next = transition(&mut f, &p, old, p.schema.eax, effect).unwrap();
        let actual = decode_state(&f, p.schema, next).unwrap();
        assert_eq!(actual.eax, 0x1122_3345);
        assert_eq!(actual.ebx, old_value.ebx);
        assert_eq!(actual.cf, old_value.cf);
        assert_eq!(actual.pf, Some(1));
    }

    #[test]
    fn m5a_unknown_target_and_conflicting_patch_fail_closed() {
        let mut f = FullFixture::new();
        let p = ArchitecturalStateProgram::install(&mut f);
        let old = make_state(&mut f, &p, initial());
        let effect = make_full_effect(
            &mut f,
            &p,
            1,
            1,
            [0, 0, 0, 0, 0, 0],
        );

        let unknown_target =
            f.store.ensure_pair(p.schema.eax, p.schema.ebx).unwrap();
        assert!(
            transition(&mut f, &p, old, unknown_target, effect).is_none(),
            "unknown register id must not match a state-application rule"
        );

        let a = set_flag_action(
            &mut f.store,
            p.schema.flags,
            p.schema.flags.cf,
            f.zero,
        );
        let b = set_flag_action(
            &mut f.store,
            p.schema.flags,
            p.schema.flags.cf,
            f.one,
        );
        let bad_patch =
            materialize_exact_sequence(&mut f.store, &[a, b]).unwrap();
        let bad_word = word(&mut f, 7);
        let bad_payload = materialize_exact_sequence(
            &mut f.store,
            &[f.one, bad_word, bad_patch],
        )
        .unwrap();
        let bad_effect = f
            .store
            .ensure_pair(p.schema.effect_result_tag, bad_payload)
            .unwrap();

        assert!(
            transition(&mut f, &p, old, p.schema.eax, bad_effect).is_none(),
            "conflicting/unsupported patch shape must fail closed"
        );
    }
}
