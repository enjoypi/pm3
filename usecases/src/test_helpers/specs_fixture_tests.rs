use entities::{
    AppSpec, ProcessRuntime, ProcessStatus, ReadScope, ReadyProbe, SandboxMode, SandboxPolicy,
};

use super::FakePorts;
use crate::{
    ports::{SpecResolveError, SpecResolver},
    record::ProcessRecord,
    start::start_apps,
    supervisor::Supervisor,
    table::ProcessTable,
};

#[must_use]
pub fn spec(name: &str) -> AppSpec {
    AppSpec {
        env_origin: entities::EnvOrigin::default(),
        max_memory_kib: None,
        ready_probe: None,
        liveness_probe: None,
        listen_timeout_ms: None,
        stop_exit_codes: Vec::new(),
        self_responsible: false,
        priority: crate::Priority::Background,
        name: name.to_string(),
        script: "/usr/bin/true".to_string(),
        args: Vec::new(),
        cwd: "/srv/app".to_string(),
        env: Vec::new(),
        autorestart: true,
        min_uptime_ms: 1000,
        max_restarts: 2,
        restart_delay_ms: 250,
        max_restart_delay_ms: 15000,
        schedule: None,
        depends_on: Vec::new(),
        sandbox: SandboxPolicy {
            mode: SandboxMode::WorkspaceWrite,
            read: ReadScope::Minimal,
            network: false,
            writable_roots: Vec::new(),
            readable_roots: Vec::new(),
            derived_readable_roots: Vec::new(),
            derived_roots: Vec::new(),
            unreadable_roots: Vec::new(),
        },
    }
}

#[must_use]
pub fn spec_with_deps(name: &str, depends_on: &[&str]) -> AppSpec {
    AppSpec {
        depends_on: depends_on.iter().map(|dep| (*dep).to_string()).collect(),
        ..spec(name)
    }
}

#[must_use]
pub fn record(name: &str, pm_id: u32) -> ProcessRecord {
    ProcessRecord {
        spec: spec(name),
        runtime: ProcessRuntime::new(pm_id, name.to_string(), 1000),
    }
}

#[must_use]
pub fn spec_probed(name: &str) -> AppSpec {
    AppSpec {
        ready_probe: Some(ReadyProbe::Exec {
            command: vec!["/usr/bin/true".to_string()],
        }),
        ..spec(name)
    }
}

pub const LOGS_DIR: &str = "/fake/logs";

pub async fn started_table(ports: &FakePorts) -> ProcessTable {
    let mut table = ProcessTable::new();
    start_apps(&mut table, &[spec("api")], LOGS_DIR, ports).await;
    table
}

pub const KILL_TIMEOUT_MS: u64 = 1600;
pub const READY_TIMEOUT_MS: u64 = 30000;
pub const READY_POLL_MS: u64 = 200;
pub const NOTIFY_COOLDOWN_MS: u64 = 3_600_000;

pub fn supervisor() -> Supervisor {
    Supervisor::new(
        LOGS_DIR.to_string(),
        KILL_TIMEOUT_MS,
        READY_TIMEOUT_MS,
        READY_POLL_MS,
        NOTIFY_COOLDOWN_MS,
    )
}

pub fn status_of(supervisor: &Supervisor, name: &str) -> ProcessStatus {
    supervisor
        .table
        .find_by_name(name)
        .expect("the record should exist")
        .runtime
        .status
}

pub struct StaticResolver(Option<&'static str>);

impl StaticResolver {
    pub const fn always() -> Self {
        Self(None)
    }

    pub const fn failing(name: &'static str) -> Self {
        Self(Some(name))
    }
}

#[expect(
    clippy::unused_async_trait_impl,
    reason = "桩实现直接返回既有值，无 await；改 impl Future + ready 会让测试夹具难读"
)]
impl SpecResolver for StaticResolver {
    async fn prepare(&self, name: &str) -> Result<AppSpec, SpecResolveError> {
        if self.0 == Some(name) {
            return Err(SpecResolveError::Missing {
                name: name.to_string(),
                reason: "the declaration vanished".to_string(),
            });
        }
        Ok(spec(name))
    }
}
