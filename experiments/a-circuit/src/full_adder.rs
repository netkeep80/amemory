use amemory_optimized_cpu_probe::{
    structural::{
        admit_structural_rule, define_structural_interpreter,
        define_structural_role_dictionary, define_structural_rule,
        index_structural_rule_trigger, materialize_exact_sequence,
        read_exact_sequence, OptimizedStructuralEngine,
    },
    Handle, OptimizedLinkStore, ROOT_HANDLE,
};

fn basis(store: &mut OptimizedLinkStore) -> (Handle, Handle, Handle, Handle) {
    let o = store.ensure_start_self_closed(ROOT_HANDLE).unwrap();
    let c = store.ensure_end_self_closed(ROOT_HANDLE).unwrap();
    let l = store.ensure_pair(o, c).unwrap();
    let u = store.ensure_pair(c, o).unwrap();
    (o, c, l, u)
}

fn fresh(
    store: &mut OptimizedLinkStore,
    o: Handle,
    c: Handle,
    u: Handle,
    l: Handle,
    count: usize,
) -> Vec<Handle> {
    let mut seed = store.ensure_pair(u, l).unwrap();
    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        seed = store
            .ensure_pair(seed, if i % 2 == 0 { o } else { c })
            .unwrap();
        result.push(seed);
    }
    result
}

fn call(
    store: &mut OptimizedLinkStore,
    apply: Handle,
    function: Handle,
    argument: Handle,
) -> Handle {
    let invocation = store.ensure_pair(function, argument).unwrap();
    store.ensure_pair(apply, invocation).unwrap()
}

fn done(
    store: &mut OptimizedLinkStore,
    done_tag: Handle,
    value: Handle,
) -> Handle {
    store.ensure_pair(done_tag, value).unwrap()
}

fn half_xor_frame(
    store: &mut OptimizedLinkStore,
    caller: Handle,
    args: Handle,
) -> Handle {
    let descriptor = store.ensure_pair(caller, args).unwrap();
    store.ensure_start_self_closed(descriptor).unwrap()
}

fn half_and_frame(
    store: &mut OptimizedLinkStore,
    caller: Handle,
    args: Handle,
    sum: Handle,
) -> Handle {
    let args_sum = store.ensure_pair(args, sum).unwrap();
    let descriptor = store.ensure_pair(caller, args_sum).unwrap();
    store.ensure_start_self_closed(descriptor).unwrap()
}

fn half_finish_frame(
    store: &mut OptimizedLinkStore,
    caller: Handle,
    sum: Handle,
) -> Handle {
    let descriptor = store.ensure_pair(caller, sum).unwrap();
    store.ensure_start_self_closed(descriptor).unwrap()
}

fn tagged_frame(
    store: &mut OptimizedLinkStore,
    tag: Handle,
    caller: Handle,
    value: Handle,
) -> Handle {
    let caller_value = store.ensure_pair(caller, value).unwrap();
    let descriptor = store.ensure_pair(tag, caller_value).unwrap();
    store.ensure_start_self_closed(descriptor).unwrap()
}

fn define_bundle_rule(
    store: &mut OptimizedLinkStore,
    theory: Handle,
    roles: &[Handle],
    before: Handle,
    after: &[Handle],
) -> (Handle, Handle) {
    let dictionary = define_structural_role_dictionary(store, roles).unwrap();
    let output_bundle = materialize_exact_sequence(store, after).unwrap();
    let body = store.ensure_pair(before, output_bundle).unwrap();
    let rule = define_structural_rule(store, dictionary, body).unwrap();
    let admission = admit_structural_rule(store, theory, rule).unwrap();
    (rule, admission)
}

fn admit_bundle_rule(
    store: &mut OptimizedLinkStore,
    theory: Handle,
    trigger_key: Handle,
    roles: &[Handle],
    before: Handle,
    after: &[Handle],
) -> Handle {
    let (rule, admission) = define_bundle_rule(store, theory, roles, before, after);
    index_structural_rule_trigger(store, trigger_key, admission).unwrap();
    rule
}

fn index_rule_for(
    store: &mut OptimizedLinkStore,
    trigger_keys: &[Handle],
    admission: Handle,
) {
    for key in trigger_keys {
        index_structural_rule_trigger(store, *key, admission).unwrap();
    }
}

struct Fixture {
    store: OptimizedLinkStore,
    engine: OptimizedStructuralEngine,
    full: Handle,
    k: Handle,
    zero: Handle,
    one: Handle,
    apply: Handle,
    args3: [[[Handle; 2]; 2]; 2],
}

impl Fixture {
    fn new() -> Self {
        let mut store = OptimizedLinkStore::new();
        let (o, c, l, u) = basis(&mut store);
        let anchors = fresh(&mut store, o, c, u, l, 320);
        let at = |i: usize| anchors[i];

        let theory = store.ensure_pair(at(0), at(1)).unwrap();
        let authority_dictionary =
            define_structural_role_dictionary(&mut store, &[]).unwrap();
        let grammar = store.ensure_pair(at(2), at(3)).unwrap();
        let interpreter = define_structural_interpreter(
            &mut store,
            authority_dictionary,
            grammar,
            theory,
        )
        .unwrap();

        // Ordinary Link identities; executor has no gate/opcode knowledge.
        let half = store.ensure_pair(at(4), at(5)).unwrap();
        let xor = store.ensure_pair(at(6), at(7)).unwrap();
        let and = store.ensure_pair(at(8), at(9)).unwrap();
        let or = store.ensure_pair(at(10), at(11)).unwrap();
        let full = store.ensure_pair(at(12), at(13)).unwrap();
        let k = store.ensure_pair(at(14), at(15)).unwrap();

        let h1_tag = store.ensure_pair(at(16), at(17)).unwrap();
        let h2_tag = store.ensure_pair(at(18), at(19)).unwrap();
        let or_tag = store.ensure_pair(at(20), at(21)).unwrap();
        let full_finish_tag = store.ensure_pair(at(22), at(23)).unwrap();
        let full_done_tag = store.ensure_pair(at(24), at(25)).unwrap();

        // Benchmark-local call/done tags use accepted structural Links.
        let apply = o;
        let done_tag = c;
        let zero = u;
        let one = l;

        let bit = [zero, one];

        let mut args2 = [[ROOT_HANDLE; 2]; 2];
        for ai in 0..2 {
            for bi in 0..2 {
                args2[ai][bi] =
                    materialize_exact_sequence(&mut store, &[bit[ai], bit[bi]]).unwrap();
            }
        }

        let mut args3 = [[[ROOT_HANDLE; 2]; 2]; 2];
        for ai in 0..2 {
            for bi in 0..2 {
                for ci in 0..2 {
                    args3[ai][bi][ci] = materialize_exact_sequence(
                        &mut store,
                        &[bit[ai], bit[bi], bit[ci]],
                    )
                    .unwrap();
                }
            }
        }

        // -----------------------------------------------------------------
        // FULL OPEN — generic decomposition of the 3-argument ExactSequence.
        //
        // K -> Call(FULL,[a,b,cin])
        //   =>
        // H1Frame(K,cin) -> Call(HALF,[a,b])
        // -----------------------------------------------------------------
        {
            let k_role = at(40);
            let a_role = at(41);
            let b_role = at(42);
            let cin_role = at(43);

            let args_template = materialize_exact_sequence(
                &mut store,
                &[a_role, b_role, cin_role],
            )
            .unwrap();
            let before_call = call(&mut store, apply, full, args_template);
            let before = store.ensure_pair(k_role, before_call).unwrap();

            let first_half_args =
                materialize_exact_sequence(&mut store, &[a_role, b_role]).unwrap();
            let h1 = tagged_frame(
                &mut store,
                h1_tag,
                k_role,
                cin_role,
            );
            let after_call = call(&mut store, apply, half, first_half_args);
            let after = store.ensure_pair(h1, after_call).unwrap();

            admit_bundle_rule(
                &mut store,
                theory,
                o,
                &[k_role, a_role, b_role, cin_role],
                before,
                &[after],
            );
        }

        // -----------------------------------------------------------------
        // Reusable gate-composed HALF from PR #67:
        // generic OPEN + XOR rows + AND rows + generic FINALIZE.
        // -----------------------------------------------------------------
        {
            let caller_role = at(50);
            let args_role = at(51);
            let before_call = call(&mut store, apply, half, args_role);
            let before = store.ensure_pair(caller_role, before_call).unwrap();
            let frame = half_xor_frame(&mut store, caller_role, args_role);
            let after_call = call(&mut store, apply, xor, args_role);
            let after = store.ensure_pair(frame, after_call).unwrap();

            admit_bundle_rule(
                &mut store,
                theory,
                o,
                &[caller_role, args_role],
                before,
                &[after],
            );
        }

        let xor_rows = [
            (0usize, 0usize, 0usize),
            (0, 1, 1),
            (1, 0, 1),
            (1, 1, 0),
        ];

        let mut role_seed = 60usize;
        for (ai, bi, si) in xor_rows {
            let argseq = args2[ai][bi];
            let sum = bit[si];
            let caller_role = at(role_seed);
            role_seed += 1;

            let frame = half_xor_frame(&mut store, caller_role, argseq);
            let before_call = call(&mut store, apply, xor, argseq);
            let before = store.ensure_pair(frame, before_call).unwrap();

            let next_frame =
                half_and_frame(&mut store, caller_role, argseq, sum);
            let after_call = call(&mut store, apply, and, argseq);
            let after = store.ensure_pair(next_frame, after_call).unwrap();

            admit_bundle_rule(
                &mut store,
                theory,
                o,
                &[caller_role],
                before,
                &[after],
            );
        }

        let and_rows = [
            (0usize, 0usize, 0usize),
            (0, 1, 0),
            (1, 0, 0),
            (1, 1, 1),
        ];

        for (ai, bi, ci) in and_rows {
            let argseq = args2[ai][bi];
            let carry = bit[ci];
            let caller_role = at(role_seed);
            let sum_role = at(role_seed + 1);
            role_seed += 2;

            let frame =
                half_and_frame(&mut store, caller_role, argseq, sum_role);
            let before_call = call(&mut store, apply, and, argseq);
            let before = store.ensure_pair(frame, before_call).unwrap();

            let finish =
                half_finish_frame(&mut store, caller_role, sum_role);
            let done_carry = done(&mut store, done_tag, carry);
            let after = store.ensure_pair(finish, done_carry).unwrap();

            admit_bundle_rule(
                &mut store,
                theory,
                o,
                &[caller_role, sum_role],
                before,
                &[after],
            );
        }

        {
            let caller_role = at(90);
            let sum_role = at(91);
            let carry_role = at(92);

            let finish =
                half_finish_frame(&mut store, caller_role, sum_role);
            let done_carry = done(&mut store, done_tag, carry_role);
            let before = store.ensure_pair(finish, done_carry).unwrap();

            let result =
                materialize_exact_sequence(&mut store, &[sum_role, carry_role])
                    .unwrap();
            let after = store.ensure_pair(caller_role, result).unwrap();

            admit_bundle_rule(
                &mut store,
                theory,
                c,
                &[caller_role, sum_role, carry_role],
                before,
                &[after],
            );
        }

        // Possible Half Adder results. Continuation rules are generic but the
        // accepted trigger projection must be indexed under each possible
        // ROOT-originating result-sequence trigger.
        let half_outputs = [
            materialize_exact_sequence(&mut store, &[zero, zero]).unwrap(),
            materialize_exact_sequence(&mut store, &[one, zero]).unwrap(),
            materialize_exact_sequence(&mut store, &[zero, one]).unwrap(),
        ];

        // -----------------------------------------------------------------
        // H1 RETURN:
        // H1Frame(K,cin) -> [s1,c1]
        //   =>
        // H2Frame(K,c1) -> Call(HALF,[s1,cin])
        // -----------------------------------------------------------------
        {
            let k_role = at(100);
            let cin_role = at(101);
            let s1_role = at(102);
            let c1_role = at(103);

            let h1 =
                tagged_frame(&mut store, h1_tag, k_role, cin_role);
            let half_result =
                materialize_exact_sequence(&mut store, &[s1_role, c1_role])
                    .unwrap();
            let before = store.ensure_pair(h1, half_result).unwrap();

            let h2 =
                tagged_frame(&mut store, h2_tag, k_role, c1_role);
            let second_args =
                materialize_exact_sequence(&mut store, &[s1_role, cin_role])
                    .unwrap();
            let second_call = call(&mut store, apply, half, second_args);
            let after = store.ensure_pair(h2, second_call).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut store,
                theory,
                &[k_role, cin_role, s1_role, c1_role],
                before,
                &[after],
            );
            index_rule_for(&mut store, &half_outputs, admission);
        }

        // -----------------------------------------------------------------
        // H2 RETURN:
        // H2Frame(K,c1) -> [sum,c2]
        //   =>
        // OrFrame(K,sum) -> Call(OR,[c1,c2])
        // -----------------------------------------------------------------
        {
            let k_role = at(110);
            let c1_role = at(111);
            let sum_role = at(112);
            let c2_role = at(113);

            let h2 =
                tagged_frame(&mut store, h2_tag, k_role, c1_role);
            let half_result =
                materialize_exact_sequence(&mut store, &[sum_role, c2_role])
                    .unwrap();
            let before = store.ensure_pair(h2, half_result).unwrap();

            let or_frame =
                tagged_frame(&mut store, or_tag, k_role, sum_role);
            let or_args =
                materialize_exact_sequence(&mut store, &[c1_role, c2_role])
                    .unwrap();
            let or_call = call(&mut store, apply, or, or_args);
            let after = store.ensure_pair(or_frame, or_call).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut store,
                theory,
                &[k_role, c1_role, sum_role, c2_role],
                before,
                &[after],
            );
            index_rule_for(&mut store, &half_outputs, admission);
        }

        // Standard OR gate rows only.
        let or_rows = [
            (0usize, 0usize, 0usize),
            (0, 1, 1),
            (1, 0, 1),
            (1, 1, 1),
        ];

        for (c1i, c2i, outi) in or_rows {
            let argseq = args2[c1i][c2i];
            let cout = bit[outi];
            let k_role = at(role_seed);
            let sum_role = at(role_seed + 1);
            role_seed += 2;

            let frame =
                tagged_frame(&mut store, or_tag, k_role, sum_role);
            let before_call = call(&mut store, apply, or, argseq);
            let before = store.ensure_pair(frame, before_call).unwrap();

            let finish = tagged_frame(
                &mut store,
                full_finish_tag,
                k_role,
                sum_role,
            );
            // Full Adder return uses a dedicated structural namespace.
            // Reusing C⟼cout here would also match the generic Half Adder
            // FINALIZE rule, which is correctly broad enough for any caller.
            let done_cout = done(&mut store, full_done_tag, cout);
            let after = store.ensure_pair(finish, done_cout).unwrap();

            admit_bundle_rule(
                &mut store,
                theory,
                o,
                &[k_role, sum_role],
                before,
                &[after],
            );
        }

        // Generic FULL FINALIZE.
        {
            let k_role = at(140);
            let sum_role = at(141);
            let cout_role = at(142);

            let finish = tagged_frame(
                &mut store,
                full_finish_tag,
                k_role,
                sum_role,
            );
            let done_cout = done(&mut store, full_done_tag, cout_role);
            let before = store.ensure_pair(finish, done_cout).unwrap();

            let result =
                materialize_exact_sequence(&mut store, &[sum_role, cout_role])
                    .unwrap();
            let after = store.ensure_pair(k_role, result).unwrap();

            admit_bundle_rule(
                &mut store,
                theory,
                full_done_tag,
                &[k_role, sum_role, cout_role],
                before,
                &[after],
            );
        }

        let mut engine = OptimizedStructuralEngine::new(32);
        engine.set_interpreter(&store, interpreter).unwrap();

        Self {
            store,
            engine,
            full,
            k,
            zero,
            one,
            apply,
            args3,
        }
    }

    fn run_case(
        &mut self,
        ai: usize,
        bi: usize,
        ci: usize,
    ) -> (Handle, Handle, Vec<u32>, Vec<u32>) {
        let args = self.args3[ai][bi][ci];
        let invocation =
            call(&mut self.store, self.apply, self.full, args);
        let initial = self.store.ensure_pair(self.k, invocation).unwrap();

        self.engine.set_current(&self.store, &[initial]).unwrap();

        let mut matches = Vec::new();
        let mut handoffs = Vec::new();

        for step in 0..13 {
            let reaction = self.engine.run(&mut self.store).unwrap();
            assert!(
                !reaction.quiescent,
                "input {ai}{bi}{ci}: step {step} unexpectedly quiescent"
            );
            assert_eq!(
                reaction.raw_rule_matches, 1,
                "input {ai}{bi}{ci}: step {step} match count"
            );
            assert_eq!(
                reaction.transitioned_members, 1,
                "input {ai}{bi}{ci}: step {step} transitioned count"
            );
            assert_eq!(
                reaction.handoff_count, 1,
                "input {ai}{bi}{ci}: step {step} handoff"
            );
            assert_eq!(
                reaction.next_members.len(),
                1,
                "input {ai}{bi}{ci}: step {step} Scope cardinality"
            );
            matches.push(reaction.raw_rule_matches);
            handoffs.push(reaction.handoff_count);
        }

        let stable_bank = self.engine.current_bank();
        let quiescent = self.engine.run(&mut self.store).unwrap();
        assert!(quiescent.quiescent, "final result must be quiescent");
        assert_eq!(quiescent.raw_rule_matches, 0);
        assert_eq!(quiescent.handoff_count, 0);
        assert_eq!(self.engine.current_bank(), stable_bank);
        assert_eq!(self.engine.current().len(), 1);

        let final_link = self.engine.current()[0];
        let (caller, result_sequence) = self.store.poles(final_link).unwrap();
        assert_eq!(caller, self.k, "final caller/context");
        let decoded =
            read_exact_sequence(&self.store, result_sequence).unwrap();
        assert_eq!(decoded.len(), 2);

        (decoded[0], decoded[1], matches, handoffs)
    }
}

#[test]
fn m2_gate_composed_full_adder_all_eight_rows() {
    let mut f = Fixture::new();

    for ai in 0..2 {
        for bi in 0..2 {
            for ci in 0..2 {
                let (sum, cout, matches, handoffs) =
                    f.run_case(ai, bi, ci);

                let total = ai + bi + ci;
                let expected_sum =
                    if total % 2 == 1 { f.one } else { f.zero };
                let expected_cout =
                    if total >= 2 { f.one } else { f.zero };

                assert_eq!(
                    sum, expected_sum,
                    "input {ai}{bi}{ci}: Sum"
                );
                assert_eq!(
                    cout, expected_cout,
                    "input {ai}{bi}{ci}: Cout"
                );
                assert_eq!(matches, vec![1; 13]);
                assert_eq!(handoffs, vec![1; 13]);

                println!(
                    "FULL_ADDER_TRACE input={}{}{} sum={} cout={} active_steps=13",
                    ai,
                    bi,
                    ci,
                    usize::from(sum == f.one),
                    usize::from(cout == f.one),
                );
            }
        }
    }
}
