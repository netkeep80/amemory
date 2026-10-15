//! A1-R3: positive-membership-only Context/Theory and Rule authority reader.
//! COMPONENT evidence until an Author-approved full launch-state source exists.
//! No authority setters, matcher, runtime state transition or Gamma.
use super::native_v015_role_bundle::{inspect as inspect_roles, RoleBundleCandidate};
use amemory_optimized_cpu_probe::{Handle, OptimizedLinkStore, ROOT_HANDLE};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveContextAuthority {
    pub gauge_boundary: String,
    pub environment: String,
    pub context: String,
    pub ancestry: Vec<String>,
    pub current_endpoint: String,
    pub theory: String,
    pub positive_currentness_witness: String,
    pub authorized_rules: Vec<RoleBundleCandidate>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RejectedCurrentness {
    pub witness: String,
    pub reason: &'static str,
}

/// An inspection is only a reaction-start *authority candidate* view, not
/// Rule applicability or a Gamma execution plan. No physical handles escape.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextAuthorityInspection {
    pub gauge_boundary: Option<String>,
    pub active_contexts: Vec<ActiveContextAuthority>,
    pub rejected_currentness: Vec<RejectedCurrentness>,
}

fn poles(store: &OptimizedLinkStore, h: Handle) -> Result<(Handle, Handle), ()> {
    store.poles(h).map_err(|_| ())
}
fn wire(store: &OptimizedLinkStore, h: Handle) -> Result<String, ()> {
    store.export_direct_recursive_wire(h).map_err(|_| ())
}

/// Accepted historical A8/A9a Context carrier: K = START(parent -> current).
/// START(ROOT) is not an environment; ancestry must terminate at an ordinary E.
fn read_context(store: &OptimizedLinkStore, k: Handle) -> Result<Option<(Handle, Handle)>, ()> {
    let (start, payload) = poles(store, k)?;
    if start != k || payload == k {
        return Ok(None);
    }
    Ok(Some(poles(store, payload)?))
}

/// Return [K, parent Contexts ..., E] or reject a cycle/malformed ancestry.
fn ancestry(store: &OptimizedLinkStore, context: Handle) -> Result<Vec<Handle>, ()> {
    let mut seen = BTreeSet::new();
    let mut chain = Vec::new();
    let mut cursor = context;
    loop {
        if !seen.insert(cursor) {
            return Err(());
        }
        chain.push(cursor);
        match read_context(store, cursor)? {
            Some((parent, _)) => cursor = parent,
            None => return Ok(chain),
        }
    }
}

/// Internal-only component algorithm. In production the Session provides the
/// immutable members of its own `LoadedSemanticAnet` after Store epoch fencing.
pub(crate) fn inspect(
    store: &OptimizedLinkStore,
    positive: &BTreeSet<Handle>,
) -> Result<ContextAuthorityInspection, ()> {
    // Discover direct C=END(R) without ensure_* or host gauge configuration.
    let mut gauge = None;
    for (index, &(start, end)) in store.export_packed_duplets().iter().enumerate() {
        let h = index as Handle + 1;
        if h != ROOT_HANDLE && start == ROOT_HANDLE && end == h {
            if gauge.replace(h).is_some() {
                return Err(()); // ambiguous gauge: never choose smallest Handle
            }
        }
    }
    let mut result = ContextAuthorityInspection {
        gauge_boundary: gauge.map(|h| wire(store, h)).transpose()?,
        active_contexts: Vec::new(),
        rejected_currentness: Vec::new(),
    };
    let Some(boundary) = gauge else {
        return Ok(result); // no authorized launch-state can be derived
    };
    // Only positive witnesses are considered. Store incidence alone cannot
    // mint E->K currentness, including after additional physical allocations.
    let mut claims: BTreeMap<Handle, BTreeSet<Handle>> = BTreeMap::new();
    for &edge in positive {
        let (env, context) = poles(store, edge)?;
        if read_context(store, context)?.is_none() {
            continue;
        }
        let (start, theory) = poles(store, env)?;
        if start == boundary && theory != env && positive.contains(&env) {
            claims.entry(context).or_default().insert(env);
        }
    }
    let roles = inspect_roles(store, positive)?;
    for &edge in positive {
        let (env, context) = poles(store, edge)?;
        let Some((_, current)) = read_context(store, context)? else {
            continue;
        };
        let (start, theory) = poles(store, env)?;
        if start != boundary || theory == env {
            continue; // not a C->Theory candidate
        }
        let reason = if !positive.contains(&env) {
            Some("ENVIRONMENT_NOT_MEMBER")
        } else if !positive.contains(&context) {
            Some("CONTEXT_NOT_MEMBER")
        } else if !positive.contains(&current) {
            Some("CURRENT_ENDPOINT_NOT_MEMBER")
        } else if claims.get(&context).is_some_and(|v| v.len() > 1) {
            Some("CONFLICTING_THEORY_CURRENTNESS")
        } else {
            None
        };
        let chain = ancestry(store, context);
        let reason = reason.or_else(|| match &chain {
            Ok(chain) if chain.last() == Some(&env) => None,
            Ok(_) => Some("FOREIGN_ENVIRONMENT_ANCESTRY"),
            Err(_) => Some("INVALID_CONTEXT_ANCESTRY"),
        });
        if let Some(reason) = reason {
            result.rejected_currentness.push(RejectedCurrentness {
                witness: wire(store, edge)?,
                reason,
            });
            continue;
        }
        let theory_wire = wire(store, theory)?;
        let mut authorized_rules: Vec<_> = roles
            .candidates
            .iter()
            .filter(|r| r.theory_pole == theory_wire)
            .cloned()
            .collect();
        authorized_rules.sort_by(|a, b| a.positive_admission.cmp(&b.positive_admission));
        result.active_contexts.push(ActiveContextAuthority {
            gauge_boundary: wire(store, boundary)?,
            environment: wire(store, env)?,
            context: wire(store, context)?,
            ancestry: chain?.into_iter().map(|h| wire(store, h)).collect::<Result<_, _>>()?,
            current_endpoint: wire(store, current)?,
            theory: theory_wire,
            positive_currentness_witness: wire(store, edge)?,
            authorized_rules,
        });
    }
    result.active_contexts.sort_by(|a, b| {
        a.context.cmp(&b.context).then(a.positive_currentness_witness.cmp(&b.positive_currentness_witness))
    });
    result.rejected_currentness.sort_by(|a, b| {
        a.witness.cmp(&b.witness).then(a.reason.cmp(b.reason))
    });
    Ok(result)
}
