use entities::AppSpec;

use crate::{
    Ports, UsecaseError,
    delete::{delete_app, delete_one},
    ports::SpecResolver,
    query::{identity_token_of, owner_of_pid, unswept_pids},
    reset::reset_app,
    restart::restart_app,
    selector::AppSelector,
    signal::signal_app,
    start::{StartKind, StartReport, refused_services, start_apps},
    stop::{stop_all_apps, stop_app},
    supervision::{SupervisionEffect, SupervisionFailure, SupervisionOutcome, SupervisionReply},
    supervisor::Supervisor,
    supervisor_log::{log_batch_skip, log_failure, log_partial_start},
    table::dependency_order,
};

#[expect(
    clippy::multiple_inherent_impl,
    reason = "the request handlers live in their own file to keep supervisor.rs under the size cap"
)]
impl Supervisor {
    pub(crate) async fn start(
        &mut self,
        services: &[String],
        resolver: &impl SpecResolver,
        ports: &impl Ports,
        effects: &mut Vec<SupervisionEffect>,
    ) -> SupervisionOutcome {
        let mut specs: Vec<AppSpec> = Vec::with_capacity(services.len());
        for name in services {
            specs.push(resolver.prepare(name).await?);
        }
        let StartReport {
            outcomes,
            failure,
            unsaved,
            pending,
        } = start_apps(&mut self.table, &specs, &self.logs_dir, ports).await;
        self.watch_all(&outcomes, effects);
        for deferred in pending {
            for dependency in &deferred.waiting_on {
                self.waiters
                    .entry(dependency.clone())
                    .or_default()
                    .push(deferred.name.clone());
            }
        }
        for outcome in &outcomes {
            self.cancel_restart(&outcome.name, effects);
            if outcome.kind != StartKind::Deferred {
                self.arm_timer(&outcome.name, ports, effects);
            }
        }
        if outcomes.is_empty() {
            if let Some(error) = failure.or(unsaved) {
                return Err(error.into());
            }
            return Ok(SupervisionReply::Started {
                outcomes,
                refused: Vec::new(),
                reason: None,
                unsaved: None,
            });
        }
        let refused = refused_services(services, &outcomes);
        let reason = failure
            .inspect(|error| log_partial_start(&refused, error))
            .as_ref()
            .map(ToString::to_string);
        Ok(SupervisionReply::Started {
            outcomes,
            refused,
            reason,
            unsaved: unsaved.as_ref().map(ToString::to_string),
        })
    }

    pub(crate) async fn stop(
        &mut self,
        selector: &AppSelector,
        ports: &impl Ports,
        effects: &mut Vec<SupervisionEffect>,
    ) -> SupervisionOutcome {
        if matches!(selector, AppSelector::All) {
            return self.stop_all(ports, effects).await;
        }
        let attempted = stop_app(&mut self.table, selector, ports).await;
        let outcome = match attempted {
            Ok(outcome) => outcome,
            Err(error) => {
                self.cover_draining(selector, effects);
                return Err(error.into());
            }
        };
        let token = self.identity_token(&outcome.name);
        self.retire(&outcome.name, outcome.force_kill_pid, token, effects);
        Ok(SupervisionReply::Stopped { name: outcome.name })
    }

    pub(crate) async fn restart(
        &mut self,
        selector: &AppSelector,
        resolver: &impl SpecResolver,
        ports: &impl Ports,
        effects: &mut Vec<SupervisionEffect>,
    ) -> SupervisionOutcome {
        if matches!(selector, AppSelector::All) {
            return self.restart_all(resolver, ports, effects).await;
        }
        self.reload_declaration(selector, resolver).await?;
        let outcome = restart_app(&mut self.table, selector, &self.logs_dir, ports).await?;
        let name = self.dispatch_restart(outcome, effects);
        self.cancel_restart(&name, effects);
        self.arm_timer(&name, ports, effects);
        Ok(SupervisionReply::Restarted { name })
    }

    async fn restart_all(
        &mut self,
        resolver: &impl SpecResolver,
        ports: &impl Ports,
        effects: &mut Vec<SupervisionEffect>,
    ) -> SupervisionOutcome {
        let mut names = Vec::new();
        for name in self.table.names_in_table_order() {
            match resolver.prepare(&name).await {
                Ok(spec) => {
                    self.reload_spec(&name, spec);
                }
                Err(error) => {
                    log_batch_skip("restart", &name, &error.to_string());
                    continue;
                }
            }
            let attempt = restart_app(
                &mut self.table,
                &AppSelector::Name(name.clone()),
                &self.logs_dir,
                ports,
            )
            .await;
            match attempt {
                Ok(outcome) => {
                    let restarted = self.dispatch_restart(outcome, effects);
                    self.cancel_restart(&restarted, effects);
                    self.arm_timer(&restarted, ports, effects);
                    names.push(restarted);
                }
                Err(error) => {
                    log_failure("restart", &name, &error);
                    self.arm_timer(&name, ports, effects);
                }
            }
        }
        Ok(SupervisionReply::RestartedAll { names })
    }

    async fn reload_declaration(
        &mut self,
        selector: &AppSelector,
        resolver: &impl SpecResolver,
    ) -> Result<(), SupervisionFailure> {
        let Some(record) = self.table.find_mut(selector) else {
            return Ok(());
        };
        let name = record.runtime.name.clone();
        record.spec = resolver.prepare(&name).await?;
        Ok(())
    }

    fn reload_spec(&mut self, name: &str, spec: AppSpec) {
        let record = self
            .table
            .find_by_name_mut(name)
            .expect("internal error: the batch only names records the table holds");
        record.spec = spec;
    }

    pub(crate) async fn delete(
        &mut self,
        selector: &AppSelector,
        ports: &impl Ports,
        effects: &mut Vec<SupervisionEffect>,
    ) -> SupervisionOutcome {
        if matches!(selector, AppSelector::All) {
            return self.delete_all(ports, effects).await;
        }
        let token = identity_token_of(&self.table, selector);
        let attempted = delete_app(&mut self.table, selector, ports).await;
        let outcome = match attempted {
            Ok(outcome) => outcome,
            Err(error) => {
                self.cover_draining(selector, effects);
                return Err(error.into());
            }
        };
        self.retire(&outcome.name, outcome.force_kill_pid, token, effects);
        Ok(SupervisionReply::Deleted { name: outcome.name })
    }

    async fn delete_all(
        &mut self,
        ports: &impl Ports,
        effects: &mut Vec<SupervisionEffect>,
    ) -> SupervisionOutcome {
        let order = dependency_order(&self.table, log_unordered_delete);
        let mut names = Vec::with_capacity(order.len());
        for name in order.iter().rev() {
            let token = self.identity_token(name);
            match delete_one(&mut self.table, name, ports).await {
                Ok(outcome) => {
                    self.retire(&outcome.name, outcome.force_kill_pid, token, effects);
                    names.push(outcome.name);
                }
                Err(error) => {
                    self.cover_draining(&AppSelector::Name(name.clone()), effects);
                    log_failure("delete", name, &error);
                }
            }
        }
        Ok(SupervisionReply::DeletedAll { names })
    }

    pub(crate) async fn reset(
        &mut self,
        selector: &AppSelector,
        ports: &impl Ports,
    ) -> SupervisionOutcome {
        if matches!(selector, AppSelector::All) {
            return self.reset_all(ports).await;
        }
        let name = reset_app(&mut self.table, selector, ports).await?;
        Ok(SupervisionReply::Reset { name })
    }

    async fn reset_all(&mut self, ports: &impl Ports) -> SupervisionOutcome {
        let mut names = Vec::new();
        for name in self.table.names_in_table_order() {
            match reset_app(&mut self.table, &AppSelector::Name(name.clone()), ports).await {
                Ok(reset) => names.push(reset),
                Err(error) => log_failure("reset", &name, &error),
            }
        }
        Ok(SupervisionReply::ResetAll { names })
    }

    pub(crate) async fn signal(
        &mut self,
        selector: &AppSelector,
        signal: &str,
        ports: &impl Ports,
    ) -> SupervisionOutcome {
        let outcome = signal_app(&mut self.table, selector, signal, ports).await?;
        Ok(SupervisionReply::Signalled {
            name: outcome.name,
            signal: outcome.signal,
        })
    }

    pub(crate) async fn stop_all(
        &mut self,
        ports: &impl Ports,
        effects: &mut Vec<SupervisionEffect>,
    ) -> SupervisionOutcome {
        effects.extend(self.disarm_everything());
        let stopped = stop_all_apps(&mut self.table, ports).await;
        let mut names = Vec::with_capacity(stopped.len());
        let mut covered = Vec::with_capacity(stopped.len());
        for outcome in &stopped {
            names.push(outcome.name.clone());
            covered.extend(outcome.force_kill_pid);
            self.cancel_ready(&outcome.name, effects);
            let token = self.identity_token(&outcome.name);
            self.schedule_force_kill(&outcome.name, outcome.force_kill_pid, token, effects);
        }
        let tracked = ports.tracked_pids().await;
        for pid in unswept_pids(&tracked, &covered) {
            let (name, token) = owner_of_pid(&self.table, pid);
            self.schedule_force_kill(&name, Some(pid), token, effects);
        }
        Ok(SupervisionReply::StoppedAll { names })
    }

    fn cover_draining(&mut self, selector: &AppSelector, effects: &mut Vec<SupervisionEffect>) {
        let Some(record) = self.table.find(selector) else {
            return;
        };
        if !record.runtime.status.is_shutting_down() {
            return;
        }
        let pid = record
            .runtime
            .pid
            .expect("internal error: a draining record always holds its pid");
        let name = record.runtime.name.clone();
        let token = identity_token_of(&self.table, selector);
        self.retire(&name, Some(pid), token, effects);
    }

    fn retire(
        &mut self,
        name: &str,
        force_kill_pid: Option<u32>,
        token: Option<String>,
        effects: &mut Vec<SupervisionEffect>,
    ) {
        self.disarm(name, effects);
        self.cancel_restart(name, effects);
        self.cancel_ready(name, effects);
        self.schedule_force_kill(name, force_kill_pid, token, effects);
    }
}

fn log_unordered_delete(error: &UsecaseError) {
    let reason = error.to_string();
    tracing::warn!(
        feature = "lifecycle",
        action = "delete_all",
        reason,
        "pm3 cannot order the deletion, so it deletes every service in table order",
    );
}

#[cfg(test)]
#[path = "tests/supervisor_batch_tests.rs"]
mod batch_tests;
