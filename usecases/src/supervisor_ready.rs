use entities::ProcessStatus;

use crate::{
    Ports, SignalScope,
    persist::save_table,
    start::{StartKind, register_one},
    supervision::SupervisionEffect,
    supervisor::Supervisor,
    supervisor_log::{log_failure, log_probe_ready, log_ready_timeout, log_waiter_cancelled},
};

#[expect(
    clippy::multiple_inherent_impl,
    reason = "the readiness orchestration lives in its own file to keep supervisor.rs under the size cap"
)]
impl Supervisor {
    pub async fn on_ready(
        &mut self,
        name: &str,
        generation: u64,
        ports: &impl Ports,
    ) -> Vec<SupervisionEffect> {
        let mut effects = Vec::new();
        if !self.timers.is_current(name, generation) {
            return effects;
        }
        if !self.is_launching(name) {
            return effects;
        }
        self.table
            .find_by_name_mut(name)
            .expect("internal error: the readiness guard checked the record")
            .runtime
            .mark_online();
        log_probe_ready(name);
        if let Err(error) = save_table(&self.table, ports).await {
            log_failure("ready", name, &error);
        }
        self.launch_waiters(name, ports, &mut effects).await;
        effects
    }

    pub async fn on_ready_timeout(
        &mut self,
        name: &str,
        generation: u64,
        reason: &str,
        ports: &impl Ports,
    ) -> Vec<SupervisionEffect> {
        let mut effects = Vec::new();
        if !self.timers.is_current(name, generation) {
            return effects;
        }
        if !self.is_launching(name) {
            return effects;
        }
        log_ready_timeout(name, reason);
        self.ready_failed.insert(name.to_string());
        let (pid, token) = {
            let record = self
                .table
                .find_by_name_mut(name)
                .expect("internal error: the readiness guard checked the record");
            record.runtime.mark_stopping();
            let pid = record
                .runtime
                .pid
                .expect("internal error: a launching service always has a pid");
            let token = record
                .runtime
                .identity
                .as_ref()
                .map(|identity| identity.token.clone());
            (pid, token)
        };
        if let Err(error) = ports.terminate(pid, SignalScope::ProcessGroup).await {
            log_failure("ready_timeout", name, &crate::UsecaseError::from(error));
        }
        self.schedule_force_kill(name, Some(pid), token, &mut effects);
        self.fail_downstream(name);
        if let Err(error) = save_table(&self.table, ports).await {
            log_failure("ready_timeout", name, &error);
        }
        effects
    }

    pub(crate) fn cancel_ready(&mut self, name: &str, effects: &mut Vec<SupervisionEffect>) {
        self.ready_failed.remove(name);
        self.forget_waiter(name);
        self.fail_downstream(name);
        effects.push(SupervisionEffect::CancelReady {
            name: name.to_string(),
        });
    }

    fn is_launching(&self, name: &str) -> bool {
        self.table
            .find_by_name(name)
            .is_some_and(|record| record.runtime.status == ProcessStatus::Launching)
    }

    async fn launch_waiters(
        &mut self,
        name: &str,
        ports: &impl Ports,
        effects: &mut Vec<SupervisionEffect>,
    ) {
        let mut queue = vec![name.to_string()];
        while let Some(ready) = queue.pop() {
            let Some(waiters) = self.waiters.remove(&ready) else {
                continue;
            };
            for waiter in waiters {
                if self.still_waiting(&waiter) {
                    continue;
                }
                self.launch_waiter(&waiter, &mut queue, ports, effects)
                    .await;
            }
        }
    }

    fn still_waiting(&self, name: &str) -> bool {
        self.waiters
            .values()
            .any(|list| list.iter().any(|waiter| waiter == name))
    }

    async fn launch_waiter(
        &mut self,
        waiter: &str,
        queue: &mut Vec<String>,
        ports: &impl Ports,
        effects: &mut Vec<SupervisionEffect>,
    ) {
        let Ok(outcome) = register_one(&mut self.table, waiter, &self.logs_dir, ports).await else {
            self.mark_errored_if_settled(waiter);
            self.fail_downstream(waiter);
            if let Err(error) = save_table(&self.table, ports).await {
                log_failure("start", waiter, &error);
            }
            return;
        };
        self.watch(&outcome, effects);
        self.arm_timer(waiter, ports, effects);
        if let Err(error) = save_table(&self.table, ports).await {
            log_failure("start", waiter, &error);
        }
        if self.releases_waiters(waiter, outcome.kind) {
            queue.push(waiter.to_string());
        }
    }

    fn releases_waiters(&self, name: &str, kind: StartKind) -> bool {
        if kind == StartKind::Scheduled {
            return true;
        }
        self.table
            .find_by_name(name)
            .is_some_and(|record| record.runtime.status == ProcessStatus::Online)
    }

    fn fail_downstream(&mut self, name: &str) {
        let mut dead = self.waiters.remove(name).unwrap_or_default();
        while let Some(doomed) = dead.pop() {
            self.forget_waiter(&doomed);
            self.mark_errored_if_settled(&doomed);
            if let Some(downstream) = self.waiters.remove(&doomed) {
                dead.extend(downstream);
            }
        }
    }

    fn mark_errored_if_settled(&mut self, name: &str) {
        let record = self
            .table
            .find_by_name_mut(name)
            .expect("internal error: a waiting service always has a record");
        if !record.runtime.status.is_settled() {
            return;
        }
        record.runtime.mark_exited(ProcessStatus::Errored);
        log_waiter_cancelled(name);
    }

    fn forget_waiter(&mut self, name: &str) {
        self.waiters
            .values_mut()
            .for_each(|list| list.retain(|waiter| waiter != name));
        self.waiters.retain(|_dependency, list| !list.is_empty());
    }
}

#[cfg(test)]
#[path = "tests/supervisor_ready_tests.rs"]
mod tests;
