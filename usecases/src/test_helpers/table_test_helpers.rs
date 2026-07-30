use entities::{AppSpec, SandboxMode, SandboxPolicy};

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

pub fn spec_with_deps(name: &str, depends_on: &[&str]) -> AppSpec {
    AppSpec {
        depends_on: depends_on.iter().map(|dep| (*dep).to_string()).collect(),
        ..spec(name)
    }
}
