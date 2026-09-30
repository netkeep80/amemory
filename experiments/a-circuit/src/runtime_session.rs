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

pub(crate) const CPU_RUN_BUDGET_SCHEMA_VERSION: u32 = 1;
pub(crate) const DEFAULT_MAX_APPENDED_LINKS_PER_RUN: u32 = 1_000_000;
pub(crate) const DEFAULT_MAX_TOTAL_LINKS: u32 = 2_000_000;
pub(crate) const DEFAULT_MAX_SCOPE_WIDTH: u32 = 65_536;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CpuRunBudgetV1 {
    pub(crate) schema_version: u32,
    pub(crate) max_reactions: u32,
    pub(crate) max_appended_links: u32,
    pub(crate) max_total_links: u32,
    pub(crate) max_scope_width: u32,
}

impl CpuRunBudgetV1 {
    pub(crate) fn scenario_default(max_reactions: u32) -> Self {
        Self {
            schema_version: CPU_RUN_BUDGET_SCHEMA_VERSION,
            max_reactions,
            max_appended_links: DEFAULT_MAX_APPENDED_LINKS_PER_RUN,
            max_total_links: DEFAULT_MAX_TOTAL_LINKS,
            max_scope_width: DEFAULT_MAX_SCOPE_WIDTH,
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub(crate) enum CpuRunStopReasonV1 {
    Quiescent,
    ReactionBudgetExceeded,
    AppendedLinksBudgetExceeded,
    TotalLinksBudgetExceeded,
    ScopeWidthBudgetExceeded,
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
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CpuRunControllerV1 {
    budget: CpuRunBudgetV1,
    run_id: u64,
    links_before_run: u32,
    steps_taken: u32,
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
        }
    }

    pub(crate) fn run_id(&self) -> u64 {
        self.run_id
    }

    pub(crate) fn links_before_run(&self) -> u32 {
        self.links_before_run
    }

    pub(crate) fn next(
        &mut self,
        session: &mut CpuRuntimeSession,
        observation_level: RunObservationLevel,
    ) -> CpuControlledStepV1 {
        if let Some(reason) = self.budget.resource_stop_reason(
            self.links_before_run,
            session.memory.store.link_count() as u32,
            session.engine.current().len() as u32,
        ) {
            session.fail_active_run();
            return CpuControlledStepV1 {
                step: None,
                stop_reason: Some(reason),
            };
        }

        if self.steps_taken >= self.budget.max_reactions {
            session.fail_active_run();
            return CpuControlledStepV1 {
                step: None,
                stop_reason: Some(
                    CpuRunStopReasonV1::ReactionBudgetExceeded,
                ),
            };
        }

        let step = match session.step(observation_level) {
            Ok(step) => step,
            Err(error) => {
                session.fail_active_run();
                return CpuControlledStepV1 {
                    step: None,
                    stop_reason: Some(
                        CpuRunStopReasonV1::from_step_error(error),
                    ),
                };
            }
        };
        self.steps_taken = self.steps_taken.saturating_add(1);

        if let Some(reason) = self.budget.resource_stop_reason(
            self.links_before_run,
            session.memory.store.link_count() as u32,
            session.engine.current().len() as u32,
        ) {
            session.fail_active_run();
            return CpuControlledStepV1 {
                step: Some(step),
                stop_reason: Some(reason),
            };
        }

        CpuControlledStepV1 {
            stop_reason: step
                .evidence
                .quiescent
                .then_some(CpuRunStopReasonV1::Quiescent),
            step: Some(step),
        }
    }
}

#[derive(Debug)]
pub(crate) struct CpuBoundedRunV1 {
    pub(crate) run_id: u64,
    pub(crate) links_before_run: u32,
    pub(crate) steps: Vec<CpuSessionStepOutcomeV1>,
    pub(crate) stop_reason: CpuRunStopReasonV1,
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
            if observation_level.traces() {
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
            } else if observation_level.profiles() {
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
            } else {
                let reaction = match self.engine.run(&mut self.memory.store) {
                    Ok(value) => value,
                    Err(_) => {
                        self.execution_state = CpuSessionState::Failed;
                        return Err(CpuSessionStepError::EngineFailure);
                    }
                };
                (reaction, StructuralRunProfile::default(), None)
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
    ) -> Result<CpuBoundedRunV1, CpuSessionStepError> {
        let mut controller = self.begin_budgeted_run(initial, budget)?;
        let run_id = controller.run_id();
        let links_before_run = controller.links_before_run();
        let mut steps = Vec::new();

        loop {
            let controlled = controller.next(self, observation_level);
            if let Some(step) = controlled.step {
                steps.push(step);
            }
            if let Some(stop_reason) = controlled.stop_reason {
                return Ok(CpuBoundedRunV1 {
                    run_id,
                    links_before_run,
                    steps,
                    stop_reason,
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
    }
}
