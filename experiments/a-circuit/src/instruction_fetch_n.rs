use super::{
    architectural_state_n::{
        decode_state_in_store, state_from_value, state_link,
        ArchitecturalStateProgram, ArchitecturalStateSchema, StateValue,
    },
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
        eip: Handle,
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
            self.esp,
            eip,
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
        for pole in [o, c, c, o, c, o, o, c, c, c, o, c, o, o, c, o] {
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

#[derive(Clone, Debug)]
pub(crate) struct InstructionFetchProgram {
    pub(crate) state: ArchitecturalStateProgram,
    pub(crate) memory: MemoryWordProgram,
    pub(crate) fetch: Handle,
    pub(crate) seed_write_fetch: Handle,
    pub(crate) fetch_result_tag: Handle,
    read_return_tag: Handle,
    next_return_tag: Handle,
    seed_write_return_tag: Handle,
}

impl InstructionFetchProgram {
    pub(crate) fn install(f: &mut FullFixture) -> Self {
        let state = ArchitecturalStateProgram::install(f);
        let memory = MemoryWordProgram::install(f);
        let namespace = f
            .store
            .ensure_pair(state.schema.state_tag, memory.address_next.function)
            .unwrap();
        let seed = f
            .store
            .ensure_pair(namespace, memory.memory.read)
            .unwrap();
        let mut anchors = AnchorGen::new(&mut f.store, seed, f.o, f.c);

        let fetch = {
            let a = anchors.next(&mut f.store);
            let b = anchors.next(&mut f.store);
            f.store.ensure_pair(a, b).unwrap()
        };
        let seed_write_fetch = {
            let a = anchors.next(&mut f.store);
            let b = anchors.next(&mut f.store);
            f.store.ensure_pair(a, b).unwrap()
        };
        let fetch_result_tag = anchors.next(&mut f.store);
        let read_return_tag = anchors.next(&mut f.store);
        let next_return_tag = anchors.next(&mut f.store);
        let seed_write_return_tag = anchors.next(&mut f.store);

        let program = Self {
            state,
            memory,
            fetch,
            seed_write_fetch,
            fetch_result_tag,
            read_return_tag,
            next_return_tag,
            seed_write_return_tag,
        };
        program.install_fetch_rules(f, &mut anchors);
        program.install_seed_write_rules(f, &mut anchors);
        program
    }

    fn install_fetch_rules(
        &self,
        f: &mut FullFixture,
        anchors: &mut AnchorGen,
    ) {
        // FETCH(State_t) structurally exposes the State fields and calls
        // READ8(MemoryRoot, EIP). No host-selected address or root participates.
        {
            let k = anchors.next(&mut f.store);
            let state = StateHandles::fresh(anchors, &mut f.store);
            let before_state = state.state(
                &mut f.store,
                self.state.schema,
                state.eip,
                state.memory_root,
            );
            let invocation = call_with(f, self.fetch, &[before_state]);
            let before = f.store.ensure_pair(k, invocation).unwrap();

            let continuation = frame(
                &mut f.store,
                self.read_return_tag,
                &state.frame_values(k),
            );
            let read = call_with(
                f,
                self.memory.memory.read,
                &[state.memory_root, state.eip],
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

        // READ8 result is carried forward while AddressNext computes EIP+1.
        {
            let k = anchors.next(&mut f.store);
            let state = StateHandles::fresh(anchors, &mut f.store);
            let byte = anchors.next(&mut f.store);

            let continuation = frame(
                &mut f.store,
                self.read_return_tag,
                &state.frame_values(k),
            );
            let read_result = tagged(
                &mut f.store,
                self.memory.memory.read_result_tag,
                byte,
            );
            let before =
                f.store.ensure_pair(continuation, read_result).unwrap();

            let mut next_values = state.frame_values(k);
            next_values.push(byte);
            let next_continuation =
                frame(&mut f.store, self.next_return_tag, &next_values);
            let next =
                call_with(f, self.memory.address_next.function, &[state.eip]);
            let after =
                f.store.ensure_pair(next_continuation, next).unwrap();

            let roles = extend_roles(k, state, &[byte]);
            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &[self.memory.memory.read_result_tag],
                admission,
            );
        }

        // AddressNext result atomically publishes one FetchResult envelope:
        // FETCH_RESULT([State_t+1, Byte8]).
        {
            let k = anchors.next(&mut f.store);
            let state = StateHandles::fresh(anchors, &mut f.store);
            let byte = anchors.next(&mut f.store);
            let next_eip = anchors.next(&mut f.store);

            let mut next_values = state.frame_values(k);
            next_values.push(byte);
            let continuation =
                frame(&mut f.store, self.next_return_tag, &next_values);
            let next_result = tagged(
                &mut f.store,
                self.memory.address_next.result_tag,
                next_eip,
            );
            let before =
                f.store.ensure_pair(continuation, next_result).unwrap();

            let successor = state.state(
                &mut f.store,
                self.state.schema,
                next_eip,
                state.memory_root,
            );
            let payload = materialize_exact_sequence(
                &mut f.store,
                &[successor, byte],
            )
            .unwrap();
            let result =
                tagged(&mut f.store, self.fetch_result_tag, payload);
            let after = f.store.ensure_pair(k, result).unwrap();

            let roles =
                extend_roles(k, state, &[byte, next_eip]);
            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &[self.memory.address_next.result_tag],
                admission,
            );
        }
    }

    fn install_seed_write_rules(
        &self,
        f: &mut FullFixture,
        anchors: &mut AnchorGen,
    ) {
        // Proof-only composition entry: seed one instruction byte through the
        // accepted structural WRITE8 path, then FETCH from the produced root.
        {
            let k = anchors.next(&mut f.store);
            let state = StateHandles::fresh(anchors, &mut f.store);
            let byte = anchors.next(&mut f.store);
            let before_state = state.state(
                &mut f.store,
                self.state.schema,
                state.eip,
                state.memory_root,
            );
            let invocation =
                call_with(f, self.seed_write_fetch, &[before_state, byte]);
            let before = f.store.ensure_pair(k, invocation).unwrap();

            let mut seed_values = state.frame_values(k);
            seed_values.push(byte);
            let continuation = frame(
                &mut f.store,
                self.seed_write_return_tag,
                &seed_values,
            );
            let write = call_with(
                f,
                self.memory.memory.write,
                &[state.memory_root, state.eip, byte],
            );
            let after = f.store.ensure_pair(continuation, write).unwrap();

            let roles = extend_roles(k, state, &[byte]);
            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        // WRITE8 result becomes the MemoryRoot of a freshly constructed State;
        // only then is normal FETCH invoked. The host never sees or injects the
        // intermediate post-write root.
        {
            let k = anchors.next(&mut f.store);
            let state = StateHandles::fresh(anchors, &mut f.store);
            let byte = anchors.next(&mut f.store);
            let new_root = anchors.next(&mut f.store);

            let mut seed_values = state.frame_values(k);
            seed_values.push(byte);
            let continuation = frame(
                &mut f.store,
                self.seed_write_return_tag,
                &seed_values,
            );
            let write_result = tagged(
                &mut f.store,
                self.memory.memory.write_result_tag,
                new_root,
            );
            let before =
                f.store.ensure_pair(continuation, write_result).unwrap();

            let seeded_state = state.state(
                &mut f.store,
                self.state.schema,
                state.eip,
                new_root,
            );
            let fetch = call_with(f, self.fetch, &[seeded_state]);
            let after = f.store.ensure_pair(k, fetch).unwrap();

            let roles =
                extend_roles(k, state, &[byte, new_root]);
            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &[self.memory.memory.write_result_tag],
                admission,
            );
        }
    }
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

fn state_schema_loaded(
    schema: ArchitecturalStateSchema,
    max: Handle,
) -> bool {
    [
        schema.state_tag,
        schema.apply_state,
        schema.apply_wide_state,
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
        schema.memory_detached,
        schema.undefined,
        schema.flags.set_tag,
        schema.flags.undefined_tag,
        schema.flags.cf,
        schema.flags.pf,
        schema.flags.af,
        schema.flags.zf,
        schema.flags.sf,
        schema.flags.of,
        schema.effect_result_tag,
        schema.wide_effect_result_tag,
    ]
    .into_iter()
    .all(|handle| handle >= 1 && handle <= max)
}

fn preserved_architecture(
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
        && before.esp == after.esp
        && before.cf == after.cf
        && before.pf == after.pf
        && before.af == after.af
        && before.zf == after.zf
        && before.sf == after.sf
        && before.of == after.of
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WebInstructionFetchOutcome {
    pub(crate) eip_before: u32,
    pub(crate) eip_after: u32,
    pub(crate) fetched_byte: u32,
    pub(crate) initial_memory_root_ref: Handle,
    pub(crate) final_memory_root_ref: Handle,
    pub(crate) seeded_write: u8,
    pub(crate) state_preserved: u8,
    pub(crate) old_state_retained: u8,
    pub(crate) atomic_scope: u8,
    pub(crate) reactions: u32,
    pub(crate) links_after_load: u32,
    pub(crate) links_final: u32,
    pub(crate) steady_link_delta: u32,
    pub(crate) quiescent: u8,
}

#[derive(Clone, Debug)]
pub(crate) struct WebInstructionFetchExecution {
    pub(crate) outcome: WebInstructionFetchOutcome,
    pub(crate) proof: WebStructuralProof,
}

pub(crate) fn web_prove_instruction_fetch(
    eip_value: u32,
    instruction_byte: u8,
    seed_write: bool,
) -> Option<WebInstructionFetchExecution> {
    if !seed_write && instruction_byte != 0 {
        return None;
    }

    let mut compiler = FullFixture::new();
    let program = InstructionFetchProgram::install(&mut compiler);

    let before_value = StateValue {
        eax: 0x1122_3344,
        ebx: 0xaabb_ccdd,
        edx: 0x5566_7788,
        ecx: 0x0102_0304,
        esi: 0x1111_2222,
        edi: 0x3333_4444,
        ebp: 0x5555_6666,
        esp: 0x7777_8888,
        eip: eip_value,
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
    let byte = exact_bits(&mut compiler, 8, instruction_byte as u32);

    let invocation = if seed_write {
        call_with(
            &mut compiler,
            program.seed_write_fetch,
            &[before_state, byte],
        )
    } else {
        call_with(&mut compiler, program.fetch, &[before_state])
    };
    let initial =
        compiler.store.ensure_pair(compiler.k, invocation).ok()?;

    let schema = program.state.schema;
    let prepared_roots = vec![
        semantic_source(&compiler.store, "function.fetch", program.fetch),
        semantic_source(
            &compiler.store,
            "function.seed_write_fetch",
            program.seed_write_fetch,
        ),
        semantic_source(
            &compiler.store,
            "result.fetch_tag",
            program.fetch_result_tag,
        ),
        semantic_source(
            &compiler.store,
            "function.memory.read8",
            program.memory.memory.read,
        ),
        semantic_source(
            &compiler.store,
            "function.memory.write8",
            program.memory.memory.write,
        ),
        semantic_source(
            &compiler.store,
            "result.memory.read8",
            program.memory.memory.read_result_tag,
        ),
        semantic_source(
            &compiler.store,
            "result.memory.write8",
            program.memory.memory.write_result_tag,
        ),
        semantic_source(
            &compiler.store,
            "function.address_next",
            program.memory.address_next.function,
        ),
        semantic_source(
            &compiler.store,
            "result.address_next",
            program.memory.address_next.result_tag,
        ),
        semantic_source(
            &compiler.store,
            "memory.zero_root",
            program.memory.memory.zero_root,
        ),
        semantic_source(
            &compiler.store,
            "state.schema.tag",
            schema.state_tag,
        ),
        semantic_source(
            &compiler.store,
            "state.register.eax",
            schema.eax,
        ),
        semantic_source(
            &compiler.store,
            "state.register.ebx",
            schema.ebx,
        ),
        semantic_source(
            &compiler.store,
            "state.register.edx",
            schema.edx,
        ),
        semantic_source(
            &compiler.store,
            "state.register.ecx",
            schema.ecx,
        ),
        semantic_source(
            &compiler.store,
            "state.register.esi",
            schema.esi,
        ),
        semantic_source(
            &compiler.store,
            "state.register.edi",
            schema.edi,
        ),
        semantic_source(
            &compiler.store,
            "state.register.ebp",
            schema.ebp,
        ),
        semantic_source(
            &compiler.store,
            "state.register.esp",
            schema.esp,
        ),
        semantic_source(
            &compiler.store,
            "state.eip",
            schema.eip,
        ),
        semantic_source(
            &compiler.store,
            "state.memory_root_id",
            schema.memory_root,
        ),
        semantic_source(
            &compiler.store,
            "state.flag.cf",
            schema.flags.cf,
        ),
        semantic_source(
            &compiler.store,
            "state.flag.pf",
            schema.flags.pf,
        ),
        semantic_source(
            &compiler.store,
            "state.flag.af",
            schema.flags.af,
        ),
        semantic_source(
            &compiler.store,
            "state.flag.zf",
            schema.flags.zf,
        ),
        semantic_source(
            &compiler.store,
            "state.flag.sf",
            schema.flags.sf,
        ),
        semantic_source(
            &compiler.store,
            "state.flag.of",
            schema.flags.of,
        ),
        semantic_source(
            &compiler.store,
            "state.flag.undefined",
            schema.undefined,
        ),
        semantic_source(
            &compiler.store,
            "state.before",
            before_state,
        ),
        semantic_source(
            &compiler.store,
            "data.instruction_byte",
            byte,
        ),
        semantic_source(
            &compiler.store,
            "data.bit.zero",
            compiler.zero,
        ),
        semantic_source(
            &compiler.store,
            "data.bit.one",
            compiler.one,
        ),
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
        semantic_source(
            &compiler.store,
            "execution.apply",
            compiler.apply,
        ),
        semantic_source(
            &compiler.store,
            "scope.initial",
            initial,
        ),
        semantic_source(
            &compiler.store,
            "context.result",
            compiler.k,
        ),
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
    let fetch_result_tag =
        loaded_handle(&load, "result.fetch_tag")?;
    let old_state = loaded_handle(&load, "state.before")?;
    let zero_root = loaded_handle(&load, "memory.zero_root")?;
    let zero = loaded_handle(&load, "data.bit.zero")?;
    let one = loaded_handle(&load, "data.bit.one")?;

    let (mut engine, execute) = execute_to_quiescence(
        &mut memory,
        interpreter,
        initial,
        64,
        4096,
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
    if tag != fetch_result_tag {
        return None;
    }
    let values = read_exact_sequence(&memory.store, payload).ok()?;
    if values.len() != 2 {
        return None;
    }
    let state_after = values[0];
    let fetched = decode_exact_store(
        &memory.store,
        zero,
        one,
        8,
        values[1],
    )?;
    let actual = decode_state_in_store(
        &memory.store,
        schema,
        state_after,
        zero,
        one,
    )?;

    let expected_eip = eip_value.wrapping_add(1);
    let architecture_preserved =
        preserved_architecture(before_value, actual);
    let root_contract = if seed_write && instruction_byte != 0 {
        actual.memory_root != zero_root
    } else {
        actual.memory_root == zero_root
    };
    let old_state_retained = memory.store.is_valid(old_state);
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
        4096,
    )?;

    let oracle_matches =
        fetched == instruction_byte as u32
            && actual.eip == expected_eip
            && architecture_preserved
            && root_contract
            && old_state_retained
            && atomic_scope
            && identical_rerun_link_delta == 0;

    let visual_links =
        visual_snapshot(&memory, &load.semantic_roots);
    let proof = WebStructuralProof {
        schema_version: 4,
        block: if seed_write {
            "M6D2_FETCH_SEEDED".to_owned()
        } else {
            "M6D2_FETCH_ZERO".to_owned()
        },
        prepare,
        load,
        execute,
        result: WebProofResultStage {
            memory_instance_id: memory.id.clone(),
            result_anum: result_recursive_wire,
            result_sequence_anum,
            decoded_value: fetched,
            decoded_value_hi: Some(actual.eip),
            oracle_value: instruction_byte as u32,
            oracle_value_hi: Some(expected_eip),
            oracle_matches,
            links_final: memory.store.link_count() as u32,
            identical_rerun_link_delta,
            visual_links,
        },
    };

    Some(WebInstructionFetchExecution {
        outcome: WebInstructionFetchOutcome {
            eip_before: eip_value,
            eip_after: actual.eip,
            fetched_byte: fetched,
            initial_memory_root_ref: zero_root,
            final_memory_root_ref: actual.memory_root,
            seeded_write: u8::from(seed_write),
            state_preserved: u8::from(architecture_preserved),
            old_state_retained: u8::from(old_state_retained),
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

    fn final_fetch_result(
        f: &FullFixture,
        p: &InstructionFetchProgram,
    ) -> Option<Handle> {
        if f.engine.current().len() != 1 {
            return None;
        }
        let final_link = f.engine.current()[0];
        let (caller, envelope) = f.store.poles(final_link).ok()?;
        if caller != f.k {
            return None;
        }
        let (tag, _payload) = f.store.poles(envelope).ok()?;
        (tag == p.fetch_result_tag).then_some(envelope)
    }

    fn run_until_quiescent(
        f: &mut FullFixture,
        initial: Handle,
    ) {
        f.engine.set_current(&f.store, &[initial]).unwrap();
        for _ in 0..4096 {
            let reaction = f.engine.run(&mut f.store).unwrap();
            if reaction.quiescent {
                return;
            }
        }
        panic!("M6d2 did not quiesce");
    }

    #[test]
    fn m6d2_zero_memory_fetch_advances_eip_and_preserves_state() {
        let execution =
            web_prove_instruction_fetch(0x0040_1000, 0, false).unwrap();
        assert_eq!(execution.outcome.fetched_byte, 0);
        assert_eq!(execution.outcome.eip_before, 0x0040_1000);
        assert_eq!(execution.outcome.eip_after, 0x0040_1001);
        assert_eq!(
            execution.outcome.final_memory_root_ref,
            execution.outcome.initial_memory_root_ref
        );
        assert_eq!(execution.outcome.state_preserved, 1);
        assert_eq!(execution.outcome.old_state_retained, 1);
        assert_eq!(execution.outcome.atomic_scope, 1);
        assert_eq!(execution.outcome.steady_link_delta, 0);
        assert_eq!(execution.outcome.quiescent, 1);
        assert!(execution.proof.result.oracle_matches);
        assert_eq!(execution.proof.block, "M6D2_FETCH_ZERO");
        assert!(execution.proof.compact().is_some());
    }

    #[test]
    fn m6d2_seed_write_fetches_nonzero_without_host_root_injection() {
        let execution =
            web_prove_instruction_fetch(0x0000_00ff, 0x90, true).unwrap();
        assert_eq!(execution.outcome.fetched_byte, 0x90);
        assert_eq!(execution.outcome.eip_after, 0x0000_0100);
        assert_ne!(
            execution.outcome.final_memory_root_ref,
            execution.outcome.initial_memory_root_ref
        );
        assert_eq!(execution.outcome.seeded_write, 1);
        assert_eq!(execution.outcome.state_preserved, 1);
        assert_eq!(execution.outcome.atomic_scope, 1);
        assert_eq!(execution.outcome.steady_link_delta, 0);
        assert!(execution.proof.result.oracle_matches);
        assert_eq!(execution.proof.block, "M6D2_FETCH_SEEDED");
        assert!(execution.proof.compact().is_some());
        assert!(execution.proof.execute.reactions.iter().all(|step| {
            step.scope_before.len() == 1
                && step.scope_after.len() == 1
        }));
    }

    #[test]
    fn m6d2_fetch_wraps_eip_modulo_32_bits() {
        let execution =
            web_prove_instruction_fetch(u32::MAX, 0, false).unwrap();
        assert_eq!(execution.outcome.eip_before, u32::MAX);
        assert_eq!(execution.outcome.eip_after, 0);
        assert_eq!(execution.outcome.fetched_byte, 0);
        assert_eq!(execution.outcome.state_preserved, 1);
        assert!(execution.proof.result.oracle_matches);
    }

    #[test]
    fn m6d2_malformed_state_and_memory_path_fail_closed() {
        let mut f = FullFixture::new();
        let p = InstructionFetchProgram::install(&mut f);

        let malformed_args =
            materialize_exact_sequence(
                &mut f.store,
                &[p.state.schema.memory_detached],
            )
            .unwrap();
        let malformed_call =
            call(&mut f.store, f.apply, p.fetch, malformed_args);
        let malformed_initial =
            f.store.ensure_pair(f.k, malformed_call).unwrap();
        run_until_quiescent(&mut f, malformed_initial);
        assert!(final_fetch_result(&f, &p).is_none());

        let detached = StateValue {
            eax: 1,
            ebx: 2,
            edx: 3,
            ecx: 4,
            esi: 5,
            edi: 6,
            ebp: 7,
            esp: 8,
            eip: 0x100,
            memory_root: p.state.schema.memory_detached,
            cf: Some(0),
            pf: Some(0),
            af: Some(0),
            zf: Some(0),
            sf: Some(0),
            of: Some(0),
        };
        let detached_state =
            state_from_value(&mut f, p.state.schema, detached).unwrap();
        let fetch = call_with(&mut f, p.fetch, &[detached_state]);
        let detached_initial = f.store.ensure_pair(f.k, fetch).unwrap();
        run_until_quiescent(&mut f, detached_initial);
        assert!(final_fetch_result(&f, &p).is_none());
    }
}
