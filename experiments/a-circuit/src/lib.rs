use amemory_optimized_cpu_probe::{OptimizedLinkStore, OptimizedReactionEngine};
use std::collections::BTreeSet;

fn normalized_handles(values: &[u32]) -> BTreeSet<u32> {
    values.iter().copied().collect()
}

fn run_once(
    store: &OptimizedLinkStore,
    theory: &[u32],
    current: &[u32],
) -> BTreeSet<u32> {
    let mut engine = OptimizedReactionEngine::new(32);
    engine.set_current(store, current).unwrap();
    engine.set_theory(store, theory).unwrap();
    engine.snapshot_theory(store).unwrap();
    engine.run(store).unwrap();
    normalized_handles(engine.current())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c0_unary_not_executes_inside_amemory() {
        let mut store = OptimizedLinkStore::new();

        // Provisional tiny structural fixture.
        //
        // Context/token K is preserved by the grounded reaction.
        // The antecedent carries the current boolean state.
        //
        // K⟼0 + (0⟼1) => K⟼1
        // K⟼1 + (1⟼0) => K⟼0
        //
        // These Anums are used only as distinct ROOT-grounded structures for
        // this benchmark witness; no host gate evaluator participates.
        let k = store.import_anum("8").unwrap();
        let bit0 = store.import_anum("98").unwrap();
        let bit1 = store.import_anum("68").unwrap();

        let not_0_to_1 = store.ensure_pair(bit0, bit1).unwrap();
        let not_1_to_0 = store.ensure_pair(bit1, bit0).unwrap();

        let k0 = store.ensure_pair(k, bit0).unwrap();
        let k1 = store.ensure_pair(k, bit1).unwrap();

        let theory = [not_0_to_1, not_1_to_0];

        let out0 = run_once(&store, &theory, &[k0]);
        let out1 = run_once(&store, &theory, &[k1]);

        assert_eq!(out0, BTreeSet::from([k1]));
        assert_eq!(out1, BTreeSet::from([k0]));

        assert_eq!(store.export_anum(k0).unwrap(), "1898");
        assert_eq!(store.export_anum(k1).unwrap(), "1868");
    }

    #[test]
    fn c1_fixed_theory_reaction_is_additive_over_independent_current_members() {
        let mut store = OptimizedLinkStore::new();

        let k = store.import_anum("8").unwrap();

        // Three ordinary non-ROOT states: A, B and desired OUT.
        let a = store.import_anum("98").unwrap();
        let b = store.import_anum("68").unwrap();
        let out = store.import_anum("16898").unwrap();
        let states = [a, b, out];

        // Pre-materialize every possible K⟼state successor. This removes
        // materialization authority from the experiment: a missing target
        // cannot explain the result.
        let current = states
            .iter()
            .map(|state| store.ensure_pair(k, *state).unwrap())
            .collect::<Vec<_>>();

        // Pre-materialize every possible unary Theory relation over this
        // three-state universe. There are 2^9 = 512 possible fixed Theories.
        let mut all_relations = Vec::new();
        for antecedent in states {
            for output in states {
                all_relations.push(store.ensure_pair(antecedent, output).unwrap());
            }
        }
        assert_eq!(all_relations.len(), 9);

        let a_current = current[0];
        let b_current = current[1];

        for mask in 0_u16..(1_u16 << all_relations.len()) {
            let theory = all_relations
                .iter()
                .enumerate()
                .filter_map(|(i, relation)| ((mask >> i) & 1 == 1).then_some(*relation))
                .collect::<Vec<_>>();

            let fa = run_once(&store, &theory, &[a_current]);
            let fb = run_once(&store, &theory, &[b_current]);
            let fab = run_once(&store, &theory, &[a_current, b_current]);

            let union = fa.union(&fb).copied().collect::<BTreeSet<_>>();

            assert_eq!(
                fab, union,
                "fixed grounded Theory ceased to be pointwise/additive for mask {mask}"
            );
        }
    }

    #[test]
    fn c1_true_and_cannot_be_realized_by_pointwise_unary_reaction_without_prejoined_state() {
        let mut store = OptimizedLinkStore::new();

        let k = store.import_anum("8").unwrap();
        let a = store.import_anum("98").unwrap();
        let b = store.import_anum("68").unwrap();
        let out = store.import_anum("16898").unwrap();
        let states = [a, b, out];

        let ka = store.ensure_pair(k, a).unwrap();
        let kb = store.ensure_pair(k, b).unwrap();
        let kout = store.ensure_pair(k, out).unwrap();

        let mut all_relations = Vec::new();
        for antecedent in states {
            for output in states {
                all_relations.push(store.ensure_pair(antecedent, output).unwrap());
            }
        }

        // A genuine two-premise AND-style condition needs a result that appears
        // for {A,B} but not for {A} and not for {B}.
        //
        // Exhaust every fixed unary Theory on this closed three-state universe.
        // None can satisfy that requirement in one reaction because the reaction
        // is the union of each member's independent image.
        let mut realizations = 0usize;

        for mask in 0_u16..(1_u16 << all_relations.len()) {
            let theory = all_relations
                .iter()
                .enumerate()
                .filter_map(|(i, relation)| ((mask >> i) & 1 == 1).then_some(*relation))
                .collect::<Vec<_>>();

            let a_only = run_once(&store, &theory, &[ka]);
            let b_only = run_once(&store, &theory, &[kb]);
            let both = run_once(&store, &theory, &[ka, kb]);

            let behaves_like_and =
                !a_only.contains(&kout) && !b_only.contains(&kout) && both.contains(&kout);

            if behaves_like_and {
                realizations += 1;
            }
        }

        assert_eq!(
            realizations, 0,
            "a true two-premise AND unexpectedly appeared without a joined antecedent"
        );
    }
}
