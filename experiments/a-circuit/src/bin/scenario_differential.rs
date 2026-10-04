#[cfg(not(target_family = "wasm"))]
use std::{
    env, fs,
    io::{self, Read},
    process,
};

#[cfg(not(target_family = "wasm"))]
fn read_manifest_source() -> Result<String, String> {
    if let Some(path) = env::args_os().nth(1) {
        return fs::read_to_string(&path).map_err(|error| {
            format!("MANIFEST_READ_FAILED: {}: {error}", path.to_string_lossy())
        });
    }

    let mut source = String::new();
    io::stdin()
        .read_to_string(&mut source)
        .map_err(|error| format!("MANIFEST_STDIN_READ_FAILED: {error}"))?;
    Ok(source)
}

#[cfg(not(target_family = "wasm"))]
fn main() {
    let source = match read_manifest_source() {
        Ok(source) => source,
        Err(error) => {
            eprintln!("{error}");
            process::exit(2);
        }
    };

    match amemory_a_circuit::run_native_cpu_linksdb_differential_json_v1(
        &source,
    ) {
        Ok(report) => println!("{report}"),
        Err(error) => {
            eprintln!("{error}");
            process::exit(1);
        }
    }
}

// Cargo builds bin targets for the WASM target as part of release verification.
// Keep this target compileable there without pretending that native LinksDB
// execution is available in WASM.
#[cfg(target_family = "wasm")]
fn main() {}
