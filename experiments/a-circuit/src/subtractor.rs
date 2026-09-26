use super::full_adder::{
    call, define_bundle_rule, index_rule_for, Fixture as FullFixture,
};
use amemory_optimized_cpu_probe::{
    structural::{materialize_exact_sequence, read_exact_sequence},
    Handle, OptimizedLinkStore,
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
        seed: Handle,
        o: Handle,
        c: Handle,
    ) -> Self {
        // Keep SUB1's structural namespace disjoint from Ripple(N).
        let current = store.ensure_pair(seed, c).unwrap();
        Self {
            current,
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

fn stage_frame(
    store: &mut OptimizedLinkStore,
    tag: Handle,
    values: &[Handle],
) -> Handle {
    let payload = materialize_exact_sequence(store, values).unwrap();
    let descriptor = store.ensure_pair(tag, payload).unwrap();
    store.ensure_start_self_closed(descriptor).unwrap()
}

#[derive(Clone, Debug)]
pub(crate) struct Sub1Program {
    pub(crate) sub: Handle,
    pub(crate) active_steps: usize,
}

impl Sub1Program {
    pub(crate) fn install(f: &mut FullFixture) -> Self {
        let seed = f.store.ensure_pair(f.full, f.k).unwrap();
        let mut anchors = AnchorGen::new(
            &mut f.store,
            seed,
            f.o,
            f.c,
        );

        let not_left = anchors.next(&mut f.store);
        let not_right = anchors.next(&mut f.store);
        let not = f.store.ensure_pair(not_left, not_right).unwrap();

        let sub_left = anchors.next(&mut f.store);
        let sub_right = anchors.next(&mut f.store);
        let sub = f.store.ensure_pair(sub_left, sub_right).unwrap();

        let b_not_tag = anchors.next(&mut f.store);
        let bin_not_tag = anchors.next(&mut f.store);
        let full_tag = anchors.next(&mut f.store);
        let cout_not_tag = anchors.next(&mut f.store);

        // A unary function result is carried as a one-position
        // ROOT-originating ExactSequence. Returning a naked U/L bit is unsafe
        // in the mixed Theory because its topology can also satisfy unrelated
        // generic templates (for example C->role).
        let not_outputs = [
            materialize_exact_sequence(&mut f.store, &[f.zero]).unwrap(),
            materialize_exact_sequence(&mut f.store, &[f.one]).unwrap(),
        ];

        // Reusable structural NOT function:
        //
        // caller -> Call(NOT,0) => caller -> 1
        // caller -> Call(NOT,1) => caller -> 0
        //
        // Runtime inversion is therefore a Structural Rule transition, not
        // host preprocessing.
        for (input, output) in [(f.zero, f.one), (f.one, f.zero)] {
            let caller = anchors.next(&mut f.store);
            let before_call = call(&mut f.store, f.apply, not, input);
            let before = f.store.ensure_pair(caller, before_call).unwrap();
            let output_index = if output == f.zero { 0 } else { 1 };
            let after = f
                .store
                .ensure_pair(caller, not_outputs[output_index])
                .unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[caller],
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        // SUB1 OPEN:
        //
        // K -> Call(SUB1,[a,b,bin])
        // =>
        // BNotFrame([K,a,bin]) -> Call(NOT,b)
        {
            let k = anchors.next(&mut f.store);
            let a = anchors.next(&mut f.store);
            let b = anchors.next(&mut f.store);
            let bin = anchors.next(&mut f.store);

            let args =
                materialize_exact_sequence(&mut f.store, &[a, b, bin]).unwrap();
            let before_call = call(&mut f.store, f.apply, sub, args);
            let before = f.store.ensure_pair(k, before_call).unwrap();

            let caller =
                stage_frame(&mut f.store, b_not_tag, &[k, a, bin]);
            let after_call = call(&mut f.store, f.apply, not, b);
            let after = f.store.ensure_pair(caller, after_call).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, a, b, bin],
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        // B inversion return:
        //
        // BNotFrame([K,a,bin]) -> ExactSequence_R([not_b])
        // =>
        // BinNotFrame([K,a,not_b]) -> Call(NOT,bin)
        {
            let k = anchors.next(&mut f.store);
            let a = anchors.next(&mut f.store);
            let bin = anchors.next(&mut f.store);
            let not_b = anchors.next(&mut f.store);

            let caller =
                stage_frame(&mut f.store, b_not_tag, &[k, a, bin]);
            let not_result =
                materialize_exact_sequence(&mut f.store, &[not_b]).unwrap();
            let before = f.store.ensure_pair(caller, not_result).unwrap();

            let next_caller =
                stage_frame(&mut f.store, bin_not_tag, &[k, a, not_b]);
            let next_call = call(&mut f.store, f.apply, not, bin);
            let after = f.store.ensure_pair(next_caller, next_call).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, a, bin, not_b],
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &not_outputs, admission);
        }

        // Borrow-in inversion return:
        //
        // BinNotFrame([K,a,not_b]) -> ExactSequence_R([not_bin])
        // =>
        // FullFrame([K]) -> Call(FULL,[a,not_b,not_bin])
        {
            let k = anchors.next(&mut f.store);
            let a = anchors.next(&mut f.store);
            let not_b = anchors.next(&mut f.store);
            let not_bin = anchors.next(&mut f.store);

            let caller =
                stage_frame(&mut f.store, bin_not_tag, &[k, a, not_b]);
            let not_result =
                materialize_exact_sequence(&mut f.store, &[not_bin]).unwrap();
            let before = f.store.ensure_pair(caller, not_result).unwrap();

            let next_caller = stage_frame(&mut f.store, full_tag, &[k]);
            let full_args = materialize_exact_sequence(
                &mut f.store,
                &[a, not_b, not_bin],
            )
            .unwrap();
            let full_call = call(&mut f.store, f.apply, f.full, full_args);
            let after = f.store.ensure_pair(next_caller, full_call).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, a, not_b, not_bin],
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &not_outputs, admission);
        }

        // Full Adder returns [diff,carry_out]. For subtraction, x86-style
        // unsigned borrow is NOT(carry_out).
        let full_outputs = [
            materialize_exact_sequence(&mut f.store, &[f.zero, f.zero]).unwrap(),
            materialize_exact_sequence(&mut f.store, &[f.one, f.zero]).unwrap(),
            materialize_exact_sequence(&mut f.store, &[f.zero, f.one]).unwrap(),
            materialize_exact_sequence(&mut f.store, &[f.one, f.one]).unwrap(),
        ];

        {
            let k = anchors.next(&mut f.store);
            let diff = anchors.next(&mut f.store);
            let carry_out = anchors.next(&mut f.store);

            let caller = stage_frame(&mut f.store, full_tag, &[k]);
            let full_result =
                materialize_exact_sequence(&mut f.store, &[diff, carry_out])
                    .unwrap();
            let before = f.store.ensure_pair(caller, full_result).unwrap();

            let next_caller =
                stage_frame(&mut f.store, cout_not_tag, &[k, diff]);
            let not_call = call(&mut f.store, f.apply, not, carry_out);
            let after = f.store.ensure_pair(next_caller, not_call).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, diff, carry_out],
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &full_outputs, admission);
        }

        // Final borrow return:
        //
        // CoutNotFrame([K,diff]) -> ExactSequence_R([borrow])
        // =>
        // K -> ExactSequence_R([diff,borrow])
        {
            let k = anchors.next(&mut f.store);
            let diff = anchors.next(&mut f.store);
            let borrow = anchors.next(&mut f.store);

            let caller =
                stage_frame(&mut f.store, cout_not_tag, &[k, diff]);
            let not_result =
                materialize_exact_sequence(&mut f.store, &[borrow]).unwrap();
            let before = f.store.ensure_pair(caller, not_result).unwrap();

            let result =
                materialize_exact_sequence(&mut f.store, &[diff, borrow])
                    .unwrap();
            let after = f.store.ensure_pair(k, result).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, diff, borrow],
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &not_outputs, admission);
        }

        Self {
            sub,
            // OPEN
            // + NOT(B) + continuation
            // + NOT(Bin) + continuation
            // + 13-step reusable Full Adder
            // + continuation + NOT(CarryOut) + final continuation
            active_steps: 21,
        }
    }
}

fn bit_value(f: &FullFixture, value: Handle) -> u8 {
    if value == f.one {
        1
    } else {
        assert_eq!(value, f.zero, "SUB1 result must be canonical 0/1");
        0
    }
}

fn run_case(
    f: &mut FullFixture,
    program: &Sub1Program,
    a: u8,
    b: u8,
    bin: u8,
) -> (u8, u8, Handle) {
    assert!(a <= 1 && b <= 1 && bin <= 1);

    let bits = [f.zero, f.one];
    let args = materialize_exact_sequence(
        &mut f.store,
        &[bits[a as usize], bits[b as usize], bits[bin as usize]],
    )
    .unwrap();
    let invocation = call(&mut f.store, f.apply, program.sub, args);
    let initial = f.store.ensure_pair(f.k, invocation).unwrap();

    f.engine.set_current(&f.store, &[initial]).unwrap();

    for step in 0..program.active_steps {
        let reaction = f.engine.run(&mut f.store).unwrap();
        assert!(
            !reaction.quiescent,
            "SUB1 a={a} b={b} bin={bin}: unexpected quiescence at step {step}",
        );
        assert_eq!(reaction.raw_rule_matches, 1, "step {step} matches");
        assert_eq!(reaction.transitioned_members, 1, "step {step} transitioned");
        assert_eq!(reaction.handoff_count, 1, "step {step} handoff");
        assert_eq!(reaction.next_members.len(), 1, "step {step} Scope");
    }

    let stable = f.engine.current_bank();
    let quiescent = f.engine.run(&mut f.store).unwrap();
    assert!(quiescent.quiescent);
    assert_eq!(quiescent.raw_rule_matches, 0);
    assert_eq!(quiescent.handoff_count, 0);
    assert_eq!(f.engine.current_bank(), stable);
    assert_eq!(f.engine.current().len(), 1);

    let final_link = f.engine.current()[0];
    let (caller, result) = f.store.poles(final_link).unwrap();
    assert_eq!(caller, f.k);

    let values = read_exact_sequence(&f.store, result).unwrap();
    assert_eq!(values.len(), 2, "SUB1 result arity");
    let diff = bit_value(f, values[0]);
    let borrow = bit_value(f, values[1]);

    (diff, borrow, result)
}

fn expected(a: u8, b: u8, bin: u8) -> (u8, u8) {
    let raw = i16::from(a) - i16::from(b) - i16::from(bin);
    (raw.rem_euclid(2) as u8, u8::from(raw < 0))
}

#[test]
fn m3_sub1_shared_adder_exhaustive_truth_table() {
    let mut f = FullFixture::new();
    let program = Sub1Program::install(&mut f);
    assert_eq!(program.active_steps, 21);

    let mut cases = 0usize;
    for a in 0u8..=1 {
        for b in 0u8..=1 {
            for bin in 0u8..=1 {
                let (diff, borrow, _) =
                    run_case(&mut f, &program, a, b, bin);
                assert_eq!(
                    (diff, borrow),
                    expected(a, b, bin),
                    "SUB1 a={a} b={b} bin={bin}"
                );
                cases += 1;
            }
        }
    }

    assert_eq!(cases, 8);
}

#[test]
fn m3_sub1_result_order_and_steady_state_are_structural() {
    let mut f = FullFixture::new();
    let program = Sub1Program::install(&mut f);

    // 1 - 0 - 0 => Diff=1, BorrowOut=0 gives an asymmetric result,
    // so reversing positions must produce a distinct ExactSequence.
    let (diff, borrow, result) =
        run_case(&mut f, &program, 1, 0, 0);
    assert_eq!((diff, borrow), (1, 0));

    let values = read_exact_sequence(&f.store, result).unwrap();
    assert_eq!(values, vec![f.one, f.zero]);

    let reversed =
        materialize_exact_sequence(&mut f.store, &[f.zero, f.one]).unwrap();
    assert_ne!(result, reversed, "[Diff,BorrowOut] order is semantic");

    // Once this exact computation has materialized every reachable Link,
    // an identical execution must not grow semantic state.
    let after_first = f.store.link_count();
    let second = run_case(&mut f, &program, 1, 0, 0);
    assert_eq!((second.0, second.1), (1, 0));
    assert_eq!(
        f.store.link_count(),
        after_first,
        "repeated identical SUB1 materialized new Links"
    );
}
