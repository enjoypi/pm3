use std::{collections::HashMap, sync::Arc, time::Duration};

use adapters::{ExitOutcome, SupervisionEffect};
use tokio::{sync::mpsc, task::JoinHandle};

use super::{events::DaemonEvent, ports::DaemonPorts};

#[derive(Debug)]
pub struct TaskBoard {
    events: mpsc::Sender<DaemonEvent>,
    ports: Arc<DaemonPorts>,
    fires: HashMap<String, JoinHandle<()>>,
    restarts: HashMap<String, JoinHandle<()>>,
    force_kills: HashMap<String, JoinHandle<()>>,
}

impl TaskBoard {
    #[must_use]
    pub fn new(events: mpsc::Sender<DaemonEvent>, ports: Arc<DaemonPorts>) -> Self {
        Self {
            events,
            ports,
            fires: HashMap::new(),
            restarts: HashMap::new(),
            force_kills: HashMap::new(),
        }
    }

    pub fn apply(&mut self, effect: SupervisionEffect) {
        use SupervisionEffect as Se;

        match effect {
            Se::ArmTimer {
                name,
                fire_at_ms,
                delay_ms,
            } => self.arm(name, fire_at_ms, delay_ms),
            Se::DisarmTimer { name } => abort(self.fires.remove(&name)),
            Se::ScheduleRestart { name, delay_ms } => self.schedule_restart(name, delay_ms),
            Se::CancelRestart { name } => abort(self.restarts.remove(&name)),
            Se::ScheduleForceKill {
                name,
                generation,
                pid,
                token,
                delay_ms,
            } => self.schedule_force_kill(name, generation, pid, token, delay_ms),
            Se::CancelForceKill { name } => abort(self.force_kills.remove(&name)),
            Se::WatchExit {
                name,
                generation,
                pid,
                token,
            } => self.watch(name, generation, pid, token),
        }
    }

    #[must_use]
    pub fn has_force_kill(&self, name: &str) -> bool {
        self.force_kills.contains_key(name)
    }

    pub fn forget_force_kill(&mut self, name: &str) {
        self.force_kills.remove(name);
    }

    fn watch(&self, name: String, generation: u64, pid: u32, token: Option<String>) {
        let ports = Arc::clone(&self.ports);
        let events = self.events.clone();
        tokio::spawn(async move {
            let outcome = ports
                .wait(pid, token)
                .await
                .unwrap_or(ExitOutcome::Unobserved);
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

    fn arm(&mut self, name: String, fire_at_ms: u64, delay_ms: u64) {
        abort(self.fires.remove(&name));
        let events = self.events.clone();
        let fired = name.clone();
        let task = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            events
                .send(DaemonEvent::Fire {
                    name: fired,
                    fire_at_ms,
                })
                .await
                .ok();
        });
        self.fires.insert(name, task);
    }

    fn schedule_restart(&mut self, name: String, delay_ms: u64) {
        abort(self.restarts.remove(&name));
        let events = self.events.clone();
        let restarted = name.clone();
        let task = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            events
                .send(DaemonEvent::Restart { name: restarted })
                .await
                .ok();
        });
        self.restarts.insert(name, task);
    }

    fn schedule_force_kill(
        &mut self,
        name: String,
        generation: u64,
        pid: u32,
        token: Option<String>,
        delay_ms: u64,
    ) {
        abort(self.force_kills.remove(&name));
        let events = self.events.clone();
        let doomed = name.clone();
        let task = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            events
                .send(DaemonEvent::ForceKill {
                    name: doomed,
                    generation,
                    pid,
                    token,
                })
                .await
                .ok();
        });
        self.force_kills.insert(name, task);
    }
}

fn abort(task: Option<JoinHandle<()>>) {
    if let Some(task) = task {
        task.abort();
    }
}
