use std::{collections::HashMap, sync::Arc, time::Duration};

use adapters::{
    AppSelector, Clock as _, DaemonCommand, DaemonOutcome, DaemonReply, DaemonRequest, ExitAction,
    ExitOutcome, ProcessStatus, ProcessTable, RestartOutcome, SpecSource, StartOutcome,
    UsecaseError, delete_app, describe_app, handle_child_exit, list_apps, load_apps_file,
    materialise_workspace, resolve_specs, restart_app, resurrect, start_apps, stop_app, topo_sort,
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
    next_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
            next_generation: 0,
        }
    }

    pub async fn resurrect_saved_apps(&mut self) {
        match resurrect(&mut self.table, &self.specs.logs_dir, &*self.ports).await {
            Ok(outcomes) => self.watch_all(&outcomes),
            Err(error) => log_failure("resurrect", "-", &error),
        }
    }

    pub async fn handle(&mut self, request: DaemonRequest) -> DaemonOutcome {
        match request {
            DaemonRequest::Start { apps_file } => self.start(&apps_file).await,
            DaemonRequest::List => Ok(DaemonReply::Listed(list_apps(
                &self.table,
                self.ports.now_ms(),
            ))),
            DaemonRequest::Describe(selector) => self.describe(&selector),
            DaemonRequest::Stop(selector) => self.stop(&selector).await,
            DaemonRequest::Restart(selector) => self.restart(&selector).await,
            DaemonRequest::Delete(selector) => self.delete(&selector).await,
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
            Ok(RestartOutcome::Started(started)) => self.watch(&started),
            Ok(RestartOutcome::AwaitingExit {
                pm_id: _,
                name: app,
                force_kill_pid,
            }) => self.schedule_force_kill(&app, force_kill_pid),
            Err(error) => log_failure("restart", name, &error),
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

    pub async fn shutdown(&mut self) {
        let order = topo_sort(&self.table.dependency_nodes()).unwrap_or_default();
        for name in order.iter().rev() {
            let selector = AppSelector::Name(name.clone());
            stop_app(&mut self.table, &selector, &*self.ports)
                .await
                .ok();
        }
        tokio::time::sleep(Duration::from_millis(self.specs.config.kill_timeout_ms)).await;
        for pid in self.ports.tracked_pids().await {
            self.ports.force_kill(pid).await.ok();
        }
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
            DaemonEvent::ForceKill {
                name,
                generation,
                pid,
            } => {
                self.on_force_kill(&name, generation, pid).await;
                Flow::Continue
            }
            DaemonEvent::Shutdown => {
                self.shutdown().await;
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
        Ok(DaemonReply::Started(outcomes))
    }

    fn describe(&self, selector: &AppSelector) -> DaemonOutcome {
        let view = describe_app(&self.table, selector, self.ports.now_ms())?;
        Ok(DaemonReply::Described(view))
    }

    async fn stop(&mut self, selector: &AppSelector) -> DaemonOutcome {
        let outcome = stop_app(&mut self.table, selector, &*self.ports).await?;
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
        let name = match outcome {
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
        };
        Ok(DaemonReply::Restarted { name })
    }

    async fn delete(&mut self, selector: &AppSelector) -> DaemonOutcome {
        let outcome = delete_app(&mut self.table, selector, &*self.ports).await?;
        self.schedule_force_kill(&outcome.name, outcome.force_kill_pid);
        Ok(DaemonReply::Deleted { name: outcome.name })
    }

    fn watch_all(&mut self, outcomes: &[StartOutcome]) {
        for outcome in outcomes {
            self.watch(outcome);
        }
    }

    fn watch(&mut self, started: &StartOutcome) {
        let (Some(pid), false) = (started.pid, started.already_running) else {
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
