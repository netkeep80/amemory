use crate::{
    Handle, OptimizedLinkStore, PackedExecutionRef, PackedExecutionView,
    StoreError, ROOT_HANDLE,
};
use std::collections::{HashMap, HashSet};
#[cfg(not(target_family = "wasm"))]
use std::time::Instant;

#[derive(Clone, Debug)]
struct ProfileTimer {
    #[cfg(not(target_family = "wasm"))]
    started: Instant,
}

impl ProfileTimer {
    fn start() -> Self {
        Self {
            #[cfg(not(target_family = "wasm"))]
            started: Instant::now(),
        }
    }

    fn elapsed_ns(&self) -> u128 {
        #[cfg(not(target_family = "wasm"))]
        {
            self.started.elapsed().as_nanos()
        }
        #[cfg(target_family = "wasm")]
        {
            0
        }
    }
}

pub const STRUCTURAL_PROFILE_TIMING_AVAILABLE: bool =
    !cfg!(target_family = "wasm");

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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StructuralRunProfile {
    pub trigger_incidence_candidates: u64,
    pub candidates_rejected_before_unification: u64,
    pub role_dictionary_decodes: u64,
    pub decoded_roles: u64,
    pub unification_attempts: u64,
    pub unification_successes: u64,
    pub contains_role_nodes_visited: u64,
    pub unification_nodes_visited: u64,
    pub instantiation_nodes_visited: u64,
    pub instantiation_constructor_attempts: u64,
    pub instantiation_canonical_hits: u64,
    pub instantiation_new_links: u64,
    pub publication_outputs: u64,
    pub rule_metadata_cache_hits: u64,
    pub rule_metadata_cache_misses: u64,
    pub grounded_path_checks: u64,
    pub grounded_path_rejects: u64,
    pub discovery_ns: u128,
    pub role_decode_ns: u128,
    pub unification_ns: u128,
    pub instantiation_ns: u128,
    pub publication_ns: u128,
    pub total_ns: u128,
}

impl StructuralRunProfile {
    pub fn accumulate(&mut self, other: &Self) {
        self.trigger_incidence_candidates += other.trigger_incidence_candidates;
        self.candidates_rejected_before_unification += other.candidates_rejected_before_unification;
        self.role_dictionary_decodes += other.role_dictionary_decodes;
        self.decoded_roles += other.decoded_roles;
        self.unification_attempts += other.unification_attempts;
        self.unification_successes += other.unification_successes;
        self.contains_role_nodes_visited += other.contains_role_nodes_visited;
        self.unification_nodes_visited += other.unification_nodes_visited;
        self.instantiation_nodes_visited += other.instantiation_nodes_visited;
        self.instantiation_constructor_attempts += other.instantiation_constructor_attempts;
        self.instantiation_canonical_hits += other.instantiation_canonical_hits;
        self.instantiation_new_links += other.instantiation_new_links;
        self.publication_outputs += other.publication_outputs;
        self.rule_metadata_cache_hits += other.rule_metadata_cache_hits;
        self.rule_metadata_cache_misses += other.rule_metadata_cache_misses;
        self.grounded_path_checks += other.grounded_path_checks;
        self.grounded_path_rejects += other.grounded_path_rejects;
        self.discovery_ns += other.discovery_ns;
        self.role_decode_ns += other.role_decode_ns;
        self.unification_ns += other.unification_ns;
        self.instantiation_ns += other.instantiation_ns;
        self.publication_ns += other.publication_ns;
        self.total_ns += other.total_ns;
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StructuralTraceEvent {
    DiscoveryComplete {
        active: Handle,
        matched_rules: u32,
    },
    RuleMatched {
        active: Handle,
        rule: Handle,
        output_bundle_template: Handle,
        bindings: Vec<StructuralRoleBinding>,
    },
    Instantiated {
        active: Handle,
        rule: Handle,
        output_bundle_template: Handle,
        grounded_bundle: Handle,
    },
    Published {
        active: Handle,
        rule: Option<Handle>,
        outputs: Vec<Handle>,
        preserved: bool,
    },
    ScopeCommitted {
        old_members: Vec<Handle>,
        next_members: Vec<Handle>,
        quiescent: bool,
        handoff_count: u32,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StructuralRunTrace {
    pub events: Vec<StructuralTraceEvent>,
    pub collection_ns: u128,
}

#[derive(Clone, Debug)]
struct GroundedPathCheck {
    // false = START, true = END
    path: Vec<bool>,
    expected: Handle,
}

#[derive(Clone, Debug)]
struct CompiledRuleMetadata {
    role_dictionary: Handle,
    body: Handle,
    before: Handle,
    output_bundle_template: Handle,
    roles: Vec<Handle>,
    grounded_checks: Vec<GroundedPathCheck>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct StructuralImage {
    rule: Handle,
    output_bundle_template: Handle,
    bindings: Vec<StructuralRoleBinding>,
}

trait StructuralRead {
    fn is_valid(&self, handle: Handle) -> bool;
    fn poles(&self, handle: Handle) -> Result<(Handle, Handle), StoreError>;
    fn start_incidence_handles(
        &self,
        start: Handle,
    ) -> Result<Vec<Handle>, StoreError>;
}

impl StructuralRead for OptimizedLinkStore {
    fn is_valid(&self, handle: Handle) -> bool {
        OptimizedLinkStore::is_valid(self, handle)
    }

    fn poles(&self, handle: Handle) -> Result<(Handle, Handle), StoreError> {
        OptimizedLinkStore::poles(self, handle)
    }

    fn start_incidence_handles(
        &self,
        start: Handle,
    ) -> Result<Vec<Handle>, StoreError> {
        Ok(OptimizedLinkStore::start_incidence(self, start)?.collect())
    }
}

impl StructuralRead for PackedExecutionView {
    fn is_valid(&self, handle: Handle) -> bool {
        PackedExecutionView::is_valid(self, handle)
    }

    fn poles(&self, handle: Handle) -> Result<(Handle, Handle), StoreError> {
        PackedExecutionView::poles(self, handle)
    }

    fn start_incidence_handles(
        &self,
        start: Handle,
    ) -> Result<Vec<Handle>, StoreError> {
        Ok(PackedExecutionView::start_incidence(self, start)?.collect())
    }
}

impl StructuralRead for PackedExecutionRef<'_> {
    fn is_valid(&self, handle: Handle) -> bool {
        PackedExecutionRef::is_valid(self, handle)
    }

    fn poles(&self, handle: Handle) -> Result<(Handle, Handle), StoreError> {
        PackedExecutionRef::poles(self, handle)
    }

    fn start_incidence_handles(
        &self,
        start: Handle,
    ) -> Result<Vec<Handle>, StoreError> {
        Ok(PackedExecutionRef::start_incidence(self, start)?.collect())
    }
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

fn read_exact_sequence_from<R: StructuralRead + ?Sized>(
    store: &R,
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

pub fn read_exact_sequence(
    store: &OptimizedLinkStore,
    final_link: Handle,
) -> Result<Vec<Handle>, StructuralError> {
    read_exact_sequence_from(store, final_link)
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

fn read_structural_role_dictionary_from<R: StructuralRead + ?Sized>(
    store: &R,
    dictionary: Handle,
) -> Result<Vec<Handle>, StructuralError> {
    let (start, end) = store.poles(dictionary)?;
    if start != dictionary || end == dictionary {
        return Err(StructuralError::InvalidRoleDictionary(dictionary));
    }

    let roles = read_exact_sequence_from(store, end)?;
    let mut unique = HashSet::new();
    for role in &roles {
        if !unique.insert(*role) {
            return Err(StructuralError::DuplicateRole);
        }
    }
    Ok(roles)
}

pub fn read_structural_role_dictionary(
    store: &OptimizedLinkStore,
    dictionary: Handle,
) -> Result<Vec<Handle>, StructuralError> {
    read_structural_role_dictionary_from(store, dictionary)
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

fn read_structural_interpreter_from<R: StructuralRead + ?Sized>(
    store: &R,
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

pub fn read_structural_interpreter(
    store: &OptimizedLinkStore,
    interpreter: Handle,
) -> Result<StructuralInterpreter, StructuralError> {
    read_structural_interpreter_from(store, interpreter)
}

const MAX_COMPILED_GROUNDED_CHECKS: usize = 32;

fn compile_grounded_path_checks<R: StructuralRead + ?Sized>(
    store: &R,
    template: Handle,
    roles: &[Handle],
) -> Result<Vec<GroundedPathCheck>, StructuralError> {
    let role_set = roles.iter().copied().collect::<HashSet<_>>();

    // Compute "subtree contains any role" bottom-up. OptimizedLinkStore Links
    // only point to existing handles, except their own START/END self-incidence,
    // so ignoring direct self-edges gives an acyclic dependency walk.
    let mut contains_role = HashMap::<Handle, bool>::new();
    let mut pending = vec![(template, false)];

    while let Some((node, expanded)) = pending.pop() {
        if contains_role.contains_key(&node) {
            continue;
        }
        if role_set.contains(&node) {
            contains_role.insert(node, true);
            continue;
        }

        let (start, end) = store.poles(node)?;
        if expanded {
            let start_has = if start == node {
                false
            } else {
                *contains_role.get(&start).unwrap_or(&false)
            };
            let end_has = if end == node {
                false
            } else {
                *contains_role.get(&end).unwrap_or(&false)
            };
            contains_role.insert(node, start_has || end_has);
            continue;
        }

        pending.push((node, true));
        if start != node && !contains_role.contains_key(&start) {
            pending.push((start, false));
        }
        if end != node && !contains_role.contains_key(&end) {
            pending.push((end, false));
        }
    }

    // Record maximal grounded subtrees: once a subtree contains no role, exact
    // canonical handle equality is a necessary condition for a true match.
    // END-first mirrors the proven useful traversal order from P1.
    let mut checks = Vec::new();
    let mut walk = vec![(template, Vec::<bool>::new())];

    while let Some((node, path)) = walk.pop() {
        if role_set.contains(&node) {
            continue;
        }
        if !contains_role.get(&node).copied().unwrap_or(false) {
            checks.push(GroundedPathCheck {
                path,
                expected: node,
            });
            if checks.len() >= MAX_COMPILED_GROUNDED_CHECKS {
                break;
            }
            continue;
        }

        let (start, end) = store.poles(node)?;
        if start != node {
            let mut start_path = path.clone();
            start_path.push(false);
            walk.push((start, start_path));
        }
        if end != node {
            let mut end_path = path;
            end_path.push(true);
            walk.push((end, end_path));
        }
    }

    checks.sort_by_key(|check| check.path.len());
    Ok(checks)
}

fn compiled_grounded_paths_match<R: StructuralRead + ?Sized>(
    store: &R,
    claimed: Handle,
    checks: &[GroundedPathCheck],
    profile: &mut Option<&mut StructuralRunProfile>,
) -> Result<bool, StructuralError> {
    for check in checks {
        if let Some(profile) = profile.as_deref_mut() {
            profile.grounded_path_checks += 1;
        }

        let mut current = claimed;
        for go_end in &check.path {
            let (start, end) = store.poles(current)?;
            current = if *go_end { end } else { start };
        }

        if current != check.expected {
            if let Some(profile) = profile.as_deref_mut() {
                profile.grounded_path_rejects += 1;
            }
            return Ok(false);
        }
    }

    Ok(true)
}

const STRUCTURAL_DISCRIMINATION_BUDGET: usize = 128;

fn structural_discriminator_matches<R: StructuralRead + ?Sized>(
    store: &R,
    template: Handle,
    claimed: Handle,
    roles: &[Handle],
) -> Result<bool, StructuralError> {
    // This is a sound prefilter, never a semantic matcher. Role nodes are
    // wildcards. A structural mismatch at any inspected non-role position is
    // sufficient to reject the candidate. Budget exhaustion is deliberately
    // fail-open *to the full unifier* (not to execution): the candidate simply
    // survives for authoritative matching.
    let mut pending = vec![(template, claimed)];
    let mut visited: Vec<(Handle, Handle)> = Vec::new();
    let mut inspected = 0usize;

    while let Some((template, claimed)) = pending.pop() {
        if roles.contains(&template) {
            continue;
        }

        // Exact local identity inside one store proves this subtree is
        // structurally compatible. It is only an optimization shortcut; local
        // identity is never exported as portable semantics.
        if template == claimed {
            continue;
        }

        if visited.contains(&(template, claimed)) {
            continue;
        }
        visited.push((template, claimed));

        if inspected >= STRUCTURAL_DISCRIMINATION_BUDGET {
            return Ok(true);
        }
        inspected += 1;

        let (template_start, template_end) = store.poles(template)?;
        let (claimed_start, claimed_end) = store.poles(claimed)?;

        if (template_start == template) != (claimed_start == claimed)
            || (template_end == template) != (claimed_end == claimed)
        {
            return Ok(false);
        }

        // Prefilter ordering is deliberately non-semantic. Explore END first:
        // in nested Link carriers this often reaches grounded discriminators
        // before large role-heavy prefixes. Any uncertainty still falls
        // through to the authoritative full unifier.
        pending.push((template_start, claimed_start));
        pending.push((template_end, claimed_end));
    }

    Ok(true)
}

fn unify_node<R: StructuralRead + ?Sized>(
    store: &R,
    template: Handle,
    claimed: Handle,
    roles: &HashSet<Handle>,
    inferred: &mut HashMap<Handle, Handle>,
    visited: &mut HashSet<(Handle, Handle)>,
    profile: &mut Option<&mut StructuralRunProfile>,
) -> Result<(), StructuralError> {
    // Use an explicit work stack rather than Rust recursion. Structural Anums
    // can be deeply nested, and native stacks are much larger than the WASM
    // call stack used by Pages. The algorithm is still the same direct
    // structural comparison: role nodes bind, all other nodes compare their
    // incidence shape and enqueue start/end pairs.
    let mut pending = vec![(template, claimed)];

    while let Some((template, claimed)) = pending.pop() {
        if let Some(profile) = profile.as_deref_mut() {
            profile.unification_nodes_visited += 1;
        }

        if roles.contains(&template) {
            if let Some(previous) = inferred.get(&template) {
                if *previous != claimed {
                    return Err(StructuralError::TemplateMismatch);
                }
            } else {
                inferred.insert(template, claimed);
            }
            continue;
        }

        if !visited.insert((template, claimed)) {
            continue;
        }

        let (template_start, template_end) = store.poles(template)?;
        let (claimed_start, claimed_end) = store.poles(claimed)?;

        if (template_start == template) != (claimed_start == claimed)
            || (template_end == template) != (claimed_end == claimed)
        {
            return Err(StructuralError::TemplateMismatch);
        }

        // LIFO: push END first so START retains the previous recursive
        // traversal order. Ordering is not semantic, but keeping it stable
        // makes profiling and debugging easier to compare.
        pending.push((template_end, claimed_end));
        pending.push((template_start, claimed_start));
    }

    Ok(())
}

fn unify_structural_rule_template_internal<R: StructuralRead + ?Sized>(
    store: &R,
    template: Handle,
    claimed: Handle,
    roles: &[Handle],
    profile: &mut Option<&mut StructuralRunProfile>,
) -> Result<Vec<StructuralRoleBinding>, StructuralError> {
    let mut role_set = HashSet::new();
    for role in roles {
        if !role_set.insert(*role) {
            return Err(StructuralError::DuplicateRole);
        }
    }

    let mut inferred = HashMap::new();
    let mut visited = HashSet::new();

    unify_node(
        store,
        template,
        claimed,
        &role_set,
        &mut inferred,
        &mut visited,
        profile,
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

pub fn unify_structural_rule_template(
    store: &OptimizedLinkStore,
    template: Handle,
    claimed: Handle,
    roles: &[Handle],
) -> Result<Vec<StructuralRoleBinding>, StructuralError> {
    let mut profile = None;
    unify_structural_rule_template_internal(store, template, claimed, roles, &mut profile)
}

fn instantiate_node(
    store: &mut OptimizedLinkStore,
    source: Handle,
    bindings: &HashMap<Handle, Handle>,
    visiting: &mut HashSet<Handle>,
    memo: &mut HashMap<Handle, Handle>,
    profile: &mut Option<&mut StructuralRunProfile>,
) -> Result<Handle, StructuralError> {
    #[derive(Clone, Copy)]
    enum Frame {
        Enter(Handle),
        FinishStart {
            source: Handle,
            child: Handle,
        },
        FinishEnd {
            source: Handle,
            child: Handle,
        },
        FinishPair {
            source: Handle,
            start: Handle,
            end: Handle,
        },
    }

    fn resolved(
        source: Handle,
        bindings: &HashMap<Handle, Handle>,
        memo: &HashMap<Handle, Handle>,
    ) -> Result<Handle, StructuralError> {
        bindings
            .get(&source)
            .copied()
            .or_else(|| memo.get(&source).copied())
            .ok_or(StructuralError::TemplateMismatch)
    }

    fn record_constructor(
        profile: &mut Option<&mut StructuralRunProfile>,
        before: usize,
        after: usize,
    ) {
        if let Some(profile) = profile.as_deref_mut() {
            profile.instantiation_constructor_attempts += 1;
            let delta = after - before;
            profile.instantiation_new_links += delta as u64;
            if delta == 0 {
                profile.instantiation_canonical_hits += 1;
            }
        }
    }

    // Structural template depth belongs to A-memory data, not to the host
    // call stack. Use explicit post-order frames so deeply recursive valid
    // structures behave identically on native and constrained WASM stacks.
    let mut pending = vec![Frame::Enter(source)];

    while let Some(frame) = pending.pop() {
        match frame {
            Frame::Enter(source) => {
                if let Some(profile) = profile.as_deref_mut() {
                    profile.instantiation_nodes_visited += 1;
                }
                if bindings.contains_key(&source) || memo.contains_key(&source) {
                    continue;
                }

                let (start, end) = store.poles(source)?;
                if start == source && end == source {
                    memo.insert(source, ROOT_HANDLE);
                    continue;
                }

                if !visiting.insert(source) {
                    return Err(StructuralError::UnsupportedCycle(source));
                }

                if start == source {
                    pending.push(Frame::FinishStart {
                        source,
                        child: end,
                    });
                    pending.push(Frame::Enter(end));
                } else if end == source {
                    pending.push(Frame::FinishEnd {
                        source,
                        child: start,
                    });
                    pending.push(Frame::Enter(start));
                } else {
                    pending.push(Frame::FinishPair { source, start, end });
                    // Preserve historical recursive traversal order: START,
                    // then END, then construct the parent.
                    pending.push(Frame::Enter(end));
                    pending.push(Frame::Enter(start));
                }
            }
            Frame::FinishStart { source, child } => {
                let child = resolved(child, bindings, memo)?;
                let before = store.link_count();
                let value = store.ensure_start_self_closed(child)?;
                record_constructor(profile, before, store.link_count());
                visiting.remove(&source);
                memo.insert(source, value);
            }
            Frame::FinishEnd { source, child } => {
                let child = resolved(child, bindings, memo)?;
                let before = store.link_count();
                let value = store.ensure_end_self_closed(child)?;
                record_constructor(profile, before, store.link_count());
                visiting.remove(&source);
                memo.insert(source, value);
            }
            Frame::FinishPair { source, start, end } => {
                let new_start = resolved(start, bindings, memo)?;
                let new_end = resolved(end, bindings, memo)?;
                let before = store.link_count();
                let value = store.ensure_pair(new_start, new_end)?;
                record_constructor(profile, before, store.link_count());
                visiting.remove(&source);
                memo.insert(source, value);
            }
        }
    }

    resolved(source, bindings, memo)
}

fn instantiate_structural_template_internal(
    store: &mut OptimizedLinkStore,
    template: Handle,
    bindings: &[StructuralRoleBinding],
    profile: &mut Option<&mut StructuralRunProfile>,
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
        profile,
    )
}

pub fn instantiate_structural_template(
    store: &mut OptimizedLinkStore,
    template: Handle,
    bindings: &[StructuralRoleBinding],
) -> Result<Handle, StructuralError> {
    let mut profile = None;
    instantiate_structural_template_internal(store, template, bindings, &mut profile)
}

fn discover_triggered_rule_images_internal<R: StructuralRead + ?Sized>(
    store: &R,
    theory: Handle,
    active: Handle,
    metadata_cache: &mut HashMap<Handle, CompiledRuleMetadata>,
    profile: &mut Option<&mut StructuralRunProfile>,
) -> Result<Vec<StructuralImage>, StructuralError> {
    let discovery_started = profile.as_ref().map(|_| ProfileTimer::start());
    let (_, endpoint) = store.poles(active)?;
    let (trigger_key, _) = store.poles(endpoint)?;

    let triggers = store.start_incidence_handles(trigger_key)?;

    if let Some(profile) = profile.as_deref_mut() {
        profile.trigger_incidence_candidates += triggers.len() as u64;
    }

    let mut images = Vec::new();

    for trigger in triggers {
        if trigger == trigger_key {
            if let Some(profile) = profile.as_deref_mut() {
                profile.candidates_rejected_before_unification += 1;
            }
            continue;
        }

        let Ok((trigger_start, admission)) = store.poles(trigger) else {
            if let Some(profile) = profile.as_deref_mut() {
                profile.candidates_rejected_before_unification += 1;
            }
            continue;
        };
        if trigger_start != trigger_key {
            if let Some(profile) = profile.as_deref_mut() {
                profile.candidates_rejected_before_unification += 1;
            }
            continue;
        }

        let Ok((admission_theory, rule)) = store.poles(admission) else {
            if let Some(profile) = profile.as_deref_mut() {
                profile.candidates_rejected_before_unification += 1;
            }
            continue;
        };
        if admission_theory != theory || rule == admission {
            if let Some(profile) = profile.as_deref_mut() {
                profile.candidates_rejected_before_unification += 1;
            }
            continue;
        }

        let metadata_missing = !metadata_cache.contains_key(&rule);
        if metadata_missing {
            if let Some(profile) = profile.as_deref_mut() {
                profile.rule_metadata_cache_misses += 1;
            }

            let Ok((role_dictionary, body)) = store.poles(rule) else {
                if let Some(profile) = profile.as_deref_mut() {
                    profile.candidates_rejected_before_unification += 1;
                }
                continue;
            };

            let role_started = profile.as_ref().map(|_| ProfileTimer::start());
            let roles_result =
                read_structural_role_dictionary_from(store, role_dictionary);
            if let Some(profile) = profile.as_deref_mut() {
                profile.role_dictionary_decodes += 1;
                if let Some(started) = role_started {
                    profile.role_decode_ns += started.elapsed_ns();
                }
            }
            let Ok(roles) = roles_result else {
                if let Some(profile) = profile.as_deref_mut() {
                    profile.candidates_rejected_before_unification += 1;
                }
                continue;
            };
            if let Some(profile) = profile.as_deref_mut() {
                profile.decoded_roles += roles.len() as u64;
            }

            let Ok((before, output_bundle_template)) = store.poles(body) else {
                if let Some(profile) = profile.as_deref_mut() {
                    profile.candidates_rejected_before_unification += 1;
                }
                continue;
            };

            let grounded_checks =
                compile_grounded_path_checks(store, before, &roles)?;

            metadata_cache.insert(
                rule,
                CompiledRuleMetadata {
                    role_dictionary,
                    body,
                    before,
                    output_bundle_template,
                    roles,
                    grounded_checks,
                },
            );
        } else if let Some(profile) = profile.as_deref_mut() {
            profile.rule_metadata_cache_hits += 1;
        }

        let Some(metadata) = metadata_cache.get(&rule) else {
            continue;
        };

        // Defensive local-store validation. Cache identity is already scoped to
        // one OptimizedLinkStore instance; these cheap checks additionally make
        // malformed or stale metadata fail open to rejection/recompile logic.
        if store.poles(rule).ok() != Some((metadata.role_dictionary, metadata.body))
            || store.poles(metadata.body).ok()
                != Some((metadata.before, metadata.output_bundle_template))
        {
            // Cache metadata is never semantic authority. If its local-store
            // invariants are broken, fail closed instead of silently turning a
            // potentially valid Rule into quiescence.
            return Err(StructuralError::InvalidRule(rule));
        }

        if !compiled_grounded_paths_match(
            store,
            active,
            &metadata.grounded_checks,
            profile,
        )? {
            if let Some(profile) = profile.as_deref_mut() {
                profile.candidates_rejected_before_unification += 1;
            }
            continue;
        }

        if !structural_discriminator_matches(
            store,
            metadata.before,
            active,
            &metadata.roles,
        )? {
            if let Some(profile) = profile.as_deref_mut() {
                profile.candidates_rejected_before_unification += 1;
            }
            continue;
        }

        if let Some(profile) = profile.as_deref_mut() {
            profile.unification_attempts += 1;
        }
        let unify_started = profile.as_ref().map(|_| ProfileTimer::start());
        let bindings_result = unify_structural_rule_template_internal(
            store,
            metadata.before,
            active,
            &metadata.roles,
            profile,
        );
        if let Some(profile) = profile.as_deref_mut() {
            if let Some(started) = unify_started {
                profile.unification_ns += started.elapsed_ns();
            }
        }
        let Ok(bindings) = bindings_result else {
            continue;
        };
        if let Some(profile) = profile.as_deref_mut() {
            profile.unification_successes += 1;
        }

        images.push(StructuralImage {
            rule,
            output_bundle_template: metadata.output_bundle_template,
            bindings,
        });
    }

    if let Some(profile) = profile.as_deref_mut() {
        if let Some(started) = discovery_started {
            profile.discovery_ns += started.elapsed_ns();
        }
    }
    Ok(images)
}

fn discover_triggered_rule_images(
    store: &OptimizedLinkStore,
    theory: Handle,
    active: Handle,
) -> Result<Vec<StructuralImage>, StructuralError> {
    let mut profile = None;
    let mut metadata_cache = HashMap::new();
    discover_triggered_rule_images_internal(
        store,
        theory,
        active,
        &mut metadata_cache,
        &mut profile,
    )
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
    metadata_store_instance: Option<u32>,
    rule_metadata_cache: HashMap<Handle, CompiledRuleMetadata>,
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
            metadata_store_instance: None,
            rule_metadata_cache: HashMap::new(),
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
        let mut profile = None;
        let mut trace = None;
        self.run_internal(store, &mut profile, &mut trace)
    }

    pub fn run_profiled(
        &mut self,
        store: &mut OptimizedLinkStore,
    ) -> Result<(StructuralReactionResult, StructuralRunProfile), StructuralError> {
        let started = ProfileTimer::start();
        let mut owned = StructuralRunProfile::default();
        let result = {
            let mut profile = Some(&mut owned);
            let mut trace = None;
            self.run_internal(store, &mut profile, &mut trace)?
        };
        owned.total_ns = started.elapsed_ns();
        Ok((result, owned))
    }

    pub fn run_traced(
        &mut self,
        store: &mut OptimizedLinkStore,
    ) -> Result<
        (
            StructuralReactionResult,
            StructuralRunProfile,
            StructuralRunTrace,
        ),
        StructuralError,
    > {
        let started = ProfileTimer::start();
        let mut owned_profile = StructuralRunProfile::default();
        let mut owned_trace = StructuralRunTrace::default();
        let result = {
            let mut profile = Some(&mut owned_profile);
            let mut trace = Some(&mut owned_trace);
            self.run_internal(store, &mut profile, &mut trace)?
        };
        owned_profile.total_ns = started
            .elapsed_ns()
            .saturating_sub(owned_trace.collection_ns);
        Ok((result, owned_profile, owned_trace))
    }

    fn run_internal(
        &mut self,
        store: &mut OptimizedLinkStore,
        profile: &mut Option<&mut StructuralRunProfile>,
        trace: &mut Option<&mut StructuralRunTrace>,
    ) -> Result<StructuralReactionResult, StructuralError> {
        self.quiescent = false;

        let store_instance = store.instance_id();
        if self.metadata_store_instance != Some(store_instance) {
            self.rule_metadata_cache.clear();
            self.metadata_store_instance = Some(store_instance);
        }

        let interpreter = self.interpreter.ok_or(StructuralError::MissingInterpreter)?;
        let old_members = self.scope_banks[self.current_bank].clone();

        if old_members.len() > self.cap {
            return Err(StructuralError::ScopeCapacity {
                requested: old_members.len(),
                cap: self.cap,
            });
        }

        // Phase 1 is read-only. Borrow only dense carrier/index slices, not
        // canonical HashMaps, and discover every old Scope member before any
        // publication mutates the store. This makes reaction observation
        // atomic without cloning the whole carrier on every run().
        let discovered = {
            let execution_view = store.packed_execution_ref();
            let authority =
                read_structural_interpreter_from(&execution_view, interpreter)?;
            let mut discovered = Vec::with_capacity(old_members.len());
            for active in old_members.iter().copied() {
                let images = discover_triggered_rule_images_internal(
                    &execution_view,
                    authority.theory,
                    active,
                    &mut self.rule_metadata_cache,
                    profile,
                )?;
                if let Some(trace) = trace.as_deref_mut() {
                    let trace_started = ProfileTimer::start();
                    trace.events.push(StructuralTraceEvent::DiscoveryComplete {
                        active,
                        matched_rules: images.len() as u32,
                    });
                    for image in &images {
                        trace.events.push(StructuralTraceEvent::RuleMatched {
                            active,
                            rule: image.rule,
                            output_bundle_template:
                                image.output_bundle_template,
                            bindings: image.bindings.clone(),
                        });
                    }
                    trace.collection_ns += trace_started.elapsed_ns();
                }
                discovered.push((active, images));
            }
            discovered
        };

        // Phase 2 is the explicit mutation/publication boundary. The borrowed
        // execution view is gone, so constructor/canonicalization writes are
        // impossible during discovery and legal only from this point onward.
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

        for (active, images) in discovered {
            if images.is_empty() {
                let publication_started = profile.as_ref().map(|_| ProfileTimer::start());
                add_next(active)?;
                if let Some(profile) = profile.as_deref_mut() {
                    profile.publication_outputs += 1;
                    if let Some(started) = publication_started {
                        profile.publication_ns += started.elapsed_ns();
                    }
                }
                if let Some(trace) = trace.as_deref_mut() {
                    let trace_started = ProfileTimer::start();
                    trace.events.push(StructuralTraceEvent::Published {
                        active,
                        rule: None,
                        outputs: vec![active],
                        preserved: true,
                    });
                    trace.collection_ns += trace_started.elapsed_ns();
                }
                continue;
            }

            transitioned_members = transitioned_members.saturating_add(1);

            for image in images {
                raw_rule_matches = raw_rule_matches.saturating_add(1);

                let instantiation_started = profile.as_ref().map(|_| ProfileTimer::start());
                let grounded_bundle = instantiate_structural_template_internal(
                    store,
                    image.output_bundle_template,
                    &image.bindings,
                    profile,
                )?;
                if let Some(profile) = profile.as_deref_mut() {
                    if let Some(started) = instantiation_started {
                        profile.instantiation_ns += started.elapsed_ns();
                    }
                }
                if let Some(trace) = trace.as_deref_mut() {
                    let trace_started = ProfileTimer::start();
                    trace.events.push(StructuralTraceEvent::Instantiated {
                        active,
                        rule: image.rule,
                        output_bundle_template:
                            image.output_bundle_template,
                        grounded_bundle,
                    });
                    trace.collection_ns += trace_started.elapsed_ns();
                }

                let publication_started = profile.as_ref().map(|_| ProfileTimer::start());
                let outputs = read_exact_sequence(store, grounded_bundle)?;
                if let Some(profile) = profile.as_deref_mut() {
                    profile.publication_outputs += outputs.len() as u64;
                }
                for successor in outputs.iter().copied() {
                    add_next(successor)?;
                }
                if let Some(profile) = profile.as_deref_mut() {
                    if let Some(started) = publication_started {
                        profile.publication_ns += started.elapsed_ns();
                    }
                }
                if let Some(trace) = trace.as_deref_mut() {
                    let trace_started = ProfileTimer::start();
                    trace.events.push(StructuralTraceEvent::Published {
                        active,
                        rule: Some(image.rule),
                        outputs,
                        preserved: false,
                    });
                    trace.collection_ns += trace_started.elapsed_ns();
                }
            }
        }

        self.raw_rule_matches = raw_rule_matches;
        self.transitioned_members = transitioned_members;
        self.handoff_count = 0;

        if raw_rule_matches == 0 {
            self.quiescent = true;
            if let Some(trace) = trace.as_deref_mut() {
                let trace_started = ProfileTimer::start();
                trace.events.push(StructuralTraceEvent::ScopeCommitted {
                    old_members: old_members.clone(),
                    next_members: old_members.clone(),
                    quiescent: true,
                    handoff_count: 0,
                });
                trace.collection_ns += trace_started.elapsed_ns();
            }
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

        if let Some(trace) = trace.as_deref_mut() {
            let trace_started = ProfileTimer::start();
            trace.events.push(StructuralTraceEvent::ScopeCommitted {
                old_members: old_members.clone(),
                next_members: next_members.clone(),
                quiescent: false,
                handoff_count: 1,
            });
            trace.collection_ns += trace_started.elapsed_ns();
        }

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
    fn structural_discriminator_only_rejects_proven_mismatch() {
        let mut store = OptimizedLinkStore::new();
        let anchors = fresh(&mut store, 40);
        let role = anchors[30];
        let left = anchors[4];
        let right = anchors[5];
        let other = anchors[6];

        let template = store.ensure_pair(role, right).unwrap();
        let arbitrary_start = store.ensure_pair(left, other).unwrap();
        let arbitrary = store.ensure_pair(arbitrary_start, right).unwrap();

        assert!(
            structural_discriminator_matches(&store, template, arbitrary, &[role]).unwrap(),
            "role wildcard must never cause an early false negative"
        );

        let grounded = store.ensure_pair(left, right).unwrap();
        let wrong = store.ensure_pair(other, right).unwrap();
        assert!(
            !structural_discriminator_matches(&store, grounded, wrong, &[]).unwrap(),
            "grounded structural mismatch should be rejected before unification"
        );

        assert!(
            structural_discriminator_matches(&store, grounded, grounded, &[]).unwrap(),
            "identical grounded structure must survive discrimination"
        );

        // Put the first real mismatch deeper than the bounded prefilter can
        // inspect. The discriminator must return "possible" and delegate the
        // final decision to the authoritative full matcher.
        let mut deep_template = left;
        let mut deep_claimed = other;
        for _ in 0..(STRUCTURAL_DISCRIMINATION_BUDGET + 8) {
            deep_template = store.ensure_pair(deep_template, right).unwrap();
            deep_claimed = store.ensure_pair(deep_claimed, right).unwrap();
        }
        assert!(
            structural_discriminator_matches(
                &store,
                deep_template,
                deep_claimed,
                &[],
            )
            .unwrap(),
            "budget exhaustion must fail open to full unification"
        );
        assert_eq!(
            unify_structural_rule_template(
                &store,
                deep_template,
                deep_claimed,
                &[],
            ),
            Err(StructuralError::TemplateMismatch),
            "full matcher remains final authority after discriminator uncertainty"
        );
    }

    #[test]
    fn compiled_grounded_paths_are_sound_prefilter_for_role_templates() {
        let mut store = OptimizedLinkStore::new();
        let anchors = fresh(&mut store, 48);
        let role = anchors[40];
        let grounded_left = anchors[6];
        let grounded_right = anchors[7];
        let bound_value = anchors[8];
        let other = anchors[9];

        let grounded_suffix =
            store.ensure_pair(grounded_left, grounded_right).unwrap();
        let template = store.ensure_pair(role, grounded_suffix).unwrap();
        let matching = store.ensure_pair(bound_value, grounded_suffix).unwrap();
        let wrong_suffix = store.ensure_pair(grounded_left, other).unwrap();
        let rejected = store.ensure_pair(bound_value, wrong_suffix).unwrap();

        let checks =
            compile_grounded_path_checks(&store, template, &[role]).unwrap();
        assert!(!checks.is_empty());

        let mut matching_profile = None;
        assert!(
            compiled_grounded_paths_match(
                &store,
                matching,
                &checks,
                &mut matching_profile,
            )
            .unwrap(),
            "true structural match must survive compiled prefilter"
        );
        assert!(
            unify_structural_rule_template(&store, template, matching, &[role])
                .is_ok(),
        );

        let mut rejected_profile = None;
        assert!(
            !compiled_grounded_paths_match(
                &store,
                rejected,
                &checks,
                &mut rejected_profile,
            )
            .unwrap(),
            "grounded mismatch should be rejected before full unification"
        );
        assert_eq!(
            unify_structural_rule_template(&store, template, rejected, &[role]),
            Err(StructuralError::TemplateMismatch),
        );
    }

    #[test]
    fn executor_metadata_cache_is_scoped_to_one_runtime_store() {
        let mut original = OptimizedLinkStore::new();
        let original_id = original.instance_id();

        let source = original.export_anum(ROOT_HANDLE).unwrap();
        original.import_anum(&source).unwrap();
        assert_eq!(
            original.instance_id(),
            original_id,
            "transactional import must preserve runtime store identity"
        );

        let cloned = original.clone();
        assert_ne!(
            cloned.instance_id(),
            original.instance_id(),
            "independent store clones must never share executor-cache identity"
        );

        let mut engine = OptimizedStructuralEngine::new(4);
        engine.metadata_store_instance = Some(original.instance_id());
        engine.rule_metadata_cache.insert(
            ROOT_HANDLE,
            CompiledRuleMetadata {
                role_dictionary: ROOT_HANDLE,
                body: ROOT_HANDLE,
                before: ROOT_HANDLE,
                output_bundle_template: ROOT_HANDLE,
                roles: Vec::new(),
                grounded_checks: Vec::new(),
            },
        );

        let interpreter = {
            let dictionary =
                define_structural_role_dictionary(&mut original, &[]).unwrap();
            let grammar = original.ensure_start_self_closed(ROOT_HANDLE).unwrap();
            let theory = original.ensure_end_self_closed(ROOT_HANDLE).unwrap();
            define_structural_interpreter(
                &mut original,
                dictionary,
                grammar,
                theory,
            )
            .unwrap()
        };
        engine.set_interpreter(&original, interpreter).unwrap();
        engine.set_current(&original, &[ROOT_HANDLE]).unwrap();
        let _ = engine.run(&mut original).unwrap();

        // Merely seeing another store instance is sufficient to invalidate all
        // local-handle metadata before any semantic discovery can use it.
        let clone_id = cloned.instance_id();
        assert_ne!(clone_id, engine.metadata_store_instance.unwrap());
    }

    #[test]
    fn direct_unifier_matches_grounded_and_role_templates_without_prescan() {
        let mut store = OptimizedLinkStore::new();
        let anchors = fresh(&mut store, 24);
        let role = anchors[20];
        let left = anchors[4];
        let right = anchors[5];

        let grounded_template = store.ensure_pair(left, right).unwrap();
        let grounded_same = store.ensure_pair(left, right).unwrap();
        let grounded_wrong = store.ensure_pair(right, left).unwrap();

        let mut profile = StructuralRunProfile::default();
        let same = {
            let mut slot = Some(&mut profile);
            unify_structural_rule_template_internal(
                &store,
                grounded_template,
                grounded_same,
                &[],
                &mut slot,
            )
            .unwrap()
        };
        assert!(same.is_empty());
        assert_eq!(profile.contains_role_nodes_visited, 0);

        let mut mismatch_profile = StructuralRunProfile::default();
        let mismatch = {
            let mut slot = Some(&mut mismatch_profile);
            unify_structural_rule_template_internal(
                &store,
                grounded_template,
                grounded_wrong,
                &[],
                &mut slot,
            )
        };
        assert_eq!(mismatch, Err(StructuralError::TemplateMismatch));
        assert_eq!(mismatch_profile.contains_role_nodes_visited, 0);

        let role_template = store.ensure_pair(role, right).unwrap();
        let claimed = store.ensure_pair(left, right).unwrap();
        let bindings = unify_structural_rule_template(
            &store,
            role_template,
            claimed,
            &[role],
        )
        .unwrap();
        assert_eq!(
            bindings,
            vec![StructuralRoleBinding {
                role,
                value: left,
            }]
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

        let execution_view = store.export_packed_execution_view();
        assert_eq!(
            read_structural_interpreter_from(&execution_view, interpreter)
                .unwrap(),
            StructuralInterpreter {
                dictionary: authority_dictionary,
                grammar,
                theory,
            },
            "packed execution view interpreter"
        );
        assert_eq!(
            read_structural_role_dictionary_from(
                &execution_view,
                role_dictionary,
            )
            .unwrap(),
            vec![role],
            "packed execution view role dictionary"
        );

        let mut view_profile = None;
        let view_bindings = unify_structural_rule_template_internal(
            &execution_view,
            before,
            active,
            &[role],
            &mut view_profile,
        )
        .unwrap();
        assert_eq!(
            view_bindings,
            vec![StructuralRoleBinding { role, value: caller }],
            "packed execution view unification"
        );

        let mut view_metadata_cache = HashMap::new();
        let mut view_discovery_profile = None;
        let view_images = discover_triggered_rule_images_internal(
            &execution_view,
            theory,
            active,
            &mut view_metadata_cache,
            &mut view_discovery_profile,
        )
        .unwrap();
        assert_eq!(
            view_images, images,
            "packed execution view changed structural discovery"
        );

        let mut engine = OptimizedStructuralEngine::new(8);
        engine.set_interpreter(&store, interpreter).unwrap();
        engine.set_current(&store, &[active]).unwrap();

        let mut profiled_store = store.clone();
        let mut profiled_engine = engine.clone();
        let mut traced_store = store.clone();
        let mut traced_engine = engine.clone();

        let reaction = engine.run(&mut store).unwrap();
        let expected = store.ensure_pair(caller, output).unwrap();

        let (profiled_reaction, profile) =
            profiled_engine.run_profiled(&mut profiled_store).unwrap();
        let profiled_expected =
            profiled_store.ensure_pair(caller, output).unwrap();

        let (traced_reaction, traced_profile, trace) =
            traced_engine.run_traced(&mut traced_store).unwrap();
        let traced_expected =
            traced_store.ensure_pair(caller, output).unwrap();

        assert_eq!(
            profiled_reaction, reaction,
            "profiled path changed reaction semantics"
        );
        assert_eq!(
            traced_reaction, reaction,
            "traced path changed reaction semantics"
        );
        assert_eq!(profiled_engine.current(), &[profiled_expected]);
        assert_eq!(traced_engine.current(), &[traced_expected]);
        assert!(profile.trigger_incidence_candidates > 0);
        assert!(profile.unification_attempts > 0);
        assert_eq!(profile.unification_successes, 1);
        assert!(profile.total_ns > 0);
        assert_eq!(
            traced_profile.unification_successes,
            profile.unification_successes,
        );
        assert!(traced_profile.total_ns > 0);
        assert!(trace.collection_ns > 0);

        assert!(trace.events.iter().any(|event| matches!(
            event,
            StructuralTraceEvent::DiscoveryComplete {
                active: traced_active,
                matched_rules: 1,
            } if *traced_active == active
        )));
        assert!(trace.events.iter().any(|event| matches!(
            event,
            StructuralTraceEvent::RuleMatched {
                active: traced_active,
                rule: traced_rule,
                output_bundle_template,
                bindings,
            } if *traced_active == active
                && *traced_rule == rule
                && *output_bundle_template == bundle
                && bindings
                    == &vec![StructuralRoleBinding {
                        role,
                        value: caller,
                    }]
        )));
        let grounded_bundle = trace
            .events
            .iter()
            .find_map(|event| match event {
                StructuralTraceEvent::Instantiated {
                    active: traced_active,
                    rule: traced_rule,
                    output_bundle_template,
                    grounded_bundle,
                } if *traced_active == active
                    && *traced_rule == rule
                    && *output_bundle_template == bundle =>
                {
                    Some(*grounded_bundle)
                }
                _ => None,
            })
            .expect("real grounded bundle trace");
        assert_eq!(
            read_exact_sequence(&traced_store, grounded_bundle).unwrap(),
            vec![traced_expected],
        );
        assert!(trace.events.iter().any(|event| matches!(
            event,
            StructuralTraceEvent::Published {
                active: traced_active,
                rule: Some(traced_rule),
                outputs,
                preserved: false,
            } if *traced_active == active
                && *traced_rule == rule
                && outputs == &vec![traced_expected]
        )));
        assert!(trace.events.iter().any(|event| matches!(
            event,
            StructuralTraceEvent::ScopeCommitted {
                old_members,
                next_members,
                quiescent: false,
                handoff_count: 1,
            } if old_members == &vec![active]
                && next_members == &vec![traced_expected]
        )));

        assert_eq!(reaction.raw_rule_matches, 1);
        assert_eq!(reaction.transitioned_members, 1);
        assert_eq!(reaction.handoff_count, 1);
        assert_eq!(
            engine.current(),
            &[expected],
            "snapshot discovery must preserve publication result"
        );
        assert!(!reaction.quiescent);
        assert_eq!(engine.current(), &[expected]);

        let stable = engine.current_bank();
        let quiescent = engine.run(&mut store).unwrap();
        assert!(quiescent.quiescent);
        assert_eq!(quiescent.handoff_count, 0);
        assert_eq!(engine.current_bank(), stable);
        assert_eq!(engine.current(), &[expected]);

        let traced_stable = traced_engine.current_bank();
        let (traced_quiescent, _quiescent_profile, quiescent_trace) =
            traced_engine.run_traced(&mut traced_store).unwrap();
        assert_eq!(traced_quiescent, quiescent);
        assert_eq!(traced_engine.current_bank(), traced_stable);
        assert_eq!(traced_engine.current(), &[traced_expected]);
        assert!(quiescent_trace.events.iter().any(|event| matches!(
            event,
            StructuralTraceEvent::DiscoveryComplete {
                active: traced_active,
                matched_rules: 0,
            } if *traced_active == traced_expected
        )));
        assert!(quiescent_trace.events.iter().any(|event| matches!(
            event,
            StructuralTraceEvent::Published {
                active: traced_active,
                rule: None,
                outputs,
                preserved: true,
            } if *traced_active == traced_expected
                && outputs == &vec![traced_expected]
        )));
        assert!(quiescent_trace.events.iter().any(|event| matches!(
            event,
            StructuralTraceEvent::ScopeCommitted {
                old_members,
                next_members,
                quiescent: true,
                handoff_count: 0,
            } if old_members == &vec![traced_expected]
                && next_members == &vec![traced_expected]
        )));
    }

    #[test]
    fn grounded_constant_mismatch_is_quiescent_not_a_false_match() {
        let mut store = OptimizedLinkStore::new();
        let anchors = fresh(&mut store, 30);
        let theory = anchors[0];
        let grammar = anchors[1];
        let caller = anchors[2];
        let common = anchors[3];
        let left = anchors[4];
        let right = anchors[5];
        let output = anchors[6];
        let role = anchors[20];

        let input = store.ensure_pair(common, left).unwrap();
        let wrong_input = store.ensure_pair(common, right).unwrap();

        let authority_dictionary =
            define_structural_role_dictionary(&mut store, &[]).unwrap();
        let interpreter =
            define_structural_interpreter(&mut store, authority_dictionary, grammar, theory)
                .unwrap();

        let dictionary =
            define_structural_role_dictionary(&mut store, &[role]).unwrap();
        let before = store.ensure_pair(role, input).unwrap();
        let after = store.ensure_pair(role, output).unwrap();
        let bundle = materialize_exact_sequence(&mut store, &[after]).unwrap();
        let body = store.ensure_pair(before, bundle).unwrap();
        let rule = define_structural_rule(&mut store, dictionary, body).unwrap();
        let admission = admit_structural_rule(&mut store, theory, rule).unwrap();
        index_structural_rule_trigger(&mut store, common, admission).unwrap();

        let active = store.ensure_pair(caller, wrong_input).unwrap();

        let mut engine = OptimizedStructuralEngine::new(8);
        engine.set_interpreter(&store, interpreter).unwrap();
        engine.set_current(&store, &[active]).unwrap();
        let bank = engine.current_bank();

        let reaction = engine.run(&mut store).unwrap();
        assert!(reaction.quiescent);
        assert_eq!(reaction.raw_rule_matches, 0);
        assert_eq!(reaction.handoff_count, 0);
        assert_eq!(engine.current_bank(), bank);
        assert_eq!(engine.current(), &[active]);
    }

    #[test]
    fn declared_role_missing_from_template_is_rejected() {
        let mut store = OptimizedLinkStore::new();
        let anchors = fresh(&mut store, 12);
        let declared_role = anchors[10];
        let grounded = store.ensure_pair(anchors[2], anchors[3]).unwrap();

        assert_eq!(
            unify_structural_rule_template(
                &store,
                grounded,
                grounded,
                &[declared_role],
            ),
            Err(StructuralError::MissingRoleBinding(declared_role))
        );
    }

    #[test]
    fn foreign_theory_projection_does_not_broaden_authority() {
        let mut store = OptimizedLinkStore::new();
        let anchors = fresh(&mut store, 32);
        let theory_a = anchors[0];
        let theory_b = anchors[1];
        let grammar = anchors[2];
        let caller = anchors[3];
        let input = anchors[4];
        let output = anchors[5];
        let role = anchors[24];

        let authority_dictionary =
            define_structural_role_dictionary(&mut store, &[]).unwrap();
        let interpreter =
            define_structural_interpreter(&mut store, authority_dictionary, grammar, theory_a)
                .unwrap();

        let dictionary =
            define_structural_role_dictionary(&mut store, &[role]).unwrap();
        let before = store.ensure_pair(role, input).unwrap();
        let after = store.ensure_pair(role, output).unwrap();
        let bundle = materialize_exact_sequence(&mut store, &[after]).unwrap();
        let body = store.ensure_pair(before, bundle).unwrap();
        let rule = define_structural_rule(&mut store, dictionary, body).unwrap();

        // The projection is discoverable by trigger, but its admission belongs
        // to theory_b while the active interpreter authorizes theory_a.
        let foreign_admission =
            admit_structural_rule(&mut store, theory_b, rule).unwrap();
        let (trigger_key, _) = store.poles(input).unwrap();
        index_structural_rule_trigger(&mut store, trigger_key, foreign_admission).unwrap();

        let active = store.ensure_pair(caller, input).unwrap();
        let mut engine = OptimizedStructuralEngine::new(8);
        engine.set_interpreter(&store, interpreter).unwrap();
        engine.set_current(&store, &[active]).unwrap();

        let reaction = engine.run(&mut store).unwrap();
        assert!(reaction.quiescent);
        assert_eq!(reaction.raw_rule_matches, 0);
        assert_eq!(reaction.handoff_count, 0);
        assert_eq!(engine.current(), &[active]);
    }

    #[test]
    fn malformed_output_bundle_fails_closed_before_scope_handoff() {
        let mut store = OptimizedLinkStore::new();
        let anchors = fresh(&mut store, 32);
        let theory = anchors[0];
        let grammar = anchors[1];
        let caller = anchors[2];
        let input = anchors[3];
        let output = anchors[4];
        let role = anchors[24];

        let authority_dictionary =
            define_structural_role_dictionary(&mut store, &[]).unwrap();
        let interpreter =
            define_structural_interpreter(&mut store, authority_dictionary, grammar, theory)
                .unwrap();

        let dictionary =
            define_structural_role_dictionary(&mut store, &[role]).unwrap();
        let before = store.ensure_pair(role, input).unwrap();

        // Deliberately not an ExactSequence carrier.
        let malformed_bundle = store.ensure_pair(role, output).unwrap();
        let body = store.ensure_pair(before, malformed_bundle).unwrap();
        let rule = define_structural_rule(&mut store, dictionary, body).unwrap();
        let admission = admit_structural_rule(&mut store, theory, rule).unwrap();
        let (trigger_key, _) = store.poles(input).unwrap();
        index_structural_rule_trigger(&mut store, trigger_key, admission).unwrap();

        let active = store.ensure_pair(caller, input).unwrap();
        let mut engine = OptimizedStructuralEngine::new(8);
        engine.set_interpreter(&store, interpreter).unwrap();
        engine.set_current(&store, &[active]).unwrap();
        let bank = engine.current_bank();

        let error = engine.run(&mut store).unwrap_err();
        assert!(matches!(error, StructuralError::InvalidExactSequence(_)));
        assert_eq!(engine.current_bank(), bank);
        assert_eq!(engine.current(), &[active]);
        assert_eq!(engine.handoff_count(), 0);
        assert!(!engine.quiescent());
    }

    #[test]
    fn duplicate_rule_images_converge_to_one_canonical_successor() {
        let mut store = OptimizedLinkStore::new();
        let anchors = fresh(&mut store, 40);
        let theory = anchors[0];
        let grammar = anchors[1];
        let caller = anchors[2];
        let input = anchors[3];
        let output = anchors[4];
        let role_a = anchors[30];
        let role_b = anchors[31];

        let authority_dictionary =
            define_structural_role_dictionary(&mut store, &[]).unwrap();
        let interpreter =
            define_structural_interpreter(&mut store, authority_dictionary, grammar, theory)
                .unwrap();

        let (trigger_key, _) = store.poles(input).unwrap();

        for role in [role_a, role_b] {
            let dictionary =
                define_structural_role_dictionary(&mut store, &[role]).unwrap();
            let before = store.ensure_pair(role, input).unwrap();
            let after = store.ensure_pair(role, output).unwrap();
            let bundle = materialize_exact_sequence(&mut store, &[after]).unwrap();
            let body = store.ensure_pair(before, bundle).unwrap();
            let rule = define_structural_rule(&mut store, dictionary, body).unwrap();
            let admission = admit_structural_rule(&mut store, theory, rule).unwrap();
            index_structural_rule_trigger(&mut store, trigger_key, admission).unwrap();
        }

        let active = store.ensure_pair(caller, input).unwrap();
        let expected = store.ensure_pair(caller, output).unwrap();

        let mut engine = OptimizedStructuralEngine::new(8);
        engine.set_interpreter(&store, interpreter).unwrap();
        engine.set_current(&store, &[active]).unwrap();

        let reaction = engine.run(&mut store).unwrap();
        assert_eq!(reaction.raw_rule_matches, 2);
        assert_eq!(reaction.transitioned_members, 1);
        assert_eq!(reaction.handoff_count, 1);
        assert_eq!(engine.current(), &[expected]);
    }

    #[test]
    fn deep_structural_template_instantiation_is_stack_safe() {
        let mut store = OptimizedLinkStore::new();
        let depth = 50_000usize;
        let mut template = ROOT_HANDLE;

        for _ in 0..depth {
            template = store.ensure_start_self_closed(template).unwrap();
        }

        let links_before = store.link_count();
        let instantiated =
            instantiate_structural_template(&mut store, template, &[]).unwrap();

        assert_eq!(instantiated, template);
        assert_eq!(
            store.link_count(),
            links_before,
            "canonical deep instantiation must not grow the carrier"
        );
    }


}
