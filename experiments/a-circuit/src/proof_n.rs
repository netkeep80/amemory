use amemory_optimized_cpu_probe::{
    structural::OptimizedStructuralEngine,
    Handle, OptimizedLinkStore,
};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    sync::atomic::{AtomicU32, Ordering},
};

static NEXT_PROOF_MEMORY_ID: AtomicU32 = AtomicU32::new(1);

const WEB_STRUCTURAL_PROOF_SCHEMA_VERSION: u32 = 4;
const WEB_COMPACT_PROOF_SCHEMA_VERSION: u32 = 2;
const WEB_COMPACT_PROOF_REPRESENTATION_ID: &str =
    "amemory-proof-compact-json";
const WEB_COMPACT_PROOF_REPRESENTATION_VERSION: &str = "0.2.0";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebProofPreparedRoot {
    pub(crate) role: String,
    pub(crate) carrier_ref: u32,
    pub(crate) source: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebProofLoadedRoot {
    pub(crate) role: String,
    pub(crate) carrier_ref: u32,
    pub(crate) source: String,
    pub(crate) local_handle: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebProofDuplet {
    pub(crate) start: u32,
    pub(crate) end: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebProofPrepareStage {
    pub(crate) compiler_label: String,
    pub(crate) runtime_memory_exists: bool,
    pub(crate) compiled_links: u32,
    pub(crate) carrier_duplets: Vec<WebProofDuplet>,
    pub(crate) semantic_roots: Vec<WebProofPreparedRoot>,
    pub(crate) theory_admissions: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebProofLoadStage {
    pub(crate) memory_instance_id: String,
    pub(crate) links_before_load: u32,
    pub(crate) links_after_load: u32,
    pub(crate) imported_duplets: u32,
    pub(crate) carrier_round_trip: bool,
    pub(crate) semantic_roots: Vec<WebProofLoadedRoot>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebProofReactionStep {
    pub(crate) memory_instance_id: String,
    pub(crate) step: u32,
    pub(crate) scope_before: Vec<String>,
    pub(crate) raw_rule_matches: u32,
    pub(crate) transitioned_members: u32,
    pub(crate) handoff_count: u32,
    pub(crate) scope_after: Vec<String>,
    pub(crate) links_after: u32,
    pub(crate) quiescent: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebProofVisualLink {
    pub(crate) key: String,
    pub(crate) start_key: String,
    pub(crate) end_key: String,
    pub(crate) local_handle: u32,
    pub(crate) label: Option<String>,
    pub(crate) tags: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebProofExecuteStage {
    pub(crate) memory_instance_id: String,
    pub(crate) reactions: Vec<WebProofReactionStep>,
    pub(crate) active_reaction_count: u32,
    pub(crate) final_quiescent: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebProofResultStage {
    pub(crate) memory_instance_id: String,
    // Internal Rust field name is retained only as a construction-compatibility detail.
    // All producer DTOs declare source schema v4; serialization names the arbitrary
    // final Link explicitly as a recursive Link wire, not as an Anum sequence representation.
    #[serde(rename = "resultRecursiveWire")]
    pub(crate) result_anum: String,
    pub(crate) result_sequence_anum: String,
    pub(crate) decoded_value: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) decoded_value_hi: Option<u32>,
    pub(crate) oracle_value: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) oracle_value_hi: Option<u32>,
    pub(crate) oracle_matches: bool,
    pub(crate) links_final: u32,
    pub(crate) identical_rerun_link_delta: u32,
    pub(crate) visual_links: Vec<WebProofVisualLink>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebStructuralProof {
    pub(crate) schema_version: u32,
    pub(crate) block: String,
    pub(crate) prepare: WebProofPrepareStage,
    pub(crate) load: WebProofLoadStage,
    pub(crate) execute: WebProofExecuteStage,
    pub(crate) result: WebProofResultStage,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebCompactPairs {
    pub(crate) starts: Vec<u32>,
    pub(crate) ends: Vec<u32>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebCompactOverlay {
    pub(crate) local_handle: u32,
    pub(crate) label: Option<String>,
    pub(crate) tags: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebCompactTopology {
    pub(crate) base: WebCompactPairs,
    pub(crate) append: WebCompactPairs,
    pub(crate) overlays: Vec<WebCompactOverlay>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebCompactRoot {
    pub(crate) role: String,
    pub(crate) carrier_ref: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebCompactPrepare {
    pub(crate) runtime_memory_exists: bool,
    pub(crate) compiled_links: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebCompactLoad {
    pub(crate) links_before_load: u32,
    pub(crate) links_after_load: u32,
    pub(crate) imported_duplets: u32,
    pub(crate) carrier_round_trip: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebCompactReactionStep {
    pub(crate) step: u32,
    pub(crate) scope_before: Vec<u32>,
    pub(crate) raw_rule_matches: u32,
    pub(crate) transitioned_members: u32,
    pub(crate) handoff_count: u32,
    pub(crate) scope_after: Vec<u32>,
    pub(crate) links_after: u32,
    pub(crate) quiescent: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebCompactExecute {
    pub(crate) active_reaction_count: u32,
    pub(crate) final_quiescent: bool,
    pub(crate) reactions: Vec<WebCompactReactionStep>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebCompactResult {
    pub(crate) result_recursive_wire: String,
    pub(crate) result_sequence_anum: String,
    pub(crate) decoded_value: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) decoded_value_hi: Option<u32>,
    pub(crate) oracle_value: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) oracle_value_hi: Option<u32>,
    pub(crate) oracle_matches: bool,
    pub(crate) links_final: u32,
    pub(crate) identical_rerun_link_delta: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebCompactProof {
    pub(crate) schema_version: u32,
    pub(crate) representation_id: String,
    pub(crate) representation_version: String,
    pub(crate) source_proof_schema_version: u32,
    pub(crate) block: String,
    pub(crate) memory_instance_id: String,
    pub(crate) prepare: WebCompactPrepare,
    pub(crate) topology: WebCompactTopology,
    pub(crate) roots: Vec<WebCompactRoot>,
    pub(crate) theory_admissions: Vec<u32>,
    pub(crate) load: WebCompactLoad,
    pub(crate) execute: WebCompactExecute,
    pub(crate) result: WebCompactResult,
}


fn parse_local_ref(value: &str, max_handle: u32) -> Option<u32> {
    let handle = value.strip_prefix('L')?.parse::<u32>().ok()?;
    (handle >= 1 && handle <= max_handle).then_some(handle)
}

fn parse_visual_ref(
    value: &str,
    memory_id: &str,
    max_handle: u32,
) -> Option<u32> {
    let prefix = format!("{memory_id}:L");
    let handle = value.strip_prefix(&prefix)?.parse::<u32>().ok()?;
    (handle >= 1 && handle <= max_handle).then_some(handle)
}

impl WebStructuralProof {
    pub(crate) fn compact(&self) -> Option<WebCompactProof> {
        if self.schema_version != WEB_STRUCTURAL_PROOF_SCHEMA_VERSION {
            return None;
        }

        let memory_id = self.result.memory_instance_id.as_str();
        if memory_id.is_empty()
            || self.load.memory_instance_id != memory_id
            || self.execute.memory_instance_id != memory_id
        {
            return None;
        }

        let base_len = self.prepare.carrier_duplets.len() as u32;
        if base_len != self.prepare.compiled_links
            || self.load.links_after_load != base_len
            || self.load.imported_duplets != base_len
            || !self.load.carrier_round_trip
            || self.result.visual_links.len() as u32 != self.result.links_final
        {
            return None;
        }

        let mut base = WebCompactPairs {
            starts: Vec::with_capacity(self.prepare.carrier_duplets.len()),
            ends: Vec::with_capacity(self.prepare.carrier_duplets.len()),
        };
        for duplet in &self.prepare.carrier_duplets {
            base.starts.push(duplet.start);
            base.ends.push(duplet.end);
        }

        let append_capacity =
            self.result.links_final.checked_sub(base_len)? as usize;
        let mut append = WebCompactPairs {
            starts: Vec::with_capacity(append_capacity),
            ends: Vec::with_capacity(append_capacity),
        };
        let mut overlays = Vec::new();

        for (index, visual) in self.result.visual_links.iter().enumerate() {
            let handle = index as u32 + 1;
            if visual.local_handle != handle
                || parse_visual_ref(
                    &visual.key,
                    memory_id,
                    self.result.links_final,
                )? != handle
            {
                return None;
            }
            let start = parse_visual_ref(
                &visual.start_key,
                memory_id,
                self.result.links_final,
            )?;
            let end = parse_visual_ref(
                &visual.end_key,
                memory_id,
                self.result.links_final,
            )?;

            if handle <= base_len {
                let prepared = &self.prepare.carrier_duplets[index];
                if prepared.start != start || prepared.end != end {
                    return None;
                }
            } else {
                append.starts.push(start);
                append.ends.push(end);
            }

            if visual.label.is_some() || !visual.tags.is_empty() {
                overlays.push(WebCompactOverlay {
                    local_handle: handle,
                    label: visual.label.clone(),
                    tags: visual.tags.clone(),
                });
            }
        }

        if append.starts.len() != append_capacity
            || append.ends.len() != append_capacity
        {
            return None;
        }

        if self.prepare.semantic_roots.len() != self.load.semantic_roots.len() {
            return None;
        }
        let loaded_by_role = self
            .load
            .semantic_roots
            .iter()
            .map(|root| (root.role.as_str(), root))
            .collect::<HashMap<_, _>>();
        if loaded_by_role.len() != self.load.semantic_roots.len() {
            return None;
        }

        let mut prepared_roles = HashSet::new();
        let mut roots = Vec::with_capacity(self.prepare.semantic_roots.len());
        for root in &self.prepare.semantic_roots {
            if !prepared_roles.insert(root.role.as_str()) {
                return None;
            }
            if root.carrier_ref < 1 || root.carrier_ref > base_len {
                return None;
            }
            let loaded = loaded_by_role.get(root.role.as_str())?;
            if loaded.carrier_ref != root.carrier_ref
                || loaded.local_handle != root.carrier_ref
                || loaded.source != root.source
            {
                return None;
            }
            roots.push(WebCompactRoot {
                role: root.role.clone(),
                carrier_ref: root.carrier_ref,
            });
        }

        let theory_admissions = self
            .prepare
            .theory_admissions
            .iter()
            .map(|value| parse_local_ref(value, base_len))
            .collect::<Option<Vec<_>>>()?;

        let expected_active_reactions = self
            .execute
            .reactions
            .iter()
            .filter(|step| !step.quiescent)
            .count() as u32;
        if expected_active_reactions != self.execute.active_reaction_count
            || self
                .execute
                .reactions
                .last()
                .map(|step| step.quiescent)
                .unwrap_or(false)
                != self.execute.final_quiescent
        {
            return None;
        }

        let mut previous_links_after = self.load.links_after_load;
        let reactions = self
            .execute
            .reactions
            .iter()
            .enumerate()
            .map(|(index, step)| {
                let links_before = previous_links_after;
                if step.memory_instance_id != memory_id
                    || step.step != index as u32
                    || step.links_after < links_before
                    || step.links_after > self.result.links_final
                {
                    return None;
                }
                let scope_before = step
                    .scope_before
                    .iter()
                    .map(|value| parse_local_ref(value, links_before))
                    .collect::<Option<Vec<_>>>()?;
                let scope_after = step
                    .scope_after
                    .iter()
                    .map(|value| parse_local_ref(value, step.links_after))
                    .collect::<Option<Vec<_>>>()?;
                previous_links_after = step.links_after;
                Some(WebCompactReactionStep {
                    step: step.step,
                    scope_before,
                    raw_rule_matches: step.raw_rule_matches,
                    transitioned_members: step.transitioned_members,
                    handoff_count: step.handoff_count,
                    scope_after,
                    links_after: step.links_after,
                    quiescent: step.quiescent,
                })
            })
            .collect::<Option<Vec<_>>>()?;

        if previous_links_after
            .checked_add(self.result.identical_rerun_link_delta)?
            != self.result.links_final
        {
            return None;
        }

        Some(WebCompactProof {
            schema_version: WEB_COMPACT_PROOF_SCHEMA_VERSION,
            representation_id:
                WEB_COMPACT_PROOF_REPRESENTATION_ID.to_owned(),
            representation_version:
                WEB_COMPACT_PROOF_REPRESENTATION_VERSION.to_owned(),
            source_proof_schema_version: self.schema_version,
            block: self.block.clone(),
            memory_instance_id: memory_id.to_owned(),
            prepare: WebCompactPrepare {
                runtime_memory_exists: self.prepare.runtime_memory_exists,
                compiled_links: self.prepare.compiled_links,
            },
            topology: WebCompactTopology {
                base,
                append,
                overlays,
            },
            roots,
            theory_admissions,
            load: WebCompactLoad {
                links_before_load: self.load.links_before_load,
                links_after_load: self.load.links_after_load,
                imported_duplets: self.load.imported_duplets,
                carrier_round_trip: self.load.carrier_round_trip,
            },
            execute: WebCompactExecute {
                active_reaction_count: self.execute.active_reaction_count,
                final_quiescent: self.execute.final_quiescent,
                reactions,
            },
            result: WebCompactResult {
                result_recursive_wire: self.result.result_anum.clone(),
                result_sequence_anum:
                    self.result.result_sequence_anum.clone(),
                decoded_value: self.result.decoded_value,
                decoded_value_hi: self.result.decoded_value_hi,
                oracle_value: self.result.oracle_value,
                oracle_value_hi: self.result.oracle_value_hi,
                oracle_matches: self.result.oracle_matches,
                links_final: self.result.links_final,
                identical_rerun_link_delta:
                    self.result.identical_rerun_link_delta,
            },
        })
    }
}

pub(crate) type WebMux1Proof = WebStructuralProof;

#[derive(Debug)]
pub(crate) struct ProofRuntimeMemory {
    pub(crate) id: String,
    pub(crate) store: OptimizedLinkStore,
}

pub(crate) fn export_packed_carrier(
    store: &OptimizedLinkStore,
) -> Vec<WebProofDuplet> {
    store
        .export_packed_duplets()
        .into_iter()
        .map(|(start, end)| WebProofDuplet { start, end })
        .collect()
}

pub(crate) fn packed_gpu_carrier_words(
    proof: &WebStructuralProof,
) -> Option<Vec<u32>> {
    let duplets = proof
        .prepare
        .carrier_duplets
        .iter()
        .map(|duplet| (duplet.start, duplet.end))
        .collect::<Vec<_>>();
    if duplets.len() != proof.prepare.compiled_links as usize {
        return None;
    }

    let mut store = OptimizedLinkStore::new();
    store.load_packed_duplets(&duplets).ok()?;
    let image = store.export_packed_gpu_carrier_image();
    Some(image.words().to_vec())
}

pub(crate) fn export_scope(
    store: &OptimizedLinkStore,
    scope: &[Handle],
) -> Vec<String> {
    scope
        .iter()
        .map(|handle| {
            store.poles(*handle).expect("scope Link reference");
            format!("L{handle}")
        })
        .collect()
}

pub(crate) fn semantic_source(
    store: &OptimizedLinkStore,
    role: &str,
    handle: Handle,
) -> WebProofPreparedRoot {
    WebProofPreparedRoot {
        role: role.to_owned(),
        carrier_ref: handle,
        source: store
            .export_anum(handle)
            .expect("semantic root export"),
    }
}

pub(crate) fn theory_admissions(
    store: &OptimizedLinkStore,
    theory: Handle,
) -> Option<Vec<String>> {
    let mut admissions = store
        .start_incidence(theory)
        .ok()?
        .filter_map(|handle| {
            let (start, _end) = store.poles(handle).ok()?;
            (start == theory).then_some(handle)
        })
        .collect::<Vec<_>>();

    admissions.sort_unstable();
    (!admissions.is_empty()).then(|| {
        admissions
            .into_iter()
            .map(|handle| format!("L{handle}"))
            .collect()
    })
}

pub(crate) fn prepare_stage(
    compiler: &OptimizedLinkStore,
    semantic_roots: Vec<WebProofPreparedRoot>,
    theory_admissions: Vec<String>,
) -> WebProofPrepareStage {
    WebProofPrepareStage {
        compiler_label:
            "CPU-built packed duplet carrier (not runtime A-memory)"
                .to_owned(),
        runtime_memory_exists: false,
        compiled_links: compiler.link_count() as u32,
        carrier_duplets: export_packed_carrier(compiler),
        semantic_roots,
        theory_admissions,
    }
}

pub(crate) fn load_runtime(
    prepare: &WebProofPrepareStage,
) -> Option<(ProofRuntimeMemory, WebProofLoadStage)> {
    let memory_number =
        NEXT_PROOF_MEMORY_ID.fetch_add(1, Ordering::SeqCst);
    let mut memory = ProofRuntimeMemory {
        id: format!("A-memory#{}", memory_number),
        store: OptimizedLinkStore::new(),
    };

    let links_before_load = memory.store.link_count() as u32;
    let carrier = prepare
        .carrier_duplets
        .iter()
        .map(|duplet| (duplet.start, duplet.end))
        .collect::<Vec<_>>();
    memory.store.load_packed_duplets(&carrier).ok()?;
    let links_after_load = memory.store.link_count() as u32;

    if links_after_load != prepare.compiled_links {
        return None;
    }

    let before_roots = memory.store.link_count();
    let mut loaded_roots =
        Vec::with_capacity(prepare.semantic_roots.len());
    let mut carrier_round_trip =
        memory.store.export_packed_duplets() == carrier;

    for root in &prepare.semantic_roots {
        let handle = root.carrier_ref;
        if handle == 0 || handle > links_after_load {
            return None;
        }
        memory.store.poles(handle).ok()?;
        if memory.store.export_anum(handle).ok().as_deref()
            != Some(root.source.as_str())
        {
            carrier_round_trip = false;
        }
        loaded_roots.push(WebProofLoadedRoot {
            role: root.role.clone(),
            carrier_ref: root.carrier_ref,
            source: root.source.clone(),
            local_handle: handle,
        });
    }
    if memory.store.link_count() != before_roots {
        return None;
    }

    let theory_handle = loaded_roots
        .iter()
        .find(|root| root.role == "execution.theory")
        .map(|root| root.local_handle)?;
    for admission in &prepare.theory_admissions {
        let handle = admission
            .strip_prefix('L')?
            .parse::<Handle>()
            .ok()?;
        let (start, _end) = memory.store.poles(handle).ok()?;
        if start != theory_handle {
            return None;
        }
    }

    let load = WebProofLoadStage {
        memory_instance_id: memory.id.clone(),
        links_before_load,
        links_after_load,
        imported_duplets: prepare.carrier_duplets.len() as u32,
        carrier_round_trip,
        semantic_roots: loaded_roots,
    };

    Some((memory, load))
}

pub(crate) fn loaded_handle(
    load: &WebProofLoadStage,
    role: &str,
) -> Option<Handle> {
    load.semantic_roots
        .iter()
        .find(|root| root.role == role)
        .map(|root| root.local_handle)
}

pub(crate) fn execute_to_quiescence(
    memory: &mut ProofRuntimeMemory,
    interpreter: Handle,
    initial: Handle,
    cap: usize,
    max_steps: u32,
) -> Option<(OptimizedStructuralEngine, WebProofExecuteStage)> {
    let mut engine = OptimizedStructuralEngine::new(cap);
    engine
        .set_interpreter(&memory.store, interpreter)
        .ok()?;
    engine.set_current(&memory.store, &[initial]).ok()?;

    let mut reactions = Vec::new();
    for step in 0..max_steps {
        let scope_before = export_scope(&memory.store, engine.current());
        let reaction = engine.run(&mut memory.store).ok()?;
        let scope_after = export_scope(&memory.store, engine.current());
        let quiescent = reaction.quiescent;
        reactions.push(WebProofReactionStep {
            memory_instance_id: memory.id.clone(),
            step,
            scope_before,
            raw_rule_matches: reaction.raw_rule_matches,
            transitioned_members: reaction.transitioned_members,
            handoff_count: reaction.handoff_count,
            scope_after,
            links_after: memory.store.link_count() as u32,
            quiescent,
        });
        if quiescent {
            break;
        }
    }

    if !reactions
        .last()
        .map(|step| step.quiescent)
        .unwrap_or(false)
    {
        return None;
    }

    let active_reaction_count =
        reactions.iter().filter(|step| !step.quiescent).count() as u32;

    let execute = WebProofExecuteStage {
        memory_instance_id: memory.id.clone(),
        reactions,
        active_reaction_count,
        final_quiescent: true,
    };

    Some((engine, execute))
}

pub(crate) fn identical_rerun(
    memory: &mut ProofRuntimeMemory,
    engine: &mut OptimizedStructuralEngine,
    initial: Handle,
    expected_result_anum: &str,
    max_steps: u32,
) -> Option<u32> {
    let links_before_rerun = memory.store.link_count();
    engine.set_current(&memory.store, &[initial]).ok()?;

    for _ in 0..max_steps {
        let repeat = engine.run(&mut memory.store).ok()?;
        if repeat.quiescent {
            break;
        }
    }

    if !engine.quiescent() || engine.current().len() != 1 {
        return None;
    }

    let repeat_result =
        memory.store.export_anum(engine.current()[0]).ok()?;
    if repeat_result != expected_result_anum {
        return None;
    }

    Some((memory.store.link_count() - links_before_rerun) as u32)
}

pub(crate) fn visual_snapshot(
    memory: &ProofRuntimeMemory,
    loaded_roots: &[WebProofLoadedRoot],
) -> Vec<WebProofVisualLink> {
    let mut roles_by_handle: HashMap<u32, Vec<String>> = HashMap::new();
    for root in loaded_roots {
        roles_by_handle
            .entry(root.local_handle)
            .or_default()
            .push(root.role.clone());
    }

    (1..=memory.store.link_count() as u32)
        .map(|handle| {
            let (start, end) =
                memory.store.poles(handle).expect("visual poles");
            let roles = roles_by_handle
                .get(&handle)
                .cloned()
                .unwrap_or_default();
            WebProofVisualLink {
                key: format!("{}:L{}", memory.id, handle),
                start_key: format!("{}:L{}", memory.id, start),
                end_key: format!("{}:L{}", memory.id, end),
                local_handle: handle,
                label: (!roles.is_empty()).then(|| roles.join(" + ")),
                tags: roles,
            }
        })
        .collect()
}


#[cfg(test)]
mod tests {
    use super::*;
    use amemory_optimized_cpu_probe::ROOT_HANDLE;


    fn minimal_structural_proof() -> WebStructuralProof {
        let memory_id = "A-memory#test".to_owned();
        WebStructuralProof {
            schema_version: WEB_STRUCTURAL_PROOF_SCHEMA_VERSION,
            block: "test".to_owned(),
            prepare: WebProofPrepareStage {
                compiler_label: "test".to_owned(),
                runtime_memory_exists: false,
                compiled_links: 1,
                carrier_duplets: vec![WebProofDuplet { start: 1, end: 1 }],
                semantic_roots: vec![WebProofPreparedRoot {
                    role: "root".to_owned(),
                    carrier_ref: 1,
                    source: "8".to_owned(),
                }],
                theory_admissions: vec![],
            },
            load: WebProofLoadStage {
                memory_instance_id: memory_id.clone(),
                links_before_load: 1,
                links_after_load: 1,
                imported_duplets: 1,
                carrier_round_trip: true,
                semantic_roots: vec![WebProofLoadedRoot {
                    role: "root".to_owned(),
                    carrier_ref: 1,
                    source: "8".to_owned(),
                    local_handle: 1,
                }],
            },
            execute: WebProofExecuteStage {
                memory_instance_id: memory_id.clone(),
                reactions: vec![WebProofReactionStep {
                    memory_instance_id: memory_id.clone(),
                    step: 0,
                    scope_before: vec!["L1".to_owned()],
                    raw_rule_matches: 0,
                    transitioned_members: 0,
                    handoff_count: 0,
                    scope_after: vec!["L1".to_owned()],
                    links_after: 1,
                    quiescent: true,
                }],
                active_reaction_count: 0,
                final_quiescent: true,
            },
            result: WebProofResultStage {
                memory_instance_id: memory_id.clone(),
                result_anum: "8".to_owned(),
                result_sequence_anum: "8".to_owned(),
                decoded_value: 0,
                decoded_value_hi: None,
                oracle_value: 0,
                oracle_value_hi: None,
                oracle_matches: true,
                links_final: 1,
                identical_rerun_link_delta: 0,
                visual_links: vec![WebProofVisualLink {
                    key: format!("{memory_id}:L1"),
                    start_key: format!("{memory_id}:L1"),
                    end_key: format!("{memory_id}:L1"),
                    local_handle: 1,
                    label: Some("root".to_owned()),
                    tags: vec!["root".to_owned()],
                }],
            },
        }
    }

    #[test]
    fn compact_accepts_consistent_v3_proof() {
        let proof = minimal_structural_proof();
        let compact = proof.compact().expect("compact proof");
        assert_eq!(compact.schema_version, WEB_COMPACT_PROOF_SCHEMA_VERSION);
        assert_eq!(
            compact.representation_id,
            WEB_COMPACT_PROOF_REPRESENTATION_ID
        );
        assert_eq!(
            compact.representation_version,
            WEB_COMPACT_PROOF_REPRESENTATION_VERSION
        );
        assert_eq!(compact.source_proof_schema_version, 4);
        assert_eq!(compact.topology.base.starts, vec![1]);
        assert_eq!(compact.topology.base.ends, vec![1]);

        let compact_json = serde_json::to_value(&compact).expect("compact JSON");
        assert_eq!(compact_json["result"]["resultRecursiveWire"], "8");
        assert_eq!(compact_json["result"]["resultSequenceAnum"], "8");
        assert!(compact_json["result"].get("resultAnum").is_none());

        let source_json = serde_json::to_value(&proof).expect("source proof JSON");
        assert_eq!(source_json["schemaVersion"], 4);
        assert_eq!(source_json["result"]["resultRecursiveWire"], "8");
        assert_eq!(source_json["result"]["resultSequenceAnum"], "8");
        assert!(source_json["result"].get("resultAnum").is_none());
    }

    #[test]
    fn compact_rejects_unknown_source_schema() {
        let mut proof = minimal_structural_proof();
        proof.schema_version += 1;
        assert!(proof.compact().is_none());
    }

    #[test]
    fn compact_rejects_duplicate_root_roles() {
        let mut proof = minimal_structural_proof();
        proof.prepare.semantic_roots.push(WebProofPreparedRoot {
            role: "root".to_owned(),
            carrier_ref: 1,
            source: "8".to_owned(),
        });
        proof.load.semantic_roots.push(WebProofLoadedRoot {
            role: "root".to_owned(),
            carrier_ref: 1,
            source: "8".to_owned(),
            local_handle: 1,
        });
        assert!(proof.compact().is_none());
    }

    #[test]
    fn compact_rejects_inconsistent_reaction_summary() {
        let mut proof = minimal_structural_proof();
        proof.execute.active_reaction_count = 1;
        assert!(proof.compact().is_none());
    }

    #[test]
    fn compact_rejects_non_monotonic_runtime_link_counts() {
        let mut proof = minimal_structural_proof();
        proof.load.links_after_load = 2;
        proof.prepare.compiled_links = 2;
        proof.prepare.carrier_duplets.push(WebProofDuplet { start: 1, end: 1 });
        proof.load.imported_duplets = 2;
        proof.result.visual_links.push(WebProofVisualLink {
            key: "A-memory#test:L2".to_owned(),
            start_key: "A-memory#test:L1".to_owned(),
            end_key: "A-memory#test:L1".to_owned(),
            local_handle: 2,
            label: None,
            tags: vec![],
        });
        proof.result.links_final = 2;
        proof.execute.reactions[0].links_after = 1;
        assert!(proof.compact().is_none());
    }

    #[test]
    fn compact_rejects_scope_before_link_created_by_same_reaction() {
        let mut proof = minimal_structural_proof();
        proof.result.visual_links.push(WebProofVisualLink {
            key: "A-memory#test:L2".to_owned(),
            start_key: "A-memory#test:L1".to_owned(),
            end_key: "A-memory#test:L1".to_owned(),
            local_handle: 2,
            label: None,
            tags: vec![],
        });
        proof.result.links_final = 2;
        proof.execute.reactions[0].scope_before = vec!["L2".to_owned()];
        proof.execute.reactions[0].scope_after = vec!["L2".to_owned()];
        proof.execute.reactions[0].links_after = 2;
        assert!(proof.compact().is_none());
    }

    #[test]
    fn packed_carrier_reconstructs_complete_shared_topology() {
        let mut store = OptimizedLinkStore::new();
        let o = store.import_anum("98").unwrap();
        let c = store.import_anum("68").unwrap();
        let l = store.ensure_pair(o, c).unwrap();
        let u = store.ensure_pair(c, o).unwrap();
        let left = store.ensure_pair(l, u).unwrap();
        let right = store.ensure_pair(u, l).unwrap();
        let _top = store.ensure_pair(left, right).unwrap();
        let _second_top = store.ensure_pair(right, ROOT_HANDLE).unwrap();

        let carrier = export_packed_carrier(&store);
        assert_eq!(carrier.len(), store.link_count());

        let packed = carrier
            .iter()
            .map(|duplet| (duplet.start, duplet.end))
            .collect::<Vec<_>>();
        let mut reconstructed = OptimizedLinkStore::new();
        reconstructed.load_packed_duplets(&packed).unwrap();

        assert_eq!(reconstructed.link_count(), store.link_count());
        assert_eq!(reconstructed.export_packed_duplets(), packed);
        for handle in 1..=store.link_count() as u32 {
            assert_eq!(
                reconstructed.export_anum(handle).unwrap(),
                store.export_anum(handle).unwrap()
            );
        }
    }
}
