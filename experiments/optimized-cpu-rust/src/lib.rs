use std::collections::{HashMap, HashSet};

pub type Handle = u32;
pub const ROOT_HANDLE: Handle = 1;

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
    by_start: HashMap<Handle, Vec<Handle>>,
    by_end: HashMap<Handle, Vec<Handle>>,
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
            by_start: HashMap::new(),
            by_end: HashMap::new(),
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

    pub fn start_incidence(&self, start: Handle) -> Result<&[Handle], StoreError> {
        self.require_valid(start)?;
        Ok(self.by_start.get(&start).map(Vec::as_slice).unwrap_or(&[]))
    }

    pub fn end_incidence(&self, end: Handle) -> Result<&[Handle], StoreError> {
        self.require_valid(end)?;
        Ok(self.by_end.get(&end).map(Vec::as_slice).unwrap_or(&[]))
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
        self.by_start.entry(start).or_default().push(handle);
        self.by_end.entry(end).or_default().push(handle);
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
    };
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
