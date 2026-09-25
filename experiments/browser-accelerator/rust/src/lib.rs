#[no_mangle]
pub extern "C" fn amemory_probe() -> u32 {
    0xA013
}

#[no_mangle]
pub extern "C" fn amemory_cpu_step(value: u32) -> u32 {
    value.wrapping_add(1)
}

/// Reference CPU/WASM oracle for the first A-memory-specific differential witness.
///
/// bit 0: START pole matches query_start
/// bit 1: END pole matches query_end
/// bit 2: both poles match
#[no_mangle]
pub extern "C" fn amemory_incidence_flag(
    start: u32,
    end: u32,
    query_start: u32,
    query_end: u32,
) -> u32 {
    let mut flags = 0_u32;
    if start == query_start {
        flags |= 1;
    }
    if end == query_end {
        flags |= 2;
    }
    if flags & 3 == 3 {
        flags |= 4;
    }
    flags
}
