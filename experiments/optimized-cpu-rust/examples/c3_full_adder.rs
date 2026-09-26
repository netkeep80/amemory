use amemory_optimized_cpu_probe::{Handle, OptimizedLinkStore, ROOT_HANDLE};
use amemory_optimized_cpu_probe::structural::{
    OptimizedStructuralEngine,
    admit_structural_rule,
    define_structural_interpreter,
    define_structural_role_dictionary,
    define_structural_rule,
    index_structural_rule_trigger,
    materialize_exact_sequence,
    read_exact_sequence,
};

#[derive(Clone, Copy)]
struct Basis {
    o: Handle,
    c: Handle,
    l: Handle,
    u: Handle,
}

fn basis(store: &mut OptimizedLinkStore) -> Basis {
    let o = store.ensure_start_self_closed(ROOT_HANDLE).unwrap();
    let c = store.ensure_end_self_closed(ROOT_HANDLE).unwrap();
    let l = store.ensure_pair(o, c).unwrap();
    let u = store.ensure_pair(c, o).unwrap();
    Basis { o, c, l, u }
}

fn call(store: &mut OptimizedLinkStore, b: Basis, function: Handle, arg: Handle) -> Handle {
    let application = store.ensure_pair(function, arg).unwrap();
    store.ensure_pair(b.o, application).unwrap()
}

fn done(store: &mut OptimizedLinkStore, b: Basis, value: Handle) -> Handle {
    store.ensure_pair(b.c, value).unwrap()
}

fn frame(
    store: &mut OptimizedLinkStore,
    tag: Handle,
    values: &[Handle],
) -> Handle {
    let payload_values = materialize_exact_sequence(store, values).unwrap();
    let payload = store.ensure_pair(tag, payload_values).unwrap();
    store.ensure_start_self_closed(payload).unwrap()
}

fn admit_bundle_rule(
    store: &mut OptimizedLinkStore,
    theory: Handle,
    trigger_key: Handle,
    roles: &[Handle],
    before: Handle,
    after: &[Handle],
) {
    let dictionary = define_structural_role_dictionary(store, roles).unwrap();
    let bundle = materialize_exact_sequence(store, after).unwrap();
    let body = store.ensure_pair(before, bundle).unwrap();
    let rule = define_structural_rule(store, dictionary, body).unwrap();
    let admission = admit_structural_rule(store, theory, rule).unwrap();
    index_structural_rule_trigger(store, trigger_key, admission).unwrap();
}

fn fresh_anchors(store: &mut OptimizedLinkStore, b: Basis, count: usize) -> Vec<Handle> {
    let mut seed = store.ensure_pair(b.u, b.l).unwrap();
    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        seed = store
            .ensure_pair(seed, if i % 2 == 0 { b.o } else { b.c })
            .unwrap();
        result.push(seed);
    }
    result
}

fn exact2(store: &mut OptimizedLinkStore, a: Handle, b: Handle) -> Handle {
    materialize_exact_sequence(store, &[a, b]).unwrap()
}

fn exact3(
    store: &mut OptimizedLinkStore,
    a: Handle,
    b: Handle,
    c: Handle,
) -> Handle {
    materialize_exact_sequence(store, &[a, b, c]).unwrap()
}

fn define_binary_gate_rows(
    store: &mut OptimizedLinkStore,
    theory: Handle,
    b: Basis,
    function: Handle,
    caller_role: Handle,
    rows: &[(Handle, Handle, Handle)],
) {
    for &(left, right, output) in rows {
        let args = exact2(store, left, right);
        let gate_call = call(store, b, function, args);
        let before = store.ensure_pair(caller_role, gate_call).unwrap();
        let gate_done = done(store, b, output);
        let after = store.ensure_pair(caller_role, gate_done).unwrap();

        admit_bundle_rule(
            store,
            theory,
            b.o,
            &[caller_role],
            before,
            &[after],
        );
    }
}

struct Fixture {
    store: OptimizedLinkStore,
    b: Basis,
    interpreter: Handle,
    full: Handle,
    k: Handle,
    zero: Handle,
    one: Handle,
}

fn build_fixture() -> Fixture {
    let mut store = OptimizedLinkStore::new();
    let b = basis(&mut store);
    let fresh = fresh_anchors(&mut store, b, 420);
    let at = |index: usize| fresh[index];

    let theory = store.ensure_pair(at(0), at(1)).unwrap();
    let authority_dictionary = define_structural_role_dictionary(&mut store, &[]).unwrap();
    let grammar = store.ensure_pair(at(2), at(3)).unwrap();
    let interpreter =
        define_structural_interpreter(&mut store, authority_dictionary, grammar, theory).unwrap();

    let full = store.ensure_pair(at(4), at(5)).unwrap();
    let xor = store.ensure_pair(at(6), at(7)).unwrap();
    let and = store.ensure_pair(at(8), at(9)).unwrap();
    let or = store.ensure_pair(at(10), at(11)).unwrap();
    let k = store.ensure_pair(at(12), at(13)).unwrap();

    let stage1 = store.ensure_pair(at(14), at(15)).unwrap();
    let stage2 = store.ensure_pair(at(16), at(17)).unwrap();
    let stage3 = store.ensure_pair(at(18), at(19)).unwrap();
    let stage4 = store.ensure_pair(at(20), at(21)).unwrap();
    let stage5 = store.ensure_pair(at(22), at(23)).unwrap();

    let zero = b.u;
    let one = b.l;

    // Placeholder Links are chosen after every grounded fixture anchor, so no
    // grounded constant structurally contains one of these role identities.
    let rk = at(300);
    let ra = at(301);
    let rb = at(302);
    let rcin = at(303);
    let rs1 = at(304);
    let rc1 = at(305);
    let rsum = at(306);
    let rc2 = at(307);
    let rcout = at(308);
    let rcaller = at(309);

    let xor_rows = [
        (zero, zero, zero),
        (zero, one, one),
        (one, zero, one),
        (one, one, zero),
    ];
    let and_rows = [
        (zero, zero, zero),
        (zero, one, zero),
        (one, zero, zero),
        (one, one, one),
    ];
    let or_rows = [
        (zero, zero, zero),
        (zero, one, one),
        (one, zero, one),
        (one, one, one),
    ];

    // Pure reusable gate definitions. They know nothing about Full Adder stages.
    define_binary_gate_rows(&mut store, theory, b, xor, rcaller, &xor_rows);
    define_binary_gate_rows(&mut store, theory, b, and, rcaller, &and_rows);
    define_binary_gate_rows(&mut store, theory, b, or, rcaller, &or_rows);

    // Generic FULL open:
    // K -> FULL([a,b,cin])
    //   => Stage1([K,a,b,cin]) -> XOR([a,b])
    {
        let input_args = exact3(&mut store, ra, rb, rcin);
        let full_call = call(&mut store, b, full, input_args);
        let before = store.ensure_pair(rk, full_call).unwrap();

        let stage = frame(&mut store, stage1, &[rk, ra, rb, rcin]);
        let xor_args = exact2(&mut store, ra, rb);
        let xor_call = call(&mut store, b, xor, xor_args);
        let after = store.ensure_pair(stage, xor_call).unwrap();

        admit_bundle_rule(
            &mut store,
            theory,
            b.o,
            &[rk, ra, rb, rcin],
            before,
            &[after],
        );
    }

    // Resume after XOR(a,b): capture s1, then compute c1 = AND(a,b).
    {
        let stage = frame(&mut store, stage1, &[rk, ra, rb, rcin]);
        let s1_done = done(&mut store, b, rs1);
        let before = store.ensure_pair(stage, s1_done).unwrap();

        let next_stage = frame(&mut store, stage2, &[rk, ra, rb, rcin, rs1]);
        let args = exact2(&mut store, ra, rb);
        let and_call = call(&mut store, b, and, args);
        let after = store.ensure_pair(next_stage, and_call).unwrap();

        admit_bundle_rule(
            &mut store,
            theory,
            b.c,
            &[rk, ra, rb, rcin, rs1],
            before,
            &[after],
        );
    }

    // Resume after AND(a,b): capture c1, then Sum = XOR(s1,cin).
    {
        let stage = frame(&mut store, stage2, &[rk, ra, rb, rcin, rs1]);
        let c1_done = done(&mut store, b, rc1);
        let before = store.ensure_pair(stage, c1_done).unwrap();

        let next_stage = frame(&mut store, stage3, &[rk, rcin, rs1, rc1]);
        let args = exact2(&mut store, rs1, rcin);
        let xor_call = call(&mut store, b, xor, args);
        let after = store.ensure_pair(next_stage, xor_call).unwrap();

        admit_bundle_rule(
            &mut store,
            theory,
            b.c,
            &[rk, ra, rb, rcin, rs1, rc1],
            before,
            &[after],
        );
    }

    // Resume after XOR(s1,cin): capture Sum, then c2 = AND(s1,cin).
    {
        let stage = frame(&mut store, stage3, &[rk, rcin, rs1, rc1]);
        let sum_done = done(&mut store, b, rsum);
        let before = store.ensure_pair(stage, sum_done).unwrap();

        let next_stage = frame(
            &mut store,
            stage4,
            &[rk, rcin, rs1, rc1, rsum],
        );
        let args = exact2(&mut store, rs1, rcin);
        let and_call = call(&mut store, b, and, args);
        let after = store.ensure_pair(next_stage, and_call).unwrap();

        admit_bundle_rule(
            &mut store,
            theory,
            b.c,
            &[rk, rcin, rs1, rc1, rsum],
            before,
            &[after],
        );
    }

    // Resume after AND(s1,cin): capture c2, then Cout = OR(c1,c2).
    {
        let stage = frame(
            &mut store,
            stage4,
            &[rk, rcin, rs1, rc1, rsum],
        );
        let c2_done = done(&mut store, b, rc2);
        let before = store.ensure_pair(stage, c2_done).unwrap();

        let next_stage = frame(&mut store, stage5, &[rk, rsum, rc1, rc2]);
        let args = exact2(&mut store, rc1, rc2);
        let or_call = call(&mut store, b, or, args);
        let after = store.ensure_pair(next_stage, or_call).unwrap();

        admit_bundle_rule(
            &mut store,
            theory,
            b.c,
            &[rk, rcin, rs1, rc1, rsum, rc2],
            before,
            &[after],
        );
    }

    // Generic finalizer after OR(c1,c2): return [Sum,Cout].
    {
        let stage = frame(&mut store, stage5, &[rk, rsum, rc1, rc2]);
        let cout_done = done(&mut store, b, rcout);
        let before = store.ensure_pair(stage, cout_done).unwrap();

        let result = exact2(&mut store, rsum, rcout);
        let after = store.ensure_pair(rk, result).unwrap();

        admit_bundle_rule(
            &mut store,
            theory,
            b.c,
            &[rk, rsum, rc1, rc2, rcout],
            before,
            &[after],
        );
    }

    Fixture {
        store,
        b,
        interpreter,
        full,
        k,
        zero,
        one,
    }
}

#[derive(Debug)]
struct Row {
    input: String,
    output: String,
    matches: Vec<u32>,
    handoffs: Vec<u32>,
    quiescent: bool,
}

fn bit(value: bool, zero: Handle, one: Handle) -> Handle {
    if value { one } else { zero }
}

fn run_case(f: &mut Fixture, a: bool, b_value: bool, cin: bool) -> Row {
    let a_link = bit(a, f.zero, f.one);
    let b_link = bit(b_value, f.zero, f.one);
    let cin_link = bit(cin, f.zero, f.one);

    let input_args = exact3(&mut f.store, a_link, b_link, cin_link);
    let full_call = call(&mut f.store, f.b, f.full, input_args);
    let initial = f.store.ensure_pair(f.k, full_call).unwrap();

    let mut engine = OptimizedStructuralEngine::new(32);
    engine.set_interpreter(&f.store, f.interpreter).unwrap();
    engine.set_current(&f.store, &[initial]).unwrap();

    let mut matches = Vec::new();
    let mut handoffs = Vec::new();

    for step in 0..11 {
        let reaction = engine.run(&mut f.store).unwrap();
        assert!(!reaction.quiescent, "step {step} unexpectedly quiescent");
        assert_eq!(reaction.raw_rule_matches, 1, "step {step} matches");
        assert_eq!(reaction.transitioned_members, 1, "step {step} transitioned");
        assert_eq!(reaction.handoff_count, 1, "step {step} handoff");
        matches.push(reaction.raw_rule_matches);
        handoffs.push(reaction.handoff_count);
    }

    let stable_bank = engine.current_bank();
    let quiescent = engine.run(&mut f.store).unwrap();
    assert!(quiescent.quiescent);
    assert_eq!(quiescent.raw_rule_matches, 0);
    assert_eq!(quiescent.handoff_count, 0);
    assert_eq!(engine.current_bank(), stable_bank);
    assert_eq!(engine.current().len(), 1);

    let final_member = engine.current()[0];
    let (caller, result_sequence) = f.store.poles(final_member).unwrap();
    assert_eq!(caller, f.k);

    let result = read_exact_sequence(&f.store, result_sequence).unwrap();
    assert_eq!(result.len(), 2);

    let sum = result[0] == f.one;
    let cout = result[1] == f.one;

    let expected_sum = a ^ b_value ^ cin;
    let expected_cout = (a && b_value) || (a && cin) || (b_value && cin);

    assert_eq!(sum, expected_sum);
    assert_eq!(cout, expected_cout);

    Row {
        input: format!(
            "{}{}{}",
            if a { 1 } else { 0 },
            if b_value { 1 } else { 0 },
            if cin { 1 } else { 0 }
        ),
        output: format!(
            "{}{}",
            if sum { 1 } else { 0 },
            if cout { 1 } else { 0 }
        ),
        matches,
        handoffs,
        quiescent: quiescent.quiescent,
    }
}

fn vec_json(values: &[u32]) -> String {
    values.iter().map(u32::to_string).collect::<Vec<_>>().join(",")
}

fn main() {
    let mut fixture = build_fixture();
    let mut rows = Vec::new();

    for a in [false, true] {
        for b in [false, true] {
            for cin in [false, true] {
                rows.push(run_case(&mut fixture, a, b, cin));
            }
        }
    }

    let rows_json = rows
        .iter()
        .map(|row| {
            format!(
                "{{\"input\":\"{}\",\"output\":\"{}\",\"matches\":[{}],\"handoffs\":[{}],\"quiescent\":{}}}",
                row.input,
                row.output,
                vec_json(&row.matches),
                vec_json(&row.handoffs),
                if row.quiescent { "true" } else { "false" },
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    println!(
        "C3_RESULT_JSON={{\"rows\":[{}],\"activeReactionsPerCase\":11}}",
        rows_json
    );
    println!(
        "A_CIRCUIT_C3=GREEN FULL_ADDER=FIVE_GATE_COMPOSITION \
         PURE_XOR_AND_OR_RULES=TRUE FULL_GLOBAL_TRUTH_TABLE=ABSENT \
         HOST_RUNTIME_VALUE_JOIN=0 HOST_GATE_EVALUATOR=0 FINAL_QUIESCENCE=TRUE"
    );
}
