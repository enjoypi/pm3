use std::{collections::HashMap, time::Duration};

use adapters::TimerState;
use tokio::{sync::mpsc, task::JoinHandle};

use super::events::DaemonEvent;

#[derive(Debug)]
pub struct TimerBoard {
    events: mpsc::Sender<DaemonEvent>,
    state: TimerState,
    fire_tasks: HashMap<String, JoinHandle<()>>,
    restart_tasks: HashMap<String, JoinHandle<()>>,
    force_kill_tasks: HashMap<String, JoinHandle<()>>,
}

impl TimerBoard {
    #[must_use]
    pub fn new(events: mpsc::Sender<DaemonEvent>) -> Self {
        Self {
            events,
            state: TimerState::new(),
            fire_tasks: HashMap::new(),
            restart_tasks: HashMap::new(),
            force_kill_tasks: HashMap::new(),
        }
    }

    #[must_use]
    pub fn next_fire_of(&self, name: &str) -> Option<u64> {
        self.state.next_fire_of(name)
    }

    #[must_use]
    pub fn fire_is_due(&self, name: &str, fire_at_ms: u64) -> bool {
        self.state.fire_is_due(name, fire_at_ms)
    }

    pub fn arm(&mut self, name: &str, fire_at_ms: u64, delay: Duration) {
        self.disarm(name);
        self.state.arm(name, fire_at_ms);
        let events = self.events.clone();
        let fired = name.to_string();
        let task = tokio::spawn(async move {
            tokio::time::sleep(delay).await;
            events
                .send(DaemonEvent::Fire {
                    name: fired,
                    fire_at_ms,
                })
                .await
                .ok();
        });
        self.fire_tasks.insert(name.to_string(), task);
    }

    pub fn disarm(&mut self, name: &str) {
        self.state.disarm(name);
        abort(self.fire_tasks.remove(name));
    }

    pub fn disarm_all(&mut self) {
        for name in self.state.disarm_all() {
            abort(self.fire_tasks.remove(&name));
        }
    }

    pub fn schedule_restart(&mut self, name: &str, delay_ms: u64) {
        self.cancel_restart(name);
        self.state.queue_restart(name);
        let events = self.events.clone();
        let restarted = name.to_string();
        let task = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            events
                .send(DaemonEvent::Restart { name: restarted })
                .await
                .ok();
        });
        self.restart_tasks.insert(name.to_string(), task);
    }

    pub fn claim_restart(&mut self, name: &str) -> bool {
        self.restart_tasks.remove(name);
        self.state.claim_restart(name)
    }

    pub fn cancel_restart(&mut self, name: &str) {
        self.state.cancel_restart(name);
        abort(self.restart_tasks.remove(name));
    }

    pub fn cancel_all_restarts(&mut self) {
        for name in self.state.cancel_all_restarts() {
            abort(self.restart_tasks.remove(&name));
        }
    }

    pub fn schedule_force_kill(
        &mut self,
        name: &str,
        pid: Option<u32>,
        token: Option<String>,
        delay: Duration,
    ) {
        let Some(pid) = pid else {
            return;
        };
        self.cancel_force_kill(name);
        self.state.queue_force_kill(name);
        let events = self.events.clone();
        let doomed = name.to_string();
        let generation = self.state.current_generation(name);
        let task = tokio::spawn(async move {
            tokio::time::sleep(delay).await;
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
        self.force_kill_tasks.insert(name.to_string(), task);
    }

    pub fn cancel_force_kill(&mut self, name: &str) {
        self.state.cancel_force_kill(name);
        abort(self.force_kill_tasks.remove(name));
    }

    #[must_use]
    pub fn has_force_kill(&self, name: &str) -> bool {
        self.state.has_force_kill(name)
    }

    pub fn bump(&mut self, name: &str) -> u64 {
        self.state.bump(name)
    }

    pub fn forget_generation(&mut self, name: &str) {
        self.state.forget_generation(name);
    }

    #[must_use]
    pub fn is_current(&self, name: &str, generation: u64) -> bool {
        self.state.is_current(name, generation)
    }
}

fn abort(task: Option<JoinHandle<()>>) {
    if let Some(task) = task {
        task.abort();
    }
}

pub fn log_armed(app: &str, fire_at_ms: u64) {
    tracing::debug!(
        feature = "supervisor",
        action = "arm",
        app,
        fire_at_ms,
        "pm3 daemon armed the next cron fire",
    );
}

pub fn log_unschedulable(app: &str, cron: &str) {
    tracing::warn!(
        feature = "supervisor",
        action = "arm",
        app,
        cron,
        "pm3 daemon cannot work out a next fire for a schedule",
    );
}
