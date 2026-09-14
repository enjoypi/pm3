use crate::{
    Ports,
    query::{liveness_watch_list, record_liveness},
    supervision::SupervisionEffect,
    supervisor::Supervisor,
    supervisor_log::log_liveness_failure,
};

#[expect(
    clippy::multiple_inherent_impl,
    reason = "the liveness orchestration lives in its own file to keep supervisor.rs under the size cap"
)]
impl Supervisor {
    pub async fn on_liveness_sample(
        &mut self,
        interval_ms: u64,
        threshold: u32,
        ports: &impl Ports,
    ) -> Vec<SupervisionEffect> {
        let mut effects = vec![SupervisionEffect::ScheduleLivenessSample {
            delay_ms: interval_ms,
        }];
        for watch in liveness_watch_list(&self.table) {
            let verdict = ports.check_ready(&watch.probe).await;
            let tripped = record_liveness(
                self.table.find_by_name_mut(&watch.name),
                &verdict,
                threshold,
            );
            if tripped {
                log_liveness_failure(&watch.name, threshold);
                self.restart_now(&watch.name, ports, &mut effects).await;
                self.hand_liveness_to_the_breaker(&watch.name);
            }
        }
        effects
    }

    fn hand_liveness_to_the_breaker(&mut self, name: &str) {
        crate::query::hand_to_the_breaker(self.table.find_by_name_mut(name));
    }
}

#[cfg(test)]
#[path = "tests/supervisor_liveness_tests.rs"]
mod tests;
