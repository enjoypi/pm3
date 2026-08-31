use std::{collections::HashMap, sync::Arc, time::Duration};

use adapters::{ExitOutcome, Readiness, ReadyProbe, ReadyProber as _, SupervisionEffect};
use tokio::{sync::mpsc, task::JoinHandle};

use super::{events::DaemonEvent, ports::DaemonPorts};

#[derive(Debug)]
pub struct TaskBoard {
    events: mpsc::Sender<DaemonEvent>,
    ports: Arc<DaemonPorts>,
    fires: HashMap<String, JoinHandle<()>>,
    restarts: HashMap<String, JoinHandle<()>>,
    force_kills: HashMap<String, JoinHandle<()>>,
    ready: HashMap<String, JoinHandle<()>>,
    memory_sample: Option<JoinHandle<()>>,
    log_rotate: Option<JoinHandle<()>>,
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
            ready: HashMap::new(),
            memory_sample: None,
            log_rotate: None,
        }
    }

    pub fn apply(&mut self, effect: SupervisionEffect) {
        use SupervisionEffect as Se;

        match effect {
            Se::ScheduleMemorySample { delay_ms } => self.schedule_memory_sample(delay_ms),
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
            Se::ScheduleLogRotate { delay_ms } => self.schedule_log_rotate(delay_ms),
            Se::AwaitReady {
                name,
                generation,
                probe,
                timeout_ms,
                interval_ms,
            } => self.await_ready(name, generation, probe, timeout_ms, interval_ms),
            Se::CancelReady { name } => abort(self.ready.remove(&name)),
        }
    }

    #[cfg(test)]
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

    fn schedule_memory_sample(&mut self, delay_ms: u64) {
        rearm_once(
            &mut self.memory_sample,
            delay_ms,
            &self.events,
            DaemonEvent::SampleMemory,
        );
    }

    fn schedule_log_rotate(&mut self, delay_ms: u64) {
        rearm_once(
            &mut self.log_rotate,
            delay_ms,
            &self.events,
            DaemonEvent::RotateLogs,
        );
    }

    fn await_ready(
        &mut self,
        name: String,
        generation: u64,
        probe: ReadyProbe,
        timeout_ms: u64,
        interval_ms: u64,
    ) {
        abort(self.ready.remove(&name));
        let ports = Arc::clone(&self.ports);
        let events = self.events.clone();
        let probing = name.clone();
        let task = tokio::spawn(async move {
            let budget = Duration::from_millis(timeout_ms);
            let step = Duration::from_millis(interval_ms.max(1));
            let started = tokio::time::Instant::now();
            loop {
                match ports.check_ready(&probe).await {
                    Readiness::Ready => {
                        events
                            .send(DaemonEvent::Ready {
                                name: probing,
                                generation,
                            })
                            .await
                            .ok();
                        return;
                    }
                    Readiness::Failed(reason) => {
                        events
                            .send(DaemonEvent::ReadyTimeout {
                                name: probing,
                                generation,
                                reason,
                            })
                            .await
                            .ok();
                        return;
                    }
                    Readiness::Pending => {}
                }
                let remaining = budget.saturating_sub(started.elapsed());
                if remaining.is_zero() {
                    events
                        .send(DaemonEvent::ReadyTimeout {
                            name: probing,
                            generation,
                            reason: format!("not ready within {timeout_ms}ms"),
                        })
                        .await
                        .ok();
                    return;
                }
                tokio::time::sleep(remaining.min(step)).await;
            }
        });
        self.ready.insert(name, task);
    }

    fn arm(&mut self, name: String, fire_at_ms: u64, delay_ms: u64) {
        let fired = DaemonEvent::Fire {
            name: name.clone(),
            fire_at_ms,
        };
        rearm(&mut self.fires, name, delay_ms, &self.events, fired);
    }

    fn schedule_restart(&mut self, name: String, delay_ms: u64) {
        let restarted = DaemonEvent::Restart { name: name.clone() };
        rearm(&mut self.restarts, name, delay_ms, &self.events, restarted);
    }

    fn schedule_force_kill(
        &mut self,
        name: String,
        generation: u64,
        pid: u32,
        token: Option<String>,
        delay_ms: u64,
    ) {
        let doomed = DaemonEvent::ForceKill {
            name: name.clone(),
            generation,
            pid,
            token,
        };
        rearm(&mut self.force_kills, name, delay_ms, &self.events, doomed);
    }
}

fn rearm(
    slot: &mut HashMap<String, JoinHandle<()>>,
    name: String,
    delay_ms: u64,
    events: &mpsc::Sender<DaemonEvent>,
    event: DaemonEvent,
) {
    abort(slot.remove(&name));
    slot.insert(name, spawn_delayed(events, delay_ms, event));
}

fn rearm_once(
    slot: &mut Option<JoinHandle<()>>,
    delay_ms: u64,
    events: &mpsc::Sender<DaemonEvent>,
    event: DaemonEvent,
) {
    abort(slot.take());
    *slot = Some(spawn_delayed(events, delay_ms, event));
}

fn spawn_delayed(
    events: &mpsc::Sender<DaemonEvent>,
    delay_ms: u64,
    event: DaemonEvent,
) -> JoinHandle<()> {
    let sender = events.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        sender.send(event).await.ok();
    })
}

fn abort(task: Option<JoinHandle<()>>) {
    if let Some(task) = task {
        task.abort();
    }
}
