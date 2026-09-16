use usecases::{EnvDisplay, ProcessStatus, ProcessView};

pub const RUNNING_PID: u32 = 4242;
pub const RUNNING_UPTIME_MS: u64 = 5_000;
pub const RESTART_TIME: u32 = 47;
pub const UNSTABLE_RESTARTS: u32 = 3;
pub const MAX_RESTARTS: u32 = 15;

pub fn running_view(pm_id: u32, name: &str) -> ProcessView {
    ProcessView {
        env_origin: usecases::EnvOrigin::default(),
        env_declared: 0,
        env: Vec::new(),
        pm_id,
        name: name.to_string(),
        pid: Some(RUNNING_PID),
        status: ProcessStatus::Online,
        restart_time: RESTART_TIME,
        unstable_restarts: UNSTABLE_RESTARTS,
        max_restarts: MAX_RESTARTS,
        autorestart: true,
        uptime_ms: Some(RUNNING_UPTIME_MS),
        next_fire_ms: None,
        schedule: None,
        sandbox_mode: "workspace-write".to_string(),
        sandbox_read: "minimal".to_string(),
        sandbox_network: false,
        script: "/usr/bin/node".to_string(),
        args: vec!["server.js".to_string(), "--port=8080".to_string()],
        cwd: "/srv/web".to_string(),
        depends_on: vec!["db".to_string()],
        writable_roots: vec!["/srv/web".to_string()],
        rss_kib: None,
        cpu_tenths: None,
    }
}

pub fn view_with_env(pm_id: u32, name: &str, env: Vec<EnvDisplay>) -> ProcessView {
    ProcessView {
        env_declared: env.len(),
        env,
        ..running_view(pm_id, name)
    }
}

pub fn idle_view(pm_id: u32, name: &str) -> ProcessView {
    ProcessView {
        pid: None,
        status: ProcessStatus::Stopped,
        uptime_ms: None,
        next_fire_ms: None,
        schedule: None,
        args: Vec::new(),
        depends_on: Vec::new(),
        writable_roots: Vec::new(),
        ..running_view(pm_id, name)
    }
}
