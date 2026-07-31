use std::{collections::HashMap, sync::Arc, time::Duration};

use adapters::{
    AppSelector, Clock as _, DaemonCommand, DaemonOutcome, DaemonReply, DaemonRequest, ExitAction,
    ExitOutcome, ProcessStatus, ProcessTable, ProcessView, RestartOutcome, Scheduler as _,
    SpecSource, StartOutcome, UsecaseError, delete_app, describe_app, handle_child_exit, list_apps,
    load_apps_file, materialise_workspace, resolve_specs, restart_app, resurrect, start_apps,
    stop_all_apps, stop_app,
};
use tokio::sync::mpsc;

use super::ports::DaemonPorts;

#[derive(Debug)]
pub enum DaemonEvent {
    Command(DaemonCommand),
    Exited {
        name: String,
        generation: u64,
        outcome: ExitOutcome,
    },
    Restart {
        name: String,
    },
    Fire {
        name: String,
        fire_at_ms: u64,
    },
    ForceKill {
        name: String,
        generation: u64,
        pid: u32,
    },
    Shutdown,
}

#[derive(Debug)]
pub struct Daemon {
    table: ProcessTable,
    ports: Arc<DaemonPorts>,
    specs: SpecSource,
    events: mpsc::Sender<DaemonEvent>,
    generations: HashMap<String, u64>,
    timers: HashMap<String, u64>,
    next_generation: u64,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum Flow {
    Continue,
    Stop,
}

pub async fn run(
    mut daemon: Daemon,
    commands: mpsc::Receiver<DaemonCommand>,
    mut events: mpsc::Receiver<DaemonEvent>,
) {
    tokio::spawn(forward_commands(commands, daemon.events.clone()));
    loop {
        let event = events
            .recv()
            .await
            .expect("internal error: the daemon holds an event sender, so the queue stays open");
        match daemon.apply(event).await {
            Flow::Continue => {}
            Flow::Stop => return,
        }
    }
}

async fn forward_commands(
    mut commands: mpsc::Receiver<DaemonCommand>,
    events: mpsc::Sender<DaemonEvent>,
) {
    while let Some(command) = commands.recv().await {
        events.send(DaemonEvent::Command(command)).await.ok();
    }
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
            events,
            generations: HashMap::new(),
            timers: HashMap::new(),
            next_generation: 0,
        }
    }

    pub async fn resurrect_saved_apps(&mut self) {
        match resurrect(&mut self.table, &self.specs.logs_dir, &*self.ports).await {
            Ok(outcomes) => self.watch_all(&outcomes),
            Err(error) => log_failure("resurrect", "-", &error),
        }
        self.arm_known_timers();
    }

    pub async fn handle(&mut self, request: DaemonRequest) -> DaemonOutcome {
        match request {
            DaemonRequest::Start { apps_file } => self.start(&apps_file).await,
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
        if !self.is_current(name, generation) {
            return;
        }
        match handle_child_exit(&mut self.table, name, outcome, &*self.ports).await {
            Ok(ExitAction::RestartAfter { delay_ms }) => self.schedule_restart(name, delay_ms),
            Ok(ExitAction::Settled { status }) => log_settled(name, status),
            Err(error) => log_failure("exit", name, &error),
        }
    }

    pub async fn on_restart(&mut self, name: &str) {
        let selector = AppSelector::Name(name.to_string());
        match restart_app(
            &mut self.table,
            &selector,
            &self.specs.logs_dir,
            &*self.ports,
        )
        .await
        {
            Ok(outcome) => {
                self.dispatch_restart(outcome);
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
                pm_id: _,
                name,
                force_kill_pid,
            } => {
                self.schedule_force_kill(&name, force_kill_pid);
                name
            }
        }
    }

    pub async fn on_force_kill(&self, name: &str, generation: u64, pid: u32) {
        if !self.is_current(name, generation) {
            return;
        }
        if !self.ports.tracked_pids().await.contains(&pid) {
            return;
        }
        self.ports.force_kill(pid).await.ok();
    }

    pub async fn tracked_pids(&self) -> Vec<u32> {
        self.ports.tracked_pids().await
    }

    pub fn shutdown(&self) {
        let survivors = self.running_names().len();
        tracing::info!(
            feature = "lifecycle",
            operation = "shutdown",
            mode = "preserve",
            survivors,
            "pm3 daemon is leaving its services running for the next daemon to reclaim",
        );
    }

    async fn stop_all(&mut self) -> DaemonOutcome {
        let stopped = stop_all_apps(&mut self.table, &*self.ports).await?;
        tokio::time::sleep(Duration::from_millis(self.specs.config.kill_timeout_ms)).await;
        for pid in self.ports.tracked_pids().await {
            self.ports.force_kill(pid).await.ok();
        }
        Ok(DaemonReply::StoppedAll { names: stopped })
    }

    fn running_names(&self) -> Vec<String> {
        self.table
            .records()
            .iter()
            .filter(|record| record.runtime.status.is_running())
            .map(|record| record.runtime.name.clone())
            .collect()
    }

    async fn apply(&mut self, event: DaemonEvent) -> Flow {
        match event {
            DaemonEvent::Command(command) => {
                let DaemonCommand { request, reply } = command;
                let outcome = self.handle(request).await;
                reply.send(outcome).ok();
                Flow::Continue
            }
            DaemonEvent::Exited {
                name,
                generation,
                outcome,
            } => {
                self.on_exit(&name, generation, outcome).await;
                Flow::Continue
            }
            DaemonEvent::Restart { name } => {
                self.on_restart(&name).await;
                Flow::Continue
            }
            DaemonEvent::Fire { name, fire_at_ms } => {
                self.on_fire(&name, fire_at_ms).await;
                Flow::Continue
            }
            DaemonEvent::ForceKill {
                name,
                generation,
                pid,
            } => {
                self.on_force_kill(&name, generation, pid).await;
                Flow::Continue
            }
            DaemonEvent::Shutdown => {
                self.shutdown();
                Flow::Stop
            }
        }
    }

    async fn start(&mut self, apps_file: &str) -> DaemonOutcome {
        let apps = load_apps_file(apps_file)?;
        let mut specs = resolve_specs(&self.specs.defaults()?, &apps)?;
        for spec in &mut specs {
            materialise_workspace(spec).await;
        }
        let outcomes =
            start_apps(&mut self.table, &specs, &self.specs.logs_dir, &*self.ports).await?;
        self.watch_all(&outcomes);
        for outcome in &outcomes {
            if outcome.kind.needs_timer() {
                self.arm_timer(&outcome.name);
            }
        }
        Ok(DaemonReply::Started(outcomes))
    }

    fn describe(&self, selector: &AppSelector) -> DaemonOutcome {
        let view = describe_app(&self.table, selector, self.ports.now_ms())?;
        Ok(DaemonReply::Described(self.with_next_fire(view)))
    }

    async fn stop(&mut self, selector: &AppSelector) -> DaemonOutcome {
        let outcome = stop_app(&mut self.table, selector, &*self.ports).await?;
        self.timers.remove(&outcome.name);
        self.schedule_force_kill(&outcome.name, outcome.force_kill_pid);
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
        self.arm_timer(&name);
        Ok(DaemonReply::Restarted { name })
    }

    async fn delete(&mut self, selector: &AppSelector) -> DaemonOutcome {
        let outcome = delete_app(&mut self.table, selector, &*self.ports).await?;
        self.timers.remove(&outcome.name);
        self.schedule_force_kill(&outcome.name, outcome.force_kill_pid);
        Ok(DaemonReply::Deleted { name: outcome.name })
    }

    fn with_next_fire(&self, view: ProcessView) -> ProcessView {
        let next_fire_ms = self.timers.get(&view.name).copied();
        ProcessView {
            next_fire_ms,
            ..view
        }
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
        let generation = self.bump(&started.name);
        let ports = Arc::clone(&self.ports);
        let events = self.events.clone();
        let name = started.name.clone();
        tokio::spawn(async move {
            let outcome = ports
                .wait(pid)
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

    pub async fn on_fire(&mut self, name: &str, fire_at_ms: u64) {
        if self.timers.get(name) != Some(&fire_at_ms) {
            return;
        }
        self.on_restart(name).await;
        self.arm_timer(name);
    }

    fn arm_known_timers(&mut self) {
        let scheduled = self.scheduled_names();
        for name in scheduled {
            self.arm_timer(&name);
        }
    }

    fn scheduled_names(&self) -> Vec<String> {
        self.table
            .records()
            .iter()
            .filter(|record| record.spec.schedule.is_some())
            .map(|record| record.runtime.name.clone())
            .collect()
    }

    fn arm_timer(&mut self, name: &str) {
        let Some(cron) = self.schedule_of(name) else {
            self.timers.remove(name);
            return;
        };
        let now_ms = self.ports.now_ms();
        let Some(fire_at_ms) = self.ports.next_fire_ms(&cron, now_ms) else {
            self.timers.remove(name);
            log_unschedulable(name, &cron);
            return;
        };
        self.timers.insert(name.to_string(), fire_at_ms);
        log_armed(name, fire_at_ms);

        let events = self.events.clone();
        let name = name.to_string();
        let delay = Duration::from_millis(fire_at_ms.saturating_sub(now_ms));
        tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            events
                .send(DaemonEvent::Fire { name, fire_at_ms })
                .await
                .ok();
        });
    }

    fn schedule_of(&self, name: &str) -> Option<String> {
        self.table
            .find(&AppSelector::Name(name.to_string()))
            .and_then(|record| record.spec.schedule.clone())
    }

    fn schedule_restart(&self, name: &str, delay_ms: u64) {
        let events = self.events.clone();
        let name = name.to_string();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            events.send(DaemonEvent::Restart { name }).await.ok();
        });
    }

    fn schedule_force_kill(&self, name: &str, pid: Option<u32>) {
        let Some(pid) = pid else {
            return;
        };
        let events = self.events.clone();
        let name = name.to_string();
        let generation = self.current_generation(&name);
        let delay = Duration::from_millis(self.specs.config.kill_timeout_ms);
        tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            events
                .send(DaemonEvent::ForceKill {
                    name,
                    generation,
                    pid,
                })
                .await
                .ok();
        });
    }

    fn bump(&mut self, name: &str) -> u64 {
        self.next_generation = self.next_generation.saturating_add(1);
        self.generations
            .insert(name.to_string(), self.next_generation);
        self.next_generation
    }

    fn current_generation(&self, name: &str) -> u64 {
        self.generations.get(name).copied().unwrap_or_default()
    }

    fn is_current(&self, name: &str, generation: u64) -> bool {
        self.current_generation(name) == generation
    }
}

fn log_settled(app: &str, status: ProcessStatus) {
    let status = status.as_str();
    tracing::debug!(
        feature = "supervisor",
        action = "settled",
        app,
        status,
        "managed app settled",
    );
}

fn log_armed(app: &str, fire_at_ms: u64) {
    tracing::debug!(
        feature = "supervisor",
        action = "arm",
        app,
        fire_at_ms,
        "pm3 daemon armed the next cron fire",
    );
}

fn log_unschedulable(app: &str, cron: &str) {
    tracing::warn!(
        feature = "supervisor",
        action = "arm",
        app,
        cron,
        "pm3 daemon cannot work out a next fire for a schedule",
    );
}

fn log_failure(action: &str, app: &str, error: &UsecaseError) {
    let reason = error.to_string();
    tracing::warn!(
        feature = "supervisor",
        action,
        app,
        reason,
        "pm3 daemon cannot finish a supervision step",
    );
}

#[cfg(test)]
#[path = "../test_helpers/daemon_actor_test_helpers.rs"]
mod test_helpers;
#[cfg(test)]
#[path = "../tests/daemon_actor_tests.rs"]
mod tests;
