use std::{collections::HashMap, time::Duration};

use tokio::{sync::mpsc, task::JoinHandle};

use super::actor::DaemonEvent;

#[derive(Debug)]
struct Timer {
    fire_at_ms: u64,
    task: JoinHandle<()>,
}

#[derive(Debug)]
pub struct TimerBoard {
    events: mpsc::Sender<DaemonEvent>,
    timers: HashMap<String, Timer>,
    restarts: HashMap<String, JoinHandle<()>>,
    generations: HashMap<String, u64>,
    next_generation: u64,
}

impl TimerBoard {
    #[must_use]
    pub fn new(events: mpsc::Sender<DaemonEvent>) -> Self {
        Self {
            events,
            timers: HashMap::new(),
            restarts: HashMap::new(),
            generations: HashMap::new(),
            next_generation: 0,
        }
    }

    #[must_use]
    pub fn next_fire_of(&self, name: &str) -> Option<u64> {
        self.timers.get(name).map(|timer| timer.fire_at_ms)
    }

    pub fn arm(&mut self, name: &str, fire_at_ms: u64, delay: Duration) {
        self.disarm(name);
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
        self.timers
            .insert(name.to_string(), Timer { fire_at_ms, task });
    }

    pub fn disarm(&mut self, name: &str) {
        if let Some(timer) = self.timers.remove(name) {
            timer.task.abort();
        }
    }

    pub fn disarm_all(&mut self) {
        for (_name, timer) in self.timers.drain() {
            timer.task.abort();
        }
    }

    pub fn schedule_restart(&mut self, name: &str, delay_ms: u64) {
        self.cancel_restart(name);
        let events = self.events.clone();
        let restarted = name.to_string();
        let task = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            events
                .send(DaemonEvent::Restart { name: restarted })
                .await
                .ok();
        });
        self.restarts.insert(name.to_string(), task);
    }

    pub fn claim_restart(&mut self, name: &str) -> bool {
        self.restarts.remove(name).is_some()
    }

    pub fn cancel_restart(&mut self, name: &str) {
        if let Some(task) = self.restarts.remove(name) {
            task.abort();
        }
    }

    pub fn cancel_all_restarts(&mut self) {
        for (_name, task) in self.restarts.drain() {
            task.abort();
        }
    }

    pub fn schedule_force_kill(&self, name: &str, pid: Option<u32>, delay: Duration) {
        let Some(pid) = pid else {
            return;
        };
        let events = self.events.clone();
        let name = name.to_string();
        let generation = self.current_generation(&name);
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

    pub fn bump(&mut self, name: &str) -> u64 {
        self.next_generation = self.next_generation.saturating_add(1);
        self.generations
            .insert(name.to_string(), self.next_generation);
        self.next_generation
    }

    pub fn forget_generation(&mut self, name: &str) {
        self.generations.remove(name);
    }

    #[must_use]
    pub fn is_current(&self, name: &str, generation: u64) -> bool {
        self.current_generation(name) == generation
    }

    fn current_generation(&self, name: &str) -> u64 {
        self.generations.get(name).copied().unwrap_or_default()
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
