pub mod orientation;
pub mod structural;

use std::{
    collections::{HashMap, HashSet},
    sync::atomic::{AtomicU32, Ordering},
};

pub type Handle = u32;
pub const ROOT_HANDLE: Handle = 1;
pub const PACKED_CARRIER_SCHEMA_VERSION: u32 = 1;
pub const PACKED_INCIDENCE_INDEX_SCHEMA_VERSION: u32 = 1;
const NO_HANDLE: Handle = 0;
static NEXT_STORE_INSTANCE_ID: AtomicU32 = AtomicU32::new(1);

fn next_store_instance_id() -> u32 {
    let id = NEXT_STORE_INSTANCE_ID.fetch_add(1, Ordering::Relaxed);
    assert!(id != u32::MAX, "OptimizedLinkStore instance id exhausted");
    id
}

#[derive(Clone, Debug)]
pub struct IncidenceIter<'a> {
    next: &'a [Handle],
    current: Handle,
}

impl Iterator for IncidenceIter<'_> {
    type Item = Handle;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current == NO_HANDLE {
            return None;
        }
        let out = self.current;
        self.current = self.next[out as usize];
        Some(out)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Pair {
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
    InvalidPackedCarrier {
        handle: Handle,
        start: Handle,
        end: Handle,
    },
    UnsupportedPackedCarrierSchema(u32),
    PackedCarrierLengthMismatch {
        starts: usize,
        ends: usize,
    },
    UnsupportedPackedIncidenceIndexSchema(u32),
    PackedIncidenceIndexLengthMismatch {
        expected: usize,
        start_head: usize,
        end_head: usize,
        next_by_start: usize,
        next_by_end: usize,
    },
    InvalidPackedIncidenceIndex,
    InvalidPackedCarrierRoot {
        expected: Handle,
        actual: Handle,
    },
    NonWellFounded(Handle),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackedCarrierImage {
    schema_version: u32,
    root_handle: Handle,
    starts: Vec<Handle>,
    ends: Vec<Handle>,
}

impl PackedCarrierImage {
    pub fn from_parts(
        schema_version: u32,
        root_handle: Handle,
        starts: Vec<Handle>,
        ends: Vec<Handle>,
    ) -> Result<Self, StoreError> {
        let image = Self {
            schema_version,
            root_handle,
            starts,
            ends,
        };
        image.validate()?;
        Ok(image)
    }

    pub fn from_duplets(
        duplets: &[(Handle, Handle)],
    ) -> Result<Self, StoreError> {
        let mut starts = Vec::with_capacity(duplets.len());
        let mut ends = Vec::with_capacity(duplets.len());
        for &(start, end) in duplets {
            starts.push(start);
            ends.push(end);
        }
        Self::from_parts(
            PACKED_CARRIER_SCHEMA_VERSION,
            ROOT_HANDLE,
            starts,
            ends,
        )
    }

    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub fn root_handle(&self) -> Handle {
        self.root_handle
    }

    pub fn link_count(&self) -> usize {
        self.starts.len()
    }

    pub fn starts(&self) -> &[Handle] {
        &self.starts
    }

    pub fn ends(&self) -> &[Handle] {
        &self.ends
    }

    pub fn duplet(&self, handle: Handle) -> Option<(Handle, Handle)> {
        if handle == 0 {
            return None;
        }
        let index = usize::try_from(handle - 1).ok()?;
        Some((*self.starts.get(index)?, *self.ends.get(index)?))
    }

    pub fn duplets(
        &self,
    ) -> impl ExactSizeIterator<Item = (Handle, Handle)> + '_ {
        self.starts
            .iter()
            .copied()
            .zip(self.ends.iter().copied())
    }

    pub fn validate(&self) -> Result<(), StoreError> {
        if self.schema_version != PACKED_CARRIER_SCHEMA_VERSION {
            return Err(StoreError::UnsupportedPackedCarrierSchema(
                self.schema_version,
            ));
        }
        if self.root_handle != ROOT_HANDLE {
            return Err(StoreError::InvalidPackedCarrierRoot {
                expected: ROOT_HANDLE,
                actual: self.root_handle,
            });
        }
        if self.starts.len() != self.ends.len() {
            return Err(StoreError::PackedCarrierLengthMismatch {
                starts: self.starts.len(),
                ends: self.ends.len(),
            });
        }
        if self.starts.is_empty()
            || self.starts[0] != ROOT_HANDLE
            || self.ends[0] != ROOT_HANDLE
        {
            return Err(StoreError::InvalidPackedCarrier {
                handle: ROOT_HANDLE,
                start: self.starts.first().copied().unwrap_or(NO_HANDLE),
                end: self.ends.first().copied().unwrap_or(NO_HANDLE),
            });
        }

        let mut ordinary_pairs = HashSet::new();
        let mut start_forms = HashSet::new();
        let mut end_forms = HashSet::new();

        for index in 1..self.starts.len() {
            let handle = Handle::try_from(index + 1)
                .map_err(|_| StoreError::CapacityExceeded)?;
            let start = self.starts[index];
            let end = self.ends[index];

            let valid = if start == handle {
                end != NO_HANDLE
                    && end < handle
                    && start_forms.insert(end)
            } else if end == handle {
                start != NO_HANDLE
                    && start < handle
                    && end_forms.insert(start)
            } else {
                start != NO_HANDLE
                    && end != NO_HANDLE
                    && start < handle
                    && end < handle
                    && ordinary_pairs.insert(Pair { start, end })
            };
            if !valid {
                return Err(StoreError::InvalidPackedCarrier {
                    handle,
                    start,
                    end,
                });
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackedIncidenceIndexImage {
    schema_version: u32,
    start_head: Vec<Handle>,
    end_head: Vec<Handle>,
    next_by_start: Vec<Handle>,
    next_by_end: Vec<Handle>,
}

fn derive_packed_incidence_arrays(
    carrier: &PackedCarrierImage,
) -> Result<(Vec<Handle>, Vec<Handle>, Vec<Handle>, Vec<Handle>), StoreError> {
    carrier.validate()?;
    let len = carrier
        .link_count()
        .checked_add(1)
        .ok_or(StoreError::CapacityExceeded)?;
    let mut start_head = vec![NO_HANDLE; len];
    let mut end_head = vec![NO_HANDLE; len];
    let mut next_by_start = vec![NO_HANDLE; len];
    let mut next_by_end = vec![NO_HANDLE; len];

    for raw_handle in 1..=carrier.link_count() {
        let handle =
            Handle::try_from(raw_handle).map_err(|_| StoreError::CapacityExceeded)?;
        let (start, end) = carrier
            .duplet(handle)
            .ok_or(StoreError::InvalidPackedIncidenceIndex)?;
        let hi = handle as usize;
        let si = start as usize;
        let ei = end as usize;

        next_by_start[hi] = start_head[si];
        start_head[si] = handle;
        next_by_end[hi] = end_head[ei];
        end_head[ei] = handle;
    }

    Ok((start_head, end_head, next_by_start, next_by_end))
}

impl PackedIncidenceIndexImage {
    pub fn from_carrier(carrier: &PackedCarrierImage) -> Result<Self, StoreError> {
        let (start_head, end_head, next_by_start, next_by_end) =
            derive_packed_incidence_arrays(carrier)?;
        Ok(Self {
            schema_version: PACKED_INCIDENCE_INDEX_SCHEMA_VERSION,
            start_head,
            end_head,
            next_by_start,
            next_by_end,
        })
    }

    pub fn from_parts(
        schema_version: u32,
        carrier: &PackedCarrierImage,
        start_head: Vec<Handle>,
        end_head: Vec<Handle>,
        next_by_start: Vec<Handle>,
        next_by_end: Vec<Handle>,
    ) -> Result<Self, StoreError> {
        let image = Self {
            schema_version,
            start_head,
            end_head,
            next_by_start,
            next_by_end,
        };
        image.validate_against(carrier)?;
        Ok(image)
    }

    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub fn link_count(&self) -> usize {
        self.start_head.len().saturating_sub(1)
    }

    pub fn start_heads(&self) -> &[Handle] {
        &self.start_head
    }

    pub fn end_heads(&self) -> &[Handle] {
        &self.end_head
    }

    pub fn next_by_start(&self) -> &[Handle] {
        &self.next_by_start
    }

    pub fn next_by_end(&self) -> &[Handle] {
        &self.next_by_end
    }

    pub fn start_incidence(
        &self,
        start: Handle,
    ) -> Result<IncidenceIter<'_>, StoreError> {
        if start == NO_HANDLE || start as usize >= self.start_head.len() {
            return Err(StoreError::UnknownHandle(start));
        }
        Ok(IncidenceIter {
            next: &self.next_by_start,
            current: self.start_head[start as usize],
        })
    }

    pub fn end_incidence(
        &self,
        end: Handle,
    ) -> Result<IncidenceIter<'_>, StoreError> {
        if end == NO_HANDLE || end as usize >= self.end_head.len() {
            return Err(StoreError::UnknownHandle(end));
        }
        Ok(IncidenceIter {
            next: &self.next_by_end,
            current: self.end_head[end as usize],
        })
    }

    pub fn validate_against(
        &self,
        carrier: &PackedCarrierImage,
    ) -> Result<(), StoreError> {
        if self.schema_version != PACKED_INCIDENCE_INDEX_SCHEMA_VERSION {
            return Err(StoreError::UnsupportedPackedIncidenceIndexSchema(
                self.schema_version,
            ));
        }
        carrier.validate()?;
        let expected = carrier
            .link_count()
            .checked_add(1)
            .ok_or(StoreError::CapacityExceeded)?;
        if self.start_head.len() != expected
            || self.end_head.len() != expected
            || self.next_by_start.len() != expected
            || self.next_by_end.len() != expected
        {
            return Err(StoreError::PackedIncidenceIndexLengthMismatch {
                expected,
                start_head: self.start_head.len(),
                end_head: self.end_head.len(),
                next_by_start: self.next_by_start.len(),
                next_by_end: self.next_by_end.len(),
            });
        }

        let (start_head, end_head, next_by_start, next_by_end) =
            derive_packed_incidence_arrays(carrier)?;
        if self.start_head != start_head
            || self.end_head != end_head
            || self.next_by_start != next_by_start
            || self.next_by_end != next_by_end
        {
            return Err(StoreError::InvalidPackedIncidenceIndex);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackedExecutionView {
    carrier: PackedCarrierImage,
    incidence: PackedIncidenceIndexImage,
}

impl PackedExecutionView {
    pub fn from_carrier(
        carrier: PackedCarrierImage,
    ) -> Result<Self, StoreError> {
        carrier.validate()?;
        let incidence = PackedIncidenceIndexImage::from_carrier(&carrier)?;
        Ok(Self { carrier, incidence })
    }

    pub fn from_parts(
        carrier: PackedCarrierImage,
        incidence: PackedIncidenceIndexImage,
    ) -> Result<Self, StoreError> {
        carrier.validate()?;
        incidence.validate_against(&carrier)?;
        Ok(Self { carrier, incidence })
    }

    pub fn carrier(&self) -> &PackedCarrierImage {
        &self.carrier
    }

    pub fn incidence_index(&self) -> &PackedIncidenceIndexImage {
        &self.incidence
    }

    pub fn link_count(&self) -> usize {
        self.carrier.link_count()
    }

    pub fn is_valid(&self, handle: Handle) -> bool {
        handle > 0 && (handle as usize) <= self.link_count()
    }

    pub fn poles(
        &self,
        handle: Handle,
    ) -> Result<(Handle, Handle), StoreError> {
        self.carrier
            .duplet(handle)
            .ok_or(StoreError::UnknownHandle(handle))
    }

    pub fn start_incidence(
        &self,
        start: Handle,
    ) -> Result<IncidenceIter<'_>, StoreError> {
        self.incidence.start_incidence(start)
    }

    pub fn end_incidence(
        &self,
        end: Handle,
    ) -> Result<IncidenceIter<'_>, StoreError> {
        self.incidence.end_incidence(end)
    }
}

#[derive(Debug)]
pub struct OptimizedLinkStore {
    // Non-semantic runtime identity used only to scope executor caches. It is
    // never exported as Link/Anum identity.
    instance_id: u32,
    // Dense SoA carrier. Every index >=1 and <len is a valid local Link handle;
    // no per-record Option/discriminant is needed because this prototype does
    // not delete physical records in place.
    starts: Vec<Handle>,
    ends: Vec<Handle>,
    canonical_by_pair: HashMap<Pair, Handle>,
    start_forms: HashMap<Handle, Handle>,
    end_forms: HashMap<Handle, Handle>,
    // Dense intrusive incidence lists. Handles are dense local substrate IDs,
    // so per-handle arrays avoid one HashMap entry + Vec allocation per pole.
    start_head: Vec<Handle>,
    end_head: Vec<Handle>,
    next_by_start: Vec<Handle>,
    next_by_end: Vec<Handle>,
    max_links: Option<usize>,
}

impl Clone for OptimizedLinkStore {
    fn clone(&self) -> Self {
        Self {
            instance_id: next_store_instance_id(),
            starts: self.starts.clone(),
            ends: self.ends.clone(),
            canonical_by_pair: self.canonical_by_pair.clone(),
            start_forms: self.start_forms.clone(),
            end_forms: self.end_forms.clone(),
            start_head: self.start_head.clone(),
            end_head: self.end_head.clone(),
            next_by_start: self.next_by_start.clone(),
            next_by_end: self.next_by_end.clone(),
            max_links: self.max_links,
        }
    }
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
            instance_id: next_store_instance_id(),
            starts: vec![NO_HANDLE, ROOT_HANDLE],
            ends: vec![NO_HANDLE, ROOT_HANDLE],
            canonical_by_pair: HashMap::new(),
            start_forms: HashMap::new(),
            end_forms: HashMap::new(),
            start_head: vec![NO_HANDLE; 2],
            end_head: vec![NO_HANDLE; 2],
            next_by_start: vec![NO_HANDLE; 2],
            next_by_end: vec![NO_HANDLE; 2],
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
        self.starts.len() - 1
    }

    pub(crate) fn instance_id(&self) -> u32 {
        self.instance_id
    }

    pub fn is_valid(&self, handle: Handle) -> bool {
        handle > 0 && (handle as usize) < self.starts.len()
    }

    pub fn poles(&self, handle: Handle) -> Result<(Handle, Handle), StoreError> {
        if !self.is_valid(handle) {
            return Err(StoreError::UnknownHandle(handle));
        }
        let index = handle as usize;
        Ok((self.starts[index], self.ends[index]))
    }

    fn import_direct_recursive_wire_in_place(
        &mut self,
        source: &str,
    ) -> Result<Handle, StoreError> {
        if source.is_empty() {
            return Err(StoreError::EmptyAnum);
        }

        let bytes = source.as_bytes();
        let mut cursor = 0usize;
        let handle = self.parse_node(bytes, &mut cursor)?;
        if cursor != bytes.len() {
            return Err(StoreError::TrailingInput(cursor));
        }
        Ok(handle)
    }

    /// Transactionally import one direct-gauge technical recursive Link wire.
    ///
    /// This API names the representation it actually consumes. It is not
    /// semantic orientation authority; accepted-v0.14 semantic consumers use
    /// `orientation::SemanticOrientation::import_recursive_wire`.
    pub fn import_direct_recursive_wire(
        &mut self,
        source: &str,
    ) -> Result<Handle, StoreError> {
        // Whole-source import is transactional. Parsing/canonicalization happens
        // against a staging clone; only complete success replaces live state.
        let runtime_instance_id = self.instance_id;
        let mut staging = self.clone();
        staging.instance_id = runtime_instance_id;
        let handle = staging.import_direct_recursive_wire_in_place(source)?;
        *self = staging;
        Ok(handle)
    }

    /// Historical direct-gauge compatibility name.
    ///
    /// Accepted MTS v0.14 separates recursive Link wire from Anum/ExactSequence.
    /// Keep this wrapper only for existing callers while P5d migrates them.
    pub fn import_anum(&mut self, source: &str) -> Result<Handle, StoreError> {
        self.import_direct_recursive_wire(source)
    }

    /// Transactionally import direct-gauge recursive Link wires in source order.
    ///
    /// The whole batch uses one staging clone and is published atomically.
    /// Any malformed/capacity failure leaves the live store unchanged.
    pub fn import_direct_recursive_wires(
        &mut self,
        sources: &[String],
    ) -> Result<Vec<Handle>, StoreError> {
        if sources.is_empty() {
            return Ok(Vec::new());
        }

        let runtime_instance_id = self.instance_id;
        let mut staging = self.clone();
        staging.instance_id = runtime_instance_id;

        let mut handles = Vec::with_capacity(sources.len());
        for source in sources {
            handles.push(staging.import_direct_recursive_wire_in_place(source)?);
        }

        *self = staging;
        Ok(handles)
    }

    /// Historical batch compatibility name for direct recursive Link wires.
    pub fn import_anums(
        &mut self,
        sources: &[String],
    ) -> Result<Vec<Handle>, StoreError> {
        self.import_direct_recursive_wires(sources)
    }

    /// Freezes the current canonical store into a backend-neutral immutable
    /// carrier image. The image contains only dense local substrate topology;
    /// HashMap canonicalization state and executor caches are deliberately not
    /// part of the transport boundary.
    pub fn export_packed_carrier_image(&self) -> PackedCarrierImage {
        PackedCarrierImage::from_parts(
            PACKED_CARRIER_SCHEMA_VERSION,
            ROOT_HANDLE,
            self.starts[1..].to_vec(),
            self.ends[1..].to_vec(),
        )
        .expect("canonical store must always export a valid packed carrier")
    }

    /// Freezes the current start/end incidence projection independently from
    /// canonical HashMap identity. It is fully derivable from the immutable
    /// packed carrier and therefore carries no additional semantic authority.
    pub fn export_packed_incidence_index_image(
        &self,
    ) -> PackedIncidenceIndexImage {
        PackedIncidenceIndexImage::from_carrier(
            &self.export_packed_carrier_image(),
        )
        .expect("canonical store must always export a valid incidence index")
    }

    /// Projects the canonical store into a read-only execution boundary.
    ///
    /// The view deliberately contains no canonical HashMaps and exposes no Link
    /// construction/publication API. It is a snapshot of substrate topology +
    /// incidence indexes suitable for matching/discovery consumers.
    pub fn export_packed_execution_view(&self) -> PackedExecutionView {
        PackedExecutionView::from_carrier(self.export_packed_carrier_image())
            .expect("canonical store must always export a valid execution view")
    }

    /// Atomically loads a typed packed carrier image without parsing recursive
    /// structural wires. Compatibility reconstruction still rebuilds the CPU
    /// canonical/index state; C2 will separate that execution state further.
    pub fn load_packed_carrier_image(
        &mut self,
        image: &PackedCarrierImage,
    ) -> Result<(), StoreError> {
        image.validate()?;
        let duplets = image.duplets().collect::<Vec<_>>();
        self.load_packed_duplets(&duplets)
    }

    /// Exports the executable carrier as dense Link duplets in local-handle
    /// order. Entry `i - 1` is Link handle `i`.
    ///
    /// Compatibility projection. New carrier-boundary code should prefer
    /// `export_packed_carrier_image`.
    pub fn export_packed_duplets(&self) -> Vec<(Handle, Handle)> {
        (1..=self.link_count() as Handle)
            .map(|handle| {
                self.poles(handle)
                    .expect("dense packed carrier must contain every Link")
            })
            .collect()
    }

    /// Atomically replaces this store with a preassembled dense duplet carrier.
    ///
    /// The packed image is expected to be canonical and constructor ordered:
    /// each non-self pole must already refer to an earlier Link. This matches
    /// the runtime allocation invariant and permits direct CPU -> GPU style
    /// carrier transport without recursive Anum reconstruction.
    pub fn load_packed_duplets(
        &mut self,
        duplets: &[(Handle, Handle)],
    ) -> Result<(), StoreError> {
        if duplets.is_empty() || duplets[0] != (ROOT_HANDLE, ROOT_HANDLE) {
            let (start, end) = duplets.first().copied().unwrap_or((0, 0));
            return Err(StoreError::InvalidPackedCarrier {
                handle: ROOT_HANDLE,
                start,
                end,
            });
        }

        let runtime_instance_id = self.instance_id;
        let max_links = self.max_links;
        let mut staging = Self::with_optional_limit(max_links);
        staging.instance_id = runtime_instance_id;

        for (offset, &(start, end)) in duplets.iter().enumerate().skip(1) {
            let handle = Handle::try_from(offset + 1)
                .map_err(|_| StoreError::CapacityExceeded)?;

            let rebuilt = if start == handle {
                if end >= handle {
                    return Err(StoreError::InvalidPackedCarrier {
                        handle,
                        start,
                        end,
                    });
                }
                staging.ensure_start_form(end)?
            } else if end == handle {
                if start >= handle {
                    return Err(StoreError::InvalidPackedCarrier {
                        handle,
                        start,
                        end,
                    });
                }
                staging.ensure_end_form(start)?
            } else {
                if start >= handle || end >= handle {
                    return Err(StoreError::InvalidPackedCarrier {
                        handle,
                        start,
                        end,
                    });
                }
                staging.ensure_pair(start, end)?
            };

            if rebuilt != handle {
                return Err(StoreError::InvalidPackedCarrier {
                    handle,
                    start,
                    end,
                });
            }
        }

        *self = staging;
        Ok(())
    }

    /// Serialize one Link as a direct-gauge technical recursive Link wire.
    ///
    /// New accepted-v0.14 semantic consumers should use
    /// `orientation::SemanticOrientation::recursive_wire` so raw carrier pole
    /// order cannot become START_K/END_K authority.
    pub fn export_direct_recursive_wire(
        &self,
        handle: Handle,
    ) -> Result<String, StoreError> {
        let mut visiting = HashSet::new();
        let mut output = String::new();
        self.write_node(handle, &mut visiting, &mut output)?;
        Ok(output)
    }

    /// Historical direct-gauge compatibility name.
    ///
    /// This wrapper does not assert that an arbitrary recursive Link wire is an
    /// Anum/ExactSequence representation.
    pub fn export_anum(&self, handle: Handle) -> Result<String, StoreError> {
        self.export_direct_recursive_wire(handle)
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

    /// Canonical first-pole self-closed Link constructor in technical carrier coordinates.
    ///
    /// This historical direct-gauge helper remains for compatibility. Under
    /// accepted MTS v0.14 it is not semantic START_K authority by itself;
    /// semantic START_K/END_K construction goes through orientation::SemanticOrientation.
    pub fn ensure_start_self_closed(
        &mut self,
        child: Handle,
    ) -> Result<Handle, StoreError> {
        self.ensure_start_form(child)
    }

    /// Canonical second-pole self-closed Link constructor in technical carrier coordinates.
    ///
    /// This historical direct-gauge helper is substrate-facing under MTS v0.14;
    /// semantic END_K is derived by orientation::SemanticOrientation.
    pub fn ensure_end_self_closed(
        &mut self,
        child: Handle,
    ) -> Result<Handle, StoreError> {
        self.ensure_end_form(child)
    }

    pub fn start_incidence(&self, start: Handle) -> Result<IncidenceIter<'_>, StoreError> {
        self.require_valid(start)?;
        Ok(IncidenceIter {
            next: &self.next_by_start,
            current: self.start_head[start as usize],
        })
    }

    pub fn end_incidence(&self, end: Handle) -> Result<IncidenceIter<'_>, StoreError> {
        self.require_valid(end)?;
        Ok(IncidenceIter {
            next: &self.next_by_end,
            current: self.end_head[end as usize],
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
        let next = self.starts.len();
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
        self.starts.push(start);
        self.ends.push(end);
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
        self.starts.push(handle);
        self.ends.push(child);
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
        self.starts.push(child);
        self.ends.push(handle);
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
        }

        let hi = handle as usize;
        let si = start as usize;
        let ei = end as usize;

        self.next_by_start[hi] = self.start_head[si];
        self.start_head[si] = handle;

        self.next_by_end[hi] = self.end_head[ei];
        self.end_head[ei] = handle;
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

    fn write_node(
        &self,
        handle: Handle,
        visiting: &mut HashSet<Handle>,
        output: &mut String,
    ) -> Result<(), StoreError> {
        let (start, end) = self.poles(handle)?;

        if start == handle && end == handle {
            output.push('8');
            return Ok(());
        }

        if !visiting.insert(handle) {
            return Err(StoreError::NonWellFounded(handle));
        }

        if start == handle {
            output.push('9');
            self.write_node(end, visiting, output)?;
        } else if end == handle {
            output.push('6');
            self.write_node(start, visiting, output)?;
        } else {
            output.push('1');
            self.write_node(start, visiting, output)?;
            self.write_node(end, visiting, output)?;
        }

        visiting.remove(&handle);
        Ok(())
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
            .map(|handle| {
                store
                    .export_direct_recursive_wire(*handle)
                    .map_err(ReactionError::Store)
            })
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
    fn legacy_anum_api_is_exact_direct_recursive_wire_compatibility_facade() {
        let mut canonical = OptimizedLinkStore::new();
        let canonical_handle = canonical
            .import_direct_recursive_wire("19868")
            .unwrap();
        assert_eq!(
            canonical.export_direct_recursive_wire(canonical_handle).unwrap(),
            "19868"
        );

        let legacy_handle = canonical.import_anum("19868").unwrap();
        assert_eq!(legacy_handle, canonical_handle);
        assert_eq!(canonical.export_anum(legacy_handle).unwrap(), "19868");

        let sources = vec!["98".to_owned(), "68".to_owned(), "16898".to_owned()];
        let mut canonical_batch = OptimizedLinkStore::new();
        let mut legacy_batch = OptimizedLinkStore::new();
        let canonical_handles = canonical_batch
            .import_direct_recursive_wires(&sources)
            .unwrap();
        let legacy_handles = legacy_batch.import_anums(&sources).unwrap();
        assert_eq!(canonical_handles, legacy_handles);
        assert_eq!(
            canonical_batch.export_packed_duplets(),
            legacy_batch.export_packed_duplets()
        );
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
    fn batch_import_matches_sequential_import_and_is_atomic() {
        let sources = vec![
            "98".to_owned(),
            "68".to_owned(),
            "16898".to_owned(),
            "19868".to_owned(),
            "16816898".to_owned(),
        ];

        let mut sequential = OptimizedLinkStore::new();
        let sequential_handles = sources
            .iter()
            .map(|source| sequential.import_anum(source).unwrap())
            .collect::<Vec<_>>();

        let mut batched = OptimizedLinkStore::new();
        let instance_id = batched.instance_id;
        let batch_handles = batched.import_anums(&sources).unwrap();

        assert_eq!(batch_handles, sequential_handles);
        assert_eq!(batched.instance_id, instance_id);
        assert_eq!(batched.link_count(), sequential.link_count());
        for (source, handle) in sources.iter().zip(batch_handles.iter()) {
            assert_eq!(batched.export_anum(*handle).unwrap(), *source);
        }

        let stable = batched.import_anum("198698").unwrap();
        let before_count = batched.link_count();
        let before_stable = batched.export_anum(stable).unwrap();
        let failed = vec!["116898998".to_owned(), "19868x".to_owned()];
        assert!(batched.import_anums(&failed).is_err());
        assert_eq!(batched.instance_id, instance_id);
        assert_eq!(batched.link_count(), before_count);
        assert_eq!(batched.export_anum(stable).unwrap(), before_stable);
    }

    #[test]
    fn typed_packed_carrier_image_is_exact_and_backend_neutral() {
        let mut source = OptimizedLinkStore::new();
        let o = source.import_anum("98").unwrap();
        let c = source.import_anum("68").unwrap();
        let l = source.ensure_pair(o, c).unwrap();
        let _u = source.ensure_pair(c, o).unwrap();
        let _top = source.ensure_pair(l, ROOT_HANDLE).unwrap();

        let image = source.export_packed_carrier_image();
        assert_eq!(image.schema_version(), PACKED_CARRIER_SCHEMA_VERSION);
        assert_eq!(image.root_handle(), ROOT_HANDLE);
        assert_eq!(image.link_count(), source.link_count());
        assert_eq!(image.duplet(ROOT_HANDLE), Some((ROOT_HANDLE, ROOT_HANDLE)));
        assert_eq!(
            image.duplets().collect::<Vec<_>>(),
            source.export_packed_duplets()
        );
        assert_eq!(image.starts().len(), image.ends().len());

        let mut loaded = OptimizedLinkStore::new();
        let instance_id = loaded.instance_id;
        loaded.load_packed_carrier_image(&image).unwrap();

        assert_eq!(loaded.instance_id, instance_id);
        assert_eq!(loaded.export_packed_carrier_image(), image);
        for handle in 1..=source.link_count() as Handle {
            assert_eq!(
                loaded.export_anum(handle).unwrap(),
                source.export_anum(handle).unwrap()
            );
        }
    }

    #[test]
    fn typed_packed_carrier_image_rejects_invalid_transport_metadata() {
        assert_eq!(
            PackedCarrierImage::from_parts(
                PACKED_CARRIER_SCHEMA_VERSION + 1,
                ROOT_HANDLE,
                vec![ROOT_HANDLE],
                vec![ROOT_HANDLE],
            ),
            Err(StoreError::UnsupportedPackedCarrierSchema(
                PACKED_CARRIER_SCHEMA_VERSION + 1
            ))
        );
        assert_eq!(
            PackedCarrierImage::from_parts(
                PACKED_CARRIER_SCHEMA_VERSION,
                ROOT_HANDLE,
                vec![ROOT_HANDLE, ROOT_HANDLE],
                vec![ROOT_HANDLE],
            ),
            Err(StoreError::PackedCarrierLengthMismatch {
                starts: 2,
                ends: 1,
            })
        );
        assert_eq!(
            PackedCarrierImage::from_parts(
                PACKED_CARRIER_SCHEMA_VERSION,
                ROOT_HANDLE + 1,
                vec![ROOT_HANDLE],
                vec![ROOT_HANDLE],
            ),
            Err(StoreError::InvalidPackedCarrierRoot {
                expected: ROOT_HANDLE,
                actual: ROOT_HANDLE + 1,
            })
        );

        // Handle 2 and handle 3 cannot both be the same ordinary (ROOT,ROOT)
        // Link. The transport validator rejects the duplicate before loading.
        assert!(matches!(
            PackedCarrierImage::from_duplets(&[
                (ROOT_HANDLE, ROOT_HANDLE),
                (ROOT_HANDLE, ROOT_HANDLE),
                (ROOT_HANDLE, ROOT_HANDLE),
            ]),
            Err(StoreError::InvalidPackedCarrier {
                handle: 3,
                start: ROOT_HANDLE,
                end: ROOT_HANDLE,
            })
        ));

        // Forward references would leak backend construction order into an
        // invalid executable image and therefore fail closed.
        assert!(matches!(
            PackedCarrierImage::from_duplets(&[
                (ROOT_HANDLE, ROOT_HANDLE),
                (3, ROOT_HANDLE),
                (ROOT_HANDLE, ROOT_HANDLE),
            ]),
            Err(StoreError::InvalidPackedCarrier {
                handle: 2,
                start: 3,
                end: ROOT_HANDLE,
            })
        ));
    }

    #[test]
    fn packed_incidence_index_image_is_deterministic_and_exact() {
        let mut store = OptimizedLinkStore::new();
        let o = store.import_anum("98").unwrap();
        let c = store.import_anum("68").unwrap();
        let l = store.ensure_pair(o, c).unwrap();
        let _u = store.ensure_pair(c, o).unwrap();
        let _top = store.ensure_pair(l, ROOT_HANDLE).unwrap();

        let carrier = store.export_packed_carrier_image();
        let index = store.export_packed_incidence_index_image();
        let rebuilt = PackedIncidenceIndexImage::from_carrier(&carrier).unwrap();

        assert_eq!(index, rebuilt);
        assert_eq!(
            index.schema_version(),
            PACKED_INCIDENCE_INDEX_SCHEMA_VERSION
        );
        assert_eq!(index.link_count(), carrier.link_count());
        assert_eq!(index.start_heads().len(), carrier.link_count() + 1);
        assert_eq!(index.end_heads().len(), carrier.link_count() + 1);
        assert_eq!(index.next_by_start().len(), carrier.link_count() + 1);
        assert_eq!(index.next_by_end().len(), carrier.link_count() + 1);

        let round_trip = PackedIncidenceIndexImage::from_parts(
            index.schema_version(),
            &carrier,
            index.start_heads().to_vec(),
            index.end_heads().to_vec(),
            index.next_by_start().to_vec(),
            index.next_by_end().to_vec(),
        )
        .unwrap();
        assert_eq!(round_trip, index);

        let mut start_seen = vec![0u8; carrier.link_count() + 1];
        let mut end_seen = vec![0u8; carrier.link_count() + 1];
        for raw_pole in 1..=carrier.link_count() {
            let pole = raw_pole as Handle;
            let indexed_start = index.start_incidence(pole).unwrap().collect::<Vec<_>>();
            let store_start = store.start_incidence(pole).unwrap().collect::<Vec<_>>();
            assert_eq!(indexed_start, store_start);
            for handle in indexed_start {
                assert_eq!(carrier.duplet(handle).unwrap().0, pole);
                start_seen[handle as usize] += 1;
            }

            let indexed_end = index.end_incidence(pole).unwrap().collect::<Vec<_>>();
            let store_end = store.end_incidence(pole).unwrap().collect::<Vec<_>>();
            assert_eq!(indexed_end, store_end);
            for handle in indexed_end {
                assert_eq!(carrier.duplet(handle).unwrap().1, pole);
                end_seen[handle as usize] += 1;
            }
        }

        assert!(start_seen[1..].iter().all(|count| *count == 1));
        assert!(end_seen[1..].iter().all(|count| *count == 1));
    }

    #[test]
    fn packed_incidence_index_image_rejects_non_derivable_transport() {
        let mut store = OptimizedLinkStore::new();
        let o = store.import_anum("98").unwrap();
        let c = store.import_anum("68").unwrap();
        let _pair = store.ensure_pair(o, c).unwrap();

        let carrier = store.export_packed_carrier_image();
        let index = store.export_packed_incidence_index_image();

        let mut wrong_schema = index.clone();
        wrong_schema.schema_version += 1;
        assert_eq!(
            wrong_schema.validate_against(&carrier),
            Err(StoreError::UnsupportedPackedIncidenceIndexSchema(
                PACKED_INCIDENCE_INDEX_SCHEMA_VERSION + 1
            ))
        );

        let mut wrong_length = index.clone();
        wrong_length.next_by_end.pop();
        assert!(matches!(
            wrong_length.validate_against(&carrier),
            Err(StoreError::PackedIncidenceIndexLengthMismatch { .. })
        ));

        let mut wrong_chain = index;
        wrong_chain.start_head[ROOT_HANDLE as usize] = NO_HANDLE;
        assert_eq!(
            wrong_chain.validate_against(&carrier),
            Err(StoreError::InvalidPackedIncidenceIndex)
        );
    }

    #[test]
    fn packed_execution_view_matches_store_read_boundary_exactly() {
        let mut store = OptimizedLinkStore::new();
        let o = store.import_anum("98").unwrap();
        let c = store.import_anum("68").unwrap();
        let l = store.ensure_pair(o, c).unwrap();
        let _u = store.ensure_pair(c, o).unwrap();
        let _top = store.ensure_pair(l, ROOT_HANDLE).unwrap();

        let view = store.export_packed_execution_view();
        assert_eq!(view.link_count(), store.link_count());
        assert_eq!(
            view.carrier(),
            &store.export_packed_carrier_image()
        );
        assert_eq!(
            view.incidence_index(),
            &store.export_packed_incidence_index_image()
        );

        for raw_handle in 1..=store.link_count() {
            let handle = raw_handle as Handle;
            assert!(view.is_valid(handle));
            assert_eq!(view.poles(handle).unwrap(), store.poles(handle).unwrap());
            assert_eq!(
                view.start_incidence(handle).unwrap().collect::<Vec<_>>(),
                store.start_incidence(handle).unwrap().collect::<Vec<_>>()
            );
            assert_eq!(
                view.end_incidence(handle).unwrap().collect::<Vec<_>>(),
                store.end_incidence(handle).unwrap().collect::<Vec<_>>()
            );
        }
        assert!(!view.is_valid(NO_HANDLE));
        assert_eq!(
            view.poles(NO_HANDLE),
            Err(StoreError::UnknownHandle(NO_HANDLE))
        );
    }

    #[test]
    fn packed_execution_view_is_an_immutable_snapshot() {
        let mut store = OptimizedLinkStore::new();
        let o = store.import_anum("98").unwrap();
        let c = store.import_anum("68").unwrap();
        let view_before = store.export_packed_execution_view();

        let pair = store.ensure_pair(o, c).unwrap();
        let view_after = store.export_packed_execution_view();

        assert!(!view_before.is_valid(pair));
        assert!(view_after.is_valid(pair));
        assert_eq!(view_before.link_count() + 1, view_after.link_count());
        assert_eq!(
            view_before.poles(ROOT_HANDLE).unwrap(),
            (ROOT_HANDLE, ROOT_HANDLE)
        );
    }

    #[test]
    fn packed_execution_view_rejects_mismatched_index_projection() {
        let mut first = OptimizedLinkStore::new();
        let o = first.import_anum("98").unwrap();
        let c = first.import_anum("68").unwrap();
        let carrier_before = first.export_packed_carrier_image();
        let index_before = first.export_packed_incidence_index_image();

        let _pair = first.ensure_pair(o, c).unwrap();
        let carrier_after = first.export_packed_carrier_image();

        assert!(matches!(
            PackedExecutionView::from_parts(carrier_after, index_before),
            Err(StoreError::PackedIncidenceIndexLengthMismatch { .. })
                | Err(StoreError::InvalidPackedIncidenceIndex)
        ));

        let view = PackedExecutionView::from_carrier(carrier_before).unwrap();
        assert_eq!(view.link_count(), 3);
    }

    #[test]
    fn packed_duplet_carrier_rebuilds_exact_store_and_is_atomic() {
        let mut source = OptimizedLinkStore::new();
        let o = source.import_anum("98").unwrap();
        let c = source.import_anum("68").unwrap();
        let l = source.ensure_pair(o, c).unwrap();
        let _u = source.ensure_pair(c, o).unwrap();
        let _top = source.ensure_pair(l, ROOT_HANDLE).unwrap();

        let carrier = source.export_packed_duplets();

        let mut loaded = OptimizedLinkStore::new();
        let instance_id = loaded.instance_id;
        loaded.load_packed_duplets(&carrier).unwrap();

        assert_eq!(loaded.instance_id, instance_id);
        assert_eq!(loaded.link_count(), source.link_count());
        assert_eq!(loaded.export_packed_duplets(), carrier);
        for handle in 1..=source.link_count() as Handle {
            assert_eq!(
                loaded.export_anum(handle).unwrap(),
                source.export_anum(handle).unwrap()
            );
        }

        let stable = loaded.export_packed_duplets();
        let malformed = vec![
            (ROOT_HANDLE, ROOT_HANDLE),
            (ROOT_HANDLE, ROOT_HANDLE),
            (ROOT_HANDLE, ROOT_HANDLE),
        ];
        assert!(matches!(
            loaded.load_packed_duplets(&malformed),
            Err(StoreError::InvalidPackedCarrier { .. })
        ));
        assert_eq!(loaded.instance_id, instance_id);
        assert_eq!(loaded.export_packed_duplets(), stable);
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

        let from_k = store.start_incidence(k).unwrap().collect::<Vec<_>>();
        assert!(from_k.contains(&k)); // START(K child ROOT) is self-start incident.
        assert!(from_k.contains(&current));
        assert!(from_k.contains(&successor));

        let to_a = store.end_incidence(a).unwrap().collect::<Vec<_>>();
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
        store.starts.push(3); store.ends.push(1); // handle 2
        store.starts.push(2); store.ends.push(1); // handle 3

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

        // Full incidence count is intentionally O(k); count arrays are not kept.
        assert_eq!(
            store.start_incidence(ROOT_HANDLE).unwrap().count(),
            LINKS + 1
        );

        // Measure only O(1) entry into the intrusive list.
        let incidence_started = Instant::now();
        let mut observed = NO_HANDLE;
        for _ in 0..ITERS {
            observed = store
                .start_incidence(ROOT_HANDLE)
                .unwrap()
                .next()
                .unwrap_or(NO_HANDLE);
            black_box(observed);
        }
        let incidence_ns = incidence_started.elapsed().as_nanos() / ITERS;
        assert_ne!(observed, NO_HANDLE);

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
