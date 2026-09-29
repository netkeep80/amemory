use amemory_optimized_cpu_probe::{
    OptimizedLinkStore, OptimizedReactionEngine,
    PackedBinaryIncidenceIndexImage, PackedGpuCarrierImage,
    PackedIncidenceIndexImage, ROOT_HANDLE,
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

    // C3a index-strategy evidence. The canonical store already owns the
    // intrusive baseline; projection timings below measure backend-neutral
    // index images rebuilt from the exact same immutable carrier.
    let carrier = store.export_packed_carrier_image();

    let intrusive_build_started = Instant::now();
    let intrusive = PackedIncidenceIndexImage::from_carrier(&carrier).unwrap();
    let intrusive_build_elapsed = intrusive_build_started.elapsed();

    let intrusive_root_started = Instant::now();
    let mut intrusive_root = 0u32;
    for _ in 0..probes {
        let pole = black_box(ROOT_HANDLE);
        intrusive_root = intrusive.start_heads()[pole as usize];
        black_box(intrusive_root);
    }
    let intrusive_root_elapsed = intrusive_root_started.elapsed();

    let membership_probes =
        probes.min((40_000_000usize / store.link_count().max(1)).max(4));
    let membership_target = |probe: usize| -> u32 {
        let count = store.link_count() as u64;
        let mixed = (probe as u64)
            .wrapping_mul(2_654_435_761)
            .wrapping_add(count / 2);
        1 + (mixed % count) as u32
    };

    let intrusive_membership_started = Instant::now();
    let mut intrusive_found = 0usize;
    for probe in 0..membership_probes {
        let target = black_box(membership_target(probe));
        let found = intrusive
            .start_incidence(ROOT_HANDLE)
            .unwrap()
            .any(|handle| handle == target);
        intrusive_found += usize::from(found);
        black_box(found);
    }
    let intrusive_membership_elapsed = intrusive_membership_started.elapsed();
    assert_eq!(intrusive_found, membership_probes);

    let intrusive_full_started = Instant::now();
    let intrusive_full_count =
        intrusive.start_incidence(ROOT_HANDLE).unwrap().count();
    let intrusive_full_elapsed = intrusive_full_started.elapsed();
    assert_eq!(intrusive_full_count, size + 1);

    let intrusive_words = intrusive.start_heads().len()
        + intrusive.end_heads().len()
        + intrusive.next_by_start().len()
        + intrusive.next_by_end().len();
    drop(intrusive);

    let binary_build_started = Instant::now();
    let binary = PackedBinaryIncidenceIndexImage::from_carrier(&carrier).unwrap();
    let binary_build_elapsed = binary_build_started.elapsed();

    let binary_root_started = Instant::now();
    let mut binary_root = 0u32;
    for _ in 0..probes {
        let pole = black_box(ROOT_HANDLE);
        binary_root = binary.start_root(pole).unwrap();
        black_box(binary_root);
    }
    let binary_root_elapsed = binary_root_started.elapsed();
    assert_ne!(binary_root, 0);

    let binary_membership_started = Instant::now();
    let mut binary_found = 0usize;
    for probe in 0..membership_probes {
        let target = black_box(membership_target(probe));
        let found = binary
            .start_contains(ROOT_HANDLE, target)
            .unwrap();
        binary_found += usize::from(found);
        black_box(found);
    }
    let binary_membership_elapsed = binary_membership_started.elapsed();
    assert_eq!(binary_found, membership_probes);

    let binary_full_started = Instant::now();
    let binary_full_count =
        binary.start_incidence(ROOT_HANDLE).unwrap().count();
    let binary_full_elapsed = binary_full_started.elapsed();
    assert_eq!(binary_full_count, size + 1);

    let binary_words = binary.start_roots().len()
        + binary.end_roots().len()
        + binary.start_left().len()
        + binary.start_right().len()
        + binary.end_left().len()
        + binary.end_right().len();
    drop(binary);

    let gpu_carrier_started = Instant::now();
    let gpu_carrier = store.export_packed_gpu_carrier_image();
    let gpu_carrier_elapsed = gpu_carrier_started.elapsed();
    let parsed_gpu_carrier =
        PackedGpuCarrierImage::from_words(gpu_carrier.words().to_vec()).unwrap();
    assert_eq!(parsed_gpu_carrier.word_len(), gpu_carrier.word_len());

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
    println!(
        "OPT_CPU_SCALE_INTRUSIVE_INDEX_BUILD_NS_PER_LINK={}",
        intrusive_build_elapsed.as_nanos() / store.link_count().max(1) as u128
    );
    println!(
        "OPT_CPU_SCALE_BINARY_INDEX_BUILD_NS_PER_LINK={}",
        binary_build_elapsed.as_nanos() / store.link_count().max(1) as u128
    );
    println!(
        "OPT_CPU_SCALE_INTRUSIVE_ROOT_NS_PER_OP={}",
        intrusive_root_elapsed.as_nanos() / probes.max(1) as u128
    );
    println!(
        "OPT_CPU_SCALE_BINARY_ROOT_NS_PER_OP={}",
        binary_root_elapsed.as_nanos() / probes.max(1) as u128
    );
    println!("OPT_CPU_SCALE_INDEX_MEMBERSHIP_PROBES={membership_probes}");
    println!(
        "OPT_CPU_SCALE_INTRUSIVE_MEMBERSHIP_NS_PER_OP={}",
        intrusive_membership_elapsed.as_nanos()
            / membership_probes.max(1) as u128
    );
    println!(
        "OPT_CPU_SCALE_BINARY_MEMBERSHIP_NS_PER_OP={}",
        binary_membership_elapsed.as_nanos()
            / membership_probes.max(1) as u128
    );
    println!(
        "OPT_CPU_SCALE_INTRUSIVE_FULL_ENUM_NS_PER_LINK={}",
        intrusive_full_elapsed.as_nanos()
            / intrusive_full_count.max(1) as u128
    );
    println!(
        "OPT_CPU_SCALE_BINARY_FULL_ENUM_NS_PER_LINK={}",
        binary_full_elapsed.as_nanos()
            / binary_full_count.max(1) as u128
    );
    println!("OPT_CPU_SCALE_INTRUSIVE_INDEX_WORDS={intrusive_words}");
    println!("OPT_CPU_SCALE_BINARY_INDEX_WORDS={binary_words}");
    println!(
        "OPT_CPU_SCALE_GPU_CARRIER_BUILD_NS_PER_LINK={}",
        gpu_carrier_elapsed.as_nanos() / store.link_count().max(1) as u128
    );
    println!(
        "OPT_CPU_SCALE_GPU_CARRIER_WORDS={}",
        gpu_carrier.word_len()
    );
    println!(
        "OPT_CPU_SCALE_GPU_CARRIER_BYTES={}",
        gpu_carrier.byte_len()
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
