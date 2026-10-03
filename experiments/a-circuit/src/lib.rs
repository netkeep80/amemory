mod memory_n;
mod memory_word_n;
mod memory32_n;
mod architectural_state_n;
mod instruction_fetch_n;
mod stack_n;
mod flag_patch;
mod flags_n;
mod logic_n;
mod logic_effect_n;
mod full_adder;
mod ripple4;
mod ripple_n;
mod subtractor;
mod subtractor_n;
mod arithmetic_n;
mod arithmetic_effect_n;
mod shift1_n;
mod shift32_n;
mod rotate32_n;
mod rotate_carry32_n;
mod mux_n;
mod observability;
mod session_contract;
mod runtime_session;
mod linksdb_session;
mod proof_n;
mod scenario;
mod scenario_registry;
mod scenario_cpu_adapter;
mod scenario_runner;
mod scenario_web;
mod unary_arith_n;
mod wide64_n;
mod mul32_n;
mod mul_effect_n;
mod web_lab;

#[cfg(test)]
mod tests {
    use super::*;
    use amemory_browser_probe::ReferenceMemoryInstance;
    use amemory_optimized_cpu_probe::OptimizedLinkStore;
    use std::collections::BTreeSet;

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct ReferenceReactionResult {
        scope: Vec<String>,
        matched_relations: u32,
        handoff: u32,
        quiescent: bool,
    }
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

    fn reference_import(
        memory: &mut ReferenceMemoryInstance,
        source: &str,
    ) -> u32 {
        memory.import_recursive_wire(source)
    }

    fn reference_export(
        memory: &ReferenceMemoryInstance,
        handle: u32,
    ) -> String {
        memory
            .export_recursive_wire(handle)
            .expect("reference export rejected valid handle")
    }

    fn reference_observe(
        memory: &ReferenceMemoryInstance,
    ) -> ReferenceReactionResult {
        let count = memory.reaction_current_count();
        let mut scope = Vec::new();
        for index in 0..count {
            scope.push(reference_export(
                memory,
                memory.reaction_current_member(index),
            ));
        }
        scope.sort();
        scope.dedup();
        ReferenceReactionResult {
            scope,
            matched_relations: memory.reaction_matched_relations(),
            handoff: memory.reaction_handoff_count(),
            quiescent: memory.reaction_quiescent() == 1,
        }
    }

    fn reference_prepare(
        memory: &mut ReferenceMemoryInstance,
        theory_sources: &[String],
        other_sources: &[String],
    ) -> (Vec<u32>, Vec<u32>) {
        memory.reset_pool();
        let theory = theory_sources
            .iter()
            .map(|source| {
                let handle = reference_import(memory, source);
                assert_ne!(handle, u32::MAX, "reference rejected Theory {source}");
                handle
            })
            .collect::<Vec<_>>();
        let other = other_sources
            .iter()
            .map(|source| {
                let handle = reference_import(memory, source);
                assert_ne!(handle, u32::MAX, "reference rejected fixture {source}");
                handle
            })
            .collect::<Vec<_>>();

        memory.reaction_reset();
        for (index, relation) in theory.iter().enumerate() {
            assert_eq!(
                memory.reaction_set_theory_relation(index as u32, *relation),
                1
            );
        }
        assert_eq!(memory.reaction_set_theory_count(theory.len() as u32), 1);
        assert_eq!(memory.reaction_snapshot_theory(), 1);
        (theory, other)
    }

    fn reference_set_single_current(
        memory: &mut ReferenceMemoryInstance,
        handle: u32,
    ) {
        assert_eq!(memory.reaction_set_current_member(0, handle), 1);
        assert_eq!(memory.reaction_set_current_count(1), 1);
    }

    fn reference_run(
        memory: &mut ReferenceMemoryInstance,
    ) -> ReferenceReactionResult {
        assert_eq!(memory.reaction_run(), 1);
        reference_observe(memory)
    }

    fn reference_run_once_handles(
        memory: &mut ReferenceMemoryInstance,
        theory: &[u32],
        current: &[u32],
    ) -> BTreeSet<u32> {
        memory.reaction_reset();
        for (index, handle) in current.iter().copied().enumerate() {
            assert_eq!(
                memory.reaction_set_current_member(index as u32, handle),
                1
            );
        }
        assert_eq!(
            memory.reaction_set_current_count(current.len() as u32),
            1
        );
        for (index, relation) in theory.iter().copied().enumerate() {
            assert_eq!(
                memory.reaction_set_theory_relation(index as u32, relation),
                1
            );
        }
        assert_eq!(
            memory.reaction_set_theory_count(theory.len() as u32),
            1
        );
        assert_eq!(memory.reaction_snapshot_theory(), 1);
        assert_eq!(memory.reaction_run(), 1);

        (0..memory.reaction_current_count())
            .map(|index| memory.reaction_current_member(index))
            .collect()
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
        let mut memory = ReferenceMemoryInstance::new();
        memory.reset_pool();

        // Preserve the grounded-unary research witness on the independent
        // reference backend; this is not a second optimized executor.
        let not_0_to_1 = reference_import(&mut memory, &pair("98", "68"));
        let not_1_to_0 = reference_import(&mut memory, &pair("68", "98"));
        let k0 = reference_import(&mut memory, &pair(K, "98"));
        let k1 = reference_import(&mut memory, &pair(K, "68"));
        let theory = [not_0_to_1, not_1_to_0];

        let out0 =
            reference_run_once_handles(&mut memory, &theory, &[k0]);
        let out1 =
            reference_run_once_handles(&mut memory, &theory, &[k1]);

        assert_eq!(out0, BTreeSet::from([k1]));
        assert_eq!(out1, BTreeSet::from([k0]));
        assert_eq!(reference_export(&memory, k0), "1898");
        assert_eq!(reference_export(&memory, k1), "1868");
    }

    #[test]
    fn c1_fixed_theory_reaction_is_additive_over_independent_current_members() {
        let mut memory = ReferenceMemoryInstance::new();
        memory.reset_pool();

        let states = ["98", "68", "16898"];
        let mut current = Vec::new();
        for state in states {
            current.push(reference_import(&mut memory, &pair(K, state)));
        }

        // Pre-materialize every unary relation over the closed three-state
        // universe so missing target materialization cannot explain a result.
        let mut all_relations = Vec::new();
        for antecedent in states {
            for output in states {
                all_relations.push(reference_import(
                    &mut memory,
                    &pair(antecedent, output),
                ));
            }
        }
        assert_eq!(all_relations.len(), 9);

        let a_current = current[0];
        let b_current = current[1];

        for mask in 0_u16..(1_u16 << all_relations.len()) {
            let theory = all_relations
                .iter()
                .enumerate()
                .filter_map(|(i, relation)| {
                    ((mask >> i) & 1 == 1).then_some(*relation)
                })
                .collect::<Vec<_>>();

            let fa = reference_run_once_handles(
                &mut memory,
                &theory,
                &[a_current],
            );
            let fb = reference_run_once_handles(
                &mut memory,
                &theory,
                &[b_current],
            );
            let fab = reference_run_once_handles(
                &mut memory,
                &theory,
                &[a_current, b_current],
            );

            let union = fa.union(&fb).copied().collect::<BTreeSet<_>>();
            assert_eq!(
                fab, union,
                "fixed grounded Theory ceased to be pointwise/additive for mask {mask}"
            );
        }
    }

    #[test]
    fn c1_true_and_cannot_be_realized_by_pointwise_unary_reaction_without_prejoined_state() {
        let mut memory = ReferenceMemoryInstance::new();
        memory.reset_pool();

        let states = ["98", "68", "16898"];
        let ka = reference_import(&mut memory, &pair(K, states[0]));
        let kb = reference_import(&mut memory, &pair(K, states[1]));
        let kout = reference_import(&mut memory, &pair(K, states[2]));

        let mut all_relations = Vec::new();
        for antecedent in states {
            for output in states {
                all_relations.push(reference_import(
                    &mut memory,
                    &pair(antecedent, output),
                ));
            }
        }

        let mut realizations = 0usize;
        for mask in 0_u16..(1_u16 << all_relations.len()) {
            let theory = all_relations
                .iter()
                .enumerate()
                .filter_map(|(i, relation)| {
                    ((mask >> i) & 1 == 1).then_some(*relation)
                })
                .collect::<Vec<_>>();

            let a_only = reference_run_once_handles(
                &mut memory,
                &theory,
                &[ka],
            );
            let b_only = reference_run_once_handles(
                &mut memory,
                &theory,
                &[kb],
            );
            let both = reference_run_once_handles(
                &mut memory,
                &theory,
                &[ka, kb],
            );

            let behaves_like_and = !a_only.contains(&kout)
                && !b_only.contains(&kout)
                && both.contains(&kout);
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
        let mut reference_memory = ReferenceMemoryInstance::new();
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
                let (_, reference_fixture) = reference_prepare(&mut reference_memory, &theory, &fixture);
                reference_set_single_current(&mut reference_memory, reference_fixture[0]);
                let reference_first = reference_run(&mut reference_memory);
                assert_eq!(reference_first.scope, vec![first_successor.clone()]);
                assert_eq!(reference_first.matched_relations, 1);
                assert_eq!(reference_first.handoff, 1);
                assert!(!reference_first.quiescent);

                // The actual first result is the portable partial function under K.
                assert_eq!(reference_first.scope[0], pair(K, &expected_partial));

                reference_set_single_current(&mut reference_memory, reference_fixture[2]);
                let reference_second = reference_run(&mut reference_memory);
                assert_eq!(reference_second.scope, vec![second_successor.clone()]);
                assert_eq!(reference_second.matched_relations, 1);
                assert_eq!(reference_second.handoff, 1);
                assert!(!reference_second.quiescent);

            }
        }
    }

    #[test]
    fn p1_parallel_and_maps_one_root_originating_sequence_directly_to_value() {
        let mut reference_memory = ReferenceMemoryInstance::new();
        let theory = parallel_theory();

        for a in [BIT0, BIT1] {
            for b in [BIT0, BIT1] {
                let args = exact_sequence(&[a, b]);
                let application = call(FN_AND, &args);
                let current = pair(K, &application);
                let expected_value = and_expected(a, b);
                let successor = pair(K, expected_value);
                let fixture = vec![current.clone(), successor.clone()];

                let (_, reference_fixture) = reference_prepare(&mut reference_memory, &theory, &fixture);
                reference_set_single_current(&mut reference_memory, reference_fixture[0]);
                let reference = reference_run(&mut reference_memory);
                assert_eq!(reference.scope, vec![successor.clone()]);
                assert_eq!(reference.matched_relations, 1);
                assert_eq!(reference.handoff, 1);
                assert!(!reference.quiescent);

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
        let mut reference_memory = ReferenceMemoryInstance::new();
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

                let (_, reference_fixture) = reference_prepare(&mut reference_memory, &theory, &fixture);
                reference_set_single_current(&mut reference_memory, reference_fixture[0]);
                let reference_first = reference_run(&mut reference_memory);
                assert_eq!(reference_first.scope, vec![first_successor.clone()]);
                assert_eq!(reference_first.matched_relations, 1);
                assert_eq!(reference_first.handoff, 1);
                assert!(!reference_first.quiescent);

                reference_set_single_current(&mut reference_memory, reference_fixture[2]);
                let reference_second = reference_run(&mut reference_memory);
                assert_eq!(reference_second.scope, vec![second_successor.clone()]);
                assert_eq!(reference_second.matched_relations, 1);
                assert_eq!(reference_second.handoff, 1);
                assert!(!reference_second.quiescent);

            }
        }
    }

    #[test]
    fn c2a_parallel_half_adder_maps_argument_sequence_to_sum_carry_sequence() {
        let mut reference_memory = ReferenceMemoryInstance::new();
        let theory = parallel_half_theory();

        for a in [BIT0, BIT1] {
            for b in [BIT0, BIT1] {
                let args = exact_sequence(&[a, b]);
                let result_sequence = half_result_sequence(a, b);
                let current = pair(K, &call(FN_HALF, &args));
                let successor = pair(K, &result_sequence);
                let fixture = vec![current.clone(), successor.clone()];

                let (_, reference_fixture) = reference_prepare(&mut reference_memory, &theory, &fixture);
                reference_set_single_current(&mut reference_memory, reference_fixture[0]);
                let reference = reference_run(&mut reference_memory);

                assert_eq!(reference.scope, vec![successor.clone()]);
                assert_eq!(reference.matched_relations, 1);
                assert_eq!(reference.handoff, 1);
                assert!(!reference.quiescent);

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
