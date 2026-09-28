use super::{
    full_adder::{
        call, define_bundle_rule, index_rule_for, Fixture as FullFixture,
    },
    memory_n::RadixMemoryProgram,
    proof_n::{
        execute_to_quiescence, identical_rerun, load_runtime, loaded_handle,
        prepare_stage, semantic_source, theory_admissions, visual_snapshot,
        ProofRuntimeMemory, WebProofResultStage, WebStructuralProof,
    },
};
use amemory_optimized_cpu_probe::{
    structural::{materialize_exact_sequence, read_exact_sequence},
    Handle, OptimizedLinkStore, ROOT_HANDLE,
};

const ADDRESS_WIDTH: usize = 32;
const OFFSET_BITS: usize = 8;
const PAGE_BITS: usize = ADDRESS_WIDTH - OFFSET_BITS;

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
        // Dedicated M6b namespace.
        for pole in [c, o, c, c, o, o, c, o, o, c, c, o, c, o, c, c] {
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

fn tagged(
    store: &mut OptimizedLinkStore,
    tag: Handle,
    value: Handle,
) -> Handle {
    store.ensure_pair(tag, value).unwrap()
}

fn branch(
    store: &mut OptimizedLinkStore,
    tag: Handle,
    low: Handle,
    high: Handle,
) -> Handle {
    let children = store.ensure_pair(low, high).unwrap();
    store.ensure_pair(tag, children).unwrap()
}

fn leaf(
    store: &mut OptimizedLinkStore,
    leaf_tag: Handle,
    page_root: Handle,
) -> Handle {
    store.ensure_pair(leaf_tag, page_root).unwrap()
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
pub(crate) struct Memory32Program {
    pub(crate) read: Handle,
    pub(crate) write: Handle,
    pub(crate) read_result_tag: Handle,
    pub(crate) write_result_tag: Handle,
    pub(crate) zero_root: Handle,
    pub(crate) page: RadixMemoryProgram,
    page_leaf_tag: Handle,
    zero: Vec<Handle>,
    branch_tag: Vec<Handle>,
    read_validate_tag: Vec<Handle>,
    write_validate_tag: Vec<Handle>,
    read_down_tag: Vec<Handle>,
    write_down_tag: Vec<Handle>,
    write_up_tag: Vec<Handle>,
    page_read_return_tag: Handle,
    page_write_return_tag: Handle,
}

impl Memory32Program {
    pub(crate) fn install(f: &mut FullFixture) -> Self {
        let page = RadixMemoryProgram::install(f);
        let namespace = f
            .store
            .ensure_pair(page.read, page.write)
            .unwrap();
        let seed = f
            .store
            .ensure_pair(f.interpreter, namespace)
            .unwrap();
        let mut anchors =
            AnchorGen::new(&mut f.store, seed, f.o, f.c);

        let read = {
            let a = anchors.next(&mut f.store);
            let b = anchors.next(&mut f.store);
            f.store.ensure_pair(a, b).unwrap()
        };
        let write = {
            let a = anchors.next(&mut f.store);
            let b = anchors.next(&mut f.store);
            f.store.ensure_pair(a, b).unwrap()
        };
        let read_result_tag = anchors.next(&mut f.store);
        let write_result_tag = anchors.next(&mut f.store);
        let page_leaf_tag = anchors.next(&mut f.store);
        let page_read_return_tag = anchors.next(&mut f.store);
        let page_write_return_tag = anchors.next(&mut f.store);

        let mut zero = Vec::with_capacity(PAGE_BITS + 1);
        let mut branch_tag = vec![ROOT_HANDLE; PAGE_BITS + 1];
        let mut read_validate_tag = Vec::with_capacity(ADDRESS_WIDTH);
        let mut write_validate_tag = Vec::with_capacity(ADDRESS_WIDTH);
        let mut read_down_tag = Vec::with_capacity(PAGE_BITS + 1);
        let mut write_down_tag = Vec::with_capacity(PAGE_BITS + 1);
        let mut write_up_tag = vec![ROOT_HANDLE; PAGE_BITS + 1];

        for _ in 0..=PAGE_BITS {
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
        for _ in 0..ADDRESS_WIDTH {
            read_validate_tag.push(anchors.next(&mut f.store));
            write_validate_tag.push(anchors.next(&mut f.store));
        }
        for _ in 0..=PAGE_BITS {
            read_down_tag.push(anchors.next(&mut f.store));
            write_down_tag.push(anchors.next(&mut f.store));
        }
        for slot in write_up_tag.iter_mut().skip(1) {
            *slot = anchors.next(&mut f.store);
        }

        let program = Self {
            read,
            write,
            read_result_tag,
            write_result_tag,
            zero_root: zero[PAGE_BITS],
            page,
            page_leaf_tag,
            zero,
            branch_tag,
            read_validate_tag,
            write_validate_tag,
            read_down_tag,
            write_down_tag,
            write_up_tag,
            page_read_return_tag,
            page_write_return_tag,
        };

        program.install_read(f, &mut anchors);
        program.install_write(f, &mut anchors);
        program
    }

    fn install_read(
        &self,
        f: &mut FullFixture,
        anchors: &mut AnchorGen,
    ) {
        // Public READ32 enters structural Address32 validation.
        {
            let k = anchors.next(&mut f.store);
            let root = anchors.next(&mut f.store);
            let address = anchors.next(&mut f.store);
            let args =
                materialize_exact_sequence(&mut f.store, &[root, address])
                    .unwrap();
            let invocation =
                call(&mut f.store, f.apply, self.read, args);
            let before = f.store.ensure_pair(k, invocation).unwrap();

            let caller = frame(
                &mut f.store,
                self.read_validate_tag[0],
                &[k, root],
            );
            let value = tagged(
                &mut f.store,
                self.read_validate_tag[0],
                address,
            );
            let after = f.store.ensure_pair(caller, value).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, root, address],
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        // Validate every Address32 bit and split the final validated sequence
        // into Offset8 and Page24. No host page selection participates.
        for position in 0..ADDRESS_WIDTH {
            for bit in [f.zero, f.one] {
                let k = anchors.next(&mut f.store);
                let root = anchors.next(&mut f.store);
                let prefix =
                    anchors.roles(&mut f.store, position);
                let rest = anchors.roles(
                    &mut f.store,
                    ADDRESS_WIDTH - position - 1,
                );

                let mut state = Vec::with_capacity(2 + prefix.len());
                state.extend_from_slice(&[k, root]);
                state.extend_from_slice(&prefix);
                let caller = frame(
                    &mut f.store,
                    self.read_validate_tag[position],
                    &state,
                );
                let tail =
                    seq_with_head(&mut f.store, bit, &rest);
                let endpoint = tagged(
                    &mut f.store,
                    self.read_validate_tag[position],
                    tail,
                );
                let before =
                    f.store.ensure_pair(caller, endpoint).unwrap();

                let mut validated =
                    Vec::with_capacity(prefix.len() + 1);
                validated.extend_from_slice(&prefix);
                validated.push(bit);

                let after = if position + 1 < ADDRESS_WIDTH {
                    let mut next_state =
                        Vec::with_capacity(2 + validated.len());
                    next_state.extend_from_slice(&[k, root]);
                    next_state.extend_from_slice(&validated);
                    let next_caller = frame(
                        &mut f.store,
                        self.read_validate_tag[position + 1],
                        &next_state,
                    );
                    let next_tail =
                        materialize_exact_sequence(
                            &mut f.store,
                            &rest,
                        )
                        .unwrap();
                    let next_endpoint = tagged(
                        &mut f.store,
                        self.read_validate_tag[position + 1],
                        next_tail,
                    );
                    f.store
                        .ensure_pair(next_caller, next_endpoint)
                        .unwrap()
                } else {
                    debug_assert_eq!(validated.len(), ADDRESS_WIDTH);
                    let offset = materialize_exact_sequence(
                        &mut f.store,
                        &validated[..OFFSET_BITS],
                    )
                    .unwrap();
                    let page = materialize_exact_sequence(
                        &mut f.store,
                        &validated[OFFSET_BITS..],
                    )
                    .unwrap();
                    let down = frame(
                        &mut f.store,
                        self.read_down_tag[PAGE_BITS],
                        &[k, offset, page],
                    );
                    f.store.ensure_pair(down, root).unwrap()
                };

                let mut roles =
                    Vec::with_capacity(2 + prefix.len() + rest.len());
                roles.extend_from_slice(&[k, root]);
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
                    &[self.read_validate_tag[position]],
                    admission,
                );
            }
        }

        // Traverse Page24 selector.
        for depth in (1..=PAGE_BITS).rev() {
            for bit in [f.zero, f.one] {
                // Existing branch.
                {
                    let k = anchors.next(&mut f.store);
                    let offset = anchors.next(&mut f.store);
                    let rest =
                        anchors.roles(&mut f.store, depth - 1);
                    let low = anchors.next(&mut f.store);
                    let high = anchors.next(&mut f.store);

                    let remaining =
                        seq_with_head(&mut f.store, bit, &rest);
                    let caller = frame(
                        &mut f.store,
                        self.read_down_tag[depth],
                        &[k, offset, remaining],
                    );
                    let node = branch(
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
                    let next = frame(
                        &mut f.store,
                        self.read_down_tag[depth - 1],
                        &[k, offset, next_remaining],
                    );
                    let selected =
                        if bit == f.zero { low } else { high };
                    let after =
                        f.store.ensure_pair(next, selected).unwrap();

                    let mut roles =
                        Vec::with_capacity(depth + 3);
                    roles.extend_from_slice(&[k, offset]);
                    roles.extend_from_slice(&rest);
                    roles.extend_from_slice(&[low, high]);
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

                // Canonical zero-map traversal.
                {
                    let k = anchors.next(&mut f.store);
                    let offset = anchors.next(&mut f.store);
                    let rest =
                        anchors.roles(&mut f.store, depth - 1);
                    let remaining =
                        seq_with_head(&mut f.store, bit, &rest);
                    let caller = frame(
                        &mut f.store,
                        self.read_down_tag[depth],
                        &[k, offset, remaining],
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
                    let next = frame(
                        &mut f.store,
                        self.read_down_tag[depth - 1],
                        &[k, offset, next_remaining],
                    );
                    let after = f
                        .store
                        .ensure_pair(next, self.zero[depth - 1])
                        .unwrap();

                    let mut roles =
                        Vec::with_capacity(depth + 1);
                    roles.extend_from_slice(&[k, offset]);
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

        // Existing PageLeaf -> invoke the already accepted M6a page READ.
        {
            let k = anchors.next(&mut f.store);
            let offset = anchors.next(&mut f.store);
            let page_root = anchors.next(&mut f.store);
            let caller = frame(
                &mut f.store,
                self.read_down_tag[0],
                &[k, offset, ROOT_HANDLE],
            );
            let page_leaf =
                leaf(&mut f.store, self.page_leaf_tag, page_root);
            let before =
                f.store.ensure_pair(caller, page_leaf).unwrap();

            let continuation = frame(
                &mut f.store,
                self.page_read_return_tag,
                &[k],
            );
            let args = materialize_exact_sequence(
                &mut f.store,
                &[page_root, offset],
            )
            .unwrap();
            let invocation =
                call(&mut f.store, f.apply, self.page.read, args);
            let after =
                f.store.ensure_pair(continuation, invocation).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, offset, page_root],
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &[self.page_leaf_tag],
                admission,
            );
        }

        // Zero PageMap leaf resolves to the canonical M6a zero page.
        {
            let k = anchors.next(&mut f.store);
            let offset = anchors.next(&mut f.store);
            let caller = frame(
                &mut f.store,
                self.read_down_tag[0],
                &[k, offset, ROOT_HANDLE],
            );
            let before = f
                .store
                .ensure_pair(caller, self.zero[0])
                .unwrap();

            let continuation = frame(
                &mut f.store,
                self.page_read_return_tag,
                &[k],
            );
            let args = materialize_exact_sequence(
                &mut f.store,
                &[self.page.zero_root, offset],
            )
            .unwrap();
            let invocation =
                call(&mut f.store, f.apply, self.page.read, args);
            let after =
                f.store.ensure_pair(continuation, invocation).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, offset],
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &[self.zero[0]],
                admission,
            );
        }

        // M6a READ_RESULT(Byte8) -> M6b READ32_RESULT(Byte8).
        {
            let k = anchors.next(&mut f.store);
            let byte = anchors.next(&mut f.store);
            let caller = frame(
                &mut f.store,
                self.page_read_return_tag,
                &[k],
            );
            let page_result = tagged(
                &mut f.store,
                self.page.read_result_tag,
                byte,
            );
            let before =
                f.store.ensure_pair(caller, page_result).unwrap();
            let result =
                tagged(&mut f.store, self.read_result_tag, byte);
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
                &[self.page.read_result_tag],
                admission,
            );
        }
    }

    fn install_write(
        &self,
        f: &mut FullFixture,
        anchors: &mut AnchorGen,
    ) {
        // Public WRITE32 validates only Address32 here. Byte8 validation is
        // delegated to the composed M6a WRITE primitive.
        {
            let k = anchors.next(&mut f.store);
            let root = anchors.next(&mut f.store);
            let address = anchors.next(&mut f.store);
            let byte = anchors.next(&mut f.store);
            let args = materialize_exact_sequence(
                &mut f.store,
                &[root, address, byte],
            )
            .unwrap();
            let invocation =
                call(&mut f.store, f.apply, self.write, args);
            let before = f.store.ensure_pair(k, invocation).unwrap();

            let caller = frame(
                &mut f.store,
                self.write_validate_tag[0],
                &[k, root, byte],
            );
            let value = tagged(
                &mut f.store,
                self.write_validate_tag[0],
                address,
            );
            let after = f.store.ensure_pair(caller, value).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, root, address, byte],
                before,
                &[after],
            );
            index_rule_for(&mut f.store, &[f.o], admission);
        }

        for position in 0..ADDRESS_WIDTH {
            for bit in [f.zero, f.one] {
                let k = anchors.next(&mut f.store);
                let root = anchors.next(&mut f.store);
                let byte = anchors.next(&mut f.store);
                let prefix =
                    anchors.roles(&mut f.store, position);
                let rest = anchors.roles(
                    &mut f.store,
                    ADDRESS_WIDTH - position - 1,
                );

                let mut state =
                    Vec::with_capacity(3 + prefix.len());
                state.extend_from_slice(&[k, root, byte]);
                state.extend_from_slice(&prefix);
                let caller = frame(
                    &mut f.store,
                    self.write_validate_tag[position],
                    &state,
                );
                let tail =
                    seq_with_head(&mut f.store, bit, &rest);
                let endpoint = tagged(
                    &mut f.store,
                    self.write_validate_tag[position],
                    tail,
                );
                let before =
                    f.store.ensure_pair(caller, endpoint).unwrap();

                let mut validated =
                    Vec::with_capacity(prefix.len() + 1);
                validated.extend_from_slice(&prefix);
                validated.push(bit);

                let after = if position + 1 < ADDRESS_WIDTH {
                    let mut next_state =
                        Vec::with_capacity(3 + validated.len());
                    next_state.extend_from_slice(&[k, root, byte]);
                    next_state.extend_from_slice(&validated);
                    let next_caller = frame(
                        &mut f.store,
                        self.write_validate_tag[position + 1],
                        &next_state,
                    );
                    let next_tail =
                        materialize_exact_sequence(
                            &mut f.store,
                            &rest,
                        )
                        .unwrap();
                    let next_endpoint = tagged(
                        &mut f.store,
                        self.write_validate_tag[position + 1],
                        next_tail,
                    );
                    f.store
                        .ensure_pair(next_caller, next_endpoint)
                        .unwrap()
                } else {
                    debug_assert_eq!(validated.len(), ADDRESS_WIDTH);
                    let offset = materialize_exact_sequence(
                        &mut f.store,
                        &validated[..OFFSET_BITS],
                    )
                    .unwrap();
                    let page = materialize_exact_sequence(
                        &mut f.store,
                        &validated[OFFSET_BITS..],
                    )
                    .unwrap();
                    let path =
                        materialize_exact_sequence(&mut f.store, &[])
                            .unwrap();
                    let down = frame(
                        &mut f.store,
                        self.write_down_tag[PAGE_BITS],
                        &[k, page, offset, byte, path],
                    );
                    f.store.ensure_pair(down, root).unwrap()
                };

                let mut roles =
                    Vec::with_capacity(3 + prefix.len() + rest.len());
                roles.extend_from_slice(&[k, root, byte]);
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
                    &[self.write_validate_tag[position]],
                    admission,
                );
            }
        }

        // Descend sparse Page24 selector and accumulate path siblings.
        for depth in (1..=PAGE_BITS).rev() {
            let consumed = PAGE_BITS - depth;
            for bit in [f.zero, f.one] {
                // Existing branch.
                {
                    let k = anchors.next(&mut f.store);
                    let offset = anchors.next(&mut f.store);
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
                        self.write_down_tag[depth],
                        &[k, remaining, offset, byte, path_seq],
                    );
                    let node = branch(
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
                    let next = frame(
                        &mut f.store,
                        self.write_down_tag[depth - 1],
                        &[k, next_remaining, offset, byte, next_path],
                    );
                    let after =
                        f.store.ensure_pair(next, selected).unwrap();

                    let mut roles =
                        Vec::with_capacity(5 + path.len() + rest.len());
                    roles.extend_from_slice(&[k, offset, byte]);
                    roles.extend_from_slice(&path);
                    roles.extend_from_slice(&rest);
                    roles.extend_from_slice(&[low, high]);
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

                // Lazy expansion of canonical zero page-map subtree.
                {
                    let k = anchors.next(&mut f.store);
                    let offset = anchors.next(&mut f.store);
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
                        self.write_down_tag[depth],
                        &[k, remaining, offset, byte, path_seq],
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
                    let next = frame(
                        &mut f.store,
                        self.write_down_tag[depth - 1],
                        &[k, next_remaining, offset, byte, next_path],
                    );
                    let after = f
                        .store
                        .ensure_pair(next, self.zero[depth - 1])
                        .unwrap();

                    let mut roles =
                        Vec::with_capacity(3 + path.len() + rest.len());
                    roles.extend_from_slice(&[k, offset, byte]);
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

        // Existing PageLeaf: delegate byte update to M6a page WRITE.
        {
            let k = anchors.next(&mut f.store);
            let offset = anchors.next(&mut f.store);
            let byte = anchors.next(&mut f.store);
            let path = anchors.roles(&mut f.store, PAGE_BITS);
            let path_seq =
                materialize_exact_sequence(&mut f.store, &path)
                    .unwrap();
            let page_root = anchors.next(&mut f.store);

            let caller = frame(
                &mut f.store,
                self.write_down_tag[0],
                &[k, ROOT_HANDLE, offset, byte, path_seq],
            );
            let page_leaf =
                leaf(&mut f.store, self.page_leaf_tag, page_root);
            let before =
                f.store.ensure_pair(caller, page_leaf).unwrap();

            let continuation = frame(
                &mut f.store,
                self.page_write_return_tag,
                &[k, path_seq],
            );
            let args = materialize_exact_sequence(
                &mut f.store,
                &[page_root, offset, byte],
            )
            .unwrap();
            let invocation =
                call(&mut f.store, f.apply, self.page.write, args);
            let after =
                f.store.ensure_pair(continuation, invocation).unwrap();

            let mut roles = Vec::with_capacity(PAGE_BITS + 4);
            roles.extend_from_slice(&[k, offset, byte, page_root]);
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
                &[self.page_leaf_tag],
                admission,
            );
        }

        // Zero PageMap leaf delegates to M6a zero page WRITE.
        {
            let k = anchors.next(&mut f.store);
            let offset = anchors.next(&mut f.store);
            let byte = anchors.next(&mut f.store);
            let path = anchors.roles(&mut f.store, PAGE_BITS);
            let path_seq =
                materialize_exact_sequence(&mut f.store, &path)
                    .unwrap();

            let caller = frame(
                &mut f.store,
                self.write_down_tag[0],
                &[k, ROOT_HANDLE, offset, byte, path_seq],
            );
            let before = f
                .store
                .ensure_pair(caller, self.zero[0])
                .unwrap();

            let continuation = frame(
                &mut f.store,
                self.page_write_return_tag,
                &[k, path_seq],
            );
            let args = materialize_exact_sequence(
                &mut f.store,
                &[self.page.zero_root, offset, byte],
            )
            .unwrap();
            let invocation =
                call(&mut f.store, f.apply, self.page.write, args);
            let after =
                f.store.ensure_pair(continuation, invocation).unwrap();

            let mut roles = Vec::with_capacity(PAGE_BITS + 3);
            roles.extend_from_slice(&[k, offset, byte]);
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

        // M6a WRITE_RESULT(PageRoot') -> PageLeaf(PageRoot') -> rebuild Page24.
        {
            let k = anchors.next(&mut f.store);
            let path = anchors.next(&mut f.store);
            let new_page_root = anchors.next(&mut f.store);
            let caller = frame(
                &mut f.store,
                self.page_write_return_tag,
                &[k, path],
            );
            let page_result = tagged(
                &mut f.store,
                self.page.write_result_tag,
                new_page_root,
            );
            let before =
                f.store.ensure_pair(caller, page_result).unwrap();

            let page_leaf =
                leaf(&mut f.store, self.page_leaf_tag, new_page_root);
            let up_caller = frame(
                &mut f.store,
                self.write_up_tag[PAGE_BITS],
                &[k, path],
            );
            let up_value = tagged(
                &mut f.store,
                self.write_up_tag[PAGE_BITS],
                page_leaf,
            );
            let after =
                f.store.ensure_pair(up_caller, up_value).unwrap();

            let (_, admission) = define_bundle_rule(
                &mut f.store,
                f.theory,
                &[k, path, new_page_root],
                before,
                &[after],
            );
            index_rule_for(
                &mut f.store,
                &[self.page.write_result_tag],
                admission,
            );
        }

        // Persistent Page24 path rebuild.
        for level in (1..=PAGE_BITS).rev() {
            let branch_depth = PAGE_BITS - level + 1;
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
                    self.write_up_tag[level],
                    &[k, path_seq],
                );
                let up_value = tagged(
                    &mut f.store,
                    self.write_up_tag[level],
                    child,
                );
                let before =
                    f.store.ensure_pair(caller, up_value).unwrap();

                let node = if bit == f.zero {
                    branch(
                        &mut f.store,
                        self.branch_tag[branch_depth],
                        child,
                        sibling,
                    )
                } else {
                    branch(
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
                        self.write_up_tag[level - 1],
                        &[k, next_path],
                    );
                    let next_value = tagged(
                        &mut f.store,
                        self.write_up_tag[level - 1],
                        node,
                    );
                    f.store
                        .ensure_pair(next_caller, next_value)
                        .unwrap()
                } else {
                    let result = tagged(
                        &mut f.store,
                        self.write_result_tag,
                        node,
                    );
                    f.store.ensure_pair(k, result).unwrap()
                };

                let mut roles =
                    Vec::with_capacity(level + 3);
                roles.push(k);
                roles.extend_from_slice(&prefix);
                roles.extend_from_slice(&[sibling, child]);
                let (_, admission) = define_bundle_rule(
                    &mut f.store,
                    f.theory,
                    &roles,
                    before,
                    &[after],
                );
                index_rule_for(
                    &mut f.store,
                    &[self.write_up_tag[level]],
                    admission,
                );
            }
        }
    }
}

fn address32(
    f: &mut FullFixture,
    value: u32,
) -> Handle {
    let bits = (0..ADDRESS_WIDTH)
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

fn byte8(
    f: &mut FullFixture,
    value: u8,
) -> Handle {
    let bits = (0..OFFSET_BITS)
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

fn decode_byte8(
    f: &FullFixture,
    value: Handle,
) -> Option<u8> {
    let bits = read_exact_sequence(&f.store, value).ok()?;
    if bits.len() != OFFSET_BITS {
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


fn decode_byte8_store(
    store: &OptimizedLinkStore,
    zero: Handle,
    one: Handle,
    value: Handle,
) -> Option<u8> {
    let bits = read_exact_sequence(store, value).ok()?;
    if bits.len() != OFFSET_BITS {
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

fn runtime_read32(
    memory: &mut ProofRuntimeMemory,
    interpreter: Handle,
    apply: Handle,
    read: Handle,
    read_result_tag: Handle,
    caller: Handle,
    root: Handle,
    address: Handle,
    zero: Handle,
    one: Handle,
) -> Option<u8> {
    let args =
        materialize_exact_sequence(&mut memory.store, &[root, address])
            .ok()?;
    let invocation =
        call(&mut memory.store, apply, read, args);
    let initial =
        memory.store.ensure_pair(caller, invocation).ok()?;
    let (engine, execute) = execute_to_quiescence(
        memory,
        interpreter,
        initial,
        32,
        160,
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
    let (tag, byte) = memory.store.poles(envelope).ok()?;
    if tag != read_result_tag {
        return None;
    }
    decode_byte8_store(&memory.store, zero, one, byte)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct WebMemory32Outcome {
    pub(crate) address: u32,
    pub(crate) page24: u32,
    pub(crate) offset8: u32,
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
pub(crate) struct WebMemory32Execution {
    pub(crate) outcome: WebMemory32Outcome,
    pub(crate) proof: WebStructuralProof,
}

pub(crate) fn web_prove_memory32(
    address_value: u32,
    byte_value: u8,
) -> Option<WebMemory32Execution> {
    let mut compiler = FullFixture::new();
    let program = Memory32Program::install(&mut compiler);
    let address = address32(&mut compiler, address_value);
    let byte = byte8(&mut compiler, byte_value);

    let args = materialize_exact_sequence(
        &mut compiler.store,
        &[program.zero_root, address, byte],
    )
    .ok()?;
    let invocation =
        call(&mut compiler.store, compiler.apply, program.write, args);
    let initial =
        compiler.store.ensure_pair(compiler.k, invocation).ok()?;

    let prepared_roots = vec![
        semantic_source(
            &compiler.store,
            "function.memory32.read",
            program.read,
        ),
        semantic_source(
            &compiler.store,
            "function.memory32.write",
            program.write,
        ),
        semantic_source(
            &compiler.store,
            "memory32.zero_root",
            program.zero_root,
        ),
        semantic_source(
            &compiler.store,
            "memory32.read_result_tag",
            program.read_result_tag,
        ),
        semantic_source(
            &compiler.store,
            "memory32.write_result_tag",
            program.write_result_tag,
        ),
        semantic_source(
            &compiler.store,
            "data.address32",
            address,
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
    let apply = loaded_handle(&load, "execution.apply")?;
    let read = loaded_handle(&load, "function.memory32.read")?;
    let initial = loaded_handle(&load, "scope.initial")?;
    let old_root = loaded_handle(&load, "memory32.zero_root")?;
    let address = loaded_handle(&load, "data.address32")?;
    let read_result_tag =
        loaded_handle(&load, "memory32.read_result_tag")?;
    let write_result_tag =
        loaded_handle(&load, "memory32.write_result_tag")?;
    let zero = loaded_handle(&load, "data.bit.zero")?;
    let one = loaded_handle(&load, "data.bit.one")?;
    let result_context =
        loaded_handle(&load, "context.result")?;

    let before_value = runtime_read32(
        &mut memory,
        interpreter,
        apply,
        read,
        read_result_tag,
        result_context,
        old_root,
        address,
        zero,
        one,
    )?;

    let (mut engine, execute) = execute_to_quiescence(
        &mut memory,
        interpreter,
        initial,
        32,
        192,
    )?;
    if !execute.final_quiescent || engine.current().len() != 1 {
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
        192,
    )?;

    // Read-back is an independent verification pass over the exact runtime
    // carrier state. It must not extend the authoritative proof memory after
    // EXECUTE, otherwise compact-proof link accounting would include
    // verification-only Links that are absent from the recorded reaction trace.
    // Cloning preserves all semantic/local Link identities while giving the
    // verifier its own non-semantic store/cache identity.
    let mut verification_memory = ProofRuntimeMemory {
        id: memory.id.clone(),
        store: memory.store.clone(),
    };
    let after_value = runtime_read32(
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
    )?;
    let old_after_value = runtime_read32(
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
    )?;

    let oracle_matches =
        before_value == 0
            && after_value == byte_value
            && old_after_value == 0
            && old_root != new_root
            && identical_rerun_link_delta == 0;

    let visual_links =
        visual_snapshot(&memory, &load.semantic_roots);
    let proof = WebStructuralProof {
        schema_version: 4,
        block: "M6B_MEMORY32".to_owned(),
        prepare,
        load,
        execute,
        result: WebProofResultStage {
            memory_instance_id: memory.id.clone(),
            result_anum: result_recursive_wire,
            // Address32 is a real ExactSequence and is retained as the
            // sequence witness for this write-result proof. MemoryRoot' is an
            // arbitrary recursive Link and therefore stays in the recursive
            // wire field above rather than being mislabeled as an Anum.
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

    Some(WebMemory32Execution {
        outcome: WebMemory32Outcome {
            address: address_value,
            page24: address_value >> OFFSET_BITS,
            offset8: address_value & 0xff,
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

        for _ in 0..192 {
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
        p: &Memory32Program,
        root: Handle,
        address: u32,
    ) -> Option<u8> {
        let address = address32(f, address);
        let envelope = invoke(f, p.read, &[root, address])?;
        let (tag, byte) = f.store.poles(envelope).ok()?;
        if tag != p.read_result_tag {
            return None;
        }
        decode_byte8(f, byte)
    }

    fn write(
        f: &mut FullFixture,
        p: &Memory32Program,
        root: Handle,
        address: u32,
        value: u8,
    ) -> Option<Handle> {
        let address = address32(f, address);
        let byte = byte8(f, value);
        let envelope =
            invoke(f, p.write, &[root, address, byte])?;
        let (tag, new_root) = f.store.poles(envelope).ok()?;
        (tag == p.write_result_tag).then_some(new_root)
    }

    fn root_children(
        f: &FullFixture,
        p: &Memory32Program,
        root: Handle,
    ) -> Option<(Handle, Handle)> {
        let (tag, children) = f.store.poles(root).ok()?;
        if tag != p.branch_tag[PAGE_BITS] {
            return None;
        }
        f.store.poles(children).ok()
    }

    #[test]
    fn m6b_real_one_memory_proof_uses_address32_and_persists_old_root() {
        let execution =
            web_prove_memory32(0x0012_3425, 0xab).unwrap();
        assert_eq!(execution.outcome.address, 0x0012_3425);
        assert_eq!(execution.outcome.page24, 0x0012_34);
        assert_eq!(execution.outcome.offset8, 0x25);
        assert_eq!(execution.outcome.write_value, 0xab);
        assert_eq!(execution.outcome.before_value, 0);
        assert_eq!(execution.outcome.after_value, 0xab);
        assert_eq!(execution.outcome.old_after_value, 0);
        assert_ne!(
            execution.outcome.old_root_ref,
            execution.outcome.new_root_ref
        );
        assert_eq!(execution.outcome.steady_link_delta, 0);
        assert_eq!(execution.outcome.quiescent, 1);
        assert_eq!(execution.proof.block, "M6B_MEMORY32");
        assert!(execution.proof.result.oracle_matches);
        assert!(execution.proof.execute.active_reaction_count > 80);
        assert!(
            execution.proof.compact().is_some(),
            "post-execute verification must not escape compact proof accounting"
        );
        assert_eq!(
            execution.proof.load.memory_instance_id,
            execution.proof.result.memory_instance_id
        );
    }

    #[test]
    fn m6b_zero_memory_reads_zero_across_distinct_pages() {
        let mut f = FullFixture::new();
        let p = Memory32Program::install(&mut f);
        for address in [
            0x0000_0000,
            0x0000_0025,
            0x0001_0025,
            0x0012_3425,
            0xffff_ff25,
        ] {
            assert_eq!(read(&mut f, &p, p.zero_root, address), Some(0));
        }
    }

    #[test]
    fn m6b_write_read_persists_across_pages_and_old_roots() {
        let mut f = FullFixture::new();
        let p = Memory32Program::install(&mut f);
        let zero = p.zero_root;

        let root1 =
            write(&mut f, &p, zero, 0x0012_3425, 0xab).unwrap();
        assert_ne!(root1, zero);
        assert_eq!(read(&mut f, &p, root1, 0x0012_3425), Some(0xab));
        assert_eq!(read(&mut f, &p, root1, 0x0012_3424), Some(0));
        assert_eq!(read(&mut f, &p, root1, 0x0012_3525), Some(0));
        assert_eq!(read(&mut f, &p, zero, 0x0012_3425), Some(0));

        let root2 =
            write(&mut f, &p, root1, 0x00ab_cd25, 0x7c).unwrap();
        assert_eq!(read(&mut f, &p, root2, 0x0012_3425), Some(0xab));
        assert_eq!(read(&mut f, &p, root2, 0x00ab_cd25), Some(0x7c));

        let root3 =
            write(&mut f, &p, root2, 0x0012_3425, 0xcd).unwrap();
        assert_eq!(read(&mut f, &p, root3, 0x0012_3425), Some(0xcd));
        assert_eq!(read(&mut f, &p, root3, 0x00ab_cd25), Some(0x7c));
        assert_eq!(read(&mut f, &p, root2, 0x0012_3425), Some(0xab));
    }

    #[test]
    fn m6b_page_map_reuses_untouched_root_sibling() {
        let mut f = FullFixture::new();
        let p = Memory32Program::install(&mut f);

        // Page 1 has Page24 bit0=1; page 0 has Page24 bit0=0.
        let root1 =
            write(&mut f, &p, p.zero_root, 0x0000_0125, 0xab).unwrap();
        let (low1, high1) = root_children(&f, &p, root1).unwrap();
        assert_eq!(low1, p.zero[PAGE_BITS - 1]);
        assert_ne!(high1, p.zero[PAGE_BITS - 1]);

        let root2 =
            write(&mut f, &p, root1, 0x0000_0025, 0x7c).unwrap();
        let (low2, high2) = root_children(&f, &p, root2).unwrap();
        assert_ne!(low2, low1);
        assert_eq!(
            high2, high1,
            "untouched Page24 sibling must be structurally reused"
        );
    }

    #[test]
    fn m6b_identical_write_canonicalizes_without_link_growth() {
        let mut f = FullFixture::new();
        let p = Memory32Program::install(&mut f);
        let root1 =
            write(&mut f, &p, p.zero_root, 0x0012_3425, 0xab).unwrap();

        let address = address32(&mut f, 0x0012_3425);
        let byte = byte8(&mut f, 0xab);
        let args = materialize_exact_sequence(
            &mut f.store,
            &[root1, address, byte],
        )
        .unwrap();
        let invocation =
            call(&mut f.store, f.apply, p.write, args);
        let initial = f.store.ensure_pair(f.k, invocation).unwrap();

        f.engine.set_current(&f.store, &[initial]).unwrap();
        for _ in 0..192 {
            let reaction = f.engine.run(&mut f.store).unwrap();
            if reaction.quiescent {
                break;
            }
        }
        assert!(f.engine.quiescent());
        let final_member = f.engine.current()[0];
        let (_, envelope) = f.store.poles(final_member).unwrap();
        let (tag, first_root) = f.store.poles(envelope).unwrap();
        assert_eq!(tag, p.write_result_tag);
        assert_eq!(first_root, root1);

        let links = f.store.link_count();
        f.engine.set_current(&f.store, &[initial]).unwrap();
        for _ in 0..192 {
            let reaction = f.engine.run(&mut f.store).unwrap();
            if reaction.quiescent {
                break;
            }
        }
        let final_member = f.engine.current()[0];
        let (_, envelope) = f.store.poles(final_member).unwrap();
        let (tag, second_root) = f.store.poles(envelope).unwrap();
        assert_eq!(tag, p.write_result_tag);
        assert_eq!(second_root, root1);
        assert_eq!(f.store.link_count(), links);
    }

    #[test]
    fn m6b_malformed_address_and_byte_fail_closed() {
        let mut f = FullFixture::new();
        let p = Memory32Program::install(&mut f);

        let short_address = materialize_exact_sequence(
            &mut f.store,
            &[f.zero; ADDRESS_WIDTH - 1],
        )
        .unwrap();
        assert!(
            invoke(&mut f, p.read, &[p.zero_root, short_address])
                .is_none()
        );

        let address = address32(&mut f, 0x0012_3425);
        let short_byte = materialize_exact_sequence(
            &mut f.store,
            &[f.one; OFFSET_BITS - 1],
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

        let wrong_depth = p.zero[PAGE_BITS - 1];
        let address = address32(&mut f, 0x0012_3425);
        assert!(
            invoke(&mut f, p.read, &[wrong_depth, address])
                .is_none()
        );
    }

    #[test]
    fn m6b_equivalent_memories_have_same_recursive_wire() {
        fn build(extra_noise: bool) -> String {
            let mut f = FullFixture::new();
            if extra_noise {
                let noise = f.store.ensure_pair(f.k, f.full).unwrap();
                let _ = f.store.ensure_pair(noise, f.o).unwrap();
            }
            let p = Memory32Program::install(&mut f);
            let root1 =
                write(&mut f, &p, p.zero_root, 0x0012_3425, 0xab)
                    .unwrap();
            let root2 =
                write(&mut f, &p, root1, 0x00ab_cd25, 0x7c)
                    .unwrap();
            f.store.export_anum(root2).unwrap()
        }

        assert_eq!(build(false), build(true));
    }
}
