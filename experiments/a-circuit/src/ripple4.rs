use super::full_adder::{
    call, define_bundle_rule, index_rule_for, Fixture as FullFixture,
};
use amemory_optimized_cpu_probe::{
    structural::{materialize_exact_sequence, read_exact_sequence},
    Handle, OptimizedLinkStore,
};

fn fresh_from(
    store: &mut OptimizedLinkStore,
    mut seed: Handle,
    o: Handle,
    c: Handle,
    count: usize,
) -> Vec<Handle> {
    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        seed = store
            .ensure_pair(seed, if i % 2 == 0 { o } else { c })
            .unwrap();
        result.push(seed);
    }
    result
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

fn index_admission(
    store: &mut OptimizedLinkStore,
    trigger_keys: &[Handle],
    admission: Handle,
) {
    index_rule_for(store, trigger_keys, admission);
}

struct Add4Program {
    add4: Handle,
    stage_tags: [Handle; 4],
    full_outputs: [Handle; 4],
}

impl Add4Program {
    fn install(f: &mut FullFixture) -> Self {
        // Start a new anchor family from an existing unique component pair so
        // this program does not rely on host IDs or collide with M2 anchors.
        let seed = f.store.ensure_pair(f.full, f.k).unwrap();
        let anchors = fresh_from(&mut f.store, seed, f.o, f.c, 180);
        let at = |i: usize| anchors[i];

        let add4 = f.store.ensure_pair(at(0), at(1)).unwrap();
        let stage_tags = [
            f.store.ensure_pair(at(2), at(3)).unwrap(),
            f.store.ensure_pair(at(4), at(5)).unwrap(),
            f.store.ensure_pair(at(6), at(7)).unwrap(),
            f.store.ensure_pair(at(8), at(9)).unwrap(),
        ];

        let full_outputs = [
            materialize_exact_sequence(&mut f.store, &[f.zero, f.zero]).unwrap(),
            materialize_exact_sequence(&mut f.store, &[f.one, f.zero]).unwrap(),
            materialize_exact_sequence(&mut f.store, &[f.zero, f.one]).unwrap(),
            materialize_exact_sequence(&mut f.store, &[f.one, f.one]).unwrap(),
        ];

        // -------------------------------------------------------------
        // ADD4 OPEN
        //
        // K -> Call(ADD4,[Aword,Bword,Cin])
        // =>
        // S0Frame(K,a1,a2,a3,b1,b2,b3)
        //   -> Call(FULL,[a0,b0,Cin])
        //
        // Word4 is ROOT-originating ExactSequence([b0,b1,b2,b3]), LSB first.
        // -------------------------------------------------------------
        {
            let k = at(20);
            let a0 = at(21);
            let a1 = at(22);
            let a2 = at(23);
            let a3 = at(24);
            let b0 = at(25);
            let b1 = at(26);
            let b2 = at(27);
            let b3 = at(28);
            let cin = at(29);

            let aword =
                materialize_exact_sequence(&mut f.store, &[a0, a1, a2, a3]).unwrap();
            let bword =
                materialize_exact_sequence(&mut f.store, &[b0, b1, b2, b3]).unwrap();
            let args =
                materialize_exact_sequence(&mut f.store, &[aword, bword, cin]).unwrap();

            let invocation = call(&mut f.store, f.apply, add4, args);
            let before = f.store.ensure_pair(k, invocation).unwrap();

            let caller = stage_frame(
                &mut f.store,
                stage_tags[0],
                &[k, a1, a2, a3, b1, b2, b3],
            );
            let fa_args =
                materialize_exact_sequence(&mut f.store, &[a0, b0, cin]).unwrap();
            let fa_call = call(&mut f.store, f.apply, f.full, fa_args);
            let after = f.store.ensure_pair(caller, fa_call).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, a0, a1, a2, a3, b0, b1, b2, b3, cin],
                before,
                &[after],
            );
            index_admission(&mut f.store, &[f.o], admission);
        }

        // S0 result -> invoke bit 1 Full Adder.
        {
            let k = at(40);
            let a1 = at(41);
            let a2 = at(42);
            let a3 = at(43);
            let b1 = at(44);
            let b2 = at(45);
            let b3 = at(46);
            let s0 = at(47);
            let c1 = at(48);

            let caller = stage_frame(
                &mut f.store,
                stage_tags[0],
                &[k, a1, a2, a3, b1, b2, b3],
            );
            let result =
                materialize_exact_sequence(&mut f.store, &[s0, c1]).unwrap();
            let before = f.store.ensure_pair(caller, result).unwrap();

            let next_caller = stage_frame(
                &mut f.store,
                stage_tags[1],
                &[k, a2, a3, b2, b3, s0],
            );
            let args =
                materialize_exact_sequence(&mut f.store, &[a1, b1, c1]).unwrap();
            let invocation = call(&mut f.store, f.apply, f.full, args);
            let after = f.store.ensure_pair(next_caller, invocation).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, a1, a2, a3, b1, b2, b3, s0, c1],
                before,
                &[after],
            );
            index_admission(&mut f.store, &full_outputs, admission);
        }

        // S1 result -> invoke bit 2 Full Adder.
        {
            let k = at(60);
            let a2 = at(61);
            let a3 = at(62);
            let b2 = at(63);
            let b3 = at(64);
            let s0 = at(65);
            let s1 = at(66);
            let c2 = at(67);

            let caller = stage_frame(
                &mut f.store,
                stage_tags[1],
                &[k, a2, a3, b2, b3, s0],
            );
            let result =
                materialize_exact_sequence(&mut f.store, &[s1, c2]).unwrap();
            let before = f.store.ensure_pair(caller, result).unwrap();

            let next_caller = stage_frame(
                &mut f.store,
                stage_tags[2],
                &[k, a3, b3, s0, s1],
            );
            let args =
                materialize_exact_sequence(&mut f.store, &[a2, b2, c2]).unwrap();
            let invocation = call(&mut f.store, f.apply, f.full, args);
            let after = f.store.ensure_pair(next_caller, invocation).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, a2, a3, b2, b3, s0, s1, c2],
                before,
                &[after],
            );
            index_admission(&mut f.store, &full_outputs, admission);
        }

        // S2 result -> invoke bit 3 Full Adder.
        {
            let k = at(80);
            let a3 = at(81);
            let b3 = at(82);
            let s0 = at(83);
            let s1 = at(84);
            let s2 = at(85);
            let c3 = at(86);

            let caller = stage_frame(
                &mut f.store,
                stage_tags[2],
                &[k, a3, b3, s0, s1],
            );
            let result =
                materialize_exact_sequence(&mut f.store, &[s2, c3]).unwrap();
            let before = f.store.ensure_pair(caller, result).unwrap();

            let next_caller = stage_frame(
                &mut f.store,
                stage_tags[3],
                &[k, s0, s1, s2],
            );
            let args =
                materialize_exact_sequence(&mut f.store, &[a3, b3, c3]).unwrap();
            let invocation = call(&mut f.store, f.apply, f.full, args);
            let after = f.store.ensure_pair(next_caller, invocation).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, a3, b3, s0, s1, s2, c3],
                before,
                &[after],
            );
            index_admission(&mut f.store, &full_outputs, admission);
        }

        // S3 result -> assemble SumWord and Cout entirely in the Rule template.
        {
            let k = at(100);
            let s0 = at(101);
            let s1 = at(102);
            let s2 = at(103);
            let s3 = at(104);
            let cout = at(105);

            let caller = stage_frame(
                &mut f.store,
                stage_tags[3],
                &[k, s0, s1, s2],
            );
            let result =
                materialize_exact_sequence(&mut f.store, &[s3, cout]).unwrap();
            let before = f.store.ensure_pair(caller, result).unwrap();

            let sum_word =
                materialize_exact_sequence(&mut f.store, &[s0, s1, s2, s3]).unwrap();
            let final_result =
                materialize_exact_sequence(&mut f.store, &[sum_word, cout]).unwrap();
            let after = f.store.ensure_pair(k, final_result).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, s0, s1, s2, s3, cout],
                before,
                &[after],
            );
            index_admission(&mut f.store, &full_outputs, admission);
        }

        Self {
            add4,
            stage_tags,
            full_outputs,
        }
    }
}

fn bit_handles(f: &FullFixture, value: u8) -> [Handle; 4] {
    [
        if value & 0b0001 != 0 { f.one } else { f.zero },
        if value & 0b0010 != 0 { f.one } else { f.zero },
        if value & 0b0100 != 0 { f.one } else { f.zero },
        if value & 0b1000 != 0 { f.one } else { f.zero },
    ]
}

fn decode_word(f: &FullFixture, word: Handle) -> u8 {
    let bits = read_exact_sequence(&f.store, word).unwrap();
    assert_eq!(bits.len(), 4, "Word4 arity");

    let mut value = 0u8;
    for (index, bit) in bits.into_iter().enumerate() {
        if bit == f.one {
            value |= 1u8 << index;
        } else {
            assert_eq!(bit, f.zero, "Word4 bit must be canonical 0/1");
        }
    }
    value
}

fn run_add4(
    f: &mut FullFixture,
    program: &Add4Program,
    a: u8,
    b: u8,
    cin: u8,
) -> (u8, u8, usize) {
    let a_bits = bit_handles(f, a);
    let b_bits = bit_handles(f, b);
    let aword = materialize_exact_sequence(&mut f.store, &a_bits).unwrap();
    let bword = materialize_exact_sequence(&mut f.store, &b_bits).unwrap();
    let cin_handle = if cin == 0 { f.zero } else { f.one };

    let args =
        materialize_exact_sequence(&mut f.store, &[aword, bword, cin_handle]).unwrap();
    let invocation = call(&mut f.store, f.apply, program.add4, args);
    let initial = f.store.ensure_pair(f.k, invocation).unwrap();

    f.engine.set_current(&f.store, &[initial]).unwrap();

    const ACTIVE_STEPS: usize = 57;
    for step in 0..ACTIVE_STEPS {
        let reaction = f.engine.run(&mut f.store).unwrap();
        assert!(!reaction.quiescent, "{a}+{b}+{cin}: step {step}");
        assert_eq!(reaction.raw_rule_matches, 1, "step {step} matches");
        assert_eq!(reaction.transitioned_members, 1, "step {step} members");
        assert_eq!(reaction.handoff_count, 1, "step {step} handoff");
        assert_eq!(reaction.next_members.len(), 1, "step {step} scope size");
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

    let result_values = read_exact_sequence(&f.store, result).unwrap();
    assert_eq!(result_values.len(), 2);
    let sum = decode_word(f, result_values[0]);
    let cout = if result_values[1] == f.one {
        1
    } else {
        assert_eq!(result_values[1], f.zero);
        0
    };

    (sum, cout, ACTIVE_STEPS)
}

#[test]
fn m0_word4_is_root_originating_lsb_first_and_ordered() {
    let mut f = FullFixture::new();

    let bits = bit_handles(&f, 0b1010);
    let word = materialize_exact_sequence(&mut f.store, &bits).unwrap();
    assert_eq!(decode_word(&f, word), 0b1010);

    let reversed = [bits[3], bits[2], bits[1], bits[0]];
    let reversed_word =
        materialize_exact_sequence(&mut f.store, &reversed).unwrap();

    assert_ne!(word, reversed_word);
    assert_eq!(decode_word(&f, reversed_word), 0b0101);
}

#[test]
fn m3_ripple4_exhaustive_512_cases() {
    let mut f = FullFixture::new();
    let program = Add4Program::install(&mut f);

    let before = f.store.link_count();
    let mut cases = 0usize;

    for a in 0u8..16 {
        for b in 0u8..16 {
            for cin in 0u8..=1 {
                let (sum, cout, steps) =
                    run_add4(&mut f, &program, a, b, cin);
                let total = u16::from(a) + u16::from(b) + u16::from(cin);

                assert_eq!(sum, (total & 0x0f) as u8, "A={a} B={b} Cin={cin}");
                assert_eq!(cout, ((total >> 4) & 1) as u8, "A={a} B={b} Cin={cin}");
                assert_eq!(steps, 57);
                cases += 1;
            }
        }
    }

    assert_eq!(cases, 512);

    // Representative human-readable checkpoints.
    for (a, b, cin) in [
        (0, 0, 0),
        (1, 1, 0),
        (3, 5, 0),
        (7, 1, 0),
        (7, 8, 0),
        (15, 0, 0),
        (15, 1, 0),
        (15, 15, 0),
        (15, 15, 1),
    ] {
        let (sum, cout, _) = run_add4(&mut f, &program, a, b, cin);
        println!(
            "RIPPLE4_TRACE a={a} b={b} cin={cin} sum={sum} cout={cout} active_steps=57"
        );
    }

    let after = f.store.link_count();
    println!(
        "RIPPLE4_EXHAUSTIVE=GREEN cases=512 links_before={before} links_after={after}"
    );
}

#[test]
fn m3_ripple4_rejects_wrong_word_arity_by_no_match() {
    let mut f = FullFixture::new();
    let program = Add4Program::install(&mut f);

    let a3 = materialize_exact_sequence(
        &mut f.store,
        &[f.zero, f.zero, f.zero],
    )
    .unwrap();
    let b4 = materialize_exact_sequence(
        &mut f.store,
        &[f.zero, f.zero, f.zero, f.zero],
    )
    .unwrap();
    let args =
        materialize_exact_sequence(&mut f.store, &[a3, b4, f.zero]).unwrap();
    let invocation = call(&mut f.store, f.apply, program.add4, args);
    let initial = f.store.ensure_pair(f.k, invocation).unwrap();

    f.engine.set_current(&f.store, &[initial]).unwrap();
    let reaction = f.engine.run(&mut f.store).unwrap();

    assert!(reaction.quiescent);
    assert_eq!(reaction.raw_rule_matches, 0);
    assert_eq!(reaction.handoff_count, 0);
    assert_eq!(f.engine.current(), &[initial]);
}
