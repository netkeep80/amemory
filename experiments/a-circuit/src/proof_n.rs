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

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebProofPreparedRoot {
    pub(crate) role: String,
    pub(crate) source: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebProofLoadedRoot {
    pub(crate) role: String,
    pub(crate) source: String,
    pub(crate) local_handle: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebProofPrepareStage {
    pub(crate) compiler_label: String,
    pub(crate) runtime_memory_exists: bool,
    pub(crate) compiled_links: u32,
    pub(crate) aset_anums: Vec<String>,
    pub(crate) semantic_roots: Vec<WebProofPreparedRoot>,
    pub(crate) theory_admissions: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WebProofLoadStage {
    pub(crate) memory_instance_id: String,
    pub(crate) links_before_load: u32,
    pub(crate) links_after_load: u32,
    pub(crate) imported_anums: u32,
    pub(crate) portable_round_trip: bool,
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
    pub(crate) result_anum: String,
    pub(crate) result_sequence_anum: String,
    pub(crate) decoded_value: u8,
    pub(crate) oracle_value: u8,
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

pub(crate) type WebMux1Proof = WebStructuralProof;

#[derive(Debug)]
pub(crate) struct ProofRuntimeMemory {
    pub(crate) id: String,
    pub(crate) store: OptimizedLinkStore,
}

pub(crate) fn export_portable_aset(store: &OptimizedLinkStore) -> Vec<String> {
    let count = store.link_count() as u32;
    let mut referenced = HashSet::new();
    for handle in 1..=count {
        let (start, end) = store.poles(handle).expect("proof poles");
        if start != handle {
            referenced.insert(start);
        }
        if end != handle {
            referenced.insert(end);
        }
    }

    let mut sources = Vec::new();
    let mut reconstructed = OptimizedLinkStore::new();

    for handle in 1..=count {
        if referenced.contains(&handle) {
            continue;
        }
        let source = store.export_anum(handle).expect("proof root export");
        reconstructed.import_anum(&source).expect("proof root import");
        sources.push(source);
    }

    for handle in 1..=count {
        let source = store.export_anum(handle).expect("proof completion export");
        let before = reconstructed.link_count();
        let imported = reconstructed
            .import_anum(&source)
            .expect("proof completion import");
        assert_eq!(
            reconstructed
                .export_anum(imported)
                .expect("proof completion round-trip"),
            source,
        );
        if reconstructed.link_count() > before {
            sources.push(source);
        }
    }

    for handle in 1..=count {
        let source = store.export_anum(handle).expect("proof coverage export");
        let before = reconstructed.link_count();
        let imported = reconstructed
            .import_anum(&source)
            .expect("proof coverage import");
        assert_eq!(
            reconstructed.link_count(),
            before,
            "portable Aset omitted topology"
        );
        assert_eq!(reconstructed.export_anum(imported).unwrap(), source);
    }

    sources
}

pub(crate) fn export_scope(
    store: &OptimizedLinkStore,
    scope: &[Handle],
) -> Vec<String> {
    scope
        .iter()
        .map(|handle| store.export_anum(*handle).expect("scope export"))
        .collect()
}

pub(crate) fn semantic_source(
    store: &OptimizedLinkStore,
    role: &str,
    handle: Handle,
) -> WebProofPreparedRoot {
    WebProofPreparedRoot {
        role: role.to_owned(),
        source: store
            .export_anum(handle)
            .expect("semantic root export"),
    }
}

pub(crate) fn theory_admissions(
    store: &OptimizedLinkStore,
    theory: Handle,
) -> Option<Vec<String>> {
    let admissions = store
        .start_incidence(theory)
        .ok()?
        .filter_map(|handle| {
            let (start, _end) = store.poles(handle).ok()?;
            (start == theory)
                .then(|| store.export_anum(handle).ok())
                .flatten()
        })
        .collect::<Vec<_>>();

    (!admissions.is_empty()).then_some(admissions)
}

pub(crate) fn prepare_stage(
    compiler: &OptimizedLinkStore,
    semantic_roots: Vec<WebProofPreparedRoot>,
    theory_admissions: Vec<String>,
) -> WebProofPrepareStage {
    WebProofPrepareStage {
        compiler_label:
            "portable Aset compiler/preparation state (not runtime A-memory)"
                .to_owned(),
        runtime_memory_exists: false,
        compiled_links: compiler.link_count() as u32,
        aset_anums: export_portable_aset(compiler),
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
    for source in &prepare.aset_anums {
        memory.store.import_anum(source).ok()?;
    }
    let links_after_load = memory.store.link_count() as u32;

    if links_after_load != prepare.compiled_links {
        return None;
    }

    let mut loaded_roots =
        Vec::with_capacity(prepare.semantic_roots.len());
    let mut portable_round_trip = true;

    for root in &prepare.semantic_roots {
        let before = memory.store.link_count();
        let handle = memory.store.import_anum(&root.source).ok()?;
        if memory.store.link_count() != before {
            return None;
        }
        if memory.store.export_anum(handle).ok().as_deref()
            != Some(root.source.as_str())
        {
            portable_round_trip = false;
        }
        loaded_roots.push(WebProofLoadedRoot {
            role: root.role.clone(),
            source: root.source.clone(),
            local_handle: handle,
        });
    }

    let load = WebProofLoadStage {
        memory_instance_id: memory.id.clone(),
        links_before_load,
        links_after_load,
        imported_anums: prepare.aset_anums.len() as u32,
        portable_round_trip,
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
