use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::{Mutex, MutexGuard, PoisonError},
};

use entities::{AppSpec, ReadScope, ReadyProbe, SandboxMode, SandboxPolicy};

use crate::{
    Ports,
    ports::{
        Clock, CommandWrapper, DumpContents, DumpError, DumpStore, FingerprintError, Fingerprinter,
        LaunchError, LaunchSpec, LaunchedProcess, Liveness, LogRotateError, LogRotator,
        ProcessLauncher, ProcessProbe, Readiness, ReadyProber, ResourceSample, RotatedLog,
        SandboxError, Scheduler, SignalError, SignalScope, Signaler, StrandedProcess,
        WrappedCommand,
    },
    record::ProcessRecord,
    start::start_apps,
    table::ProcessTable,
};

pub const SANDBOX_PREFIX: &str = "/usr/bin/pm3-sandbox";
pub const UNSCHEDULABLE_CRON: &str = "not a cron expression";
pub const FAKE_FIRE_INTERVAL_MS: u64 = 60_000;
pub const TEXT_DIGEST_PREFIX: &str = "text:";
pub const FILE_DIGEST_PREFIX: &str = "file:";
pub const LIVE_TOKEN_PREFIX: &str = "live:";
pub const RECYCLED_TOKEN_PREFIX: &str = "recycled:";

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
    delivered: Vec<(String, u32)>,
    signal_failures: Vec<u32>,
    stubborn: Vec<u32>,
    recycled_on_signal: Vec<u32>,
    recycled_after_probe: Vec<u32>,
    vanished_after_probe: Vec<u32>,
    probed: BTreeSet<u32>,
    waited: Vec<u32>,
    events: Vec<String>,
    slow_wait: bool,
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
    resources: BTreeMap<u32, ResourceSample>,
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

    pub fn recycle_on_signal(&self, pid: u32) {
        self.with_state(|state| state.recycled_on_signal.push(pid));
    }

    pub fn recycle_after_probe(&self, pid: u32) {
        self.with_state(|state| state.recycled_after_probe.push(pid));
    }

    pub fn vanish_after_probe(&self, pid: u32) {
        self.with_state(|state| state.vanished_after_probe.push(pid));
    }

    pub fn slow_waits(&self) {
        self.with_state(|state| state.slow_wait = true);
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

    pub fn seed_resource(&self, pid: u32, sample: ResourceSample) {
        self.with_state(|state| {
            state.resources.insert(pid, sample);
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
    pub fn delivered(&self) -> Vec<(String, u32)> {
        self.read(|state| state.delivered.clone())
    }

    #[must_use]
    pub fn waited(&self) -> Vec<u32> {
        self.read(|state| state.waited.clone())
    }

    #[must_use]
    pub fn events(&self) -> Vec<String> {
        self.read(|state| state.events.clone())
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
            guard.events.push(format!("terminate:{pid}"));
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
            if guard.recycled_on_signal.contains(&pid) {
                guard
                    .live
                    .insert(pid, format!("{RECYCLED_TOKEN_PREFIX}{pid}"));
            }
            drop(guard);
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

    fn record_deliver(
        &self,
        signal: &str,
        pid: u32,
        scope: SignalScope,
    ) -> Result<(), SignalError> {
        {
            let mut guard = self.locked();
            guard.signal_scopes.push((pid, scope));
            if guard.signal_failures.contains(&pid) {
                return Err(SignalError::Delivery {
                    pid,
                    reason: "injected signal failure".to_string(),
                });
            }
            guard.delivered.push((signal.to_string(), pid));
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

#[must_use]
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

#[path = "ports_fake_impls_test_helpers.rs"]
mod impls;
