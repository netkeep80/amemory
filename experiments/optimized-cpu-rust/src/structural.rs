use crate::{Handle, OptimizedLinkStore, StoreError, ROOT_HANDLE};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StructuralError {
    Store(StoreError),
    DuplicateRole,
    InvalidRoleDictionary(Handle),
    InvalidRule(Handle),
    RuleNotAdmitted,
    TemplateMismatch,
    MissingRoleBinding(Handle),
    InvalidExactSequence(Handle),
    UnsupportedCycle(Handle),
    InvalidInterpreter(Handle),
    MissingInterpreter,
    ScopeCapacity { requested: usize, cap: usize },
}

impl From<StoreError> for StructuralError {
    fn from(value: StoreError) -> Self {
        Self::Store(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StructuralRoleBinding {
    pub role: Handle,
    pub value: Handle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StructuralInterpreter {
    pub dictionary: Handle,
    pub grammar: Handle,
    pub theory: Handle,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructuralReactionResult {
    pub old_members: Vec<Handle>,
    pub next_members: Vec<Handle>,
    pub raw_rule_matches: u32,
    pub transitioned_members: u32,
    pub quiescent: bool,
    pub handoff_count: u32,
}

#[derive(Clone, Debug)]
struct StructuralImage {
    output_bundle_template: Handle,
    bindings: Vec<StructuralRoleBinding>,
}

pub fn materialize_exact_sequence(
    store: &mut OptimizedLinkStore,
    values: &[Handle],
) -> Result<Handle, StructuralError> {
    let mut current = ROOT_HANDLE;
    for value in values {
        if !store.is_valid(*value) {
            return Err(StructuralError::Store(StoreError::UnknownHandle(*value)));
        }
        let payload = store.ensure_pair(current, *value)?;
        current = store.ensure_start_self_closed(payload)?;
    }
    Ok(current)
}

pub fn read_exact_sequence(
    store: &OptimizedLinkStore,
    final_link: Handle,
) -> Result<Vec<Handle>, StructuralError> {
    if final_link == ROOT_HANDLE {
        return Ok(Vec::new());
    }
    if !store.is_valid(final_link) {
        return Err(StructuralError::Store(StoreError::UnknownHandle(final_link)));
    }

    let mut reversed = Vec::new();
    let mut visited = HashSet::new();
    let mut current = final_link;

    while current != ROOT_HANDLE {
        if !visited.insert(current) {
            return Err(StructuralError::InvalidExactSequence(final_link));
        }

        let (cell_start, cell_end) = store.poles(current)?;
        if cell_start != current {
            return Err(StructuralError::InvalidExactSequence(final_link));
        }

        let (previous, value) = store.poles(cell_end)?;
        reversed.push(value);
        current = previous;
    }

    reversed.reverse();
    Ok(reversed)
}

pub fn define_structural_role_dictionary(
    store: &mut OptimizedLinkStore,
    roles: &[Handle],
) -> Result<Handle, StructuralError> {
    let mut unique = HashSet::new();
    for role in roles {
        if !store.is_valid(*role) {
            return Err(StructuralError::Store(StoreError::UnknownHandle(*role)));
        }
        if !unique.insert(*role) {
            return Err(StructuralError::DuplicateRole);
        }
    }

    let sequence = materialize_exact_sequence(store, roles)?;
    Ok(store.ensure_start_self_closed(sequence)?)
}

pub fn read_structural_role_dictionary(
    store: &OptimizedLinkStore,
    dictionary: Handle,
) -> Result<Vec<Handle>, StructuralError> {
    let (start, end) = store.poles(dictionary)?;
    if start != dictionary || end == dictionary {
        return Err(StructuralError::InvalidRoleDictionary(dictionary));
    }

    let roles = read_exact_sequence(store, end)?;
    let mut unique = HashSet::new();
    for role in &roles {
        if !unique.insert(*role) {
            return Err(StructuralError::DuplicateRole);
        }
    }
    Ok(roles)
}

pub fn define_structural_rule(
    store: &mut OptimizedLinkStore,
    role_dictionary: Handle,
    body: Handle,
) -> Result<Handle, StructuralError> {
    Ok(store.ensure_pair(role_dictionary, body)?)
}

pub fn admit_structural_rule(
    store: &mut OptimizedLinkStore,
    theory: Handle,
    rule: Handle,
) -> Result<Handle, StructuralError> {
    Ok(store.ensure_pair(theory, rule)?)
}

pub fn index_structural_rule_trigger(
    store: &mut OptimizedLinkStore,
    trigger_key: Handle,
    admission: Handle,
) -> Result<Handle, StructuralError> {
    Ok(store.ensure_pair(trigger_key, admission)?)
}

pub fn define_structural_interpreter(
    store: &mut OptimizedLinkStore,
    dictionary: Handle,
    grammar: Handle,
    theory: Handle,
) -> Result<Handle, StructuralError> {
    let grammar_theory = store.ensure_pair(grammar, theory)?;
    Ok(store.ensure_pair(dictionary, grammar_theory)?)
}

pub fn read_structural_interpreter(
    store: &OptimizedLinkStore,
    interpreter: Handle,
) -> Result<StructuralInterpreter, StructuralError> {
    let (dictionary, grammar_theory) = store
        .poles(interpreter)
        .map_err(|_| StructuralError::InvalidInterpreter(interpreter))?;
    let (grammar, theory) = store
        .poles(grammar_theory)
        .map_err(|_| StructuralError::InvalidInterpreter(interpreter))?;

    Ok(StructuralInterpreter {
        dictionary,
        grammar,
        theory,
    })
}

fn contains_role(
    store: &OptimizedLinkStore,
    node: Handle,
    roles: &HashSet<Handle>,
    memo: &mut HashMap<Handle, bool>,
    active: &mut HashSet<Handle>,
) -> Result<bool, StructuralError> {
    if roles.contains(&node) {
        return Ok(true);
    }
    if let Some(value) = memo.get(&node) {
        return Ok(*value);
    }
    if !active.insert(node) {
        return Ok(false);
    }

    let result = (|| {
        let (start, end) = store.poles(node)?;
        if contains_role(store, start, roles, memo, active)? {
            return Ok(true);
        }
        contains_role(store, end, roles, memo, active)
    })();

    active.remove(&node);
    let result = result?;
    memo.insert(node, result);
    Ok(result)
}

fn unify_node(
    store: &OptimizedLinkStore,
    template: Handle,
    claimed: Handle,
    roles: &HashSet<Handle>,
    inferred: &mut HashMap<Handle, Handle>,
    contains_memo: &mut HashMap<Handle, bool>,
    contains_active: &mut HashSet<Handle>,
    visited: &mut HashSet<(Handle, Handle)>,
) -> Result<(), StructuralError> {
    if roles.contains(&template) {
        if let Some(previous) = inferred.get(&template) {
            if *previous != claimed {
                return Err(StructuralError::TemplateMismatch);
            }
        } else {
            inferred.insert(template, claimed);
        }
        return Ok(());
    }

    if !contains_role(
        store,
        template,
        roles,
        contains_memo,
        contains_active,
    )? {
        if template != claimed {
            return Err(StructuralError::TemplateMismatch);
        }
        return Ok(());
    }

    if !visited.insert((template, claimed)) {
        return Ok(());
    }

    let (template_start, template_end) = store.poles(template)?;
    let (claimed_start, claimed_end) = store.poles(claimed)?;

    if (template_start == template) != (claimed_start == claimed)
        || (template_end == template) != (claimed_end == claimed)
    {
        return Err(StructuralError::TemplateMismatch);
    }

    unify_node(
        store,
        template_start,
        claimed_start,
        roles,
        inferred,
        contains_memo,
        contains_active,
        visited,
    )?;
    unify_node(
        store,
        template_end,
        claimed_end,
        roles,
        inferred,
        contains_memo,
        contains_active,
        visited,
    )
}

pub fn unify_structural_rule_template(
    store: &OptimizedLinkStore,
    template: Handle,
    claimed: Handle,
    roles: &[Handle],
) -> Result<Vec<StructuralRoleBinding>, StructuralError> {
    let mut role_set = HashSet::new();
    for role in roles {
        if !role_set.insert(*role) {
            return Err(StructuralError::DuplicateRole);
        }
    }

    let mut inferred = HashMap::new();
    let mut contains_memo = HashMap::new();
    let mut contains_active = HashSet::new();
    let mut visited = HashSet::new();

    unify_node(
        store,
        template,
        claimed,
        &role_set,
        &mut inferred,
        &mut contains_memo,
        &mut contains_active,
        &mut visited,
    )?;

    roles
        .iter()
        .map(|role| {
            let value = inferred
                .get(role)
                .copied()
                .ok_or(StructuralError::MissingRoleBinding(*role))?;
            Ok(StructuralRoleBinding {
                role: *role,
                value,
            })
        })
        .collect()
}

fn instantiate_node(
    store: &mut OptimizedLinkStore,
    source: Handle,
    bindings: &HashMap<Handle, Handle>,
    visiting: &mut HashSet<Handle>,
    memo: &mut HashMap<Handle, Handle>,
) -> Result<Handle, StructuralError> {
    if let Some(bound) = bindings.get(&source) {
        return Ok(*bound);
    }
    if let Some(value) = memo.get(&source) {
        return Ok(*value);
    }

    let (start, end) = store.poles(source)?;

    let value = if start == source && end == source {
        ROOT_HANDLE
    } else if start == source {
        let child = instantiate_node(store, end, bindings, visiting, memo)?;
        store.ensure_start_self_closed(child)?
    } else if end == source {
        let child = instantiate_node(store, start, bindings, visiting, memo)?;
        store.ensure_end_self_closed(child)?
    } else {
        if !visiting.insert(source) {
            return Err(StructuralError::UnsupportedCycle(source));
        }
        let new_start = instantiate_node(store, start, bindings, visiting, memo)?;
        let new_end = instantiate_node(store, end, bindings, visiting, memo)?;
        visiting.remove(&source);
        store.ensure_pair(new_start, new_end)?
    };

    memo.insert(source, value);
    Ok(value)
}

pub fn instantiate_structural_template(
    store: &mut OptimizedLinkStore,
    template: Handle,
    bindings: &[StructuralRoleBinding],
) -> Result<Handle, StructuralError> {
    let mut mapping = HashMap::new();
    for binding in bindings {
        if let Some(previous) = mapping.insert(binding.role, binding.value) {
            if previous != binding.value {
                return Err(StructuralError::TemplateMismatch);
            }
        }
    }

    instantiate_node(
        store,
        template,
        &mapping,
        &mut HashSet::new(),
        &mut HashMap::new(),
    )
}

fn discover_triggered_rule_images(
    store: &OptimizedLinkStore,
    theory: Handle,
    active: Handle,
) -> Result<Vec<StructuralImage>, StructuralError> {
    let (_, endpoint) = store.poles(active)?;
    let (trigger_key, _) = store.poles(endpoint)?;

    let triggers = store
        .start_incidence(trigger_key)?
        .collect::<Vec<_>>();

    let mut images = Vec::new();

    for trigger in triggers {
        if trigger == trigger_key {
            continue;
        }

        let Ok((trigger_start, admission)) = store.poles(trigger) else {
            continue;
        };
        if trigger_start != trigger_key {
            continue;
        }

        let Ok((admission_theory, rule)) = store.poles(admission) else {
            continue;
        };
        if admission_theory != theory || rule == admission {
            continue;
        }

        let Ok((role_dictionary, body)) = store.poles(rule) else {
            continue;
        };
        let Ok(roles) = read_structural_role_dictionary(store, role_dictionary) else {
            continue;
        };
        let Ok((before, output_bundle_template)) = store.poles(body) else {
            continue;
        };

        let Ok(bindings) =
            unify_structural_rule_template(store, before, active, &roles)
        else {
            continue;
        };

        images.push(StructuralImage {
            output_bundle_template,
            bindings,
        });
    }

    Ok(images)
}

#[derive(Clone, Debug)]
pub struct OptimizedStructuralEngine {
    cap: usize,
    scope_banks: [Vec<Handle>; 2],
    current_bank: usize,
    interpreter: Option<Handle>,
    raw_rule_matches: u32,
    transitioned_members: u32,
    handoff_count: u32,
    quiescent: bool,
}

impl OptimizedStructuralEngine {
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0);
        Self {
            cap,
            scope_banks: [Vec::new(), Vec::new()],
            current_bank: 0,
            interpreter: None,
            raw_rule_matches: 0,
            transitioned_members: 0,
            handoff_count: 0,
            quiescent: false,
        }
    }

    pub fn set_interpreter(
        &mut self,
        store: &OptimizedLinkStore,
        interpreter: Handle,
    ) -> Result<(), StructuralError> {
        read_structural_interpreter(store, interpreter)?;
        self.interpreter = Some(interpreter);
        Ok(())
    }

    pub fn set_current(
        &mut self,
        store: &OptimizedLinkStore,
        members: &[Handle],
    ) -> Result<(), StructuralError> {
        if members.len() > self.cap {
            return Err(StructuralError::ScopeCapacity {
                requested: members.len(),
                cap: self.cap,
            });
        }

        let mut seen = HashSet::new();
        let mut normalized = Vec::new();
        for member in members {
            if !store.is_valid(*member) {
                return Err(StructuralError::Store(StoreError::UnknownHandle(*member)));
            }
            if seen.insert(*member) {
                normalized.push(*member);
            }
        }
        self.scope_banks[self.current_bank] = normalized;
        Ok(())
    }

    pub fn run(
        &mut self,
        store: &mut OptimizedLinkStore,
    ) -> Result<StructuralReactionResult, StructuralError> {
        self.quiescent = false;

        let interpreter = self.interpreter.ok_or(StructuralError::MissingInterpreter)?;
        let authority = read_structural_interpreter(store, interpreter)?;
        let old_members = self.scope_banks[self.current_bank].clone();

        if old_members.len() > self.cap {
            return Err(StructuralError::ScopeCapacity {
                requested: old_members.len(),
                cap: self.cap,
            });
        }

        let mut next_members = Vec::new();
        let mut next_seen = HashSet::new();
        let mut raw_rule_matches = 0u32;
        let mut transitioned_members = 0u32;

        let mut add_next = |link: Handle| -> Result<(), StructuralError> {
            if next_seen.insert(link) {
                if next_members.len() >= self.cap {
                    return Err(StructuralError::ScopeCapacity {
                        requested: next_members.len() + 1,
                        cap: self.cap,
                    });
                }
                next_members.push(link);
            }
            Ok(())
        };

        for active in old_members.iter().copied() {
            let images = discover_triggered_rule_images(store, authority.theory, active)?;

            if images.is_empty() {
                add_next(active)?;
                continue;
            }

            transitioned_members = transitioned_members.saturating_add(1);

            for image in images {
                raw_rule_matches = raw_rule_matches.saturating_add(1);
                let grounded_bundle = instantiate_structural_template(
                    store,
                    image.output_bundle_template,
                    &image.bindings,
                )?;
                let outputs = read_exact_sequence(store, grounded_bundle)?;
                for successor in outputs {
                    add_next(successor)?;
                }
            }
        }

        self.raw_rule_matches = raw_rule_matches;
        self.transitioned_members = transitioned_members;
        self.handoff_count = 0;

        if raw_rule_matches == 0 {
            self.quiescent = true;
            return Ok(StructuralReactionResult {
                old_members: old_members.clone(),
                next_members: old_members,
                raw_rule_matches,
                transitioned_members,
                quiescent: true,
                handoff_count: 0,
            });
        }

        let target_bank = 1usize - self.current_bank;
        self.scope_banks[target_bank] = next_members.clone();
        self.current_bank = target_bank;
        self.handoff_count = 1;

        Ok(StructuralReactionResult {
            old_members,
            next_members,
            raw_rule_matches,
            transitioned_members,
            quiescent: false,
            handoff_count: 1,
        })
    }

    pub fn current(&self) -> &[Handle] {
        &self.scope_banks[self.current_bank]
    }

    pub fn current_bank(&self) -> usize {
        self.current_bank
    }

    pub fn bank(&self, bank: usize) -> Option<&[Handle]> {
        self.scope_banks.get(bank).map(Vec::as_slice)
    }

    pub fn raw_rule_matches(&self) -> u32 {
        self.raw_rule_matches
    }

    pub fn transitioned_members(&self) -> u32 {
        self.transitioned_members
    }

    pub fn handoff_count(&self) -> u32 {
        self.handoff_count
    }

    pub fn quiescent(&self) -> bool {
        self.quiescent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh(store: &mut OptimizedLinkStore, count: usize) -> Vec<Handle> {
        let o = store.ensure_start_self_closed(ROOT_HANDLE).unwrap();
        let c = store.ensure_end_self_closed(ROOT_HANDLE).unwrap();
        let mut seed = store.ensure_pair(c, o).unwrap();
        let mut result = Vec::new();
        for i in 0..count {
            seed = store
                .ensure_pair(seed, if i % 2 == 0 { o } else { c })
                .unwrap();
            result.push(seed);
        }
        result
    }

    #[test]
    fn exact_sequence_is_root_originating_and_ordered() {
        let mut store = OptimizedLinkStore::new();
        let o = store.ensure_start_self_closed(ROOT_HANDLE).unwrap();
        let c = store.ensure_end_self_closed(ROOT_HANDLE).unwrap();

        let empty = materialize_exact_sequence(&mut store, &[]).unwrap();
        assert_eq!(empty, ROOT_HANDLE);

        let oc = materialize_exact_sequence(&mut store, &[o, c]).unwrap();
        let co = materialize_exact_sequence(&mut store, &[c, o]).unwrap();
        assert_ne!(oc, co);
        assert_eq!(read_exact_sequence(&store, oc).unwrap(), vec![o, c]);
        assert_eq!(read_exact_sequence(&store, co).unwrap(), vec![c, o]);

        let raw_pair = store.ensure_pair(o, c).unwrap();
        assert_ne!(oc, raw_pair);
    }

    #[test]
    fn structural_unifier_preserves_self_incidence_and_repeated_roles() {
        let mut store = OptimizedLinkStore::new();
        let anchors = fresh(&mut store, 12);
        let role = anchors[0];
        let value = anchors[1];

        let repeated_template = store.ensure_pair(role, role).unwrap();
        let repeated_claim = store.ensure_pair(value, value).unwrap();
        let bindings =
            unify_structural_rule_template(&store, repeated_template, repeated_claim, &[role])
                .unwrap();
        assert_eq!(
            bindings,
            vec![StructuralRoleBinding { role, value }]
        );

        let other = anchors[2];
        let inconsistent = store.ensure_pair(value, other).unwrap();
        assert_eq!(
            unify_structural_rule_template(
                &store,
                repeated_template,
                inconsistent,
                &[role],
            ),
            Err(StructuralError::TemplateMismatch)
        );

        let start_template = store.ensure_start_self_closed(role).unwrap();
        let ordinary_claim = store.ensure_pair(value, other).unwrap();
        assert_eq!(
            unify_structural_rule_template(
                &store,
                start_template,
                ordinary_claim,
                &[role],
            ),
            Err(StructuralError::TemplateMismatch)
        );
    }

    #[test]
    fn generic_rule_instantiates_bound_output() {
        let mut store = OptimizedLinkStore::new();
        let anchors = fresh(&mut store, 20);
        let theory = anchors[0];
        let grammar = anchors[1];
        let caller = anchors[3];
        let input = anchors[4];
        let output = anchors[5];

        // fresh[] is a recursively nested chain. Use a later Link as the
        // placeholder role so the grounded input does not structurally contain
        // that same role occurrence. Otherwise the matcher correctly treats
        // the nested occurrence as a second placeholder occurrence.
        let role = anchors[10];

        let authority_dictionary =
            define_structural_role_dictionary(&mut store, &[]).unwrap();
        let interpreter =
            define_structural_interpreter(
                &mut store,
                authority_dictionary,
                grammar,
                theory,
            )
            .unwrap();

        let role_dictionary =
            define_structural_role_dictionary(&mut store, &[role]).unwrap();

        let before = store.ensure_pair(role, input).unwrap();
        let after = store.ensure_pair(role, output).unwrap();
        let bundle = materialize_exact_sequence(&mut store, &[after]).unwrap();
        let body = store.ensure_pair(before, bundle).unwrap();
        let rule =
            define_structural_rule(&mut store, role_dictionary, body).unwrap();
        let admission = admit_structural_rule(&mut store, theory, rule).unwrap();

        let endpoint = input;
        let (trigger_key, _) = store.poles(endpoint).unwrap();
        index_structural_rule_trigger(&mut store, trigger_key, admission).unwrap();

        let active = store.ensure_pair(caller, input).unwrap();

        // Audit every accepted-v0.13 discovery boundary independently before
        // the reaction engine is involved.
        assert_eq!(
            read_structural_role_dictionary(&store, role_dictionary).unwrap(),
            vec![role],
            "role dictionary"
        );
        assert_eq!(
            unify_structural_rule_template(&store, before, active, &[role]).unwrap(),
            vec![StructuralRoleBinding { role, value: caller }],
            "structural unification"
        );

        let (_, endpoint) = store.poles(active).unwrap();
        let (derived_trigger_key, _) = store.poles(endpoint).unwrap();
        assert_eq!(derived_trigger_key, trigger_key, "trigger projection");

        let trigger_members = store
            .start_incidence(trigger_key)
            .unwrap()
            .collect::<Vec<_>>();
        assert!(
            trigger_members.contains(&store.ensure_pair(trigger_key, admission).unwrap()),
            "trigger index contains admission projection"
        );

        let images =
            discover_triggered_rule_images(&store, theory, active).unwrap();
        assert_eq!(images.len(), 1, "one discoverable structural image");

        let mut engine = OptimizedStructuralEngine::new(8);
        engine.set_interpreter(&store, interpreter).unwrap();
        engine.set_current(&store, &[active]).unwrap();

        let reaction = engine.run(&mut store).unwrap();
        let expected = store.ensure_pair(caller, output).unwrap();

        assert_eq!(reaction.raw_rule_matches, 1);
        assert_eq!(reaction.transitioned_members, 1);
        assert_eq!(reaction.handoff_count, 1);
        assert!(!reaction.quiescent);
        assert_eq!(engine.current(), &[expected]);

        let stable = engine.current_bank();
        let quiescent = engine.run(&mut store).unwrap();
        assert!(quiescent.quiescent);
        assert_eq!(quiescent.handoff_count, 0);
        assert_eq!(engine.current_bank(), stable);
        assert_eq!(engine.current(), &[expected]);
    }
}
