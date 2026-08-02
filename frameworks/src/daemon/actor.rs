use std::{sync::Arc, time::Duration};

use adapters::{
    AppSelector, Clock as _, DaemonCommand, DaemonOutcome, DaemonReply, DaemonRequest, ExitAction,
    ExitOutcome, Liveness, ProcessProbe as _, ProcessTable, ProcessView, RestartOutcome,
    Scheduler as _, Signaler as _, SpecSource, StartKind, StartOutcome, StartReport, delete_app,
    describe_app, handle_child_exit, list_apps, materialise_workspace, persist_for_handover,
    restart_app, resurrect, start_apps, stop_all_apps, stop_app,
};
use tokio::sync::mpsc;

use super::{
    events::DaemonEvent,
    logging::{
        log_failure, log_handover, log_partial_start, log_settled, log_spared_force_kill,
        log_stale_restart,
    },
    ports::DaemonPorts,
    timers::{TimerBoard, log_armed, log_unschedulable},
};

#[derive(Debug)]
pub struct Daemon {
    pub(super) events: mpsc::Sender<DaemonEvent>,
    table: ProcessTable,
    ports: Arc<DaemonPorts>,
    specs: SpecSource,
    board: TimerBoard,
}

impl Daemon {
    #[must_use]
    pub fn new(
        specs: SpecSource,
        ports: Arc<DaemonPorts>,
        events: mpsc::Sender<DaemonEvent>,
    ) -> Self {
        Self {
            table: ProcessTable::new(),
            ports,
            specs,
            board: TimerBoard::new(events.clone()),
            events,
        }
    }

    pub async fn resurrect_saved_apps(&mut self) {
        let kill_timeout_ms = self.specs.config.kill_timeout_ms;
        match resurrect(
            &mut self.table,
            &self.specs.logs_dir,
            kill_timeout_ms,
            &*self.ports,
        )
        .await
        {
            Ok(outcomes) => self.watch_all(&outcomes),
            Err(error) => log_failure("resurrect", "-", &error),
        }
        self.arm_known_timers();
    }

    pub async fn handle(&mut self, request: DaemonRequest) -> DaemonOutcome {
        match request {
            DaemonRequest::Start { services } => self.start(&services).await,
            DaemonRequest::List => Ok(DaemonReply::Listed(
                list_apps(&self.table, self.ports.now_ms())
                    .into_iter()
                    .map(|view| self.with_next_fire(view))
                    .collect(),
            )),
            DaemonRequest::Describe(selector) => self.describe(&selector),
            DaemonRequest::Stop(selector) => self.stop(&selector).await,
            DaemonRequest::Restart(selector) => self.restart(&selector).await,
            DaemonRequest::Delete(selector) => self.delete(&selector).await,
            DaemonRequest::StopAll => self.stop_all().await,
        }
    }

    pub async fn on_exit(&mut self, name: &str, generation: u64, outcome: ExitOutcome) {
        if !self.board.is_current(name, generation) {
            return;
        }
        self.board.cancel_force_kill(name);
        match handle_child_exit(&mut self.table, name, outcome, &*self.ports).await {
            Ok(ExitAction::RestartAfter { delay_ms }) => {
                self.board.schedule_restart(name, delay_ms);
            }
            Ok(ExitAction::Settled { status }) => log_settled(name, status),
            Err(error) => log_failure("exit", name, &error),
        }
    }

    pub async fn on_restart(&mut self, name: &str) {
        if !self.board.claim_restart(name) {
            log_stale_restart(name);
            return;
        }
        self.restart_now(name).await;
    }

    pub(super) async fn restart_now(&mut self, name: &str) {
        self.board.cancel_restart(name);
        let attempt = restart_app(
            &mut self.table,
            &AppSelector::Name(name.to_string()),
            &self.specs.logs_dir,
            &*self.ports,
        )
        .await;
        match attempt {
            Ok(outcome) => {
                let restarted = self.dispatch_restart(outcome);
                self.arm_timer(&restarted);
            }
            Err(error) => log_failure("restart", name, &error),
        }
    }

    fn dispatch_restart(&mut self, outcome: RestartOutcome) -> String {
        match outcome {
            RestartOutcome::Started(started) => {
                let name = started.name.clone();
                self.watch(&started);
                name
            }
            RestartOutcome::AwaitingExit {
                name,
                force_kill_pid,
            } => {
                let token = self.identity_token(&name);
                self.schedule_force_kill(&name, force_kill_pid, token);
                name
            }
        }
    }

    pub async fn on_force_kill(&self, name: &str, generation: u64, pid: u32, token: Option<&str>) {
        if !self.board.is_current(name, generation) {
            return;
        }
        if !self.ports.tracked_pids().await.contains(&pid) {
            return;
        }
        if self.pid_was_recycled(pid, token).await {
            log_spared_force_kill(name, pid);
            return;
        }
        self.ports.force_kill(pid).await.ok();
    }

    async fn pid_was_recycled(&self, pid: u32, token: Option<&str>) -> bool {
        let (Liveness::Alive(current), Some(expected)) = (self.ports.identity(pid).await, token)
        else {
            return false;
        };
        current != expected
    }

    pub async fn shutdown(&mut self) {
        self.board.disarm_all();
        self.board.cancel_all_restarts();
        match persist_for_handover(&self.table, &*self.ports).await {
            Ok(draining) => log_handover(draining.len()),
            Err(error) => log_failure("shutdown", "-", &error),
        }
        if !self.wait_until_drained().await {
            self.force_kill_survivors().await;
        }
        let survivors = self
            .table
            .records()
            .iter()
            .filter(|record| !record.runtime.status.is_settled())
            .count();
        tracing::info!(
            feature = "lifecycle",
            operation = "shutdown",
            mode = "preserve",
            survivors,
            "pm3 daemon is leaving its services running for the next daemon to reclaim",
        );
    }

    pub(super) async fn apply(&mut self, event: DaemonEvent) {
        match event {
            DaemonEvent::Command(command) => {
                let DaemonCommand { request, reply } = command;
                let outcome = self.handle(request).await;
                reply.send(outcome).ok();
            }
            DaemonEvent::Exited {
                name,
                generation,
                outcome,
            } => self.on_exit(&name, generation, outcome).await,
            DaemonEvent::Restart { name } => self.on_restart(&name).await,
            DaemonEvent::Fire { name, fire_at_ms } => self.on_fire(&name, fire_at_ms).await,
            DaemonEvent::ForceKill {
                name,
                generation,
                pid,
                token,
            } => {
                self.on_force_kill(&name, generation, pid, token.as_deref())
                    .await;
            }
            DaemonEvent::Shutdown => self.shutdown().await,
        }
    }

    async fn start(&mut self, services: &[String]) -> DaemonOutcome {
        let mut specs = Vec::with_capacity(services.len());
        for name in services {
            specs.push(self.specs.resolve_service(name).await?);
        }
        for spec in &mut specs {
            materialise_workspace(spec).await;
        }
        let report = start_apps(&mut self.table, &specs, &self.specs.logs_dir, &*self.ports).await;
        let StartReport { outcomes, failure } = report;
        self.watch_all(&outcomes);
        for outcome in &outcomes {
            self.board.cancel_restart(&outcome.name);
            match outcome.kind {
                StartKind::AlreadyRunning => self.reconcile_timer(&outcome.name),
                StartKind::Spawned | StartKind::Adopted | StartKind::Scheduled => {
                    self.arm_timer(&outcome.name);
                }
            }
        }
        let Some(error) = failure else {
            return Ok(DaemonReply::Started {
                outcomes,
                refused: Vec::new(),
                reason: None,
            });
        };
        if outcomes.is_empty() {
            return Err(error.into());
        }
        let refused = refused_services(services, &outcomes);
        log_partial_start(&refused, &error);
        Ok(DaemonReply::Started {
            outcomes,
            refused,
            reason: Some(error.to_string()),
        })
    }

    fn describe(&self, selector: &AppSelector) -> DaemonOutcome {
        let view = describe_app(&self.table, selector, self.ports.now_ms())?;
        Ok(DaemonReply::Described(self.with_next_fire(view)))
    }

    async fn stop(&mut self, selector: &AppSelector) -> DaemonOutcome {
        let outcome = stop_app(&mut self.table, selector, &*self.ports).await?;
        self.board.disarm(&outcome.name);
        self.board.cancel_restart(&outcome.name);
        let token = self.identity_token(&outcome.name);
        self.schedule_force_kill(&outcome.name, outcome.force_kill_pid, token);
        Ok(DaemonReply::Stopped { name: outcome.name })
    }

    async fn restart(&mut self, selector: &AppSelector) -> DaemonOutcome {
        let outcome = restart_app(
            &mut self.table,
            selector,
            &self.specs.logs_dir,
            &*self.ports,
        )
        .await?;
        let name = self.dispatch_restart(outcome);
        self.board.cancel_restart(&name);
        self.arm_timer(&name);
        Ok(DaemonReply::Restarted { name })
    }

    async fn delete(&mut self, selector: &AppSelector) -> DaemonOutcome {
        let token = self.identity_token_of(selector);
        let outcome = delete_app(&mut self.table, selector, &*self.ports).await?;
        self.board.disarm(&outcome.name);
        self.board.cancel_restart(&outcome.name);
        self.board.forget_generation(&outcome.name);
        self.schedule_force_kill(&outcome.name, outcome.force_kill_pid, token);
        Ok(DaemonReply::Deleted { name: outcome.name })
    }

    async fn stop_all(&mut self) -> DaemonOutcome {
        self.board.disarm_all();
        self.board.cancel_all_restarts();
        let stopped = stop_all_apps(&mut self.table, &*self.ports).await?;
        let mut names = Vec::with_capacity(stopped.len());
        let mut covered = Vec::with_capacity(stopped.len());
        for outcome in &stopped {
            names.push(outcome.name.clone());
            covered.extend(outcome.force_kill_pid);
            let token = self.identity_token(&outcome.name);
            self.schedule_force_kill(&outcome.name, outcome.force_kill_pid, token);
        }
        self.sweep_strays(&covered).await;
        Ok(DaemonReply::StoppedAll { names })
    }

    async fn sweep_strays(&mut self, covered: &[u32]) {
        let strays: Vec<u32> = self
            .ports
            .tracked_pids()
            .await
            .into_iter()
            .filter(|pid| !covered.contains(pid))
            .collect();
        for pid in strays {
            let (name, token) = self.stray_owner(pid);
            self.schedule_force_kill(&name, Some(pid), token);
        }
    }

    fn stray_owner(&self, pid: u32) -> (String, Option<String>) {
        let Some(record) = self
            .table
            .records()
            .iter()
            .find(|record| record.runtime.pid == Some(pid))
        else {
            return (format!("stray-{pid}"), None);
        };
        let token = record
            .runtime
            .identity
            .as_ref()
            .map(|identity| identity.token.clone());
        (record.runtime.name.clone(), token)
    }

    async fn wait_until_drained(&self) -> bool {
        let budget_ms = self.specs.config.kill_timeout_ms;
        let step_ms = self.specs.config.daemon_poll_interval_ms.max(1);
        let mut waited_ms = 0;
        while waited_ms < budget_ms {
            if self.drained().await {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(step_ms)).await;
            waited_ms = waited_ms.saturating_add(step_ms);
        }
        self.drained().await
    }

    async fn drained(&self) -> bool {
        let tracked = self.ports.tracked_pids().await;
        let preserved = self.preserved_pids();
        tracked.iter().all(|pid| preserved.contains(pid))
    }

    fn preserved_pids(&self) -> Vec<u32> {
        self.table
            .records()
            .iter()
            .filter(|record| record.runtime.status.is_running())
            .filter_map(|record| record.runtime.pid)
            .collect()
    }

    async fn force_kill_survivors(&self) {
        let preserved = self.preserved_pids();
        let mut sweeps = Vec::new();
        for pid in self.ports.tracked_pids().await {
            if preserved.contains(&pid) {
                continue;
            }
            let ports = Arc::clone(&self.ports);
            sweeps.push(tokio::spawn(
                async move { ports.force_kill(pid).await.ok() },
            ));
        }
        for sweep in sweeps {
            sweep.await.ok();
        }
    }

    pub async fn on_fire(&mut self, name: &str, fire_at_ms: u64) {
        if self.board.next_fire_of(name) != Some(fire_at_ms) {
            return;
        }
        self.restart_now(name).await;
    }

    fn with_next_fire(&self, view: ProcessView) -> ProcessView {
        let next_fire_ms = self.board.next_fire_of(&view.name);
        ProcessView {
            next_fire_ms,
            ..view
        }
    }

    fn arm_known_timers(&mut self) {
        for name in self.scheduled_names() {
            self.arm_timer(&name);
        }
    }

    fn scheduled_names(&self) -> Vec<String> {
        self.table
            .records()
            .iter()
            .filter(|record| record.spec.schedule.is_some() && record.runtime.schedule_armed)
            .map(|record| record.runtime.name.clone())
            .collect()
    }

    fn arm_timer(&mut self, name: &str) {
        self.board.disarm(name);
        let Some(cron) = self.schedule_of(name) else {
            return;
        };
        let now_ms = self.ports.now_ms();
        let Some(fire_at_ms) = self.ports.next_fire_ms(&cron, now_ms) else {
            log_unschedulable(name, &cron);
            return;
        };
        log_armed(name, fire_at_ms);
        let delay = Duration::from_millis(fire_at_ms.saturating_sub(now_ms));
        self.board.arm(name, fire_at_ms, delay);
    }

    fn schedule_of(&self, name: &str) -> Option<String> {
        self.table
            .find(&AppSelector::Name(name.to_string()))
            .and_then(|record| record.spec.schedule.clone())
    }

    fn reconcile_timer(&mut self, name: &str) {
        if self.schedule_of(name).is_some() {
            self.arm_timer(name);
        } else {
            self.board.disarm(name);
        }
    }

    fn schedule_force_kill(&mut self, name: &str, pid: Option<u32>, token: Option<String>) {
        let delay = Duration::from_millis(self.specs.config.kill_timeout_ms);
        self.board.schedule_force_kill(name, pid, token, delay);
    }

    fn watch_all(&mut self, outcomes: &[StartOutcome]) {
        for outcome in outcomes {
            self.watch(outcome);
        }
    }

    fn watch(&mut self, started: &StartOutcome) {
        let (Some(pid), true) = (started.pid, started.kind.needs_watching()) else {
            return;
        };
        let token = self.identity_token(&started.name);
        let generation = self.board.bump(&started.name);
        let ports = Arc::clone(&self.ports);
        let events = self.events.clone();
        let name = started.name.clone();
        tokio::spawn(async move {
            let outcome = ports
                .wait(pid, token)
                .await
                .unwrap_or(ExitOutcome { exit_code: None });
            events
                .send(DaemonEvent::Exited {
                    name,
                    generation,
                    outcome,
                })
                .await
                .ok();
        });
    }

    fn identity_token(&self, name: &str) -> Option<String> {
        self.identity_token_of(&AppSelector::Name(name.to_string()))
    }

    fn identity_token_of(&self, selector: &AppSelector) -> Option<String> {
        self.table
            .find(selector)
            .and_then(|record| record.runtime.identity.as_ref())
            .map(|identity| identity.token.clone())
    }
}

fn refused_services(requested: &[String], outcomes: &[StartOutcome]) -> Vec<String> {
    requested
        .iter()
        .filter(|name| !outcomes.iter().any(|outcome| &outcome.name == *name))
        .cloned()
        .collect()
}

#[cfg(test)]
#[path = "../tests/daemon_actor_cron_tests.rs"]
mod cron_tests;
#[cfg(test)]
#[path = "../tests/daemon_actor_lifecycle_tests.rs"]
mod lifecycle_tests;
#[cfg(test)]
#[path = "../tests/daemon_actor_shared_tests.rs"]
mod shared;
#[cfg(test)]
#[path = "../test_helpers/daemon_actor_test_helpers.rs"]
mod test_helpers;
#[cfg(test)]
#[path = "../tests/daemon_actor_tests.rs"]
mod tests;
