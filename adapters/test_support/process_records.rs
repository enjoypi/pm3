use usecases::{
    AppSpec, ProcessIdentity, ProcessRecord, ProcessRuntime, ProcessStatus, ReadScope, SandboxMode,
    SandboxPolicy,
};

pub const CREATED_AT_MS: u64 = 1_700_000_000_000;
pub const STARTED_AT_MS: u64 = 1_700_000_001_000;
pub const SAMPLE_PID: u32 = 4242;
pub const SAMPLE_TOKEN: &str = "Tue Jul 28 14:06:28 2026";
pub const SAMPLE_BOOT: &str = "Mon Jul 27 08:00:00 2026";
pub const SAMPLE_LAUNCH_DIGEST: &str = "1111111111111111";
pub const SAMPLE_BINARY_DIGEST: &str = "2222222222222222";

pub fn sample_identity() -> ProcessIdentity {
    ProcessIdentity {
        token: SAMPLE_TOKEN.to_string(),
        launch_digest: SAMPLE_LAUNCH_DIGEST.to_string(),
        binary_digest: SAMPLE_BINARY_DIGEST.to_string(),
    }
}

pub fn sample_spec(name: &str) -> AppSpec {
    AppSpec {
        max_memory_kib: None,
        ready_probe: None,
        listen_timeout_ms: None,
        name: name.to_string(),
        script: "/usr/bin/node".to_string(),
        args: vec!["server.js".to_string(), "--port=8080".to_string()],
        cwd: "/srv/web".to_string(),
        env: vec![("PORT".to_string(), "8080".to_string())],
        autorestart: true,
        min_uptime_ms: 1000,
        max_restarts: 15,
        restart_delay_ms: 40,
        max_restart_delay_ms: 15000,
        schedule: None,
        depends_on: vec!["db".to_string()],
        sandbox: SandboxPolicy {
            mode: SandboxMode::WorkspaceWrite,
            read: ReadScope::Minimal,
            network: false,
            writable_roots: vec!["/srv/web".to_string()],
            readable_roots: Vec::new(),
            derived_roots: Vec::new(),
            unreadable_roots: Vec::new(),
        },
    }
}

pub fn sample_runtime(name: &str) -> ProcessRuntime {
    ProcessRuntime {
        pm_id: 3,
        name: name.to_string(),
        pid: Some(SAMPLE_PID),
        status: ProcessStatus::Online,
        restart_time: 2,
        unstable_restarts: 1,
        created_at_ms: CREATED_AT_MS,
        started_at_ms: Some(STARTED_AT_MS),
        identity: Some(sample_identity()),
        pending_restart: false,
        schedule_armed: true,
    }
}

pub fn sample_record(name: &str) -> ProcessRecord {
    ProcessRecord {
        spec: sample_spec(name),
        runtime: sample_runtime(name),
    }
}

pub fn stopped_record(name: &str) -> ProcessRecord {
    ProcessRecord {
        spec: sample_spec(name),
        runtime: ProcessRuntime {
            pid: None,
            status: ProcessStatus::Stopped,
            started_at_ms: None,
            identity: None,
            ..sample_runtime(name)
        },
    }
}
