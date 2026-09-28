use super::{
    full_adder::{
        call, define_bundle_rule, index_rule_for, Fixture as FullFixture,
    },
    proof_n::{
        execute_to_quiescence, identical_rerun, load_runtime, loaded_handle,
        prepare_stage, semantic_source, theory_admissions, visual_snapshot,
        WebProofResultStage, WebStructuralProof,
    },
};
use amemory_optimized_cpu_probe::{
    structural::{materialize_exact_sequence, read_exact_sequence},
    Handle, OptimizedLinkStore, ROOT_HANDLE,
};

const WIDTH: usize = 8;

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
        // Dedicated M6 memory namespace.
        for pole in [o, c, c, o, o, c, o, c, c, c, o, o, c, o] {
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

fn frame(
    store: &mut OptimizedLinkStore,
    tag: Handle,
    values: &[Handle],
) -> Handle {
    let payload = materialize_exact_sequence(store, values).unwrap();
    let descriptor = store.ensure_pair(tag, payload).unwrap();
    store.ensure_start_self_closed(descriptor).unwrap()
}

fn radix_branch(
    store: &mut OptimizedLinkStore,
    tag: Handle,
    low: Handle,
    high: Handle,
) -> Handle {
    let children = store.ensure_pair(low, high).unwrap();
    store.ensure_pair(tag, children).unwrap()
}

fn byte_leaf(
    store: &mut OptimizedLinkStore,
    leaf_tag: Handle,
    byte: Handle,
) -> Handle {
    store.ensure_pair(leaf_tag, byte).unwrap()
}

fn seq_with_head(
    store: &mut OptimizedLinkStore,
    head: Handle,
    rest: &[Handle],
) -> Handle {
    let mut values = Vec::with_capacity(1 + rest.len());
    values.push(head);
    values.extend_from_slice(rest);
    materialize_exact_sequence(store, &values).unwrap()
}

fn path_with_entry(
    store: &mut OptimizedLinkStore,
    path: &[Handle],
    bit: Handle,
    sibling: Handle,
) -> Handle {
    let entry = store.ensure_pair(bit, sibling).unwrap();
    let mut values = Vec::with_capacity(path.len() + 1);
    values.extend_from_slice(path);
    values.push(entry);
    materialize_exact_sequence(store, &values).unwrap()
}

#[derive(Clone, Debug)]
pub(crate) struct RadixMemoryProgram {
    pub(crate) read: Handle,
    pub(crate) write: Handle,
    pub(crate) witness: Handle,
    pub(crate) result_tag: Handle,
    pub(crate) zero_root: Handle,
    pub(crate) read_result_tag: Handle,
    pub(crate) write_result_tag: Handle,
    leaf_tag: Handle,
    witness_stage: Vec<Handle>,
    zero: Vec<Handle>,
    branch_tag: Vec<Handle>,
    read_tag: Vec<Handle>,
    validate_tag: Vec<Handle>,
    down_tag: Vec<Handle>,
    up_tag: Vec<Handle>,
    byte_zero: Handle,
}

impl RadixMemoryProgram {
    pub(crate) fn install(f: &mut FullFixture) -> Self {
        let seed = f
            .store
            .ensure_pair(f.interpreter, f.k)
            .unwrap();
        let mut anchors =
            AnchorGen::new(&mut f.store, seed, f.o, f.c);

        let read_left = anchors.next(&mut f.store);
        let read_right = anchors.next(&mut f.store);
        let read = f.store.ensure_pair(read_left, read_right).unwrap();

        let write_left = anchors.next(&mut f.store);
        let write_right = anchors.next(&mut f.store);
        let write = f.store.ensure_pair(write_left, write_right).unwrap();

        let witness_left = anchors.next(&mut f.store);
        let witness_right = anchors.next(&mut f.store);
        let witness =
            f.store.ensure_pair(witness_left, witness_right).unwrap();
        let result_tag = anchors.next(&mut f.store);
        let read_result_tag = anchors.next(&mut f.store);
        let write_result_tag = anchors.next(&mut f.store);
        let leaf_tag = anchors.next(&mut f.store);

        let mut witness_stage = Vec::with_capacity(4);
        for _ in 0..4 {
            witness_stage.push(anchors.next(&mut f.store));
        }

        let mut zero = Vec::with_capacity(WIDTH + 1);
        let mut branch_tag = vec![ROOT_HANDLE; WIDTH + 1];
        let mut read_tag = Vec::with_capacity(WIDTH + 1);
        let mut down_tag = Vec::with_capacity(WIDTH + 1);
        let mut up_tag = vec![ROOT_HANDLE; WIDTH + 1];
        let mut validate_tag = Vec::with_capacity(WIDTH);

        for _ in 0..=WIDTH {
            let descriptor = anchors.next(&mut f.store);
            zero.push(
                f.store
                    .ensure_start_self_closed(descriptor)
                    .unwrap(),
            );
        }
        for slot in branch_tag.iter_mut().skip(1) {
            *slot = anchors.next(&mut f.store);
        }
        for _ in 0..=WIDTH {
            read_tag.push(anchors.next(&mut f.store));
        }
        for _ in 0..WIDTH {
            validate_tag.push(anchors.next(&mut f.store));
        }
        for _ in 0..=WIDTH {
            down_tag.push(anchors.next(&mut f.store));
        }
        for slot in up_tag.iter_mut().skip(1) {
            *slot = anchors.next(&mut f.store);
        }

        let byte_zero = materialize_exact_sequence(
            &mut f.store,
            &[f.zero; WIDTH],
        )
        .unwrap();

        let program = Self {
            read,
            write,
            witness,
            result_tag,
            zero_root: zero[WIDTH],
            read_result_tag,
            write_result_tag,
            leaf_tag,
            witness_stage,
            zero,
            branch_tag,
            read_tag,
            validate_tag,
            down_tag,
            up_tag,
            byte_zero,
        };

        program.install_read_rules(f, &mut anchors);
        program.install_write_rules(f, &mut anchors);
        program.install_witness_rules(f, &mut anchors);
        program
    }

    fn install_read_rules(
        &self,
        f: &mut FullFixture,
        anchors: &mut AnchorGen,
    ) {
        // K -> Call(READ,[root,offset]) => ReadFrame8(K,offset) -> root
        {
            let k = anchors.next(&mut f.store);
            let root = anchors.next(&mut f.store);
            let offset = anchors.next(&mut f.store);
            let args = materialize_exact_sequence(
                &mut f.store,
                &[root, offset],
            )
            .unwrap();
            let invocation = call(&mut f.store, f.apply, self.read, args);
            let before = f.store.ensure_pair(k, invocation).unwrap();
            let caller = frame(
                &mut f.store,
                self.read_tag[WIDTH],
                &[k, offset],
            );
            let after = f.store.ensure_pair(caller, root).unwrap();
            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, root, offset],
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        // Each level consumes one canonical address bit. Zero subtrees still
        // consume all remaining bits, so malformed addresses never short-cut.
        for depth in (1..=WIDTH).rev() {
            for bit in [f.zero, f.one] {
                // Branch traversal.
                {
                    let k = anchors.next(&mut f.store);
                    let rest =
                        anchors.roles(&mut f.store, depth - 1);
                    let low = anchors.next(&mut f.store);
                    let high = anchors.next(&mut f.store);

                    let remaining =
                        seq_with_head(&mut f.store, bit, &rest);
                    let caller = frame(
                        &mut f.store,
                        self.read_tag[depth],
                        &[k, remaining],
                    );
                    let node = radix_branch(
                        &mut f.store,
                        self.branch_tag[depth],
                        low,
                        high,
                    );
                    let before =
                        f.store.ensure_pair(caller, node).unwrap();

                    let next_remaining =
                        materialize_exact_sequence(
                            &mut f.store,
                            &rest,
                        )
                        .unwrap();
                    let next_caller = frame(
                        &mut f.store,
                        self.read_tag[depth - 1],
                        &[k, next_remaining],
                    );
                    let selected =
                        if bit == f.zero { low } else { high };
                    let after = f
                        .store
                        .ensure_pair(next_caller, selected)
                        .unwrap();

                    let mut roles =
                        Vec::with_capacity(depth + 2);
                    roles.push(k);
                    roles.extend_from_slice(&rest);
                    roles.push(low);
                    roles.push(high);
                    let (_, admission) = define_bundle_rule(
                        &mut f.store,
                        f.theory,
                        &roles,
                        before,
                        &[after],
                    );
                    index_rule_for(
                        &mut f.store,
                        &[self.branch_tag[depth]],
                        admission,
                    );
                }

                // Canonical zero-subtree traversal.
                {
                    let k = anchors.next(&mut f.store);
                    let rest =
                        anchors.roles(&mut f.store, depth - 1);
                    let remaining =
                        seq_with_head(&mut f.store, bit, &rest);
                    let caller = frame(
                        &mut f.store,
                        self.read_tag[depth],
                        &[k, remaining],
                    );
                    let before = f
                        .store
                        .ensure_pair(caller, self.zero[depth])
                        .unwrap();

                    let next_remaining =
                        materialize_exact_sequence(
                            &mut f.store,
                            &rest,
                        )
                        .unwrap();
                    let next_caller = frame(
                        &mut f.store,
                        self.read_tag[depth - 1],
                        &[k, next_remaining],
                    );
                    let after = f
                        .store
                        .ensure_pair(
                            next_caller,
                            self.zero[depth - 1],
                        )
                        .unwrap();

                    let mut roles =
                        Vec::with_capacity(depth);
                    roles.push(k);
                    roles.extend_from_slice(&rest);
                    let (_, admission) = define_bundle_rule(
                        &mut f.store,
                        f.theory,
                        &roles,
                        before,
                        &[after],
                    );
                    index_rule_for(
                        &mut f.store,
                        &[self.zero[depth]],
                        admission,
                    );
                }
            }
        }

        // Leaf -> Byte8.
        {
            let k = anchors.next(&mut f.store);
            let byte = anchors.next(&mut f.store);
            let caller = frame(
                &mut f.store,
                self.read_tag[0],
                &[k, ROOT_HANDLE],
            );
            let leaf =
                byte_leaf(&mut f.store, self.leaf_tag, byte);
            let before = f.store.ensure_pair(caller, leaf).unwrap();
            let result =
                f.store.ensure_pair(self.read_result_tag, byte).unwrap();
            let after = f.store.ensure_pair(k, result).unwrap();
            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, byte],
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &[self.leaf_tag],
                admission,
            );
        }

        // Zero leaf -> canonical Byte8(0).
        {
            let k = anchors.next(&mut f.store);
            let caller = frame(
                &mut f.store,
                self.read_tag[0],
                &[k, ROOT_HANDLE],
            );
            let before = f
                .store
                .ensure_pair(caller, self.zero[0])
                .unwrap();
            let result = f
                .store
                .ensure_pair(self.read_result_tag, self.byte_zero)
                .unwrap();
            let after = f.store.ensure_pair(k, result).unwrap();
            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k],
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &[self.zero[0]],
                admission,
            );
        }
    }

    fn install_write_rules(
        &self,
        f: &mut FullFixture,
        anchors: &mut AnchorGen,
    ) {
        // Public WRITE call enters an eight-step Byte8 validator.
        {
            let k = anchors.next(&mut f.store);
            let root = anchors.next(&mut f.store);
            let offset = anchors.next(&mut f.store);
            let byte = anchors.next(&mut f.store);
            let args = materialize_exact_sequence(
                &mut f.store,
                &[root, offset, byte],
            )
            .unwrap();
            let invocation =
                call(&mut f.store, f.apply, self.write, args);
            let before = f.store.ensure_pair(k, invocation).unwrap();
            let caller = frame(
                &mut f.store,
                self.validate_tag[0],
                &[k, root, offset],
            );
            let validation = f
                .store
                .ensure_pair(self.validate_tag[0], byte)
                .unwrap();
            let after =
                f.store.ensure_pair(caller, validation).unwrap();
            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, root, offset, byte],
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        // Validate every Byte8 bit structurally. The validated prefix is
        // carried by the temporary Context; the original arbitrary Link is
        // never trusted as a byte value.
        for position in 0..WIDTH {
            for bit in [f.zero, f.one] {
                let k = anchors.next(&mut f.store);
                let root = anchors.next(&mut f.store);
                let offset = anchors.next(&mut f.store);
                let prefix =
                    anchors.roles(&mut f.store, position);
                let rest = anchors.roles(
                    &mut f.store,
                    WIDTH - position - 1,
                );

                let mut state =
                    Vec::with_capacity(3 + prefix.len());
                state.extend_from_slice(&[k, root, offset]);
                state.extend_from_slice(&prefix);
                let caller = frame(
                    &mut f.store,
                    self.validate_tag[position],
                    &state,
                );
                let tail =
                    seq_with_head(&mut f.store, bit, &rest);
                let validation = f
                    .store
                    .ensure_pair(self.validate_tag[position], tail)
                    .unwrap();
                let before =
                    f.store.ensure_pair(caller, validation).unwrap();

                let mut validated =
                    Vec::with_capacity(prefix.len() + 1);
                validated.extend_from_slice(&prefix);
                validated.push(bit);

                let after = if position + 1 < WIDTH {
                    let mut next_state =
                        Vec::with_capacity(3 + validated.len());
                    next_state.extend_from_slice(&[
                        k, root, offset,
                    ]);
                    next_state.extend_from_slice(&validated);
                    let next_caller = frame(
                        &mut f.store,
                        self.validate_tag[position + 1],
                        &next_state,
                    );
                    let next_tail =
                        materialize_exact_sequence(
                            &mut f.store,
                            &rest,
                        )
                        .unwrap();
                    let next_validation = f
                        .store
                        .ensure_pair(
                            self.validate_tag[position + 1],
                            next_tail,
                        )
                        .unwrap();
                    f.store
                        .ensure_pair(next_caller, next_validation)
                        .unwrap()
                } else {
                    let byte = materialize_exact_sequence(
                        &mut f.store,
                        &validated,
                    )
                    .unwrap();
                    let write_caller = frame(
                        &mut f.store,
                        self.down_tag[WIDTH],
                        &[k, offset, byte, ROOT_HANDLE],
                    );
                    f.store
                        .ensure_pair(write_caller, root)
                        .unwrap()
                };

                let mut roles = Vec::with_capacity(
                    3 + prefix.len() + rest.len(),
                );
                roles.extend_from_slice(&[k, root, offset]);
                roles.extend_from_slice(&prefix);
                roles.extend_from_slice(&rest);
                let (_, admission) = define_bundle_rule(
                    &mut f.store,
                    f.theory,
                    &roles,
                    before,
                    &[after],
                );
                index_rule_for(
                    &mut f.store,
                    &[self.validate_tag[position]],
                    admission,
                );
            }
        }

        // Descend the radix tree. Every consumed address bit is constrained
        // to canonical 0/1. The Context accumulates (bit,sibling) path entries.
        for depth in (1..=WIDTH).rev() {
            let consumed = WIDTH - depth;
            for bit in [f.zero, f.one] {
                // Existing branch.
                {
                    let k = anchors.next(&mut f.store);
                    let byte = anchors.next(&mut f.store);
                    let path =
                        anchors.roles(&mut f.store, consumed);
                    let rest =
                        anchors.roles(&mut f.store, depth - 1);
                    let low = anchors.next(&mut f.store);
                    let high = anchors.next(&mut f.store);

                    let remaining =
                        seq_with_head(&mut f.store, bit, &rest);
                    let path_seq =
                        materialize_exact_sequence(
                            &mut f.store,
                            &path,
                        )
                        .unwrap();
                    let caller = frame(
                        &mut f.store,
                        self.down_tag[depth],
                        &[k, remaining, byte, path_seq],
                    );
                    let node = radix_branch(
                        &mut f.store,
                        self.branch_tag[depth],
                        low,
                        high,
                    );
                    let before =
                        f.store.ensure_pair(caller, node).unwrap();

                    let selected =
                        if bit == f.zero { low } else { high };
                    let sibling =
                        if bit == f.zero { high } else { low };
                    let next_path = path_with_entry(
                        &mut f.store,
                        &path,
                        bit,
                        sibling,
                    );
                    let next_remaining =
                        materialize_exact_sequence(
                            &mut f.store,
                            &rest,
                        )
                        .unwrap();
                    let next_caller = frame(
                        &mut f.store,
                        self.down_tag[depth - 1],
                        &[k, next_remaining, byte, next_path],
                    );
                    let after = f
                        .store
                        .ensure_pair(next_caller, selected)
                        .unwrap();

                    let mut roles = Vec::with_capacity(
                        4 + path.len() + rest.len(),
                    );
                    roles.push(k);
                    roles.push(byte);
                    roles.extend_from_slice(&path);
                    roles.extend_from_slice(&rest);
                    roles.push(low);
                    roles.push(high);
                    let (_, admission) = define_bundle_rule(
                        &mut f.store,
                        f.theory,
                        &roles,
                        before,
                        &[after],
                    );
                    index_rule_for(
                        &mut f.store,
                        &[self.branch_tag[depth]],
                        admission,
                    );
                }

                // Lazy expansion of canonical zero subtree.
                {
                    let k = anchors.next(&mut f.store);
                    let byte = anchors.next(&mut f.store);
                    let path =
                        anchors.roles(&mut f.store, consumed);
                    let rest =
                        anchors.roles(&mut f.store, depth - 1);
                    let remaining =
                        seq_with_head(&mut f.store, bit, &rest);
                    let path_seq =
                        materialize_exact_sequence(
                            &mut f.store,
                            &path,
                        )
                        .unwrap();
                    let caller = frame(
                        &mut f.store,
                        self.down_tag[depth],
                        &[k, remaining, byte, path_seq],
                    );
                    let before = f
                        .store
                        .ensure_pair(caller, self.zero[depth])
                        .unwrap();

                    let next_path = path_with_entry(
                        &mut f.store,
                        &path,
                        bit,
                        self.zero[depth - 1],
                    );
                    let next_remaining =
                        materialize_exact_sequence(
                            &mut f.store,
                            &rest,
                        )
                        .unwrap();
                    let next_caller = frame(
                        &mut f.store,
                        self.down_tag[depth - 1],
                        &[k, next_remaining, byte, next_path],
                    );
                    let after = f
                        .store
                        .ensure_pair(
                            next_caller,
                            self.zero[depth - 1],
                        )
                        .unwrap();

                    let mut roles = Vec::with_capacity(
                        2 + path.len() + rest.len(),
                    );
                    roles.push(k);
                    roles.push(byte);
                    roles.extend_from_slice(&path);
                    roles.extend_from_slice(&rest);
                    let (_, admission) = define_bundle_rule(
                        &mut f.store,
                        f.theory,
                        &roles,
                        before,
                        &[after],
                    );
                    index_rule_for(
                        &mut f.store,
                        &[self.zero[depth]],
                        admission,
                    );
                }
            }
        }

        // Depth-0 replacement for an existing leaf.
        {
            let k = anchors.next(&mut f.store);
            let byte = anchors.next(&mut f.store);
            let old_byte = anchors.next(&mut f.store);
            let path = anchors.roles(&mut f.store, WIDTH);
            let path_seq = materialize_exact_sequence(
                &mut f.store,
                &path,
            )
            .unwrap();
            let caller = frame(
                &mut f.store,
                self.down_tag[0],
                &[k, ROOT_HANDLE, byte, path_seq],
            );
            let old_leaf =
                byte_leaf(&mut f.store, self.leaf_tag, old_byte);
            let before =
                f.store.ensure_pair(caller, old_leaf).unwrap();
            let new_leaf =
                byte_leaf(&mut f.store, self.leaf_tag, byte);
            let up_caller = frame(
                &mut f.store,
                self.up_tag[WIDTH],
                &[k, path_seq],
            );
            let up_value = f
                .store
                .ensure_pair(self.up_tag[WIDTH], new_leaf)
                .unwrap();
            let after =
                f.store.ensure_pair(up_caller, up_value).unwrap();

            let mut roles = Vec::with_capacity(WIDTH + 3);
            roles.extend_from_slice(&[k, byte, old_byte]);
            roles.extend_from_slice(&path);
            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &[self.leaf_tag],
                admission,
            );
        }

        // Depth-0 expansion of zero leaf.
        {
            let k = anchors.next(&mut f.store);
            let byte = anchors.next(&mut f.store);
            let path = anchors.roles(&mut f.store, WIDTH);
            let path_seq = materialize_exact_sequence(
                &mut f.store,
                &path,
            )
            .unwrap();
            let caller = frame(
                &mut f.store,
                self.down_tag[0],
                &[k, ROOT_HANDLE, byte, path_seq],
            );
            let before = f
                .store
                .ensure_pair(caller, self.zero[0])
                .unwrap();
            let new_leaf =
                byte_leaf(&mut f.store, self.leaf_tag, byte);
            let up_caller = frame(
                &mut f.store,
                self.up_tag[WIDTH],
                &[k, path_seq],
            );
            let up_value = f
                .store
                .ensure_pair(self.up_tag[WIDTH], new_leaf)
                .unwrap();
            let after =
                f.store.ensure_pair(up_caller, up_value).unwrap();

            let mut roles = Vec::with_capacity(WIDTH + 2);
            roles.extend_from_slice(&[k, byte]);
            roles.extend_from_slice(&path);
            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &roles,
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &[self.zero[0]],
                admission,
            );
        }

        // Rebuild the path bottom-up. The last path entry determines whether
        // the newly built child occupies the low or high pole.
        for level in (1..=WIDTH).rev() {
            let branch_depth = WIDTH - level + 1;
            for bit in [f.zero, f.one] {
                let k = anchors.next(&mut f.store);
                let prefix =
                    anchors.roles(&mut f.store, level - 1);
                let sibling = anchors.next(&mut f.store);
                let child = anchors.next(&mut f.store);

                let last =
                    f.store.ensure_pair(bit, sibling).unwrap();
                let mut path_values =
                    Vec::with_capacity(prefix.len() + 1);
                path_values.extend_from_slice(&prefix);
                path_values.push(last);
                let path_seq =
                    materialize_exact_sequence(
                        &mut f.store,
                        &path_values,
                    )
                    .unwrap();
                let caller = frame(
                    &mut f.store,
                    self.up_tag[level],
                    &[k, path_seq],
                );
                let up_value = f
                    .store
                    .ensure_pair(self.up_tag[level], child)
                    .unwrap();
                let before =
                    f.store.ensure_pair(caller, up_value).unwrap();

                let node = if bit == f.zero {
                    radix_branch(
                        &mut f.store,
                        self.branch_tag[branch_depth],
                        child,
                        sibling,
                    )
                } else {
                    radix_branch(
                        &mut f.store,
                        self.branch_tag[branch_depth],
                        sibling,
                        child,
                    )
                };

                let after = if level > 1 {
                    let next_path =
                        materialize_exact_sequence(
                            &mut f.store,
                            &prefix,
                        )
                        .unwrap();
                    let next_caller = frame(
                        &mut f.store,
                        self.up_tag[level - 1],
                        &[k, next_path],
                    );
                    let next_up_value = f
                        .store
                        .ensure_pair(self.up_tag[level - 1], node)
                        .unwrap();
                    f.store
                        .ensure_pair(next_caller, next_up_value)
                        .unwrap()
                } else {
                    let result = f
                        .store
                        .ensure_pair(self.write_result_tag, node)
                        .unwrap();
                    f.store.ensure_pair(k, result).unwrap()
                };

                let mut roles =
                    Vec::with_capacity(level + 2);
                roles.push(k);
                roles.extend_from_slice(&prefix);
                roles.push(sibling);
                roles.push(child);
                let (_, admission) = define_bundle_rule(
                    &mut f.store,
                    f.theory,
                    &roles,
                    before,
                    &[after],
                );
                index_rule_for(
                    &mut f.store,
                    &[self.up_tag[level]],
                    admission,
                );
            }
        }
    }

    fn install_witness_rules(
        &self,
        f: &mut FullFixture,
        anchors: &mut AnchorGen,
    ) {
        // K -> Call(WITNESS,[oldRoot,offset,byte])
        // => Stage0(K,oldRoot,offset,byte) -> Call(READ,[oldRoot,offset])
        {
            let k = anchors.next(&mut f.store);
            let root = anchors.next(&mut f.store);
            let offset = anchors.next(&mut f.store);
            let byte = anchors.next(&mut f.store);
            let args = materialize_exact_sequence(
                &mut f.store,
                &[root, offset, byte],
            )
            .unwrap();
            let invocation =
                call(&mut f.store, f.apply, self.witness, args);
            let before = f.store.ensure_pair(k, invocation).unwrap();

            let caller = frame(
                &mut f.store,
                self.witness_stage[0],
                &[k, root, offset, byte],
            );
            let read_args = materialize_exact_sequence(
                &mut f.store,
                &[root, offset],
            )
            .unwrap();
            let read_call =
                call(&mut f.store, f.apply, self.read, read_args);
            let after =
                f.store.ensure_pair(caller, read_call).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, root, offset, byte],
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        // Stage0 -> beforeByte; continue with WRITE.
        {
            let k = anchors.next(&mut f.store);
            let root = anchors.next(&mut f.store);
            let offset = anchors.next(&mut f.store);
            let byte = anchors.next(&mut f.store);
            let before_byte = anchors.next(&mut f.store);
            let caller = frame(
                &mut f.store,
                self.witness_stage[0],
                &[k, root, offset, byte],
            );
            let read_result = f
                .store
                .ensure_pair(self.read_result_tag, before_byte)
                .unwrap();
            let before =
                f.store.ensure_pair(caller, read_result).unwrap();

            let next = frame(
                &mut f.store,
                self.witness_stage[1],
                &[k, root, offset, byte, before_byte],
            );
            let write_args = materialize_exact_sequence(
                &mut f.store,
                &[root, offset, byte],
            )
            .unwrap();
            let write_call =
                call(&mut f.store, f.apply, self.write, write_args);
            let after =
                f.store.ensure_pair(next, write_call).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, root, offset, byte, before_byte],
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &[self.read_result_tag],
                admission,
            );
        }

        // Stage1 -> newRoot; read from the new root.
        {
            let k = anchors.next(&mut f.store);
            let root = anchors.next(&mut f.store);
            let offset = anchors.next(&mut f.store);
            let byte = anchors.next(&mut f.store);
            let before_byte = anchors.next(&mut f.store);
            let new_root = anchors.next(&mut f.store);
            let caller = frame(
                &mut f.store,
                self.witness_stage[1],
                &[k, root, offset, byte, before_byte],
            );
            let write_result = f
                .store
                .ensure_pair(self.write_result_tag, new_root)
                .unwrap();
            let before =
                f.store.ensure_pair(caller, write_result).unwrap();

            let next = frame(
                &mut f.store,
                self.witness_stage[2],
                &[k, root, offset, byte, before_byte, new_root],
            );
            let read_args = materialize_exact_sequence(
                &mut f.store,
                &[new_root, offset],
            )
            .unwrap();
            let read_call =
                call(&mut f.store, f.apply, self.read, read_args);
            let after =
                f.store.ensure_pair(next, read_call).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, root, offset, byte, before_byte, new_root],
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &[self.write_result_tag],
                admission,
            );
        }

        // Stage2 -> afterByte; re-read the old root after the write.
        {
            let k = anchors.next(&mut f.store);
            let root = anchors.next(&mut f.store);
            let offset = anchors.next(&mut f.store);
            let byte = anchors.next(&mut f.store);
            let before_byte = anchors.next(&mut f.store);
            let new_root = anchors.next(&mut f.store);
            let after_byte = anchors.next(&mut f.store);
            let caller = frame(
                &mut f.store,
                self.witness_stage[2],
                &[k, root, offset, byte, before_byte, new_root],
            );
            let read_result = f
                .store
                .ensure_pair(self.read_result_tag, after_byte)
                .unwrap();
            let before =
                f.store.ensure_pair(caller, read_result).unwrap();

            let next = frame(
                &mut f.store,
                self.witness_stage[3],
                &[
                    k,
                    root,
                    offset,
                    byte,
                    before_byte,
                    new_root,
                    after_byte,
                ],
            );
            let old_read_args = materialize_exact_sequence(
                &mut f.store,
                &[root, offset],
            )
            .unwrap();
            let old_read =
                call(&mut f.store, f.apply, self.read, old_read_args);
            let after =
                f.store.ensure_pair(next, old_read).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[
                    k,
                    root,
                    offset,
                    byte,
                    before_byte,
                    new_root,
                    after_byte,
                ],
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &[self.read_result_tag],
                admission,
            );
        }

        // Final persistence result:
        // RESULT([oldRoot,newRoot,before,after,oldAfter]).
        {
            let k = anchors.next(&mut f.store);
            let root = anchors.next(&mut f.store);
            let offset = anchors.next(&mut f.store);
            let byte = anchors.next(&mut f.store);
            let before_byte = anchors.next(&mut f.store);
            let new_root = anchors.next(&mut f.store);
            let after_byte = anchors.next(&mut f.store);
            let old_after = anchors.next(&mut f.store);
            let caller = frame(
                &mut f.store,
                self.witness_stage[3],
                &[
                    k,
                    root,
                    offset,
                    byte,
                    before_byte,
                    new_root,
                    after_byte,
                ],
            );
            let read_result = f
                .store
                .ensure_pair(self.read_result_tag, old_after)
                .unwrap();
            let before =
                f.store.ensure_pair(caller, read_result).unwrap();

            let result_sequence = materialize_exact_sequence(
                &mut f.store,
                &[
                    root,
                    new_root,
                    before_byte,
                    after_byte,
                    old_after,
                ],
            )
            .unwrap();
            let result = f
                .store
                .ensure_pair(self.result_tag, result_sequence)
                .unwrap();
            let after = f.store.ensure_pair(k, result).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[
                    k,
                    root,
                    offset,
                    byte,
                    before_byte,
                    new_root,
                    after_byte,
                    old_after,
                ],
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &[self.read_result_tag],
                admission,
            );
        }
    }
}

fn word8(
    f: &mut FullFixture,
    value: u8,
) -> Handle {
    let bits = (0..WIDTH)
        .map(|bit| {
            if (value >> bit) & 1 == 1 {
                f.one
            } else {
                f.zero
            }
        })
        .collect::<Vec<_>>();
    materialize_exact_sequence(&mut f.store, &bits).unwrap()
}

fn decode_word8(
    f: &FullFixture,
    value: Handle,
) -> Option<u8> {
    let bits = read_exact_sequence(&f.store, value).ok()?;
    if bits.len() != WIDTH {
        return None;
    }
    let mut out = 0u8;
    for (index, bit) in bits.into_iter().enumerate() {
        if bit == f.one {
            out |= 1u8 << index;
        } else if bit != f.zero {
            return None;
        }
    }
    Some(out)
}

fn decode_word8_store(
    store: &OptimizedLinkStore,
    zero: Handle,
    one: Handle,
    value: Handle,
) -> Option<u8> {
    let bits = read_exact_sequence(store, value).ok()?;
    if bits.len() != WIDTH {
        return None;
    }
    let mut out = 0u8;
    for (index, bit) in bits.into_iter().enumerate() {
        if bit == one {
            out |= 1u8 << index;
        } else if bit != zero {
            return None;
        }
    }
    Some(out)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WebRadixMemoryOutcome {
    pub(crate) offset: u32,
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
    pub(crate) quiescent: u8,
}

#[derive(Clone, Debug)]
pub(crate) struct WebRadixMemoryExecution {
    pub(crate) outcome: WebRadixMemoryOutcome,
    pub(crate) proof: WebStructuralProof,
}

pub(crate) fn web_prove_radix_memory(
    offset_value: u8,
    byte_value: u8,
) -> Option<WebRadixMemoryExecution> {
    let mut compiler = FullFixture::new();
    let program = RadixMemoryProgram::install(&mut compiler);
    let offset = word8(&mut compiler, offset_value);
    let byte = word8(&mut compiler, byte_value);

    let args = materialize_exact_sequence(
        &mut compiler.store,
        &[program.zero_root, offset, byte],
    )
    .ok()?;
    let invocation =
        call(&mut compiler.store, compiler.apply, program.witness, args);
    let initial =
        compiler.store.ensure_pair(compiler.k, invocation).ok()?;

    let prepared_roots = vec![
        semantic_source(
            &compiler.store,
            "function.memory.witness",
            program.witness,
        ),
        semantic_source(
            &compiler.store,
            "function.memory.read",
            program.read,
        ),
        semantic_source(
            &compiler.store,
            "function.memory.write",
            program.write,
        ),
        semantic_source(
            &compiler.store,
            "memory.zero_root",
            program.zero_root,
        ),
        semantic_source(
            &compiler.store,
            "memory.result_tag",
            program.result_tag,
        ),
        semantic_source(
            &compiler.store,
            "data.offset8",
            offset,
        ),
        semantic_source(
            &compiler.store,
            "data.byte8",
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

    let interpreter =
        loaded_handle(&load, "execution.interpreter")?;
    let initial = loaded_handle(&load, "scope.initial")?;
    let expected_old_root =
        loaded_handle(&load, "memory.zero_root")?;
    let result_tag =
        loaded_handle(&load, "memory.result_tag")?;
    let zero = loaded_handle(&load, "data.bit.zero")?;
    let one = loaded_handle(&load, "data.bit.one")?;
    let result_context =
        loaded_handle(&load, "context.result")?;

    let (mut engine, execute) = execute_to_quiescence(
        &mut memory,
        interpreter,
        initial,
        128,
        96,
    )?;
    if engine.current().len() != 1 {
        return None;
    }

    let final_link = engine.current()[0];
    let (caller, envelope) = memory.store.poles(final_link).ok()?;
    if caller != result_context {
        return None;
    }
    let (tag, result_sequence) =
        memory.store.poles(envelope).ok()?;
    if tag != result_tag {
        return None;
    }
    let values =
        read_exact_sequence(&memory.store, result_sequence).ok()?;
    if values.len() != 5 {
        return None;
    }

    let old_root = values[0];
    let new_root = values[1];
    let before_byte = values[2];
    let after_byte = values[3];
    let old_after_byte = values[4];
    if old_root != expected_old_root
        || !memory.store.is_valid(old_root)
        || !memory.store.is_valid(new_root)
    {
        return None;
    }

    let before_value =
        decode_word8_store(&memory.store, zero, one, before_byte)?;
    let after_value =
        decode_word8_store(&memory.store, zero, one, after_byte)?;
    let old_after_value =
        decode_word8_store(&memory.store, zero, one, old_after_byte)?;
    let oracle_matches = before_value == 0
        && after_value == byte_value
        && old_after_value == 0;

    let result_recursive_wire =
        memory.store.export_anum(final_link).ok()?;
    let result_sequence_anum =
        memory.store.export_anum(result_sequence).ok()?;
    let identical_rerun_link_delta = identical_rerun(
        &mut memory,
        &mut engine,
        initial,
        &result_recursive_wire,
        96,
    )?;
    let visual_links = visual_snapshot(&memory, &load.semantic_roots);

    let proof = WebStructuralProof {
        schema_version: 4,
        block: "M6A_RADIX_PAGE".to_owned(),
        prepare,
        load,
        execute,
        result: WebProofResultStage {
            memory_instance_id: memory.id.clone(),
            result_anum: result_recursive_wire,
            result_sequence_anum,
            decoded_value: u32::from(after_value),
            decoded_value_hi: Some(u32::from(old_after_value)),
            oracle_value: u32::from(byte_value),
            oracle_value_hi: Some(0),
            oracle_matches,
            links_final: memory.store.link_count() as u32,
            identical_rerun_link_delta,
            visual_links,
        },
    };

    Some(WebRadixMemoryExecution {
        outcome: WebRadixMemoryOutcome {
            offset: u32::from(offset_value),
            write_value: u32::from(byte_value),
            before_value: u32::from(before_value),
            after_value: u32::from(after_value),
            old_after_value: u32::from(old_after_value),
            old_root_ref: old_root,
            new_root_ref: new_root,
            reactions: proof.execute.active_reaction_count,
            links_after_load: proof.load.links_after_load,
            links_final: proof.result.links_final,
            steady_link_delta: proof.result.identical_rerun_link_delta,
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
        let argument =
            materialize_exact_sequence(&mut f.store, args).unwrap();
        let invocation =
            call(&mut f.store, f.apply, function, argument);
        let initial = f.store.ensure_pair(f.k, invocation).unwrap();
        f.engine.set_current(&f.store, &[initial]).unwrap();

        for _ in 0..64 {
            let reaction = f.engine.run(&mut f.store).unwrap();
            if reaction.quiescent {
                break;
            }
        }
        if !f.engine.quiescent() || f.engine.current().len() != 1 {
            return None;
        }
        let final_member = f.engine.current()[0];
        let (caller, value) = f.store.poles(final_member).ok()?;
        (caller == f.k).then_some(value)
    }

    fn read(
        f: &mut FullFixture,
        p: &RadixMemoryProgram,
        root: Handle,
        offset: u8,
    ) -> Option<u8> {
        let offset = word8(f, offset);
        let envelope = invoke(f, p.read, &[root, offset])?;
        let (tag, value) = f.store.poles(envelope).ok()?;
        if tag != p.read_result_tag {
            return None;
        }
        decode_word8(f, value)
    }

    fn write(
        f: &mut FullFixture,
        p: &RadixMemoryProgram,
        root: Handle,
        offset: u8,
        value: u8,
    ) -> Option<Handle> {
        let offset = word8(f, offset);
        let byte = word8(f, value);
        let envelope = invoke(f, p.write, &[root, offset, byte])?;
        let (tag, new_root) = f.store.poles(envelope).ok()?;
        (tag == p.write_result_tag).then_some(new_root)
    }

    #[test]
    fn m6a_real_one_memory_persistence_proof() {
        let execution = web_prove_radix_memory(0x25, 0xab).unwrap();
        assert_eq!(execution.outcome.offset, 0x25);
        assert_eq!(execution.outcome.write_value, 0xab);
        assert_eq!(execution.outcome.before_value, 0);
        assert_eq!(execution.outcome.after_value, 0xab);
        assert_eq!(execution.outcome.old_after_value, 0);
        assert_ne!(
            execution.outcome.old_root_ref,
            execution.outcome.new_root_ref
        );
        assert!(execution.outcome.reactions > 40);
        assert_eq!(execution.outcome.steady_link_delta, 0);
        assert_eq!(execution.outcome.quiescent, 1);
        assert_eq!(execution.proof.block, "M6A_RADIX_PAGE");
        assert!(execution.proof.result.oracle_matches);
        assert!(execution.proof.execute.reactions.iter().all(|step| {
            step.scope_before.len() == 1 && step.scope_after.len() == 1
        }));
    }

    #[test]
    fn m6a_zero_page_reads_zero_at_distinct_offsets() {
        let mut f = FullFixture::new();
        let p = RadixMemoryProgram::install(&mut f);
        for offset in [0x00, 0x01, 0x25, 0x80, 0xff] {
            assert_eq!(read(&mut f, &p, p.zero_root, offset), Some(0));
        }
    }

    #[test]
    fn m6a_write_read_and_persistent_old_root() {
        let mut f = FullFixture::new();
        let p = RadixMemoryProgram::install(&mut f);
        let zero_root = p.zero_root;

        let root1 =
            write(&mut f, &p, zero_root, 0x25, 0xab).unwrap();
        assert_ne!(root1, zero_root);
        assert_eq!(read(&mut f, &p, root1, 0x25), Some(0xab));
        assert_eq!(read(&mut f, &p, root1, 0x24), Some(0));
        assert_eq!(read(&mut f, &p, zero_root, 0x25), Some(0));

        let root2 =
            write(&mut f, &p, root1, 0xa5, 0x7c).unwrap();
        assert_eq!(read(&mut f, &p, root2, 0x25), Some(0xab));
        assert_eq!(read(&mut f, &p, root2, 0xa5), Some(0x7c));

        let root3 =
            write(&mut f, &p, root2, 0x25, 0xcd).unwrap();
        assert_eq!(read(&mut f, &p, root3, 0x25), Some(0xcd));
        assert_eq!(read(&mut f, &p, root3, 0xa5), Some(0x7c));
        assert_eq!(read(&mut f, &p, root2, 0x25), Some(0xab));
    }

    #[test]
    fn m6a_identical_write_canonicalizes_without_link_growth() {
        let mut f = FullFixture::new();
        let p = RadixMemoryProgram::install(&mut f);
        let root1 =
            write(&mut f, &p, p.zero_root, 0x25, 0xab).unwrap();

        let offset = word8(&mut f, 0x25);
        let byte = word8(&mut f, 0xab);
        let args =
            materialize_exact_sequence(
                &mut f.store,
                &[root1, offset, byte],
            )
            .unwrap();
        let invocation =
            call(&mut f.store, f.apply, p.write, args);
        let initial = f.store.ensure_pair(f.k, invocation).unwrap();
        f.engine.set_current(&f.store, &[initial]).unwrap();
        for _ in 0..64 {
            let reaction = f.engine.run(&mut f.store).unwrap();
            if reaction.quiescent {
                break;
            }
        }
        assert!(f.engine.quiescent());
        let first_final = f.engine.current()[0];
        let (_, first_envelope) = f.store.poles(first_final).unwrap();
        let (first_tag, first_root) =
            f.store.poles(first_envelope).unwrap();
        assert_eq!(first_tag, p.write_result_tag);
        assert_eq!(first_root, root1);

        let links = f.store.link_count();
        f.engine.set_current(&f.store, &[initial]).unwrap();
        for _ in 0..64 {
            let reaction = f.engine.run(&mut f.store).unwrap();
            if reaction.quiescent {
                break;
            }
        }
        let second_final = f.engine.current()[0];
        let (_, second_envelope) = f.store.poles(second_final).unwrap();
        let (second_tag, second_root) =
            f.store.poles(second_envelope).unwrap();
        assert_eq!(second_tag, p.write_result_tag);
        assert_eq!(second_root, root1);
        assert_eq!(f.store.link_count(), links);
    }

    #[test]
    fn m6a_malformed_address_byte_and_depth_fail_closed() {
        let mut f = FullFixture::new();
        let p = RadixMemoryProgram::install(&mut f);

        let short_address = materialize_exact_sequence(
            &mut f.store,
            &[f.zero; WIDTH - 1],
        )
        .unwrap();
        assert!(
            invoke(&mut f, p.read, &[p.zero_root, short_address])
                .is_none()
        );

        let address = word8(&mut f, 0x25);
        let short_byte = materialize_exact_sequence(
            &mut f.store,
            &[f.one; WIDTH - 1],
        )
        .unwrap();
        assert!(
            invoke(
                &mut f,
                p.write,
                &[p.zero_root, address, short_byte],
            )
            .is_none()
        );

        let wrong_depth = p.zero[WIDTH - 1];
        let address = word8(&mut f, 0x25);
        assert!(
            invoke(&mut f, p.read, &[wrong_depth, address])
                .is_none()
        );
    }

    #[test]
    fn m6a_equivalent_pages_have_same_recursive_wire() {
        fn build(extra_noise: bool) -> String {
            let mut f = FullFixture::new();
            if extra_noise {
                let noise = f.store.ensure_pair(f.k, f.full).unwrap();
                let _ = f.store.ensure_pair(noise, f.o).unwrap();
            }
            let p = RadixMemoryProgram::install(&mut f);
            let root1 =
                write(&mut f, &p, p.zero_root, 0x25, 0xab).unwrap();
            let root2 =
                write(&mut f, &p, root1, 0xa5, 0x7c).unwrap();
            f.store.export_anum(root2).unwrap()
        }

        assert_eq!(build(false), build(true));
    }
}
