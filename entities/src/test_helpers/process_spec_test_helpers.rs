use super::*;
use crate::{ReadScope, SandboxMode};

pub fn confined_policy() -> SandboxPolicy {
    SandboxPolicy {
        mode: SandboxMode::WorkspaceWrite,
        read: ReadScope::Minimal,
        network: false,
        writable_roots: Vec::new(),
        readable_roots: Vec::new(),
        derived_readable_roots: Vec::new(),
        derived_roots: Vec::new(),
        unreadable_roots: Vec::new(),
    }
}

pub fn spec(name: &str) -> AppSpec {
    AppSpec {
        max_memory_kib: None,
        ready_probe: None,
        listen_timeout_ms: None,
        stop_exit_codes: Vec::new(),
        name: name.to_string(),
        script: "/usr/bin/true".to_string(),
        args: Vec::new(),
        cwd: "/srv/app".to_string(),
        env: Vec::new(),
        autorestart: true,
        min_uptime_ms: 1000,
        max_restarts: 15,
        restart_delay_ms: 0,
        max_restart_delay_ms: 15000,
        schedule: None,
        depends_on: Vec::new(),
        sandbox: confined_policy(),
    }
}
