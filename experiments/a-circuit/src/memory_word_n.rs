use super::{
    full_adder::{
        call, define_bundle_rule, index_rule_for, Fixture as FullFixture,
    },
    memory32_n::Memory32Program,
    proof_n::{
        execute_to_quiescence, identical_rerun, load_runtime, loaded_handle,
        prepare_stage, semantic_source, theory_admissions, visual_snapshot,
        ProofRuntimeMemory, WebProofResultStage, WebStructuralProof,
    },
};
use amemory_optimized_cpu_probe::{
    structural::{materialize_exact_sequence, read_exact_sequence},
    Handle, OptimizedLinkStore,
};

const ADDRESS_BITS: usize = 32;
const BYTE_BITS: usize = 8;

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
        for pole in [o,c,o,o,c,c,o,c,o,c,c,o,o,c,o,c,c,o] {
            seed = store.ensure_pair(seed, pole).unwrap();
        }
        Self { current: seed, flip: false, o, c }
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

fn seq_with_head(
    store: &mut OptimizedLinkStore,
    head: Handle,
    rest: &[Handle],
) -> Handle {
    let mut values = Vec::with_capacity(rest.len() + 1);
    values.push(head);
    values.extend_from_slice(rest);
    materialize_exact_sequence(store, &values).unwrap()
}

#[derive(Clone, Debug)]
pub(crate) struct AddressNextProgram {
    pub(crate) function: Handle,
    pub(crate) result_tag: Handle,
    carry_tag: Vec<Handle>,
    copy_tag: Vec<Handle>,
}

impl AddressNextProgram {
    fn install(f: &mut FullFixture, namespace: Handle) -> Self {
        let seed = f.store.ensure_pair(namespace, f.full).unwrap();
        let mut anchors = AnchorGen::new(&mut f.store, seed, f.o, f.c);
        let function = {
            let left = anchors.next(&mut f.store);
            let right = anchors.next(&mut f.store);
            f.store.ensure_pair(left, right).unwrap()
        };
        let result_tag = anchors.next(&mut f.store);
        let carry_tag = anchors.roles(&mut f.store, ADDRESS_BITS);
        let copy_tag = anchors.roles(&mut f.store, ADDRESS_BITS);

        let program = Self {
            function,
            result_tag,
            carry_tag,
            copy_tag,
        };
        program.install_rules(f, &mut anchors);
        program
    }

    fn install_rules(
        &self,
        f: &mut FullFixture,
        anchors: &mut AnchorGen,
    ) {
        // NEXT(Address32) begins with carry=1 at bit 0.
        {
            let k = anchors.next(&mut f.store);
            let address = anchors.next(&mut f.store);
            let args =
                materialize_exact_sequence(&mut f.store, &[address]).unwrap();
            let invocation =
                call(&mut f.store, f.apply, self.function, args);
            let before = f.store.ensure_pair(k, invocation).unwrap();

            let caller =
                frame(&mut f.store, self.carry_tag[0], &[k]);
            let endpoint =
                tagged(&mut f.store, self.carry_tag[0], address);
            let after = f.store.ensure_pair(caller, endpoint).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, address],
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        // Validate exactly 32 canonical bits while incrementing LSB-first.
        // carry-state: 0 -> output 1 and switch to copy, 1 -> output 0.
        // copy-state: output bit unchanged. Final carry is discarded, giving
        // natural modulo-2^32 wrap.
        for position in 0..ADDRESS_BITS {
            let rest_len = ADDRESS_BITS - position - 1;
            for carry in [true, false] {
                for input_one in [false, true] {
                    let k = anchors.next(&mut f.store);
                    let prefix = anchors.roles(&mut f.store, position);
                    let rest = anchors.roles(&mut f.store, rest_len);
                    let tag = if carry {
                        self.carry_tag[position]
                    } else {
                        self.copy_tag[position]
                    };
                    let input = if input_one { f.one } else { f.zero };

                    let mut state = Vec::with_capacity(1 + prefix.len());
                    state.push(k);
                    state.extend_from_slice(&prefix);
                    let caller = frame(&mut f.store, tag, &state);
                    let remaining =
                        seq_with_head(&mut f.store, input, &rest);
                    let endpoint =
                        tagged(&mut f.store, tag, remaining);
                    let before =
                        f.store.ensure_pair(caller, endpoint).unwrap();

                    let (output, next_carry) = if carry {
                        if input_one {
                            (f.zero, true)
                        } else {
                            (f.one, false)
                        }
                    } else {
                        (input, false)
                    };

                    let mut next_prefix =
                        Vec::with_capacity(prefix.len() + 1);
                    next_prefix.extend_from_slice(&prefix);
                    next_prefix.push(output);

                    let after = if position + 1 == ADDRESS_BITS {
                        let result_address =
                            materialize_exact_sequence(
                                &mut f.store,
                                &next_prefix,
                            )
                            .unwrap();
                        let result =
                            tagged(&mut f.store, self.result_tag, result_address);
                        f.store.ensure_pair(k, result).unwrap()
                    } else {
                        let next_tag = if next_carry {
                            self.carry_tag[position + 1]
                        } else {
                            self.copy_tag[position + 1]
                        };
                        let mut next_state =
                            Vec::with_capacity(1 + next_prefix.len());
                        next_state.push(k);
                        next_state.extend_from_slice(&next_prefix);
                        let next_caller =
                            frame(&mut f.store, next_tag, &next_state);
                        let next_remaining =
                            materialize_exact_sequence(
                                &mut f.store,
                                &rest,
                            )
                            .unwrap();
                        let next_endpoint =
                            tagged(&mut f.store, next_tag, next_remaining);
                        f.store
                            .ensure_pair(next_caller, next_endpoint)
                            .unwrap()
                    };

                    let mut roles =
                        Vec::with_capacity(1 + prefix.len() + rest.len());
                    roles.push(k);
                    roles.extend_from_slice(&prefix);
                    roles.extend_from_slice(&rest);
                    let (_, admission) = define_bundle_rule(
                        &mut f.store,
                        f.theory,
                        &roles,
                        before,
                        &[after],
                    );
                    index_rule_for(&mut f.store, &[tag], admission);
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
struct ReadPipeline {
    byte_tag: Vec<Handle>,
    next_tag: Vec<Handle>,
}

#[derive(Clone, Debug)]
struct WritePipeline {
    byte_tag: Vec<Handle>,
    next_tag: Vec<Handle>,
}

#[derive(Clone, Debug)]
pub(crate) struct MemoryWordProgram {
    pub(crate) memory: Memory32Program,
    pub(crate) address_next: AddressNextProgram,
    pub(crate) read16: Handle,
    pub(crate) write16: Handle,
    pub(crate) read32: Handle,
    pub(crate) write32: Handle,
    pub(crate) read16_result_tag: Handle,
    pub(crate) write16_result_tag: Handle,
    pub(crate) read32_result_tag: Handle,
    pub(crate) write32_result_tag: Handle,
}

impl MemoryWordProgram {
    pub(crate) fn install(f: &mut FullFixture) -> Self {
        let memory = Memory32Program::install(f);
        let namespace =
            f.store.ensure_pair(memory.read, memory.write).unwrap();
        let address_next = AddressNextProgram::install(f, namespace);

        let seed = f
            .store
            .ensure_pair(namespace, address_next.function)
            .unwrap();
        let mut anchors =
            AnchorGen::new(&mut f.store, seed, f.o, f.c);

        let mut function = || {
            let left = anchors.next(&mut f.store);
            let right = anchors.next(&mut f.store);
            f.store.ensure_pair(left, right).unwrap()
        };

        let read16 = function();
        let write16 = function();
        let read32 = function();
        let write32 = function();
        let read16_result_tag = anchors.next(&mut f.store);
        let write16_result_tag = anchors.next(&mut f.store);
        let read32_result_tag = anchors.next(&mut f.store);
        let write32_result_tag = anchors.next(&mut f.store);

        let read16_pipe = ReadPipeline {
            byte_tag: anchors.roles(&mut f.store, 2),
            next_tag: anchors.roles(&mut f.store, 1),
        };
        let write16_pipe = WritePipeline {
            byte_tag: anchors.roles(&mut f.store, 2),
            next_tag: anchors.roles(&mut f.store, 1),
        };
        let read32_pipe = ReadPipeline {
            byte_tag: anchors.roles(&mut f.store, 4),
            next_tag: anchors.roles(&mut f.store, 3),
        };
        let write32_pipe = WritePipeline {
            byte_tag: anchors.roles(&mut f.store, 4),
            next_tag: anchors.roles(&mut f.store, 3),
        };

        let program = Self {
            memory,
            address_next,
            read16,
            write16,
            read32,
            write32,
            read16_result_tag,
            write16_result_tag,
            read32_result_tag,
            write32_result_tag,
        };

        program.install_read(
            f, &mut anchors, read16, read16_result_tag, 2, &read16_pipe,
        );
        program.install_write(
            f, &mut anchors, write16, write16_result_tag, 2, &write16_pipe,
        );
        program.install_read(
            f, &mut anchors, read32, read32_result_tag, 4, &read32_pipe,
        );
        program.install_write(
            f, &mut anchors, write32, write32_result_tag, 4, &write32_pipe,
        );

        program
    }

    fn install_read(
        &self,
        f: &mut FullFixture,
        anchors: &mut AnchorGen,
        function: Handle,
        result_tag: Handle,
        byte_count: usize,
        pipeline: &ReadPipeline,
    ) {
        debug_assert_eq!(pipeline.byte_tag.len(), byte_count);
        debug_assert_eq!(pipeline.next_tag.len(), byte_count - 1);

        // Public read starts with the accepted byte-memory READ at A0.
        {
            let k = anchors.next(&mut f.store);
            let root = anchors.next(&mut f.store);
            let address = anchors.next(&mut f.store);
            let args =
                materialize_exact_sequence(&mut f.store, &[root, address])
                    .unwrap();
            let invocation =
                call(&mut f.store, f.apply, function, args);
            let before = f.store.ensure_pair(k, invocation).unwrap();

            let caller = frame(
                &mut f.store,
                pipeline.byte_tag[0],
                &[k, root, address],
            );
            let byte_args =
                materialize_exact_sequence(&mut f.store, &[root, address])
                    .unwrap();
            let byte_call =
                call(&mut f.store, f.apply, self.memory.read, byte_args);
            let after = f.store.ensure_pair(caller, byte_call).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, root, address],
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        for index in 0..byte_count {
            // Byte result is expanded to eight structural bit roles and
            // accumulated in little-endian order.
            let k = anchors.next(&mut f.store);
            let root = anchors.next(&mut f.store);
            let address = anchors.next(&mut f.store);
            let collected =
                anchors.roles(&mut f.store, index * BYTE_BITS);
            let byte_bits = anchors.roles(&mut f.store, BYTE_BITS);

            let mut state =
                Vec::with_capacity(3 + collected.len());
            state.extend_from_slice(&[k, root, address]);
            state.extend_from_slice(&collected);
            let caller =
                frame(&mut f.store, pipeline.byte_tag[index], &state);

            let byte =
                materialize_exact_sequence(&mut f.store, &byte_bits)
                    .unwrap();
            let byte_result =
                tagged(&mut f.store, self.memory.read_result_tag, byte);
            let before =
                f.store.ensure_pair(caller, byte_result).unwrap();

            let mut all_bits =
                Vec::with_capacity(collected.len() + BYTE_BITS);
            all_bits.extend_from_slice(&collected);
            all_bits.extend_from_slice(&byte_bits);

            let after = if index + 1 == byte_count {
                let word =
                    materialize_exact_sequence(&mut f.store, &all_bits)
                        .unwrap();
                let result =
                    tagged(&mut f.store, result_tag, word);
                f.store.ensure_pair(k, result).unwrap()
            } else {
                let mut next_state =
                    Vec::with_capacity(3 + all_bits.len());
                next_state.extend_from_slice(&[k, root, address]);
                next_state.extend_from_slice(&all_bits);
                let next_caller = frame(
                    &mut f.store,
                    pipeline.next_tag[index],
                    &next_state,
                );
                let next_args =
                    materialize_exact_sequence(&mut f.store, &[address])
                        .unwrap();
                let next_call = call(
                    &mut f.store,
                    f.apply,
                    self.address_next.function,
                    next_args,
                );
                f.store.ensure_pair(next_caller, next_call).unwrap()
            };

            let mut roles =
                Vec::with_capacity(3 + collected.len() + BYTE_BITS);
            roles.extend_from_slice(&[k, root, address]);
            roles.extend_from_slice(&collected);
            roles.extend_from_slice(&byte_bits);
            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &[self.memory.read_result_tag],
                admission,
            );

            if index + 1 < byte_count {
                self.install_read_next(
                    f,
                    anchors,
                    index,
                    root,
                    k,
                    &all_bits,
                    pipeline,
                );
            }
        }
    }

    fn install_read_next(
        &self,
        f: &mut FullFixture,
        anchors: &mut AnchorGen,
        index: usize,
        _template_root: Handle,
        _template_k: Handle,
        _template_bits: &[Handle],
        pipeline: &ReadPipeline,
    ) {
        // Fresh roles are required for the continuation rule. The template
        // handles supplied above describe only the previous rule and must not
        // leak as grounded constants.
        let k = anchors.next(&mut f.store);
        let root = anchors.next(&mut f.store);
        let old_address = anchors.next(&mut f.store);
        let collected =
            anchors.roles(&mut f.store, (index + 1) * BYTE_BITS);
        let next_address = anchors.next(&mut f.store);

        let mut state =
            Vec::with_capacity(3 + collected.len());
        state.extend_from_slice(&[k, root, old_address]);
        state.extend_from_slice(&collected);
        let caller =
            frame(&mut f.store, pipeline.next_tag[index], &state);

        let next_result = tagged(
            &mut f.store,
            self.address_next.result_tag,
            next_address,
        );
        let before =
            f.store.ensure_pair(caller, next_result).unwrap();

        let mut next_state =
            Vec::with_capacity(3 + collected.len());
        next_state.extend_from_slice(&[k, root, next_address]);
        next_state.extend_from_slice(&collected);
        let byte_caller = frame(
            &mut f.store,
            pipeline.byte_tag[index + 1],
            &next_state,
        );
        let args =
            materialize_exact_sequence(
                &mut f.store,
                &[root, next_address],
            )
            .unwrap();
        let byte_call =
            call(&mut f.store, f.apply, self.memory.read, args);
        let after =
            f.store.ensure_pair(byte_caller, byte_call).unwrap();

        let mut roles =
            Vec::with_capacity(4 + collected.len());
        roles.extend_from_slice(&[k, root, old_address]);
        roles.extend_from_slice(&collected);
        roles.push(next_address);
        let (_, admission) = define_bundle_rule(
            &mut f.store,
            f.theory,
            &roles,
            before,
            &[after],
        );
        index_rule_for(
            &mut f.store,
            &[self.address_next.result_tag],
            admission,
        );
    }

    fn install_write(
        &self,
        f: &mut FullFixture,
        anchors: &mut AnchorGen,
        function: Handle,
        result_tag: Handle,
        byte_count: usize,
        pipeline: &WritePipeline,
    ) {
        let bit_count = byte_count * BYTE_BITS;
        debug_assert_eq!(pipeline.byte_tag.len(), byte_count);
        debug_assert_eq!(pipeline.next_tag.len(), byte_count - 1);

        // ExactSequence length is matched structurally here. Canonical bit
        // validation is delegated byte-by-byte to M6b WRITE.
        {
            let k = anchors.next(&mut f.store);
            let root = anchors.next(&mut f.store);
            let address = anchors.next(&mut f.store);
            let word_bits = anchors.roles(&mut f.store, bit_count);
            let word =
                materialize_exact_sequence(&mut f.store, &word_bits)
                    .unwrap();
            let args = materialize_exact_sequence(
                &mut f.store,
                &[root, address, word],
            )
            .unwrap();
            let invocation =
                call(&mut f.store, f.apply, function, args);
            let before = f.store.ensure_pair(k, invocation).unwrap();

            let caller_state = {
                let mut values =
                    Vec::with_capacity(2 + word_bits.len());
                values.extend_from_slice(&[k, address]);
                values.extend_from_slice(&word_bits);
                values
            };
            let caller = frame(
                &mut f.store,
                pipeline.byte_tag[0],
                &caller_state,
            );
            let byte = materialize_exact_sequence(
                &mut f.store,
                &word_bits[..BYTE_BITS],
            )
            .unwrap();
            let byte_args = materialize_exact_sequence(
                &mut f.store,
                &[root, address, byte],
            )
            .unwrap();
            let byte_call =
                call(&mut f.store, f.apply, self.memory.write, byte_args);
            let after = f.store.ensure_pair(caller, byte_call).unwrap();

            let mut roles = Vec::with_capacity(3 + word_bits.len());
            roles.extend_from_slice(&[k, root, address]);
            roles.extend_from_slice(&word_bits);
            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        for index in 0..byte_count {
            let k = anchors.next(&mut f.store);
            let address = anchors.next(&mut f.store);
            let word_bits = anchors.roles(&mut f.store, bit_count);
            let new_root = anchors.next(&mut f.store);

            let mut state =
                Vec::with_capacity(2 + word_bits.len());
            state.extend_from_slice(&[k, address]);
            state.extend_from_slice(&word_bits);
            let caller =
                frame(&mut f.store, pipeline.byte_tag[index], &state);

            let byte_result = tagged(
                &mut f.store,
                self.memory.write_result_tag,
                new_root,
            );
            let before =
                f.store.ensure_pair(caller, byte_result).unwrap();

            let after = if index + 1 == byte_count {
                let result =
                    tagged(&mut f.store, result_tag, new_root);
                f.store.ensure_pair(k, result).unwrap()
            } else {
                let mut next_state =
                    Vec::with_capacity(3 + word_bits.len());
                next_state.extend_from_slice(&[k, new_root, address]);
                next_state.extend_from_slice(&word_bits);
                let next_caller = frame(
                    &mut f.store,
                    pipeline.next_tag[index],
                    &next_state,
                );
                let args =
                    materialize_exact_sequence(&mut f.store, &[address])
                        .unwrap();
                let next_call = call(
                    &mut f.store,
                    f.apply,
                    self.address_next.function,
                    args,
                );
                f.store.ensure_pair(next_caller, next_call).unwrap()
            };

            let mut roles =
                Vec::with_capacity(3 + word_bits.len());
            roles.extend_from_slice(&[k, address, new_root]);
            roles.extend_from_slice(&word_bits);
            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &[self.memory.write_result_tag],
                admission,
            );

            if index + 1 < byte_count {
                let k2 = anchors.next(&mut f.store);
                let root2 = anchors.next(&mut f.store);
                let old_address = anchors.next(&mut f.store);
                let bits2 = anchors.roles(&mut f.store, bit_count);
                let next_address = anchors.next(&mut f.store);

                let mut next_state =
                    Vec::with_capacity(3 + bits2.len());
                next_state.extend_from_slice(&[k2, root2, old_address]);
                next_state.extend_from_slice(&bits2);
                let next_caller = frame(
                    &mut f.store,
                    pipeline.next_tag[index],
                    &next_state,
                );
                let next_result = tagged(
                    &mut f.store,
                    self.address_next.result_tag,
                    next_address,
                );
                let next_before =
                    f.store.ensure_pair(next_caller, next_result).unwrap();

                let mut byte_state =
                    Vec::with_capacity(2 + bits2.len());
                byte_state.extend_from_slice(&[k2, next_address]);
                byte_state.extend_from_slice(&bits2);
                let byte_caller = frame(
                    &mut f.store,
                    pipeline.byte_tag[index + 1],
                    &byte_state,
                );
                let begin = (index + 1) * BYTE_BITS;
                let byte = materialize_exact_sequence(
                    &mut f.store,
                    &bits2[begin..begin + BYTE_BITS],
                )
                .unwrap();
                let byte_args = materialize_exact_sequence(
                    &mut f.store,
                    &[root2, next_address, byte],
                )
                .unwrap();
                let byte_call =
                    call(&mut f.store, f.apply, self.memory.write, byte_args);
                let next_after =
                    f.store.ensure_pair(byte_caller, byte_call).unwrap();

                let mut next_roles =
                    Vec::with_capacity(4 + bits2.len());
                next_roles.extend_from_slice(
                    &[k2, root2, old_address, next_address],
                );
                next_roles.extend_from_slice(&bits2);
                let (_, next_admission) = define_bundle_rule(
                    &mut f.store,
                    f.theory,
                    &next_roles,
                    next_before,
                    &[next_after],
                );
                index_rule_for(
                    &mut f.store,
                    &[self.address_next.result_tag],
                    next_admission,
                );
            }
        }
    }
}

fn bits(
    f: &FullFixture,
    width: usize,
    value: u32,
) -> Vec<Handle> {
    (0..width)
        .map(|bit| {
            if (value >> bit) & 1 == 1 { f.one } else { f.zero }
        })
        .collect()
}

fn exact_value(
    f: &mut FullFixture,
    width: usize,
    value: u32,
) -> Handle {
    let values = bits(f, width, value);
    materialize_exact_sequence(&mut f.store, &values).unwrap()
}

fn decode_exact(
    f: &FullFixture,
    width: usize,
    value: Handle,
) -> Option<u32> {
    let values = read_exact_sequence(&f.store, value).ok()?;
    if values.len() != width {
        return None;
    }
    let mut out = 0u32;
    for (bit, handle) in values.into_iter().enumerate() {
        if handle == f.one {
            out |= 1u32 << bit;
        } else if handle != f.zero {
            return None;
        }
    }
    Some(out)
}


fn decode_exact_store(
    store: &OptimizedLinkStore,
    zero: Handle,
    one: Handle,
    width: usize,
    value: Handle,
) -> Option<u32> {
    let values = read_exact_sequence(store, value).ok()?;
    if values.len() != width {
        return None;
    }
    let mut out = 0u32;
    for (bit, handle) in values.into_iter().enumerate() {
        if handle == one {
            out |= 1u32 << bit;
        } else if handle != zero {
            return None;
        }
    }
    Some(out)
}

fn runtime_read_word(
    memory: &mut ProofRuntimeMemory,
    interpreter: Handle,
    apply: Handle,
    function: Handle,
    result_tag: Handle,
    caller: Handle,
    root: Handle,
    address: Handle,
    zero: Handle,
    one: Handle,
    width: usize,
) -> Option<u32> {
    let args =
        materialize_exact_sequence(&mut memory.store, &[root, address])
            .ok()?;
    let invocation =
        call(&mut memory.store, apply, function, args);
    let initial =
        memory.store.ensure_pair(caller, invocation).ok()?;
    let (engine, execute) = execute_to_quiescence(
        memory,
        interpreter,
        initial,
        32,
        2048,
    )?;
    if !execute.final_quiescent || engine.current().len() != 1 {
        return None;
    }

    let final_link = engine.current()[0];
    let (final_caller, envelope) =
        memory.store.poles(final_link).ok()?;
    if final_caller != caller {
        return None;
    }
    let (tag, word) = memory.store.poles(envelope).ok()?;
    if tag != result_tag {
        return None;
    }
    decode_exact_store(
        &memory.store,
        zero,
        one,
        width,
        word,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WebWordMemoryOutcome {
    pub(crate) width: u32,
    pub(crate) address: u32,
    pub(crate) write_value: u32,
    pub(crate) before_value: u32,
    pub(crate) after_value: u32,
    pub(crate) old_after_value: u32,
    pub(crate) old_root_ref: u32,
    pub(crate) new_root_ref: u32,
    pub(crate) reactions: u32,
    pub(crate) links_after_load: u32,
    pub(crate) links_final: u32,
    pub(crate) steady_link_delta: u32,
    pub(crate) atomic_scope: u8,
    pub(crate) crosses_page: u8,
    pub(crate) quiescent: u8,
}

#[derive(Clone, Debug)]
pub(crate) struct WebWordMemoryExecution {
    pub(crate) outcome: WebWordMemoryOutcome,
    pub(crate) proof: WebStructuralProof,
}

fn web_prove_word_memory(
    address_value: u32,
    word_value: u32,
    width: usize,
) -> Option<WebWordMemoryExecution> {
    if !matches!(width, 16 | 32) {
        return None;
    }
    if width == 16 && word_value > 0xffff {
        return None;
    }

    let mut compiler = FullFixture::new();
    let program = MemoryWordProgram::install(&mut compiler);
    let (read, write, read_result_tag, write_result_tag, block_name) =
        if width == 16 {
            (
                program.read16,
                program.write16,
                program.read16_result_tag,
                program.write16_result_tag,
                "M6C_MEMORY_WORD16",
            )
        } else {
            (
                program.read32,
                program.write32,
                program.read32_result_tag,
                program.write32_result_tag,
                "M6C_MEMORY_WORD32",
            )
        };

    let address = exact_value(&mut compiler, 32, address_value);
    let word = exact_value(&mut compiler, width, word_value);
    let args = materialize_exact_sequence(
        &mut compiler.store,
        &[program.memory.zero_root, address, word],
    )
    .ok()?;
    let invocation =
        call(&mut compiler.store, compiler.apply, write, args);
    let initial =
        compiler.store.ensure_pair(compiler.k, invocation).ok()?;

    let prepared_roots = vec![
        semantic_source(
            &compiler.store,
            "function.memory_word.read",
            read,
        ),
        semantic_source(
            &compiler.store,
            "function.memory_word.write",
            write,
        ),
        semantic_source(
            &compiler.store,
            "function.memory_word.address_next",
            program.address_next.function,
        ),
        semantic_source(
            &compiler.store,
            "memory_word.zero_root",
            program.memory.zero_root,
        ),
        semantic_source(
            &compiler.store,
            "memory_word.read_result_tag",
            read_result_tag,
        ),
        semantic_source(
            &compiler.store,
            "memory_word.write_result_tag",
            write_result_tag,
        ),
        semantic_source(
            &compiler.store,
            "data.address32",
            address,
        ),
        semantic_source(
            &compiler.store,
            "data.word",
            word,
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

    let interpreter =
        loaded_handle(&load, "execution.interpreter")?;
    let apply = loaded_handle(&load, "execution.apply")?;
    let read =
        loaded_handle(&load, "function.memory_word.read")?;
    let initial = loaded_handle(&load, "scope.initial")?;
    let old_root =
        loaded_handle(&load, "memory_word.zero_root")?;
    let address = loaded_handle(&load, "data.address32")?;
    let read_result_tag =
        loaded_handle(&load, "memory_word.read_result_tag")?;
    let write_result_tag =
        loaded_handle(&load, "memory_word.write_result_tag")?;
    let zero = loaded_handle(&load, "data.bit.zero")?;
    let one = loaded_handle(&load, "data.bit.one")?;
    let result_context =
        loaded_handle(&load, "context.result")?;

    let (mut engine, execute) = execute_to_quiescence(
        &mut memory,
        interpreter,
        initial,
        32,
        2048,
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
    let (tag, new_root) = memory.store.poles(envelope).ok()?;
    if tag != write_result_tag
        || !memory.store.is_valid(old_root)
        || !memory.store.is_valid(new_root)
        || old_root == new_root
    {
        return None;
    }

    let result_recursive_wire =
        memory.store.export_anum(final_link).ok()?;
    let result_sequence_anum =
        memory.store.export_anum(address).ok()?;
    let identical_rerun_link_delta = identical_rerun(
        &mut memory,
        &mut engine,
        initial,
        &result_recursive_wire,
        2048,
    )?;

    // Verification uses a clone of the exact runtime carrier so read-back
    // cannot pollute authoritative compact-proof link accounting.
    let mut verification_memory = ProofRuntimeMemory {
        id: memory.id.clone(),
        store: memory.store.clone(),
    };
    let after_value = runtime_read_word(
        &mut verification_memory,
        interpreter,
        apply,
        read,
        read_result_tag,
        result_context,
        new_root,
        address,
        zero,
        one,
        width,
    )?;
    let old_after_value = runtime_read_word(
        &mut verification_memory,
        interpreter,
        apply,
        read,
        read_result_tag,
        result_context,
        old_root,
        address,
        zero,
        one,
        width,
    )?;

    let oracle_matches =
        after_value == word_value
            && old_after_value == 0
            && identical_rerun_link_delta == 0;

    let visual_links =
        visual_snapshot(&memory, &load.semantic_roots);
    let proof = WebStructuralProof {
        schema_version: 4,
        block: block_name.to_owned(),
        prepare,
        load,
        execute,
        result: WebProofResultStage {
            memory_instance_id: memory.id.clone(),
            result_anum: result_recursive_wire,
            result_sequence_anum,
            decoded_value: after_value,
            decoded_value_hi: Some(old_after_value),
            oracle_value: word_value,
            oracle_value_hi: Some(0),
            oracle_matches,
            links_final: memory.store.link_count() as u32,
            identical_rerun_link_delta,
            visual_links,
        },
    };

    let byte_count = width / 8;
    let end_offset =
        (address_value & 0xff) as usize + byte_count - 1;

    Some(WebWordMemoryExecution {
        outcome: WebWordMemoryOutcome {
            width: width as u32,
            address: address_value,
            write_value: word_value,
            before_value: 0,
            after_value,
            old_after_value,
            old_root_ref: old_root,
            new_root_ref: new_root,
            reactions: proof.execute.active_reaction_count,
            links_after_load: proof.load.links_after_load,
            links_final: proof.result.links_final,
            steady_link_delta:
                proof.result.identical_rerun_link_delta,
            atomic_scope: 1,
            crosses_page: u8::from(end_offset > 0xff),
            quiescent: u8::from(proof.execute.final_quiescent),
        },
        proof,
    })
}

pub(crate) fn web_prove_word16_memory(
    address_value: u32,
    word_value: u32,
) -> Option<WebWordMemoryExecution> {
    web_prove_word_memory(address_value, word_value, 16)
}

pub(crate) fn web_prove_word32_memory(
    address_value: u32,
    word_value: u32,
) -> Option<WebWordMemoryExecution> {
    web_prove_word_memory(address_value, word_value, 32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invoke(
        f: &mut FullFixture,
        function: Handle,
        args: &[Handle],
    ) -> Option<(Handle, usize, bool)> {
        let argument =
            materialize_exact_sequence(&mut f.store, args).ok()?;
        let invocation =
            call(&mut f.store, f.apply, function, argument);
        let initial = f.store.ensure_pair(f.k, invocation).ok()?;
        f.engine.set_current(&f.store, &[initial]).ok()?;

        let mut reactions = 0usize;
        let mut atomic = true;
        for _ in 0..4096 {
            if f.engine.current().len() != 1 {
                atomic = false;
            }
            let reaction = f.engine.run(&mut f.store).ok()?;
            if reaction.quiescent {
                break;
            }
            reactions += 1;
            if reaction.next_members.len() != 1 {
                atomic = false;
            }
        }
        if reactions == 0
            || !f.engine.quiescent()
            || f.engine.current().len() != 1
        {
            return None;
        }
        let final_member = f.engine.current()[0];
        let (caller, result) = f.store.poles(final_member).ok()?;
        (caller == f.k).then_some((result, reactions, atomic))
    }

    fn read_word(
        f: &mut FullFixture,
        p: &MemoryWordProgram,
        root: Handle,
        address_value: u32,
        width: usize,
    ) -> Option<u32> {
        let address = exact_value(f, 32, address_value);
        let (function, tag) = if width == 16 {
            (p.read16, p.read16_result_tag)
        } else {
            (p.read32, p.read32_result_tag)
        };
        let (envelope, _, _) =
            invoke(f, function, &[root, address])?;
        let (actual_tag, word) = f.store.poles(envelope).ok()?;
        if actual_tag != tag {
            return None;
        }
        decode_exact(f, width, word)
    }

    fn write_word(
        f: &mut FullFixture,
        p: &MemoryWordProgram,
        root: Handle,
        address_value: u32,
        width: usize,
        value: u32,
    ) -> Option<(Handle, usize, bool)> {
        let address = exact_value(f, 32, address_value);
        let word = exact_value(f, width, value);
        let (function, tag) = if width == 16 {
            (p.write16, p.write16_result_tag)
        } else {
            (p.write32, p.write32_result_tag)
        };
        let (envelope, reactions, atomic) =
            invoke(f, function, &[root, address, word])?;
        let (actual_tag, new_root) = f.store.poles(envelope).ok()?;
        (actual_tag == tag).then_some((new_root, reactions, atomic))
    }

    fn read_byte(
        f: &mut FullFixture,
        p: &MemoryWordProgram,
        root: Handle,
        address_value: u32,
    ) -> Option<u8> {
        let address = exact_value(f, 32, address_value);
        let (envelope, _, _) =
            invoke(f, p.memory.read, &[root, address])?;
        let (tag, byte) = f.store.poles(envelope).ok()?;
        if tag != p.memory.read_result_tag {
            return None;
        }
        decode_exact(f, 8, byte).map(|v| v as u8)
    }

    #[test]
    fn m6c_real_cross_page_word16_proof_is_atomic_and_compact() {
        let execution =
            web_prove_word16_memory(0x0000_00ff, 0xabcd)
                .unwrap();
        assert_eq!(execution.outcome.width, 16);
        assert_eq!(execution.outcome.address, 0x0000_00ff);
        assert_eq!(execution.outcome.after_value, 0xabcd);
        assert_eq!(execution.outcome.old_after_value, 0);
        assert_eq!(execution.outcome.crosses_page, 1);
        assert_eq!(execution.outcome.atomic_scope, 1);
        assert_eq!(execution.outcome.steady_link_delta, 0);
        assert_eq!(execution.proof.block, "M6C_MEMORY_WORD16");
        assert!(execution.proof.result.oracle_matches);
        assert!(execution.proof.compact().is_some());
    }

    #[test]
    fn m6c_real_cross_page_word32_proof_is_atomic_and_compact() {
        let execution =
            web_prove_word32_memory(0x0000_00fe, 0x1234_5678)
                .unwrap();
        assert_eq!(execution.outcome.width, 32);
        assert_eq!(execution.outcome.address, 0x0000_00fe);
        assert_eq!(execution.outcome.write_value, 0x1234_5678);
        assert_eq!(execution.outcome.after_value, 0x1234_5678);
        assert_eq!(execution.outcome.old_after_value, 0);
        assert_ne!(
            execution.outcome.old_root_ref,
            execution.outcome.new_root_ref
        );
        assert_eq!(execution.outcome.atomic_scope, 1);
        assert_eq!(execution.outcome.crosses_page, 1);
        assert_eq!(execution.outcome.steady_link_delta, 0);
        assert_eq!(execution.outcome.quiescent, 1);
        assert!(execution.outcome.reactions > 100);
        assert_eq!(execution.proof.block, "M6C_MEMORY_WORD32");
        assert!(execution.proof.result.oracle_matches);
        assert!(
            execution.proof.compact().is_some(),
            "M6c proof must remain inside compact-proof link accounting"
        );
        assert!(execution.proof.execute.reactions.iter().all(|step| {
            step.scope_before.len() == 1
                && step.scope_after.len() == 1
        }));
    }

    #[test]
    fn m6c_address_next_validates_and_wraps() {
        let mut f = FullFixture::new();
        let p = MemoryWordProgram::install(&mut f);
        for (input, expected) in [
            (0u32, 1u32),
            (0xfe, 0xff),
            (0xff, 0x100),
            (0x0012_34ff, 0x0012_3500),
            (u32::MAX, 0),
        ] {
            let address = exact_value(&mut f, 32, input);
            let (envelope, reactions, atomic) =
                invoke(&mut f, p.address_next.function, &[address]).unwrap();
            let (tag, result) = f.store.poles(envelope).unwrap();
            assert_eq!(tag, p.address_next.result_tag);
            assert_eq!(decode_exact(&f, 32, result), Some(expected));
            assert_eq!(reactions, 33);
            assert!(atomic);
        }

        let malformed = exact_value(&mut f, 16, 7);
        assert!(
            invoke(&mut f, p.address_next.function, &[malformed]).is_none()
        );
    }

    #[test]
    fn m6c_word_reads_writes_little_endian_cross_page_and_wrap() {
        let mut f = FullFixture::new();
        let p = MemoryWordProgram::install(&mut f);
        let zero = p.memory.zero_root;

        assert_eq!(read_word(&mut f, &p, zero, 0x100, 16), Some(0));
        assert_eq!(read_word(&mut f, &p, zero, 0x100, 32), Some(0));

        let (root1, reactions1, atomic1) =
            write_word(&mut f, &p, zero, 0x0000_00fe, 32, 0x1234_5678)
                .unwrap();
        assert!(reactions1 > 100);
        assert!(atomic1);
        assert_eq!(
            read_word(&mut f, &p, root1, 0x0000_00fe, 32),
            Some(0x1234_5678)
        );
        assert_eq!(
            [
                read_byte(&mut f, &p, root1, 0x0000_00fe).unwrap(),
                read_byte(&mut f, &p, root1, 0x0000_00ff).unwrap(),
                read_byte(&mut f, &p, root1, 0x0000_0100).unwrap(),
                read_byte(&mut f, &p, root1, 0x0000_0101).unwrap(),
            ],
            [0x78, 0x56, 0x34, 0x12],
        );
        assert_eq!(read_byte(&mut f, &p, root1, 0x0000_00fd), Some(0));
        assert_eq!(read_byte(&mut f, &p, zero, 0x0000_00fe), Some(0));

        let (root2, _, atomic2) =
            write_word(&mut f, &p, root1, 0x0000_01ff, 16, 0xabcd)
                .unwrap();
        assert!(atomic2);
        assert_eq!(
            read_word(&mut f, &p, root2, 0x0000_01ff, 16),
            Some(0xabcd)
        );

        let (root3, _, atomic3) =
            write_word(&mut f, &p, root2, 0xffff_fffe, 32, 0x89ab_cdef)
                .unwrap();
        assert!(atomic3);
        assert_eq!(
            read_word(&mut f, &p, root3, 0xffff_fffe, 32),
            Some(0x89ab_cdef)
        );
        assert_eq!(read_byte(&mut f, &p, root3, 0xffff_fffe), Some(0xef));
        assert_eq!(read_byte(&mut f, &p, root3, 0xffff_ffff), Some(0xcd));
        assert_eq!(read_byte(&mut f, &p, root3, 0x0000_0000), Some(0xab));
        assert_eq!(read_byte(&mut f, &p, root3, 0x0000_0001), Some(0x89));
        assert_eq!(
            read_word(&mut f, &p, root2, 0xffff_fffe, 32),
            Some(0),
            "old MemoryRoot must preserve pre-write semantics"
        );
    }

    #[test]
    fn m6c_identical_write_is_canonical_and_malformed_word_fails_closed() {
        let mut f = FullFixture::new();
        let p = MemoryWordProgram::install(&mut f);
        let (root1, _, _) =
            write_word(
                &mut f,
                &p,
                p.memory.zero_root,
                0x0000_00ff,
                32,
                0x1234_5678,
            )
            .unwrap();

        let links = f.store.link_count();
        let (root2, _, atomic) =
            write_word(
                &mut f,
                &p,
                root1,
                0x0000_00ff,
                32,
                0x1234_5678,
            )
            .unwrap();
        assert_eq!(root2, root1);
        assert!(atomic);

        // The first identical write may reuse every canonical Link already
        // materialized by the original transition; a second rerun must not
        // grow the carrier further.
        let after_first_repeat = f.store.link_count();
        let (root3, _, _) =
            write_word(
                &mut f,
                &p,
                root1,
                0x0000_00ff,
                32,
                0x1234_5678,
            )
            .unwrap();
        assert_eq!(root3, root1);
        assert_eq!(f.store.link_count(), after_first_repeat);
        assert!(after_first_repeat >= links);

        let address = exact_value(&mut f, 32, 0x100);
        let malformed =
            exact_value(&mut f, 31, 0x1234_5678 & 0x7fff_ffff);
        assert!(
            invoke(&mut f, p.write32, &[root1, address, malformed])
                .is_none()
        );
    }
}
