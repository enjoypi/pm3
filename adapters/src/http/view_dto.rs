use serde::{Deserialize, Serialize};
use usecases::ProcessView;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProcessViewDto {
    pub pm_id: u32,
    pub name: String,
    pub pid: Option<u32>,
    pub status: String,
    pub restart_time: u32,
    pub uptime_ms: Option<u64>,
    pub next_fire_ms: Option<u64>,
    pub schedule: Option<String>,
    pub sandbox_mode: String,
    pub sandbox_network: bool,
    pub script: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub depends_on: Vec<String>,
    pub writable_roots: Vec<String>,
    pub rss_kib: Option<u64>,
    pub cpu_tenths: Option<u32>,
}

impl From<&ProcessView> for ProcessViewDto {
    fn from(view: &ProcessView) -> Self {
        Self {
            pm_id: view.pm_id,
            name: view.name.clone(),
            pid: view.pid,
            status: view.status.as_str().to_string(),
            restart_time: view.restart_time,
            uptime_ms: view.uptime_ms,
            next_fire_ms: view.next_fire_ms,
            schedule: view.schedule.clone(),
            sandbox_mode: view.sandbox_mode.clone(),
            sandbox_network: view.sandbox_network,
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
