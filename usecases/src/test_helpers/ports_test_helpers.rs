use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Mutex, MutexGuard, PoisonError},
};

use entities::{AppSpec, ReadScope, SandboxMode, SandboxPolicy};

use crate::{
    Ports,
    ports::{
        Clock, CommandWrapper, DumpContents, DumpError, DumpStore, FingerprintError, Fingerprinter,
        LaunchError, LaunchSpec, LaunchedProcess, Liveness, ProcessLauncher, ProcessProbe,
        SandboxError, Scheduler, SignalError, SignalScope, Signaler, StrandedProcess,
        WrappedCommand,
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
    stubborn: Vec<u32>,
    waited: Vec<u32>,
    force_killed: Vec<u32>,
    signal_scopes: Vec<(u32, SignalScope)>,
    force_failures: Vec<u32>,
    stored: Vec<ProcessRecord>,
    stranded: Vec<StrandedProcess>,
    stored_boot: Option<String>,
    saved_boot: Option<String>,
    saves: usize,
    load_fails: bool,
    save_fails: bool,
    live: BTreeMap<u32, String>,
    probe_blind: Vec<u32>,
    probe_broken: Vec<u32>,
    adopted: Vec<u32>,
    tracked: BTreeSet<u32>,
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

    pub fn fail_force_kill_for(&self, pid: u32) {
        self.with_state(|state| state.force_failures.push(pid));
    }

    pub fn make_stubborn(&self, pid: u32) {
        self.with_state(|state| state.stubborn.push(pid));
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

    pub fn seed_stranded(&self, stranded: Vec<StrandedProcess>) {
        self.with_state(|state| state.stranded = stranded);
    }

    pub fn seed_boot(&self, boot: &str) {
        self.with_state(|state| state.stored_boot = Some(boot.to_string()));
    }

    #[must_use]
    pub fn saved_boot(&self) -> Option<String> {
        self.read(|state| state.saved_boot.clone())
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

    pub fn break_probe_for(&self, pid: u32) {
        self.with_state(|state| state.probe_broken.push(pid));
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
    pub fn waited(&self) -> Vec<u32> {
        self.read(|state| state.waited.clone())
    }

    #[must_use]
    pub fn force_killed(&self) -> Vec<u32> {
        self.read(|state| state.force_killed.clone())
    }

    #[must_use]
    pub fn signal_scopes(&self) -> Vec<(u32, SignalScope)> {
        self.read(|state| state.signal_scopes.clone())
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
            guard.tracked.insert(assigned);
            if !guard.probe_blind.contains(&assigned) {
                guard.live.insert(assigned, live_token(assigned));
            }
            assigned
        };
        Ok(LaunchedProcess { pid })
    }

    fn record_signal(&self, pid: u32, scope: SignalScope) -> Result<(), SignalError> {
        {
            let mut guard = self.locked();
            guard.signal_scopes.push((pid, scope));
            if guard.signal_failures.contains(&pid) {
                return Err(SignalError::Delivery {
                    pid,
                    reason: "injected signal failure".to_string(),
                });
            }
            guard.terminated.push(pid);
            if !guard.stubborn.contains(&pid) {
                guard.live.remove(&pid);
            }
        }
        Ok(())
    }

    fn record_force_kill(&self, pid: u32, scope: SignalScope) -> Result<(), SignalError> {
        {
            let mut guard = self.locked();
            guard.signal_scopes.push((pid, scope));
            if guard.force_failures.contains(&pid) {
                return Err(SignalError::Delivery {
                    pid,
                    reason: "injected force kill failure".to_string(),
                });
            }
            guard.force_killed.push(pid);
            guard.live.remove(&pid);
        }
        Ok(())
    }

    fn record_save(&self, records: &[ProcessRecord], boot: Option<&str>) -> Result<(), DumpError> {
        {
            let mut guard = self.locked();
            if guard.save_fails {
                return Err(DumpError::Write {
                    path: "/fake/dump.yaml".to_string(),
                    reason: "injected write failure".to_string(),
                });
            }
            guard.stored = records.to_vec();
            guard.saved_boot = boot.map(ToString::to_string);
            guard.saves = guard.saves.saturating_add(1);
        }
        Ok(())
    }

    fn read_stored(&self) -> Result<DumpContents, DumpError> {
        let contents = {
            let guard = self.locked();
            if guard.load_fails {
                return Err(DumpError::Read {
                    path: "/fake/dump.yaml".to_string(),
                    reason: "injected read failure".to_string(),
                });
            }
            DumpContents {
                records: guard.stored.clone(),
                stranded: guard.stranded.clone(),
                boot: guard.stored_boot.clone(),
            }
        };
        Ok(contents)
    }
}

impl ProcessLauncher for FakePorts {
    async fn spawn(&self, spec: &LaunchSpec) -> Result<LaunchedProcess, LaunchError> {
        self.record_spawn(spec)
    }

    async fn adopt(&self, pid: u32) {
        self.with_state(|state| {
            state.adopted.push(pid);
            state.tracked.insert(pid);
        });
    }

    async fn tracked_pids(&self) -> Vec<u32> {
        self.read(|state| state.tracked.iter().copied().collect())
    }
}

impl Signaler for FakePorts {
    async fn terminate(&self, pid: u32, scope: SignalScope) -> Result<(), SignalError> {
        self.record_signal(pid, scope)
    }

    async fn force_kill(&self, pid: u32, scope: SignalScope) -> Result<(), SignalError> {
        self.record_force_kill(pid, scope)
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
    async fn load(&self) -> Result<DumpContents, DumpError> {
        self.read_stored()
    }

    async fn save(&self, records: &[ProcessRecord], boot: Option<&str>) -> Result<(), DumpError> {
        self.record_save(records, boot)
    }
}

impl ProcessProbe for FakePorts {
    async fn resident_memory(&self, _pids: &[u32]) -> BTreeMap<u32, u64> {
        BTreeMap::new()
    }

    async fn identity(&self, pid: u32) -> Liveness {
        self.read(|state| {
            if state.probe_broken.contains(&pid) {
                return Liveness::Unreadable;
            }
            state
                .live
                .get(&pid)
                .cloned()
                .map_or(Liveness::Gone, Liveness::Alive)
        })
    }

    async fn wait_gone(&self, pid: u32, timeout_ms: u64) -> Liveness {
        let _ = timeout_ms;
        self.with_state(|state| state.waited.push(pid));
        self.identity(pid).await
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
        max_memory_kib: None,
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
            read: ReadScope::Minimal,
            network: false,
            writable_roots: Vec::new(),
            readable_roots: Vec::new(),
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

pub const LOGS_DIR: &str = "/fake/logs";
