use amemory_optimized_cpu_probe::{
    structural::materialize_exact_sequence,
    Handle, OptimizedLinkStore,
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct FlagPatchSchema {
    pub(crate) set_tag: Handle,
    pub(crate) undefined_tag: Handle,
    pub(crate) cf: Handle,
    pub(crate) pf: Handle,
    pub(crate) af: Handle,
    pub(crate) zf: Handle,
    pub(crate) sf: Handle,
    pub(crate) of: Handle,
}

impl FlagPatchSchema {
    pub(crate) fn install(
        store: &mut OptimizedLinkStore,
        seed: Handle,
        o: Handle,
        c: Handle,
    ) -> Self {
        // Deterministic structural namespace for the flag-effect ABI.
        // Repeated installation with the same seed reconstructs the same
        // canonical Links, allowing independent ALU components in one Memory
        // to publish patches understood by one future EFLAGS applier.
        let mut current = store.ensure_pair(seed, seed).unwrap();
        for pole in [c, o, c, c, o, o, c, o, o, c, c, o] {
            current = store.ensure_pair(current, pole).unwrap();
        }

        let mut flip = false;
        let mut next = |store: &mut OptimizedLinkStore| {
            let pole = if flip { c } else { o };
            flip = !flip;
            current = store.ensure_pair(current, pole).unwrap();
            current
        };

        let set_tag = next(store);
        let undefined_tag = next(store);
        let cf = next(store);
        let pf = next(store);
        let af = next(store);
        let zf = next(store);
        let sf = next(store);
        let of = next(store);

        Self {
            set_tag,
            undefined_tag,
            cf,
            pf,
            af,
            zf,
            sf,
            of,
        }
    }
}

pub(crate) fn set_flag_action(
    store: &mut OptimizedLinkStore,
    schema: FlagPatchSchema,
    flag: Handle,
    value: Handle,
) -> Handle {
    let payload =
        materialize_exact_sequence(store, &[flag, value]).unwrap();
    store.ensure_pair(schema.set_tag, payload).unwrap()
}

pub(crate) fn undefined_flag_action(
    store: &mut OptimizedLinkStore,
    schema: FlagPatchSchema,
    flag: Handle,
) -> Handle {
    let payload = materialize_exact_sequence(store, &[flag]).unwrap();
    store.ensure_pair(schema.undefined_tag, payload).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use amemory_optimized_cpu_probe::ROOT_HANDLE;

    #[test]
    fn m4_flag_patch_schema_reinstalls_canonically() {
        let mut store = OptimizedLinkStore::new();
        let o = store.ensure_start_self_closed(ROOT_HANDLE).unwrap();
        let c = store.ensure_end_self_closed(ROOT_HANDLE).unwrap();
        let seed = store.ensure_pair(c, o).unwrap();

        let a = FlagPatchSchema::install(&mut store, seed, o, c);
        let links = store.link_count();
        let b = FlagPatchSchema::install(&mut store, seed, o, c);

        assert_eq!(a.set_tag, b.set_tag);
        assert_eq!(a.undefined_tag, b.undefined_tag);
        assert_eq!(a.cf, b.cf);
        assert_eq!(a.pf, b.pf);
        assert_eq!(a.af, b.af);
        assert_eq!(a.zf, b.zf);
        assert_eq!(a.sf, b.sf);
        assert_eq!(a.of, b.of);
        assert_eq!(store.link_count(), links);
    }
}


pub(crate) fn install_alu_effect_result_tag(
    store: &mut OptimizedLinkStore,
    seed: Handle,
    o: Handle,
    c: Handle,
) -> Handle {
    // Shared stable envelope tag for every ALU effect producer whose payload is:
    // ExactSequence_R([WriteBack, Word, FlagPatch]).
    // Arithmetic/CMP and logic/TEST therefore converge before architectural
    // register/EFLAGS application.
    let mut current = store.ensure_pair(seed, seed).unwrap();
    for pole in [o, o, c, o, c, c, o, c, o, c, o, o, c, c] {
        current = store.ensure_pair(current, pole).unwrap();
    }
    current
}
