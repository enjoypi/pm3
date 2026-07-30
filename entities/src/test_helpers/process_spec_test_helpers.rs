use super::*;
use crate::SandboxMode;

pub fn confined_policy() -> SandboxPolicy {
    SandboxPolicy {
        mode: SandboxMode::WorkspaceWrite,
        network: false,
        writable_roots: Vec::new(),
        derived_roots: Vec::new(),
    }
}

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
        sandbox: confined_policy(),
    }
}
