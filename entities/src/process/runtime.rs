use super::status::ProcessStatus;

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
    pub pending_restart: bool,
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
            pending_restart: false,
        }
    }

    #[must_use]
    pub fn uptime_ms(&self, now_ms: u64) -> Option<u64> {
        if !self.status.is_running() {
            return None;
        }
        self.started_at_ms
            .map(|started| now_ms.saturating_sub(started))
    }

    pub const fn mark_launched(&mut self, pid: u32, now_ms: u64) {
        self.status = ProcessStatus::Launching;
        self.pid = Some(pid);
        self.started_at_ms = Some(now_ms);
    }

    pub const fn mark_online(&mut self) {
        self.status = ProcessStatus::Online;
    }

    pub const fn mark_stopping(&mut self) {
        self.status = ProcessStatus::Stopping;
    }

    pub const fn mark_exited(&mut self, status: ProcessStatus) {
        self.status = status;
        self.pid = None;
        self.started_at_ms = None;
    }

    pub const fn count_restart(&mut self, unstable_restarts: u32) {
        self.restart_time = self.restart_time.saturating_add(1);
        self.unstable_restarts = unstable_restarts;
    }

    pub const fn request_restart(&mut self) {
        self.pending_restart = true;
    }

    pub const fn take_restart_request(&mut self) -> bool {
        let requested = self.pending_restart;
        self.pending_restart = false;
        requested
    }
}

#[cfg(test)]
#[path = "../tests/process_runtime_tests.rs"]
mod tests;
