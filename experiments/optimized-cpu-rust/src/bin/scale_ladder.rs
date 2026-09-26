use amemory_optimized_cpu_probe::{
    OptimizedLinkStore, OptimizedReactionEngine, ROOT_HANDLE,
};
use std::hint::black_box;
use std::time::Instant;

fn usage() -> ! {
    eprintln!("usage: scale_ladder <storage|reaction> <size> <probes>");
    std::process::exit(2);
}

fn arg_usize(value: Option<String>) -> usize {
    value
        .unwrap_or_else(|| usage())
        .parse::<usize>()
        .unwrap_or_else(|_| usage())
}

fn linux_memory_kb(field: &str) -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        if parts.next()? != field {
            return None;
        }
        parts.next()?.parse::<u64>().ok()
    })
}

fn storage_scale(size: usize, probes: usize) {
    let mut store = OptimizedLinkStore::new();

    let build_started = Instant::now();
    let mut current = ROOT_HANDLE;
    for _ in 0..size {
        current = store.ensure_pair(ROOT_HANDLE, current).unwrap();
    }
    let build_elapsed = build_started.elapsed();
    assert_eq!(store.link_count(), size + 1);

    let (_, target_end) = store.poles(current).unwrap();

    let hit_started = Instant::now();
    let mut observed = ROOT_HANDLE;
    for _ in 0..probes {
        observed = store.ensure_pair(ROOT_HANDLE, target_end).unwrap();
        black_box(observed);
    }
    let hit_elapsed = hit_started.elapsed();
    assert_eq!(observed, current);
    assert_eq!(store.link_count(), size + 1);

    // Validate full list cardinality once. P6 intentionally does not store
    // per-handle counts, so full counting is O(k).
    assert_eq!(
        store.start_incidence(ROOT_HANDLE).unwrap().count(),
        size + 1
    );

    // Timed probe is O(1) entry into the incidence chain.
    let incidence_started = Instant::now();
    let mut incidence_head = 0u32;
    for _ in 0..probes {
        incidence_head = store
            .start_incidence(ROOT_HANDLE)
            .unwrap()
            .next()
            .unwrap_or(0);
        black_box(incidence_head);
    }
    let incidence_elapsed = incidence_started.elapsed();
    assert_ne!(incidence_head, 0);

    println!("OPT_CPU_SCALE_MODE=storage");
    println!("OPT_CPU_SCALE_LINKS={}", store.link_count());
    println!("OPT_CPU_SCALE_PROBES={probes}");
    println!(
        "OPT_CPU_SCALE_BUILD_NS_PER_LINK={}",
        build_elapsed.as_nanos() / size.max(1) as u128
    );
    println!(
        "OPT_CPU_SCALE_CANONICAL_HIT_NS_PER_OP={}",
        hit_elapsed.as_nanos() / probes.max(1) as u128
    );
    println!(
        "OPT_CPU_SCALE_START_INCIDENCE_NS_PER_OP={}",
        incidence_elapsed.as_nanos() / probes.max(1) as u128
    );
    if let Some(rss) = linux_memory_kb("VmRSS:") {
        println!("OPT_CPU_SCALE_RSS_KB={rss}");
    }
    if let Some(hwm) = linux_memory_kb("VmHWM:") {
        println!("OPT_CPU_SCALE_HWM_KB={hwm}");
    }
}

fn reaction_scale(size: usize, probes: usize) {
    let mut store = OptimizedLinkStore::new();

    // Build N distinct antecedents as ordinary Links.
    let mut antecedents = Vec::with_capacity(size);
    let mut previous = ROOT_HANDLE;
    for _ in 0..size {
        previous = store.ensure_pair(ROOT_HANDLE, previous).unwrap();
        antecedents.push(previous);
    }
    assert!(!antecedents.is_empty());

    // Each relation maps one antecedent to ROOT (empty contribution). Only one
    // relation is applicable to the selected current truth.
    let mut theory = Vec::with_capacity(size);
    for antecedent in &antecedents {
        theory.push(store.ensure_pair(*antecedent, ROOT_HANDLE).unwrap());
    }

    let target_index = if size > 1 { size / 2 - (size / 2 == size - 1) as usize } else { 0 };
    let target_antecedent = antecedents[target_index];
    let current = store.ensure_pair(ROOT_HANDLE, target_antecedent).unwrap();

    let mut engine = OptimizedReactionEngine::new(size + 4);
    engine.set_current(&store, &[current]).unwrap();
    engine.set_theory(&store, &theory).unwrap();

    let snapshot_started = Instant::now();
    engine.snapshot_theory(&store).unwrap();
    let snapshot_elapsed = snapshot_started.elapsed();
    assert_eq!(engine.snapshot_count(), size);

    // Matching run with the prebuilt snapshot index.
    let matched_started = Instant::now();
    for _ in 0..probes {
        engine.set_current(&store, &[current]).unwrap();
        engine.run(&store).unwrap();
        black_box(engine.matched_relations());
    }
    let matched_elapsed = matched_started.elapsed();
    assert_eq!(engine.matched_relations(), 1);
    assert_eq!(engine.handoff_count(), 1);
    assert!(!engine.quiescent());

    // A structurally valid current truth whose antecedent has no admitted
    // relation must hit the same snapshot index and become quiescent.
    let unmatched_antecedent = store.import_anum("998").unwrap();
    let unmatched_current = store
        .ensure_pair(ROOT_HANDLE, unmatched_antecedent)
        .unwrap();

    let quiescent_started = Instant::now();
    for _ in 0..probes {
        engine.set_current(&store, &[unmatched_current]).unwrap();
        engine.run(&store).unwrap();
        black_box(engine.quiescent());
    }
    let quiescent_elapsed = quiescent_started.elapsed();
    assert_eq!(engine.matched_relations(), 0);
    assert_eq!(engine.handoff_count(), 0);
    assert!(engine.quiescent());

    println!("OPT_CPU_SCALE_MODE=reaction");
    println!("OPT_CPU_SCALE_THEORY_RELATIONS={size}");
    println!("OPT_CPU_SCALE_LINKS={}", store.link_count());
    println!("OPT_CPU_SCALE_PROBES={probes}");
    println!(
        "OPT_CPU_SCALE_SNAPSHOT_NS_PER_RELATION={}",
        snapshot_elapsed.as_nanos() / size.max(1) as u128
    );
    println!(
        "OPT_CPU_SCALE_MATCHED_RUN_NS_PER_OP={}",
        matched_elapsed.as_nanos() / probes.max(1) as u128
    );
    println!(
        "OPT_CPU_SCALE_QUIESCENT_RUN_NS_PER_OP={}",
        quiescent_elapsed.as_nanos() / probes.max(1) as u128
    );
    if let Some(rss) = linux_memory_kb("VmRSS:") {
        println!("OPT_CPU_SCALE_RSS_KB={rss}");
    }
    if let Some(hwm) = linux_memory_kb("VmHWM:") {
        println!("OPT_CPU_SCALE_HWM_KB={hwm}");
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mode = args.next().unwrap_or_else(|| usage());
    let size = arg_usize(args.next());
    let probes = arg_usize(args.next());
    if size == 0 || probes == 0 || args.next().is_some() {
        usage();
    }

    match mode.as_str() {
        "storage" => storage_scale(size, probes),
        "reaction" => reaction_scale(size, probes),
        _ => usage(),
    }
}
