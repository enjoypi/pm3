use std::path::PathBuf;

use adapters::{
    Clock, CommandWrapper, DumpError, DumpStore, ExitOutcome, FingerprintError, Fingerprinter,
    KillSignaler, LaunchError, LaunchSpec, LaunchedProcess, Ports, ProcessLauncher, ProcessProbe,
    ProcessRecord, PsProcessProbe, SandboxBackend, SandboxCommandWrapper, SandboxError,
    SandboxPolicy, Sha256Fingerprinter, SignalError, Signaler, SpecSource, SystemClock,
    TokioProcessLauncher, WrappedCommand, YamlDumpStore, wait_for_exit,
};

#[derive(Debug)]
pub struct DaemonPorts {
    launcher: TokioProcessLauncher,
    signaler: KillSignaler,
    wrapper: SandboxCommandWrapper,
    store: YamlDumpStore,
    clock: SystemClock,
    probe: PsProcessProbe,
    fingerprinter: Sha256Fingerprinter,
    poll_interval_ms: u64,
}

impl DaemonPorts {
    #[must_use]
    pub fn new(dump_file: PathBuf, specs: SpecSource, backend: Option<SandboxBackend>) -> Self {
        let stop_signal = specs.config.stop_signal.clone();
        let command_timeout_ms = specs.config.command_timeout_ms;
        let poll_interval_ms = specs.config.daemon_poll_interval_ms;
        Self {
            launcher: TokioProcessLauncher::default(),
            signaler: KillSignaler::with_stop_signal(stop_signal, command_timeout_ms),
            wrapper: SandboxCommandWrapper::new(backend),
            store: YamlDumpStore::new(dump_file, specs),
            clock: SystemClock,
            probe: PsProcessProbe::with_timeout(command_timeout_ms),
            fingerprinter: Sha256Fingerprinter,
            poll_interval_ms,
        }
    }

    pub async fn wait(&self, pid: u32) -> Option<ExitOutcome> {
        wait_for_exit(&self.launcher, &self.probe, pid, self.poll_interval_ms).await
    }

    pub async fn tracked_pids(&self) -> Vec<u32> {
        self.launcher.tracked_pids().await
    }

    pub async fn force_kill(&self, pid: u32) -> Result<(), SignalError> {
        self.signaler.force_kill(pid).await
    }
}

impl Clock for DaemonPorts {
    fn now_ms(&self) -> u64 {
        self.clock.now_ms()
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
    async fn identity(&self, pid: u32) -> Option<String> {
        self.probe.identity(pid).await
    }
}

impl Ports for DaemonPorts {}

impl Signaler for DaemonPorts {
    async fn terminate(&self, pid: u32) -> Result<(), SignalError> {
        self.signaler.terminate(pid).await
    }
}

#[cfg(test)]
#[path = "../tests/daemon_ports_tests.rs"]
mod tests;
