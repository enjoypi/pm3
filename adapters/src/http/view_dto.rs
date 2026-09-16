use serde::{Deserialize, Serialize};
use usecases::{EnvDisplay, ProcessView};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EnvDisplayDto {
    pub key: String,
    pub value: String,
    pub scope: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProcessViewDto {
    pub pm_id: u32,
    pub name: String,
    pub pid: Option<u32>,
    pub status: String,
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
    pub env_origin: String,
    pub env_declared: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env_masked: Vec<EnvDisplayDto>,
    pub script: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub depends_on: Vec<String>,
    pub writable_roots: Vec<String>,
    pub rss_kib: Option<u64>,
    pub cpu_tenths: Option<u32>,
}

impl From<&EnvDisplay> for EnvDisplayDto {
    fn from(shown: &EnvDisplay) -> Self {
        Self {
            key: shown.key.clone(),
            value: shown.value.clone(),
            scope: shown.scope.as_str().to_string(),
        }
    }
}

impl From<&ProcessView> for ProcessViewDto {
    fn from(view: &ProcessView) -> Self {
        Self {
            pm_id: view.pm_id,
            name: view.name.clone(),
            pid: view.pid,
            status: view.status.as_str().to_string(),
            restart_time: view.restart_time,
            unstable_restarts: view.unstable_restarts,
            max_restarts: view.max_restarts,
            autorestart: view.autorestart,
            uptime_ms: view.uptime_ms,
            next_fire_ms: view.next_fire_ms,
            schedule: view.schedule.clone(),
            sandbox_mode: view.sandbox_mode.clone(),
            sandbox_read: view.sandbox_read.clone(),
            sandbox_network: view.sandbox_network,
            env_origin: view.env_origin.as_str().to_string(),
            env_declared: view.env_declared,
            env_masked: view.env.iter().map(EnvDisplayDto::from).collect(),
            script: view.script.clone(),
            args: view.args.clone(),
            cwd: view.cwd.clone(),
            depends_on: view.depends_on.clone(),
            writable_roots: view.writable_roots.clone(),
            rss_kib: view.rss_kib,
            cpu_tenths: view.cpu_tenths,
        }
    }
}

#[cfg(test)]
#[path = "../tests/http_view_dto_tests.rs"]
mod tests;
