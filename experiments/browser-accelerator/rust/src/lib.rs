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
            if self.used[i] != 0
                && self.start[i] == handle
                && self.end[i] == child
                && self.end[i] != handle
            {
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
            if self.used[i] != 0
                && self.start[i] == child
                && self.end[i] == handle
                && self.start[i] != handle
            {
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
            if self.used[i] != 0
                && self.start[i] == start
                && self.end[i] == end
                && self.start[i] != handle
                && self.end[i] != handle
            {
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


const REACTION_SCOPE_CAP: usize = 16;
const REACTION_NONE: u32 = u32::MAX;

static mut REACTION_SCOPE0: [u32; REACTION_SCOPE_CAP] = [REACTION_NONE; REACTION_SCOPE_CAP];
static mut REACTION_SCOPE1: [u32; REACTION_SCOPE_CAP] = [REACTION_NONE; REACTION_SCOPE_CAP];
static mut REACTION_SCOPE0_COUNT: u32 = 0;
static mut REACTION_SCOPE1_COUNT: u32 = 0;
static mut REACTION_CURRENT_BANK: u32 = 0;

static mut REACTION_THEORY: [u32; REACTION_SCOPE_CAP] = [REACTION_NONE; REACTION_SCOPE_CAP];
static mut REACTION_THEORY_COUNT: u32 = 0;
static mut REACTION_SNAPSHOT: [u32; REACTION_SCOPE_CAP] = [REACTION_NONE; REACTION_SCOPE_CAP];
static mut REACTION_SNAPSHOT_COUNT: u32 = 0;

static mut REACTION_MATCHED_RELATIONS: u32 = 0;
static mut REACTION_HANDOFF_COUNT: u32 = 0;
static mut REACTION_QUIESCENT: u32 = 0;

fn reaction_pair_poles(pool: &AnumCpuPool, handle: u32) -> Option<(u32, u32)> {
    if !pool.valid(handle) {
        return None;
    }
    let i = handle as usize;
    let start = pool.start[i];
    let end = pool.end[i];

    // The bounded R1 executor consumes ordinary PAIR Links as current truths
    // and grounded Theory relations. ROOT/START/END remain valid Links but are
    // not silently reinterpreted as binary reaction records.
    if start == handle || end == handle || !pool.valid(start) || !pool.valid(end) {
        return None;
    }
    Some((start, end))
}

fn reaction_append_unique(
    values: &mut [u32; REACTION_SCOPE_CAP],
    count: &mut usize,
    handle: u32,
) -> bool {
    let mut i = 0;
    while i < *count {
        if values[i] == handle {
            return true;
        }
        i += 1;
    }
    if *count >= REACTION_SCOPE_CAP {
        return false;
    }
    values[*count] = handle;
    *count += 1;
    true
}

#[no_mangle]
pub extern "C" fn amemory_reaction_reset() {
    unsafe {
        let mut i = 0;
        while i < REACTION_SCOPE_CAP {
            REACTION_SCOPE0[i] = REACTION_NONE;
            REACTION_SCOPE1[i] = REACTION_NONE;
            REACTION_THEORY[i] = REACTION_NONE;
            REACTION_SNAPSHOT[i] = REACTION_NONE;
            i += 1;
        }
        REACTION_SCOPE0_COUNT = 0;
        REACTION_SCOPE1_COUNT = 0;
        REACTION_CURRENT_BANK = 0;
        REACTION_THEORY_COUNT = 0;
        REACTION_SNAPSHOT_COUNT = 0;
        REACTION_MATCHED_RELATIONS = 0;
        REACTION_HANDOFF_COUNT = 0;
        REACTION_QUIESCENT = 0;
    }
}

#[no_mangle]
pub extern "C" fn amemory_reaction_set_current_member(index: u32, handle: u32) -> u32 {
    let i = index as usize;
    if i >= REACTION_SCOPE_CAP || !AnumCpuPool::snapshot().valid(handle) {
        return 0;
    }
    unsafe {
        if REACTION_CURRENT_BANK == 0 {
            REACTION_SCOPE0[i] = handle;
        } else {
            REACTION_SCOPE1[i] = handle;
        }
    }
    1
}

#[no_mangle]
pub extern "C" fn amemory_reaction_set_current_count(count: u32) -> u32 {
    if count as usize > REACTION_SCOPE_CAP {
        return 0;
    }
    unsafe {
        if REACTION_CURRENT_BANK == 0 {
            REACTION_SCOPE0_COUNT = count;
        } else {
            REACTION_SCOPE1_COUNT = count;
        }
    }
    1
}

#[no_mangle]
pub extern "C" fn amemory_reaction_set_theory_relation(index: u32, handle: u32) -> u32 {
    let i = index as usize;
    if i >= REACTION_SCOPE_CAP || reaction_pair_poles(&AnumCpuPool::snapshot(), handle).is_none() {
        return 0;
    }
    unsafe {
        REACTION_THEORY[i] = handle;
    }
    1
}

#[no_mangle]
pub extern "C" fn amemory_reaction_set_theory_count(count: u32) -> u32 {
    if count as usize > REACTION_SCOPE_CAP {
        return 0;
    }
    unsafe {
        REACTION_THEORY_COUNT = count;
    }
    1
}

/// Capture the explicit reaction-start TheorySnapshot_t.
///
/// Later writes to REACTION_THEORY do not alter this snapshot. The snapshot is
/// substrate state for the portable visibility boundary; it is not a new MTS
/// entity or portable identity.
#[no_mangle]
pub extern "C" fn amemory_reaction_snapshot_theory() -> u32 {
    let pool = AnumCpuPool::snapshot();
    let count = unsafe { REACTION_THEORY_COUNT as usize };
    if count > REACTION_SCOPE_CAP {
        return 0;
    }

    let mut scratch = [REACTION_NONE; REACTION_SCOPE_CAP];
    let mut i = 0;
    while i < count {
        let relation = unsafe { REACTION_THEORY[i] };
        if reaction_pair_poles(&pool, relation).is_none() {
            return 0;
        }
        scratch[i] = relation;
        i += 1;
    }

    unsafe {
        let mut j = 0;
        while j < REACTION_SCOPE_CAP {
            REACTION_SNAPSHOT[j] = scratch[j];
            j += 1;
        }
        REACTION_SNAPSHOT_COUNT = count as u32;
    }
    1
}

/// Execute one bounded grounded reaction against the captured TheorySnapshot_t.
///
/// Successor members are assembled in local scratch first. Only after the
/// complete successor is valid are they copied into the non-current Scope bank
/// and the current-bank selector is switched exactly once.
///
/// For matchedRelations == 0, the published Scope is unchanged and no handoff
/// occurs. This behavior is useful as a fail-closed control in R1, while the
/// complete NO_ADMITTED_RELATION/quiescence profile remains a later AM-C045
/// slice.
#[no_mangle]
pub extern "C" fn amemory_reaction_run() -> u32 {
    // Quiescence is a semantic result of a successful complete reaction evaluation,
    // never a stale scheduler/runtime condition. Clear it before any fail-closed exit.
    unsafe { REACTION_QUIESCENT = 0; }

    let pool = AnumCpuPool::snapshot();

    let current_bank = unsafe { REACTION_CURRENT_BANK };
    if current_bank > 1 {
        return 0;
    }
    let current_count = unsafe {
        if current_bank == 0 {
            REACTION_SCOPE0_COUNT as usize
        } else {
            REACTION_SCOPE1_COUNT as usize
        }
    };
    let snapshot_count = unsafe { REACTION_SNAPSHOT_COUNT as usize };
    if current_count > REACTION_SCOPE_CAP || snapshot_count > REACTION_SCOPE_CAP {
        return 0;
    }

    let mut current = [REACTION_NONE; REACTION_SCOPE_CAP];
    let mut snapshot = [REACTION_NONE; REACTION_SCOPE_CAP];
    let mut i = 0;
    while i < current_count {
        current[i] = unsafe {
            if current_bank == 0 {
                REACTION_SCOPE0[i]
            } else {
                REACTION_SCOPE1[i]
            }
        };
        i += 1;
    }
    let mut j = 0;
    while j < snapshot_count {
        snapshot[j] = unsafe { REACTION_SNAPSHOT[j] };
        j += 1;
    }

    let mut successor = [REACTION_NONE; REACTION_SCOPE_CAP];
    let mut successor_count = 0_usize;
    let mut matched = 0_u32;

    let mut member_index = 0;
    while member_index < current_count {
        let member = current[member_index];
        let Some((context, antecedent)) = reaction_pair_poles(&pool, member) else {
            return 0;
        };

        let mut member_matches = 0_u32;
        let mut relation_index = 0;
        while relation_index < snapshot_count {
            let relation = snapshot[relation_index];
            let Some((relation_antecedent, output)) = reaction_pair_poles(&pool, relation) else {
                return 0;
            };

            if relation_antecedent == antecedent {
                matched = matched.saturating_add(1);
                member_matches = member_matches.saturating_add(1);

                let Some(candidate) = pool.find_pair(context, output) else {
                    // R1 requires the canonical result Link to exist physically
                    // before semantic publication. Missing substrate support
                    // fails closed without switching the current Scope.
                    return 0;
                };
                if !reaction_append_unique(&mut successor, &mut successor_count, candidate) {
                    return 0;
                }
            }
            relation_index += 1;
        }

        if member_matches == 0
            && !reaction_append_unique(&mut successor, &mut successor_count, member)
        {
            return 0;
        }

        member_index += 1;
    }

    unsafe {
        REACTION_MATCHED_RELATIONS = matched;
        REACTION_HANDOFF_COUNT = 0;
    }

    if matched == 0 {
        // P07/P13: a complete successful evaluation with no applicable admitted
        // relation preserves the published Scope, performs no handoff, and is
        // explicitly quiescent. No scheduler or END-state inference is involved.
        unsafe { REACTION_QUIESCENT = 1; }
        return 1;
    }

    let target_bank = 1 - current_bank;

    // Commit the complete successor bank first.
    unsafe {
        let mut k = 0;
        while k < REACTION_SCOPE_CAP {
            if target_bank == 0 {
                REACTION_SCOPE0[k] = successor[k];
            } else {
                REACTION_SCOPE1[k] = successor[k];
            }
            k += 1;
        }
        if target_bank == 0 {
            REACTION_SCOPE0_COUNT = successor_count as u32;
        } else {
            REACTION_SCOPE1_COUNT = successor_count as u32;
        }

        // The single publication boundary for the bounded CPU/WASM prototype.
        REACTION_CURRENT_BANK = target_bank;
        REACTION_HANDOFF_COUNT = 1;
    }

    1
}

#[no_mangle]
pub extern "C" fn amemory_reaction_current_bank() -> u32 {
    unsafe { REACTION_CURRENT_BANK }
}

#[no_mangle]
pub extern "C" fn amemory_reaction_current_count() -> u32 {
    unsafe {
        if REACTION_CURRENT_BANK == 0 {
            REACTION_SCOPE0_COUNT
        } else {
            REACTION_SCOPE1_COUNT
        }
    }
}

#[no_mangle]
pub extern "C" fn amemory_reaction_current_member(index: u32) -> u32 {
    let i = index as usize;
    if i >= REACTION_SCOPE_CAP {
        return REACTION_NONE;
    }
    unsafe {
        let count = if REACTION_CURRENT_BANK == 0 {
            REACTION_SCOPE0_COUNT
        } else {
            REACTION_SCOPE1_COUNT
        };
        if index >= count {
            return REACTION_NONE;
        }
        if REACTION_CURRENT_BANK == 0 {
            REACTION_SCOPE0[i]
        } else {
            REACTION_SCOPE1[i]
        }
    }
}

#[no_mangle]
pub extern "C" fn amemory_reaction_bank_count(bank: u32) -> u32 {
    unsafe {
        match bank {
            0 => REACTION_SCOPE0_COUNT,
            1 => REACTION_SCOPE1_COUNT,
            _ => 0,
        }
    }
}

#[no_mangle]
pub extern "C" fn amemory_reaction_bank_member(bank: u32, index: u32) -> u32 {
    let i = index as usize;
    if i >= REACTION_SCOPE_CAP {
        return REACTION_NONE;
    }
    unsafe {
        let count = match bank {
            0 => REACTION_SCOPE0_COUNT,
            1 => REACTION_SCOPE1_COUNT,
            _ => return REACTION_NONE,
        };
        if index >= count {
            return REACTION_NONE;
        }
        if bank == 0 {
            REACTION_SCOPE0[i]
        } else {
            REACTION_SCOPE1[i]
        }
    }
}

#[no_mangle]
pub extern "C" fn amemory_reaction_snapshot_count() -> u32 {
    unsafe { REACTION_SNAPSHOT_COUNT }
}

#[no_mangle]
pub extern "C" fn amemory_reaction_matched_relations() -> u32 {
    unsafe { REACTION_MATCHED_RELATIONS }
}

#[no_mangle]
pub extern "C" fn amemory_reaction_handoff_count() -> u32 {
    unsafe { REACTION_HANDOFF_COUNT }
}

#[no_mangle]
pub extern "C" fn amemory_reaction_quiescent() -> u32 {
    unsafe { REACTION_QUIESCENT }
}


#[cfg(test)]
mod anum_boundary_tests {
    use super::*;
    use std::sync::Mutex;

    // All current prototype Rust/WASM witnesses share one bounded static pool.
    // Serialize tests so test-runner scheduling cannot become accidental state authority.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn import(source: &str) -> u32 {
        for (i, byte) in source.bytes().enumerate() {
            let token = if byte.is_ascii_digit() {
                (byte - b'0') as u32
            } else {
                255
            };
            assert_eq!(amemory_anum_cpu_set_token(i as u32, token), 1);
        }
        amemory_anum_cpu_import(source.len() as u32)
    }

    fn export(handle: u32) -> String {
        let len = amemory_anum_cpu_export(handle);
        assert_ne!(len, ANUM_CPU_NONE);
        let mut out = String::new();
        for i in 0..len {
            out.push(char::from_digit(amemory_anum_cpu_output_get(i), 10).unwrap());
        }
        out
    }

    #[test]
    fn portable_anum_cpu_boundary() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        amemory_anum_cpu_reset_pool();

        let fixtures = ["8", "98", "68", "19868", "16898", "198698", "119868968"];
        let mut handles = [0_u32; 7];
        for (i, source) in fixtures.iter().enumerate() {
            let handle = import(source);
            assert_ne!(handle, ANUM_CPU_NONE, "valid fixture rejected: {source}");
            assert_eq!(export(handle), *source);
            handles[i] = handle;
        }

        // ROOT must not be mistaken for START or END merely because ROOT=(self,self).
        assert_ne!(handles[0], handles[1]);
        assert_ne!(handles[0], handles[2]);
        assert_eq!(export(handles[0]), "8");
        assert_eq!(export(handles[1]), "98");
        assert_eq!(export(handles[2]), "68");

        // Canonical local reuse.
        assert_eq!(import("19868"), handles[3]);

        // Invalid/truncated/trailing forms are transactional.
        let stable_count = amemory_anum_cpu_pool_count();
        for source in ["5", "1", "9", "88", "19868x"] {
            assert_eq!(import(source), ANUM_CPU_NONE, "invalid fixture accepted: {source}");
            assert_eq!(amemory_anum_cpu_pool_count(), stable_count);
        }

        // A valid but out-of-prototype chain must fail without partial publication.
        let capacity = format!("{}8", "9".repeat(70));
        assert_eq!(import(&capacity), ANUM_CPU_NONE);
        assert_eq!(amemory_anum_cpu_pool_count(), stable_count);
    }

    #[test]
    fn minimal_grounded_reaction_r1() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        amemory_anum_cpu_reset_pool();

        let k = import("98");
        let a = import("68");
        let b = import("16898");
        let current = import("19868");
        let relation = import("16816898");
        let successor = import("19816898");

        for handle in [k, a, b, current, relation, successor] {
            assert_ne!(handle, ANUM_CPU_NONE);
        }
        assert_eq!(export(current), "19868");
        assert_eq!(export(relation), "16816898");
        assert_eq!(export(successor), "19816898");

        // Positive R1 with explicit reaction-start snapshot.
        amemory_reaction_reset();
        assert_eq!(amemory_reaction_set_current_member(0, current), 1);
        assert_eq!(amemory_reaction_set_current_count(1), 1);
        assert_eq!(amemory_reaction_set_theory_relation(0, relation), 1);
        assert_eq!(amemory_reaction_set_theory_count(1), 1);
        assert_eq!(amemory_reaction_snapshot_theory(), 1);
        assert_eq!(amemory_reaction_snapshot_count(), 1);

        // Mutating Theory storage after snapshot must not alter reaction t.
        // successor itself is K->B, therefore its antecedent K does not match A.
        assert_eq!(amemory_reaction_set_theory_relation(0, successor), 1);

        assert_eq!(amemory_reaction_current_bank(), 0);
        assert_eq!(export(amemory_reaction_current_member(0)), "19868");
        assert_eq!(amemory_reaction_run(), 1);
        assert_eq!(amemory_reaction_matched_relations(), 1);
        assert_eq!(amemory_reaction_handoff_count(), 1);
        assert_eq!(amemory_reaction_quiescent(), 0);
        assert_eq!(amemory_reaction_current_bank(), 1);
        assert_eq!(amemory_reaction_current_count(), 1);
        assert_eq!(export(amemory_reaction_current_member(0)), "19816898");

        // The old Scope bank and old current truth remain physically available.
        assert_eq!(amemory_reaction_bank_count(0), 1);
        assert_eq!(export(amemory_reaction_bank_member(0, 0)), "19868");
        assert_eq!(export(current), "19868");

        // R2 P07/P13: a valid admitted relation with the wrong antecedent
        // yields NO_ADMITTED_RELATION for this current truth.
        amemory_reaction_reset();
        assert_eq!(amemory_reaction_set_current_member(0, current), 1);
        assert_eq!(amemory_reaction_set_current_count(1), 1);
        assert_eq!(amemory_reaction_set_theory_relation(0, successor), 1);
        assert_eq!(amemory_reaction_set_theory_count(1), 1);
        assert_eq!(amemory_reaction_snapshot_theory(), 1);
        assert_eq!(amemory_reaction_run(), 1);
        assert_eq!(amemory_reaction_matched_relations(), 0);
        assert_eq!(amemory_reaction_handoff_count(), 0);
        assert_eq!(amemory_reaction_quiescent(), 1);
        assert_eq!(amemory_reaction_current_bank(), 0);
        assert_eq!(export(amemory_reaction_current_member(0)), "19868");

        // Runtime/fail-closed failure is not semantic quiescence.
        amemory_reaction_reset();
        assert_eq!(amemory_reaction_set_current_count(1), 1);
        assert_eq!(amemory_reaction_set_theory_relation(0, successor), 1);
        assert_eq!(amemory_reaction_set_theory_count(1), 1);
        assert_eq!(amemory_reaction_snapshot_theory(), 1);
        assert_eq!(amemory_reaction_run(), 0);
        assert_eq!(amemory_reaction_quiescent(), 0);
    }

}
