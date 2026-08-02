use std::{path::PathBuf, sync::Arc};

use adapters::{
    AdoptedWatch, Clock, CommandWrapper, CronScheduler, DumpError, DumpStore, ExitOutcome,
    FingerprintError, Fingerprinter, HostSandbox, KillSignaler, LaunchError, LaunchSpec,
    LaunchedProcess, Liveness, PollCadence, Ports, ProcessLauncher, ProcessProbe, ProcessRecord,
    PsProcessProbe, SandboxCommandWrapper, SandboxError, SandboxPolicy, Scheduler,
    Sha256Fingerprinter, SignalError, Signaler, SpecSource, SystemClock, TokioProcessLauncher,
    WrappedCommand, YamlDumpStore, wait_for_exit,
};

#[derive(Debug)]
pub struct DaemonPorts {
    launcher: TokioProcessLauncher,
    signaler: KillSignaler,
    wrapper: SandboxCommandWrapper,
    store: YamlDumpStore,
    clock: SystemClock,
    probe: Arc<PsProcessProbe>,
    watch: Arc<AdoptedWatch>,
    fingerprinter: Sha256Fingerprinter,
    scheduler: CronScheduler,
    cadence: PollCadence,
}

impl DaemonPorts {
    #[must_use]
    pub fn new(dump_file: PathBuf, specs: SpecSource, backend: Option<HostSandbox>) -> Self {
        let stop_signal = specs.config.stop_signal.clone();
        let command_timeout_ms = specs.config.command_timeout_ms;
        let poll_interval_ms = specs.config.daemon_poll_interval_ms;
        let cadence = PollCadence {
            interval_ms: specs.config.daemon_poll_interval_ms,
            max_interval_ms: specs.config.daemon_poll_max_interval_ms,
        };
        Self {
            launcher: TokioProcessLauncher::default(),
            signaler: KillSignaler::with_stop_signal(stop_signal, command_timeout_ms),
            wrapper: SandboxCommandWrapper::new(backend),
            store: YamlDumpStore::new(dump_file, specs),
            clock: SystemClock,
            probe: Arc::new(
                PsProcessProbe::with_timeout(command_timeout_ms)
                    .with_poll_interval(poll_interval_ms),
            ),
            watch: Arc::new(AdoptedWatch::default()),
            fingerprinter: Sha256Fingerprinter,
            scheduler: CronScheduler,
            cadence,
        }
    }

    pub async fn wait(&self, pid: u32, token: Option<String>) -> Option<ExitOutcome> {
        wait_for_exit(
            &self.launcher,
            &self.watch,
            Arc::clone(&self.probe),
            pid,
            token,
            self.cadence,
        )
        .await
    }

    pub async fn tracked_pids(&self) -> Vec<u32> {
        self.launcher.tracked_pids().await
    }
}

impl Clock for DaemonPorts {
    fn now_ms(&self) -> u64 {
        self.clock.now_ms()
    }
}

impl Scheduler for DaemonPorts {
    fn next_fire_ms(&self, cron: &str, after_ms: u64) -> Option<u64> {
        self.scheduler.next_fire_ms(cron, after_ms)
    }
}

impl CommandWrapper for DaemonPorts {
    fn wrap(
        &self,
        app: &str,
        policy: &SandboxPolicy,
        program: &str,
        args: &[String],
    ) -> Result<WrappedCommand, SandboxError> {
        self.wrapper.wrap(app, policy, program, args)
    }
}

impl DumpStore for DaemonPorts {
    async fn load(&self) -> Result<Vec<ProcessRecord>, DumpError> {
        self.store.load().await
    }

    async fn save(&self, records: &[ProcessRecord]) -> Result<(), DumpError> {
        self.store.save(records).await
    }
}

impl Fingerprinter for DaemonPorts {
    fn digest(&self, text: &str) -> String {
        self.fingerprinter.digest(text)
    }

    async fn file_digest(&self, path: &str) -> Result<String, FingerprintError> {
        self.fingerprinter.file_digest(path).await
    }
}

impl ProcessLauncher for DaemonPorts {
    async fn spawn(&self, spec: &LaunchSpec) -> Result<LaunchedProcess, LaunchError> {
        self.launcher.spawn(spec).await
    }

    async fn adopt(&self, pid: u32) {
        self.launcher.adopt(pid).await;
    }
}

impl ProcessProbe for DaemonPorts {
    async fn identity(&self, pid: u32) -> Liveness {
        self.probe.identity(pid).await
    }

    async fn wait_gone(&self, pid: u32, timeout_ms: u64) -> Liveness {
        self.probe.wait_gone(pid, timeout_ms).await
    }
}

impl Ports for DaemonPorts {}

impl Signaler for DaemonPorts {
    async fn terminate(&self, pid: u32) -> Result<(), SignalError> {
        self.signaler.terminate(pid).await
    }

    async fn force_kill(&self, pid: u32) -> Result<(), SignalError> {
        self.signaler.force_kill(pid).await
    }
}

#[cfg(test)]
#[path = "../tests/daemon_ports_tests.rs"]
mod tests;
