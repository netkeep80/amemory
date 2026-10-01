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

const ANUM_CPU_SLOTS: usize = 64;
const ANUM_CPU_MAX_HANDLE: u32 = 63;
const ANUM_CPU_ROOT: u32 = 1;
const ANUM_CPU_MAX_TOKENS: usize = 256;
const ANUM_CPU_NONE: u32 = u32::MAX;
const REACTION_SCOPE_CAP: usize = 16;
const REACTION_NONE: u32 = u32::MAX;

#[derive(Clone, Copy)]
struct AnumCpuPool {
    start: [u32; ANUM_CPU_SLOTS],
    end: [u32; ANUM_CPU_SLOTS],
    used: [u32; ANUM_CPU_SLOTS],
}

impl AnumCpuPool {
    fn root_only() -> Self {
        let mut pool = Self {
            start: [0; ANUM_CPU_SLOTS],
            end: [0; ANUM_CPU_SLOTS],
            used: [0; ANUM_CPU_SLOTS],
        };
        pool.used[ANUM_CPU_ROOT as usize] = 1;
        pool.start[ANUM_CPU_ROOT as usize] = ANUM_CPU_ROOT;
        pool.end[ANUM_CPU_ROOT as usize] = ANUM_CPU_ROOT;
        pool
    }

    fn valid(&self, handle: u32) -> bool {
        handle >= 1
            && handle <= ANUM_CPU_MAX_HANDLE
            && self.used[handle as usize] != 0
    }

    fn find_start(&self, child: u32) -> Option<u32> {
        (1..=ANUM_CPU_MAX_HANDLE).find(|handle| {
            let i = *handle as usize;
            self.used[i] != 0
                && self.start[i] == *handle
                && self.end[i] == child
                && self.end[i] != *handle
        })
    }

    fn find_end(&self, child: u32) -> Option<u32> {
        (1..=ANUM_CPU_MAX_HANDLE).find(|handle| {
            let i = *handle as usize;
            self.used[i] != 0
                && self.start[i] == child
                && self.end[i] == *handle
                && self.start[i] != *handle
        })
    }

    fn find_pair(&self, start: u32, end: u32) -> Option<u32> {
        (1..=ANUM_CPU_MAX_HANDLE).find(|handle| {
            let i = *handle as usize;
            self.used[i] != 0
                && self.start[i] == start
                && self.end[i] == end
                && self.start[i] != *handle
                && self.end[i] != *handle
        })
    }

    fn allocate(&mut self, start: u32, end: u32, aspect: u32) -> Option<u32> {
        for handle in 2..=ANUM_CPU_MAX_HANDLE {
            let i = handle as usize;
            if self.used[i] == 0 {
                self.used[i] = 1;
                match aspect {
                    1 => {
                        self.start[i] = handle;
                        self.end[i] = end;
                    }
                    2 => {
                        self.start[i] = start;
                        self.end[i] = handle;
                    }
                    _ => {
                        self.start[i] = start;
                        self.end[i] = end;
                    }
                }
                return Some(handle);
            }
        }
        None
    }

    fn ensure_start(&mut self, child: u32) -> Option<u32> {
        self.find_start(child).or_else(|| self.allocate(0, child, 1))
    }

    fn ensure_end(&mut self, child: u32) -> Option<u32> {
        self.find_end(child).or_else(|| self.allocate(child, 0, 2))
    }

    fn ensure_pair(&mut self, start: u32, end: u32) -> Option<u32> {
        self.find_pair(start, end)
            .or_else(|| self.allocate(start, end, 3))
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

fn anum_cpu_push_output(
    output: &mut [u32; ANUM_CPU_MAX_TOKENS],
    len: &mut usize,
    token: u32,
) -> bool {
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

fn reaction_pair_poles(pool: &AnumCpuPool, handle: u32) -> Option<(u32, u32)> {
    if !pool.valid(handle) {
        return None;
    }
    let i = handle as usize;
    let start = pool.start[i];
    let end = pool.end[i];
    if start == handle || end == handle || !pool.valid(start) || !pool.valid(end) {
        return None;
    }
    Some((start, end))
}

fn reaction_is_root(pool: &AnumCpuPool, handle: u32) -> bool {
    if !pool.valid(handle) {
        return false;
    }
    let i = handle as usize;
    pool.start[i] == handle && pool.end[i] == handle
}

fn reaction_append_unique(
    values: &mut [u32; REACTION_SCOPE_CAP],
    count: &mut usize,
    handle: u32,
) -> bool {
    if values[..*count].contains(&handle) {
        return true;
    }
    if *count >= REACTION_SCOPE_CAP {
        return false;
    }
    values[*count] = handle;
    *count += 1;
    true
}

#[derive(Clone)]
struct ReferenceReactionState {
    scope: [[u32; REACTION_SCOPE_CAP]; 2],
    scope_count: [u32; 2],
    current_bank: u32,
    theory: [u32; REACTION_SCOPE_CAP],
    theory_count: u32,
    snapshot: [u32; REACTION_SCOPE_CAP],
    snapshot_count: u32,
    matched_relations: u32,
    handoff_count: u32,
    quiescent: u32,
}

impl ReferenceReactionState {
    fn new() -> Self {
        Self {
            scope: [[REACTION_NONE; REACTION_SCOPE_CAP]; 2],
            scope_count: [0; 2],
            current_bank: 0,
            theory: [REACTION_NONE; REACTION_SCOPE_CAP],
            theory_count: 0,
            snapshot: [REACTION_NONE; REACTION_SCOPE_CAP],
            snapshot_count: 0,
            matched_relations: 0,
            handoff_count: 0,
            quiescent: 0,
        }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }
}

/// Explicit owner for one bounded reference CPU/WASM A-memory.
///
/// Local handles and instance ids are physical identity only. They are not
/// portable semantic identity and do not alter the execution profile.
pub struct ReferenceMemoryInstance {
    pool: AnumCpuPool,
    input: [u32; ANUM_CPU_MAX_TOKENS],
    output: [u32; ANUM_CPU_MAX_TOKENS],
    stage: Option<AnumCpuPool>,
    stage_member_count: u32,
    reaction: ReferenceReactionState,
}

impl Default for ReferenceMemoryInstance {
    fn default() -> Self {
        Self::new()
    }
}

impl ReferenceMemoryInstance {
    pub fn new() -> Self {
        Self {
            pool: AnumCpuPool::root_only(),
            input: [0; ANUM_CPU_MAX_TOKENS],
            output: [0; ANUM_CPU_MAX_TOKENS],
            stage: None,
            stage_member_count: 0,
            reaction: ReferenceReactionState::new(),
        }
    }

    pub fn reset_pool(&mut self) {
        self.pool = AnumCpuPool::root_only();
        self.stage = None;
        self.stage_member_count = 0;
    }

    pub fn set_token(&mut self, index: u32, token: u32) -> u32 {
        let i = index as usize;
        if i >= ANUM_CPU_MAX_TOKENS {
            return 0;
        }
        self.input[i] = token;
        1
    }

    pub fn load_begin(&mut self) -> u32 {
        self.stage = Some(AnumCpuPool::root_only());
        self.stage_member_count = 0;
        1
    }

    pub fn load_member(&mut self, token_count: u32) -> u32 {
        let count = token_count as usize;
        if self.stage.is_none() || count == 0 || count > ANUM_CPU_MAX_TOKENS {
            return ANUM_CPU_NONE;
        }

        let mut scratch = self.stage.expect("stage checked above");
        let mut cursor = 0;
        let Some(handle) =
            anum_cpu_import_node(&self.input[..count], &mut cursor, &mut scratch)
        else {
            return ANUM_CPU_NONE;
        };
        if cursor != count {
            return ANUM_CPU_NONE;
        }

        self.stage = Some(scratch);
        self.stage_member_count += 1;
        handle
    }

    pub fn load_commit(&mut self) -> u32 {
        if self.stage_member_count == 0 {
            return 0;
        }
        let Some(stage) = self.stage.take() else {
            return 0;
        };
        self.pool = stage;
        self.stage_member_count = 0;
        1
    }

    pub fn load_abort(&mut self) {
        self.stage = None;
        self.stage_member_count = 0;
    }

    pub fn load_active(&self) -> u32 {
        u32::from(self.stage.is_some())
    }

    pub fn import_tokens(&mut self, token_count: u32) -> u32 {
        if self.stage.is_some() {
            return ANUM_CPU_NONE;
        }
        let count = token_count as usize;
        if count == 0 || count > ANUM_CPU_MAX_TOKENS {
            return ANUM_CPU_NONE;
        }

        let mut scratch = self.pool;
        let mut cursor = 0;
        let Some(handle) =
            anum_cpu_import_node(&self.input[..count], &mut cursor, &mut scratch)
        else {
            return ANUM_CPU_NONE;
        };
        if cursor != count {
            return ANUM_CPU_NONE;
        }

        self.pool = scratch;
        handle
    }

    pub fn import_recursive_wire(&mut self, source: &str) -> u32 {
        if source.len() > ANUM_CPU_MAX_TOKENS {
            return ANUM_CPU_NONE;
        }
        for (index, byte) in source.bytes().enumerate() {
            let token = if byte.is_ascii_digit() {
                (byte - b'0') as u32
            } else {
                255
            };
            if self.set_token(index as u32, token) != 1 {
                return ANUM_CPU_NONE;
            }
        }
        self.import_tokens(source.len() as u32)
    }

    pub fn stage_recursive_wire(&mut self, source: &str) -> u32 {
        if source.len() > ANUM_CPU_MAX_TOKENS {
            return ANUM_CPU_NONE;
        }
        for (index, byte) in source.bytes().enumerate() {
            let token = if byte.is_ascii_digit() {
                (byte - b'0') as u32
            } else {
                255
            };
            if self.set_token(index as u32, token) != 1 {
                return ANUM_CPU_NONE;
            }
        }
        self.load_member(source.len() as u32)
    }

    pub fn export_tokens(&mut self, handle: u32) -> u32 {
        let mut visiting = [false; ANUM_CPU_SLOTS];
        let mut output = [0_u32; ANUM_CPU_MAX_TOKENS];
        let mut len = 0_usize;
        if !anum_cpu_export_node(
            &self.pool,
            handle,
            &mut visiting,
            &mut output,
            &mut len,
        ) {
            return ANUM_CPU_NONE;
        }
        self.output[..len].copy_from_slice(&output[..len]);
        len as u32
    }

    pub fn export_recursive_wire(&mut self, handle: u32) -> Option<String> {
        let len = self.export_tokens(handle);
        if len == ANUM_CPU_NONE {
            return None;
        }
        let mut out = String::new();
        for index in 0..len {
            out.push(char::from_digit(self.output_get(index), 10)?);
        }
        Some(out)
    }

    pub fn output_get(&self, index: u32) -> u32 {
        self.output
            .get(index as usize)
            .copied()
            .unwrap_or(ANUM_CPU_NONE)
    }

    pub fn pool_count(&self) -> u32 {
        (1..=ANUM_CPU_MAX_HANDLE)
            .filter(|handle| self.pool.used[*handle as usize] != 0)
            .count() as u32
    }

    pub fn used(&self, handle: u32) -> u32 {
        if handle == 0 || handle > ANUM_CPU_MAX_HANDLE {
            return ANUM_CPU_NONE;
        }
        self.pool.used[handle as usize]
    }

    pub fn start(&self, handle: u32) -> u32 {
        if handle == 0 || handle > ANUM_CPU_MAX_HANDLE {
            return ANUM_CPU_NONE;
        }
        if self.pool.used[handle as usize] == 0 {
            ANUM_CPU_NONE
        } else {
            self.pool.start[handle as usize]
        }
    }

    pub fn end(&self, handle: u32) -> u32 {
        if handle == 0 || handle > ANUM_CPU_MAX_HANDLE {
            return ANUM_CPU_NONE;
        }
        if self.pool.used[handle as usize] == 0 {
            ANUM_CPU_NONE
        } else {
            self.pool.end[handle as usize]
        }
    }

    pub fn reaction_reset(&mut self) {
        self.reaction.reset();
    }

    pub fn reaction_set_current_member(&mut self, index: u32, handle: u32) -> u32 {
        let i = index as usize;
        if i >= REACTION_SCOPE_CAP || !self.pool.valid(handle) {
            return 0;
        }
        let bank = self.reaction.current_bank as usize;
        self.reaction.scope[bank][i] = handle;
        1
    }

    pub fn reaction_set_current_count(&mut self, count: u32) -> u32 {
        if count as usize > REACTION_SCOPE_CAP {
            return 0;
        }
        self.reaction.scope_count[self.reaction.current_bank as usize] = count;
        1
    }

    pub fn reaction_set_theory_relation(&mut self, index: u32, handle: u32) -> u32 {
        let i = index as usize;
        if i >= REACTION_SCOPE_CAP
            || reaction_pair_poles(&self.pool, handle).is_none()
        {
            return 0;
        }
        self.reaction.theory[i] = handle;
        1
    }

    pub fn reaction_set_theory_count(&mut self, count: u32) -> u32 {
        if count as usize > REACTION_SCOPE_CAP {
            return 0;
        }
        self.reaction.theory_count = count;
        1
    }

    pub fn reaction_snapshot_theory(&mut self) -> u32 {
        let count = self.reaction.theory_count as usize;
        if count > REACTION_SCOPE_CAP {
            return 0;
        }

        let mut scratch = [REACTION_NONE; REACTION_SCOPE_CAP];
        for (index, relation) in self.reaction.theory[..count].iter().copied().enumerate() {
            if reaction_pair_poles(&self.pool, relation).is_none() {
                return 0;
            }
            scratch[index] = relation;
        }
        self.reaction.snapshot = scratch;
        self.reaction.snapshot_count = count as u32;
        1
    }

    pub fn reaction_run(&mut self) -> u32 {
        self.reaction.quiescent = 0;
        if self.stage.is_some() {
            return 0;
        }

        let pool = self.pool;
        let current_bank = self.reaction.current_bank as usize;
        if current_bank > 1 {
            return 0;
        }
        let current_count = self.reaction.scope_count[current_bank] as usize;
        let snapshot_count = self.reaction.snapshot_count as usize;
        if current_count > REACTION_SCOPE_CAP || snapshot_count > REACTION_SCOPE_CAP {
            return 0;
        }

        let current = self.reaction.scope[current_bank];
        let snapshot = self.reaction.snapshot;
        let mut successor = [REACTION_NONE; REACTION_SCOPE_CAP];
        let mut successor_count = 0_usize;
        let mut matched = 0_u32;

        for member in current[..current_count].iter().copied() {
            let Some((context, antecedent)) = reaction_pair_poles(&pool, member) else {
                return 0;
            };

            let mut member_matches = 0_u32;
            for relation in snapshot[..snapshot_count].iter().copied() {
                let Some((relation_antecedent, output)) =
                    reaction_pair_poles(&pool, relation)
                else {
                    return 0;
                };

                if relation_antecedent == antecedent {
                    matched = matched.saturating_add(1);
                    member_matches = member_matches.saturating_add(1);

                    if !reaction_is_root(&pool, output) {
                        let Some(candidate) = pool.find_pair(context, output) else {
                            return 0;
                        };
                        if !reaction_append_unique(
                            &mut successor,
                            &mut successor_count,
                            candidate,
                        ) {
                            return 0;
                        }
                    }
                }
            }

            if member_matches == 0
                && !reaction_append_unique(
                    &mut successor,
                    &mut successor_count,
                    member,
                )
            {
                return 0;
            }
        }

        self.reaction.matched_relations = matched;
        self.reaction.handoff_count = 0;

        if matched == 0 {
            self.reaction.quiescent = 1;
            return 1;
        }

        let target_bank = 1 - current_bank;
        self.reaction.scope[target_bank] = successor;
        self.reaction.scope_count[target_bank] = successor_count as u32;
        self.reaction.current_bank = target_bank as u32;
        self.reaction.handoff_count = 1;
        1
    }

    pub fn reaction_current_bank(&self) -> u32 {
        self.reaction.current_bank
    }

    pub fn reaction_current_count(&self) -> u32 {
        self.reaction.scope_count[self.reaction.current_bank as usize]
    }

    pub fn reaction_current_member(&self, index: u32) -> u32 {
        self.reaction_bank_member(self.reaction.current_bank, index)
    }

    pub fn reaction_bank_count(&self, bank: u32) -> u32 {
        self.reaction
            .scope_count
            .get(bank as usize)
            .copied()
            .unwrap_or(0)
    }

    pub fn reaction_bank_member(&self, bank: u32, index: u32) -> u32 {
        let Some(count) = self.reaction.scope_count.get(bank as usize).copied() else {
            return REACTION_NONE;
        };
        if index >= count {
            return REACTION_NONE;
        }
        self.reaction.scope[bank as usize]
            .get(index as usize)
            .copied()
            .unwrap_or(REACTION_NONE)
    }

    pub fn reaction_theory_count(&self) -> u32 {
        self.reaction.theory_count
    }

    pub fn reaction_snapshot_count(&self) -> u32 {
        self.reaction.snapshot_count
    }

    pub fn reaction_matched_relations(&self) -> u32 {
        self.reaction.matched_relations
    }

    pub fn reaction_handoff_count(&self) -> u32 {
        self.reaction.handoff_count
    }

    pub fn reaction_quiescent(&self) -> u32 {
        self.reaction.quiescent
    }
}

struct ReferenceMemorySlot {
    generation: u16,
    instance: Option<ReferenceMemoryInstance>,
}

static REFERENCE_MEMORY_ARENA: std::sync::Mutex<Vec<ReferenceMemorySlot>> =
    std::sync::Mutex::new(Vec::new());

fn reference_instance_id(index: usize, generation: u16) -> u32 {
    ((generation as u32) << 16) | ((index as u32) + 1)
}

fn reference_instance_parts(id: u32) -> Option<(usize, u16)> {
    let slot = (id & 0xffff) as usize;
    let generation = (id >> 16) as u16;
    if slot == 0 || generation == 0 {
        None
    } else {
        Some((slot - 1, generation))
    }
}

fn with_reference_instance_mut<T>(
    id: u32,
    f: impl FnOnce(&mut ReferenceMemoryInstance) -> T,
) -> Option<T> {
    let (index, generation) = reference_instance_parts(id)?;
    let mut arena = REFERENCE_MEMORY_ARENA
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let slot = arena.get_mut(index)?;
    if slot.generation != generation {
        return None;
    }
    slot.instance.as_mut().map(f)
}

#[no_mangle]
pub extern "C" fn amemory_reference_create() -> u32 {
    let mut arena = REFERENCE_MEMORY_ARENA
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    for (index, slot) in arena.iter_mut().enumerate() {
        if slot.instance.is_none() {
            let next = slot.generation.wrapping_add(1);
            slot.generation = if next == 0 { 1 } else { next };
            slot.instance = Some(ReferenceMemoryInstance::new());
            return reference_instance_id(index, slot.generation);
        }
    }

    if arena.len() >= u16::MAX as usize {
        return 0;
    }
    arena.push(ReferenceMemorySlot {
        generation: 1,
        instance: Some(ReferenceMemoryInstance::new()),
    });
    reference_instance_id(arena.len() - 1, 1)
}

#[no_mangle]
pub extern "C" fn amemory_reference_destroy(id: u32) -> u32 {
    let Some((index, generation)) = reference_instance_parts(id) else {
        return 0;
    };
    let mut arena = REFERENCE_MEMORY_ARENA
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(slot) = arena.get_mut(index) else {
        return 0;
    };
    if slot.generation != generation || slot.instance.is_none() {
        return 0;
    }
    slot.instance = None;
    1
}

macro_rules! reference_mut_export {
    ($name:ident, $method:ident, ($($arg:ident : $ty:ty),*), $invalid:expr) => {
        #[no_mangle]
        pub extern "C" fn $name(id: u32, $($arg: $ty),*) -> u32 {
            with_reference_instance_mut(id, |instance| instance.$method($($arg),*))
                .unwrap_or($invalid)
        }
    };
}

reference_mut_export!(amemory_reference_reset_pool, reset_pool_export, (), 0);

impl ReferenceMemoryInstance {
    fn reset_pool_export(&mut self) -> u32 {
        self.reset_pool();
        1
    }
    fn load_abort_export(&mut self) -> u32 {
        self.load_abort();
        1
    }
    fn reaction_reset_export(&mut self) -> u32 {
        self.reaction_reset();
        1
    }
}

reference_mut_export!(amemory_reference_set_token, set_token, (index: u32, token: u32), 0);
reference_mut_export!(amemory_reference_load_begin, load_begin, (), 0);
reference_mut_export!(amemory_reference_load_member, load_member, (token_count: u32), ANUM_CPU_NONE);
reference_mut_export!(amemory_reference_load_commit, load_commit, (), 0);
reference_mut_export!(amemory_reference_load_abort, load_abort_export, (), 0);
reference_mut_export!(amemory_reference_load_active, load_active, (), ANUM_CPU_NONE);
reference_mut_export!(amemory_reference_import, import_tokens, (token_count: u32), ANUM_CPU_NONE);
reference_mut_export!(amemory_reference_export, export_tokens, (handle: u32), ANUM_CPU_NONE);
reference_mut_export!(amemory_reference_output_get, output_get, (index: u32), ANUM_CPU_NONE);
reference_mut_export!(amemory_reference_pool_count, pool_count, (), ANUM_CPU_NONE);
reference_mut_export!(amemory_reference_used, used, (handle: u32), ANUM_CPU_NONE);
reference_mut_export!(amemory_reference_start, start, (handle: u32), ANUM_CPU_NONE);
reference_mut_export!(amemory_reference_end, end, (handle: u32), ANUM_CPU_NONE);
reference_mut_export!(amemory_reference_reaction_reset, reaction_reset_export, (), 0);
reference_mut_export!(
    amemory_reference_reaction_set_current_member,
    reaction_set_current_member,
    (index: u32, handle: u32),
    0
);
reference_mut_export!(
    amemory_reference_reaction_set_current_count,
    reaction_set_current_count,
    (count: u32),
    0
);
reference_mut_export!(
    amemory_reference_reaction_set_theory_relation,
    reaction_set_theory_relation,
    (index: u32, handle: u32),
    0
);
reference_mut_export!(
    amemory_reference_reaction_set_theory_count,
    reaction_set_theory_count,
    (count: u32),
    0
);
reference_mut_export!(
    amemory_reference_reaction_snapshot_theory,
    reaction_snapshot_theory,
    (),
    0
);
reference_mut_export!(amemory_reference_reaction_run, reaction_run, (), 0);
reference_mut_export!(
    amemory_reference_reaction_current_bank,
    reaction_current_bank,
    (),
    ANUM_CPU_NONE
);
reference_mut_export!(
    amemory_reference_reaction_current_count,
    reaction_current_count,
    (),
    ANUM_CPU_NONE
);
reference_mut_export!(
    amemory_reference_reaction_current_member,
    reaction_current_member,
    (index: u32),
    ANUM_CPU_NONE
);
reference_mut_export!(
    amemory_reference_reaction_bank_count,
    reaction_bank_count,
    (bank: u32),
    ANUM_CPU_NONE
);
reference_mut_export!(
    amemory_reference_reaction_bank_member,
    reaction_bank_member,
    (bank: u32, index: u32),
    ANUM_CPU_NONE
);
reference_mut_export!(
    amemory_reference_reaction_theory_count,
    reaction_theory_count,
    (),
    ANUM_CPU_NONE
);
reference_mut_export!(
    amemory_reference_reaction_snapshot_count,
    reaction_snapshot_count,
    (),
    ANUM_CPU_NONE
);
reference_mut_export!(
    amemory_reference_reaction_matched_relations,
    reaction_matched_relations,
    (),
    ANUM_CPU_NONE
);
reference_mut_export!(
    amemory_reference_reaction_handoff_count,
    reaction_handoff_count,
    (),
    ANUM_CPU_NONE
);
reference_mut_export!(
    amemory_reference_reaction_quiescent,
    reaction_quiescent,
    (),
    ANUM_CPU_NONE
);

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

    fn stage_import(source: &str) -> u32 {
        for (i, byte) in source.bytes().enumerate() {
            let token = if byte.is_ascii_digit() {
                (byte - b'0') as u32
            } else {
                255
            };
            assert_eq!(amemory_anum_cpu_set_token(i as u32, token), 1);
        }
        amemory_anum_cpu_load_member(source.len() as u32)
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
    fn cpu_topology_getters_are_read_only_technical_coordinates() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        amemory_anum_cpu_reset_pool();

        let start_marker = import("98");
        let end_marker = import("68");
        assert_ne!(start_marker, ANUM_CPU_NONE);
        assert_ne!(end_marker, ANUM_CPU_NONE);

        assert_eq!(amemory_anum_cpu_used(ANUM_CPU_ROOT), 1);
        assert_eq!(amemory_anum_cpu_start(ANUM_CPU_ROOT), ANUM_CPU_ROOT);
        assert_eq!(amemory_anum_cpu_end(ANUM_CPU_ROOT), ANUM_CPU_ROOT);

        assert_eq!(amemory_anum_cpu_used(start_marker), 1);
        assert_eq!(amemory_anum_cpu_start(start_marker), start_marker);
        assert_eq!(amemory_anum_cpu_end(start_marker), ANUM_CPU_ROOT);

        assert_eq!(amemory_anum_cpu_used(end_marker), 1);
        assert_eq!(amemory_anum_cpu_start(end_marker), ANUM_CPU_ROOT);
        assert_eq!(amemory_anum_cpu_end(end_marker), end_marker);

        assert_eq!(amemory_anum_cpu_used(63), 0);
        assert_eq!(amemory_anum_cpu_start(63), ANUM_CPU_NONE);
        assert_eq!(amemory_anum_cpu_end(63), ANUM_CPU_NONE);
        assert_eq!(amemory_anum_cpu_used(64), ANUM_CPU_NONE);
    }

    #[test]
    fn atomic_multi_anum_aset_load() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        amemory_anum_cpu_reset_pool();

        let baseline = import("998");
        assert_ne!(baseline, ANUM_CPU_NONE);
        let baseline_count = amemory_anum_cpu_pool_count();
        assert_eq!(export(baseline), "998");

        assert_eq!(amemory_anum_cpu_load_begin(), 1);
        assert_eq!(amemory_anum_cpu_load_active(), 1);

        for (i, byte) in "98".bytes().enumerate() {
            assert_eq!(amemory_anum_cpu_set_token(i as u32, (byte - b'0') as u32), 1);
        }
        assert_ne!(amemory_anum_cpu_load_member(2), ANUM_CPU_NONE);

        assert_eq!(amemory_anum_cpu_set_token(0, 5), 1);
        assert_eq!(amemory_anum_cpu_load_member(1), ANUM_CPU_NONE);
        assert_eq!(amemory_anum_cpu_pool_count(), baseline_count);
        assert_eq!(export(baseline), "998");

        // No ordinary publication or execution may bypass LOADING.
        assert_eq!(import("68"), ANUM_CPU_NONE);
        assert_eq!(amemory_reaction_run(), 0);
        assert_eq!(amemory_reaction_quiescent(), 0);

        amemory_anum_cpu_load_abort();
        assert_eq!(amemory_anum_cpu_load_active(), 0);
        assert_eq!(amemory_anum_cpu_pool_count(), baseline_count);

        assert_eq!(amemory_anum_cpu_load_begin(), 1);
        let mut staged = [ANUM_CPU_NONE; 3];
        for (index, source) in ["98", "68", "19868"].iter().enumerate() {
            for (i, byte) in source.bytes().enumerate() {
                assert_eq!(amemory_anum_cpu_set_token(i as u32, (byte - b'0') as u32), 1);
            }
            staged[index] = amemory_anum_cpu_load_member(source.len() as u32);
            assert_ne!(staged[index], ANUM_CPU_NONE);
        }

        // New Aset is still invisible until the single commit.
        assert_eq!(amemory_anum_cpu_pool_count(), baseline_count);
        assert_eq!(amemory_anum_cpu_load_commit(), 1);
        assert_eq!(amemory_anum_cpu_load_active(), 0);

        // ROOT + START(ROOT) + END(ROOT) + PAIR(START, END).
        assert_eq!(amemory_anum_cpu_pool_count(), 4);
        assert_eq!(export(staged[0]), "98");
        assert_eq!(export(staged[1]), "68");
        assert_eq!(export(staged[2]), "19868");

        assert_eq!(amemory_anum_cpu_load_begin(), 1);
        assert_eq!(amemory_anum_cpu_load_commit(), 0);
        amemory_anum_cpu_load_abort();
        assert_eq!(amemory_anum_cpu_pool_count(), 4);
    }

    #[test]
    fn atomic_aset_load_executes_without_reimport() {
        let _guard = TEST_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        amemory_anum_cpu_reset_pool();

        assert_eq!(amemory_anum_cpu_load_begin(), 1);
        let k = stage_import("98");
        let a = stage_import("68");
        let b = stage_import("16898");
        let current = stage_import("19868");
        let relation = stage_import("16816898");
        let successor = stage_import("19816898");
        for handle in [k, a, b, current, relation, successor] {
            assert_ne!(handle, ANUM_CPU_NONE);
        }

        assert_eq!(amemory_anum_cpu_load_commit(), 1);
        assert_eq!(amemory_anum_cpu_load_active(), 0);
        assert_eq!(export(current), "19868");
        assert_eq!(export(relation), "16816898");
        assert_eq!(export(successor), "19816898");

        // Configure and execute directly against handles reconstructed by the
        // committed Aset transaction. No ordinary import occurs after commit.
        amemory_reaction_reset();
        assert_eq!(amemory_reaction_set_current_member(0, current), 1);
        assert_eq!(amemory_reaction_set_current_count(1), 1);
        assert_eq!(amemory_reaction_set_theory_relation(0, relation), 1);
        assert_eq!(amemory_reaction_set_theory_count(1), 1);
        assert_eq!(amemory_reaction_snapshot_theory(), 1);

        // The bounded executor currently requires the canonical successor Link
        // to be present in the loaded A-network; it was part of the batch above.
        assert_eq!(amemory_reaction_run(), 1);
        assert_eq!(amemory_reaction_matched_relations(), 1);
        assert_eq!(amemory_reaction_handoff_count(), 1);
        assert_eq!(amemory_reaction_quiescent(), 0);
        assert_eq!(amemory_reaction_current_count(), 1);
        assert_eq!(export(amemory_reaction_current_member(0)), "19816898");
    }

    #[test]
    #[ignore = "informational performance baseline; no acceptance threshold"]
    fn reference_cpu_benchmark_baseline() {
        use std::hint::black_box;
        use std::time::Instant;

        let _guard = TEST_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        const ITERS: u128 = 1_000_000;
        const SOURCES: [&str; 6] = ["98", "68", "16898", "19868", "16816898", "19816898"];

        // 1) Atomic multi-Anum load. This includes parsing, canonical lookup,
        // overlapping-substructure convergence and one publication handoff.
        let load_started = Instant::now();
        let mut last_handles = [ANUM_CPU_NONE; 6];
        for _ in 0..ITERS {
            amemory_anum_cpu_reset_pool();
            assert_eq!(amemory_anum_cpu_load_begin(), 1);
            for (index, source) in SOURCES.iter().enumerate() {
                last_handles[index] = stage_import(source);
                assert_ne!(last_handles[index], ANUM_CPU_NONE);
            }
            assert_eq!(amemory_anum_cpu_load_commit(), 1);
            black_box(last_handles);
        }
        let load_ns = load_started.elapsed().as_nanos() / ITERS;

        // Reconstruct one stable published R1 Aset for the operation-level probes.
        amemory_anum_cpu_reset_pool();
        assert_eq!(amemory_anum_cpu_load_begin(), 1);
        for (index, source) in SOURCES.iter().enumerate() {
            last_handles[index] = stage_import(source);
            assert_ne!(last_handles[index], ANUM_CPU_NONE);
        }
        assert_eq!(amemory_anum_cpu_load_commit(), 1);
        let current = last_handles[3];
        let relation = last_handles[4];
        let successor = last_handles[5];

        // 2) One complete bounded R1 reaction, including Scope/Theory setup,
        // TheorySnapshot capture, successor construction and atomic Scope handoff.
        let reaction_started = Instant::now();
        for _ in 0..ITERS {
            amemory_reaction_reset();
            assert_eq!(amemory_reaction_set_current_member(0, current), 1);
            assert_eq!(amemory_reaction_set_current_count(1), 1);
            assert_eq!(amemory_reaction_set_theory_relation(0, relation), 1);
            assert_eq!(amemory_reaction_set_theory_count(1), 1);
            assert_eq!(amemory_reaction_snapshot_theory(), 1);
            assert_eq!(amemory_reaction_run(), 1);
            black_box(amemory_reaction_current_member(0));
        }
        let reaction_ns = reaction_started.elapsed().as_nanos() / ITERS;

        // 3) Portable structural export of the canonical successor.
        let export_started = Instant::now();
        let mut last_export = String::new();
        for _ in 0..ITERS {
            last_export = export(successor);
            black_box(&last_export);
        }
        let export_ns = export_started.elapsed().as_nanos() / ITERS;
        assert_eq!(last_export, "19816898");

        // 4) Full load -> execute -> portable export path.
        let lifecycle_started = Instant::now();
        for _ in 0..ITERS {
            amemory_anum_cpu_reset_pool();
            assert_eq!(amemory_anum_cpu_load_begin(), 1);
            for (index, source) in SOURCES.iter().enumerate() {
                last_handles[index] = stage_import(source);
                assert_ne!(last_handles[index], ANUM_CPU_NONE);
            }
            assert_eq!(amemory_anum_cpu_load_commit(), 1);

            amemory_reaction_reset();
            assert_eq!(amemory_reaction_set_current_member(0, last_handles[3]), 1);
            assert_eq!(amemory_reaction_set_current_count(1), 1);
            assert_eq!(amemory_reaction_set_theory_relation(0, last_handles[4]), 1);
            assert_eq!(amemory_reaction_set_theory_count(1), 1);
            assert_eq!(amemory_reaction_snapshot_theory(), 1);
            assert_eq!(amemory_reaction_run(), 1);
            let result = amemory_reaction_current_member(0);
            assert_eq!(export(result), "19816898");
            black_box(result);
        }
        let lifecycle_ns = lifecycle_started.elapsed().as_nanos() / ITERS;

        println!("REFERENCE_CPU_BASELINE_ITERS={ITERS}");
        println!("REFERENCE_CPU_ATOMIC_LOAD_NS_PER_OP={load_ns}");
        println!("REFERENCE_CPU_R1_REACTION_NS_PER_OP={reaction_ns}");
        println!("REFERENCE_CPU_EXPORT_NS_PER_OP={export_ns}");
        println!("REFERENCE_CPU_END_TO_END_NS_PER_OP={lifecycle_ns}");
        println!("REFERENCE_CPU_BASELINE_NOTE=informational-only-no-performance-threshold");
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

        // R3 ZERO: A -> ExactSequence() is encoded structurally as A -> ROOT.
        let root = import("8");
        let zero_relation = import("1688");
        assert_ne!(root, ANUM_CPU_NONE);
        assert_ne!(zero_relation, ANUM_CPU_NONE);

        amemory_reaction_reset();
        assert_eq!(amemory_reaction_set_current_member(0, current), 1);
        assert_eq!(amemory_reaction_set_current_count(1), 1);
        assert_eq!(amemory_reaction_set_theory_relation(0, zero_relation), 1);
        assert_eq!(amemory_reaction_set_theory_count(1), 1);
        assert_eq!(amemory_reaction_snapshot_theory(), 1);
        let zero_old_bank = amemory_reaction_current_bank();

        assert_eq!(amemory_reaction_run(), 1);
        assert_eq!(amemory_reaction_matched_relations(), 1);
        assert_eq!(amemory_reaction_handoff_count(), 1);
        assert_eq!(amemory_reaction_quiescent(), 0);
        assert_eq!(amemory_reaction_current_count(), 0);
        assert_ne!(amemory_reaction_current_bank(), zero_old_bank);
        assert_eq!(amemory_reaction_bank_count(zero_old_bank), 1);
        assert_eq!(export(amemory_reaction_bank_member(zero_old_bank, 0)), "19868");
        assert_eq!(export(current), "19868");

        // R3 mixed ZERO + duplicate convergence:
        //   current = [K->A, K->R]
        //   theory  = [A->R(ZERO), A->B, R->B]
        // Both non-zero branches derive the same K->B and must converge.
        let current_root = import("1988");
        let root_to_b = import("1816898");
        assert_ne!(current_root, ANUM_CPU_NONE);
        assert_ne!(root_to_b, ANUM_CPU_NONE);

        amemory_reaction_reset();
        assert_eq!(amemory_reaction_set_current_member(0, current), 1);
        assert_eq!(amemory_reaction_set_current_member(1, current_root), 1);
        assert_eq!(amemory_reaction_set_current_count(2), 1);
        assert_eq!(amemory_reaction_set_theory_relation(0, zero_relation), 1);
        assert_eq!(amemory_reaction_set_theory_relation(1, relation), 1);
        assert_eq!(amemory_reaction_set_theory_relation(2, root_to_b), 1);
        assert_eq!(amemory_reaction_set_theory_count(3), 1);
        assert_eq!(amemory_reaction_snapshot_theory(), 1);

        assert_eq!(amemory_reaction_run(), 1);
        assert_eq!(amemory_reaction_matched_relations(), 3);
        assert_eq!(amemory_reaction_handoff_count(), 1);
        assert_eq!(amemory_reaction_quiescent(), 0);
        assert_eq!(amemory_reaction_current_count(), 1);
        assert_eq!(export(amemory_reaction_current_member(0)), "19816898");

        // R6 direct MANY/exhaustiveness witness:
        //   1->N: [K->A] with [A->B, A->C] -> [K->B, K->C]
        //   N->M: [K->A, K->ROOT] with [A->B, ROOT->C] -> [K->B, K->C]
        // Reversing both physical iteration orders must preserve the normalized result.
        let c = import("998");
        let relation_ac = import("168998");
        let root_to_c = import("18998");
        let successor_c = import("198998");
        for handle in [c, relation_ac, root_to_c, successor_c] {
            assert_ne!(handle, ANUM_CPU_NONE);
        }

        amemory_reaction_reset();
        assert_eq!(amemory_reaction_set_current_member(0, current), 1);
        assert_eq!(amemory_reaction_set_current_count(1), 1);
        assert_eq!(amemory_reaction_set_theory_relation(0, relation), 1);
        assert_eq!(amemory_reaction_set_theory_relation(1, relation_ac), 1);
        assert_eq!(amemory_reaction_set_theory_count(2), 1);
        assert_eq!(amemory_reaction_snapshot_theory(), 1);
        assert_eq!(amemory_reaction_run(), 1);
        assert_eq!(amemory_reaction_matched_relations(), 2);
        assert_eq!(amemory_reaction_handoff_count(), 1);
        assert_eq!(amemory_reaction_current_count(), 2);
        let mut one_to_n = [
            export(amemory_reaction_current_member(0)),
            export(amemory_reaction_current_member(1)),
        ];
        one_to_n.sort();
        assert_eq!(one_to_n, ["19816898".to_string(), "198998".to_string()]);

        amemory_reaction_reset();
        assert_eq!(amemory_reaction_set_current_member(0, current), 1);
        assert_eq!(amemory_reaction_set_current_member(1, current_root), 1);
        assert_eq!(amemory_reaction_set_current_count(2), 1);
        assert_eq!(amemory_reaction_set_theory_relation(0, relation), 1);
        assert_eq!(amemory_reaction_set_theory_relation(1, root_to_c), 1);
        assert_eq!(amemory_reaction_set_theory_count(2), 1);
        assert_eq!(amemory_reaction_snapshot_theory(), 1);
        assert_eq!(amemory_reaction_run(), 1);
        assert_eq!(amemory_reaction_matched_relations(), 2);
        assert_eq!(amemory_reaction_current_count(), 2);
        let mut n_to_m = [
            export(amemory_reaction_current_member(0)),
            export(amemory_reaction_current_member(1)),
        ];
        n_to_m.sort();
        assert_eq!(n_to_m, ["19816898".to_string(), "198998".to_string()]);

        amemory_reaction_reset();
        assert_eq!(amemory_reaction_set_current_member(0, current_root), 1);
        assert_eq!(amemory_reaction_set_current_member(1, current), 1);
        assert_eq!(amemory_reaction_set_current_count(2), 1);
        assert_eq!(amemory_reaction_set_theory_relation(0, root_to_c), 1);
        assert_eq!(amemory_reaction_set_theory_relation(1, relation), 1);
        assert_eq!(amemory_reaction_set_theory_count(2), 1);
        assert_eq!(amemory_reaction_snapshot_theory(), 1);
        assert_eq!(amemory_reaction_run(), 1);
        let mut reordered = [
            export(amemory_reaction_current_member(0)),
            export(amemory_reaction_current_member(1)),
        ];
        reordered.sort();
        assert_eq!(reordered, n_to_m);

        // Valid semantic cardinality outside this bounded CPU prototype scope
        // is rejected at the substrate boundary with no handoff/publication.
        amemory_reaction_reset();
        assert_eq!(amemory_reaction_set_current_count(17), 0);
        assert_eq!(amemory_reaction_current_count(), 0);
        assert_eq!(amemory_reaction_handoff_count(), 0);

        // R4 P14: a new live Theory admission added after snapshot_t is
        // invisible to reaction t and becomes executable only after the next
        // explicit reaction-start snapshot.
        let relation_bc = import("116898998");
        for handle in [c, relation_bc, successor_c] {
            assert_ne!(handle, ANUM_CPU_NONE);
        }

        amemory_reaction_reset();
        assert_eq!(amemory_reaction_set_current_member(0, current), 1);
        assert_eq!(amemory_reaction_set_current_count(1), 1);
        assert_eq!(amemory_reaction_set_theory_relation(0, relation), 1);
        assert_eq!(amemory_reaction_set_theory_count(1), 1);
        assert_eq!(amemory_reaction_snapshot_theory(), 1);
        assert_eq!(amemory_reaction_theory_count(), 1);
        assert_eq!(amemory_reaction_snapshot_count(), 1);

        // Add B->C to live Theory only. Snapshot_t remains [A->B].
        assert_eq!(amemory_reaction_set_theory_relation(1, relation_bc), 1);
        assert_eq!(amemory_reaction_set_theory_count(2), 1);
        assert_eq!(amemory_reaction_theory_count(), 2);
        assert_eq!(amemory_reaction_snapshot_count(), 1);

        // reaction t: K->A -> K->B. The new B->C admission must not leak.
        assert_eq!(amemory_reaction_run(), 1);
        assert_eq!(amemory_reaction_matched_relations(), 1);
        assert_eq!(amemory_reaction_handoff_count(), 1);
        assert_eq!(amemory_reaction_quiescent(), 0);
        assert_eq!(export(amemory_reaction_current_member(0)), "19816898");

        // reaction t+1 starts with a fresh snapshot and now B->C is visible.
        assert_eq!(amemory_reaction_snapshot_theory(), 1);
        assert_eq!(amemory_reaction_snapshot_count(), 2);
        assert_eq!(amemory_reaction_run(), 1);
        assert_eq!(amemory_reaction_matched_relations(), 1);
        assert_eq!(amemory_reaction_handoff_count(), 1);
        assert_eq!(amemory_reaction_quiescent(), 0);
        assert_eq!(export(amemory_reaction_current_member(0)), "198998");

        // Negative control on an independent substrate state: adding B->C to
        // live Theory without taking a new snapshot leaves K->B unchanged.
        amemory_reaction_reset();
        assert_eq!(amemory_reaction_set_current_member(0, successor), 1);
        assert_eq!(amemory_reaction_set_current_count(1), 1);
        assert_eq!(amemory_reaction_set_theory_relation(0, relation), 1);
        assert_eq!(amemory_reaction_set_theory_count(1), 1);
        assert_eq!(amemory_reaction_snapshot_theory(), 1);
        assert_eq!(amemory_reaction_set_theory_relation(1, relation_bc), 1);
        assert_eq!(amemory_reaction_set_theory_count(2), 1);
        assert_eq!(amemory_reaction_snapshot_count(), 1);
        assert_eq!(amemory_reaction_run(), 1);
        assert_eq!(amemory_reaction_matched_relations(), 0);
        assert_eq!(amemory_reaction_handoff_count(), 0);
        assert_eq!(amemory_reaction_quiescent(), 1);
        assert_eq!(export(amemory_reaction_current_member(0)), "19816898");

        // R5 P16/P17: bounded observation of an active recurrence through
        // structural END. The unchanged TheorySnapshot contains both
        // directions, so the same semantic states can repeat indefinitely
        // without ever becoming quiescent. END is ordinary structural data,
        // not a global halt condition.
        let r5_context = import("998");
        let r5_start_value = import("98");
        let r5_end_value = import("68");
        let r5_state_start = import("199898");
        let r5_state_end = import("199868");
        let r5_relation_start_end = import("19868");
        let r5_relation_end_start = import("16898");
        for handle in [
            r5_context,
            r5_start_value,
            r5_end_value,
            r5_state_start,
            r5_state_end,
            r5_relation_start_end,
            r5_relation_end_start,
        ] {
            assert_ne!(handle, ANUM_CPU_NONE);
        }

        let pool = AnumCpuPool::snapshot();
        assert_eq!(pool.end[r5_end_value as usize], r5_end_value);
        assert_ne!(pool.start[r5_end_value as usize], r5_end_value);
        assert_eq!(export(r5_end_value), "68");

        amemory_reaction_reset();
        assert_eq!(amemory_reaction_set_current_member(0, r5_state_start), 1);
        assert_eq!(amemory_reaction_set_current_count(1), 1);
        assert_eq!(amemory_reaction_set_theory_relation(0, r5_relation_start_end), 1);
        assert_eq!(amemory_reaction_set_theory_relation(1, r5_relation_end_start), 1);
        assert_eq!(amemory_reaction_set_theory_count(2), 1);
        assert_eq!(amemory_reaction_snapshot_theory(), 1);
        assert_eq!(amemory_reaction_snapshot_count(), 2);

        let expected = ["199898", "199868", "199898", "199868", "199898"];
        assert_eq!(export(amemory_reaction_current_member(0)), expected[0]);
        for step in 0..4 {
            assert_eq!(amemory_reaction_run(), 1);
            assert_eq!(amemory_reaction_matched_relations(), 1);
            assert_eq!(amemory_reaction_handoff_count(), 1);
            assert_eq!(amemory_reaction_quiescent(), 0);
            assert_eq!(export(amemory_reaction_current_member(0)), expected[step + 1]);
        }

        // S0=S2=S4 and S1=S3 semantically, with active transitions between.
        // In particular S1 contains K->END and still transitions to S2.
        assert_eq!(expected[0], expected[2]);
        assert_eq!(expected[2], expected[4]);
        assert_eq!(expected[1], expected[3]);
        assert_eq!(expected[1], "199868");
        assert_eq!(expected[2], "199898");
    }

}
