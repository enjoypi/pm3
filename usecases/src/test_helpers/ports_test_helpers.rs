use std::{
    collections::BTreeMap,
    sync::{Mutex, MutexGuard, PoisonError},
};

use entities::{AppSpec, SandboxMode, SandboxPolicy};

use crate::{
    Ports,
    ports::{
        Clock, CommandWrapper, DumpError, DumpStore, FingerprintError, Fingerprinter, LaunchError,
        LaunchSpec, LaunchedProcess, ProcessLauncher, ProcessProbe, SandboxError, Scheduler,
        SignalError, Signaler, WrappedCommand,
    },
    record::ProcessRecord,
};

pub const SANDBOX_PREFIX: &str = "/usr/bin/pm3-sandbox";
pub const UNSCHEDULABLE_CRON: &str = "not a cron expression";
pub const FAKE_FIRE_INTERVAL_MS: u64 = 60_000;
pub const TEXT_DIGEST_PREFIX: &str = "text:";
pub const FILE_DIGEST_PREFIX: &str = "file:";
pub const LIVE_TOKEN_PREFIX: &str = "live:";

#[must_use]
pub fn live_token(pid: u32) -> String {
    format!("{LIVE_TOKEN_PREFIX}{pid}")
}

#[derive(Debug, Default)]
struct FakeState {
    now_ms: u64,
    next_pid: u32,
    spawned: Vec<LaunchSpec>,
    spawn_failures: Vec<String>,
    wrap_failures: Vec<String>,
    terminated: Vec<u32>,
    signal_failures: Vec<u32>,
    stored: Vec<ProcessRecord>,
    saves: usize,
    load_fails: bool,
    save_fails: bool,
    live: BTreeMap<u32, String>,
    probe_blind: Vec<u32>,
    adopted: Vec<u32>,
    file_digests: BTreeMap<String, String>,
    digest_failures: Vec<String>,
}

#[derive(Debug, Default)]
pub struct FakePorts {
    state: Mutex<FakeState>,
}

impl FakePorts {
    #[must_use]
    pub fn new(now_ms: u64) -> Self {
        Self {
            state: Mutex::new(FakeState {
                now_ms,
                next_pid: 100,
                ..FakeState::default()
            }),
        }
    }

    pub fn advance_to(&self, now_ms: u64) {
        self.with_state(|state| state.now_ms = now_ms);
    }

    pub fn fail_spawn_for(&self, app: &str) {
        self.with_state(|state| state.spawn_failures.push(app.to_string()));
    }

    pub fn fail_wrap_for(&self, app: &str) {
        self.with_state(|state| state.wrap_failures.push(app.to_string()));
    }

    pub fn fail_signal_for(&self, pid: u32) {
        self.with_state(|state| state.signal_failures.push(pid));
    }

    pub fn fail_load(&self) {
        self.with_state(|state| state.load_fails = true);
    }

    pub fn fail_save(&self) {
        self.with_state(|state| state.save_fails = true);
    }

    pub fn seed_stored(&self, records: Vec<ProcessRecord>) {
        self.with_state(|state| state.stored = records);
    }

    pub fn seed_live(&self, pid: u32, token: &str) {
        self.with_state(|state| {
            state.live.insert(pid, token.to_string());
        });
    }

    pub fn seed_file_digest(&self, path: &str, digest: &str) {
        self.with_state(|state| {
            state
                .file_digests
                .insert(path.to_string(), digest.to_string());
        });
    }

    pub fn fail_file_digest_for(&self, path: &str) {
        self.with_state(|state| state.digest_failures.push(path.to_string()));
    }

    pub fn hide_from_probe(&self, pid: u32) {
        self.with_state(|state| {
            state.probe_blind.push(pid);
            state.live.remove(&pid);
        });
    }

    pub fn kill_silently(&self, pid: u32) {
        self.with_state(|state| {
            state.live.remove(&pid);
        });
    }

    #[must_use]
    pub fn spawned_names(&self) -> Vec<String> {
        self.read(|state| state.spawned.iter().map(|spec| spec.name.clone()).collect())
    }

    #[must_use]
    pub fn spawned(&self) -> Vec<LaunchSpec> {
        self.read(|state| state.spawned.clone())
    }

    #[must_use]
    pub fn adopted(&self) -> Vec<u32> {
        self.read(|state| state.adopted.clone())
    }

    #[must_use]
    pub fn terminated(&self) -> Vec<u32> {
        self.read(|state| state.terminated.clone())
    }

    #[must_use]
    pub fn save_count(&self) -> usize {
        self.read(|state| state.saves)
    }

    #[must_use]
    pub fn stored(&self) -> Vec<ProcessRecord> {
        self.read(|state| state.stored.clone())
    }

    fn locked(&self) -> MutexGuard<'_, FakeState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn with_state<F: FnOnce(&mut FakeState)>(&self, apply: F) {
        apply(&mut self.locked());
    }

    fn read<T, F: FnOnce(&FakeState) -> T>(&self, project: F) -> T {
        project(&self.locked())
    }

    fn record_spawn(&self, spec: &LaunchSpec) -> Result<LaunchedProcess, LaunchError> {
        let pid = {
            let mut guard = self.locked();
            if guard.spawn_failures.iter().any(|app| app == &spec.name) {
                return Err(LaunchError::Spawn {
                    app: spec.name.clone(),
                    reason: "injected spawn failure".to_string(),
                });
            }
            guard.spawned.push(spec.clone());
            let assigned = guard.next_pid;
            guard.next_pid = assigned.saturating_add(1);
            if !guard.probe_blind.contains(&assigned) {
                guard.live.insert(assigned, live_token(assigned));
            }
            assigned
        };
        Ok(LaunchedProcess { pid })
    }

    fn record_signal(&self, pid: u32) -> Result<(), SignalError> {
        {
            let mut guard = self.locked();
            if guard.signal_failures.contains(&pid) {
                return Err(SignalError::Delivery {
                    pid,
                    reason: "injected signal failure".to_string(),
                });
            }
            guard.terminated.push(pid);
        }
        Ok(())
    }

    fn record_save(&self, records: &[ProcessRecord]) -> Result<(), DumpError> {
        {
            let mut guard = self.locked();
            if guard.save_fails {
                return Err(DumpError::Write {
                    path: "/fake/dump.yaml".to_string(),
                    reason: "injected write failure".to_string(),
                });
            }
            guard.stored = records.to_vec();
            guard.saves = guard.saves.saturating_add(1);
        }
        Ok(())
    }

    fn read_stored(&self) -> Result<Vec<ProcessRecord>, DumpError> {
        let stored = {
            let guard = self.locked();
            if guard.load_fails {
                return Err(DumpError::Read {
                    path: "/fake/dump.yaml".to_string(),
                    reason: "injected read failure".to_string(),
                });
            }
            guard.stored.clone()
        };
        Ok(stored)
    }
}

impl ProcessLauncher for FakePorts {
    async fn spawn(&self, spec: &LaunchSpec) -> Result<LaunchedProcess, LaunchError> {
        self.record_spawn(spec)
    }

    async fn adopt(&self, pid: u32) {
        self.with_state(|state| state.adopted.push(pid));
    }
}

impl Signaler for FakePorts {
    async fn terminate(&self, pid: u32) -> Result<(), SignalError> {
        self.record_signal(pid)
    }
}

impl CommandWrapper for FakePorts {
    fn wrap(
        &self,
        app: &str,
        policy: &SandboxPolicy,
        program: &str,
        args: &[String],
    ) -> Result<WrappedCommand, SandboxError> {
        if self.read(|state| state.wrap_failures.iter().any(|name| name == app)) {
            return Err(SandboxError::NoBackend {
                app: app.to_string(),
            });
        }
        if policy.mode.is_unconfined() {
            return Ok(WrappedCommand {
                program: program.to_string(),
                args: args.to_vec(),
            });
        }
        let mut wrapped_args = vec![program.to_string()];
        wrapped_args.extend_from_slice(args);
        Ok(WrappedCommand {
            program: SANDBOX_PREFIX.to_string(),
            args: wrapped_args,
        })
    }
}

impl DumpStore for FakePorts {
    async fn load(&self) -> Result<Vec<ProcessRecord>, DumpError> {
        self.read_stored()
    }

    async fn save(&self, records: &[ProcessRecord]) -> Result<(), DumpError> {
        self.record_save(records)
    }
}

impl ProcessProbe for FakePorts {
    async fn identity(&self, pid: u32) -> Option<String> {
        self.read(|state| state.live.get(&pid).cloned())
    }
}

impl Fingerprinter for FakePorts {
    fn digest(&self, text: &str) -> String {
        format!("{TEXT_DIGEST_PREFIX}{text}")
    }

    async fn file_digest(&self, path: &str) -> Result<String, FingerprintError> {
        let digest = {
            let guard = self.locked();
            if guard.digest_failures.iter().any(|failed| failed == path) {
                return Err(FingerprintError::Read {
                    path: path.to_string(),
                    reason: "injected digest failure".to_string(),
                });
            }
            guard
                .file_digests
                .get(path)
                .cloned()
                .unwrap_or_else(|| format!("{FILE_DIGEST_PREFIX}{path}"))
        };
        Ok(digest)
    }
}

impl Ports for FakePorts {}

impl Clock for FakePorts {
    fn now_ms(&self) -> u64 {
        self.read(|state| state.now_ms)
    }
}

impl Scheduler for FakePorts {
    fn next_fire_ms(&self, cron: &str, after_ms: u64) -> Option<u64> {
        if cron == UNSCHEDULABLE_CRON {
            return None;
        }
        Some(after_ms.saturating_add(FAKE_FIRE_INTERVAL_MS))
    }
}

#[must_use]
pub fn spec(name: &str) -> AppSpec {
    AppSpec {
        name: name.to_string(),
        script: "/usr/bin/true".to_string(),
        args: Vec::new(),
        cwd: "/srv/app".to_string(),
        env: Vec::new(),
        autorestart: true,
        min_uptime_ms: 1000,
        max_restarts: 2,
        restart_delay_ms: 250,
        schedule: None,
        depends_on: Vec::new(),
        sandbox: SandboxPolicy {
            mode: SandboxMode::WorkspaceWrite,
            network: false,
            writable_roots: Vec::new(),
            derived_roots: Vec::new(),
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

pub const LOGS_DIR: &str = "/fake/logs";
