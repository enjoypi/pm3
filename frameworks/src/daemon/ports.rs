use std::path::PathBuf;

use adapters::{
    Clock, CommandWrapper, DumpError, DumpStore, ExitOutcome, KillSignaler, LaunchError,
    LaunchSpec, LaunchedProcess, Ports, ProcessLauncher, ProcessRecord, SandboxBackend,
    SandboxCommandWrapper, SandboxError, SandboxPolicy, SignalError, Signaler, SpecSource,
    SystemClock, TokioProcessLauncher, WrappedCommand, YamlDumpStore,
};

#[derive(Debug)]
pub struct DaemonPorts {
    launcher: TokioProcessLauncher,
    signaler: KillSignaler,
    wrapper: SandboxCommandWrapper,
    store: YamlDumpStore,
    clock: SystemClock,
}

impl DaemonPorts {
    #[must_use]
    pub fn new(dump_file: PathBuf, specs: SpecSource, backend: Option<SandboxBackend>) -> Self {
        Self {
            launcher: TokioProcessLauncher::default(),
            signaler: KillSignaler::default(),
            wrapper: SandboxCommandWrapper::new(backend),
            store: YamlDumpStore::new(dump_file, specs),
            clock: SystemClock,
        }
    }

    pub async fn wait(&self, pid: u32) -> Option<ExitOutcome> {
        self.launcher.wait(pid).await
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

impl ProcessLauncher for DaemonPorts {
    async fn spawn(&self, spec: &LaunchSpec) -> Result<LaunchedProcess, LaunchError> {
        self.launcher.spawn(spec).await
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
