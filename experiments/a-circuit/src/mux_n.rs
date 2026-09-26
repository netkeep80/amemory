use super::{
    full_adder::{
        call, define_bundle_rule, index_rule_for, Fixture as FullFixture,
    },
    logic_n::{install_gate_basis, GateSet},
};
use amemory_optimized_cpu_probe::{
    structural::{materialize_exact_sequence, read_exact_sequence},
    Handle, OptimizedLinkStore,
};

const WIDTH: usize = 32;

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
    ) -> Self {
        for pole in [o, c, o, o, c, o, c, c, o, c, c, c, o] {
            seed = store.ensure_pair(seed, pole).unwrap();
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

fn stage_frame(
    store: &mut OptimizedLinkStore,
    tag: Handle,
    values: &[Handle],
) -> Handle {
    let payload = materialize_exact_sequence(store, values).unwrap();
    let descriptor = store.ensure_pair(tag, payload).unwrap();
    store.ensure_start_self_closed(descriptor).unwrap()
}

fn binary_call(
    f: &mut FullFixture,
    function: Handle,
    a: Handle,
    b: Handle,
) -> Handle {
    let args = materialize_exact_sequence(&mut f.store, &[a, b]).unwrap();
    call(&mut f.store, f.apply, function, args)
}

#[derive(Clone, Debug)]
struct MuxProgram {
    direct_mux1: Handle,
    mux1: Handle,
    mux32: Handle,
    bit_result_tag: Handle,
    word_result_tag: Handle,
    gates: GateSet,
    direct_steps: usize,
    mux1_steps: usize,
    mux32_steps: usize,
    links_after_build: usize,
}

impl MuxProgram {
    fn install(f: &mut FullFixture) -> Self {
        let gates = install_gate_basis(f);

        let seed0 = f.store.ensure_pair(f.k, gates.xor2).unwrap();
        let mut anchors =
            AnchorGen::new(&mut f.store, seed0, f.o, f.c);

        let direct_left = anchors.next(&mut f.store);
        let direct_right = anchors.next(&mut f.store);
        let direct_mux1 =
            f.store.ensure_pair(direct_left, direct_right).unwrap();

        let mux1_left = anchors.next(&mut f.store);
        let mux1_right = anchors.next(&mut f.store);
        let mux1 =
            f.store.ensure_pair(mux1_left, mux1_right).unwrap();

        let mux32_left = anchors.next(&mut f.store);
        let mux32_right = anchors.next(&mut f.store);
        let mux32 =
            f.store.ensure_pair(mux32_left, mux32_right).unwrap();

        let bit_tag_left = anchors.next(&mut f.store);
        let bit_tag_right = anchors.next(&mut f.store);
        let bit_result_tag =
            f.store.ensure_pair(bit_tag_left, bit_tag_right).unwrap();

        let word_tag_left = anchors.next(&mut f.store);
        let word_tag_right = anchors.next(&mut f.store);
        let word_result_tag =
            f.store.ensure_pair(word_tag_left, word_tag_right).unwrap();

        let xor_ab_tag = anchors.next(&mut f.store);
        let and_s_tag = anchors.next(&mut f.store);
        let xor_out_tag = anchors.next(&mut f.store);

        for (select, choose_b) in [(f.zero, false), (f.one, true)] {
            let k = anchors.next(&mut f.store);
            let a = anchors.next(&mut f.store);
            let b = anchors.next(&mut f.store);

            let args = materialize_exact_sequence(
                &mut f.store,
                &[select, a, b],
            )
            .unwrap();
            let invocation =
                call(&mut f.store, f.apply, direct_mux1, args);
            let before = f.store.ensure_pair(k, invocation).unwrap();

            let selected = if choose_b { b } else { a };
            let payload =
                materialize_exact_sequence(&mut f.store, &[selected])
                    .unwrap();
            let endpoint =
                f.store.ensure_pair(bit_result_tag, payload).unwrap();
            let after = f.store.ensure_pair(k, endpoint).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, a, b],
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        {
            let k = anchors.next(&mut f.store);
            let s = anchors.next(&mut f.store);
            let a = anchors.next(&mut f.store);
            let b = anchors.next(&mut f.store);

            let args =
                materialize_exact_sequence(&mut f.store, &[s, a, b])
                    .unwrap();
            let invocation =
                call(&mut f.store, f.apply, mux1, args);
            let before = f.store.ensure_pair(k, invocation).unwrap();

            let caller =
                stage_frame(&mut f.store, xor_ab_tag, &[k, s, a]);
            let xor_call =
                binary_call(f, gates.xor2, a, b);
            let after = f.store.ensure_pair(caller, xor_call).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, s, a, b],
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        {
            let k = anchors.next(&mut f.store);
            let s = anchors.next(&mut f.store);
            let a = anchors.next(&mut f.store);
            let x = anchors.next(&mut f.store);

            let caller =
                stage_frame(&mut f.store, xor_ab_tag, &[k, s, a]);
            let x_result =
                materialize_exact_sequence(&mut f.store, &[x]).unwrap();
            let before = f.store.ensure_pair(caller, x_result).unwrap();

            let next_caller =
                stage_frame(&mut f.store, and_s_tag, &[k, a]);
            let and_call =
                binary_call(f, gates.and2, s, x);
            let after =
                f.store.ensure_pair(next_caller, and_call).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, s, a, x],
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &gates.bit_outputs,
                admission,
            );
        }

        {
            let k = anchors.next(&mut f.store);
            let a = anchors.next(&mut f.store);
            let y = anchors.next(&mut f.store);

            let caller =
                stage_frame(&mut f.store, and_s_tag, &[k, a]);
            let y_result =
                materialize_exact_sequence(&mut f.store, &[y]).unwrap();
            let before = f.store.ensure_pair(caller, y_result).unwrap();

            let next_caller =
                stage_frame(&mut f.store, xor_out_tag, &[k]);
            let xor_call =
                binary_call(f, gates.xor2, a, y);
            let after =
                f.store.ensure_pair(next_caller, xor_call).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, a, y],
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &gates.bit_outputs,
                admission,
            );
        }

        {
            let k = anchors.next(&mut f.store);
            let out = anchors.next(&mut f.store);

            let caller =
                stage_frame(&mut f.store, xor_out_tag, &[k]);
            let out_result =
                materialize_exact_sequence(&mut f.store, &[out]).unwrap();
            let before =
                f.store.ensure_pair(caller, out_result).unwrap();

            let payload =
                materialize_exact_sequence(&mut f.store, &[out]).unwrap();
            let endpoint =
                f.store.ensure_pair(bit_result_tag, payload).unwrap();
            let after = f.store.ensure_pair(k, endpoint).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, out],
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &gates.bit_outputs,
                admission,
            );
        }

        let mut word_tags = Vec::with_capacity(WIDTH);
        for _ in 0..WIDTH {
            let left = anchors.next(&mut f.store);
            let right = anchors.next(&mut f.store);
            word_tags.push(f.store.ensure_pair(left, right).unwrap());
        }

        {
            let k = anchors.next(&mut f.store);
            let s = anchors.next(&mut f.store);
            let a_bits = anchors.roles(&mut f.store, WIDTH);
            let b_bits = anchors.roles(&mut f.store, WIDTH);

            let aword =
                materialize_exact_sequence(&mut f.store, &a_bits).unwrap();
            let bword =
                materialize_exact_sequence(&mut f.store, &b_bits).unwrap();
            let args =
                materialize_exact_sequence(&mut f.store, &[s, aword, bword])
                    .unwrap();
            let invocation =
                call(&mut f.store, f.apply, mux32, args);
            let before = f.store.ensure_pair(k, invocation).unwrap();

            let mut state = Vec::with_capacity(2 + 2 * (WIDTH - 1));
            state.push(k);
            state.push(s);
            state.extend_from_slice(&a_bits[1..]);
            state.extend_from_slice(&b_bits[1..]);

            let caller =
                stage_frame(&mut f.store, word_tags[0], &state);
            let mux_args = materialize_exact_sequence(
                &mut f.store,
                &[s, a_bits[0], b_bits[0]],
            )
            .unwrap();
            let mux_call =
                call(&mut f.store, f.apply, mux1, mux_args);
            let after = f.store.ensure_pair(caller, mux_call).unwrap();

            let mut roles = Vec::with_capacity(2 + 2 * WIDTH);
            roles.push(k);
            roles.push(s);
            roles.extend_from_slice(&a_bits);
            roles.extend_from_slice(&b_bits);

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        for i in 0..WIDTH {
            let k = anchors.next(&mut f.store);
            let s = anchors.next(&mut f.store);
            let remaining = WIDTH - i - 1;
            let a_rem = anchors.roles(&mut f.store, remaining);
            let b_rem = anchors.roles(&mut f.store, remaining);
            let previous = anchors.roles(&mut f.store, i);
            let out = anchors.next(&mut f.store);

            let mut state =
                Vec::with_capacity(2 + 2 * remaining + i);
            state.push(k);
            state.push(s);
            state.extend_from_slice(&a_rem);
            state.extend_from_slice(&b_rem);
            state.extend_from_slice(&previous);

            let caller =
                stage_frame(&mut f.store, word_tags[i], &state);
            let bit_payload =
                materialize_exact_sequence(&mut f.store, &[out]).unwrap();
            let bit_endpoint =
                f.store.ensure_pair(bit_result_tag, bit_payload).unwrap();
            let before =
                f.store.ensure_pair(caller, bit_endpoint).unwrap();

            let after = if i + 1 < WIDTH {
                let mut next_previous = previous.clone();
                next_previous.push(out);

                let mut next_state =
                    Vec::with_capacity(2 + 2 * (remaining - 1) + i + 1);
                next_state.push(k);
                next_state.push(s);
                next_state.extend_from_slice(&a_rem[1..]);
                next_state.extend_from_slice(&b_rem[1..]);
                next_state.extend_from_slice(&next_previous);

                let next_caller =
                    stage_frame(&mut f.store, word_tags[i + 1], &next_state);
                let mux_args = materialize_exact_sequence(
                    &mut f.store,
                    &[s, a_rem[0], b_rem[0]],
                )
                .unwrap();
                let mux_call =
                    call(&mut f.store, f.apply, mux1, mux_args);
                f.store.ensure_pair(next_caller, mux_call).unwrap()
            } else {
                let mut result_bits = previous.clone();
                result_bits.push(out);
                let word = materialize_exact_sequence(
                    &mut f.store,
                    &result_bits,
                )
                .unwrap();
                let payload =
                    materialize_exact_sequence(&mut f.store, &[word]).unwrap();
                let endpoint =
                    f.store.ensure_pair(word_result_tag, payload).unwrap();
                f.store.ensure_pair(k, endpoint).unwrap()
            };

            let mut roles =
                Vec::with_capacity(3 + 2 * remaining + i);
            roles.push(k);
            roles.push(s);
            roles.extend_from_slice(&a_rem);
            roles.extend_from_slice(&b_rem);
            roles.extend_from_slice(&previous);
            roles.push(out);

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &[bit_result_tag],
                admission,
            );
        }

        Self {
            direct_mux1,
            mux1,
            mux32,
            bit_result_tag,
            word_result_tag,
            gates,
            direct_steps: 1,
            mux1_steps: 7,
            mux32_steps: 1 + WIDTH * 8,
            links_after_build: f.store.link_count(),
        }
    }
}

fn decode_bit(f: &FullFixture, bit: Handle) -> u8 {
    if bit == f.one {
        1
    } else {
        assert_eq!(bit, f.zero);
        0
    }
}

fn run_exact_steps(
    f: &mut FullFixture,
    steps: usize,
    label: &str,
) {
    for step in 0..steps {
        let result = f.engine.run(&mut f.store).unwrap();
        assert!(
            !result.quiescent,
            "{label}: unexpected quiescence at {step}"
        );
        assert_eq!(result.raw_rule_matches, 1, "{label} step {step}");
        assert_eq!(result.transitioned_members, 1, "{label} step {step}");
        assert_eq!(result.handoff_count, 1, "{label} step {step}");
        assert_eq!(result.next_members.len(), 1, "{label} step {step}");
    }

    let stable = f.engine.current_bank();
    let quiescent = f.engine.run(&mut f.store).unwrap();
    assert!(quiescent.quiescent, "{label}: final quiescence");
    assert_eq!(quiescent.raw_rule_matches, 0);
    assert_eq!(quiescent.handoff_count, 0);
    assert_eq!(f.engine.current_bank(), stable);
}

fn run_mux1(
    f: &mut FullFixture,
    program: &MuxProgram,
    function: Handle,
    steps: usize,
    s: u8,
    a: u8,
    b: u8,
) -> u8 {
    let bits = [f.zero, f.one];
    let args = materialize_exact_sequence(
        &mut f.store,
        &[bits[s as usize], bits[a as usize], bits[b as usize]],
    )
    .unwrap();
    let invocation =
        call(&mut f.store, f.apply, function, args);
    let initial = f.store.ensure_pair(f.k, invocation).unwrap();

    f.engine.set_current(&f.store, &[initial]).unwrap();
    run_exact_steps(f, steps, "MUX1");

    let final_link = f.engine.current()[0];
    let (caller, endpoint) = f.store.poles(final_link).unwrap();
    assert_eq!(caller, f.k);
    let (tag, payload) = f.store.poles(endpoint).unwrap();
    assert_eq!(tag, program.bit_result_tag);
    let values = read_exact_sequence(&f.store, payload).unwrap();
    assert_eq!(values.len(), 1);
    decode_bit(f, values[0])
}

fn bit_handles(
    f: &FullFixture,
    value: u32,
) -> Vec<Handle> {
    (0..WIDTH)
        .map(|bit| {
            if (value >> bit) & 1 == 1 {
                f.one
            } else {
                f.zero
            }
        })
        .collect()
}

fn decode_word(
    f: &FullFixture,
    word: Handle,
) -> u32 {
    let bits = read_exact_sequence(&f.store, word).unwrap();
    assert_eq!(bits.len(), WIDTH);

    let mut value = 0u32;
    for (i, bit) in bits.into_iter().enumerate() {
        value |= u32::from(decode_bit(f, bit)) << i;
    }
    value
}

fn run_mux32(
    f: &mut FullFixture,
    program: &MuxProgram,
    s: u8,
    a: u32,
    b: u32,
) -> u32 {
    let bits = [f.zero, f.one];
    let a_bits = bit_handles(f, a);
    let b_bits = bit_handles(f, b);
    let aword =
        materialize_exact_sequence(&mut f.store, &a_bits).unwrap();
    let bword =
        materialize_exact_sequence(&mut f.store, &b_bits).unwrap();
    let args = materialize_exact_sequence(
        &mut f.store,
        &[bits[s as usize], aword, bword],
    )
    .unwrap();
    let invocation =
        call(&mut f.store, f.apply, program.mux32, args);
    let initial = f.store.ensure_pair(f.k, invocation).unwrap();

    f.engine.set_current(&f.store, &[initial]).unwrap();
    run_exact_steps(f, program.mux32_steps, "MUX32");

    let final_link = f.engine.current()[0];
    let (caller, endpoint) = f.store.poles(final_link).unwrap();
    assert_eq!(caller, f.k);
    let (tag, payload) = f.store.poles(endpoint).unwrap();
    assert_eq!(tag, program.word_result_tag);
    let values = read_exact_sequence(&f.store, payload).unwrap();
    assert_eq!(values.len(), 1);
    decode_word(f, values[0])
}

fn word_vectors() -> Vec<(u32, u32)> {
    let mut out = vec![
        (0, 0),
        (0, u32::MAX),
        (u32::MAX, 0),
        (u32::MAX, u32::MAX),
        (0xaaaa_aaaa, 0x5555_5555),
        (0x8000_0001, 0x7fff_fffe),
        (0x1234_5678, 0x9abc_def0),
    ];

    let mut z = 0x85eb_ca6bu32;
    for _ in 0..5 {
        z = z.wrapping_mul(1664525).wrapping_add(1013904223);
        let a = z;
        z = z.wrapping_mul(1664525).wrapping_add(1013904223);
        out.push((a, z));
    }

    out
}


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WebMuxOutcome {
    pub(crate) value: u32,
    pub(crate) reactions: u32,
    pub(crate) links_after_build: u32,
    pub(crate) links_after_first: u32,
    pub(crate) steady_link_delta: u32,
    pub(crate) quiescent: u8,
}

pub(crate) fn web_run_mux32(
    select: u32,
    a: u32,
    b: u32,
) -> Option<WebMuxOutcome> {
    if select > 1 {
        return None;
    }
    let mut f = FullFixture::new();
    let program = MuxProgram::install(&mut f);
    let links_after_build = f.store.link_count() as u32;
    let first = run_mux32(&mut f, &program, select as u8, a, b);
    let links_after_first = f.store.link_count() as u32;
    let second = run_mux32(&mut f, &program, select as u8, a, b);
    assert_eq!(second, first, "web MUX32 repeat changed result");
    let links_after_second = f.store.link_count() as u32;

    Some(WebMuxOutcome {
        value: first,
        reactions: program.mux32_steps as u32,
        links_after_build,
        links_after_first,
        steady_link_delta: links_after_second - links_after_first,
        quiescent: 1,
    })
}


#[test]
fn m1_mux1_direct_and_composed_all_rows() {
    let mut f = FullFixture::new();
    let program = MuxProgram::install(&mut f);

    assert_eq!(program.direct_steps, 1);
    assert_eq!(program.mux1_steps, 7);

    for s in 0u8..=1 {
        for a in 0u8..=1 {
            for b in 0u8..=1 {
                let direct = run_mux1(
                    &mut f,
                    &program,
                    program.direct_mux1,
                    program.direct_steps,
                    s,
                    a,
                    b,
                );
                let composed = run_mux1(
                    &mut f,
                    &program,
                    program.mux1,
                    program.mux1_steps,
                    s,
                    a,
                    b,
                );
                let expected = if s == 0 { a } else { b };
                assert_eq!(direct, expected);
                assert_eq!(composed, expected);
                assert_eq!(direct, composed);
            }
        }
    }
}

#[test]
#[ignore = "heavy composed MUX32 suite; mandatory release workflow"]
fn m1_mux32_composed_selects_canonical_word() {
    let mut f = FullFixture::new();
    let program = MuxProgram::install(&mut f);

    assert_eq!(program.mux32_steps, 257);

    let vectors = word_vectors();
    for &(a, b) in &vectors {
        let select_a = run_mux32(&mut f, &program, 0, a, b);
        let select_b = run_mux32(&mut f, &program, 1, a, b);
        assert_eq!(select_a, a);
        assert_eq!(select_b, b);
    }

    println!(
        "M1_MUX32 vectors={} reactions={} program_links={} xor={} and={}",
        vectors.len() * 2,
        program.mux32_steps,
        program.links_after_build,
        program.gates.xor2,
        program.gates.and2,
    );
}

#[test]
#[ignore = "heavy composed MUX32 suite; mandatory release workflow"]
fn m1_mux32_steady_state_has_zero_link_growth() {
    let mut f = FullFixture::new();
    let program = MuxProgram::install(&mut f);
    let a = 0x1357_9bdfu32;
    let b = 0x2468_ace0u32;

    let first = run_mux32(&mut f, &program, 1, a, b);
    assert_eq!(first, b);

    let links = f.store.link_count();
    let second = run_mux32(&mut f, &program, 1, a, b);
    assert_eq!(second, first);
    assert_eq!(
        f.store.link_count(),
        links,
        "repeated identical MUX32 materialized new Links"
    );
}
