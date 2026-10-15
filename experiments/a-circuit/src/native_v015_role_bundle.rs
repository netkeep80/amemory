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
fn poles(s: &OptimizedLinkStore, h: Handle) -> Result<(Handle,Handle), ()> {
    s.poles(h).map_err(|_| ())
}
fn wire(s: &OptimizedLinkStore, h: Handle) -> Result<String, ()> {
    s.export_direct_recursive_wire(h).map_err(|_| ())
}
/// Existing ExactSequence carrier: empty=ROOT; Cell(prev,value)=START(prev->value).
fn exact_sequence(s: &OptimizedLinkStore, end: Handle) -> Result<Vec<Handle>, ()> {
    let mut current=end;
    let mut visited=BTreeSet::new();
    let mut reversed=Vec::new();
    while current!=ROOT_HANDLE {
        if !visited.insert(current) { return Err(()); }
        let (a,payload)=poles(s,current)?;
        if a!=current || payload==current { return Err(()); }
        let (prev,value)=poles(s,payload)?;
        reversed.push(value);
        current=prev;
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
    let mut result=RoleBundleInspection {
        candidates:Vec::new(), unclassified_positive_members:Vec::new(),
        malformed_sequence_members:Vec::new(), ambiguous_members:Vec::new(),
    };
    for &admission in positive {
        let evidence=wire(s,admission)?;
        let (theory,rule)=poles(s,admission)?;
        if admission==ROOT_HANDLE || theory==admission || rule==admission {
            result.unclassified_positive_members.push(evidence);
            continue;
        }
        let (bundle,body)=poles(s,rule)?;
        if rule==ROOT_HANDLE || bundle==rule || body==rule {
            result.unclassified_positive_members.push(evidence);
            continue;
        }
        let (antecedent,image)=poles(s,body)?;
        if body==ROOT_HANDLE || antecedent==body || image==body {
            result.unclassified_positive_members.push(evidence);
            continue;
        }
        let outputs=match exact_sequence(s,image) {
            Ok(v)=>v,
            Err(())=>{ result.malformed_sequence_members.push(evidence);continue; }
        };
        // If Theory == V, one member has two possible structural roles.
        // Do not invent a semantic priority among interpretations.
        if theory==bundle {
            result.ambiguous_members.push(evidence);
            continue;
        }
        let mut roles=Vec::new();
        for &member in positive {
            if poles(s,member)?.0==bundle { roles.push(wire(s,member)?); }
        }
        roles.sort_unstable();
        result.candidates.push(RoleBundleCandidate {
            positive_admission:evidence, theory_pole:wire(s,theory)?,
            rule:wire(s,rule)?, role_bundle_anchor:wire(s,bundle)?,
            roles, antecedent_template:wire(s,antecedent)?,
            output_sequence_carrier:wire(s,image)?,
            output_templates:outputs.into_iter().map(|h|wire(s,h))
                .collect::<Result<Vec<_>,_>>()?,
        });
    }
    result.candidates.sort_by(|a,b|a.positive_admission.cmp(&b.positive_admission));
    result.unclassified_positive_members.sort_unstable();
    result.malformed_sequence_members.sort_unstable();
    result.ambiguous_members.sort_unstable();
    Ok(result)
}
