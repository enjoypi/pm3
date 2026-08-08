use std::{sync::Arc, time::Duration};

use adapters::{
    DaemonCommand, ExitOutcome, ProcessLauncher as _, SpecSource, SupervisionEffect,
    SupervisionOutcome, SupervisionRequest, Supervisor,
};
use tokio::sync::mpsc;

use super::{events::DaemonEvent, ports::DaemonPorts, timers::TaskBoard};

#[derive(Debug)]
pub struct Daemon {
    pub(super) events: mpsc::Sender<DaemonEvent>,
    supervisor: Supervisor,
    ports: Arc<DaemonPorts>,
    specs: SpecSource,
    board: TaskBoard,
    poll_interval_ms: u64,
    kill_timeout_ms: u64,
    memory_poll_interval_ms: u64,
    log_rotate_max_bytes: u64,
    log_rotate_interval_ms: u64,
}

impl Daemon {
    #[must_use]
    pub fn new(
        specs: SpecSource,
        ports: Arc<DaemonPorts>,
        events: mpsc::Sender<DaemonEvent>,
    ) -> Self {
        let kill_timeout_ms = specs.config.kill_timeout_ms;
        Self {
            supervisor: Supervisor::new(
                specs.logs_dir.clone(),
                kill_timeout_ms,
                specs.config.ready_timeout_ms,
                specs.config.ready_poll_interval_ms.max(1),
            ),
            board: TaskBoard::new(events.clone(), Arc::clone(&ports)),
            ports,
            poll_interval_ms: specs.config.daemon_poll_interval_ms.max(1),
            kill_timeout_ms,
            memory_poll_interval_ms: specs.config.memory_poll_interval_ms.max(1),
            log_rotate_max_bytes: specs.config.log_rotate_max_bytes,
            log_rotate_interval_ms: specs.config.log_rotate_interval_ms.max(1),
            specs,
            events,
        }
    }

    pub async fn resurrect_saved_apps(&mut self) {
        let effects = self.supervisor.resurrect_saved(&*self.ports).await;
        self.run(effects);
    }

    pub async fn handle(&mut self, request: SupervisionRequest) -> SupervisionOutcome {
        let (outcome, effects) = self
            .supervisor
            .handle(request, &self.specs, &*self.ports)
            .await;
        self.run(effects);
        outcome
    }

    pub async fn on_exit(&mut self, name: &str, generation: u64, outcome: ExitOutcome) {
        let effects = self
            .supervisor
            .on_exit(name, generation, outcome, &*self.ports)
            .await;
        self.run(effects);
    }

    pub async fn on_restart(&mut self, name: &str) {
        let effects = self.supervisor.on_restart(name, &*self.ports).await;
        self.run(effects);
    }

    pub async fn on_fire(&mut self, name: &str, fire_at_ms: u64) {
        let effects = self
            .supervisor
            .on_fire(name, fire_at_ms, &*self.ports)
            .await;
        self.run(effects);
    }

    pub async fn on_memory_sample(&mut self) {
        let effects = self
            .supervisor
            .on_memory_sample(self.memory_poll_interval_ms, &*self.ports)
            .await;
        self.run(effects);
    }

    pub async fn on_log_rotate(&mut self) {
        let effects = self
            .supervisor
            .on_log_rotate(
                self.log_rotate_max_bytes,
                self.log_rotate_interval_ms,
                &*self.ports,
            )
            .await;
        self.run(effects);
    }

    pub async fn on_ready(&mut self, name: &str, generation: u64) {
        let effects = self
            .supervisor
            .on_ready(name, generation, &*self.ports)
            .await;
        self.run(effects);
    }

    pub async fn on_ready_timeout(&mut self, name: &str, generation: u64, reason: &str) {
        let effects = self
            .supervisor
            .on_ready_timeout(name, generation, reason, &*self.ports)
            .await;
        self.run(effects);
    }

    pub async fn on_force_kill(&self, name: &str, generation: u64, pid: u32, token: Option<&str>) {
        self.supervisor
            .on_force_kill(name, generation, pid, token, &*self.ports)
            .await;
    }

    pub async fn shutdown(&mut self) {
        let effects = self.supervisor.prepare_shutdown(&*self.ports).await;
        self.run(effects);
        if !self.wait_until_drained().await {
            self.force_kill_survivors().await;
        }
        let survivors = self.supervisor.unsettled();
        tracing::info!(
            feature = "lifecycle",
            action = "shutdown",
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
                self.board.forget_force_kill(&name);
            }
            DaemonEvent::SampleMemory => self.on_memory_sample().await,
            DaemonEvent::RotateLogs => self.on_log_rotate().await,
            DaemonEvent::Ready { name, generation } => self.on_ready(&name, generation).await,
            DaemonEvent::ReadyTimeout {
                name,
                generation,
                reason,
            } => {
                self.on_ready_timeout(&name, generation, &reason).await;
            }
            DaemonEvent::Shutdown => self.shutdown().await,
        }
    }

    fn run(&mut self, effects: Vec<SupervisionEffect>) {
        for effect in effects {
            self.board.apply(effect);
        }
    }

    async fn wait_until_drained(&self) -> bool {
        let mut waited_ms = 0;
        while waited_ms < self.kill_timeout_ms {
            if self.drained().await {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(self.poll_interval_ms)).await;
            waited_ms = waited_ms.saturating_add(self.poll_interval_ms);
        }
        self.drained().await
    }

    async fn drained(&self) -> bool {
        self.supervisor.drained(&self.ports.tracked_pids().await)
    }

    async fn force_kill_survivors(&self) {
        let tracked = self.ports.tracked_pids().await;
        self.supervisor
            .force_kill_survivors(&tracked, &*self.ports)
            .await;
    }
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
