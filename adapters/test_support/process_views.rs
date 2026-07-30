use usecases::{ProcessStatus, ProcessView};

pub const RUNNING_PID: u32 = 4242;
pub const RUNNING_UPTIME_MS: u64 = 5_000;

pub fn running_view(pm_id: u32, name: &str) -> ProcessView {
    ProcessView {
        pm_id,
        name: name.to_string(),
        pid: Some(RUNNING_PID),
        status: ProcessStatus::Online,
        restart_time: 2,
        uptime_ms: Some(RUNNING_UPTIME_MS),
        sandbox_mode: "workspace-write".to_string(),
        sandbox_network: false,
        script: "/usr/bin/node".to_string(),
        args: vec!["server.js".to_string(), "--port=8080".to_string()],
        cwd: "/srv/web".to_string(),
        depends_on: vec!["db".to_string()],
        writable_roots: vec!["/srv/web".to_string()],
    }
}

pub fn idle_view(pm_id: u32, name: &str) -> ProcessView {
    ProcessView {
        pid: None,
        status: ProcessStatus::Stopped,
        uptime_ms: None,
        args: Vec::new(),
        depends_on: Vec::new(),
        writable_roots: Vec::new(),
        ..running_view(pm_id, name)
    }
}
