use entities::AppSpec;

use crate::{
    Ports, SignalScope,
    delete::delete_app,
    fingerprint::pid_was_recycled,
    ports::{ExitOutcome, SpecResolver},
    query::{
        armed_schedule_names, breached_memory, describe_app, identity_token_of, list_apps,
        memory_watch_list, owner_of_pid, running_pids, schedule_of, unswept_pids,
    },
    record::ProcessView,
    restart::{RestartOutcome, restart_app},
    resurrect::resurrect,
    selector::AppSelector,
    start::{StartOutcome, StartReport, refused_services, start_apps},
    stop::{persist_for_handover, stop_all_apps, stop_app},
    supervise::{ExitAction, handle_child_exit},
    supervision::{
        SupervisionEffect, SupervisionFailure, SupervisionOutcome, SupervisionReply,
        SupervisionRequest,
    },
    supervisor_log::{
        log_armed, log_exit_after_delete, log_failure, log_handover, log_memory_breach,
        log_partial_start, log_settled, log_spared_force_kill, log_stale_restart,
        log_stuck_force_kill, log_unschedulable,
    },
    table::ProcessTable,
    timer_state::TimerState,
};

#[derive(Debug)]
pub struct Supervisor {
    table: ProcessTable,
    timers: TimerState,
    logs_dir: String,
    kill_timeout_ms: u64,
}

impl Supervisor {
    #[must_use]
    pub fn new(logs_dir: String, kill_timeout_ms: u64) -> Self {
        Self {
            table: ProcessTable::new(),
            timers: TimerState::new(),
            logs_dir,
            kill_timeout_ms,
        }
    }

    #[must_use]
    pub fn unsettled(&self) -> usize {
        crate::query::unsettled_count(&self.table)
    }

    #[must_use]
    pub fn drained(&self, tracked: &[u32]) -> bool {
        self.survivor_pids(tracked).is_empty()
    }

    pub async fn force_kill_survivors(&self, tracked: &[u32], ports: &impl Ports) {
        for pid in self.survivor_pids(tracked) {
            let (name, token) = owner_of_pid(&self.table, pid);
            self.sweep_pid(&name, pid, token.as_deref(), ports).await;
        }
    }

    pub async fn resurrect_saved(&mut self, ports: &impl Ports) -> Vec<SupervisionEffect> {
        let mut effects = Vec::new();
        match resurrect(&mut self.table, &self.logs_dir, self.kill_timeout_ms, ports).await {
            Ok(outcomes) => self.watch_all(&outcomes, &mut effects),
            Err(error) => log_failure("resurrect", "-", &error),
        }
        for name in armed_schedule_names(&self.table) {
            self.arm_timer(&name, ports, &mut effects);
        }
        effects
    }

    pub async fn handle(
        &mut self,
        request: SupervisionRequest,
        resolver: &impl SpecResolver,
        ports: &impl Ports,
    ) -> (SupervisionOutcome, Vec<SupervisionEffect>) {
        let mut effects = Vec::new();
        let outcome = match request {
            SupervisionRequest::Start { services } => {
                self.start(&services, resolver, ports, &mut effects).await
            }
            SupervisionRequest::List => Ok(SupervisionReply::Listed(
                list_apps(&self.table, ports.now_ms())
                    .into_iter()
                    .map(|view| self.with_next_fire(view))
                    .collect(),
            )),
            SupervisionRequest::Describe(selector) => {
                describe_app(&self.table, &selector, ports.now_ms())
                    .map(|view| SupervisionReply::Described(self.with_next_fire(view)))
                    .map_err(Into::into)
            }
            SupervisionRequest::Stop(selector) => self.stop(&selector, ports, &mut effects).await,
            SupervisionRequest::Restart(selector) => {
                self.restart(&selector, resolver, ports, &mut effects).await
            }
            SupervisionRequest::Delete(selector) => {
                self.delete(&selector, ports, &mut effects).await
            }
            SupervisionRequest::StopAll => self.stop_all(ports, &mut effects).await,
        };
        (outcome, effects)
    }

    pub async fn on_exit(
        &mut self,
        name: &str,
        generation: u64,
        outcome: ExitOutcome,
        ports: &impl Ports,
    ) -> Vec<SupervisionEffect> {
        if !self.timers.is_current(name, generation) {
            return Vec::new();
        }
        let mut effects = vec![SupervisionEffect::CancelForceKill {
            name: name.to_string(),
        }];
        if self.table.find_by_name(name).is_none() {
            log_exit_after_delete(name);
            return effects;
        }
        match handle_child_exit(&mut self.table, name, outcome, ports).await {
            Ok(ExitAction::RestartAfter { delay_ms }) => {
                effects.push(self.queue_restart(name, delay_ms));
            }
            Ok(ExitAction::Settled { status }) => log_settled(name, status),
            Err(error) => log_failure("exit", name, &error),
        }
        effects
    }

    pub fn queue_restart(&mut self, name: &str, delay_ms: u64) -> SupervisionEffect {
        self.timers.queue_restart(name);
        SupervisionEffect::ScheduleRestart {
            name: name.to_string(),
            delay_ms,
        }
    }

    pub async fn on_restart(&mut self, name: &str, ports: &impl Ports) -> Vec<SupervisionEffect> {
        if !self.timers.claim_restart(name) {
            log_stale_restart(name);
            return Vec::new();
        }
        let mut effects = Vec::new();
        self.restart_now(name, ports, &mut effects).await;
        effects
    }

    pub async fn on_fire(
        &mut self,
        name: &str,
        fire_at_ms: u64,
        ports: &impl Ports,
    ) -> Vec<SupervisionEffect> {
        if !self.timers.fire_is_due(name, fire_at_ms) {
            return Vec::new();
        }
        let mut effects = Vec::new();
        self.restart_now(name, ports, &mut effects).await;
        effects
    }

    pub async fn on_memory_sample(
        &mut self,
        interval_ms: u64,
        ports: &impl Ports,
    ) -> Vec<SupervisionEffect> {
        let mut effects = vec![SupervisionEffect::ScheduleMemorySample {
            delay_ms: interval_ms,
        }];
        let watched = memory_watch_list(&self.table);
        if watched.is_empty() {
            return effects;
        }
        let pids: Vec<u32> = watched.iter().map(|watch| watch.pid).collect();
        let sampled = ports.resident_memory(&pids).await;
        for breach in breached_memory(&watched, &sampled) {
            log_memory_breach(&breach);
            self.restart_now(&breach.name, ports, &mut effects).await;
        }
        effects
    }

    pub async fn on_force_kill(
        &self,
        name: &str,
        generation: u64,
        pid: u32,
        token: Option<&str>,
        ports: &impl Ports,
    ) {
        if !self.timers.is_current(name, generation) {
            return;
        }
        if !ports.tracked_pids().await.contains(&pid) {
            return;
        }
        self.sweep_pid(name, pid, token, ports).await;
    }

    async fn sweep_pid(&self, name: &str, pid: u32, token: Option<&str>, ports: &impl Ports) {
        if pid_was_recycled(&ports.identity(pid).await, token) {
            log_spared_force_kill(name, pid);
            return;
        }
        if let Err(error) = ports.force_kill(pid, SignalScope::ProcessGroup).await {
            log_stuck_force_kill(name, pid, &error.to_string());
        }
    }

    pub async fn prepare_shutdown(&mut self, ports: &impl Ports) -> Vec<SupervisionEffect> {
        let effects = self.disarm_everything();
        match persist_for_handover(&self.table, ports).await {
            Ok(draining) => log_handover(draining.len()),
            Err(error) => log_failure("shutdown", "-", &error),
        }
        effects
    }

    async fn start(
        &mut self,
        services: &[String],
        resolver: &impl SpecResolver,
        ports: &impl Ports,
        effects: &mut Vec<SupervisionEffect>,
    ) -> SupervisionOutcome {
        let mut specs: Vec<AppSpec> = Vec::with_capacity(services.len());
        for name in services {
            specs.push(resolver.prepare(name).await?);
        }
        let StartReport {
            outcomes,
            failure,
            unsaved,
        } = start_apps(&mut self.table, &specs, &self.logs_dir, ports).await;
        self.watch_all(&outcomes, effects);
        for outcome in &outcomes {
            self.cancel_restart(&outcome.name, effects);
            self.arm_timer(&outcome.name, ports, effects);
        }
        if outcomes.is_empty() {
            if let Some(error) = failure.or(unsaved) {
                return Err(error.into());
            }
            return Ok(SupervisionReply::Started {
                outcomes,
                refused: Vec::new(),
                reason: None,
                unsaved: None,
            });
        }
        let refused = refused_services(services, &outcomes);
        let reason = failure
            .inspect(|error| log_partial_start(&refused, error))
            .as_ref()
            .map(ToString::to_string);
        Ok(SupervisionReply::Started {
            outcomes,
            refused,
            reason,
            unsaved: unsaved.as_ref().map(ToString::to_string),
        })
    }

    async fn stop(
        &mut self,
        selector: &AppSelector,
        ports: &impl Ports,
        effects: &mut Vec<SupervisionEffect>,
    ) -> SupervisionOutcome {
        let outcome = stop_app(&mut self.table, selector, ports).await?;
        self.disarm(&outcome.name, effects);
        self.cancel_restart(&outcome.name, effects);
        let token = self.identity_token(&outcome.name);
        self.schedule_force_kill(&outcome.name, outcome.force_kill_pid, token, effects);
        Ok(SupervisionReply::Stopped { name: outcome.name })
    }

    async fn restart(
        &mut self,
        selector: &AppSelector,
        resolver: &impl SpecResolver,
        ports: &impl Ports,
        effects: &mut Vec<SupervisionEffect>,
    ) -> SupervisionOutcome {
        self.reload_declaration(selector, resolver).await?;
        let outcome = restart_app(&mut self.table, selector, &self.logs_dir, ports).await?;
        let name = self.dispatch_restart(outcome, effects);
        self.cancel_restart(&name, effects);
        self.arm_timer(&name, ports, effects);
        Ok(SupervisionReply::Restarted { name })
    }

    async fn reload_declaration(
        &mut self,
        selector: &AppSelector,
        resolver: &impl SpecResolver,
    ) -> Result<(), SupervisionFailure> {
        let Some(record) = self.table.find_mut(selector) else {
            return Ok(());
        };
        let name = record.runtime.name.clone();
        record.spec = resolver.prepare(&name).await?;
        Ok(())
    }

    async fn delete(
        &mut self,
        selector: &AppSelector,
        ports: &impl Ports,
        effects: &mut Vec<SupervisionEffect>,
    ) -> SupervisionOutcome {
        let token = identity_token_of(&self.table, selector);
        let outcome = delete_app(&mut self.table, selector, ports).await?;
        self.disarm(&outcome.name, effects);
        self.cancel_restart(&outcome.name, effects);
        self.schedule_force_kill(&outcome.name, outcome.force_kill_pid, token, effects);
        Ok(SupervisionReply::Deleted { name: outcome.name })
    }

    async fn stop_all(
        &mut self,
        ports: &impl Ports,
        effects: &mut Vec<SupervisionEffect>,
    ) -> SupervisionOutcome {
        effects.extend(self.disarm_everything());
        let stopped = stop_all_apps(&mut self.table, ports).await?;
        let mut names = Vec::with_capacity(stopped.len());
        let mut covered = Vec::with_capacity(stopped.len());
        for outcome in &stopped {
            names.push(outcome.name.clone());
            covered.extend(outcome.force_kill_pid);
            let token = self.identity_token(&outcome.name);
            self.schedule_force_kill(&outcome.name, outcome.force_kill_pid, token, effects);
        }
        let tracked = ports.tracked_pids().await;
        for pid in unswept_pids(&tracked, &covered) {
            let (name, token) = owner_of_pid(&self.table, pid);
            self.schedule_force_kill(&name, Some(pid), token, effects);
        }
        Ok(SupervisionReply::StoppedAll { names })
    }

    async fn restart_now(
        &mut self,
        name: &str,
        ports: &impl Ports,
        effects: &mut Vec<SupervisionEffect>,
    ) {
        self.cancel_restart(name, effects);
        let attempt = restart_app(
            &mut self.table,
            &AppSelector::Name(name.to_string()),
            &self.logs_dir,
            ports,
        )
        .await;
        match attempt {
            Ok(outcome) => {
                let restarted = self.dispatch_restart(outcome, effects);
                self.arm_timer(&restarted, ports, effects);
            }
            Err(error) => {
                log_failure("restart", name, &error);
                self.arm_timer(name, ports, effects);
            }
        }
    }

    fn dispatch_restart(
        &mut self,
        outcome: RestartOutcome,
        effects: &mut Vec<SupervisionEffect>,
    ) -> String {
        match outcome {
            RestartOutcome::Started(started) => {
                let name = started.name.clone();
                self.watch(&started, effects);
                name
            }
            RestartOutcome::AwaitingExit {
                name,
                force_kill_pid,
            } => {
                let token = self.identity_token(&name);
                self.schedule_force_kill(&name, force_kill_pid, token, effects);
                name
            }
        }
    }

    fn disarm_everything(&mut self) -> Vec<SupervisionEffect> {
        let mut effects = Vec::new();
        for name in self.timers.disarm_all() {
            effects.push(SupervisionEffect::DisarmTimer { name });
        }
        for name in self.timers.cancel_all_restarts() {
            effects.push(SupervisionEffect::CancelRestart { name });
        }
        effects
    }

    fn disarm(&mut self, name: &str, effects: &mut Vec<SupervisionEffect>) {
        self.timers.disarm(name);
        effects.push(SupervisionEffect::DisarmTimer {
            name: name.to_string(),
        });
    }

    fn cancel_restart(&mut self, name: &str, effects: &mut Vec<SupervisionEffect>) {
        self.timers.claim_restart(name);
        effects.push(SupervisionEffect::CancelRestart {
            name: name.to_string(),
        });
    }

    fn arm_timer(&mut self, name: &str, ports: &impl Ports, effects: &mut Vec<SupervisionEffect>) {
        self.disarm(name, effects);
        let Some(cron) = schedule_of(&self.table, name) else {
            return;
        };
        let now_ms = ports.now_ms();
        let Some(fire_at_ms) = ports.next_fire_ms(&cron, now_ms) else {
            log_unschedulable(name, &cron);
            return;
        };
        log_armed(name, fire_at_ms);
        self.timers.arm(name, fire_at_ms);
        effects.push(SupervisionEffect::ArmTimer {
            name: name.to_string(),
            fire_at_ms,
            delay_ms: fire_at_ms.saturating_sub(now_ms),
        });
    }

    fn schedule_force_kill(
        &self,
        name: &str,
        pid: Option<u32>,
        token: Option<String>,
        effects: &mut Vec<SupervisionEffect>,
    ) {
        let Some(pid) = pid else {
            return;
        };
        effects.push(SupervisionEffect::ScheduleForceKill {
            name: name.to_string(),
            generation: self.timers.current_generation(name),
            pid,
            token,
            delay_ms: self.kill_timeout_ms,
        });
    }

    fn watch_all(&mut self, outcomes: &[StartOutcome], effects: &mut Vec<SupervisionEffect>) {
        for outcome in outcomes {
            self.watch(outcome, effects);
        }
    }

    fn watch(&mut self, started: &StartOutcome, effects: &mut Vec<SupervisionEffect>) {
        let (Some(pid), true) = (started.pid, started.kind.needs_watching()) else {
            return;
        };
        let token = self.identity_token(&started.name);
        let generation = self.timers.bump(&started.name);
        effects.push(SupervisionEffect::WatchExit {
            name: started.name.clone(),
            generation,
            pid,
            token,
        });
    }

    fn survivor_pids(&self, tracked: &[u32]) -> Vec<u32> {
        unswept_pids(tracked, &running_pids(&self.table))
    }

    fn identity_token(&self, name: &str) -> Option<String> {
        identity_token_of(&self.table, &AppSelector::Name(name.to_string()))
    }

    fn with_next_fire(&self, view: ProcessView) -> ProcessView {
        let next_fire_ms = self.timers.next_fire_of(&view.name);
        ProcessView {
            next_fire_ms,
            ..view
        }
    }
}
