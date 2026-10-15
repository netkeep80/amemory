//! A1-3: read-only structural RoleBundle *candidate* inspection.
//! Positive membership is mandatory; A9b V->Body layout is research evidence,
//! not a new normative rule format. No Theory selection, Gamma, or legacy path.
//! Canonical recursive wires, never raw local handles, are portable.
use amemory_optimized_cpu_probe::{Handle, OptimizedLinkStore, ROOT_HANDLE};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoleBundleCandidate {
    pub positive_admission: String,
    pub theory_pole: String,
    pub rule: String,
    pub role_bundle_anchor: String,
    pub roles: Vec<String>,
    pub antecedent_template: String,
    pub output_sequence_carrier: String,
    pub output_templates: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoleBundleInspection {
    pub candidates: Vec<RoleBundleCandidate>,
    pub unclassified_positive_members: Vec<String>,
    pub malformed_sequence_members: Vec<String>,
    pub ambiguous_members: Vec<String>,
}

fn poles(s: &OptimizedLinkStore, h: Handle) -> Result<(Handle, Handle), ()> {
    s.poles(h).map_err(|_| ())
}

fn wire(s: &OptimizedLinkStore, h: Handle) -> Result<String, ()> {
    s.export_direct_recursive_wire(h).map_err(|_| ())
}

/// Existing ExactSequence carrier: empty=ROOT; Cell(prev,value)=START(prev->value).
fn exact_sequence(s: &OptimizedLinkStore, end: Handle) -> Result<Vec<Handle>, ()> {
    let mut current = end;
    let mut visited = BTreeSet::new();
    let mut reversed = Vec::new();
    while current != ROOT_HANDLE {
        if !visited.insert(current) {
            return Err(());
        }
        let (a, payload) = poles(s, current)?;
        if a != current || payload == current {
            return Err(());
        }
        let (prev, value) = poles(s, payload)?;
        reversed.push(value);
        current = prev;
    }
    reversed.reverse();
    Ok(reversed)
}

/// Internal structural classifier. The Session is the only production caller
/// and passes members from its bound, independently source-verified ANet.
/// Component tests may supply their own membership without minting a receipt.
pub(crate) fn inspect(
    s: &OptimizedLinkStore,
    positive: &BTreeSet<Handle>,
) -> Result<RoleBundleInspection, ()> {
    let mut result = RoleBundleInspection {
        candidates: Vec::new(),
        unclassified_positive_members: Vec::new(),
        malformed_sequence_members: Vec::new(),
        ambiguous_members: Vec::new(),
    };
    for &admission in positive {
        let evidence = wire(s, admission)?;
        let (theory, rule) = poles(s, admission)?;
        if admission == ROOT_HANDLE || theory == admission || rule == admission {
            result.unclassified_positive_members.push(evidence);
            continue;
        }
        let (bundle, body) = poles(s, rule)?;
        if rule == ROOT_HANDLE || bundle == rule || body == rule {
            result.unclassified_positive_members.push(evidence);
            continue;
        }
        let (antecedent, image) = poles(s, body)?;
        if body == ROOT_HANDLE || antecedent == body || image == body {
            result.unclassified_positive_members.push(evidence);
            continue;
        }
        let outputs = match exact_sequence(s, image) {
            Ok(v) => v,
            Err(()) => {
                result.malformed_sequence_members.push(evidence);
                continue;
            }
        };
        // Theory == V allows competing interpretations of a member.
        // Neither structural shape nor host preference resolves the ambiguity.
        if theory == bundle {
            result.ambiguous_members.push(evidence);
            continue;
        }
        let mut roles = Vec::new();
        for &member in positive {
            if poles(s, member)?.0 == bundle {
                roles.push(wire(s, member)?); // Role LINK, not its name pole.
            }
        }
        roles.sort_unstable();
        result.candidates.push(RoleBundleCandidate {
            positive_admission: evidence,
            theory_pole: wire(s, theory)?,
            rule: wire(s, rule)?,
            role_bundle_anchor: wire(s, bundle)?,
            roles,
            antecedent_template: wire(s, antecedent)?,
            output_sequence_carrier: wire(s, image)?,
            output_templates: outputs
                .into_iter()
                .map(|h| wire(s, h))
                .collect::<Result<Vec<_>, _>>()?,
        });
    }
    result
        .candidates
        .sort_by(|a, b| a.positive_admission.cmp(&b.positive_admission));
    result.unclassified_positive_members.sort_unstable();
    result.malformed_sequence_members.sort_unstable();
    result.ambiguous_members.sort_unstable();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pair(s: &mut OptimizedLinkStore, a: Handle, b: Handle) -> Handle {
        s.ensure_pair(a, b).unwrap()
    }
    fn fresh(s: &mut OptimizedLinkStore, cursor: &mut Handle) -> Handle {
        *cursor = pair(s, *cursor, ROOT_HANDLE);
        *cursor
    }
    fn seq(s: &mut OptimizedLinkStore, values: &[Handle]) -> Handle {
        let mut current = ROOT_HANDLE;
        for &value in values {
            let payload = pair(s, current, value);
            current = s.ensure_start_self_closed(payload).unwrap();
        }
        current
    }
    fn fixture(
        noisy: bool,
        reverse_roles: bool,
    ) -> (OptimizedLinkStore, BTreeSet<Handle>, Handle, Handle) {
        let mut s = OptimizedLinkStore::new();
        if noisy {
            let n = pair(&mut s, ROOT_HANDLE, ROOT_HANDLE);
            s.ensure_start_self_closed(n).unwrap();
        }
        let mut cursor = ROOT_HANDLE;
        let theory = fresh(&mut s, &mut cursor);
        let bundle = fresh(&mut s, &mut cursor);
        // The names must not themselves be V->value. Otherwise positive
        // membership would correctly make such a name a *third* role Link.
        let _spacer = fresh(&mut s, &mut cursor);
        let n1 = fresh(&mut s, &mut cursor);
        let n2 = fresh(&mut s, &mut cursor);
        // N10: reverse the allocation order of the two real Role Links, not
        // the insertion order of a BTreeSet (which always re-sorts itself).
        let (r1, r2) = if reverse_roles {
            let r2 = pair(&mut s, bundle, n2);
            let r1 = pair(&mut s, bundle, n1);
            (r1, r2)
        } else {
            let r1 = pair(&mut s, bundle, n1);
            let r2 = pair(&mut s, bundle, n2);
            (r1, r2)
        };
        let antecedent = pair(&mut s, r1, r2);
        let output = pair(&mut s, r2, r1);
        let image = seq(&mut s, &[output, r1]);
        let body = pair(&mut s, antecedent, image);
        let rule = pair(&mut s, bundle, body);
        let admission = pair(&mut s, theory, rule);
        let ambient_name = fresh(&mut s, &mut cursor);
        let ambient = pair(&mut s, bundle, ambient_name);
        (s, BTreeSet::from([admission, r1, r2]), ambient, admission)
    }

    #[test]
    fn a1_3_component_two_roles_positive_membership_and_read_only() {
        let (s, members, ambient, admission) = fixture(false, false);
        let before = s.export_packed_duplets();
        let index = s.export_packed_incidence_index_image();
        let report = inspect(&s, &members).unwrap();
        assert_eq!(report.candidates.len(), 1);
        assert_eq!(report.candidates[0].roles.len(), 2);
        assert_eq!(report.candidates[0].output_templates.len(), 2);
        assert!(!report.candidates[0]
            .roles
            .contains(&wire(&s, ambient).unwrap()));
        assert!(!report.candidates[0]
            .roles
            .contains(&wire(&s, poles(&s, ambient).unwrap().1).unwrap()));
        assert!(inspect(&s, &BTreeSet::new()).unwrap().candidates.is_empty());
        let grounded = inspect(&s, &BTreeSet::from([admission])).unwrap();
        assert_eq!(grounded.candidates.len(), 1);
        assert!(grounded.candidates[0].roles.is_empty());
        assert_eq!(s.export_packed_duplets(), before);
        assert_eq!(s.export_packed_incidence_index_image(), index);
    }

    #[test]
    fn a1_3_component_empty_one_many_exact_sequence_and_corruption() {
        let (mut s, _, _, _) = fixture(false, false);
        let mut cursor = ROOT_HANDLE;
        for _ in 0..20 {
            fresh(&mut s, &mut cursor);
        }
        let theory = fresh(&mut s, &mut cursor);
        let bundle = fresh(&mut s, &mut cursor);
        let antecedent = fresh(&mut s, &mut cursor);
        let a = fresh(&mut s, &mut cursor);
        let b = fresh(&mut s, &mut cursor);
        for values in [vec![], vec![a], vec![a, b]] {
            let image = seq(&mut s, &values);
            let body = pair(&mut s, antecedent, image);
            let rule = pair(&mut s, bundle, body);
            let admission = pair(&mut s, theory, rule);
            let report = inspect(&s, &BTreeSet::from([admission])).unwrap();
            assert_eq!(report.candidates.len(), 1);
            assert!(report.candidates[0].roles.is_empty());
            assert_eq!(
                report.candidates[0].output_templates,
                values
                    .iter()
                    .map(|&h| wire(&s, h).unwrap())
                    .collect::<Vec<_>>()
            );
        }
        let bad = pair(&mut s, a, b); // not START(prev -> value)
        let body = pair(&mut s, antecedent, bad);
        let rule = pair(&mut s, bundle, body);
        let admission = pair(&mut s, theory, rule);
        let report = inspect(&s, &BTreeSet::from([admission])).unwrap();
        assert!(report.candidates.is_empty());
        assert_eq!(report.malformed_sequence_members.len(), 1);
    }

    #[test]
    fn a1_3_component_nested_three_roles_and_absent_admission() {
        let (mut s, _, _, _) = fixture(false, false);
        let mut cursor = ROOT_HANDLE;
        for _ in 0..40 {
            fresh(&mut s, &mut cursor);
        }
        let theory = fresh(&mut s, &mut cursor);
        let bundle = fresh(&mut s, &mut cursor);
        let a = fresh(&mut s, &mut cursor);
        let b = fresh(&mut s, &mut cursor);
        let c = fresh(&mut s, &mut cursor);
        let roles = [a, b, c].map(|name| pair(&mut s, bundle, name));
        let inner = pair(&mut s, roles[1], roles[2]);
        let outer = pair(&mut s, roles[0], inner);
        let antecedent = s.ensure_start_self_closed(outer).unwrap();
        let output = s.ensure_end_self_closed(inner).unwrap();
        let image = seq(&mut s, &[output]);
        let body = pair(&mut s, antecedent, image);
        let rule = pair(&mut s, bundle, body);
        let admission = pair(&mut s, theory, rule);
        let members = BTreeSet::from([admission, roles[0], roles[1], roles[2]]);
        let carrier = s.export_packed_duplets();
        let report = inspect(&s, &members).unwrap();
        assert_eq!(report.candidates.len(), 1);
        assert_eq!(report.candidates[0].roles.len(), 3);
        assert_eq!(s.export_packed_duplets(), carrier);
        let foreign_theory = fresh(&mut s, &mut cursor);
        let physical_only = pair(&mut s, foreign_theory, rule);
        assert!(!members.contains(&physical_only));
        assert_eq!(inspect(&s, &members).unwrap().candidates, report.candidates);
    }

    #[test]
    fn a1_3_component_portable_wires_across_noisy_memories() {
        let (first, a_members, _, _) = fixture(false, false);
        let (second, b_members, _, _) = fixture(true, false);
        assert_eq!(
            inspect(&first, &a_members).unwrap().candidates,
            inspect(&second, &b_members).unwrap().candidates
        );
        assert_ne!(
            first.export_packed_duplets(),
            second.export_packed_duplets()
        );
    }

    #[test]
    fn a1_3_n2_positive_role_names_are_not_role_links() {
        for reverse in [false, true] {
            let (s, members, _, admission) = fixture(false, reverse);
            let rule = poles(&s, admission).unwrap().1;
            let body = poles(&s, rule).unwrap().1;
            let antecedent = poles(&s, body).unwrap().0;
            let (role_a, role_b) = poles(&s, antecedent).unwrap();
            let name_a = poles(&s, role_a).unwrap().1;
            let name_b = poles(&s, role_b).unwrap().1;
            let mut with_names = members.clone();
            with_names.insert(name_a);
            with_names.insert(name_b);
            let original = inspect(&s, &members).unwrap().candidates;
            let enlarged = inspect(&s, &with_names).unwrap();
            let candidate = enlarged
                .candidates
                .iter()
                .find(|c| c.positive_admission == wire(&s, admission).unwrap())
                .unwrap();
            assert_eq!(candidate, &original[0]);
            assert_eq!(candidate.roles.len(), 2);
            assert_eq!(
                candidate.roles,
                [role_a, role_b]
                    .map(|h| wire(&s, h).unwrap())
                    .into_iter()
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>()
            );
            assert!(!candidate.roles.contains(&wire(&s, name_a).unwrap()));
            assert!(!candidate.roles.contains(&wire(&s, name_b).unwrap()));
        }
    }

    #[test]
    fn a1_3_n3_anchor_substitution_does_not_inherit_positive_admission() {
        let (mut s, members, _, admission) = fixture(false, false);
        let rule = poles(&s, admission).unwrap().1;
        let (bundle, body) = poles(&s, rule).unwrap();
        let theory = poles(&s, admission).unwrap().0;
        let baseline = inspect(&s, &members).unwrap().candidates;
        let alternative = pair(&mut s, ROOT_HANDLE, bundle);
        assert_ne!(alternative, bundle);
        let substitute_rule = pair(&mut s, alternative, body);
        let substitute_admission = pair(&mut s, theory, substitute_rule);
        // Both impostors exist physically. Neither is in the positive ANet.
        assert!(!members.contains(&substitute_admission));
        assert_eq!(inspect(&s, &members).unwrap().candidates, baseline);
        let mut expanded = members.clone();
        expanded.insert(substitute_admission);
        let expanded_report = inspect(&s, &expanded).unwrap();
        // Explicit membership permits another *candidate*, not a selected
        // active Theory or authorization for execution.
        assert_eq!(expanded_report.candidates.len(), 2);
        assert!(expanded_report
            .candidates
            .iter()
            .any(|c| c.role_bundle_anchor == wire(&s, alternative).unwrap() && c.roles.is_empty()));
    }

    #[test]
    fn a1_3_n5_malformed_rule_and_body_are_not_candidates() {
        let (mut s, members, _, admission) = fixture(false, false);
        let theory = poles(&s, admission).unwrap().0;
        let rule = poles(&s, admission).unwrap().1;
        let body = poles(&s, rule).unwrap().1;
        let anchor = poles(&s, rule).unwrap().0;
        // Shape-invalid Rule: START(x) has its own handle as start pole.
        let self_closed_rule = s.ensure_start_self_closed(rule).unwrap();
        let invalid_rule_admission = pair(&mut s, theory, self_closed_rule);
        // Shape-invalid Body: END(body) has its own handle as end pole.
        let self_closed_body = s.ensure_end_self_closed(body).unwrap();
        let malformed_rule = pair(&mut s, anchor, self_closed_body);
        let invalid_body_admission = pair(&mut s, theory, malformed_rule);
        for bad_admission in [invalid_rule_admission, invalid_body_admission] {
            let report = inspect(&s, &BTreeSet::from([bad_admission])).unwrap();
            assert!(report.candidates.is_empty());
            assert_eq!(
                report.unclassified_positive_members,
                vec![wire(&s, bad_admission).unwrap()]
            );
        }
        // A merely physical disconnected Rule/Body has no membership grant.
        let detached_antecedent = pair(&mut s, ROOT_HANDLE, anchor);
        let detached_body = pair(&mut s, detached_antecedent, ROOT_HANDLE);
        let detached_rule = pair(&mut s, anchor, detached_body);
        let detached_admission = pair(&mut s, theory, detached_rule);
        assert!(!members.contains(&detached_admission));
        assert_eq!(inspect(&s, &members).unwrap().candidates.len(), 1);
        // This shape is structurally well-formed with empty ExactSequence:
        // no general normative prohibition can be invented from A9b.
        assert_eq!(
            inspect(&s, &BTreeSet::from([detached_admission]))
                .unwrap()
                .candidates
                .len(),
            1
        );
    }

    #[test]
    fn a1_3_n10_opposite_role_carrier_allocation_order() {
        let (first, members_a, _, admission_a) = fixture(false, false);
        let (second, members_b, _, admission_b) = fixture(false, true);
        let role_pair = |s: &OptimizedLinkStore, admission: Handle| {
            let rule = poles(s, admission).unwrap().1;
            let body = poles(s, rule).unwrap().1;
            let antecedent = poles(s, body).unwrap().0;
            poles(s, antecedent).unwrap()
        };
        let (a1, a2) = role_pair(&first, admission_a);
        let (b1, b2) = role_pair(&second, admission_b);
        assert!(a1 < a2);
        assert!(b1 > b2); // actual BTreeSet iteration of role handles flips
        let ordered_role_wires =
            |store: &OptimizedLinkStore, members: &BTreeSet<Handle>, bundle: Handle| {
                members
                    .iter()
                    .copied()
                    .filter(|&h| poles(store, h).unwrap().0 == bundle)
                    .map(|h| wire(store, h).unwrap())
                    .collect::<Vec<_>>()
            };
        let bundle_a = poles(&first, poles(&first, admission_a).unwrap().1)
            .unwrap()
            .0;
        let bundle_b = poles(&second, poles(&second, admission_b).unwrap().1)
            .unwrap()
            .0;
        assert_ne!(
            ordered_role_wires(&first, &members_a, bundle_a),
            ordered_role_wires(&second, &members_b, bundle_b)
        );
        assert_eq!(
            inspect(&first, &members_a).unwrap().candidates,
            inspect(&second, &members_b).unwrap().candidates
        );
    }

    #[test]
    fn a1_3_component_ambiguous_theory_is_not_selected() {
        let mut s = OptimizedLinkStore::new();
        let mut cursor = ROOT_HANDLE;
        let theory = fresh(&mut s, &mut cursor);
        let antecedent = fresh(&mut s, &mut cursor);
        let body = pair(&mut s, antecedent, ROOT_HANDLE);
        let rule = pair(&mut s, theory, body);
        let admission = pair(&mut s, theory, rule);
        let report = inspect(&s, &BTreeSet::from([admission])).unwrap();
        assert!(report.candidates.is_empty());
        assert_eq!(report.ambiguous_members.len(), 1);
    }
}
