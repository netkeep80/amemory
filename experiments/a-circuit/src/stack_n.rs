use super::{
    architectural_state_n::{
        decode_state_in_store, state_from_value, state_link,
        ArchitecturalStateProgram, ArchitecturalStateSchema, StateValue,
    },
    arithmetic_n::ArithmeticProgram,
    full_adder::{
        call, define_bundle_rule, index_rule_for, Fixture as FullFixture,
    },
    memory_word_n::MemoryWordProgram,
    proof_n::{
        execute_to_quiescence, identical_rerun, load_runtime, loaded_handle,
        prepare_stage, semantic_source, theory_admissions, visual_snapshot,
        WebProofResultStage, WebStructuralProof,
    },
};
use amemory_optimized_cpu_probe::{
    structural::{materialize_exact_sequence, read_exact_sequence},
    Handle, OptimizedLinkStore,
};

#[derive(Clone, Copy)]
struct StateHandles {
    eax: Handle,
    ebx: Handle,
    edx: Handle,
    cf: Handle,
    pf: Handle,
    af: Handle,
    zf: Handle,
    sf: Handle,
    of: Handle,
    ecx: Handle,
    esi: Handle,
    edi: Handle,
    ebp: Handle,
    esp: Handle,
    eip: Handle,
    memory_root: Handle,
}

impl StateHandles {
    fn fresh(
        anchors: &mut AnchorGen,
        store: &mut OptimizedLinkStore,
    ) -> Self {
        Self {
            eax: anchors.next(store),
            ebx: anchors.next(store),
            edx: anchors.next(store),
            cf: anchors.next(store),
            pf: anchors.next(store),
            af: anchors.next(store),
            zf: anchors.next(store),
            sf: anchors.next(store),
            of: anchors.next(store),
            ecx: anchors.next(store),
            esi: anchors.next(store),
            edi: anchors.next(store),
            ebp: anchors.next(store),
            esp: anchors.next(store),
            eip: anchors.next(store),
            memory_root: anchors.next(store),
        }
    }

    fn roles(self) -> [Handle; 16] {
        [
            self.eax,
            self.ebx,
            self.edx,
            self.cf,
            self.pf,
            self.af,
            self.zf,
            self.sf,
            self.of,
            self.ecx,
            self.esi,
            self.edi,
            self.ebp,
            self.esp,
            self.eip,
            self.memory_root,
        ]
    }

    fn frame_values(self, caller: Handle) -> Vec<Handle> {
        let mut values = Vec::with_capacity(17);
        values.push(caller);
        values.extend_from_slice(&self.roles());
        values
    }

    fn state(
        self,
        store: &mut OptimizedLinkStore,
        schema: ArchitecturalStateSchema,
        esp: Handle,
        memory_root: Handle,
    ) -> Handle {
        state_link(
            store,
            schema,
            self.eax,
            self.ebx,
            self.edx,
            self.cf,
            self.pf,
            self.af,
            self.zf,
            self.sf,
            self.of,
            self.ecx,
            self.esi,
            self.edi,
            self.ebp,
            esp,
            self.eip,
            memory_root,
        )
    }
}

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
        for pole in [c, c, o, c, o, c, o, o, c, o, c, c, o, o, c, c] {
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
}

fn frame(
    store: &mut OptimizedLinkStore,
    tag: Handle,
    values: &[Handle],
) -> Handle {
    let payload = materialize_exact_sequence(store, values).unwrap();
    let descriptor = store.ensure_pair(tag, payload).unwrap();
    store.ensure_start_self_closed(descriptor).unwrap()
}

fn tagged(
    store: &mut OptimizedLinkStore,
    tag: Handle,
    value: Handle,
) -> Handle {
    store.ensure_pair(tag, value).unwrap()
}

fn call_with(
    f: &mut FullFixture,
    function: Handle,
    values: &[Handle],
) -> Handle {
    let args = materialize_exact_sequence(&mut f.store, values).unwrap();
    call(&mut f.store, f.apply, function, args)
}

fn extend_roles(
    caller: Handle,
    state: StateHandles,
    extra: &[Handle],
) -> Vec<Handle> {
    let mut roles = Vec::with_capacity(17 + extra.len());
    roles.push(caller);
    roles.extend_from_slice(&state.roles());
    roles.extend_from_slice(extra);
    roles
}

fn exact_bits(
    f: &mut FullFixture,
    width: usize,
    value: u32,
) -> Handle {
    let values = (0..width)
        .map(|bit| {
            if (value >> bit) & 1 == 1 {
                f.one
            } else {
                f.zero
            }
        })
        .collect::<Vec<_>>();
    materialize_exact_sequence(&mut f.store, &values).unwrap()
}

fn decode_exact_store(
    store: &OptimizedLinkStore,
    zero: Handle,
    one: Handle,
    width: usize,
    value: Handle,
) -> Option<u32> {
    let bits = read_exact_sequence(store, value).ok()?;
    if bits.len() != width {
        return None;
    }
    let mut out = 0u32;
    for (index, bit) in bits.into_iter().enumerate() {
        if bit == one {
            out |= 1u32 << index;
        } else if bit != zero {
            return None;
        }
    }
    Some(out)
}

fn arithmetic_result(
    f: &mut FullFixture,
    tag: Handle,
    value: Handle,
    status: Handle,
    aux: Handle,
    sign_in: Handle,
    final_raw: Handle,
    mode: Handle,
) -> Handle {
    let payload = materialize_exact_sequence(
        &mut f.store,
        &[value, status, aux, sign_in, final_raw, mode],
    )
    .unwrap();
    tagged(&mut f.store, tag, payload)
}

#[derive(Clone, Debug)]
pub(crate) struct StackProgram {
    pub(crate) state: ArchitecturalStateProgram,
    pub(crate) memory: MemoryWordProgram,
    pub(crate) arithmetic: ArithmeticProgram,
    pub(crate) push32: Handle,
    pub(crate) pop32: Handle,
    pub(crate) roundtrip: Handle,
    pub(crate) push_result_tag: Handle,
    pub(crate) pop_result_tag: Handle,
    pub(crate) roundtrip_result_tag: Handle,
    word_four: Handle,
    push_arith_return_tag: Handle,
    push_write_return_tag: Handle,
    pop_read_return_tag: Handle,
    pop_arith_return_tag: Handle,
    roundtrip_push_return_tag: Handle,
    roundtrip_pop_return_tag: Handle,
}

impl StackProgram {
    pub(crate) fn install(f: &mut FullFixture) -> Self {
        let state = ArchitecturalStateProgram::install(f);
        let memory = MemoryWordProgram::install(f);
        let arithmetic = ArithmeticProgram::install(f, 32);
        let word_four = exact_bits(f, 32, 4);

        let namespace = f
            .store
            .ensure_pair(state.schema.state_tag, memory.write32)
            .unwrap();
        let seed = f
            .store
            .ensure_pair(namespace, arithmetic.arithmetic)
            .unwrap();
        let mut anchors = AnchorGen::new(&mut f.store, seed, f.o, f.c);

        let mut function = || {
            let left = anchors.next(&mut f.store);
            let right = anchors.next(&mut f.store);
            f.store.ensure_pair(left, right).unwrap()
        };
        let push32 = function();
        let pop32 = function();
        let roundtrip = function();

        let push_result_tag = anchors.next(&mut f.store);
        let pop_result_tag = anchors.next(&mut f.store);
        let roundtrip_result_tag = anchors.next(&mut f.store);
        let push_arith_return_tag = anchors.next(&mut f.store);
        let push_write_return_tag = anchors.next(&mut f.store);
        let pop_read_return_tag = anchors.next(&mut f.store);
        let pop_arith_return_tag = anchors.next(&mut f.store);
        let roundtrip_push_return_tag = anchors.next(&mut f.store);
        let roundtrip_pop_return_tag = anchors.next(&mut f.store);

        let program = Self {
            state,
            memory,
            arithmetic,
            push32,
            pop32,
            roundtrip,
            push_result_tag,
            pop_result_tag,
            roundtrip_result_tag,
            word_four,
            push_arith_return_tag,
            push_write_return_tag,
            pop_read_return_tag,
            pop_arith_return_tag,
            roundtrip_push_return_tag,
            roundtrip_pop_return_tag,
        };
        program.install_push_rules(f, &mut anchors);
        program.install_pop_rules(f, &mut anchors);
        program.install_roundtrip_rules(f, &mut anchors);
        program
    }

    fn install_push_rules(
        &self,
        f: &mut FullFixture,
        anchors: &mut AnchorGen,
    ) {
        // PUSH32 opens by computing ESP-4 through accepted structural
        // 32-bit arithmetic. The host never constructs ESP'.
        {
            let k = anchors.next(&mut f.store);
            let state = StateHandles::fresh(anchors, &mut f.store);
            let word = anchors.next(&mut f.store);
            let before_state = state.state(
                &mut f.store,
                self.state.schema,
                state.esp,
                state.memory_root,
            );
            let invocation =
                call_with(f, self.push32, &[before_state, word]);
            let before = f.store.ensure_pair(k, invocation).unwrap();

            let mut values = state.frame_values(k);
            values.push(word);
            let continuation = frame(
                &mut f.store,
                self.push_arith_return_tag,
                &values,
            );
            let arithmetic = call_with(
                f,
                self.arithmetic.arithmetic,
                &[state.esp, self.word_four, f.zero, f.one],
            );
            let after =
                f.store.ensure_pair(continuation, arithmetic).unwrap();

            let roles = extend_roles(k, state, &[word]);
            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        // Arithmetic SUB result supplies ESP'. Status taps are structurally
        // produced but stack address formation only consumes the result word.
        {
            let k = anchors.next(&mut f.store);
            let state = StateHandles::fresh(anchors, &mut f.store);
            let word = anchors.next(&mut f.store);
            let next_esp = anchors.next(&mut f.store);
            let status = anchors.next(&mut f.store);
            let aux = anchors.next(&mut f.store);
            let sign_in = anchors.next(&mut f.store);
            let final_raw = anchors.next(&mut f.store);

            let mut values = state.frame_values(k);
            values.push(word);
            let continuation = frame(
                &mut f.store,
                self.push_arith_return_tag,
                &values,
            );
            let result = arithmetic_result(
                f,
                self.arithmetic.result_tag,
                next_esp,
                status,
                aux,
                sign_in,
                final_raw,
                f.one,
            );
            let before =
                f.store.ensure_pair(continuation, result).unwrap();

            let mut write_values = state.frame_values(k);
            write_values.extend_from_slice(&[word, next_esp]);
            let write_continuation = frame(
                &mut f.store,
                self.push_write_return_tag,
                &write_values,
            );
            let write = call_with(
                f,
                self.memory.write32,
                &[state.memory_root, next_esp, word],
            );
            let after =
                f.store.ensure_pair(write_continuation, write).unwrap();

            let roles = extend_roles(
                k,
                state,
                &[word, next_esp, status, aux, sign_in, final_raw],
            );
            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &[self.arithmetic.result_tag],
                admission,
            );
        }

        // WRITE32 result is the only MemoryRoot published into State'.
        {
            let k = anchors.next(&mut f.store);
            let state = StateHandles::fresh(anchors, &mut f.store);
            let word = anchors.next(&mut f.store);
            let next_esp = anchors.next(&mut f.store);
            let new_root = anchors.next(&mut f.store);

            let mut values = state.frame_values(k);
            values.extend_from_slice(&[word, next_esp]);
            let continuation = frame(
                &mut f.store,
                self.push_write_return_tag,
                &values,
            );
            let write_result = tagged(
                &mut f.store,
                self.memory.write32_result_tag,
                new_root,
            );
            let before =
                f.store.ensure_pair(continuation, write_result).unwrap();

            let successor = state.state(
                &mut f.store,
                self.state.schema,
                next_esp,
                new_root,
            );
            let payload = materialize_exact_sequence(
                &mut f.store,
                &[successor, word],
            )
            .unwrap();
            let result =
                tagged(&mut f.store, self.push_result_tag, payload);
            let after = f.store.ensure_pair(k, result).unwrap();

            let roles =
                extend_roles(k, state, &[word, next_esp, new_root]);
            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &[self.memory.write32_result_tag],
                admission,
            );
        }
    }

    fn install_pop_rules(
        &self,
        f: &mut FullFixture,
        anchors: &mut AnchorGen,
    ) {
        // POP32 reads the current top-of-stack before changing ESP.
        {
            let k = anchors.next(&mut f.store);
            let state = StateHandles::fresh(anchors, &mut f.store);
            let before_state = state.state(
                &mut f.store,
                self.state.schema,
                state.esp,
                state.memory_root,
            );
            let invocation = call_with(f, self.pop32, &[before_state]);
            let before = f.store.ensure_pair(k, invocation).unwrap();

            let continuation = frame(
                &mut f.store,
                self.pop_read_return_tag,
                &state.frame_values(k),
            );
            let read = call_with(
                f,
                self.memory.read32,
                &[state.memory_root, state.esp],
            );
            let after = f.store.ensure_pair(continuation, read).unwrap();

            let roles = extend_roles(k, state, &[]);
            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        // READ32 result is carried while structural arithmetic computes ESP+4.
        {
            let k = anchors.next(&mut f.store);
            let state = StateHandles::fresh(anchors, &mut f.store);
            let word = anchors.next(&mut f.store);

            let continuation = frame(
                &mut f.store,
                self.pop_read_return_tag,
                &state.frame_values(k),
            );
            let read_result = tagged(
                &mut f.store,
                self.memory.read32_result_tag,
                word,
            );
            let before =
                f.store.ensure_pair(continuation, read_result).unwrap();

            let mut values = state.frame_values(k);
            values.push(word);
            let arithmetic_continuation = frame(
                &mut f.store,
                self.pop_arith_return_tag,
                &values,
            );
            let arithmetic = call_with(
                f,
                self.arithmetic.arithmetic,
                &[state.esp, self.word_four, f.zero, f.zero],
            );
            let after = f
                .store
                .ensure_pair(arithmetic_continuation, arithmetic)
                .unwrap();

            let roles = extend_roles(k, state, &[word]);
            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &[self.memory.read32_result_tag],
                admission,
            );
        }

        // ADD result publishes State' with exact same MemoryRoot and popped word.
        {
            let k = anchors.next(&mut f.store);
            let state = StateHandles::fresh(anchors, &mut f.store);
            let word = anchors.next(&mut f.store);
            let next_esp = anchors.next(&mut f.store);
            let status = anchors.next(&mut f.store);
            let aux = anchors.next(&mut f.store);
            let sign_in = anchors.next(&mut f.store);
            let final_raw = anchors.next(&mut f.store);

            let mut values = state.frame_values(k);
            values.push(word);
            let continuation = frame(
                &mut f.store,
                self.pop_arith_return_tag,
                &values,
            );
            let arithmetic_result = arithmetic_result(
                f,
                self.arithmetic.result_tag,
                next_esp,
                status,
                aux,
                sign_in,
                final_raw,
                f.zero,
            );
            let before =
                f.store.ensure_pair(continuation, arithmetic_result).unwrap();

            let successor = state.state(
                &mut f.store,
                self.state.schema,
                next_esp,
                state.memory_root,
            );
            let payload = materialize_exact_sequence(
                &mut f.store,
                &[successor, word],
            )
            .unwrap();
            let result =
                tagged(&mut f.store, self.pop_result_tag, payload);
            let after = f.store.ensure_pair(k, result).unwrap();

            let roles = extend_roles(
                k,
                state,
                &[word, next_esp, status, aux, sign_in, final_raw],
            );
            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &[self.arithmetic.result_tag],
                admission,
            );
        }
    }

    fn install_roundtrip_rules(
        &self,
        f: &mut FullFixture,
        anchors: &mut AnchorGen,
    ) {
        // Proof composition: PUSH32 followed by POP32 in the same active Scope.
        {
            let k = anchors.next(&mut f.store);
            let state = anchors.next(&mut f.store);
            let word = anchors.next(&mut f.store);
            let invocation =
                call_with(f, self.roundtrip, &[state, word]);
            let before = f.store.ensure_pair(k, invocation).unwrap();

            let continuation = frame(
                &mut f.store,
                self.roundtrip_push_return_tag,
                &[k, word],
            );
            let push = call_with(f, self.push32, &[state, word]);
            let after = f.store.ensure_pair(continuation, push).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, state, word],
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        {
            let k = anchors.next(&mut f.store);
            let expected = anchors.next(&mut f.store);
            let pushed_state = anchors.next(&mut f.store);
            let pushed_word = anchors.next(&mut f.store);

            let continuation = frame(
                &mut f.store,
                self.roundtrip_push_return_tag,
                &[k, expected],
            );
            let payload = materialize_exact_sequence(
                &mut f.store,
                &[pushed_state, pushed_word],
            )
            .unwrap();
            let push_result =
                tagged(&mut f.store, self.push_result_tag, payload);
            let before =
                f.store.ensure_pair(continuation, push_result).unwrap();

            let pop_continuation = frame(
                &mut f.store,
                self.roundtrip_pop_return_tag,
                &[k, expected],
            );
            let pop = call_with(f, self.pop32, &[pushed_state]);
            let after =
                f.store.ensure_pair(pop_continuation, pop).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, expected, pushed_state, pushed_word],
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &[self.push_result_tag],
                admission,
            );
        }

        {
            let k = anchors.next(&mut f.store);
            let expected = anchors.next(&mut f.store);
            let final_state = anchors.next(&mut f.store);
            let popped = anchors.next(&mut f.store);

            let continuation = frame(
                &mut f.store,
                self.roundtrip_pop_return_tag,
                &[k, expected],
            );
            let payload = materialize_exact_sequence(
                &mut f.store,
                &[final_state, popped],
            )
            .unwrap();
            let pop_result =
                tagged(&mut f.store, self.pop_result_tag, payload);
            let before =
                f.store.ensure_pair(continuation, pop_result).unwrap();

            let final_payload = materialize_exact_sequence(
                &mut f.store,
                &[final_state, popped],
            )
            .unwrap();
            let result = tagged(
                &mut f.store,
                self.roundtrip_result_tag,
                final_payload,
            );
            let after = f.store.ensure_pair(k, result).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, expected, final_state, popped],
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &[self.pop_result_tag],
                admission,
            );
        }
    }
}

fn state_schema_loaded(
    schema: ArchitecturalStateSchema,
    max: Handle,
) -> bool {
    [
        schema.state_tag,
        schema.eax,
        schema.ebx,
        schema.edx,
        schema.ecx,
        schema.esi,
        schema.edi,
        schema.ebp,
        schema.esp,
        schema.eip,
        schema.memory_root,
        schema.undefined,
        schema.flags.cf,
        schema.flags.pf,
        schema.flags.af,
        schema.flags.zf,
        schema.flags.sf,
        schema.flags.of,
    ]
    .into_iter()
    .all(|handle| handle >= 1 && handle <= max)
}

fn preserved_except_esp_and_memory(
    before: StateValue,
    after: StateValue,
) -> bool {
    before.eax == after.eax
        && before.ebx == after.ebx
        && before.edx == after.edx
        && before.ecx == after.ecx
        && before.esi == after.esi
        && before.edi == after.edi
        && before.ebp == after.ebp
        && before.eip == after.eip
        && before.cf == after.cf
        && before.pf == after.pf
        && before.af == after.af
        && before.zf == after.zf
        && before.sf == after.sf
        && before.of == after.of
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WebStackOutcome {
    pub(crate) esp_before: u32,
    pub(crate) esp_after: u32,
    pub(crate) value: u32,
    pub(crate) initial_memory_root_ref: Handle,
    pub(crate) final_memory_root_ref: Handle,
    pub(crate) state_preserved: u8,
    pub(crate) old_state_retained: u8,
    pub(crate) old_memory_retained: u8,
    pub(crate) atomic_scope: u8,
    pub(crate) reactions: u32,
    pub(crate) links_after_load: u32,
    pub(crate) links_final: u32,
    pub(crate) steady_link_delta: u32,
    pub(crate) quiescent: u8,
}

#[derive(Clone, Debug)]
pub(crate) struct WebStackExecution {
    pub(crate) outcome: WebStackOutcome,
    pub(crate) proof: WebStructuralProof,
}

pub(crate) fn web_prove_stack_roundtrip(
    esp_value: u32,
    value: u32,
) -> Option<WebStackExecution> {
    let mut compiler = FullFixture::new();
    let program = StackProgram::install(&mut compiler);

    let before_value = StateValue {
        eax: 0x1122_3344,
        ebx: 0xaabb_ccdd,
        edx: 0x5566_7788,
        ecx: 0x0102_0304,
        esi: 0x1111_2222,
        edi: 0x3333_4444,
        ebp: 0x5555_6666,
        esp: esp_value,
        eip: 0x0040_1000,
        memory_root: program.memory.memory.zero_root,
        cf: Some(1),
        pf: Some(0),
        af: Some(1),
        zf: Some(0),
        sf: Some(1),
        of: Some(0),
    };
    let before_state =
        state_from_value(&mut compiler, program.state.schema, before_value)?;
    let word = exact_bits(&mut compiler, 32, value);
    let invocation =
        call_with(&mut compiler, program.roundtrip, &[before_state, word]);
    let initial =
        compiler.store.ensure_pair(compiler.k, invocation).ok()?;

    let schema = program.state.schema;
    let prepared_roots = vec![
        semantic_source(&compiler.store, "function.stack.push32", program.push32),
        semantic_source(&compiler.store, "function.stack.pop32", program.pop32),
        semantic_source(
            &compiler.store,
            "function.stack.roundtrip",
            program.roundtrip,
        ),
        semantic_source(
            &compiler.store,
            "result.stack.push",
            program.push_result_tag,
        ),
        semantic_source(
            &compiler.store,
            "result.stack.pop",
            program.pop_result_tag,
        ),
        semantic_source(
            &compiler.store,
            "result.stack.roundtrip",
            program.roundtrip_result_tag,
        ),
        semantic_source(
            &compiler.store,
            "function.memory.read32",
            program.memory.read32,
        ),
        semantic_source(
            &compiler.store,
            "function.memory.write32",
            program.memory.write32,
        ),
        semantic_source(
            &compiler.store,
            "function.arithmetic32",
            program.arithmetic.arithmetic,
        ),
        semantic_source(
            &compiler.store,
            "result.arithmetic32",
            program.arithmetic.result_tag,
        ),
        semantic_source(
            &compiler.store,
            "memory.zero_root",
            program.memory.memory.zero_root,
        ),
        semantic_source(
            &compiler.store,
            "constant.word32.four",
            program.word_four,
        ),
        semantic_source(
            &compiler.store,
            "state.schema.tag",
            schema.state_tag,
        ),
        semantic_source(&compiler.store, "state.register.eax", schema.eax),
        semantic_source(&compiler.store, "state.register.ebx", schema.ebx),
        semantic_source(&compiler.store, "state.register.edx", schema.edx),
        semantic_source(&compiler.store, "state.register.ecx", schema.ecx),
        semantic_source(&compiler.store, "state.register.esi", schema.esi),
        semantic_source(&compiler.store, "state.register.edi", schema.edi),
        semantic_source(&compiler.store, "state.register.ebp", schema.ebp),
        semantic_source(&compiler.store, "state.register.esp", schema.esp),
        semantic_source(&compiler.store, "state.eip", schema.eip),
        semantic_source(
            &compiler.store,
            "state.memory_root_id",
            schema.memory_root,
        ),
        semantic_source(&compiler.store, "state.before", before_state),
        semantic_source(&compiler.store, "data.word32", word),
        semantic_source(&compiler.store, "data.bit.zero", compiler.zero),
        semantic_source(&compiler.store, "data.bit.one", compiler.one),
        semantic_source(
            &compiler.store,
            "execution.interpreter",
            compiler.interpreter,
        ),
        semantic_source(
            &compiler.store,
            "execution.theory",
            compiler.theory,
        ),
        semantic_source(&compiler.store, "execution.apply", compiler.apply),
        semantic_source(&compiler.store, "scope.initial", initial),
        semantic_source(&compiler.store, "context.result", compiler.k),
    ];

    let admissions =
        theory_admissions(&compiler.store, compiler.theory)?;
    let prepare =
        prepare_stage(&compiler.store, prepared_roots, admissions);
    let (mut memory, load) = load_runtime(&prepare)?;
    if !state_schema_loaded(schema, load.links_after_load) {
        return None;
    }

    let interpreter =
        loaded_handle(&load, "execution.interpreter")?;
    let initial = loaded_handle(&load, "scope.initial")?;
    let result_context =
        loaded_handle(&load, "context.result")?;
    let result_tag =
        loaded_handle(&load, "result.stack.roundtrip")?;
    let old_state = loaded_handle(&load, "state.before")?;
    let old_root = loaded_handle(&load, "memory.zero_root")?;
    let zero = loaded_handle(&load, "data.bit.zero")?;
    let one = loaded_handle(&load, "data.bit.one")?;

    let (mut engine, execute) = execute_to_quiescence(
        &mut memory,
        interpreter,
        initial,
        64,
        8192,
    )?;
    if !execute.final_quiescent
        || engine.current().len() != 1
        || !execute.reactions.iter().all(|step| {
            step.scope_before.len() == 1
                && step.scope_after.len() == 1
        })
    {
        return None;
    }

    let final_link = engine.current()[0];
    let (caller, envelope) = memory.store.poles(final_link).ok()?;
    if caller != result_context {
        return None;
    }
    let (tag, payload) = memory.store.poles(envelope).ok()?;
    if tag != result_tag {
        return None;
    }
    let values = read_exact_sequence(&memory.store, payload).ok()?;
    if values.len() != 2 {
        return None;
    }
    let state_after = values[0];
    let popped = decode_exact_store(
        &memory.store,
        zero,
        one,
        32,
        values[1],
    )?;
    let actual = decode_state_in_store(
        &memory.store,
        schema,
        state_after,
        zero,
        one,
    )?;

    let preserved =
        preserved_except_esp_and_memory(before_value, actual)
            && actual.esp == esp_value;
    let old_state_retained = memory.store.is_valid(old_state);
    let old_memory_retained = memory.store.is_valid(old_root);
    let atomic_scope = execute.reactions.iter().all(|step| {
        step.scope_before.len() == 1 && step.scope_after.len() == 1
    });

    let result_recursive_wire =
        memory.store.export_anum(final_link).ok()?;
    let result_sequence_anum =
        memory.store.export_anum(payload).ok()?;
    let identical_rerun_link_delta = identical_rerun(
        &mut memory,
        &mut engine,
        initial,
        &result_recursive_wire,
        8192,
    )?;

    let oracle_matches =
        popped == value
            && actual.esp == esp_value
            && actual.memory_root != old_root
            && preserved
            && old_state_retained
            && old_memory_retained
            && atomic_scope
            && identical_rerun_link_delta == 0;

    let visual_links =
        visual_snapshot(&memory, &load.semantic_roots);
    let proof = WebStructuralProof {
        schema_version: 4,
        block: "M6D3_STACK_ROUNDTRIP".to_owned(),
        prepare,
        load,
        execute,
        result: WebProofResultStage {
            memory_instance_id: memory.id.clone(),
            result_anum: result_recursive_wire,
            result_sequence_anum,
            decoded_value: popped,
            decoded_value_hi: Some(actual.esp),
            oracle_value: value,
            oracle_value_hi: Some(esp_value),
            oracle_matches,
            links_final: memory.store.link_count() as u32,
            identical_rerun_link_delta,
            visual_links,
        },
    };

    Some(WebStackExecution {
        outcome: WebStackOutcome {
            esp_before: esp_value,
            esp_after: actual.esp,
            value: popped,
            initial_memory_root_ref: old_root,
            final_memory_root_ref: actual.memory_root,
            state_preserved: u8::from(preserved),
            old_state_retained: u8::from(old_state_retained),
            old_memory_retained: u8::from(old_memory_retained),
            atomic_scope: u8::from(atomic_scope),
            reactions: proof.execute.active_reaction_count,
            links_after_load: proof.load.links_after_load,
            links_final: proof.result.links_final,
            steady_link_delta:
                proof.result.identical_rerun_link_delta,
            quiescent: u8::from(proof.execute.final_quiescent),
        },
        proof,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invoke(
        f: &mut FullFixture,
        function: Handle,
        args: &[Handle],
    ) -> Option<Handle> {
        let invocation = call_with(f, function, args);
        let initial = f.store.ensure_pair(f.k, invocation).ok()?;
        f.engine.set_current(&f.store, &[initial]).ok()?;
        for _ in 0..8192 {
            let reaction = f.engine.run(&mut f.store).ok()?;
            if reaction.quiescent {
                break;
            }
        }
        if !f.engine.quiescent() || f.engine.current().len() != 1 {
            return None;
        }
        let final_link = f.engine.current()[0];
        let (caller, result) = f.store.poles(final_link).ok()?;
        (caller == f.k).then_some(result)
    }

    fn parse_stack_result(
        f: &FullFixture,
        schema: ArchitecturalStateSchema,
        envelope: Handle,
        expected_tag: Handle,
    ) -> Option<(StateValue, u32)> {
        let (tag, payload) = f.store.poles(envelope).ok()?;
        if tag != expected_tag {
            return None;
        }
        let values = read_exact_sequence(&f.store, payload).ok()?;
        if values.len() != 2 {
            return None;
        }
        let state = decode_state_in_store(
            &f.store,
            schema,
            values[0],
            f.zero,
            f.one,
        )?;
        let word =
            decode_exact_store(&f.store, f.zero, f.one, 32, values[1])?;
        Some((state, word))
    }

    fn read_byte(
        f: &mut FullFixture,
        p: &StackProgram,
        root: Handle,
        address_value: u32,
    ) -> Option<u8> {
        let address = exact_bits(f, 32, address_value);
        let envelope = invoke(
            f,
            p.memory.memory.read,
            &[root, address],
        )?;
        let (tag, byte) = f.store.poles(envelope).ok()?;
        if tag != p.memory.memory.read_result_tag {
            return None;
        }
        decode_exact_store(&f.store, f.zero, f.one, 8, byte)
            .map(|v| v as u8)
    }

    fn base_state(
        p: &StackProgram,
        esp: u32,
    ) -> StateValue {
        StateValue {
            eax: 0x1122_3344,
            ebx: 0xaabb_ccdd,
            edx: 0x5566_7788,
            ecx: 0x0102_0304,
            esi: 0x1111_2222,
            edi: 0x3333_4444,
            ebp: 0x5555_6666,
            esp,
            eip: 0x0040_1000,
            memory_root: p.memory.memory.zero_root,
            cf: Some(1),
            pf: Some(0),
            af: Some(1),
            zf: Some(0),
            sf: Some(1),
            of: Some(0),
        }
    }

    #[test]
    fn m6d3_cross_page_push_pop_roundtrip_is_compact_and_atomic() {
        let execution =
            web_prove_stack_roundtrip(0x0000_0103, 0x1234_5678)
                .unwrap();
        assert_eq!(execution.outcome.esp_before, 0x0000_0103);
        assert_eq!(execution.outcome.esp_after, 0x0000_0103);
        assert_eq!(execution.outcome.value, 0x1234_5678);
        assert_ne!(
            execution.outcome.initial_memory_root_ref,
            execution.outcome.final_memory_root_ref
        );
        assert_eq!(execution.outcome.state_preserved, 1);
        assert_eq!(execution.outcome.old_state_retained, 1);
        assert_eq!(execution.outcome.old_memory_retained, 1);
        assert_eq!(execution.outcome.atomic_scope, 1);
        assert_eq!(execution.outcome.steady_link_delta, 0);
        assert_eq!(execution.outcome.quiescent, 1);
        assert!(execution.proof.result.oracle_matches);
        assert_eq!(execution.proof.block, "M6D3_STACK_ROUNDTRIP");
        assert!(execution.proof.compact().is_some());
    }

    #[test]
    fn m6d3_push_uses_new_esp_and_little_endian_word32() {
        let mut f = FullFixture::new();
        let p = StackProgram::install(&mut f);
        let before_value = base_state(&p, 0x0000_0103);
        let before =
            state_from_value(&mut f, p.state.schema, before_value).unwrap();
        let word = exact_bits(&mut f, 32, 0x1234_5678);

        let push = invoke(&mut f, p.push32, &[before, word]).unwrap();
        let (pushed, echoed) = parse_stack_result(
            &f,
            p.state.schema,
            push,
            p.push_result_tag,
        )
        .unwrap();
        assert_eq!(echoed, 0x1234_5678);
        assert_eq!(pushed.esp, 0x0000_00ff);
        assert_ne!(pushed.memory_root, before_value.memory_root);
        assert!(preserved_except_esp_and_memory(before_value, pushed));

        assert_eq!(
            [
                read_byte(&mut f, &p, pushed.memory_root, 0x0000_00ff)
                    .unwrap(),
                read_byte(&mut f, &p, pushed.memory_root, 0x0000_0100)
                    .unwrap(),
                read_byte(&mut f, &p, pushed.memory_root, 0x0000_0101)
                    .unwrap(),
                read_byte(&mut f, &p, pushed.memory_root, 0x0000_0102)
                    .unwrap(),
            ],
            [0x78, 0x56, 0x34, 0x12],
        );

        let pop = invoke(
            &mut f,
            p.pop32,
            &[state_from_value(&mut f, p.state.schema, pushed).unwrap()],
        )
        .unwrap();
        let (popped, value) = parse_stack_result(
            &f,
            p.state.schema,
            pop,
            p.pop_result_tag,
        )
        .unwrap();
        assert_eq!(value, 0x1234_5678);
        assert_eq!(popped.esp, 0x0000_0103);
        assert_eq!(popped.memory_root, pushed.memory_root);
        assert!(preserved_except_esp_and_memory(pushed, popped));
    }

    #[test]
    fn m6d3_stack_arithmetic_wraps_modulo_32_bits() {
        let mut f = FullFixture::new();
        let p = StackProgram::install(&mut f);
        let before_value = base_state(&p, 2);
        let before =
            state_from_value(&mut f, p.state.schema, before_value).unwrap();
        let word = exact_bits(&mut f, 32, 0x89ab_cdef);

        let push = invoke(&mut f, p.push32, &[before, word]).unwrap();
        let (pushed, _) = parse_stack_result(
            &f,
            p.state.schema,
            push,
            p.push_result_tag,
        )
        .unwrap();
        assert_eq!(pushed.esp, 0xffff_fffe);

        let pushed_state =
            state_from_value(&mut f, p.state.schema, pushed).unwrap();
        let pop = invoke(&mut f, p.pop32, &[pushed_state]).unwrap();
        let (popped, value) = parse_stack_result(
            &f,
            p.state.schema,
            pop,
            p.pop_result_tag,
        )
        .unwrap();
        assert_eq!(value, 0x89ab_cdef);
        assert_eq!(popped.esp, 2);
        assert_eq!(popped.memory_root, pushed.memory_root);
    }

    #[test]
    fn m6d3_malformed_state_and_word_fail_closed() {
        let mut f = FullFixture::new();
        let p = StackProgram::install(&mut f);
        let word = exact_bits(&mut f, 32, 7);

        assert!(
            invoke(
                &mut f,
                p.push32,
                &[p.state.schema.memory_detached, word],
            )
            .and_then(|result| {
                parse_stack_result(
                    &f,
                    p.state.schema,
                    result,
                    p.push_result_tag,
                )
            })
            .is_none()
        );

        let before_value = base_state(&p, 0x100);
        let before =
            state_from_value(&mut f, p.state.schema, before_value).unwrap();
        let malformed_word = exact_bits(&mut f, 31, 7);
        assert!(
            invoke(&mut f, p.push32, &[before, malformed_word])
                .and_then(|result| {
                    parse_stack_result(
                        &f,
                        p.state.schema,
                        result,
                        p.push_result_tag,
                    )
                })
                .is_none()
        );
    }
}
