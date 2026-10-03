use amemory_optimized_cpu_probe::{
    structural::{
        export_direct_recursive_wire_from, read_exact_sequence,
        OptimizedStructuralEngine,
    },
    OptimizedLinkStore,
};
use serde::{Deserialize, Serialize};
use std::{env, fs, process};

const SCHEMA: &str = "mts-v015-recursive-execution-package/v0.1";

#[derive(Debug, Deserialize)]
struct Package {
    schema: String,
    links: Vec<String>,
    entry: String,
    #[serde(rename = "negativeEntry")]
    negative_entry: Option<String>,
}

#[derive(Debug, Serialize)]
struct RunEvidence {
    index: usize,
    current: String,
    expected: String,
    result: String,
    raw_rule_matches: u32,
    transitioned_members: u32,
    quiescent: bool,
    handoff_count: u32,
}

#[derive(Debug, Serialize)]
struct ReplayEvidence {
    schema: &'static str,
    runs: Vec<RunEvidence>,
    negative_run: Option<RunEvidence>,
}

fn wire(store: &OptimizedLinkStore, handle: u32) -> Result<String, String> {
    export_direct_recursive_wire_from(store, handle)
        .map_err(|error| format!("recursive export failed: {error:?}"))
}

fn replay_launch(
    store: &mut OptimizedLinkStore,
    launch: u32,
    index: usize,
) -> Result<RunEvidence, String> {
    let fields = read_exact_sequence(store, launch)
        .map_err(|error| format!("launch {index}: invalid ExactSequence: {error:?}"))?;
    if fields.len() != 3 {
        return Err(format!("launch {index}: expected [interpreter,current,expected]"));
    }
    let interpreter = fields[0];
    let current = fields[1];
    let expected = fields[2];

    let mut engine = OptimizedStructuralEngine::new(64);
    engine
        .set_interpreter(store, interpreter)
        .map_err(|error| format!("launch {index}: interpreter rejected: {error:?}"))?;
    engine
        .set_current(store, &[current])
        .map_err(|error| format!("launch {index}: current rejected: {error:?}"))?;
    let reaction = engine
        .run(store)
        .map_err(|error| format!("launch {index}: execution failed: {error:?}"))?;

    if reaction.next_members.len() != 1 {
        return Err(format!(
            "launch {index}: expected one result, got {}",
            reaction.next_members.len()
        ));
    }
    let result = reaction.next_members[0];
    if result != expected {
        return Err(format!(
            "launch {index}: result mismatch: {} != {}",
            wire(store, result)?,
            wire(store, expected)?
        ));
    }

    Ok(RunEvidence {
        index,
        current: wire(store, current)?,
        expected: wire(store, expected)?,
        result: wire(store, result)?,
        raw_rule_matches: reaction.raw_rule_matches,
        transitioned_members: reaction.transitioned_members,
        quiescent: reaction.quiescent,
        handoff_count: reaction.handoff_count,
    })
}

fn execute(input: &str) -> Result<ReplayEvidence, String> {
    let package: Package =
        serde_json::from_str(input).map_err(|error| format!("invalid package JSON: {error}"))?;
    if package.schema != SCHEMA {
        return Err(format!("unsupported package schema: {}", package.schema));
    }

    let mut store = OptimizedLinkStore::new();
    for (index, source) in package.links.iter().enumerate() {
        store
            .import_anum(source)
            .map_err(|error| format!("link {index}: invalid recursive wire: {error:?}"))?;
    }

    let entry = store
        .import_anum(&package.entry)
        .map_err(|error| format!("invalid entry wire: {error:?}"))?;
    let launches = read_exact_sequence(&store, entry)
        .map_err(|error| format!("entry is not an ExactSequence: {error:?}"))?;
    if launches.is_empty() {
        return Err("entry contains no launches".to_owned());
    }

    let mut runs = Vec::with_capacity(launches.len());
    for (index, launch) in launches.into_iter().enumerate() {
        runs.push(replay_launch(&mut store, launch, index)?);
    }

    let negative_run = match package.negative_entry {
        Some(source) => {
            let launch = store
                .import_anum(&source)
                .map_err(|error| format!("invalid negative entry wire: {error:?}"))?;
            Some(replay_launch(&mut store, launch, runs.len())?)
        }
        None => None,
    };

    Ok(ReplayEvidence {
        schema: "mts-v015-recursive-execution-evidence/v0.1",
        runs,
        negative_run,
    })
}

fn main() {
    let Some(path) = env::args().nth(1) else {
        eprintln!("usage: formal_recursive_replay <package.json>");
        process::exit(2);
    };
    let input = match fs::read_to_string(&path) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("cannot read {path}: {error}");
            process::exit(2);
        }
    };
    match execute(&input) {
        Ok(evidence) => println!("{}", serde_json::to_string(&evidence).unwrap()),
        Err(error) => {
            eprintln!("{error}");
            process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_schema() {
        let error = execute(r#"{"schema":"wrong","links":[],"entry":"8"}"#)
            .expect_err("unknown schema must fail");
        assert!(error.contains("unsupported package schema"));
    }
}
