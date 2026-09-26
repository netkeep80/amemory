use std::collections::{HashMap, HashSet};

pub type Handle = u32;
pub const ROOT_HANDLE: Handle = 1;
const NO_HANDLE: Handle = 0;

#[derive(Clone, Debug)]
pub struct IncidenceIter<'a> {
    next: &'a [Handle],
    current: Handle,
    remaining: usize,
}

impl Iterator for IncidenceIter<'_> {
    type Item = Handle;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current == NO_HANDLE {
            self.remaining = 0;
            return None;
        }
        let out = self.current;
        self.current = self.next[out as usize];
        self.remaining = self.remaining.saturating_sub(1);
        Some(out)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl ExactSizeIterator for IncidenceIter<'_> {
    fn len(&self) -> usize {
        self.remaining
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Pair {
    start: Handle,
    end: Handle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Record {
    start: Handle,
    end: Handle,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreError {
    EmptyAnum,
    InvalidToken(char),
    TruncatedAnum,
    TrailingInput(usize),
    CapacityExceeded,
    UnknownHandle(Handle),
    NonWellFounded(Handle),
}

#[derive(Clone, Debug)]
pub struct OptimizedLinkStore {
    records: Vec<Option<Record>>,
    canonical_by_pair: HashMap<Pair, Handle>,
    start_forms: HashMap<Handle, Handle>,
    end_forms: HashMap<Handle, Handle>,
    // Dense intrusive incidence lists. Handles are dense local substrate IDs,
    // so per-handle arrays avoid one HashMap entry + Vec allocation per pole.
    start_head: Vec<Handle>,
    end_head: Vec<Handle>,
    next_by_start: Vec<Handle>,
    next_by_end: Vec<Handle>,
    start_count: Vec<u32>,
    end_count: Vec<u32>,
    max_links: Option<usize>,
}

impl Default for OptimizedLinkStore {
    fn default() -> Self {
        Self::new()
    }
}

impl OptimizedLinkStore {
    pub fn new() -> Self {
        Self::with_optional_limit(None)
    }

    pub fn with_max_links(max_links: usize) -> Self {
        Self::with_optional_limit(Some(max_links))
    }

    fn with_optional_limit(max_links: Option<usize>) -> Self {
        assert!(max_links.map_or(true, |limit| limit >= 1));
        let mut store = Self {
            records: vec![None, Some(Record {
                start: ROOT_HANDLE,
                end: ROOT_HANDLE,
            })],
            canonical_by_pair: HashMap::new(),
            start_forms: HashMap::new(),
            end_forms: HashMap::new(),
            start_head: vec![NO_HANDLE; 2],
            end_head: vec![NO_HANDLE; 2],
            next_by_start: vec![NO_HANDLE; 2],
            next_by_end: vec![NO_HANDLE; 2],
            start_count: vec![0; 2],
            end_count: vec![0; 2],
            max_links,
        };
        // ROOT and self-incidence forms are not ordinary PAIR representatives.
        // An ordinary PAIR may have the same two pole handles as such a record
        // while remaining a distinct target Link because self-incidence is
        // relative to the record's own handle.
        store.index_record(ROOT_HANDLE, ROOT_HANDLE, ROOT_HANDLE);
        store
    }

    pub fn link_count(&self) -> usize {
        self.records.len() - 1
    }

    pub fn is_valid(&self, handle: Handle) -> bool {
        handle > 0
            && (handle as usize) < self.records.len()
            && self.records[handle as usize].is_some()
    }

    pub fn poles(&self, handle: Handle) -> Result<(Handle, Handle), StoreError> {
        let record = self
            .records
            .get(handle as usize)
            .and_then(|record| *record)
            .ok_or(StoreError::UnknownHandle(handle))?;
        Ok((record.start, record.end))
    }

    pub fn import_anum(&mut self, source: &str) -> Result<Handle, StoreError> {
        if source.is_empty() {
            return Err(StoreError::EmptyAnum);
        }

        // Whole-Anum import is transactional. Parsing/canonicalization happens
        // against a staging clone; only complete success replaces live state.
        let mut staging = self.clone();
        let bytes = source.as_bytes();
        let mut cursor = 0usize;
        let handle = staging.parse_node(bytes, &mut cursor)?;
        if cursor != bytes.len() {
            return Err(StoreError::TrailingInput(cursor));
        }
        *self = staging;
        Ok(handle)
    }

    pub fn export_anum(&self, handle: Handle) -> Result<String, StoreError> {
        let mut visiting = HashSet::new();
        self.export_node(handle, &mut visiting)
    }

    pub fn ensure_pair(
        &mut self,
        start: Handle,
        end: Handle,
    ) -> Result<Handle, StoreError> {
        self.require_valid(start)?;
        self.require_valid(end)?;

        let key = Pair { start, end };
        if let Some(handle) = self.canonical_by_pair.get(&key) {
            return Ok(*handle);
        }

        self.allocate_record(start, end)
    }

    pub fn start_incidence(&self, start: Handle) -> Result<IncidenceIter<'_>, StoreError> {
        self.require_valid(start)?;
        Ok(IncidenceIter {
            next: &self.next_by_start,
            current: self.start_head[start as usize],
            remaining: self.start_count[start as usize] as usize,
        })
    }

    pub fn end_incidence(&self, end: Handle) -> Result<IncidenceIter<'_>, StoreError> {
        self.require_valid(end)?;
        Ok(IncidenceIter {
            next: &self.next_by_end,
            current: self.end_head[end as usize],
            remaining: self.end_count[end as usize] as usize,
        })
    }

    fn ordinary_pair_poles(&self, handle: Handle) -> Result<Option<(Handle, Handle)>, StoreError> {
        let (start, end) = self.poles(handle)?;
        if start == handle || end == handle {
            return Ok(None);
        }
        self.require_valid(start)?;
        self.require_valid(end)?;
        Ok(Some((start, end)))
    }

    fn is_root_link(&self, handle: Handle) -> Result<bool, StoreError> {
        let (start, end) = self.poles(handle)?;
        Ok(start == handle && end == handle)
    }

    fn find_ordinary_pair(
        &self,
        start: Handle,
        end: Handle,
    ) -> Result<Option<Handle>, StoreError> {
        self.require_valid(start)?;
        self.require_valid(end)?;
        Ok(self.canonical_by_pair.get(&Pair { start, end }).copied())
    }

    fn require_valid(&self, handle: Handle) -> Result<(), StoreError> {
        if self.is_valid(handle) {
            Ok(())
        } else {
            Err(StoreError::UnknownHandle(handle))
        }
    }

    fn next_handle(&self) -> Result<Handle, StoreError> {
        let next = self.records.len();
        if self.max_links.is_some_and(|limit| self.link_count() >= limit) {
            return Err(StoreError::CapacityExceeded);
        }
        Handle::try_from(next).map_err(|_| StoreError::CapacityExceeded)
    }

    fn allocate_record(
        &mut self,
        start: Handle,
        end: Handle,
    ) -> Result<Handle, StoreError> {
        let handle = self.next_handle()?;
        self.records.push(Some(Record { start, end }));
        self.canonical_by_pair.insert(Pair { start, end }, handle);
        self.index_record(handle, start, end);
        Ok(handle)
    }

    fn ensure_start_form(&mut self, child: Handle) -> Result<Handle, StoreError> {
        self.require_valid(child)?;
        if let Some(handle) = self.start_forms.get(&child) {
            return Ok(*handle);
        }
        let handle = self.next_handle()?;
        let record = Record {
            start: handle,
            end: child,
        };
        self.records.push(Some(record));
        self.start_forms.insert(child, handle);
        self.index_record(handle, handle, child);
        Ok(handle)
    }

    fn ensure_end_form(&mut self, child: Handle) -> Result<Handle, StoreError> {
        self.require_valid(child)?;
        if let Some(handle) = self.end_forms.get(&child) {
            return Ok(*handle);
        }
        let handle = self.next_handle()?;
        let record = Record {
            start: child,
            end: handle,
        };
        self.records.push(Some(record));
        self.end_forms.insert(child, handle);
        self.index_record(handle, child, handle);
        Ok(handle)
    }

    fn index_record(&mut self, handle: Handle, start: Handle, end: Handle) {
        let required = (handle.max(start).max(end) as usize) + 1;
        if self.start_head.len() < required {
            self.start_head.resize(required, NO_HANDLE);
            self.end_head.resize(required, NO_HANDLE);
            self.next_by_start.resize(required, NO_HANDLE);
            self.next_by_end.resize(required, NO_HANDLE);
            self.start_count.resize(required, 0);
            self.end_count.resize(required, 0);
        }

        let hi = handle as usize;
        let si = start as usize;
        let ei = end as usize;

        self.next_by_start[hi] = self.start_head[si];
        self.start_head[si] = handle;
        self.start_count[si] = self.start_count[si].saturating_add(1);

        self.next_by_end[hi] = self.end_head[ei];
        self.end_head[ei] = handle;
        self.end_count[ei] = self.end_count[ei].saturating_add(1);
    }

    fn parse_node(&mut self, bytes: &[u8], cursor: &mut usize) -> Result<Handle, StoreError> {
        if *cursor >= bytes.len() {
            return Err(StoreError::TruncatedAnum);
        }

        let token = bytes[*cursor] as char;
        *cursor += 1;

        match token {
            '8' => Ok(ROOT_HANDLE),
            '9' => {
                let child = self.parse_node(bytes, cursor)?;
                self.ensure_start_form(child)
            }
            '6' => {
                let child = self.parse_node(bytes, cursor)?;
                self.ensure_end_form(child)
            }
            '1' => {
                let start = self.parse_node(bytes, cursor)?;
                let end = self.parse_node(bytes, cursor)?;
                self.ensure_pair(start, end)
            }
            other => Err(StoreError::InvalidToken(other)),
        }
    }

    fn export_node(
        &self,
        handle: Handle,
        visiting: &mut HashSet<Handle>,
    ) -> Result<String, StoreError> {
        let (start, end) = self.poles(handle)?;

        if start == handle && end == handle {
            return Ok("8".to_owned());
        }

        if !visiting.insert(handle) {
            return Err(StoreError::NonWellFounded(handle));
        }

        let result = if start == handle {
            format!("9{}", self.export_node(end, visiting)?)
        } else if end == handle {
            format!("6{}", self.export_node(start, visiting)?)
        } else {
            format!(
                "1{}{}",
                self.export_node(start, visiting)?,
                self.export_node(end, visiting)?
            )
        };

        visiting.remove(&handle);
        Ok(result)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReactionError {
    ScopeCapacity { requested: usize, cap: usize },
    UnknownHandle(Handle),
    InvalidCurrentRecord(Handle),
    InvalidTheoryRelation(Handle),
    MissingPreexistingSuccessor { context: Handle, output: Handle },
    Store(StoreError),
}

impl From<StoreError> for ReactionError {
    fn from(value: StoreError) -> Self {
        Self::Store(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PortableReactionResult {
    pub scope: Vec<String>,
    pub matched_relations: u32,
    pub handoff: u32,
    pub quiescent: bool,
}

#[derive(Clone, Debug)]
pub struct OptimizedReactionEngine {
    cap: usize,
    scope_banks: [Vec<Handle>; 2],
    current_bank: usize,
    live_theory: Vec<Handle>,
    snapshot: Vec<Handle>,
    snapshot_by_antecedent: HashMap<Handle, Vec<Handle>>,
    matched_relations: u32,
    handoff_count: u32,
    quiescent: bool,
}

impl OptimizedReactionEngine {
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0);
        Self {
            cap,
            scope_banks: [Vec::new(), Vec::new()],
            current_bank: 0,
            live_theory: Vec::new(),
            snapshot: Vec::new(),
            snapshot_by_antecedent: HashMap::new(),
            matched_relations: 0,
            handoff_count: 0,
            quiescent: false,
        }
    }

    pub fn reset(&mut self) {
        self.scope_banks[0].clear();
        self.scope_banks[1].clear();
        self.current_bank = 0;
        self.live_theory.clear();
        self.snapshot.clear();
        self.snapshot_by_antecedent.clear();
        self.matched_relations = 0;
        self.handoff_count = 0;
        self.quiescent = false;
    }

    pub fn set_current(
        &mut self,
        store: &OptimizedLinkStore,
        members: &[Handle],
    ) -> Result<(), ReactionError> {
        self.check_cap(members.len())?;
        for handle in members {
            if !store.is_valid(*handle) {
                return Err(ReactionError::UnknownHandle(*handle));
            }
        }
        self.scope_banks[self.current_bank] = members.to_vec();
        Ok(())
    }

    pub fn set_theory(
        &mut self,
        store: &OptimizedLinkStore,
        relations: &[Handle],
    ) -> Result<(), ReactionError> {
        self.check_cap(relations.len())?;
        for relation in relations {
            if store.ordinary_pair_poles(*relation)?.is_none() {
                return Err(ReactionError::InvalidTheoryRelation(*relation));
            }
        }
        self.live_theory = relations.to_vec();
        Ok(())
    }

    pub fn snapshot_theory(
        &mut self,
        store: &OptimizedLinkStore,
    ) -> Result<(), ReactionError> {
        self.check_cap(self.live_theory.len())?;

        let mut snapshot_by_antecedent: HashMap<Handle, Vec<Handle>> = HashMap::new();
        for relation in &self.live_theory {
            let Some((antecedent, _)) = store.ordinary_pair_poles(*relation)? else {
                return Err(ReactionError::InvalidTheoryRelation(*relation));
            };
            snapshot_by_antecedent
                .entry(antecedent)
                .or_default()
                .push(*relation);
        }

        self.snapshot = self.live_theory.clone();
        self.snapshot_by_antecedent = snapshot_by_antecedent;
        Ok(())
    }

    pub fn run(&mut self, store: &OptimizedLinkStore) -> Result<(), ReactionError> {
        // Runtime rejection is never semantic quiescence.
        self.quiescent = false;

        let current = self.scope_banks[self.current_bank].clone();
        self.check_cap(current.len())?;
        self.check_cap(self.snapshot.len())?;

        let mut successor = Vec::with_capacity(self.cap.min(current.len().saturating_mul(2)));
        let mut successor_seen = HashSet::new();
        let mut matched = 0u32;

        for member in current {
            let Some((context, antecedent)) = store.ordinary_pair_poles(member)? else {
                return Err(ReactionError::InvalidCurrentRecord(member));
            };

            let admitted = self
                .snapshot_by_antecedent
                .get(&antecedent)
                .map(Vec::as_slice)
                .unwrap_or(&[]);

            let mut member_matches = 0u32;
            for relation in admitted {
                let Some((relation_antecedent, output)) =
                    store.ordinary_pair_poles(*relation)?
                else {
                    return Err(ReactionError::InvalidTheoryRelation(*relation));
                };

                if relation_antecedent != antecedent {
                    // Snapshot index corruption must fail closed rather than
                    // silently broadening admitted Theory authority.
                    return Err(ReactionError::InvalidTheoryRelation(*relation));
                }

                matched = matched.saturating_add(1);
                member_matches = member_matches.saturating_add(1);

                // ROOT is the bounded empty ExactSequence contribution.
                if !store.is_root_link(output)? {
                    let candidate = store
                        .find_ordinary_pair(context, output)?
                        .ok_or(ReactionError::MissingPreexistingSuccessor {
                            context,
                            output,
                        })?;
                    if successor_seen.insert(candidate) {
                        if successor.len() >= self.cap {
                            return Err(ReactionError::ScopeCapacity {
                                requested: successor.len() + 1,
                                cap: self.cap,
                            });
                        }
                        successor.push(candidate);
                    }
                }
            }

            if member_matches == 0 && successor_seen.insert(member) {
                if successor.len() >= self.cap {
                    return Err(ReactionError::ScopeCapacity {
                        requested: successor.len() + 1,
                        cap: self.cap,
                    });
                }
                successor.push(member);
            }
        }

        // Only a complete successful evaluation publishes diagnostics/state.
        self.matched_relations = matched;
        self.handoff_count = 0;

        if matched == 0 {
            self.quiescent = true;
            return Ok(());
        }

        let target_bank = 1usize - self.current_bank;
        self.scope_banks[target_bank] = successor;
        self.current_bank = target_bank;
        self.handoff_count = 1;
        Ok(())
    }

    pub fn current(&self) -> &[Handle] {
        &self.scope_banks[self.current_bank]
    }

    pub fn bank(&self, bank: usize) -> Option<&[Handle]> {
        self.scope_banks.get(bank).map(Vec::as_slice)
    }

    pub fn current_bank(&self) -> usize {
        self.current_bank
    }

    pub fn live_theory_count(&self) -> usize {
        self.live_theory.len()
    }

    pub fn snapshot_count(&self) -> usize {
        self.snapshot.len()
    }

    pub fn matched_relations(&self) -> u32 {
        self.matched_relations
    }

    pub fn handoff_count(&self) -> u32 {
        self.handoff_count
    }

    pub fn quiescent(&self) -> bool {
        self.quiescent
    }

    pub fn portable_result(
        &self,
        store: &OptimizedLinkStore,
    ) -> Result<PortableReactionResult, ReactionError> {
        let mut scope = self
            .current()
            .iter()
            .map(|handle| store.export_anum(*handle).map_err(ReactionError::Store))
            .collect::<Result<Vec<_>, _>>()?;
        scope.sort();
        scope.dedup();

        Ok(PortableReactionResult {
            scope,
            matched_relations: self.matched_relations,
            handoff: self.handoff_count,
            quiescent: self.quiescent,
        })
    }

    fn check_cap(&self, count: usize) -> Result<(), ReactionError> {
        if count > self.cap {
            Err(ReactionError::ScopeCapacity {
                requested: count,
                cap: self.cap,
            })
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use amemory_browser_probe::{
        amemory_anum_cpu_export,
        amemory_anum_cpu_import,
        amemory_anum_cpu_output_get,
        amemory_anum_cpu_pool_count,
        amemory_anum_cpu_reset_pool,
        amemory_anum_cpu_set_token,
        amemory_reaction_current_bank,
        amemory_reaction_current_count,
        amemory_reaction_current_member,
        amemory_reaction_handoff_count,
        amemory_reaction_matched_relations,
        amemory_reaction_quiescent,
        amemory_reaction_reset,
        amemory_reaction_run,
        amemory_reaction_set_current_count,
        amemory_reaction_set_current_member,
        amemory_reaction_set_theory_count,
        amemory_reaction_set_theory_relation,
        amemory_reaction_snapshot_count,
        amemory_reaction_snapshot_theory,
        amemory_reaction_theory_count,
    };
    use std::collections::HashMap as StdHashMap;
    use std::sync::Mutex;

    static REFERENCE_LOCK: Mutex<()> = Mutex::new(());

    fn reference_import(source: &str) -> u32 {
        for (index, byte) in source.bytes().enumerate() {
            let token = if byte.is_ascii_digit() {
                (byte - b'0') as u32
            } else {
                255
            };
            assert_eq!(amemory_anum_cpu_set_token(index as u32, token), 1);
        }
        amemory_anum_cpu_import(source.len() as u32)
    }

    fn reference_export(handle: u32) -> String {
        let len = amemory_anum_cpu_export(handle);
        assert_ne!(len, u32::MAX);
        let mut out = String::new();
        for index in 0..len {
            out.push(char::from_digit(amemory_anum_cpu_output_get(index), 10).unwrap());
        }
        out
    }

    const REACTION_FIXTURES: [&str; 17] = [
        "8",          // ROOT
        "98",         // K / START(ROOT)
        "68",         // A / END(ROOT)
        "16898",      // B / also R5 C->A relation
        "998",        // C / R5 context
        "19868",      // K->A / also R5 A->C relation
        "16816898",   // A->B
        "19816898",   // K->B
        "1688",       // A->ROOT ZERO
        "1988",       // K->ROOT
        "1816898",    // ROOT->B
        "116898998",  // B->C
        "198998",     // K->C
        "199898",     // R5 context->START
        "199868",     // R5 context->END
        "168998",     // A->C
        "18998",      // ROOT->C
    ];

    fn load_reference_fixture() -> StdHashMap<&'static str, u32> {
        amemory_anum_cpu_reset_pool();
        let mut map = StdHashMap::new();
        for source in REACTION_FIXTURES {
            let handle = reference_import(source);
            assert_ne!(handle, u32::MAX, "reference rejected {source}");
            map.insert(source, handle);
        }
        map
    }

    fn load_optimized_fixture() -> (OptimizedLinkStore, StdHashMap<&'static str, Handle>) {
        let mut store = OptimizedLinkStore::new();
        let mut map = StdHashMap::new();
        for source in REACTION_FIXTURES {
            let handle = store.import_anum(source).unwrap();
            map.insert(source, handle);
        }
        (store, map)
    }

    fn reference_set_current(map: &StdHashMap<&str, u32>, current: &[&str]) {
        for (index, source) in current.iter().enumerate() {
            assert_eq!(
                amemory_reaction_set_current_member(index as u32, map[*source]),
                1,
                "reference current rejected {source}"
            );
        }
        assert_eq!(amemory_reaction_set_current_count(current.len() as u32), 1);
    }

    fn reference_set_live_theory(map: &StdHashMap<&str, u32>, theory: &[&str]) {
        for (index, source) in theory.iter().enumerate() {
            assert_eq!(
                amemory_reaction_set_theory_relation(index as u32, map[*source]),
                1,
                "reference Theory rejected {source}"
            );
        }
        assert_eq!(amemory_reaction_set_theory_count(theory.len() as u32), 1);
    }

    fn reference_configure(
        map: &StdHashMap<&str, u32>,
        current: &[&str],
        theory: &[&str],
    ) {
        amemory_reaction_reset();
        reference_set_current(map, current);
        reference_set_live_theory(map, theory);
        assert_eq!(amemory_reaction_snapshot_theory(), 1);
    }

    fn reference_observe() -> PortableReactionResult {
        let count = amemory_reaction_current_count();
        let mut scope = Vec::new();
        for index in 0..count {
            scope.push(reference_export(amemory_reaction_current_member(index)));
        }
        scope.sort();
        scope.dedup();
        PortableReactionResult {
            scope,
            matched_relations: amemory_reaction_matched_relations(),
            handoff: amemory_reaction_handoff_count(),
            quiescent: amemory_reaction_quiescent() == 1,
        }
    }

    fn reference_run_result() -> PortableReactionResult {
        assert_eq!(amemory_reaction_run(), 1);
        reference_observe()
    }

    fn optimized_handles(
        map: &StdHashMap<&str, Handle>,
        sources: &[&str],
    ) -> Vec<Handle> {
        sources.iter().map(|source| map[*source]).collect()
    }

    fn optimized_configure(
        engine: &mut OptimizedReactionEngine,
        store: &OptimizedLinkStore,
        map: &StdHashMap<&str, Handle>,
        current: &[&str],
        theory: &[&str],
    ) {
        engine.reset();
        engine
            .set_current(store, &optimized_handles(map, current))
            .unwrap();
        engine
            .set_theory(store, &optimized_handles(map, theory))
            .unwrap();
        engine.snapshot_theory(store).unwrap();
    }

    #[test]
    fn structural_roundtrip_matches_reference_oracle() {
        let _guard = REFERENCE_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        amemory_anum_cpu_reset_pool();

        let fixtures = [
            "8",
            "98",
            "68",
            "19868",
            "16898",
            "198698",
            "119868968",
            "16816898",
            "19816898",
        ];

        let mut optimized = OptimizedLinkStore::new();
        for source in fixtures {
            let reference = reference_import(source);
            assert_ne!(reference, u32::MAX, "reference rejected {source}");
            let optimized_handle = optimized.import_anum(source).expect("optimized import");
            assert_eq!(reference_export(reference), source);
            assert_eq!(optimized.export_anum(optimized_handle).unwrap(), source);
            assert_eq!(
                optimized.export_anum(optimized_handle).unwrap(),
                reference_export(reference),
                "portable differential mismatch for {source}"
            );
        }

        // Canonical replay returns the same local optimized handle.
        let first = optimized.import_anum("19868").unwrap();
        let second = optimized.import_anum("19868").unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn allocation_order_changes_handles_not_portable_identity() {
        let mut left = OptimizedLinkStore::new();
        let mut right = OptimizedLinkStore::new();

        let left_target = left.import_anum("19868").unwrap();

        // Shift only the right local allocation history with an unrelated
        // canonical structure before importing the same semantic target.
        right.import_anum("998").unwrap();
        let right_target = right.import_anum("19868").unwrap();

        assert_ne!(left_target, right_target);
        assert_eq!(left.export_anum(left_target).unwrap(), "19868");
        assert_eq!(right.export_anum(right_target).unwrap(), "19868");
    }

    #[test]
    fn hash_canonicalization_and_incidence_indexes_are_consistent() {
        let mut store = OptimizedLinkStore::new();

        let k = store.import_anum("98").unwrap();
        let a = store.import_anum("68").unwrap();
        let b = store.import_anum("16898").unwrap();
        let current = store.import_anum("19868").unwrap();
        let relation = store.import_anum("16816898").unwrap();
        let successor = store.import_anum("19816898").unwrap();

        assert_eq!(store.ensure_pair(k, a).unwrap(), current);
        assert_eq!(store.ensure_pair(a, b).unwrap(), relation);
        assert_eq!(store.ensure_pair(k, b).unwrap(), successor);

        // Critical cyclic/self-incidence distinction:
        // Q = END(O) has poles (O,Q), but ordinary PAIR(O,Q) is a different
        // target P whose end points to Q rather than to P itself.
        let end_of_k = store.import_anum("698").unwrap();
        let ordinary_over_end = store.ensure_pair(k, end_of_k).unwrap();
        assert_ne!(ordinary_over_end, end_of_k);
        assert_eq!(store.export_anum(end_of_k).unwrap(), "698");
        assert_eq!(store.export_anum(ordinary_over_end).unwrap(), "198698");

        // Likewise ROOT=(R,R) does not collapse ordinary PAIR(R,R).
        let root_pair = store.ensure_pair(ROOT_HANDLE, ROOT_HANDLE).unwrap();
        assert_ne!(root_pair, ROOT_HANDLE);
        assert_eq!(store.export_anum(root_pair).unwrap(), "188");

        let from_k = store.start_incidence(k).unwrap();
        assert!(from_k.contains(&k)); // START(K child ROOT) is self-start incident.
        assert!(from_k.contains(&current));
        assert!(from_k.contains(&successor));

        let to_a = store.end_incidence(a).unwrap();
        assert!(to_a.contains(&a)); // END(ROOT) is self-end incident.
        assert!(to_a.contains(&current));

        // Index observations are still local/substrate facts. Portable
        // comparison remains canonical structural export.
        let mut outgoing = from_k
            .iter()
            .copied()
            .map(|handle| store.export_anum(handle).unwrap())
            .collect::<Vec<_>>();
        outgoing.sort();
        assert!(outgoing.contains(&"19868".to_owned()));
        assert!(outgoing.contains(&"19816898".to_owned()));
    }

    #[test]
    fn malformed_and_capacity_failures_are_transactional() {
        let mut store = OptimizedLinkStore::with_max_links(4);

        let stable = store.import_anum("19868").unwrap();
        assert_eq!(store.link_count(), 4);
        assert_eq!(store.export_anum(stable).unwrap(), "19868");

        for source in ["", "5", "1", "9", "88", "19868x"] {
            let before = store.link_count();
            assert!(store.import_anum(source).is_err(), "accepted malformed {source}");
            assert_eq!(store.link_count(), before, "partial publication for {source}");
            assert_eq!(store.export_anum(stable).unwrap(), "19868");
        }

        let before = store.link_count();
        assert_eq!(store.import_anum("16898"), Err(StoreError::CapacityExceeded));
        assert_eq!(store.link_count(), before);
        assert_eq!(store.export_anum(stable).unwrap(), "19868");
    }

    #[test]
    fn exporter_rejects_non_well_founded_corruption() {
        let mut store = OptimizedLinkStore::new();

        // Inject a substrate corruption that cannot be produced through the
        // public immutable/canonical construction API.
        store.records.push(Some(Record { start: 3, end: 1 })); // handle 2
        store.records.push(Some(Record { start: 2, end: 1 })); // handle 3

        assert!(matches!(
            store.export_anum(2),
            Err(StoreError::NonWellFounded(2 | 3))
        ));
    }

    #[test]
    fn optimized_reaction_r1_r2_r3_matches_reference() {
        let _guard = REFERENCE_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let reference = load_reference_fixture();
        let (store, optimized) = load_optimized_fixture();
        let mut engine = OptimizedReactionEngine::new(16);

        // R1 ONE.
        reference_configure(&reference, &["19868"], &["16816898"]);
        optimized_configure(&mut engine, &store, &optimized, &["19868"], &["16816898"]);
        let old_bank = engine.current_bank();
        let reference_r1 = reference_run_result();
        engine.run(&store).unwrap();
        let optimized_r1 = engine.portable_result(&store).unwrap();
        assert_eq!(optimized_r1, reference_r1);
        assert_eq!(
            optimized_r1,
            PortableReactionResult {
                scope: vec!["19816898".to_owned()],
                matched_relations: 1,
                handoff: 1,
                quiescent: false,
            }
        );
        assert_ne!(engine.current_bank(), old_bank);
        assert_eq!(
            store.export_anum(engine.bank(old_bank).unwrap()[0]).unwrap(),
            "19868"
        );

        // R2 no admitted relation / semantic quiescence.
        reference_configure(&reference, &["19868"], &["19816898"]);
        optimized_configure(&mut engine, &store, &optimized, &["19868"], &["19816898"]);
        let reference_bank = amemory_reaction_current_bank();
        let optimized_bank = engine.current_bank();
        let reference_r2 = reference_run_result();
        engine.run(&store).unwrap();
        let optimized_r2 = engine.portable_result(&store).unwrap();
        assert_eq!(optimized_r2, reference_r2);
        assert_eq!(optimized_r2.scope, vec!["19868"]);
        assert_eq!(optimized_r2.matched_relations, 0);
        assert_eq!(optimized_r2.handoff, 0);
        assert!(optimized_r2.quiescent);
        assert_eq!(amemory_reaction_current_bank(), reference_bank);
        assert_eq!(engine.current_bank(), optimized_bank);

        // R3 ZERO: admitted match, empty published successor, one handoff.
        reference_configure(&reference, &["19868"], &["1688"]);
        optimized_configure(&mut engine, &store, &optimized, &["19868"], &["1688"]);
        let reference_r3_zero = reference_run_result();
        engine.run(&store).unwrap();
        let optimized_r3_zero = engine.portable_result(&store).unwrap();
        assert_eq!(optimized_r3_zero, reference_r3_zero);
        assert!(optimized_r3_zero.scope.is_empty());
        assert_eq!(optimized_r3_zero.matched_relations, 1);
        assert_eq!(optimized_r3_zero.handoff, 1);
        assert!(!optimized_r3_zero.quiescent);

        // R3 mixed ZERO + two duplicate non-zero productions -> one successor.
        let mixed_current = ["19868", "1988"];
        let mixed_theory = ["1688", "16816898", "1816898"];
        reference_configure(&reference, &mixed_current, &mixed_theory);
        optimized_configure(&mut engine, &store, &optimized, &mixed_current, &mixed_theory);
        let reference_r3_mixed = reference_run_result();
        engine.run(&store).unwrap();
        let optimized_r3_mixed = engine.portable_result(&store).unwrap();
        assert_eq!(optimized_r3_mixed, reference_r3_mixed);
        assert_eq!(optimized_r3_mixed.scope, vec!["19816898"]);
        assert_eq!(optimized_r3_mixed.matched_relations, 3);
        assert_eq!(optimized_r3_mixed.handoff, 1);
        assert!(!optimized_r3_mixed.quiescent);
    }

    #[test]
    fn optimized_reaction_r4_theory_snapshot_matches_reference() {
        let _guard = REFERENCE_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let reference = load_reference_fixture();
        let (store, optimized) = load_optimized_fixture();
        let mut engine = OptimizedReactionEngine::new(16);

        reference_configure(&reference, &["19868"], &["16816898"]);
        optimized_configure(&mut engine, &store, &optimized, &["19868"], &["16816898"]);

        // Admit B->C only after snapshot_t.
        reference_set_live_theory(&reference, &["16816898", "116898998"]);
        engine
            .set_theory(
                &store,
                &optimized_handles(&optimized, &["16816898", "116898998"]),
            )
            .unwrap();

        assert_eq!(amemory_reaction_snapshot_count(), 1);
        assert_eq!(engine.snapshot_count(), 1);
        assert_eq!(amemory_reaction_theory_count(), 2);
        assert_eq!(engine.live_theory_count(), 2);

        let reference_t = reference_run_result();
        engine.run(&store).unwrap();
        let optimized_t = engine.portable_result(&store).unwrap();
        assert_eq!(optimized_t, reference_t);
        assert_eq!(optimized_t.scope, vec!["19816898"]);

        // New admission becomes visible only after snapshot_t+1.
        assert_eq!(amemory_reaction_snapshot_theory(), 1);
        engine.snapshot_theory(&store).unwrap();
        assert_eq!(amemory_reaction_snapshot_count(), 2);
        assert_eq!(engine.snapshot_count(), 2);

        let reference_t1 = reference_run_result();
        engine.run(&store).unwrap();
        let optimized_t1 = engine.portable_result(&store).unwrap();
        assert_eq!(optimized_t1, reference_t1);
        assert_eq!(optimized_t1.scope, vec!["198998"]);

        // Independent stale-snapshot negative control.
        reference_configure(&reference, &["19816898"], &["16816898"]);
        optimized_configure(
            &mut engine,
            &store,
            &optimized,
            &["19816898"],
            &["16816898"],
        );
        reference_set_live_theory(&reference, &["16816898", "116898998"]);
        engine
            .set_theory(
                &store,
                &optimized_handles(&optimized, &["16816898", "116898998"]),
            )
            .unwrap();

        let reference_stale = reference_run_result();
        engine.run(&store).unwrap();
        let optimized_stale = engine.portable_result(&store).unwrap();
        assert_eq!(optimized_stale, reference_stale);
        assert_eq!(optimized_stale.scope, vec!["19816898"]);
        assert_eq!(optimized_stale.matched_relations, 0);
        assert!(optimized_stale.quiescent);
    }

    #[test]
    fn optimized_reaction_r5_r6_matches_reference() {
        let _guard = REFERENCE_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let reference = load_reference_fixture();
        let (store, optimized) = load_optimized_fixture();
        let mut engine = OptimizedReactionEngine::new(16);

        // R5 bounded recurrence through structural END.
        reference_configure(&reference, &["199898"], &["19868", "16898"]);
        optimized_configure(
            &mut engine,
            &store,
            &optimized,
            &["199898"],
            &["19868", "16898"],
        );
        let expected = ["199868", "199898", "199868", "199898"];
        for expected_scope in expected {
            let reference_step = reference_run_result();
            engine.run(&store).unwrap();
            let optimized_step = engine.portable_result(&store).unwrap();
            assert_eq!(optimized_step, reference_step);
            assert_eq!(optimized_step.scope, vec![expected_scope]);
            assert_eq!(optimized_step.matched_relations, 1);
            assert_eq!(optimized_step.handoff, 1);
            assert!(!optimized_step.quiescent);
        }

        // R6 1->N.
        let one_many_theory = ["16816898", "168998"];
        reference_configure(&reference, &["19868"], &one_many_theory);
        optimized_configure(&mut engine, &store, &optimized, &["19868"], &one_many_theory);
        let reference_one_many = reference_run_result();
        engine.run(&store).unwrap();
        let optimized_one_many = engine.portable_result(&store).unwrap();
        assert_eq!(optimized_one_many, reference_one_many);
        assert_eq!(
            optimized_one_many.scope,
            vec!["19816898".to_owned(), "198998".to_owned()]
        );
        assert_eq!(optimized_one_many.matched_relations, 2);

        // R6 N->M.
        let many_current = ["19868", "1988"];
        let many_theory = ["16816898", "18998"];
        reference_configure(&reference, &many_current, &many_theory);
        optimized_configure(&mut engine, &store, &optimized, &many_current, &many_theory);
        let reference_many = reference_run_result();
        engine.run(&store).unwrap();
        let optimized_many = engine.portable_result(&store).unwrap();
        assert_eq!(optimized_many, reference_many);

        // P15: reverse both physical iteration orders; normalized result unchanged.
        let reversed_current = ["1988", "19868"];
        let reversed_theory = ["18998", "16816898"];
        reference_configure(&reference, &reversed_current, &reversed_theory);
        optimized_configure(
            &mut engine,
            &store,
            &optimized,
            &reversed_current,
            &reversed_theory,
        );
        let reference_reordered = reference_run_result();
        engine.run(&store).unwrap();
        let optimized_reordered = engine.portable_result(&store).unwrap();
        assert_eq!(optimized_reordered, reference_reordered);
        assert_eq!(optimized_reordered, optimized_many);

        // Same bounded scope guard as reference prototype.
        amemory_reaction_reset();
        assert_eq!(amemory_reaction_set_current_count(17), 0);
        let too_many = vec![optimized["19868"]; 17];
        assert!(matches!(
            engine.set_current(&store, &too_many),
            Err(ReactionError::ScopeCapacity {
                requested: 17,
                cap: 16
            })
        ));
    }

    #[test]
    fn optimized_reaction_missing_successor_fails_closed_like_reference() {
        let _guard = REFERENCE_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

        let sources = ["98", "68", "16898", "19868", "16816898"];

        amemory_anum_cpu_reset_pool();
        let mut reference = StdHashMap::new();
        for source in sources {
            reference.insert(source, reference_import(source));
        }

        let mut store = OptimizedLinkStore::new();
        let mut optimized = StdHashMap::new();
        for source in sources {
            optimized.insert(source, store.import_anum(source).unwrap());
        }

        reference_configure(&reference, &["19868"], &["16816898"]);
        let reference_bank = amemory_reaction_current_bank();

        let mut engine = OptimizedReactionEngine::new(16);
        optimized_configure(&mut engine, &store, &optimized, &["19868"], &["16816898"]);
        let optimized_bank = engine.current_bank();

        assert_eq!(amemory_reaction_run(), 0);
        let optimized_error = engine.run(&store).unwrap_err();
        assert!(matches!(
            optimized_error,
            ReactionError::MissingPreexistingSuccessor { .. }
        ));

        let reference_after = reference_observe();
        let optimized_after = engine.portable_result(&store).unwrap();
        assert_eq!(reference_after, optimized_after);
        assert_eq!(reference_after.scope, vec!["19868"]);
        assert_eq!(reference_after.handoff, 0);
        assert!(!reference_after.quiescent);
        assert_eq!(amemory_reaction_current_bank(), reference_bank);
        assert_eq!(engine.current_bank(), optimized_bank);
    }

    #[test]
    #[ignore = "informational optimized-reaction baseline; no performance threshold"]
    fn optimized_reaction_benchmark_baseline() {
        use std::hint::black_box;
        use std::time::Instant;

        const ITERS: u128 = 1_000_000;

        let (store, optimized) = load_optimized_fixture();
        let current = optimized["19868"];
        let relation = optimized["16816898"];
        let successor = optimized["19816898"];

        // Full bounded R1 orchestration: current + live Theory + snapshot index + run.
        let mut engine = OptimizedReactionEngine::new(16);
        let full_started = Instant::now();
        for _ in 0..ITERS {
            engine.reset();
            engine.set_current(&store, &[current]).unwrap();
            engine.set_theory(&store, &[relation]).unwrap();
            engine.snapshot_theory(&store).unwrap();
            engine.run(&store).unwrap();
            black_box(engine.current()[0]);
        }
        let full_ns = full_started.elapsed().as_nanos() / ITERS;
        assert_eq!(engine.current(), &[successor]);

        // Reuse one TheorySnapshot/index across reactions. Reset only current
        // semantic Scope before each run; published bank may alternate.
        engine.reset();
        engine.set_theory(&store, &[relation]).unwrap();
        engine.snapshot_theory(&store).unwrap();
        let reused_started = Instant::now();
        for _ in 0..ITERS {
            engine.set_current(&store, &[current]).unwrap();
            engine.run(&store).unwrap();
            black_box(engine.current()[0]);
        }
        let reused_ns = reused_started.elapsed().as_nanos() / ITERS;
        assert_eq!(engine.current(), &[successor]);

        println!("OPT_CPU_P2_ITERS={ITERS}");
        println!("OPT_CPU_P2_R1_FULL_SNAPSHOT_NS_PER_OP={full_ns}");
        println!("OPT_CPU_P2_R1_REUSED_SNAPSHOT_NS_PER_OP={reused_ns}");
        println!("OPT_CPU_P2_NOTE=informational-only-no-performance-threshold");
    }

    #[test]
    #[ignore = "informational optimized-index baseline; no performance threshold"]
    fn optimized_hash_index_benchmark_baseline() {
        use std::hint::black_box;
        use std::time::Instant;

        const LINKS: usize = 50_000;
        const ITERS: u128 = 1_000_000;

        let mut store = OptimizedLinkStore::new();
        let build_started = Instant::now();
        let mut current = ROOT_HANDLE;
        for _ in 0..LINKS {
            current = store.ensure_pair(ROOT_HANDLE, current).unwrap();
        }
        let build_ns_per_link = build_started.elapsed().as_nanos() / LINKS as u128;
        assert_eq!(store.link_count(), LINKS + 1);

        let (_, target_end) = store.poles(current).unwrap();

        let canonical_started = Instant::now();
        let mut last = ROOT_HANDLE;
        for _ in 0..ITERS {
            last = store.ensure_pair(ROOT_HANDLE, target_end).unwrap();
            black_box(last);
        }
        let canonical_ns = canonical_started.elapsed().as_nanos() / ITERS;
        assert_eq!(last, current);
        assert_eq!(store.link_count(), LINKS + 1);

        let incidence_started = Instant::now();
        let mut observed = 0usize;
        for _ in 0..ITERS {
            observed = store.start_incidence(ROOT_HANDLE).unwrap().len();
            black_box(observed);
        }
        let incidence_ns = incidence_started.elapsed().as_nanos() / ITERS;
        assert_eq!(observed, LINKS + 1); // ROOT plus all ordinary root-start pairs.

        println!("OPT_CPU_P1_LINKS={LINKS}");
        println!("OPT_CPU_P1_ITERS={ITERS}");
        println!("OPT_CPU_P1_BUILD_NS_PER_LINK={build_ns_per_link}");
        println!("OPT_CPU_P1_CANONICAL_HIT_NS_PER_OP={canonical_ns}");
        println!("OPT_CPU_P1_START_INCIDENCE_LOOKUP_NS_PER_OP={incidence_ns}");
        println!("OPT_CPU_P1_NOTE=informational-only-no-performance-threshold");
    }

    #[test]
    fn reference_and_optimized_pool_counts_need_not_match() {
        let _guard = REFERENCE_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        amemory_anum_cpu_reset_pool();
        let reference = reference_import("19868");
        assert_ne!(reference, u32::MAX);

        let mut optimized = OptimizedLinkStore::new();
        let target = optimized.import_anum("19868").unwrap();

        // Local count/handle layout are explicitly not portable identity.
        assert_eq!(reference_export(reference), optimized.export_anum(target).unwrap());
        assert!(amemory_anum_cpu_pool_count() >= 1);
        assert!(optimized.link_count() >= 1);
    }
}
