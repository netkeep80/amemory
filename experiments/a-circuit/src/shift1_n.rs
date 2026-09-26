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
        mut seed: Handle,
        o: Handle,
        c: Handle,
        width: usize,
    ) -> Self {
        // Distinct structural namespace from M3 arithmetic and M4 logic/effects.
        seed = store.ensure_pair(seed, o).unwrap();
        seed = store.ensure_pair(seed, c).unwrap();
        seed = store.ensure_pair(seed, c).unwrap();
        seed = store.ensure_pair(seed, o).unwrap();
        seed = store.ensure_pair(seed, c).unwrap();
        for _ in 0..width {
            seed = store.ensure_pair(seed, c).unwrap();
            seed = store.ensure_pair(seed, o).unwrap();
            seed = store.ensure_pair(seed, o).unwrap();
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

    fn roles(
        &mut self,
        store: &mut OptimizedLinkStore,
        count: usize,
    ) -> Vec<Handle> {
        (0..count).map(|_| self.next(store)).collect()
    }
}

#[derive(Clone, Debug)]
struct Shift1Program {
    width: usize,
    shl1: Handle,
    sal1: Handle,
    shr1: Handle,
    sar1: Handle,
    result_tag: Handle,
    active_steps: usize,
    links_after_build: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Shift1Outcome {
    value: u32,
    cf: u8,
    kind: Handle,
}

fn install_rule(
    f: &mut FullFixture,
    anchors: &mut AnchorGen,
    width: usize,
    function: Handle,
    result_tag: Handle,
    output_bits: &[Handle],
    cf: Handle,
) {
    let k = anchors.next(&mut f.store);
    let input_bits = anchors.roles(&mut f.store, width);

    let input_word =
        materialize_exact_sequence(&mut f.store, &input_bits).unwrap();
    let args =
        materialize_exact_sequence(&mut f.store, &[input_word]).unwrap();
    let invocation = call(&mut f.store, f.apply, function, args);
    let before = f.store.ensure_pair(k, invocation).unwrap();

    // output_bits is a template assembled from the input role handles and
    // structural constants. Instantiation performs the runtime wiring.
    let output_word =
        materialize_exact_sequence(&mut f.store, output_bits).unwrap();
    let payload = materialize_exact_sequence(
        &mut f.store,
        &[output_word, cf, function],
    )
    .unwrap();
    let envelope = f.store.ensure_pair(result_tag, payload).unwrap();
    let after = f.store.ensure_pair(k, envelope).unwrap();

    let mut roles = Vec::with_capacity(width + 1);
    roles.push(k);
    roles.extend_from_slice(&input_bits);

    let (_, admission) = define_bundle_rule(
        &mut f.store,
        f.theory,
        &roles,
        before,
        &[after],
    );
    index_rule_for(&mut f.store, &[f.o], admission);
}

impl Shift1Program {
    fn install(f: &mut FullFixture, width: usize) -> Self {
        assert!((8..=32).contains(&width));

        let seed0 = f.store.ensure_pair(f.k, f.full).unwrap();
        let mut anchors = AnchorGen::new(
            &mut f.store,
            seed0,
            f.o,
            f.c,
            width,
        );

        let shl_left = anchors.next(&mut f.store);
        let shl_right = anchors.next(&mut f.store);
        let shl1 = f.store.ensure_pair(shl_left, shl_right).unwrap();

        // SAL is architecturally the same left-shift operation.
        let sal1 = shl1;

        let shr_left = anchors.next(&mut f.store);
        let shr_right = anchors.next(&mut f.store);
        let shr1 = f.store.ensure_pair(shr_left, shr_right).unwrap();

        let sar_left = anchors.next(&mut f.store);
        let sar_right = anchors.next(&mut f.store);
        let sar1 = f.store.ensure_pair(sar_left, sar_right).unwrap();

        let result_left = anchors.next(&mut f.store);
        let result_right = anchors.next(&mut f.store);
        let result_tag =
            f.store.ensure_pair(result_left, result_right).unwrap();

        // Every rule gets its own role namespace. This makes all three rules
        // generic over runtime bit values while keeping the operation identity
        // fixed structurally.
        {
            let input_bits = anchors.roles(&mut f.store, width);
            let mut out = Vec::with_capacity(width);
            out.push(f.zero);
            out.extend_from_slice(&input_bits[..width - 1]);

            // Inline rule construction here because install_rule needs the same
            // input role handles in both before/output templates.
            let k = anchors.next(&mut f.store);
            let word =
                materialize_exact_sequence(&mut f.store, &input_bits).unwrap();
            let args =
                materialize_exact_sequence(&mut f.store, &[word]).unwrap();
            let invocation = call(&mut f.store, f.apply, shl1, args);
            let before = f.store.ensure_pair(k, invocation).unwrap();

            let output_word =
                materialize_exact_sequence(&mut f.store, &out).unwrap();
            let payload = materialize_exact_sequence(
                &mut f.store,
                &[output_word, input_bits[width - 1], shl1],
            )
            .unwrap();
            let envelope =
                f.store.ensure_pair(result_tag, payload).unwrap();
            let after = f.store.ensure_pair(k, envelope).unwrap();

            let mut roles = Vec::with_capacity(width + 1);
            roles.push(k);
            roles.extend_from_slice(&input_bits);
            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        {
            let input_bits = anchors.roles(&mut f.store, width);
            let mut out = Vec::with_capacity(width);
            out.extend_from_slice(&input_bits[1..]);
            out.push(f.zero);

            let k = anchors.next(&mut f.store);
            let word =
                materialize_exact_sequence(&mut f.store, &input_bits).unwrap();
            let args =
                materialize_exact_sequence(&mut f.store, &[word]).unwrap();
            let invocation = call(&mut f.store, f.apply, shr1, args);
            let before = f.store.ensure_pair(k, invocation).unwrap();

            let output_word =
                materialize_exact_sequence(&mut f.store, &out).unwrap();
            let payload = materialize_exact_sequence(
                &mut f.store,
                &[output_word, input_bits[0], shr1],
            )
            .unwrap();
            let envelope =
                f.store.ensure_pair(result_tag, payload).unwrap();
            let after = f.store.ensure_pair(k, envelope).unwrap();

            let mut roles = Vec::with_capacity(width + 1);
            roles.push(k);
            roles.extend_from_slice(&input_bits);
            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        {
            let input_bits = anchors.roles(&mut f.store, width);
            let mut out = Vec::with_capacity(width);
            out.extend_from_slice(&input_bits[1..]);
            out.push(input_bits[width - 1]);

            let k = anchors.next(&mut f.store);
            let word =
                materialize_exact_sequence(&mut f.store, &input_bits).unwrap();
            let args =
                materialize_exact_sequence(&mut f.store, &[word]).unwrap();
            let invocation = call(&mut f.store, f.apply, sar1, args);
            let before = f.store.ensure_pair(k, invocation).unwrap();

            let output_word =
                materialize_exact_sequence(&mut f.store, &out).unwrap();
            let payload = materialize_exact_sequence(
                &mut f.store,
                &[output_word, input_bits[0], sar1],
            )
            .unwrap();
            let envelope =
                f.store.ensure_pair(result_tag, payload).unwrap();
            let after = f.store.ensure_pair(k, envelope).unwrap();

            let mut roles = Vec::with_capacity(width + 1);
            roles.push(k);
            roles.extend_from_slice(&input_bits);
            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        Self {
            width,
            shl1,
            sal1,
            shr1,
            sar1,
            result_tag,
            active_steps: 1,
            links_after_build: f.store.link_count(),
        }
    }
}

fn mask(width: usize) -> u32 {
    if width == 32 {
        u32::MAX
    } else {
        (1u32 << width) - 1
    }
}

fn bit_handles(
    f: &FullFixture,
    width: usize,
    value: u32,
) -> Vec<Handle> {
    (0..width)
        .map(|bit| {
            if (value >> bit) & 1 == 1 {
                f.one
            } else {
                f.zero
            }
        })
        .collect()
}

fn decode_bit(f: &FullFixture, bit: Handle) -> u8 {
    if bit == f.one {
        1
    } else {
        assert_eq!(bit, f.zero);
        0
    }
}

fn decode_word(
    f: &FullFixture,
    width: usize,
    word: Handle,
) -> u32 {
    let bits = read_exact_sequence(&f.store, word).unwrap();
    assert_eq!(bits.len(), width);

    let mut value = 0u32;
    for (index, bit) in bits.into_iter().enumerate() {
        value |= u32::from(decode_bit(f, bit)) << index;
    }
    value
}

fn run_shift1(
    f: &mut FullFixture,
    program: &Shift1Program,
    function: Handle,
    value: u32,
) -> Shift1Outcome {
    let m = mask(program.width);
    assert_eq!(value & !m, 0);

    let bits = bit_handles(f, program.width, value);
    let word =
        materialize_exact_sequence(&mut f.store, &bits).unwrap();
    let args =
        materialize_exact_sequence(&mut f.store, &[word]).unwrap();
    let invocation =
        call(&mut f.store, f.apply, function, args);
    let initial = f.store.ensure_pair(f.k, invocation).unwrap();

    f.engine.set_current(&f.store, &[initial]).unwrap();

    let reaction = f.engine.run(&mut f.store).unwrap();
    assert!(!reaction.quiescent);
    assert_eq!(reaction.raw_rule_matches, 1);
    assert_eq!(reaction.transitioned_members, 1);
    assert_eq!(reaction.handoff_count, 1);
    assert_eq!(reaction.next_members.len(), 1);

    let stable = f.engine.current_bank();
    let quiescent = f.engine.run(&mut f.store).unwrap();
    assert!(quiescent.quiescent);
    assert_eq!(quiescent.raw_rule_matches, 0);
    assert_eq!(quiescent.handoff_count, 0);
    assert_eq!(f.engine.current_bank(), stable);
    assert_eq!(f.engine.current().len(), 1);

    let final_link = f.engine.current()[0];
    let (caller, endpoint) = f.store.poles(final_link).unwrap();
    assert_eq!(caller, f.k);

    let (tag, payload) = f.store.poles(endpoint).unwrap();
    assert_eq!(tag, program.result_tag);
    let values = read_exact_sequence(&f.store, payload).unwrap();
    assert_eq!(values.len(), 3);

    Shift1Outcome {
        value: decode_word(f, program.width, values[0]),
        cf: decode_bit(f, values[1]),
        kind: values[2],
    }
}

fn expected(
    width: usize,
    function: Handle,
    program: &Shift1Program,
    value: u32,
) -> Shift1Outcome {
    let m = mask(width);
    if function == program.shl1 {
        Shift1Outcome {
            value: value.wrapping_shl(1) & m,
            cf: ((value >> (width - 1)) & 1) as u8,
            kind: program.shl1,
        }
    } else if function == program.shr1 {
        Shift1Outcome {
            value: value >> 1,
            cf: (value & 1) as u8,
            kind: program.shr1,
        }
    } else {
        assert_eq!(function, program.sar1);
        let sign = value & (1u32 << (width - 1));
        Shift1Outcome {
            value: (value >> 1) | sign,
            cf: (value & 1) as u8,
            kind: program.sar1,
        }
    }
}

fn vectors(width: usize) -> Vec<u32> {
    let m = mask(width);
    let sign = 1u32 << (width - 1);
    let mut out = vec![
        0,
        1,
        2 & m,
        3 & m,
        m,
        sign,
        sign | 1,
        0xaaaa_aaaa & m,
        0x5555_5555 & m,
        0x8000_0003 & m,
    ];

    let mut z = 0x1656_67b1u32 ^ width as u32;
    for _ in 0..16 {
        z = z.wrapping_mul(1664525).wrapping_add(1013904223);
        out.push(z & m);
    }

    out.sort_unstable();
    out.dedup();
    out
}

#[test]
#[ignore = "heavy M4 SHIFT1 structural suite; mandatory release workflow"]
fn m4_shift1_8_16_32_structural_wiring() {
    for width in [8usize, 16, 32] {
        let mut f = FullFixture::new();
        let program = Shift1Program::install(&mut f, width);

        assert_eq!(program.active_steps, 1);
        assert_eq!(program.sal1, program.shl1);

        let cases = vectors(width);
        for &value in &cases {
            for function in [program.shl1, program.shr1, program.sar1] {
                let actual =
                    run_shift1(&mut f, &program, function, value);
                assert_eq!(
                    actual,
                    expected(width, function, &program, value),
                    "width={width} value={value:#x} function={function}"
                );
            }
        }

        println!(
            "M4_SHIFT1 width={} cases={} reactions_per_shift={} program_links={}",
            width,
            cases.len() * 3,
            program.active_steps,
            program.links_after_build,
        );
    }
}

#[test]
#[ignore = "heavy M4 SHIFT1 structural suite; mandatory release workflow"]
fn m4_shift1_cf_sign_fill_and_steady_state() {
    let mut f = FullFixture::new();
    let program = Shift1Program::install(&mut f, 32);

    let shl = run_shift1(
        &mut f,
        &program,
        program.shl1,
        0x8000_0000,
    );
    assert_eq!(shl.value, 0);
    assert_eq!(shl.cf, 1);
    assert_eq!(shl.kind, program.shl1);

    let shr = run_shift1(&mut f, &program, program.shr1, 1);
    assert_eq!(shr.value, 0);
    assert_eq!(shr.cf, 1);
    assert_eq!(shr.kind, program.shr1);

    let sar = run_shift1(
        &mut f,
        &program,
        program.sar1,
        0x8000_0001,
    );
    assert_eq!(sar.value, 0xc000_0000);
    assert_eq!(sar.cf, 1);
    assert_eq!(sar.kind, program.sar1);

    let first = run_shift1(
        &mut f,
        &program,
        program.sar1,
        0x9234_5679,
    );
    let links = f.store.link_count();
    let second = run_shift1(
        &mut f,
        &program,
        program.sar1,
        0x9234_5679,
    );
    assert_eq!(second, first);
    assert_eq!(
        f.store.link_count(),
        links,
        "repeated identical SHIFT1_32 materialized new Links"
    );
}
