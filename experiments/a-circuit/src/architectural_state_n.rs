use super::{
    flag_patch::{
        install_alu_effect_result_tag, set_flag_action, undefined_flag_action,
        FlagPatchSchema,
    },
    full_adder::{
        call, define_bundle_rule, index_rule_for, Fixture as FullFixture,
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
    pub(crate) eax: Handle,
    pub(crate) ebx: Handle,
    pub(crate) undefined: Handle,
    pub(crate) flags: FlagPatchSchema,
    pub(crate) effect_result_tag: Handle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StateValue {
    eax: u32,
    ebx: u32,
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
    cf: Handle,
    pf: Handle,
    af: Handle,
    zf: Handle,
    sf: Handle,
    of: Handle,
) -> Handle {
    let fields = [
        binding(store, schema.eax, eax),
        binding(store, schema.ebx, ebx),
        binding(store, schema.flags.cf, cf),
        binding(store, schema.flags.pf, pf),
        binding(store, schema.flags.af, af),
        binding(store, schema.flags.zf, zf),
        binding(store, schema.flags.sf, sf),
        binding(store, schema.flags.of, of),
    ];
    let payload = materialize_exact_sequence(store, &fields).unwrap();
    store.ensure_pair(schema.state_tag, payload).unwrap()
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
    let k = anchors.next(&mut f.store);
    let old_eax = anchors.next(&mut f.store);
    let old_ebx = anchors.next(&mut f.store);
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
        old_cf,
        old_pf,
        old_af,
        old_zf,
        old_sf,
        old_of,
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
    let args = materialize_exact_sequence(
        &mut f.store,
        &[before_state, target, before_effect],
    )
    .unwrap();
    let invocation = call(
        &mut f.store,
        f.apply,
        schema.apply_state,
        args,
    );
    let before = f.store.ensure_pair(k, invocation).unwrap();

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
        next_cf,
        next_pf,
        next_af,
        next_zf,
        next_sf,
        next_of,
    );
    let after = f.store.ensure_pair(k, after_state).unwrap();

    let mut roles = vec![
        k, old_eax, old_ebx, old_cf, old_pf, old_af, old_zf, old_sf,
        old_of, word,
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
    index_rule_for(&mut f.store, &[f.apply], admission);
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

        let seed = f
            .store
            .ensure_pair(f.interpreter, effect_result_tag)
            .unwrap();
        let mut anchors = AnchorGen::new(&mut f.store, seed, f.o, f.c);
        let state_tag = anchors.next(&mut f.store);
        let apply_state = anchors.next(&mut f.store);
        let eax = anchors.next(&mut f.store);
        let ebx = anchors.next(&mut f.store);
        let undefined = anchors.next(&mut f.store);

        let schema = ArchitecturalStateSchema {
            state_tag,
            apply_state,
            eax,
            ebx,
            undefined,
            flags,
            effect_result_tag,
        };

        for target in [schema.eax, schema.ebx] {
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
    if fields.len() != 8 {
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
    let cf = decode_flag(
        f,
        schema,
        decode_binding(&f.store, fields[2], schema.flags.cf)?,
    )?;
    let pf = decode_flag(
        f,
        schema,
        decode_binding(&f.store, fields[3], schema.flags.pf)?,
    )?;
    let af = decode_flag(
        f,
        schema,
        decode_binding(&f.store, fields[4], schema.flags.af)?,
    )?;
    let zf = decode_flag(
        f,
        schema,
        decode_binding(&f.store, fields[5], schema.flags.zf)?,
    )?;
    let sf = decode_flag(
        f,
        schema,
        decode_binding(&f.store, fields[6], schema.flags.sf)?,
    )?;
    let of = decode_flag(
        f,
        schema,
        decode_binding(&f.store, fields[7], schema.flags.of)?,
    )?;

    Some(StateValue {
        eax,
        ebx,
        cf,
        pf,
        af,
        zf,
        sf,
        of,
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
            cf,
            pf,
            af,
            zf,
            sf,
            of,
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

    fn transition(
        f: &mut FullFixture,
        p: &ArchitecturalStateProgram,
        state: Handle,
        target: Handle,
        effect: Handle,
    ) -> Option<Handle> {
        let args =
            materialize_exact_sequence(&mut f.store, &[state, target, effect])
                .unwrap();
        let invocation =
            call(&mut f.store, f.apply, p.schema.apply_state, args);
        let initial = f.store.ensure_pair(f.k, invocation).unwrap();
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
            cf: Some(1),
            pf: Some(0),
            af: Some(1),
            zf: Some(0),
            sf: Some(1),
            of: Some(0),
        }
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
