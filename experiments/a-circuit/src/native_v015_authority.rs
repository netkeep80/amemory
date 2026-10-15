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

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(s: &mut OptimizedLinkStore, a: Handle, b: Handle) -> Handle {
        s.ensure_pair(a, b).unwrap()
    }
    fn atom(s: &mut OptimizedLinkStore, cursor: &mut Handle) -> Handle {
        *cursor = s.ensure_start_self_closed(*cursor).unwrap();
        *cursor
    }
    fn context(s: &mut OptimizedLinkStore, parent: Handle, current: Handle) -> Handle {
        let payload = pair(s, parent, current);
        s.ensure_start_self_closed(payload).unwrap()
    }
    fn rule(
        s: &mut OptimizedLinkStore,
        cursor: &mut Handle,
        theory: Handle,
        with_roles: bool,
    ) -> (Handle, Vec<Handle>) {
        let v = atom(s, cursor);
        let a = atom(s, cursor);
        let body = pair(s, a, ROOT_HANDLE); // accepted empty ExactSequence
        let rule = pair(s, v, body);
        let admission = pair(s, theory, rule);
        let roles = if with_roles {
            let x = atom(s, cursor);
            let y = atom(s, cursor);
            vec![pair(s, v, x), pair(s, v, y)]
        } else {
            Vec::new()
        };
        (admission, roles)
    }
    struct Fixture {
        store: OptimizedLinkStore,
        positive: BTreeSet<Handle>,
        c: Handle,
        e1: Handle,
        e2: Handle,
        k1: Handle,
        k2: Handle,
        truth1: Handle,
        edge1: Handle,
        zero: Handle,
        two: Handle,
        foreign: Handle,
        theory1: Handle,
        theory2: Handle,
    }
    fn fixture(noisy: bool, reverse_context_allocation: bool) -> Fixture {
        let mut s = OptimizedLinkStore::new();
        if noisy {
            let noise = pair(&mut s, ROOT_HANDLE, ROOT_HANDLE);
            s.ensure_start_self_closed(noise).unwrap();
        }
        let mut seed = ROOT_HANDLE;
        let c = s.ensure_end_self_closed(ROOT_HANDLE).unwrap();
        let theory1 = atom(&mut s, &mut seed);
        let theory2 = atom(&mut s, &mut seed);
        let e1 = pair(&mut s, c, theory1);
        let e2 = pair(&mut s, c, theory2);
        let truth1 = atom(&mut s, &mut seed);
        let truth2 = atom(&mut s, &mut seed);
        let (k1, k2) = if reverse_context_allocation {
            let k2 = context(&mut s, e2, truth2);
            let k1 = context(&mut s, e1, truth1);
            (k1, k2)
        } else {
            let k1 = context(&mut s, e1, truth1);
            let k2 = context(&mut s, e2, truth2);
            (k1, k2)
        };
        let edge1 = pair(&mut s, e1, k1);
        let edge2 = pair(&mut s, e2, k2);
        let (zero, _) = rule(&mut s, &mut seed, theory1, false);
        let (two, roles) = rule(&mut s, &mut seed, theory1, true);
        let foreign = pair(&mut s, theory2, poles(&s, two).unwrap().1);
        let mut positive = BTreeSet::from([
            e1, e2, k1, k2, truth1, truth2, edge1, edge2,
            zero, two, foreign,
        ]);
        positive.extend(roles);
        Fixture {
            store: s, positive, c, e1, e2, k1, k2, truth1,
            edge1, zero, two, foreign, theory1, theory2,
        }
    }
    fn reason(report: &ContextAuthorityInspection, reason: &str) -> bool {
        report.rejected_currentness.iter().any(|r| r.reason == reason)
    }

    #[test]
    fn a1_r3_component_context_theory_roles_multiple_current_and_read_only() {
        let f = fixture(false, false);
        let before = f.store.export_packed_duplets();
        let index = f.store.export_packed_incidence_index_image();
        let first = inspect(&f.store, &f.positive).unwrap();
        assert_eq!(first.gauge_boundary, Some(wire(&f.store, f.c).unwrap()));
        assert_eq!(first.active_contexts.len(), 2);
        assert!(first.rejected_currentness.is_empty());
        let a = first.active_contexts.iter()
            .find(|c| c.context == wire(&f.store, f.k1).unwrap()).unwrap();
        assert_eq!(a.environment, wire(&f.store, f.e1).unwrap());
        assert_eq!(a.theory, wire(&f.store, f.theory1).unwrap());
        assert_eq!(a.current_endpoint, wire(&f.store, f.truth1).unwrap());
        assert_eq!(a.positive_currentness_witness, wire(&f.store, f.edge1).unwrap());
        assert_eq!(a.ancestry, vec![wire(&f.store, f.k1).unwrap(), wire(&f.store, f.e1).unwrap()]);
        assert_eq!(a.authorized_rules.len(), 2);
        assert!(a.authorized_rules.iter().any(|r| r.roles.is_empty()
            && r.positive_admission == wire(&f.store, f.zero).unwrap()));
        assert!(a.authorized_rules.iter().any(|r| r.roles.len() == 2
            && r.positive_admission == wire(&f.store, f.two).unwrap()));
        assert!(!a.authorized_rules.iter().any(|r| r.positive_admission
            == wire(&f.store, f.foreign).unwrap()));
        let b = first.active_contexts.iter()
            .find(|c| c.context == wire(&f.store, f.k2).unwrap()).unwrap();
        assert_eq!(b.theory, wire(&f.store, f.theory2).unwrap());
        assert_eq!(b.authorized_rules.len(), 1);
        assert_eq!(b.authorized_rules[0].positive_admission, wire(&f.store, f.foreign).unwrap());
        assert_eq!(first, inspect(&f.store, &f.positive).unwrap());
        assert_eq!(before, f.store.export_packed_duplets());
        assert_eq!(index, f.store.export_packed_incidence_index_image());
        let alt = fixture(true, true);
        assert_eq!(first, inspect(&alt.store, &alt.positive).unwrap());
        assert_ne!(f.store.instance_id(), alt.store.instance_id());
        assert_ne!(before, alt.store.export_packed_duplets());
        println!("A1_R3_COMPONENT_CONTEXT_THEORY_RULE_AUTHORITY=PASS");
    }

    #[test]
    fn a1_r3_component_positive_currentness_and_foreign_falsifiers() {
        let f = fixture(false, false);
        for (remove, expected) in [
            (f.e1, "ENVIRONMENT_NOT_MEMBER"),
            (f.k1, "CONTEXT_NOT_MEMBER"),
            (f.truth1, "CURRENT_ENDPOINT_NOT_MEMBER"),
        ] {
            let mut pos = f.positive.clone();
            pos.remove(&remove);
            let out = inspect(&f.store, &pos).unwrap();
            assert_eq!(out.active_contexts.len(), 1);
            assert!(reason(&out, expected), "{expected}");
        }
        let mut no_edge = f.positive.clone();
        no_edge.remove(&f.edge1);
        let out = inspect(&f.store, &no_edge).unwrap();
        assert_eq!(out.active_contexts.len(), 1);
        // Physical E->K remains, but is never promoted to authority.
        assert!(!out.active_contexts.iter().any(|a| a.context == wire(&f.store, f.k1).unwrap()));
        let mut clone = f.store.clone();
        let rogue = pair(&mut clone, f.e2, f.k1);
        assert_eq!(inspect(&clone, &f.positive).unwrap(), inspect(&f.store, &f.positive).unwrap());
        let mut pos = f.positive.clone();
        pos.insert(rogue);
        let conflict = inspect(&clone, &pos).unwrap();
        assert_eq!(conflict.active_contexts.len(), 1);
        assert!(reason(&conflict, "CONFLICTING_THEORY_CURRENTNESS"));
        // A positively witnessed foreign environment without a legitimate
        // ancestor is rejected, even if the intended E->K edge is absent.
        pos.remove(&f.edge1);
        let wrong = inspect(&clone, &pos).unwrap();
        assert!(reason(&wrong, "FOREIGN_ENVIRONMENT_ANCESTRY"));
        assert_eq!(wrong.active_contexts.len(), 1);
        let no_gauge = inspect(&OptimizedLinkStore::new(), &BTreeSet::new()).unwrap();
        assert!(no_gauge.gauge_boundary.is_none());
        assert!(no_gauge.active_contexts.is_empty());
        println!("A1_R3_COMPONENT_POSITIVE_AUTHORITY_FALSIFIERS=PASS");
    }

    #[test]
    fn a1_r3_component_foreign_admission_ancestry_and_freeze() {
        let f = fixture(false, false);
        let mut s = f.store.clone();
        let mut pos = f.positive.clone();
        let initial = inspect(&s, &pos).unwrap();
        // Future physical admission cannot enter reaction-start authority.
        let rule = poles(&s, f.two).unwrap().1;
        let unrelated = s.ensure_start_self_closed(rule).unwrap();
        let physical_only = pair(&mut s, f.theory1, unrelated);
        assert!(!pos.contains(&physical_only));
        assert_eq!(inspect(&s, &pos).unwrap(), initial);
        // A new positive admission belongs only to a DIFFERENT future state.
        let before_snapshot = pos.clone();
        pos.insert(physical_only);
        assert!(!before_snapshot.contains(&physical_only));
        assert_eq!(inspect(&s, &before_snapshot).unwrap(), initial);
        // Replacing a RoleBundle anchor gives a different Rule Link, and
        // its physical admission cannot inherit authority from the original.
        assert!(!initial.active_contexts[0].authorized_rules.iter().any(|r|
            r.positive_admission == wire(&s, physical_only).unwrap()));
        // Nested Context ancestry, with only leaf K current, is permitted.
        let mut nested_members = f.positive.clone();
        nested_members.remove(&f.edge1);
        let nested_truth = s.ensure_start_self_closed(f.truth1).unwrap();
        let nested_k = context(&mut s, f.k1, nested_truth);
        let nested_edge = pair(&mut s, f.e1, nested_k);
        nested_members.extend([nested_truth, nested_k, nested_edge]);
        let nested = inspect(&s, &nested_members).unwrap();
        let leaf = nested.active_contexts.iter()
            .find(|a| a.context == wire(&s, nested_k).unwrap()).unwrap();
        assert_eq!(leaf.ancestry.len(), 3);
        assert_eq!(leaf.ancestry[2], wire(&s, f.e1).unwrap());
        let packed = s.export_packed_duplets();
        let mut resumed = OptimizedLinkStore::new();
        resumed.load_packed_duplets(&packed).unwrap();
        // Component-only carrier restore: independent semantic member set is
        // retained in this test; not a new production current-pointer format.
        assert_ne!(s.instance_id(), resumed.instance_id());
        assert_eq!(inspect(&s, &nested_members).unwrap(),
            inspect(&resumed, &nested_members).unwrap());
        println!("A1_R3_COMPONENT_SNAPSHOT_AND_FREEZE_PORTABILITY=PASS");
    }
}
