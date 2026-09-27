use std::collections::HashSet;

use crate::{Handle, OptimizedLinkStore, StoreError, ROOT_HANDLE};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GaugeTransport {
    Id,
    J,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OrientationError {
    Store(StoreError),
    InvalidOneSidedMarker(Handle),
    EmptyRecursiveWire,
    InvalidRecursiveToken(char),
    TruncatedRecursiveWire,
    TrailingRecursiveInput(usize),
    ForeignStore {
        expected_instance_id: u32,
        actual_instance_id: u32,
    },
}

impl From<StoreError> for OrientationError {
    fn from(value: StoreError) -> Self {
        Self::Store(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MarkerObservation {
    body: Handle,
    first_pole_self_closed: bool,
}

fn observe_one_sided_marker(
    store: &OptimizedLinkStore,
    marker: Handle,
) -> Result<MarkerObservation, OrientationError> {
    let (first, second) = store.poles(marker)?;

    let first_self_closed = first == marker && second != marker;
    let second_self_closed = second == marker && first != marker;
    if first_self_closed == second_self_closed {
        return Err(OrientationError::InvalidOneSidedMarker(marker));
    }

    Ok(MarkerObservation {
        body: if first_self_closed { second } else { first },
        first_pole_self_closed: first_self_closed,
    })
}

/// Context-relative semantic orientation derived from Link-native one-sided
/// self-incidence markers.
///
/// The carrier keeps its technical ordered coordinates. This value only
/// supplies the semantic view that interprets those coordinates relative to
/// two actual structural markers. Callers cannot select Id/J directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SemanticOrientation {
    store_instance_id: u32,
    reference_marker: Handle,
    reference_body: Handle,
    context_marker: Handle,
    context_body: Handle,
    transport: GaugeTransport,
}

impl SemanticOrientation {
    /// Derive the relative gauge between two proper one-sided Link markers.
    ///
    /// Same technical self-incidence class gives Id; the opposite class gives
    /// J. The markers, not a host bool/enum, are the orientation authority.
    pub fn between(
        store: &OptimizedLinkStore,
        reference_marker: Handle,
        context_marker: Handle,
    ) -> Result<Self, OrientationError> {
        let reference = observe_one_sided_marker(store, reference_marker)?;
        let context = observe_one_sided_marker(store, context_marker)?;
        let transport =
            if reference.first_pole_self_closed == context.first_pole_self_closed {
                GaugeTransport::Id
            } else {
                GaugeTransport::J
            };

        Ok(Self {
            store_instance_id: store.instance_id(),
            reference_marker,
            reference_body: reference.body,
            context_marker,
            context_body: context.body,
            transport,
        })
    }

    pub fn transport(&self) -> GaugeTransport {
        self.transport
    }

    pub fn reference_marker(&self) -> Handle {
        self.reference_marker
    }

    pub fn reference_body(&self) -> Handle {
        self.reference_body
    }

    pub fn context_marker(&self) -> Handle {
        self.context_marker
    }

    pub fn context_body(&self) -> Handle {
        self.context_body
    }

    fn require_store(&self, store: &OptimizedLinkStore) -> Result<(), OrientationError> {
        let actual = store.instance_id();
        if actual != self.store_instance_id {
            return Err(OrientationError::ForeignStore {
                expected_instance_id: self.store_instance_id,
                actual_instance_id: actual,
            });
        }
        Ok(())
    }

    /// Read semantic START_K/END_K pole positions without rewriting carrier
    /// duplets. J is implemented as the accepted pole swap.
    pub fn poles(
        &self,
        store: &OptimizedLinkStore,
        link: Handle,
    ) -> Result<(Handle, Handle), OrientationError> {
        self.require_store(store)?;
        let (first, second) = store.poles(link)?;
        Ok(match self.transport {
            GaugeTransport::Id => (first, second),
            GaugeTransport::J => (second, first),
        })
    }

    /// Form one semantic Link. Under J the physical carrier arguments are
    /// swapped so the semantic equation remains covariant.
    pub fn ensure(
        &self,
        store: &mut OptimizedLinkStore,
        start: Handle,
        end: Handle,
    ) -> Result<Handle, OrientationError> {
        self.require_store(store)?;
        Ok(match self.transport {
            GaugeTransport::Id => store.ensure_pair(start, end)?,
            GaugeTransport::J => store.ensure_pair(end, start)?,
        })
    }

    pub fn ensure_start_self_closed(
        &self,
        store: &mut OptimizedLinkStore,
        body: Handle,
    ) -> Result<Handle, OrientationError> {
        self.require_store(store)?;
        Ok(match self.transport {
            GaugeTransport::Id => store.ensure_start_self_closed(body)?,
            GaugeTransport::J => store.ensure_end_self_closed(body)?,
        })
    }

    pub fn ensure_end_self_closed(
        &self,
        store: &mut OptimizedLinkStore,
        body: Handle,
    ) -> Result<Handle, OrientationError> {
        self.require_store(store)?;
        Ok(match self.transport {
            GaugeTransport::Id => store.ensure_end_self_closed(body)?,
            GaugeTransport::J => store.ensure_start_self_closed(body)?,
        })
    }

    /// Transactionally materialize one semantic recursive Link wire through
    /// this context-relative orientation.
    ///
    /// The live store is replaced only after the complete source parses and
    /// materializes successfully. Under J, 9/6 and ordinary pair construction
    /// automatically use the opposite technical carrier representatives.
    pub fn import_recursive_wire(
        &self,
        store: &mut OptimizedLinkStore,
        source: &str,
    ) -> Result<Handle, OrientationError> {
        self.require_store(store)?;
        if source.is_empty() {
            return Err(OrientationError::EmptyRecursiveWire);
        }

        let runtime_instance_id = store.instance_id();
        let mut staging = store.clone();
        // A transactional staging clone is still this logical A-memory. Keep
        // the runtime identity so this already-derived Link-native orientation
        // remains valid throughout atomic materialization.
        staging.instance_id = runtime_instance_id;

        let bytes = source.as_bytes();
        let mut cursor = 0usize;
        let handle = self.parse_recursive_wire_node(&mut staging, bytes, &mut cursor)?;
        if cursor != bytes.len() {
            return Err(OrientationError::TrailingRecursiveInput(cursor));
        }

        *store = staging;
        Ok(handle)
    }

    fn parse_recursive_wire_node(
        &self,
        store: &mut OptimizedLinkStore,
        bytes: &[u8],
        cursor: &mut usize,
    ) -> Result<Handle, OrientationError> {
        if *cursor >= bytes.len() {
            return Err(OrientationError::TruncatedRecursiveWire);
        }

        let token = bytes[*cursor] as char;
        *cursor += 1;

        match token {
            '8' => Ok(ROOT_HANDLE),
            '9' => {
                let child = self.parse_recursive_wire_node(store, bytes, cursor)?;
                self.ensure_start_self_closed(store, child)
            }
            '6' => {
                let child = self.parse_recursive_wire_node(store, bytes, cursor)?;
                self.ensure_end_self_closed(store, child)
            }
            '1' => {
                let start = self.parse_recursive_wire_node(store, bytes, cursor)?;
                let end = self.parse_recursive_wire_node(store, bytes, cursor)?;
                self.ensure(store, start, end)
            }
            other => Err(OrientationError::InvalidRecursiveToken(other)),
        }
    }

    /// Serialize recursive Link structure in semantic orientation.
    ///
    /// This is deliberately named "recursive wire", not Anum: accepted MTS
    /// v0.14 separates the recursive Link codec from Anum/ExactSequence.
    pub fn recursive_wire(
        &self,
        store: &OptimizedLinkStore,
        link: Handle,
    ) -> Result<String, OrientationError> {
        self.require_store(store)?;
        let mut active = HashSet::new();
        let mut output = String::new();
        self.write_recursive_wire(store, link, &mut active, &mut output)?;
        Ok(output)
    }

    fn write_recursive_wire(
        &self,
        store: &OptimizedLinkStore,
        link: Handle,
        active: &mut HashSet<Handle>,
        output: &mut String,
    ) -> Result<(), OrientationError> {
        let (start, end) = self.poles(store, link)?;

        if start == link && end == link {
            output.push('8');
            return Ok(());
        }

        if !active.insert(link) {
            return Err(StoreError::NonWellFounded(link).into());
        }

        if start == link {
            output.push('9');
            self.write_recursive_wire(store, end, active, output)?;
        } else if end == link {
            output.push('6');
            self.write_recursive_wire(store, start, active, output)?;
        } else {
            output.push('1');
            self.write_recursive_wire(store, start, active, output)?;
            self.write_recursive_wire(store, end, active, output)?;
        }

        active.remove(&link);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ROOT_HANDLE;

    #[test]
    fn direct_and_mirror_views_preserve_semantic_foundation_without_carrier_rewrite() {
        let mut store = OptimizedLinkStore::new();

        // These two calls create the objective one-sided orbit using historical
        // technical helpers. Their names are not semantic orientation authority.
        let technical_first = store.ensure_start_self_closed(ROOT_HANDLE).unwrap();
        let technical_second = store.ensure_end_self_closed(ROOT_HANDLE).unwrap();

        let direct =
            SemanticOrientation::between(&store, technical_first, technical_first).unwrap();
        let mirror =
            SemanticOrientation::between(&store, technical_first, technical_second).unwrap();

        assert_eq!(direct.transport(), GaugeTransport::Id);
        assert_eq!(mirror.transport(), GaugeTransport::J);

        let d_o = direct.ensure_start_self_closed(&mut store, ROOT_HANDLE).unwrap();
        let d_c = direct.ensure_end_self_closed(&mut store, ROOT_HANDLE).unwrap();
        let d_l = direct.ensure(&mut store, d_o, d_c).unwrap();
        let d_u = direct.ensure(&mut store, d_c, d_o).unwrap();

        let m_o = mirror.ensure_start_self_closed(&mut store, ROOT_HANDLE).unwrap();
        let m_c = mirror.ensure_end_self_closed(&mut store, ROOT_HANDLE).unwrap();
        let m_l = mirror.ensure(&mut store, m_o, m_c).unwrap();
        let m_u = mirror.ensure(&mut store, m_c, m_o).unwrap();

        assert_eq!(d_o, technical_first);
        assert_eq!(d_c, technical_second);
        assert_eq!(m_o, technical_second);
        assert_eq!(m_c, technical_first);
        assert_eq!(m_l, d_l);
        assert_eq!(m_u, d_u);

        let carrier_before_reads = store.export_packed_duplets();

        for (orientation, foundation) in [
            (direct, [ROOT_HANDLE, d_o, d_c, d_l, d_u]),
            (mirror, [ROOT_HANDLE, m_o, m_c, m_l, m_u]),
        ] {
            let expected = ["8", "98", "68", "19868", "16898"];
            for (link, wire) in foundation.into_iter().zip(expected) {
                assert_eq!(orientation.recursive_wire(&store, link).unwrap(), wire);
            }
        }

        // Raw direct-gauge technical serialization sees the opposite spellings
        // for mirror O/C. Semantic recursive wires stay gauge invariant.
        assert_eq!(store.export_anum(m_o).unwrap(), "68");
        assert_eq!(store.export_anum(m_c).unwrap(), "98");
        assert_eq!(mirror.recursive_wire(&store, m_o).unwrap(), "98");
        assert_eq!(mirror.recursive_wire(&store, m_c).unwrap(), "68");

        assert_eq!(store.export_packed_duplets(), carrier_before_reads);
    }

    #[test]
    fn marker_chirality_derives_relative_id_or_j_for_arbitrary_context_body() {
        let mut store = OptimizedLinkStore::new();
        let reference = store.ensure_start_self_closed(ROOT_HANDLE).unwrap();
        let root_mirror = store.ensure_end_self_closed(ROOT_HANDLE).unwrap();

        let body = store.ensure_pair(reference, root_mirror).unwrap();
        let same_class = store.ensure_start_self_closed(body).unwrap();
        let opposite_class = store.ensure_end_self_closed(body).unwrap();

        let id = SemanticOrientation::between(&store, reference, same_class).unwrap();
        let j = SemanticOrientation::between(&store, reference, opposite_class).unwrap();

        assert_eq!(id.transport(), GaugeTransport::Id);
        assert_eq!(j.transport(), GaugeTransport::J);
        assert_eq!(id.reference_body(), ROOT_HANDLE);
        assert_eq!(id.context_body(), body);
        assert_eq!(j.context_body(), body);
    }

    #[test]
    fn semantic_recursive_import_is_atomic_and_gauge_covariant() {
        let mut store = OptimizedLinkStore::new();
        let first_marker = store.ensure_start_self_closed(ROOT_HANDLE).unwrap();
        let second_marker = store.ensure_end_self_closed(ROOT_HANDLE).unwrap();

        let direct =
            SemanticOrientation::between(&store, first_marker, first_marker).unwrap();
        let mirror =
            SemanticOrientation::between(&store, first_marker, second_marker).unwrap();

        let store_id = store.instance_id();

        // Direct semantic codec is exactly the historical direct-gauge spelling.
        let direct_l = direct.import_recursive_wire(&mut store, "19868").unwrap();
        let legacy_l = store.import_anum("19868").unwrap();
        assert_eq!(direct_l, legacy_l);
        assert_eq!(direct.recursive_wire(&store, direct_l).unwrap(), "19868");
        assert_eq!(store.export_anum(direct_l).unwrap(), "19868");

        // Mirror O/C use opposite physical representatives while retaining the
        // same semantic recursive alphabet.
        let mirror_o = mirror.import_recursive_wire(&mut store, "98").unwrap();
        let mirror_c = mirror.import_recursive_wire(&mut store, "68").unwrap();
        assert_eq!(store.export_anum(mirror_o).unwrap(), "68");
        assert_eq!(store.export_anum(mirror_c).unwrap(), "98");
        assert_eq!(mirror.recursive_wire(&store, mirror_o).unwrap(), "98");
        assert_eq!(mirror.recursive_wire(&store, mirror_c).unwrap(), "68");

        // Ordinary semantic composition also commutes with J.
        let mirror_l = mirror.import_recursive_wire(&mut store, "19868").unwrap();
        let mirror_u = mirror.import_recursive_wire(&mut store, "16898").unwrap();
        assert_eq!(mirror_l, direct_l);
        assert_eq!(mirror.recursive_wire(&store, mirror_l).unwrap(), "19868");
        assert_eq!(mirror.recursive_wire(&store, mirror_u).unwrap(), "16898");

        let nested = mirror
            .import_recursive_wire(&mut store, "198998")
            .unwrap();
        assert_eq!(mirror.recursive_wire(&store, nested).unwrap(), "198998");
        assert_ne!(store.export_anum(nested).unwrap(), "198998");

        assert_eq!(store.instance_id(), store_id);

        // A malformed source may mutate staging while parsing, but can never
        // partially publish to the live A-memory.
        let before = store.export_packed_duplets();
        assert!(matches!(
            mirror.import_recursive_wire(&mut store, "1986x"),
            Err(OrientationError::InvalidRecursiveToken('x'))
        ));
        assert_eq!(store.export_packed_duplets(), before);
        assert_eq!(store.instance_id(), store_id);
    }

    #[test]
    fn semantic_recursive_import_rejects_foreign_memory_before_staging() {
        let mut source = OptimizedLinkStore::new();
        let marker = source.ensure_start_self_closed(ROOT_HANDLE).unwrap();
        let orientation = SemanticOrientation::between(&source, marker, marker).unwrap();

        let mut foreign = OptimizedLinkStore::new();
        let before = foreign.export_packed_duplets();
        assert!(matches!(
            orientation.import_recursive_wire(&mut foreign, "98"),
            Err(OrientationError::ForeignStore { .. })
        ));
        assert_eq!(foreign.export_packed_duplets(), before);
    }

    #[test]
    fn root_ordinary_pair_and_foreign_store_cannot_be_orientation_authority() {
        let mut store = OptimizedLinkStore::new();
        let reference = store.ensure_start_self_closed(ROOT_HANDLE).unwrap();
        let other = store.ensure_end_self_closed(ROOT_HANDLE).unwrap();
        let ordinary = store.ensure_pair(reference, other).unwrap();

        assert_eq!(
            SemanticOrientation::between(&store, reference, ROOT_HANDLE),
            Err(OrientationError::InvalidOneSidedMarker(ROOT_HANDLE)),
        );
        assert_eq!(
            SemanticOrientation::between(&store, reference, ordinary),
            Err(OrientationError::InvalidOneSidedMarker(ordinary)),
        );

        let orientation = SemanticOrientation::between(&store, reference, reference).unwrap();
        let foreign = OptimizedLinkStore::new();
        assert!(matches!(
            orientation.poles(&foreign, ROOT_HANDLE),
            Err(OrientationError::ForeignStore { .. })
        ));
    }
}
