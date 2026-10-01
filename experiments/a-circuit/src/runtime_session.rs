use super::observability::{
    ObservationTimer, RunObservationLevel, RunStructuralFactV1,
    RUN_OBSERVABILITY_SCHEMA_VERSION,
};
use amemory_optimized_cpu_probe::{
    structural::{OptimizedStructuralEngine, StructuralRunProfile},
    Handle, OptimizedLinkStore,
};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};

static NEXT_CPU_MEMORY_ID: AtomicU32 = AtomicU32::new(1);

/// One physical optimized-CPU A-memory instance.
///
/// Program/proof/scenario layers may load data into this instance, but they do
/// not own its identity or carrier semantics.
#[derive(Debug)]
pub(crate) struct CpuMemoryInstance {
    pub(crate) id: String,
    pub(crate) store: OptimizedLinkStore,
}

impl CpuMemoryInstance {
    pub(crate) fn new() -> Self {
        let memory_number =
            NEXT_CPU_MEMORY_ID.fetch_add(1, Ordering::SeqCst);
        Self {
            id: format!("A-memory#{}", memory_number),
            store: OptimizedLinkStore::new(),
        }
    }
}

/// Execution-control state of one long-lived optimized-CPU A-memory Session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum CpuSessionState {
    Open,
    Configured,
    Running,
    Quiescent,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CpuSessionStepError {
    InvalidState(CpuSessionState),
    EngineFailure,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CpuSessionReactionEvidenceV1 {
    pub(crate) schema_version: u32,
    pub(crate) session_id: String,
    pub(crate) run_id: u64,
    pub(crate) reaction_index: u32,
    pub(crate) scope_before: Vec<u32>,
    pub(crate) scope_after: Vec<u32>,
    pub(crate) links_before: u32,
    pub(crate) links_after: u32,
    pub(crate) raw_rule_matches: u32,
    pub(crate) transitioned_members: u32,
    pub(crate) handoff_count: u32,
    pub(crate) quiescent: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) structural_facts: Option<Vec<RunStructuralFactV1>>,
}

#[derive(Debug)]
pub(crate) struct CpuSessionStepOutcomeV1 {
    pub(crate) evidence: CpuSessionReactionEvidenceV1,
    pub(crate) structural_profile: StructuralRunProfile,
    pub(crate) trace_projection_ns: u128,
}

pub(crate) const CPU_RUN_BUDGET_SCHEMA_VERSION: u32 = 2;
pub(crate) const DEFAULT_MAX_APPENDED_LINKS_PER_RUN: u32 = 1_000_000;
pub(crate) const DEFAULT_MAX_TOTAL_LINKS: u32 = 2_000_000;
pub(crate) const DEFAULT_MAX_SCOPE_WIDTH: u32 = 65_536;
pub(crate) const DEFAULT_MAX_MATCH_CANDIDATES: u64 = 1_000_000_000;
pub(crate) const DEFAULT_MAX_UNIFICATION_NODES: u64 = 1_000_000_000;
pub(crate) const DEFAULT_MAX_INSTANTIATION_NODES: u64 = 1_000_000_000;
pub(crate) const DEFAULT_MAX_DENSE_CARRIER_BYTES: u64 = 256 * 1024 * 1024;

fn default_max_dense_carrier_bytes() -> u64 {
    DEFAULT_MAX_DENSE_CARRIER_BYTES
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CpuRunBudgetV1 {
    pub(crate) schema_version: u32,
    pub(crate) max_reactions: u32,
    pub(crate) max_appended_links: u32,
    pub(crate) max_total_links: u32,
    pub(crate) max_scope_width: u32,
    pub(crate) max_match_candidates: u64,
    pub(crate) max_unification_nodes: u64,
    pub(crate) max_instantiation_nodes: u64,
    #[serde(default = "default_max_dense_carrier_bytes")]
    pub(crate) max_dense_carrier_bytes: u64,
}

impl CpuRunBudgetV1 {
    pub(crate) fn scenario_default(max_reactions: u32) -> Self {
        Self {
            schema_version: CPU_RUN_BUDGET_SCHEMA_VERSION,
            max_reactions,
            max_appended_links: DEFAULT_MAX_APPENDED_LINKS_PER_RUN,
            max_total_links: DEFAULT_MAX_TOTAL_LINKS,
            max_scope_width: DEFAULT_MAX_SCOPE_WIDTH,
            max_match_candidates: DEFAULT_MAX_MATCH_CANDIDATES,
            max_unification_nodes: DEFAULT_MAX_UNIFICATION_NODES,
            max_instantiation_nodes: DEFAULT_MAX_INSTANTIATION_NODES,
            max_dense_carrier_bytes: DEFAULT_MAX_DENSE_CARRIER_BYTES,
        }
    }

    fn resource_stop_reason(
        self,
        links_before_run: u32,
        links_now: u32,
        scope_width: u32,
    ) -> Option<CpuRunStopReasonV1> {
        if links_now > self.max_total_links {
            return Some(CpuRunStopReasonV1::TotalLinksBudgetExceeded);
        }
        if links_now.saturating_sub(links_before_run)
            > self.max_appended_links
        {
            return Some(
                CpuRunStopReasonV1::AppendedLinksBudgetExceeded,
            );
        }
        if scope_width > self.max_scope_width {
            return Some(CpuRunStopReasonV1::ScopeWidthBudgetExceeded);
        }
        None
    }

    fn carrier_stop_reason(
        self,
        dense_carrier_allocated_bytes: u64,
    ) -> Option<CpuRunStopReasonV1> {
        if dense_carrier_allocated_bytes > self.max_dense_carrier_bytes {
            return Some(CpuRunStopReasonV1::CarrierBytesBudgetExceeded);
        }
        None
    }

    fn work_stop_reason(
        self,
        usage: CpuRunWorkUsageV1,
    ) -> Option<CpuRunStopReasonV1> {
        if usage.match_candidates > self.max_match_candidates {
            return Some(CpuRunStopReasonV1::MatchWorkBudgetExceeded);
        }
        if usage.unification_nodes > self.max_unification_nodes {
            return Some(
                CpuRunStopReasonV1::UnificationWorkBudgetExceeded,
            );
        }
        if usage.instantiation_nodes > self.max_instantiation_nodes {
            return Some(
                CpuRunStopReasonV1::InstantiationWorkBudgetExceeded,
            );
        }
        None
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CpuRunWorkUsageV1 {
    pub(crate) match_candidates: u64,
    pub(crate) unification_nodes: u64,
    pub(crate) instantiation_nodes: u64,
}

impl CpuRunWorkUsageV1 {
    fn accumulate(&mut self, profile: &StructuralRunProfile) {
        self.match_candidates = self
            .match_candidates
            .saturating_add(profile.trigger_incidence_candidates);
        self.unification_nodes = self
            .unification_nodes
            .saturating_add(profile.unification_nodes_visited);
        self.instantiation_nodes = self
            .instantiation_nodes
            .saturating_add(profile.instantiation_nodes_visited);
    }
}

pub(crate) const CPU_RUN_BUDGET_ACCOUNTING_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CpuRunBudgetAccountingV1 {
    pub(crate) schema_version: u32,
    pub(crate) reactions_consumed: u32,
    pub(crate) max_reactions: u32,
    pub(crate) appended_links_consumed: u32,
    pub(crate) max_appended_links: u32,
    pub(crate) total_links: u32,
    pub(crate) max_total_links: u32,
    pub(crate) scope_width: u32,
    pub(crate) max_scope_width: u32,
    pub(crate) match_candidates: u64,
    pub(crate) max_match_candidates: u64,
    pub(crate) unification_nodes: u64,
    pub(crate) max_unification_nodes: u64,
    pub(crate) instantiation_nodes: u64,
    pub(crate) max_instantiation_nodes: u64,
    pub(crate) dense_carrier_allocated_bytes: u64,
    pub(crate) max_dense_carrier_bytes: u64,
    pub(crate) full_resident_bytes_available: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum CpuRunStopReasonV1 {
    Quiescent,
    ReactionBudgetExceeded,
    AppendedLinksBudgetExceeded,
    TotalLinksBudgetExceeded,
    ScopeWidthBudgetExceeded,
    MatchWorkBudgetExceeded,
    UnificationWorkBudgetExceeded,
    InstantiationWorkBudgetExceeded,
    CarrierBytesBudgetExceeded,
    EngineFailure,
    InvalidState,
}

impl CpuRunStopReasonV1 {
    pub(crate) fn from_step_error(error: CpuSessionStepError) -> Self {
        match error {
            CpuSessionStepError::InvalidState(_) => Self::InvalidState,
            CpuSessionStepError::EngineFailure => Self::EngineFailure,
        }
    }
}

#[derive(Debug)]
pub(crate) struct CpuControlledStepV1 {
    pub(crate) step: Option<CpuSessionStepOutcomeV1>,
    pub(crate) stop_reason: Option<CpuRunStopReasonV1>,
    pub(crate) budget_accounting: CpuRunBudgetAccountingV1,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CpuRunControllerV1 {
    budget: CpuRunBudgetV1,
    run_id: u64,
    links_before_run: u32,
    steps_taken: u32,
    work_usage: CpuRunWorkUsageV1,
}

impl CpuRunControllerV1 {
    fn new(
        budget: CpuRunBudgetV1,
        run_id: u64,
        links_before_run: u32,
    ) -> Self {
        Self {
            budget,
            run_id,
            links_before_run,
            steps_taken: 0,
            work_usage: CpuRunWorkUsageV1::default(),
        }
    }

    pub(crate) fn run_id(&self) -> u64 {
        self.run_id
    }

    pub(crate) fn links_before_run(&self) -> u32 {
        self.links_before_run
    }

    pub(crate) fn budget_accounting(
        &self,
        session: &CpuRuntimeSession,
    ) -> CpuRunBudgetAccountingV1 {
        let total_links = session.memory.store.link_count() as u32;
        CpuRunBudgetAccountingV1 {
            schema_version: CPU_RUN_BUDGET_ACCOUNTING_SCHEMA_VERSION,
            reactions_consumed: self.steps_taken,
            max_reactions: self.budget.max_reactions,
            appended_links_consumed:
                total_links.saturating_sub(self.links_before_run),
            max_appended_links: self.budget.max_appended_links,
            total_links,
            max_total_links: self.budget.max_total_links,
            scope_width: session.engine.current().len() as u32,
            max_scope_width: self.budget.max_scope_width,
            match_candidates: self.work_usage.match_candidates,
            max_match_candidates: self.budget.max_match_candidates,
            unification_nodes: self.work_usage.unification_nodes,
            max_unification_nodes: self.budget.max_unification_nodes,
            instantiation_nodes: self.work_usage.instantiation_nodes,
            max_instantiation_nodes: self.budget.max_instantiation_nodes,
            dense_carrier_allocated_bytes: session
                .memory
                .store
                .dense_carrier_index_allocated_bytes(),
            max_dense_carrier_bytes: self.budget.max_dense_carrier_bytes,
            full_resident_bytes_available: false,
        }
    }

    fn stopped(
        &self,
        session: &CpuRuntimeSession,
        step: Option<CpuSessionStepOutcomeV1>,
        stop_reason: CpuRunStopReasonV1,
    ) -> CpuControlledStepV1 {
        CpuControlledStepV1 {
            step,
            stop_reason: Some(stop_reason),
            budget_accounting: self.budget_accounting(session),
        }
    }

    pub(crate) fn next(
        &mut self,
        session: &mut CpuRuntimeSession,
        observation_level: RunObservationLevel,
    ) -> CpuControlledStepV1 {
        if let Some(reason) = self.budget.carrier_stop_reason(
            session.memory.store.dense_carrier_index_allocated_bytes(),
        ) {
            session.fail_active_run();
            return self.stopped(session, None, reason);
        }

        if let Some(reason) = self.budget.resource_stop_reason(
            self.links_before_run,
            session.memory.store.link_count() as u32,
            session.engine.current().len() as u32,
        ) {
            session.fail_active_run();
            return self.stopped(session, None, reason);
        }

        if self.steps_taken >= self.budget.max_reactions {
            session.fail_active_run();
            return self.stopped(
                session,
                None,
                CpuRunStopReasonV1::ReactionBudgetExceeded,
            );
        }

        let step = match session.step(observation_level) {
            Ok(step) => step,
            Err(error) => {
                session.fail_active_run();
                return self.stopped(
                    session,
                    None,
                    CpuRunStopReasonV1::from_step_error(error),
                );
            }
        };
        self.steps_taken = self.steps_taken.saturating_add(1);
        self.work_usage.accumulate(&step.structural_profile);

        if let Some(reason) = self.budget.resource_stop_reason(
            self.links_before_run,
            session.memory.store.link_count() as u32,
            session.engine.current().len() as u32,
        ) {
            session.fail_active_run();
            return self.stopped(session, Some(step), reason);
        }

        if let Some(reason) = self.budget.carrier_stop_reason(
            session.memory.store.dense_carrier_index_allocated_bytes(),
        ) {
            session.fail_active_run();
            return self.stopped(session, Some(step), reason);
        }

        if let Some(reason) = self.budget.work_stop_reason(self.work_usage) {
            session.fail_active_run();
            return self.stopped(session, Some(step), reason);
        }

        let stop_reason = step
            .evidence
            .quiescent
            .then_some(CpuRunStopReasonV1::Quiescent);
        CpuControlledStepV1 {
            step: Some(step),
            stop_reason,
            budget_accounting: self.budget_accounting(session),
        }
    }
}

#[derive(Debug)]
pub(crate) struct CpuBoundedRunV1 {
    pub(crate) run_id: u64,
    pub(crate) links_before_run: u32,
    pub(crate) steps: Vec<CpuSessionStepOutcomeV1>,
    pub(crate) stop_reason: CpuRunStopReasonV1,
    pub(crate) budget_accounting: CpuRunBudgetAccountingV1,
}

fn runtime_metering_level(
    observation_level: RunObservationLevel,
) -> RunObservationLevel {
    match observation_level {
        RunObservationLevel::Off => RunObservationLevel::Profile,
        level => level,
    }
}

/// Long-lived optimized-CPU execution Session over one loaded A-memory.
///
/// `begin_run` publishes an initial Scope into the already loaded engine.
/// `step` is the one authoritative semantic reaction operation. Higher-level
/// run-to-quiescence APIs are bounded loops over this primitive.
#[derive(Debug)]
pub(crate) struct CpuRuntimeSession {
    pub(crate) memory: CpuMemoryInstance,
    pub(crate) engine: OptimizedStructuralEngine,
    pub(crate) base_link_count: usize,
    next_run_id: u64,
    execution_state: CpuSessionState,
    active_run_id: Option<u64>,
    next_reaction_index: u32,
}

impl CpuRuntimeSession {
    pub(crate) fn new(
        memory: CpuMemoryInstance,
        engine: OptimizedStructuralEngine,
        base_link_count: usize,
    ) -> Self {
        Self {
            memory,
            engine,
            base_link_count,
            next_run_id: 1,
            execution_state: CpuSessionState::Open,
            active_run_id: None,
            next_reaction_index: 0,
        }
    }

    pub(crate) fn execution_state(&self) -> CpuSessionState {
        self.execution_state
    }

    pub(crate) fn begin_run(
        &mut self,
        initial: Handle,
    ) -> Result<u64, CpuSessionStepError> {
        if !matches!(
            self.execution_state,
            CpuSessionState::Open | CpuSessionState::Quiescent
        ) {
            return Err(CpuSessionStepError::InvalidState(
                self.execution_state,
            ));
        }

        if self
            .engine
            .set_current(&self.memory.store, &[initial])
            .is_err()
        {
            self.execution_state = CpuSessionState::Failed;
            return Err(CpuSessionStepError::EngineFailure);
        }

        let run_id = self.next_run_id;
        self.next_run_id = self.next_run_id.saturating_add(1);
        self.active_run_id = Some(run_id);
        self.next_reaction_index = 0;
        self.execution_state = CpuSessionState::Configured;
        Ok(run_id)
    }

    pub(crate) fn step(
        &mut self,
        observation_level: RunObservationLevel,
    ) -> Result<CpuSessionStepOutcomeV1, CpuSessionStepError> {
        if !matches!(
            self.execution_state,
            CpuSessionState::Configured | CpuSessionState::Running
        ) {
            return Err(CpuSessionStepError::InvalidState(
                self.execution_state,
            ));
        }

        let run_id = match self.active_run_id {
            Some(run_id) => run_id,
            None => {
                self.execution_state = CpuSessionState::Failed;
                return Err(CpuSessionStepError::EngineFailure);
            }
        };
        let reaction_index = self.next_reaction_index;
        let links_before = self.memory.store.link_count() as u32;
        let metering_level = runtime_metering_level(observation_level);

        let mut trace_projection_ns = 0u128;
        let scope_before = if observation_level.traces() {
            let projection_started = ObservationTimer::start();
            let scope = self.engine.current().to_vec();
            trace_projection_ns = trace_projection_ns
                .saturating_add(projection_started.elapsed_ns());
            scope
        } else {
            self.engine.current().to_vec()
        };

        let (reaction, structural_profile, structural_facts) =
            if metering_level.traces() {
                let (reaction, profile, native_trace) = match self
                    .engine
                    .run_traced(&mut self.memory.store)
                {
                    Ok(value) => value,
                    Err(_) => {
                        self.execution_state = CpuSessionState::Failed;
                        return Err(CpuSessionStepError::EngineFailure);
                    }
                };

                let projection_started = ObservationTimer::start();
                let collection_ns = native_trace.collection_ns;
                let facts = native_trace
                    .events
                    .into_iter()
                    .map(RunStructuralFactV1::from)
                    .collect::<Vec<_>>();
                trace_projection_ns = trace_projection_ns
                    .saturating_add(collection_ns)
                    .saturating_add(projection_started.elapsed_ns());
                (reaction, profile, Some(facts))
            } else {
                // Runtime safety metering remains active even when profile
                // projection is suppressed with observation=OFF.
                let (reaction, profile) = match self
                    .engine
                    .run_profiled(&mut self.memory.store)
                {
                    Ok(value) => value,
                    Err(_) => {
                        self.execution_state = CpuSessionState::Failed;
                        return Err(CpuSessionStepError::EngineFailure);
                    }
                };
                (reaction, profile, None)
            };

        let scope_after = if observation_level.traces() {
            let projection_started = ObservationTimer::start();
            let scope = self.engine.current().to_vec();
            trace_projection_ns = trace_projection_ns
                .saturating_add(projection_started.elapsed_ns());
            scope
        } else {
            self.engine.current().to_vec()
        };
        let quiescent = reaction.quiescent;

        self.next_reaction_index =
            self.next_reaction_index.saturating_add(1);
        self.execution_state = if quiescent {
            CpuSessionState::Quiescent
        } else {
            CpuSessionState::Running
        };

        Ok(CpuSessionStepOutcomeV1 {
            evidence: CpuSessionReactionEvidenceV1 {
                schema_version: RUN_OBSERVABILITY_SCHEMA_VERSION,
                session_id: self.memory.id.clone(),
                run_id,
                reaction_index,
                scope_before,
                scope_after,
                links_before,
                links_after: self.memory.store.link_count() as u32,
                raw_rule_matches: reaction.raw_rule_matches,
                transitioned_members: reaction.transitioned_members,
                handoff_count: reaction.handoff_count,
                quiescent,
                structural_facts,
            },
            structural_profile,
            trace_projection_ns,
        })
    }

    pub(crate) fn begin_budgeted_run(
        &mut self,
        initial: Handle,
        budget: CpuRunBudgetV1,
    ) -> Result<CpuRunControllerV1, CpuSessionStepError> {
        let run_id = self.begin_run(initial)?;
        Ok(CpuRunControllerV1::new(
            budget,
            run_id,
            self.memory.store.link_count() as u32,
        ))
    }

    pub(crate) fn run_to_quiescence(
        &mut self,
        initial: Handle,
        budget: CpuRunBudgetV1,
        observation_level: RunObservationLevel,
    ) -> Result<CpuBoundedRunV1, CpuRunStopReasonV1> {
        let mut controller = self
            .begin_budgeted_run(initial, budget)
            .map_err(CpuRunStopReasonV1::from_step_error)?;
        let run_id = controller.run_id();
        let links_before_run = controller.links_before_run();
        let mut steps = Vec::new();

        loop {
            let controlled = controller.next(self, observation_level);
            let budget_accounting = controlled.budget_accounting;
            if let Some(step) = controlled.step {
                steps.push(step);
            }
            if let Some(stop_reason) = controlled.stop_reason {
                return Ok(CpuBoundedRunV1 {
                    run_id,
                    links_before_run,
                    steps,
                    stop_reason,
                    budget_accounting,
                });
            }
        }
    }

    pub(crate) fn fail_active_run(&mut self) {
        self.execution_state = CpuSessionState::Failed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budget_resource_reasons_are_distinct_and_deterministic() {
        let budget = CpuRunBudgetV1 {
            schema_version: CPU_RUN_BUDGET_SCHEMA_VERSION,
            max_reactions: 8,
            max_appended_links: 5,
            max_total_links: 20,
            max_scope_width: 3,
            max_match_candidates: 100,
            max_unification_nodes: 100,
            max_instantiation_nodes: 100,
            max_dense_carrier_bytes: 1_000,
        };

        assert_eq!(
            budget.resource_stop_reason(10, 21, 1),
            Some(CpuRunStopReasonV1::TotalLinksBudgetExceeded),
        );
        assert_eq!(
            budget.resource_stop_reason(10, 16, 1),
            Some(CpuRunStopReasonV1::AppendedLinksBudgetExceeded),
        );
        assert_eq!(
            budget.resource_stop_reason(10, 15, 4),
            Some(CpuRunStopReasonV1::ScopeWidthBudgetExceeded),
        );
        assert_eq!(budget.resource_stop_reason(10, 15, 3), None);
    }

    #[test]
    fn budget_work_reasons_are_distinct_and_deterministic() {
        let budget = CpuRunBudgetV1 {
            schema_version: CPU_RUN_BUDGET_SCHEMA_VERSION,
            max_reactions: 8,
            max_appended_links: 5,
            max_total_links: 20,
            max_scope_width: 3,
            max_match_candidates: 10,
            max_unification_nodes: 20,
            max_instantiation_nodes: 30,
            max_dense_carrier_bytes: 1_000,
        };

        assert_eq!(
            budget.work_stop_reason(CpuRunWorkUsageV1 {
                match_candidates: 11,
                ..CpuRunWorkUsageV1::default()
            }),
            Some(CpuRunStopReasonV1::MatchWorkBudgetExceeded),
        );
        assert_eq!(
            budget.work_stop_reason(CpuRunWorkUsageV1 {
                unification_nodes: 21,
                ..CpuRunWorkUsageV1::default()
            }),
            Some(CpuRunStopReasonV1::UnificationWorkBudgetExceeded),
        );
        assert_eq!(
            budget.work_stop_reason(CpuRunWorkUsageV1 {
                instantiation_nodes: 31,
                ..CpuRunWorkUsageV1::default()
            }),
            Some(CpuRunStopReasonV1::InstantiationWorkBudgetExceeded),
        );
        assert_eq!(
            budget.work_stop_reason(CpuRunWorkUsageV1 {
                match_candidates: 10,
                unification_nodes: 20,
                instantiation_nodes: 30,
            }),
            None,
        );
    }

    #[test]
    fn budget_v2_without_new_byte_field_uses_finite_default() {
        let mut legacy =
            serde_json::to_value(CpuRunBudgetV1::scenario_default(8)).unwrap();
        legacy
            .as_object_mut()
            .unwrap()
            .remove("maxDenseCarrierBytes");
        let decoded: CpuRunBudgetV1 = serde_json::from_value(legacy).unwrap();
        assert_eq!(
            decoded.max_dense_carrier_bytes,
            DEFAULT_MAX_DENSE_CARRIER_BYTES,
        );
    }

    #[test]
    fn budget_accounting_snapshot_uses_controller_and_live_runtime_state() {
        let mut memory = CpuMemoryInstance::new();
        let initial = memory.store.ensure(1, 1);
        let mut session = CpuRuntimeSession::new(
            memory,
            OptimizedStructuralEngine::new(8),
            1,
        );
        let budget = CpuRunBudgetV1 {
            max_reactions: 7,
            max_appended_links: 11,
            max_total_links: 22,
            max_scope_width: 3,
            max_match_candidates: 44,
            max_unification_nodes: 55,
            max_instantiation_nodes: 66,
            max_dense_carrier_bytes: 77_777,
            ..CpuRunBudgetV1::scenario_default(7)
        };
        let controller = session.begin_budgeted_run(initial, budget).unwrap();
        let accounting = controller.budget_accounting(&session);

        assert_eq!(accounting.schema_version, 1);
        assert_eq!(accounting.reactions_consumed, 0);
        assert_eq!(accounting.max_reactions, 7);
        assert_eq!(accounting.appended_links_consumed, 0);
        assert_eq!(accounting.max_appended_links, 11);
        assert_eq!(accounting.total_links, session.memory.store.link_count() as u32);
        assert_eq!(accounting.max_total_links, 22);
        assert_eq!(accounting.scope_width, 1);
        assert_eq!(accounting.max_scope_width, 3);
        assert_eq!(accounting.match_candidates, 0);
        assert_eq!(accounting.max_match_candidates, 44);
        assert_eq!(accounting.unification_nodes, 0);
        assert_eq!(accounting.max_unification_nodes, 55);
        assert_eq!(accounting.instantiation_nodes, 0);
        assert_eq!(accounting.max_instantiation_nodes, 66);
        assert_eq!(accounting.max_dense_carrier_bytes, 77_777);
        assert!(!accounting.full_resident_bytes_available);
    }

    #[test]
    fn carrier_byte_reason_is_exact_and_resident_bytes_are_not_claimed() {
        let budget = CpuRunBudgetV1 {
            max_dense_carrier_bytes: 99,
            ..CpuRunBudgetV1::scenario_default(8)
        };
        assert_eq!(budget.carrier_stop_reason(99), None);
        assert_eq!(
            budget.carrier_stop_reason(100),
            Some(CpuRunStopReasonV1::CarrierBytesBudgetExceeded),
        );
    }

    #[test]
    fn observation_off_still_enables_runtime_metering() {
        assert_eq!(
            runtime_metering_level(RunObservationLevel::Off),
            RunObservationLevel::Profile,
        );
        assert_eq!(
            runtime_metering_level(RunObservationLevel::Trace),
            RunObservationLevel::Trace,
        );
        assert_eq!(
            runtime_metering_level(RunObservationLevel::Full),
            RunObservationLevel::Full,
        );
    }

    #[test]
    fn stop_reasons_are_stable_machine_values() {
        assert_eq!(
            serde_json::to_string(&CpuRunStopReasonV1::Quiescent).unwrap(),
            "\"QUIESCENT\"",
        );
        assert_eq!(
            serde_json::to_string(
                &CpuRunStopReasonV1::ReactionBudgetExceeded,
            )
            .unwrap(),
            "\"REACTION_BUDGET_EXCEEDED\"",
        );
        assert_eq!(
            serde_json::to_string(
                &CpuRunStopReasonV1::MatchWorkBudgetExceeded,
            )
            .unwrap(),
            "\"MATCH_WORK_BUDGET_EXCEEDED\"",
        );
        assert_eq!(
            serde_json::to_string(
                &CpuRunStopReasonV1::UnificationWorkBudgetExceeded,
            )
            .unwrap(),
            "\"UNIFICATION_WORK_BUDGET_EXCEEDED\"",
        );
        assert_eq!(
            serde_json::to_string(
                &CpuRunStopReasonV1::InstantiationWorkBudgetExceeded,
            )
            .unwrap(),
            "\"INSTANTIATION_WORK_BUDGET_EXCEEDED\"",
        );
        assert_eq!(
            serde_json::to_string(
                &CpuRunStopReasonV1::CarrierBytesBudgetExceeded,
            )
            .unwrap(),
            "\"CARRIER_BYTES_BUDGET_EXCEEDED\"",
        );
        assert_eq!(
            CpuRunStopReasonV1::from_step_error(
                CpuSessionStepError::EngineFailure,
            ),
            CpuRunStopReasonV1::EngineFailure,
        );
        assert_eq!(
            CpuRunStopReasonV1::from_step_error(
                CpuSessionStepError::InvalidState(
                    CpuSessionState::Quiescent,
                ),
            ),
            CpuRunStopReasonV1::InvalidState,
        );
    }

    #[test]
    fn scenario_budget_policy_is_versioned_and_finite() {
        let budget = CpuRunBudgetV1::scenario_default(64);
        assert_eq!(
            budget.schema_version,
            CPU_RUN_BUDGET_SCHEMA_VERSION,
        );
        assert_eq!(budget.max_reactions, 64);
        assert!(budget.max_appended_links < u32::MAX);
        assert!(budget.max_total_links < u32::MAX);
        assert!(budget.max_scope_width < u32::MAX);
        assert!(budget.max_match_candidates < u64::MAX);
        assert!(budget.max_unification_nodes < u64::MAX);
        assert!(budget.max_instantiation_nodes < u64::MAX);
        assert!(budget.max_dense_carrier_bytes < u64::MAX);
    }
}
