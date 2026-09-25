const MAX_CANONICAL_PAIRS: usize = 64;

static mut EXISTING_START: [u32; MAX_CANONICAL_PAIRS] = [0; MAX_CANONICAL_PAIRS];
static mut EXISTING_END: [u32; MAX_CANONICAL_PAIRS] = [0; MAX_CANONICAL_PAIRS];
static mut CANDIDATE_START: [u32; MAX_CANONICAL_PAIRS] = [0; MAX_CANONICAL_PAIRS];
static mut CANDIDATE_END: [u32; MAX_CANONICAL_PAIRS] = [0; MAX_CANONICAL_PAIRS];

#[no_mangle]
pub extern "C" fn amemory_probe() -> u32 {
    0xA013
}

#[no_mangle]
pub extern "C" fn amemory_cpu_step(value: u32) -> u32 {
    value.wrapping_add(1)
}

/// Reference CPU/WASM oracle for Link incidence matching.
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

#[no_mangle]
pub extern "C" fn amemory_canonical_set_existing(index: u32, start: u32, end: u32) -> u32 {
    let i = index as usize;
    if i >= MAX_CANONICAL_PAIRS {
        return 0;
    }
    unsafe {
        EXISTING_START[i] = start;
        EXISTING_END[i] = end;
    }
    1
}

#[no_mangle]
pub extern "C" fn amemory_canonical_set_candidate(index: u32, start: u32, end: u32) -> u32 {
    let i = index as usize;
    if i >= MAX_CANONICAL_PAIRS {
        return 0;
    }
    unsafe {
        CANDIDATE_START[i] = start;
        CANDIDATE_END[i] = end;
    }
    1
}

/// Reference CPU/WASM canonicalization-plan oracle.
///
/// bit 0: same pair already exists in the committed/existing input
/// bit 1: same pair occurred earlier in this candidate batch
/// bit 2: this occurrence is the first new representative of the pair
///
/// The representative index is a physical batch choice only and is not semantic identity.
#[no_mangle]
pub extern "C" fn amemory_canonical_flag(index: u32, existing_count: u32) -> u32 {
    let i = index as usize;
    let n_existing = existing_count as usize;
    if i >= MAX_CANONICAL_PAIRS || n_existing > MAX_CANONICAL_PAIRS {
        return u32::MAX;
    }

    let (start, end) = unsafe { (CANDIDATE_START[i], CANDIDATE_END[i]) };

    let mut exists = false;
    for e in 0..n_existing {
        let same = unsafe { EXISTING_START[e] == start && EXISTING_END[e] == end };
        if same {
            exists = true;
            break;
        }
    }

    let mut prior = false;
    for j in 0..i {
        let same = unsafe { CANDIDATE_START[j] == start && CANDIDATE_END[j] == end };
        if same {
            prior = true;
            break;
        }
    }

    let mut flags = 0_u32;
    if exists {
        flags |= 1;
    }
    if prior {
        flags |= 2;
    }
    if !exists && !prior {
        flags |= 4;
    }
    flags
}

const STATE_SIDE: u32 = 16;
const STATE_CELLS: usize = (STATE_SIDE as usize) * (STATE_SIDE as usize);
static mut STATE_CELLS_DATA: [u32; STATE_CELLS] = [0; STATE_CELLS];

fn state_index(start: u32, end: u32) -> Option<usize> {
    if start >= STATE_SIDE || end >= STATE_SIDE {
        None
    } else {
        Some((start * STATE_SIDE + end) as usize)
    }
}

#[no_mangle]
pub extern "C" fn amemory_state_reset() {
    unsafe {
        let mut i = 0;
        while i < STATE_CELLS {
            STATE_CELLS_DATA[i] = 0;
            i += 1;
        }
    }
}

/// Commit a pair that has already passed semantic authorization above this storage boundary.
/// Returns 1 on commit/idempotent replay and 0 when the physical prototype scope is exceeded.
#[no_mangle]
pub extern "C" fn amemory_state_commit(start: u32, end: u32) -> u32 {
    let Some(index) = state_index(start, end) else {
        return 0;
    };
    unsafe {
        STATE_CELLS_DATA[index] = 1;
    }
    1
}

/// Returns 1/0 for an in-scope pair and u32::MAX for an out-of-scope physical query.
#[no_mangle]
pub extern "C" fn amemory_state_get(start: u32, end: u32) -> u32 {
    let Some(index) = state_index(start, end) else {
        return u32::MAX;
    };
    unsafe { STATE_CELLS_DATA[index] }
}
