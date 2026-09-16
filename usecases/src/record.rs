use std::collections::BTreeMap;

use entities::{
    AppSpec, EnvOrigin, EnvScope, EnvValue, ProcessRuntime, ProcessStatus, mask_secret,
};

use crate::ports::ResourceSample;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessRecord {
    pub spec: AppSpec,
    pub runtime: ProcessRuntime,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvDisplay {
    pub key: String,
    pub value: String,
    pub scope: EnvScope,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessView {
    pub pm_id: u32,
    pub name: String,
    pub pid: Option<u32>,
    pub status: ProcessStatus,
    pub restart_time: u32,
    pub unstable_restarts: u32,
    pub max_restarts: u32,
    pub autorestart: bool,
    pub uptime_ms: Option<u64>,
    pub next_fire_ms: Option<u64>,
    pub schedule: Option<String>,
    pub sandbox_mode: String,
    pub sandbox_read: String,
    pub sandbox_network: bool,
    pub env_origin: EnvOrigin,
    pub env_declared: usize,
    pub env: Vec<EnvDisplay>,
    pub script: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub depends_on: Vec<String>,
    pub writable_roots: Vec<String>,
    pub rss_kib: Option<u64>,
    pub cpu_tenths: Option<u32>,
}

impl ProcessView {
    #[must_use]
    pub fn with_sample(mut self, samples: &BTreeMap<u32, ResourceSample>) -> Self {
        if let Some(sample) = self.pid.and_then(|pid| samples.get(&pid)) {
            self.rss_kib = Some(sample.rss_kib);
            self.cpu_tenths = Some(sample.cpu_tenths);
        }
        self
    }
}

impl ProcessRecord {
    #[must_use]
    pub fn view(&self, now_ms: u64) -> ProcessView {
        ProcessView {
            pm_id: self.runtime.pm_id,
            name: self.runtime.name.clone(),
            pid: self.runtime.pid,
            status: self.runtime.status,
            restart_time: self.runtime.restart_time,
            unstable_restarts: self.runtime.unstable_restarts,
            max_restarts: self.spec.max_restarts,
            autorestart: self.spec.autorestart,
            uptime_ms: self.runtime.uptime_ms(now_ms),
            next_fire_ms: None,
            schedule: self.spec.schedule.clone(),
            sandbox_mode: self.spec.sandbox.mode.as_str().to_string(),
            sandbox_read: self.spec.sandbox.read.as_str().to_string(),
            sandbox_network: self.spec.sandbox.network,
            env_origin: self.spec.env_origin,
            env_declared: self.spec.declared_env_count(),
            env: masked_environment(&self.spec.env),
            script: self.spec.script.clone(),
            args: self.spec.args.clone(),
            cwd: self.spec.cwd.clone(),
            depends_on: self.spec.depends_on.clone(),
            writable_roots: self
                .spec
                .sandbox
                .granted_roots()
                .into_iter()
                .map(ToString::to_string)
                .collect(),
            rss_kib: None,
            cpu_tenths: None,
        }
    }
}

fn masked_environment(env: &[EnvValue]) -> Vec<EnvDisplay> {
    env.iter().map(masked_entry).collect()
}

fn masked_entry(entry: &EnvValue) -> EnvDisplay {
    EnvDisplay {
        key: entry.key.clone(),
        value: mask_secret(&entry.value),
        scope: entry.scope,
    }
}

#[cfg(test)]
#[path = "test_helpers/record_test_helpers.rs"]
mod test_helpers;
#[cfg(test)]
#[path = "tests/record_tests.rs"]
mod tests;
