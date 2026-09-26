#[cfg(test)]
mod flag_patch;
#[cfg(test)]
mod flags_n;
#[cfg(test)]
mod logic_n;
#[cfg(test)]
mod logic_effect_n;
#[cfg(test)]
mod full_adder;
#[cfg(test)]
mod ripple4;
#[cfg(test)]
mod ripple_n;
#[cfg(test)]
mod subtractor;
#[cfg(test)]
mod subtractor_n;
#[cfg(test)]
mod arithmetic_n;
#[cfg(test)]
mod arithmetic_effect_n;
#[cfg(test)]
mod shift1_n;
#[cfg(test)]
mod shift32_n;
#[cfg(test)]
mod rotate32_n;
#[cfg(test)]
mod rotate_carry32_n;
#[cfg(test)]
mod mux_n;
#[cfg(test)]
mod unary_arith_n;
#[cfg(test)]
mod wide64_n;

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
    use amemory_browser_probe::{
        amemory_anum_cpu_export,
        amemory_anum_cpu_import,
        amemory_anum_cpu_output_get,
        amemory_anum_cpu_reset_pool,
        amemory_anum_cpu_set_token,
        amemory_reaction_current_count,
        amemory_reaction_current_member,
        amemory_reaction_handoff_count,
        amemory_reaction_matched_relations,
        amemory_reaction_quiescent,
        amemory_reaction_reset,
        amemory_reaction_run,
        amemory_reaction_set_current_count,
        amemory_reaction_set_current_member,
        amemory_reaction_set_theory_count,
        amemory_reaction_set_theory_relation,
        amemory_reaction_snapshot_theory,
    };
    use amemory_optimized_cpu_probe::PortableReactionResult;
    use std::sync::Mutex;

    static REFERENCE_LOCK: Mutex<()> = Mutex::new(());

    // Benchmark-local role assignment over already accepted ROOT-basis Links.
    // No claim is made that O "means AND" or C "means APPLY" in MTS itself.
    // They are simply distinct portable structural symbols used by this fixture.
    const K: &str = "8";       // R
    const FN_AND: &str = "98"; // O, benchmark-local role: AND function
    const APPLY: &str = "68";  // C, benchmark-local role: application constructor
    const FN_HALF: &str = "998"; // benchmark-local role: Half Adder function
    const BIT0: &str = "16898"; // U / Q-denotation 0
    const BIT1: &str = "19868"; // L / Q-denotation 1

    fn pair(start: &str, end: &str) -> String {
        format!("1{start}{end}")
    }

    fn start(value: &str) -> String {
        format!("9{value}")
    }

    /// Accepted MTS ExactSequence topology:
    ///
    ///   current := R
    ///   payload := current ⟼ value
    ///   current := START(payload)
    ///
    /// The sequence therefore always originates at A-root R.
    fn exact_sequence(values: &[&str]) -> String {
        let mut current = K.to_owned();
        for value in values {
            current = start(&pair(&current, value));
        }
        current
    }

    /// Structural application term. APPLY is only a benchmark-local role;
    /// application identity itself is fully structural and portable.
    fn call(function: &str, argument: &str) -> String {
        pair(APPLY, &pair(function, argument))
    }

    /// Sequential partial function after one argument.
    ///
    /// F_a is a real Link, not host closure state.
    fn partial(function: &str, argument: &str) -> String {
        pair(function, argument)
    }

    fn reference_import(source: &str) -> u32 {
        for (index, byte) in source.bytes().enumerate() {
            let token = if byte.is_ascii_digit() {
                (byte - b'0') as u32
            } else {
                255
            };
            assert_eq!(amemory_anum_cpu_set_token(index as u32, token), 1);
        }
        amemory_anum_cpu_import(source.len() as u32)
    }

    fn reference_export(handle: u32) -> String {
        let len = amemory_anum_cpu_export(handle);
        assert_ne!(len, u32::MAX);
        let mut out = String::new();
        for index in 0..len {
            out.push(char::from_digit(amemory_anum_cpu_output_get(index), 10).unwrap());
        }
        out
    }

    fn reference_observe() -> PortableReactionResult {
        let count = amemory_reaction_current_count();
        let mut scope = Vec::new();
        for index in 0..count {
            scope.push(reference_export(amemory_reaction_current_member(index)));
        }
        scope.sort();
        scope.dedup();
        PortableReactionResult {
            scope,
            matched_relations: amemory_reaction_matched_relations(),
            handoff: amemory_reaction_handoff_count(),
            quiescent: amemory_reaction_quiescent() == 1,
        }
    }

    fn reference_prepare(
        theory_sources: &[String],
        other_sources: &[String],
    ) -> (Vec<u32>, Vec<u32>) {
        amemory_anum_cpu_reset_pool();
        let theory = theory_sources
            .iter()
            .map(|source| {
                let handle = reference_import(source);
                assert_ne!(handle, u32::MAX, "reference rejected Theory {source}");
                handle
            })
            .collect::<Vec<_>>();
        let other = other_sources
            .iter()
            .map(|source| {
                let handle = reference_import(source);
                assert_ne!(handle, u32::MAX, "reference rejected fixture {source}");
                handle
            })
            .collect::<Vec<_>>();

        amemory_reaction_reset();
        for (index, relation) in theory.iter().enumerate() {
            assert_eq!(
                amemory_reaction_set_theory_relation(index as u32, *relation),
                1
            );
        }
        assert_eq!(amemory_reaction_set_theory_count(theory.len() as u32), 1);
        assert_eq!(amemory_reaction_snapshot_theory(), 1);
        (theory, other)
    }

    fn reference_set_single_current(handle: u32) {
        assert_eq!(amemory_reaction_set_current_member(0, handle), 1);
        assert_eq!(amemory_reaction_set_current_count(1), 1);
    }

    fn reference_run() -> PortableReactionResult {
        assert_eq!(amemory_reaction_run(), 1);
        reference_observe()
    }

    fn optimized_prepare(
        theory_sources: &[String],
        other_sources: &[String],
    ) -> (OptimizedLinkStore, OptimizedReactionEngine, Vec<u32>, Vec<u32>) {
        let mut store = OptimizedLinkStore::new();
        let theory = theory_sources
            .iter()
            .map(|source| store.import_anum(source).unwrap())
            .collect::<Vec<_>>();
        let other = other_sources
            .iter()
            .map(|source| store.import_anum(source).unwrap())
            .collect::<Vec<_>>();

        let mut engine = OptimizedReactionEngine::new(32);
        engine.set_theory(&store, &theory).unwrap();
        engine.snapshot_theory(&store).unwrap();
        (store, engine, theory, other)
    }

    fn optimized_run_single(
        engine: &mut OptimizedReactionEngine,
        store: &OptimizedLinkStore,
        current: u32,
    ) -> PortableReactionResult {
        engine.set_current(store, &[current]).unwrap();
        engine.run(store).unwrap();
        engine.portable_result(store).unwrap()
    }

    fn and_expected(a: &str, b: &str) -> &'static str {
        if a == BIT1 && b == BIT1 { BIT1 } else { BIT0 }
    }

    fn sequential_theory() -> Vec<String> {
        let and0 = partial(FN_AND, BIT0);
        let and1 = partial(FN_AND, BIT1);

        vec![
            pair(&call(FN_AND, BIT0), &and0),
            pair(&call(FN_AND, BIT1), &and1),
            pair(&call(&and0, BIT0), BIT0),
            pair(&call(&and0, BIT1), BIT0),
            pair(&call(&and1, BIT0), BIT0),
            pair(&call(&and1, BIT1), BIT1),
        ]
    }

    fn parallel_theory() -> Vec<String> {
        [BIT0, BIT1]
            .into_iter()
            .flat_map(|a| {
                [BIT0, BIT1].into_iter().map(move |b| {
                    let args = exact_sequence(&[a, b]);
                    pair(&call(FN_AND, &args), and_expected(a, b))
                })
            })
            .collect()
    }

    fn half_expected(a: &str, b: &str) -> (&'static str, &'static str) {
        match (a == BIT1, b == BIT1) {
            (false, false) => (BIT0, BIT0),
            (false, true) | (true, false) => (BIT1, BIT0),
            (true, true) => (BIT0, BIT1),
        }
    }

    fn half_result_sequence(a: &str, b: &str) -> String {
        let (sum, carry) = half_expected(a, b);
        exact_sequence(&[sum, carry])
    }

    fn sequential_half_theory() -> Vec<String> {
        let half0 = partial(FN_HALF, BIT0);
        let half1 = partial(FN_HALF, BIT1);

        vec![
            pair(&call(FN_HALF, BIT0), &half0),
            pair(&call(FN_HALF, BIT1), &half1),
            pair(&call(&half0, BIT0), &half_result_sequence(BIT0, BIT0)),
            pair(&call(&half0, BIT1), &half_result_sequence(BIT0, BIT1)),
            pair(&call(&half1, BIT0), &half_result_sequence(BIT1, BIT0)),
            pair(&call(&half1, BIT1), &half_result_sequence(BIT1, BIT1)),
        ]
    }

    fn parallel_half_theory() -> Vec<String> {
        [BIT0, BIT1]
            .into_iter()
            .flat_map(|a| {
                [BIT0, BIT1].into_iter().map(move |b| {
                    let args = exact_sequence(&[a, b]);
                    pair(&call(FN_HALF, &args), &half_result_sequence(a, b))
                })
            })
            .collect()
    }

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

    #[test]
    fn p1_exact_sequence_is_root_originating_and_not_an_ordinary_pair() {
        assert_eq!(exact_sequence(&[]), "8");

        let seq01 = exact_sequence(&[BIT0, BIT1]);
        let seq10 = exact_sequence(&[BIT1, BIT0]);
        let raw_pair01 = pair(BIT0, BIT1);

        assert_ne!(seq01, raw_pair01, "ExactSequence([0,1]) != PAIR(0,1)");
        assert_ne!(seq01, seq10, "ExactSequence order is semantic");

        // Reconstruct the accepted two-step topology literally.
        let s1 = start(&pair(K, BIT0));
        let s2 = start(&pair(&s1, BIT1));
        assert_eq!(seq01, s2);
    }

    #[test]
    fn s1_sequential_and_returns_portable_partial_function_then_value() {
        let _guard = REFERENCE_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let theory = sequential_theory();

        for a in [BIT0, BIT1] {
            for b in [BIT0, BIT1] {
                let expected_partial = partial(FN_AND, a);
                let first_call = call(FN_AND, a);
                let first_current = pair(K, &first_call);
                let first_successor = pair(K, &expected_partial);

                // The second invocation uses the function structurally returned
                // by stage 1. Host supplies only the next argument b.
                let second_call = call(&expected_partial, b);
                let second_current = pair(K, &second_call);
                let expected_value = and_expected(a, b);
                let second_successor = pair(K, expected_value);

                let fixture = vec![
                    first_current.clone(),
                    first_successor.clone(),
                    second_current.clone(),
                    second_successor.clone(),
                ];

                // Reference CPU: one fixed TheorySnapshot for both applications.
                let (_, reference_fixture) = reference_prepare(&theory, &fixture);
                reference_set_single_current(reference_fixture[0]);
                let reference_first = reference_run();
                assert_eq!(reference_first.scope, vec![first_successor.clone()]);
                assert_eq!(reference_first.matched_relations, 1);
                assert_eq!(reference_first.handoff, 1);
                assert!(!reference_first.quiescent);

                // The actual first result is the portable partial function under K.
                assert_eq!(reference_first.scope[0], pair(K, &expected_partial));

                reference_set_single_current(reference_fixture[2]);
                let reference_second = reference_run();
                assert_eq!(reference_second.scope, vec![second_successor.clone()]);
                assert_eq!(reference_second.matched_relations, 1);
                assert_eq!(reference_second.handoff, 1);
                assert!(!reference_second.quiescent);

                // Optimized CPU runs the exact same structural fixture/Theory.
                let (store, mut engine, _, optimized_fixture) =
                    optimized_prepare(&theory, &fixture);
                let optimized_first =
                    optimized_run_single(&mut engine, &store, optimized_fixture[0]);
                assert_eq!(optimized_first, reference_first);

                let optimized_second =
                    optimized_run_single(&mut engine, &store, optimized_fixture[2]);
                assert_eq!(optimized_second, reference_second);

                // Portable function/result identity, not local handle equality.
                assert_eq!(
                    store.export_anum(optimized_fixture[1]).unwrap(),
                    first_successor
                );
                assert_eq!(
                    store.export_anum(optimized_fixture[3]).unwrap(),
                    second_successor
                );
            }
        }
    }

    #[test]
    fn p1_parallel_and_maps_one_root_originating_sequence_directly_to_value() {
        let _guard = REFERENCE_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let theory = parallel_theory();

        for a in [BIT0, BIT1] {
            for b in [BIT0, BIT1] {
                let args = exact_sequence(&[a, b]);
                let application = call(FN_AND, &args);
                let current = pair(K, &application);
                let expected_value = and_expected(a, b);
                let successor = pair(K, expected_value);
                let fixture = vec![current.clone(), successor.clone()];

                let (_, reference_fixture) = reference_prepare(&theory, &fixture);
                reference_set_single_current(reference_fixture[0]);
                let reference = reference_run();
                assert_eq!(reference.scope, vec![successor.clone()]);
                assert_eq!(reference.matched_relations, 1);
                assert_eq!(reference.handoff, 1);
                assert!(!reference.quiescent);

                let (store, mut engine, _, optimized_fixture) =
                    optimized_prepare(&theory, &fixture);
                let optimized =
                    optimized_run_single(&mut engine, &store, optimized_fixture[0]);

                assert_eq!(optimized, reference);
                assert_eq!(optimized.scope, vec![successor]);
            }
        }
    }

    #[test]
    fn sequential_and_partial_functions_are_structural_and_distinct() {
        let mut store = OptimizedLinkStore::new();

        let and0 = partial(FN_AND, BIT0);
        let and1 = partial(FN_AND, BIT1);
        let f0 = store.import_anum(&and0).unwrap();
        let f1 = store.import_anum(&and1).unwrap();

        assert_ne!(f0, f1);
        assert_eq!(store.export_anum(f0).unwrap(), and0);
        assert_eq!(store.export_anum(f1).unwrap(), and1);

        // Canonical reconstruction reuses the same local identity within memory.
        assert_eq!(store.import_anum(&and0).unwrap(), f0);
        assert_eq!(store.import_anum(&and1).unwrap(), f1);
    }


    #[test]
    fn c2a_half_adder_result_is_one_root_originating_two_position_sequence() {
        for a in [BIT0, BIT1] {
            for b in [BIT0, BIT1] {
                let result = half_result_sequence(a, b);
                let (sum, carry) = half_expected(a, b);

                assert_eq!(result, exact_sequence(&[sum, carry]));
                assert_ne!(result, pair(sum, carry), "Half Adder result must not collapse to PAIR");

                let reversed = exact_sequence(&[carry, sum]);
                if sum != carry {
                    assert_ne!(result, reversed, "Sum/Carry positions are ordered");
                }
            }
        }
    }

    #[test]
    fn c2a_sequential_half_adder_returns_partial_function_then_sum_carry_sequence() {
        let _guard = REFERENCE_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let theory = sequential_half_theory();

        for a in [BIT0, BIT1] {
            for b in [BIT0, BIT1] {
                let expected_partial = partial(FN_HALF, a);
                let first_call = call(FN_HALF, a);
                let first_current = pair(K, &first_call);
                let first_successor = pair(K, &expected_partial);

                let second_call = call(&expected_partial, b);
                let second_current = pair(K, &second_call);
                let result_sequence = half_result_sequence(a, b);
                let second_successor = pair(K, &result_sequence);

                let fixture = vec![
                    first_current.clone(),
                    first_successor.clone(),
                    second_current.clone(),
                    second_successor.clone(),
                ];

                let (_, reference_fixture) = reference_prepare(&theory, &fixture);
                reference_set_single_current(reference_fixture[0]);
                let reference_first = reference_run();
                assert_eq!(reference_first.scope, vec![first_successor.clone()]);
                assert_eq!(reference_first.matched_relations, 1);
                assert_eq!(reference_first.handoff, 1);
                assert!(!reference_first.quiescent);

                reference_set_single_current(reference_fixture[2]);
                let reference_second = reference_run();
                assert_eq!(reference_second.scope, vec![second_successor.clone()]);
                assert_eq!(reference_second.matched_relations, 1);
                assert_eq!(reference_second.handoff, 1);
                assert!(!reference_second.quiescent);

                let (store, mut engine, _, optimized_fixture) =
                    optimized_prepare(&theory, &fixture);

                let optimized_first =
                    optimized_run_single(&mut engine, &store, optimized_fixture[0]);
                assert_eq!(optimized_first, reference_first);

                let optimized_second =
                    optimized_run_single(&mut engine, &store, optimized_fixture[2]);
                assert_eq!(optimized_second, reference_second);

                assert_eq!(
                    store.export_anum(optimized_fixture[1]).unwrap(),
                    first_successor
                );
                assert_eq!(
                    store.export_anum(optimized_fixture[3]).unwrap(),
                    second_successor
                );
            }
        }
    }

    #[test]
    fn c2a_parallel_half_adder_maps_argument_sequence_to_sum_carry_sequence() {
        let _guard = REFERENCE_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let theory = parallel_half_theory();

        for a in [BIT0, BIT1] {
            for b in [BIT0, BIT1] {
                let args = exact_sequence(&[a, b]);
                let result_sequence = half_result_sequence(a, b);
                let current = pair(K, &call(FN_HALF, &args));
                let successor = pair(K, &result_sequence);
                let fixture = vec![current.clone(), successor.clone()];

                let (_, reference_fixture) = reference_prepare(&theory, &fixture);
                reference_set_single_current(reference_fixture[0]);
                let reference = reference_run();

                assert_eq!(reference.scope, vec![successor.clone()]);
                assert_eq!(reference.matched_relations, 1);
                assert_eq!(reference.handoff, 1);
                assert!(!reference.quiescent);

                let (store, mut engine, _, optimized_fixture) =
                    optimized_prepare(&theory, &fixture);
                let optimized =
                    optimized_run_single(&mut engine, &store, optimized_fixture[0]);

                assert_eq!(optimized, reference);
                assert_eq!(optimized.scope, vec![successor]);
            }
        }
    }

    #[test]
    fn c2a_half_adder_truth_table_is_exact() {
        assert_eq!(half_expected(BIT0, BIT0), (BIT0, BIT0));
        assert_eq!(half_expected(BIT0, BIT1), (BIT1, BIT0));
        assert_eq!(half_expected(BIT1, BIT0), (BIT1, BIT0));
        assert_eq!(half_expected(BIT1, BIT1), (BIT0, BIT1));
    }

}
