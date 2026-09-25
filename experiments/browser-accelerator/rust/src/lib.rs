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


const ANUM_CPU_SLOTS: usize = 64;
const ANUM_CPU_MAX_HANDLE: u32 = 63;
const ANUM_CPU_ROOT: u32 = 1;
const ANUM_CPU_MAX_TOKENS: usize = 256;
const ANUM_CPU_NONE: u32 = u32::MAX;

static mut ANUM_CPU_START: [u32; ANUM_CPU_SLOTS] = [0; ANUM_CPU_SLOTS];
static mut ANUM_CPU_END: [u32; ANUM_CPU_SLOTS] = [0; ANUM_CPU_SLOTS];
static mut ANUM_CPU_USED: [u32; ANUM_CPU_SLOTS] = [0; ANUM_CPU_SLOTS];
static mut ANUM_CPU_INPUT: [u32; ANUM_CPU_MAX_TOKENS] = [0; ANUM_CPU_MAX_TOKENS];
static mut ANUM_CPU_OUTPUT: [u32; ANUM_CPU_MAX_TOKENS] = [0; ANUM_CPU_MAX_TOKENS];

#[derive(Clone, Copy)]
struct AnumCpuPool {
    start: [u32; ANUM_CPU_SLOTS],
    end: [u32; ANUM_CPU_SLOTS],
    used: [u32; ANUM_CPU_SLOTS],
}

impl AnumCpuPool {
    fn snapshot() -> Self {
        let mut pool = Self {
            start: [0; ANUM_CPU_SLOTS],
            end: [0; ANUM_CPU_SLOTS],
            used: [0; ANUM_CPU_SLOTS],
        };
        let mut i = 0;
        while i < ANUM_CPU_SLOTS {
            unsafe {
                pool.start[i] = ANUM_CPU_START[i];
                pool.end[i] = ANUM_CPU_END[i];
                pool.used[i] = ANUM_CPU_USED[i];
            }
            i += 1;
        }
        pool
    }

    fn publish(&self) {
        let mut i = 0;
        while i < ANUM_CPU_SLOTS {
            unsafe {
                ANUM_CPU_START[i] = self.start[i];
                ANUM_CPU_END[i] = self.end[i];
                ANUM_CPU_USED[i] = self.used[i];
            }
            i += 1;
        }
    }

    fn valid(&self, handle: u32) -> bool {
        handle >= 1
            && handle <= ANUM_CPU_MAX_HANDLE
            && self.used[handle as usize] != 0
    }

    fn find_start(&self, child: u32) -> Option<u32> {
        let mut handle = 1;
        while handle <= ANUM_CPU_MAX_HANDLE {
            let i = handle as usize;
            if self.used[i] != 0 && self.start[i] == handle && self.end[i] == child {
                return Some(handle);
            }
            handle += 1;
        }
        None
    }

    fn find_end(&self, child: u32) -> Option<u32> {
        let mut handle = 1;
        while handle <= ANUM_CPU_MAX_HANDLE {
            let i = handle as usize;
            if self.used[i] != 0 && self.start[i] == child && self.end[i] == handle {
                return Some(handle);
            }
            handle += 1;
        }
        None
    }

    fn find_pair(&self, start: u32, end: u32) -> Option<u32> {
        let mut handle = 1;
        while handle <= ANUM_CPU_MAX_HANDLE {
            let i = handle as usize;
            if self.used[i] != 0 && self.start[i] == start && self.end[i] == end {
                return Some(handle);
            }
            handle += 1;
        }
        None
    }

    fn allocate(&mut self, start: u32, end: u32, aspect: u32) -> Option<u32> {
        let mut handle = 2;
        while handle <= ANUM_CPU_MAX_HANDLE {
            let i = handle as usize;
            if self.used[i] == 0 {
                self.used[i] = 1;
                if aspect == 1 {
                    self.start[i] = handle;
                    self.end[i] = end;
                } else if aspect == 2 {
                    self.start[i] = start;
                    self.end[i] = handle;
                } else {
                    self.start[i] = start;
                    self.end[i] = end;
                }
                return Some(handle);
            }
            handle += 1;
        }
        None
    }

    fn ensure_start(&mut self, child: u32) -> Option<u32> {
        if let Some(handle) = self.find_start(child) {
            return Some(handle);
        }
        self.allocate(0, child, 1)
    }

    fn ensure_end(&mut self, child: u32) -> Option<u32> {
        if let Some(handle) = self.find_end(child) {
            return Some(handle);
        }
        self.allocate(child, 0, 2)
    }

    fn ensure_pair(&mut self, start: u32, end: u32) -> Option<u32> {
        if let Some(handle) = self.find_pair(start, end) {
            return Some(handle);
        }
        self.allocate(start, end, 3)
    }
}

fn anum_cpu_import_node(
    tokens: &[u32],
    cursor: &mut usize,
    pool: &mut AnumCpuPool,
) -> Option<u32> {
    if *cursor >= tokens.len() {
        return None;
    }

    let token = tokens[*cursor];
    *cursor += 1;

    match token {
        8 => Some(ANUM_CPU_ROOT),
        9 => {
            let child = anum_cpu_import_node(tokens, cursor, pool)?;
            pool.ensure_start(child)
        }
        6 => {
            let child = anum_cpu_import_node(tokens, cursor, pool)?;
            pool.ensure_end(child)
        }
        1 => {
            let start = anum_cpu_import_node(tokens, cursor, pool)?;
            let end = anum_cpu_import_node(tokens, cursor, pool)?;
            pool.ensure_pair(start, end)
        }
        _ => None,
    }
}

fn anum_cpu_push_output(output: &mut [u32; ANUM_CPU_MAX_TOKENS], len: &mut usize, token: u32) -> bool {
    if *len >= output.len() {
        return false;
    }
    output[*len] = token;
    *len += 1;
    true
}

fn anum_cpu_export_node(
    pool: &AnumCpuPool,
    handle: u32,
    visiting: &mut [bool; ANUM_CPU_SLOTS],
    output: &mut [u32; ANUM_CPU_MAX_TOKENS],
    len: &mut usize,
) -> bool {
    if !pool.valid(handle) {
        return false;
    }

    let i = handle as usize;
    if visiting[i] {
        return false;
    }
    visiting[i] = true;

    let start = pool.start[i];
    let end = pool.end[i];

    let ok = if start == handle && end == handle {
        anum_cpu_push_output(output, len, 8)
    } else if start == handle {
        anum_cpu_push_output(output, len, 9)
            && anum_cpu_export_node(pool, end, visiting, output, len)
    } else if end == handle {
        anum_cpu_push_output(output, len, 6)
            && anum_cpu_export_node(pool, start, visiting, output, len)
    } else {
        anum_cpu_push_output(output, len, 1)
            && anum_cpu_export_node(pool, start, visiting, output, len)
            && anum_cpu_export_node(pool, end, visiting, output, len)
    };

    visiting[i] = false;
    ok
}

#[no_mangle]
pub extern "C" fn amemory_anum_cpu_reset_pool() {
    unsafe {
        let mut i = 0;
        while i < ANUM_CPU_SLOTS {
            ANUM_CPU_START[i] = 0;
            ANUM_CPU_END[i] = 0;
            ANUM_CPU_USED[i] = 0;
            i += 1;
        }
        ANUM_CPU_USED[ANUM_CPU_ROOT as usize] = 1;
        ANUM_CPU_START[ANUM_CPU_ROOT as usize] = ANUM_CPU_ROOT;
        ANUM_CPU_END[ANUM_CPU_ROOT as usize] = ANUM_CPU_ROOT;
    }
}

#[no_mangle]
pub extern "C" fn amemory_anum_cpu_set_token(index: u32, token: u32) -> u32 {
    let i = index as usize;
    if i >= ANUM_CPU_MAX_TOKENS {
        return 0;
    }
    unsafe {
        ANUM_CPU_INPUT[i] = token;
    }
    1
}

/// Transactional bounded import. The global pool is published only after the
/// complete prefix Anum is consumed and a local handle is obtained.
#[no_mangle]
pub extern "C" fn amemory_anum_cpu_import(token_count: u32) -> u32 {
    let count = token_count as usize;
    if count == 0 || count > ANUM_CPU_MAX_TOKENS {
        return ANUM_CPU_NONE;
    }

    let mut tokens = [0_u32; ANUM_CPU_MAX_TOKENS];
    let mut i = 0;
    while i < count {
        unsafe {
            tokens[i] = ANUM_CPU_INPUT[i];
        }
        i += 1;
    }

    let mut scratch = AnumCpuPool::snapshot();
    let mut cursor = 0;
    let Some(handle) = anum_cpu_import_node(&tokens[..count], &mut cursor, &mut scratch) else {
        return ANUM_CPU_NONE;
    };
    if cursor != count {
        return ANUM_CPU_NONE;
    }

    scratch.publish();
    handle
}

#[no_mangle]
pub extern "C" fn amemory_anum_cpu_export(handle: u32) -> u32 {
    let pool = AnumCpuPool::snapshot();
    let mut visiting = [false; ANUM_CPU_SLOTS];
    let mut output = [0_u32; ANUM_CPU_MAX_TOKENS];
    let mut len = 0_usize;

    if !anum_cpu_export_node(&pool, handle, &mut visiting, &mut output, &mut len) {
        return ANUM_CPU_NONE;
    }

    let mut i = 0;
    while i < len {
        unsafe {
            ANUM_CPU_OUTPUT[i] = output[i];
        }
        i += 1;
    }
    len as u32
}

#[no_mangle]
pub extern "C" fn amemory_anum_cpu_output_get(index: u32) -> u32 {
    let i = index as usize;
    if i >= ANUM_CPU_MAX_TOKENS {
        return ANUM_CPU_NONE;
    }
    unsafe { ANUM_CPU_OUTPUT[i] }
}

#[no_mangle]
pub extern "C" fn amemory_anum_cpu_pool_count() -> u32 {
    let mut count = 0_u32;
    let mut handle = 1_u32;
    while handle <= ANUM_CPU_MAX_HANDLE {
        unsafe {
            if ANUM_CPU_USED[handle as usize] != 0 {
                count += 1;
            }
        }
        handle += 1;
    }
    count
}
