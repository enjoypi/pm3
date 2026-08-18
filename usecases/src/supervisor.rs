use std::collections::{HashMap, HashSet};

use entities::ProcessStatus;

use crate::{
    Ports, SignalScope,
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
    start::StartOutcome,
    stop::persist_for_handover,
    supervise::{ExitAction, handle_child_exit, settle_failed_probe},
    supervision::{SupervisionEffect, SupervisionOutcome, SupervisionReply, SupervisionRequest},
    supervisor_log::{
        log_armed, log_exit_after_delete, log_failure, log_handover, log_memory_breach,
        log_rotate_failed, log_rotated, log_settled, log_spared_force_kill, log_stale_restart,
        log_stuck_force_kill, log_unschedulable,
    },
    table::ProcessTable,
    timer_state::TimerState,
};

#[derive(Debug)]
pub struct Supervisor {
    pub(crate) table: ProcessTable,
    pub(crate) timers: TimerState,
    pub(crate) logs_dir: String,
    pub(crate) kill_timeout_ms: u64,
    pub(crate) ready_timeout_ms: u64,
    pub(crate) ready_poll_interval_ms: u64,
    pub(crate) waiters: HashMap<String, Vec<String>>,
    pub(crate) ready_failed: HashSet<String>,
}

impl Supervisor {
    #[must_use]
    pub fn new(
        logs_dir: String,
        kill_timeout_ms: u64,
        ready_timeout_ms: u64,
        ready_poll_interval_ms: u64,
    ) -> Self {
        Self {
            table: ProcessTable::new(),
            timers: TimerState::new(),
            logs_dir,
            kill_timeout_ms,
            ready_timeout_ms,
            ready_poll_interval_ms,
            waiters: HashMap::new(),
            ready_failed: HashSet::new(),
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
            SupervisionRequest::List => {
                let views: Vec<ProcessView> = list_apps(&self.table, ports.now_ms())
                    .into_iter()
                    .map(|view| self.with_next_fire(view))
                    .collect();
                let pids: Vec<u32> = views.iter().filter_map(|view| view.pid).collect();
                let samples = ports.resource_usage(&pids).await;
                Ok(SupervisionReply::Listed(
                    views
                        .into_iter()
                        .map(|view| view.with_sample(&samples))
                        .collect(),
                ))
            }
            SupervisionRequest::Describe(selector) => {
                match describe_app(&self.table, &selector, ports.now_ms()) {
                    Ok(view) => {
                        let view = self.with_next_fire(view);
                        let pids: Vec<u32> = view.pid.into_iter().collect();
                        let samples = ports.resource_usage(&pids).await;
                        Ok(SupervisionReply::Described(view.with_sample(&samples)))
                    }
                    Err(error) => Err(error.into()),
                }
            }
            SupervisionRequest::Stop(selector) => self.stop(&selector, ports, &mut effects).await,
            SupervisionRequest::Restart(selector) => {
                self.restart(&selector, resolver, ports, &mut effects).await
            }
            SupervisionRequest::Delete(selector) => {
                self.delete(&selector, ports, &mut effects).await
            }
            SupervisionRequest::Reset(selector) => self.reset(&selector, ports).await,
            SupervisionRequest::Signal { selector, signal } => {
                self.signal(&selector, &signal, ports).await
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
        if self.ready_failed.remove(name) {
            match settle_failed_probe(&mut self.table, name, ports).await {
                Ok(()) => log_settled(name, ProcessStatus::Errored),
                Err(error) => log_failure("exit", name, &error),
            }
            return effects;
        }
        let action = handle_child_exit(&mut self.table, name, outcome, ports)
            .await
            .expect("internal error: the exit guard checked the record");
        match action {
            ExitAction::RestartAfter { delay_ms } => {
                effects.push(self.queue_restart(name, delay_ms));
            }
            ExitAction::Settled { status } => log_settled(name, status),
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

    pub async fn on_log_rotate(
        &self,
        max_bytes: u64,
        interval_ms: u64,
        ports: &impl Ports,
    ) -> Vec<SupervisionEffect> {
        let effects = vec![SupervisionEffect::ScheduleLogRotate {
            delay_ms: interval_ms,
        }];
        if max_bytes == 0 {
            return effects;
        }
        match ports.rotate_logs(&self.logs_dir, max_bytes).await {
            Ok(rotated) => {
                for done in &rotated {
                    log_rotated(done);
                }
            }
            Err(error) => log_rotate_failed(&error.to_string()),
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
        if token.is_none() && !self.timers.is_current(name, generation) {
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

    pub(crate) fn dispatch_restart(
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

    pub(crate) fn disarm_everything(&mut self) -> Vec<SupervisionEffect> {
        let mut effects = Vec::new();
        for name in self.timers.disarm_all() {
            effects.push(SupervisionEffect::DisarmTimer { name });
        }
        for name in self.timers.cancel_all_restarts() {
            effects.push(SupervisionEffect::CancelRestart { name });
        }
        effects
    }

    pub(crate) fn disarm(&mut self, name: &str, effects: &mut Vec<SupervisionEffect>) {
        self.timers.disarm(name);
        effects.push(SupervisionEffect::DisarmTimer {
            name: name.to_string(),
        });
    }

    pub(crate) fn cancel_restart(&mut self, name: &str, effects: &mut Vec<SupervisionEffect>) {
        self.timers.claim_restart(name);
        effects.push(SupervisionEffect::CancelRestart {
            name: name.to_string(),
        });
    }

    pub(crate) fn arm_timer(
        &mut self,
        name: &str,
        ports: &impl Ports,
        effects: &mut Vec<SupervisionEffect>,
    ) {
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

    pub(crate) fn schedule_force_kill(
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

    pub(crate) fn watch_all(
        &mut self,
        outcomes: &[StartOutcome],
        effects: &mut Vec<SupervisionEffect>,
    ) {
        for outcome in outcomes {
            self.watch(outcome, effects);
        }
    }

    pub(crate) fn watch(&mut self, started: &StartOutcome, effects: &mut Vec<SupervisionEffect>) {
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
        self.await_ready_if_probing(&started.name, generation, effects);
    }

    fn await_ready_if_probing(
        &self,
        name: &str,
        generation: u64,
        effects: &mut Vec<SupervisionEffect>,
    ) {
        let record = self
            .table
            .find_by_name(name)
            .expect("internal error: a watched outcome always has a record");
        let Some(probe) = record.spec.ready_probe.clone() else {
            return;
        };
        if record.runtime.status != ProcessStatus::Launching {
            return;
        }
        effects.push(SupervisionEffect::AwaitReady {
            name: name.to_string(),
            generation,
            probe,
            timeout_ms: record
                .spec
                .listen_timeout_ms
                .unwrap_or(self.ready_timeout_ms),
            interval_ms: self.ready_poll_interval_ms,
        });
    }

    fn survivor_pids(&self, tracked: &[u32]) -> Vec<u32> {
        unswept_pids(tracked, &running_pids(&self.table))
    }

    pub(crate) fn identity_token(&self, name: &str) -> Option<String> {
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

#[cfg(test)]
#[path = "tests/supervisor_stop_tests.rs"]
mod tests;
