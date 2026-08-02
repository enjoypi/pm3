use thiserror::Error;

use super::status::ProcessStatus;

#[derive(Debug, Eq, PartialEq, Error)]
pub enum RuntimeError {
    #[error("cannot accept process '{app}' marked '{status}' without a pid")]
    RunningWithoutPid { app: String, status: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessIdentity {
    pub token: String,
    pub launch_digest: String,
    pub binary_digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessRuntime {
    pub pm_id: u32,
    pub name: String,
    pub pid: Option<u32>,
    pub status: ProcessStatus,
    pub restart_time: u32,
    pub unstable_restarts: u32,
    pub created_at_ms: u64,
    pub started_at_ms: Option<u64>,
    pub identity: Option<ProcessIdentity>,
    pub pending_restart: bool,
    pub schedule_armed: bool,
}

impl ProcessRuntime {
    #[must_use]
    pub const fn new(pm_id: u32, name: String, created_at_ms: u64) -> Self {
        Self {
            pm_id,
            name,
            pid: None,
            status: ProcessStatus::Stopped,
            restart_time: 0,
            unstable_restarts: 0,
            created_at_ms,
            started_at_ms: None,
            identity: None,
            pending_restart: false,
            schedule_armed: false,
        }
    }

    #[must_use]
    pub fn uptime_ms(&self, now_ms: u64) -> Option<u64> {
        if !self.status.is_running() {
            return None;
        }
        self.started_at_ms
            .and_then(|started| now_ms.checked_sub(started))
    }

    pub fn validate_consistency(&self) -> Result<(), RuntimeError> {
        if self.status.is_running() && self.pid.is_none() {
            return Err(RuntimeError::RunningWithoutPid {
                app: self.name.clone(),
                status: self.status.as_str().to_string(),
            });
        }
        Ok(())
    }

    pub fn mark_launched(&mut self, pid: u32, now_ms: u64) {
        self.status = ProcessStatus::Launching;
        self.pid = Some(pid);
        self.started_at_ms = Some(now_ms);
        self.identity = None;
    }

    pub fn record_identity(&mut self, identity: Option<ProcessIdentity>) {
        self.identity = identity;
    }

    pub const fn mark_online(&mut self) {
        self.status = ProcessStatus::Online;
    }

    pub const fn mark_stopping(&mut self) {
        self.status = ProcessStatus::Stopping;
    }

    pub fn mark_exited(&mut self, status: ProcessStatus) {
        self.status = status;
        self.pid = None;
        self.started_at_ms = None;
        self.identity = None;
    }

    pub const fn count_restart(&mut self, unstable_restarts: u32) {
        self.restart_time = self.restart_time.saturating_add(1);
        self.unstable_restarts = unstable_restarts;
    }

    pub const fn request_restart(&mut self) {
        self.pending_restart = true;
    }

    pub const fn cancel_restart(&mut self) {
        self.pending_restart = false;
    }

    pub const fn arm_schedule(&mut self) {
        self.schedule_armed = true;
    }

    pub const fn disarm_schedule(&mut self) {
        self.schedule_armed = false;
    }

    pub const fn take_restart_request(&mut self) -> bool {
        let requested = self.pending_restart;
        self.cancel_restart();
        requested
    }
}

#[cfg(test)]
#[path = "../tests/process_runtime_tests.rs"]
mod tests;
