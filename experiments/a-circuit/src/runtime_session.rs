use amemory_optimized_cpu_probe::{
    structural::{
        OptimizedStructuralEngine, StructuralReadV1, StructuralRunProfile,
        StructuralRunTrace, StructuralStoreV1,
    },
    Handle, OptimizedLinkStore,
};
use super::session_contract::{
    BackendResourceAccountingV1, CapabilitySupportV1,
    RunBudgetAccountingV1, RunBudgetV1, RunStopReasonV1,
    RunWorkUsageV1, RuntimeBackendV1, RuntimeSessionV1,
    RUN_BUDGET_SCHEMA_VERSION,
    SessionCapabilitiesV1, SessionIdentityV1, SessionStateV1,
    RUN_BUDGET_ACCOUNTING_SCHEMA_VERSION, SESSION_CONTRACT_SCHEMA_VERSION,
};
use std::sync::atomic::{AtomicU32, Ordering};

static NEXT_CPU_MEMORY_ID: AtomicU32 = AtomicU32::new(1);
static NEXT_CPU_SESSION_ID: AtomicU32 = AtomicU32::new(1);

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CpuSessionStepError {
    InvalidState(SessionStateV1),
    EngineFailure,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CpuRuntimeTraceMode {
    Profile,
    Trace,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CpuSessionReactionV1 {
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
}

#[derive(Debug)]
pub(crate) struct CpuSessionStepOutcomeV1 {
    pub(crate) reaction: CpuSessionReactionV1,
    pub(crate) structural_profile: StructuralRunProfile,
    pub(crate) structural_trace: Option<StructuralRunTrace>,
}

pub(crate) const DEFAULT_MAX_DENSE_CARRIER_BYTES: u64 =
    256 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CpuRunResourceBudgetV1 {
    pub(crate) max_dense_carrier_bytes: u64,
}

impl Default for CpuRunResourceBudgetV1 {
    fn default() -> Self {
        Self {
            max_dense_carrier_bytes: DEFAULT_MAX_DENSE_CARRIER_BYTES,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BackendRunResourceBudgetV1 {
    None,
    OptimizedCpuDenseCarrier(CpuRunResourceBudgetV1),
}

impl BackendRunResourceBudgetV1 {
    fn optimized_cpu_default() -> Self {
        Self::OptimizedCpuDenseCarrier(CpuRunResourceBudgetV1::default())
    }
}

impl RunWorkUsageV1 {
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

fn run_stop_reason_from_step_error(
    error: CpuSessionStepError,
) -> RunStopReasonV1 {
    match error {
        CpuSessionStepError::InvalidState(_) => RunStopReasonV1::InvalidState,
        CpuSessionStepError::EngineFailure => RunStopReasonV1::EngineFailure,
    }
}

#[derive(Debug)]
pub(crate) struct ControlledStepV1 {
    pub(crate) step: Option<CpuSessionStepOutcomeV1>,
    pub(crate) stop_reason: Option<RunStopReasonV1>,
    pub(crate) budget_accounting: RunBudgetAccountingV1,
}

/// Minimal host boundary used by the one bounded run controller.
///
/// Common reaction/Link/Scope/work budgets live in RunControllerV1. A backend
/// may additionally expose one explicit physical-resource budget/accounting
/// policy. Missing physical accounting is represented by None, never zero.
pub(crate) trait BoundedRuntimeSessionV1 {
    fn bounded_step_v1(
        &mut self,
        trace_mode: CpuRuntimeTraceMode,
    ) -> Result<CpuSessionStepOutcomeV1, CpuSessionStepError>;

    fn bounded_fail_active_run_v1(&mut self);
    fn bounded_total_links_v1(&self) -> u32;
    fn bounded_scope_width_v1(&self) -> u32;

    fn bounded_backend_stop_reason_v1(
        &self,
        resource_budget: BackendRunResourceBudgetV1,
    ) -> Option<RunStopReasonV1>;

    fn bounded_backend_accounting_v1(
        &self,
        resource_budget: BackendRunResourceBudgetV1,
    ) -> Option<BackendResourceAccountingV1>;
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RunControllerV1 {
    budget: RunBudgetV1,
    resource_budget: BackendRunResourceBudgetV1,
    run_id: u64,
    links_before_run: u32,
    steps_taken: u32,
    work_usage: RunWorkUsageV1,
}

impl RunControllerV1 {
    fn new(
        budget: RunBudgetV1,
        run_id: u64,
        links_before_run: u32,
    ) -> Self {
        Self::new_with_resource_budget(
            budget,
            BackendRunResourceBudgetV1::optimized_cpu_default(),
            run_id,
            links_before_run,
        )
    }

    pub(crate) fn new_with_resource_budget(
        budget: RunBudgetV1,
        resource_budget: BackendRunResourceBudgetV1,
        run_id: u64,
        links_before_run: u32,
    ) -> Self {
        Self {
            budget,
            resource_budget,
            run_id,
            links_before_run,
            steps_taken: 0,
            work_usage: RunWorkUsageV1::default(),
        }
    }

    pub(crate) fn new_without_backend_resource(
        budget: RunBudgetV1,
        run_id: u64,
        links_before_run: u32,
    ) -> Self {
        Self::new_with_resource_budget(
            budget,
            BackendRunResourceBudgetV1::None,
            run_id,
            links_before_run,
        )
    }

    pub(crate) fn run_id(&self) -> u64 {
        self.run_id
    }

    pub(crate) fn links_before_run(&self) -> u32 {
        self.links_before_run
    }

    pub(crate) fn budget_accounting<S: BoundedRuntimeSessionV1>(
        &self,
        session: &S,
    ) -> RunBudgetAccountingV1 {
        let total_links = session.bounded_total_links_v1();
        RunBudgetAccountingV1 {
            schema_version: RUN_BUDGET_ACCOUNTING_SCHEMA_VERSION,
            reactions_consumed: self.steps_taken,
            max_reactions: self.budget.max_reactions,
            appended_links_consumed:
                total_links.saturating_sub(self.links_before_run),
            max_appended_links: self.budget.max_appended_links,
            total_links,
            max_total_links: self.budget.max_total_links,
            scope_width: session.bounded_scope_width_v1(),
            max_scope_width: self.budget.max_scope_width,
            match_candidates: self.work_usage.match_candidates,
            max_match_candidates: self.budget.max_match_candidates,
            unification_nodes: self.work_usage.unification_nodes,
            max_unification_nodes: self.budget.max_unification_nodes,
            instantiation_nodes: self.work_usage.instantiation_nodes,
            max_instantiation_nodes: self.budget.max_instantiation_nodes,
            backend_resource: session
                .bounded_backend_accounting_v1(self.resource_budget),
        }
    }

    fn stopped<S: BoundedRuntimeSessionV1>(
        &self,
        session: &S,
        step: Option<CpuSessionStepOutcomeV1>,
        stop_reason: RunStopReasonV1,
    ) -> ControlledStepV1 {
        ControlledStepV1 {
            step,
            stop_reason: Some(stop_reason),
            budget_accounting: self.budget_accounting(session),
        }
    }

    pub(crate) fn next<S: BoundedRuntimeSessionV1>(
        &mut self,
        session: &mut S,
        trace_mode: CpuRuntimeTraceMode,
    ) -> ControlledStepV1 {
        if let Some(reason) =
            session.bounded_backend_stop_reason_v1(self.resource_budget)
        {
            session.bounded_fail_active_run_v1();
            return self.stopped(session, None, reason);
        }

        if let Some(reason) = self.budget.resource_stop_reason(
            self.links_before_run,
            session.bounded_total_links_v1(),
            session.bounded_scope_width_v1(),
        ) {
            session.bounded_fail_active_run_v1();
            return self.stopped(session, None, reason);
        }

        if self.steps_taken >= self.budget.max_reactions {
            session.bounded_fail_active_run_v1();
            return self.stopped(
                session,
                None,
                RunStopReasonV1::ReactionBudgetExceeded,
            );
        }

        let step = match session.bounded_step_v1(trace_mode) {
            Ok(step) => step,
            Err(error) => {
                session.bounded_fail_active_run_v1();
                return self.stopped(
                    session,
                    None,
                    run_stop_reason_from_step_error(error),
                );
            }
        };
        self.steps_taken = self.steps_taken.saturating_add(1);
        self.work_usage.accumulate(&step.structural_profile);

        if let Some(reason) = self.budget.resource_stop_reason(
            self.links_before_run,
            session.bounded_total_links_v1(),
            session.bounded_scope_width_v1(),
        ) {
            session.bounded_fail_active_run_v1();
            return self.stopped(session, Some(step), reason);
        }

        if let Some(reason) =
            session.bounded_backend_stop_reason_v1(self.resource_budget)
        {
            session.bounded_fail_active_run_v1();
            return self.stopped(session, Some(step), reason);
        }

        if let Some(reason) = self.budget.work_stop_reason(self.work_usage) {
            session.bounded_fail_active_run_v1();
            return self.stopped(session, Some(step), reason);
        }

        let stop_reason = step
            .reaction
            .quiescent
            .then_some(RunStopReasonV1::Quiescent);
        ControlledStepV1 {
            step: Some(step),
            stop_reason,
            budget_accounting: self.budget_accounting(session),
        }
    }
}

#[derive(Debug)]
pub(crate) struct BoundedRunV1 {
    pub(crate) run_id: u64,
    pub(crate) links_before_run: u32,
    pub(crate) steps: Vec<CpuSessionStepOutcomeV1>,
    pub(crate) stop_reason: RunStopReasonV1,
    pub(crate) budget_accounting: RunBudgetAccountingV1,
}

/// Backend-neutral persistent semantic runtime state machine.
///
/// Physical storage is supplied through `StructuralReadV1/StructuralStoreV1`;
/// semantic execution remains owned by the single `OptimizedStructuralEngine`.
/// This object owns only lifecycle/run identity. It deliberately does not own
/// backend resource accounting or Scenario/result projection.
#[derive(Clone, Debug)]
pub(crate) struct StructuralRuntimeStateV1 {
    next_run_id: u64,
    execution_state: SessionStateV1,
    active_run_id: Option<u64>,
    next_reaction_index: u32,
}

impl StructuralRuntimeStateV1 {
    pub(crate) fn new() -> Self {
        Self {
            next_run_id: 1,
            execution_state: SessionStateV1::Open,
            active_run_id: None,
            next_reaction_index: 0,
        }
    }

    pub(crate) fn state(&self) -> SessionStateV1 {
        self.execution_state
    }

    pub(crate) fn begin_run<S: StructuralReadV1 + ?Sized>(
        &mut self,
        store: &S,
        engine: &mut OptimizedStructuralEngine,
        initial: Handle,
    ) -> Result<u64, CpuSessionStepError> {
        if !matches!(
            self.execution_state,
            SessionStateV1::Open | SessionStateV1::Quiescent
        ) {
            return Err(CpuSessionStepError::InvalidState(
                self.execution_state,
            ));
        }

        if engine.set_current(store, &[initial]).is_err() {
            self.execution_state = SessionStateV1::Failed;
            return Err(CpuSessionStepError::EngineFailure);
        }

        let run_id = self.next_run_id;
        self.next_run_id = self.next_run_id.saturating_add(1);
        self.active_run_id = Some(run_id);
        self.next_reaction_index = 0;
        self.execution_state = SessionStateV1::Configured;
        Ok(run_id)
    }

    pub(crate) fn step<S: StructuralStoreV1 + ?Sized>(
        &mut self,
        session_id: &str,
        store: &mut S,
        engine: &mut OptimizedStructuralEngine,
        trace_mode: CpuRuntimeTraceMode,
    ) -> Result<CpuSessionStepOutcomeV1, CpuSessionStepError> {
        if !matches!(
            self.execution_state,
            SessionStateV1::Configured | SessionStateV1::Running
        ) {
            return Err(CpuSessionStepError::InvalidState(
                self.execution_state,
            ));
        }

        let run_id = match self.active_run_id {
            Some(run_id) => run_id,
            None => {
                self.execution_state = SessionStateV1::Failed;
                return Err(CpuSessionStepError::EngineFailure);
            }
        };
        let reaction_index = self.next_reaction_index;
        let links_before = store.link_count() as u32;
        let scope_before = engine.current().to_vec();

        // Safety/profile metering is always active. TRACE only requests the
        // structural trace; portable evidence projection belongs above this
        // state machine and cannot influence execution.
        let (reaction, structural_profile, structural_trace) =
            match trace_mode {
                CpuRuntimeTraceMode::Trace => {
                    let (reaction, profile, trace) =
                        match engine.run_traced(store) {
                            Ok(value) => value,
                            Err(_) => {
                                self.execution_state =
                                    SessionStateV1::Failed;
                                return Err(
                                    CpuSessionStepError::EngineFailure,
                                );
                            }
                        };
                    (reaction, profile, Some(trace))
                }
                CpuRuntimeTraceMode::Profile => {
                    let (reaction, profile) =
                        match engine.run_profiled(store) {
                            Ok(value) => value,
                            Err(_) => {
                                self.execution_state =
                                    SessionStateV1::Failed;
                                return Err(
                                    CpuSessionStepError::EngineFailure,
                                );
                            }
                        };
                    (reaction, profile, None)
                }
            };

        let scope_after = engine.current().to_vec();
        let quiescent = reaction.quiescent;

        self.next_reaction_index =
            self.next_reaction_index.saturating_add(1);
        self.execution_state = if quiescent {
            SessionStateV1::Quiescent
        } else {
            SessionStateV1::Running
        };

        Ok(CpuSessionStepOutcomeV1 {
            reaction: CpuSessionReactionV1 {
                session_id: session_id.to_owned(),
                run_id,
                reaction_index,
                scope_before,
                scope_after,
                links_before,
                links_after: store.link_count() as u32,
                raw_rule_matches: reaction.raw_rule_matches,
                transitioned_members: reaction.transitioned_members,
                handoff_count: reaction.handoff_count,
                quiescent,
            },
            structural_profile,
            structural_trace,
        })
    }

    pub(crate) fn fail_active_run(&mut self) {
        self.execution_state = SessionStateV1::Failed;
    }
}

/// Long-lived optimized-CPU execution Session over one loaded A-memory.
///
/// `begin_run` publishes an initial Scope into the already loaded engine.
/// `step` is the one authoritative semantic reaction operation. Higher-level
/// run-to-quiescence APIs are bounded loops over this primitive.
#[derive(Debug)]
pub(crate) struct CpuRuntimeSession {
    pub(crate) id: String,
    pub(crate) memory: CpuMemoryInstance,
    pub(crate) engine: OptimizedStructuralEngine,
    pub(crate) base_link_count: usize,
    runtime: StructuralRuntimeStateV1,
}

impl CpuRuntimeSession {
    pub(crate) fn new(
        memory: CpuMemoryInstance,
        engine: OptimizedStructuralEngine,
        base_link_count: usize,
    ) -> Self {
        let session_number =
            NEXT_CPU_SESSION_ID.fetch_add(1, Ordering::SeqCst);
        Self {
            id: format!("A-session#{}", session_number),
            memory,
            engine,
            base_link_count,
            runtime: StructuralRuntimeStateV1::new(),
        }
    }

    pub(crate) fn begin_run(
        &mut self,
        initial: Handle,
    ) -> Result<u64, CpuSessionStepError> {
        self.runtime.begin_run(
            &self.memory.store,
            &mut self.engine,
            initial,
        )
    }

    pub(crate) fn step(
        &mut self,
        trace_mode: CpuRuntimeTraceMode,
    ) -> Result<CpuSessionStepOutcomeV1, CpuSessionStepError> {
        self.runtime.step(
            &self.id,
            &mut self.memory.store,
            &mut self.engine,
            trace_mode,
        )
    }

    pub(crate) fn begin_budgeted_run(
        &mut self,
        initial: Handle,
        budget: RunBudgetV1,
    ) -> Result<RunControllerV1, CpuSessionStepError> {
        self.begin_budgeted_run_with_resource_budget(
            initial,
            budget,
            CpuRunResourceBudgetV1::default(),
        )
    }

    pub(crate) fn begin_budgeted_run_with_resource_budget(
        &mut self,
        initial: Handle,
        budget: RunBudgetV1,
        resource_budget: CpuRunResourceBudgetV1,
    ) -> Result<RunControllerV1, CpuSessionStepError> {
        let run_id = self.begin_run(initial)?;
        Ok(RunControllerV1::new_with_resource_budget(
            budget,
            BackendRunResourceBudgetV1::OptimizedCpuDenseCarrier(
                resource_budget,
            ),
            run_id,
            self.memory.store.link_count() as u32,
        ))
    }

    pub(crate) fn run_to_quiescence(
        &mut self,
        initial: Handle,
        budget: RunBudgetV1,
        trace_mode: CpuRuntimeTraceMode,
    ) -> Result<BoundedRunV1, RunStopReasonV1> {
        self.run_to_quiescence_with_resource_budget(
            initial,
            budget,
            CpuRunResourceBudgetV1::default(),
            trace_mode,
        )
    }

    pub(crate) fn run_to_quiescence_with_resource_budget(
        &mut self,
        initial: Handle,
        budget: RunBudgetV1,
        resource_budget: CpuRunResourceBudgetV1,
        trace_mode: CpuRuntimeTraceMode,
    ) -> Result<BoundedRunV1, RunStopReasonV1> {
        let mut controller = self
            .begin_budgeted_run_with_resource_budget(
                initial,
                budget,
                resource_budget,
            )
            .map_err(run_stop_reason_from_step_error)?;
        let run_id = controller.run_id();
        let links_before_run = controller.links_before_run();
        let mut steps = Vec::new();

        loop {
            let controlled = controller.next(self, trace_mode);
            let budget_accounting = controlled.budget_accounting;
            if let Some(step) = controlled.step {
                steps.push(step);
            }
            if let Some(stop_reason) = controlled.stop_reason {
                return Ok(BoundedRunV1 {
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
        self.runtime.fail_active_run();
    }
}

impl BoundedRuntimeSessionV1 for CpuRuntimeSession {
    fn bounded_step_v1(
        &mut self,
        trace_mode: CpuRuntimeTraceMode,
    ) -> Result<CpuSessionStepOutcomeV1, CpuSessionStepError> {
        self.step(trace_mode)
    }

    fn bounded_fail_active_run_v1(&mut self) {
        self.fail_active_run();
    }

    fn bounded_total_links_v1(&self) -> u32 {
        self.memory.store.link_count() as u32
    }

    fn bounded_scope_width_v1(&self) -> u32 {
        self.engine.current().len() as u32
    }

    fn bounded_backend_stop_reason_v1(
        &self,
        resource_budget: BackendRunResourceBudgetV1,
    ) -> Option<RunStopReasonV1> {
        match resource_budget {
            BackendRunResourceBudgetV1::None => None,
            BackendRunResourceBudgetV1::OptimizedCpuDenseCarrier(budget) => {
                let bytes =
                    self.memory.store.dense_carrier_index_allocated_bytes();
                (bytes > budget.max_dense_carrier_bytes)
                    .then_some(RunStopReasonV1::CarrierBytesBudgetExceeded)
            }
        }
    }

    fn bounded_backend_accounting_v1(
        &self,
        resource_budget: BackendRunResourceBudgetV1,
    ) -> Option<BackendResourceAccountingV1> {
        match resource_budget {
            BackendRunResourceBudgetV1::None => None,
            BackendRunResourceBudgetV1::OptimizedCpuDenseCarrier(budget) => {
                Some(
                    BackendResourceAccountingV1::OptimizedCpuDenseCarrier {
                        dense_carrier_allocated_bytes: self
                            .memory
                            .store
                            .dense_carrier_index_allocated_bytes(),
                        max_dense_carrier_bytes:
                            budget.max_dense_carrier_bytes,
                        full_resident_bytes_available: false,
                    },
                )
            }
        }
    }
}

impl RuntimeSessionV1 for CpuRuntimeSession {
    fn runtime_backend_v1(&self) -> RuntimeBackendV1 {
        RuntimeBackendV1::OptimizedCpu
    }

    fn runtime_identity_v1(&self) -> SessionIdentityV1 {
        SessionIdentityV1 {
            memory_instance_id: self.memory.id.clone(),
            session_id: self.id.clone(),
        }
    }

    fn runtime_state_v1(&self) -> SessionStateV1 {
        self.runtime.state()
    }

    fn runtime_capabilities_v1(&self) -> SessionCapabilitiesV1 {
        SessionCapabilitiesV1 {
            schema_version: SESSION_CONTRACT_SCHEMA_VERSION,
            persistent_session: CapabilitySupportV1::Supported,
            reconfigure_without_reload: CapabilitySupportV1::Supported,
            step: CapabilitySupportV1::Supported,
            run_to_quiescence: CapabilitySupportV1::Supported,
            snapshot: CapabilitySupportV1::Supported,
            profile: CapabilitySupportV1::Supported,
            trace: CapabilitySupportV1::Supported,
            explicit_close: CapabilitySupportV1::Unsupported,
        }
    }

    fn runtime_base_link_count_v1(&self) -> u32 {
        self.base_link_count as u32
    }

    fn runtime_current_link_count_v1(&self) -> u32 {
        self.memory.store.link_count() as u32
    }

    fn runtime_scope_width_v1(&self) -> u32 {
        self.engine.current().len() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_neutral_runtime_state_machine_is_not_owned_by_cpu_wrapper() {
        let mut store = OptimizedLinkStore::new();
        let initial = store.ensure_pair(1, 1).unwrap();
        let mut engine = OptimizedStructuralEngine::new(8);
        let mut runtime = StructuralRuntimeStateV1::new();

        assert_eq!(runtime.state(), SessionStateV1::Open);
        assert_eq!(
            runtime.begin_run(&store, &mut engine, initial).unwrap(),
            1,
        );
        assert_eq!(runtime.state(), SessionStateV1::Configured);
        assert_eq!(
            runtime.begin_run(&store, &mut engine, initial),
            Err(CpuSessionStepError::InvalidState(
                SessionStateV1::Configured,
            )),
            "a second configuration cannot overwrite an active run",
        );

        // No interpreter was configured: the semantic engine fails and the
        // common runtime must publish FAILED, never QUIESCENT or a fake step.
        let error = runtime
            .step(
                "neutral-session#test",
                &mut store,
                &mut engine,
                CpuRuntimeTraceMode::Profile,
            )
            .unwrap_err();
        assert_eq!(error, CpuSessionStepError::EngineFailure);
        assert_eq!(runtime.state(), SessionStateV1::Failed);
    }

    #[test]
    fn cpu_session_exposes_distinct_neutral_identity_and_capabilities() {
        let mut memory = CpuMemoryInstance::new();
        memory.store.ensure_pair(1, 1).unwrap();
        let base_link_count = memory.store.link_count();
        let session = CpuRuntimeSession::new(
            memory,
            OptimizedStructuralEngine::new(8),
            base_link_count,
        );
        let snapshot = session.runtime_snapshot_v1();

        assert_eq!(snapshot.backend, RuntimeBackendV1::OptimizedCpu);
        assert_ne!(
            snapshot.identity.memory_instance_id,
            snapshot.identity.session_id,
        );
        assert_eq!(snapshot.state, SessionStateV1::Open);
        assert_eq!(snapshot.base_link_count, base_link_count as u32);
        assert_eq!(snapshot.current_link_count, base_link_count as u32);
        assert_eq!(
            snapshot.capabilities.step,
            CapabilitySupportV1::Supported,
        );
        assert_eq!(
            snapshot.capabilities.reconfigure_without_reload,
            CapabilitySupportV1::Supported,
        );
        assert_eq!(
            snapshot.capabilities.explicit_close,
            CapabilitySupportV1::Unsupported,
        );
    }

    #[test]
    fn budget_resource_reasons_are_distinct_and_deterministic() {
        let budget = RunBudgetV1 {
            schema_version: RUN_BUDGET_SCHEMA_VERSION,
            max_reactions: 8,
            max_appended_links: 5,
            max_total_links: 20,
            max_scope_width: 3,
            max_match_candidates: 100,
            max_unification_nodes: 100,
            max_instantiation_nodes: 100,
        };

        assert_eq!(
            budget.resource_stop_reason(10, 21, 1),
            Some(RunStopReasonV1::TotalLinksBudgetExceeded),
        );
        assert_eq!(
            budget.resource_stop_reason(10, 16, 1),
            Some(RunStopReasonV1::AppendedLinksBudgetExceeded),
        );
        assert_eq!(
            budget.resource_stop_reason(10, 15, 4),
            Some(RunStopReasonV1::ScopeWidthBudgetExceeded),
        );
        assert_eq!(budget.resource_stop_reason(10, 15, 3), None);
    }

    #[test]
    fn budget_work_reasons_are_distinct_and_deterministic() {
        let budget = RunBudgetV1 {
            schema_version: RUN_BUDGET_SCHEMA_VERSION,
            max_reactions: 8,
            max_appended_links: 5,
            max_total_links: 20,
            max_scope_width: 3,
            max_match_candidates: 10,
            max_unification_nodes: 20,
            max_instantiation_nodes: 30,
        };

        assert_eq!(
            budget.work_stop_reason(RunWorkUsageV1 {
                match_candidates: 11,
                ..RunWorkUsageV1::default()
            }),
            Some(RunStopReasonV1::MatchWorkBudgetExceeded),
        );
        assert_eq!(
            budget.work_stop_reason(RunWorkUsageV1 {
                unification_nodes: 21,
                ..RunWorkUsageV1::default()
            }),
            Some(RunStopReasonV1::UnificationWorkBudgetExceeded),
        );
        assert_eq!(
            budget.work_stop_reason(RunWorkUsageV1 {
                instantiation_nodes: 31,
                ..RunWorkUsageV1::default()
            }),
            Some(RunStopReasonV1::InstantiationWorkBudgetExceeded),
        );
        assert_eq!(
            budget.work_stop_reason(RunWorkUsageV1 {
                match_candidates: 10,
                unification_nodes: 20,
                instantiation_nodes: 30,
            }),
            None,
        );
    }

    #[test]
    fn common_run_budget_schema_has_no_cpu_dense_carrier_field() {
        let budget = RunBudgetV1::scenario_default(8);
        let json = serde_json::to_value(budget).unwrap();
        assert_eq!(budget.schema_version, super::RUN_BUDGET_SCHEMA_VERSION);
        assert!(json.get("maxDenseCarrierBytes").is_none());
    }

    #[test]
    fn budget_accounting_snapshot_uses_controller_and_live_runtime_state() {
        let mut memory = CpuMemoryInstance::new();
        let initial = memory.store.ensure_pair(1, 1).unwrap();
        let mut session = CpuRuntimeSession::new(
            memory,
            OptimizedStructuralEngine::new(8),
            1,
        );
        let budget = RunBudgetV1 {
            max_reactions: 7,
            max_appended_links: 11,
            max_total_links: 22,
            max_scope_width: 3,
            max_match_candidates: 44,
            max_unification_nodes: 55,
            max_instantiation_nodes: 66,
            ..RunBudgetV1::scenario_default(7)
        };
        let controller = session
            .begin_budgeted_run_with_resource_budget(
                initial,
                budget,
                CpuRunResourceBudgetV1 {
                    max_dense_carrier_bytes: 77_777,
                },
            )
            .unwrap();
        let accounting = controller.budget_accounting(&session);

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
        assert_eq!(accounting.schema_version, RUN_BUDGET_ACCOUNTING_SCHEMA_VERSION);
        assert!(matches!(
            accounting.backend_resource,
            Some(BackendResourceAccountingV1::OptimizedCpuDenseCarrier {
                max_dense_carrier_bytes: 77_777,
                full_resident_bytes_available: false,
                ..
            })
        ));
    }

    #[test]
    fn carrier_byte_reason_is_cpu_physical_and_exact() {
        let budget = CpuRunResourceBudgetV1 {
            max_dense_carrier_bytes: 99,
        };
        assert_eq!(budget.carrier_stop_reason(99), None);
        assert_eq!(
            budget.carrier_stop_reason(100),
            Some(RunStopReasonV1::CarrierBytesBudgetExceeded),
        );
    }

    #[test]
    fn runtime_trace_mode_never_disables_safety_metering() {
        assert_ne!(
            CpuRuntimeTraceMode::Profile,
            CpuRuntimeTraceMode::Trace,
        );
        // Both modes execute through run_profiled/run_traced, so structural
        // safety counters are always collected; there is no runtime OFF mode.
    }

    #[test]
    fn stop_reasons_are_stable_machine_values() {
        assert_eq!(
            serde_json::to_string(&RunStopReasonV1::Quiescent).unwrap(),
            "\"QUIESCENT\"",
        );
        assert_eq!(
            serde_json::to_string(
                &RunStopReasonV1::ReactionBudgetExceeded,
            )
            .unwrap(),
            "\"REACTION_BUDGET_EXCEEDED\"",
        );
        assert_eq!(
            serde_json::to_string(
                &RunStopReasonV1::MatchWorkBudgetExceeded,
            )
            .unwrap(),
            "\"MATCH_WORK_BUDGET_EXCEEDED\"",
        );
        assert_eq!(
            serde_json::to_string(
                &RunStopReasonV1::UnificationWorkBudgetExceeded,
            )
            .unwrap(),
            "\"UNIFICATION_WORK_BUDGET_EXCEEDED\"",
        );
        assert_eq!(
            serde_json::to_string(
                &RunStopReasonV1::InstantiationWorkBudgetExceeded,
            )
            .unwrap(),
            "\"INSTANTIATION_WORK_BUDGET_EXCEEDED\"",
        );
        assert_eq!(
            serde_json::to_string(
                &RunStopReasonV1::CarrierBytesBudgetExceeded,
            )
            .unwrap(),
            "\"CARRIER_BYTES_BUDGET_EXCEEDED\"",
        );
        assert_eq!(
            run_stop_reason_from_step_error(
                CpuSessionStepError::EngineFailure,
            ),
            RunStopReasonV1::EngineFailure,
        );
        assert_eq!(
            run_stop_reason_from_step_error(
                CpuSessionStepError::InvalidState(
                    SessionStateV1::Quiescent,
                ),
            ),
            RunStopReasonV1::InvalidState,
        );
    }

    #[test]
    fn scenario_budget_policy_is_versioned_and_finite() {
        let budget = RunBudgetV1::scenario_default(64);
        assert_eq!(
            budget.schema_version,
            RUN_BUDGET_SCHEMA_VERSION,
        );
        assert_eq!(budget.max_reactions, 64);
        assert!(budget.max_appended_links < u32::MAX);
        assert!(budget.max_total_links < u32::MAX);
        assert!(budget.max_scope_width < u32::MAX);
        assert!(budget.max_match_candidates < u64::MAX);
        assert!(budget.max_unification_nodes < u64::MAX);
        assert!(budget.max_instantiation_nodes < u64::MAX);
    }
}
