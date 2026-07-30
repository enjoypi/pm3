use entities::{AppSpec, ProcessRuntime, SandboxMode, SandboxPolicy};

use super::ProcessRecord;

pub fn spec(name: &str) -> AppSpec {
    AppSpec {
        name: name.to_string(),
        script: "/usr/bin/true".to_string(),
        args: Vec::new(),
        cwd: "/srv/app".to_string(),
        env: Vec::new(),
        autorestart: true,
        min_uptime_ms: 1000,
        max_restarts: 15,
        restart_delay_ms: 0,
        depends_on: Vec::new(),
        sandbox: SandboxPolicy {
            mode: SandboxMode::WorkspaceWrite,
            network: false,
            writable_roots: Vec::new(),
        },
    }
}

pub fn record(name: &str, pm_id: u32) -> ProcessRecord {
    ProcessRecord {
        spec: spec(name),
        runtime: ProcessRuntime::new(pm_id, name.to_string(), 1000),
    }
}
