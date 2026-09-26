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

fn call(store: &mut OptimizedLinkStore, b: Basis, f: Handle, arg: Handle) -> Handle {
    let body = store.ensure_pair(f, arg).unwrap();
    store.ensure_pair(b.o, body).unwrap()
}

fn done(store: &mut OptimizedLinkStore, b: Basis, value: Handle) -> Handle {
    store.ensure_pair(b.c, value).unwrap()
}

fn xor_frame(store: &mut OptimizedLinkStore, caller: Handle, args: Handle) -> Handle {
    let payload = store.ensure_pair(caller, args).unwrap();
    store.ensure_start_self_closed(payload).unwrap()
}

fn and_frame(
    store: &mut OptimizedLinkStore,
    caller: Handle,
    args: Handle,
    sum: Handle,
) -> Handle {
    let args_sum = store.ensure_pair(args, sum).unwrap();
    let payload = store.ensure_pair(caller, args_sum).unwrap();
    store.ensure_start_self_closed(payload).unwrap()
}

fn finish_frame(store: &mut OptimizedLinkStore, caller: Handle, sum: Handle) -> Handle {
    let payload = store.ensure_pair(caller, sum).unwrap();
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
    let output_bundle = materialize_exact_sequence(store, after).unwrap();
    let body = store.ensure_pair(before, output_bundle).unwrap();
    let rule = define_structural_rule(store, dictionary, body).unwrap();
    let admission = admit_structural_rule(store, theory, rule).unwrap();
    index_structural_rule_trigger(store, trigger_key, admission).unwrap();
}

fn fresh_anchors(store: &mut OptimizedLinkStore, b: Basis, count: usize) -> Vec<Handle> {
    let mut seed = store.ensure_pair(b.u, b.l).unwrap();
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        seed = store
            .ensure_pair(seed, if i % 2 == 0 { b.o } else { b.c })
            .unwrap();
        out.push(seed);
    }
    out
}

struct Fixture {
    store: OptimizedLinkStore,
    b: Basis,
    interpreter: Handle,
    half: Handle,
    xor: Handle,
    and: Handle,
    k: Handle,
    zero: Handle,
    one: Handle,
    args: [[Handle; 2]; 2],
}

fn bit_index(value: Handle, one: Handle) -> usize {
    if value == one { 1 } else { 0 }
}

fn build_fixture() -> Fixture {
    let mut store = OptimizedLinkStore::new();
    let b = basis(&mut store);
    let fresh = fresh_anchors(&mut store, b, 220);
    let at = |i: usize| fresh[i];

    let theory = store.ensure_pair(at(0), at(1)).unwrap();
    let authority_dictionary = define_structural_role_dictionary(&mut store, &[]).unwrap();
    let grammar = store.ensure_pair(at(2), at(3)).unwrap();
    let interpreter =
        define_structural_interpreter(&mut store, authority_dictionary, grammar, theory).unwrap();

    let half = store.ensure_pair(at(4), at(5)).unwrap();
    let xor = store.ensure_pair(at(6), at(7)).unwrap();
    let and = store.ensure_pair(at(8), at(9)).unwrap();
    let k = store.ensure_pair(at(10), at(11)).unwrap();

    let zero = b.u;
    let one = b.l;

    let args = [
        [
            materialize_exact_sequence(&mut store, &[zero, zero]).unwrap(),
            materialize_exact_sequence(&mut store, &[zero, one]).unwrap(),
        ],
        [
            materialize_exact_sequence(&mut store, &[one, zero]).unwrap(),
            materialize_exact_sequence(&mut store, &[one, one]).unwrap(),
        ],
    ];

    // Generic HALF-open rule.
    {
        let k_role = at(20);
        let args_role = at(21);
        let half_call = call(&mut store, b, half, args_role);
        let before = store.ensure_pair(k_role, half_call).unwrap();

        let frame = xor_frame(&mut store, k_role, args_role);
        let xor_call = call(&mut store, b, xor, args_role);
        let after = store.ensure_pair(frame, xor_call).unwrap();

        admit_bundle_rule(
            &mut store,
            theory,
            b.o,
            &[k_role, args_role],
            before,
            &[after],
        );
    }

    let xor_rows = [
        (zero, zero, zero),
        (zero, one, one),
        (one, zero, one),
        (one, one, zero),
    ];

    let mut rule_seed = 30usize;
    for (a, c, sum) in xor_rows {
        let argument_sequence = args[bit_index(a, one)][bit_index(c, one)];
        let k_role = at(rule_seed);
        rule_seed += 1;

        let frame = xor_frame(&mut store, k_role, argument_sequence);
        let xor_call = call(&mut store, b, xor, argument_sequence);
        let before = store.ensure_pair(frame, xor_call).unwrap();

        let next_frame = and_frame(&mut store, k_role, argument_sequence, sum);
        let and_call = call(&mut store, b, and, argument_sequence);
        let after = store.ensure_pair(next_frame, and_call).unwrap();

        admit_bundle_rule(
            &mut store,
            theory,
            b.o,
            &[k_role],
            before,
            &[after],
        );
    }

    let and_rows = [
        (zero, zero, zero),
        (zero, one, zero),
        (one, zero, zero),
        (one, one, one),
    ];

    for (a, c, carry) in and_rows {
        let argument_sequence = args[bit_index(a, one)][bit_index(c, one)];
        let k_role = at(rule_seed);
        rule_seed += 1;
        let sum_role = at(rule_seed);
        rule_seed += 1;

        let frame = and_frame(&mut store, k_role, argument_sequence, sum_role);
        let and_call = call(&mut store, b, and, argument_sequence);
        let before = store.ensure_pair(frame, and_call).unwrap();

        let next_frame = finish_frame(&mut store, k_role, sum_role);
        let carry_done = done(&mut store, b, carry);
        let after = store.ensure_pair(next_frame, carry_done).unwrap();

        admit_bundle_rule(
            &mut store,
            theory,
            b.o,
            &[k_role, sum_role],
            before,
            &[after],
        );
    }

    // Generic finalizer: runtime Sum + Carry role bindings are assembled into
    // the ROOT-originating ExactSequence by structural template instantiation.
    {
        let k_role = at(80);
        let sum_role = at(81);
        let carry_role = at(82);

        let frame = finish_frame(&mut store, k_role, sum_role);
        let carry_done = done(&mut store, b, carry_role);
        let before = store.ensure_pair(frame, carry_done).unwrap();

        let result_template =
            materialize_exact_sequence(&mut store, &[sum_role, carry_role]).unwrap();
        let after = store.ensure_pair(k_role, result_template).unwrap();

        admit_bundle_rule(
            &mut store,
            theory,
            b.c,
            &[k_role, sum_role, carry_role],
            before,
            &[after],
        );
    }

    Fixture {
        store,
        b,
        interpreter,
        half,
        xor,
        and,
        k,
        zero,
        one,
        args,
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

fn run_case(f: &mut Fixture, a: Handle, b_arg: Handle) -> Row {
    let ai = bit_index(a, f.one);
    let bi = bit_index(b_arg, f.one);
    let argument_sequence = f.args[ai][bi];

    let half_call = call(&mut f.store, f.b, f.half, argument_sequence);
    let initial = f.store.ensure_pair(f.k, half_call).unwrap();

    let mut engine = OptimizedStructuralEngine::new(16);
    engine.set_interpreter(&f.store, f.interpreter).unwrap();
    engine.set_current(&f.store, &[initial]).unwrap();

    let mut matches = Vec::new();
    let mut handoffs = Vec::new();

    for _ in 0..4 {
        let reaction = engine.run(&mut f.store).unwrap();
        assert!(!reaction.quiescent);
        assert_eq!(reaction.raw_rule_matches, 1);
        assert_eq!(reaction.transitioned_members, 1);
        assert_eq!(reaction.handoff_count, 1);
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

    let decoded = read_exact_sequence(&f.store, result_sequence).unwrap();
    assert_eq!(decoded.len(), 2);

    let sum = decoded[0];
    let carry = decoded[1];

    let expected_sum = if (a == f.one) ^ (b_arg == f.one) { f.one } else { f.zero };
    let expected_carry = if (a == f.one) && (b_arg == f.one) { f.one } else { f.zero };
    assert_eq!(sum, expected_sum);
    assert_eq!(carry, expected_carry);

    Row {
        input: format!("{}{}", if a == f.one { 1 } else { 0 }, if b_arg == f.one { 1 } else { 0 }),
        output: format!("{}{}", if sum == f.one { 1 } else { 0 }, if carry == f.one { 1 } else { 0 }),
        matches,
        handoffs,
        quiescent: quiescent.quiescent,
    }
}

fn vec_json(values: &[u32]) -> String {
    values.iter().map(u32::to_string).collect::<Vec<_>>().join(",")
}

fn main() {
    let mut f = build_fixture();

    let zero = f.zero;
    let one = f.one;
    let rows = vec![
        run_case(&mut f, zero, zero),
        run_case(&mut f, zero, one),
        run_case(&mut f, one, zero),
        run_case(&mut f, one, one),
    ];

    let row_json = rows
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
        "C2B_RESULT_JSON={{\"rows\":[{}],\"activeReactionsPerCase\":4}}",
        row_json
    );
    println!(
        "A_CIRCUIT_C2B_RUST=GREEN STRUCTURAL_EXECUTOR=OPTIMIZED_CPU \
         HALF_ADDER=GATE_COMPOSED XOR_RULE_SET=SEPARATE AND_RULE_SET=SEPARATE \
         HALF_GLOBAL_TRUTH_TABLE=ABSENT HOST_RUNTIME_VALUE_JOIN=0 HOST_GATE_EVALUATOR=0"
    );
}
