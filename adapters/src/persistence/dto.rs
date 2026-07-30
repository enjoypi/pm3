use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use usecases::{AppSpec, ProcessRecord, ProcessRuntime, ProcessStatus, SandboxMode, SandboxPolicy};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DumpDocument {
    pub apps: Vec<RecordDto>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RecordDto {
    pub name: String,
    pub script: String,
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
    pub autorestart: bool,
    pub min_uptime_ms: u64,
    pub max_restarts: u32,
    pub restart_delay_ms: u64,
    pub sandbox: SandboxDto,
    pub runtime: RuntimeDto,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SandboxDto {
    pub mode: String,
    pub network: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub writable_roots: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RuntimeDto {
    pub pm_id: u32,
    pub status: String,
    pub restart_time: u32,
    pub unstable_restarts: u32,
    pub created_at_ms: u64,
    pub pid: Option<u32>,
    pub started_at_ms: Option<u64>,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum DecodeError {
    #[error("cannot decode app '{app}': unknown status '{status}'")]
    UnknownStatus { app: String, status: String },

    #[error("cannot decode app '{app}': unknown sandbox mode '{mode}'")]
    UnknownSandboxMode { app: String, mode: String },
}

#[must_use]
pub fn encode_records(records: &[ProcessRecord]) -> DumpDocument {
    DumpDocument {
        apps: records.iter().map(encode_record).collect(),
    }
}

pub fn decode_records(doc: DumpDocument) -> Result<Vec<ProcessRecord>, DecodeError> {
    doc.apps.into_iter().map(decode_record).collect()
}

fn encode_record(record: &ProcessRecord) -> RecordDto {
    let ProcessRecord { spec, runtime } = record;
    let AppSpec {
        name,
        script,
        args,
        cwd,
        env,
        autorestart,
        min_uptime_ms,
        max_restarts,
        restart_delay_ms,
        depends_on,
        sandbox,
    } = spec;
    RecordDto {
        name: name.clone(),
        script: script.clone(),
        cwd: cwd.clone(),
        args: args.clone(),
        env: env.iter().cloned().collect(),
        depends_on: depends_on.clone(),
        autorestart: *autorestart,
        min_uptime_ms: *min_uptime_ms,
        max_restarts: *max_restarts,
        restart_delay_ms: *restart_delay_ms,
        sandbox: encode_sandbox(sandbox),
        runtime: encode_runtime(runtime),
    }
}

fn encode_sandbox(policy: &SandboxPolicy) -> SandboxDto {
    let SandboxPolicy {
        mode,
        network,
        writable_roots,
    } = policy;
    SandboxDto {
        mode: mode.as_str().to_string(),
        network: *network,
        writable_roots: writable_roots.clone(),
    }
}

fn encode_runtime(runtime: &ProcessRuntime) -> RuntimeDto {
    let ProcessRuntime {
        pm_id,
        name: _,
        pid,
        status,
        restart_time,
        unstable_restarts,
        created_at_ms,
        started_at_ms,
        pending_restart: _,
    } = runtime;
    RuntimeDto {
        pm_id: *pm_id,
        status: status.as_str().to_string(),
        restart_time: *restart_time,
        unstable_restarts: *unstable_restarts,
        created_at_ms: *created_at_ms,
        pid: *pid,
        started_at_ms: *started_at_ms,
    }
}

fn decode_record(dto: RecordDto) -> Result<ProcessRecord, DecodeError> {
    let RecordDto {
        name,
        script,
        cwd,
        args,
        env,
        depends_on,
        autorestart,
        min_uptime_ms,
        max_restarts,
        restart_delay_ms,
        sandbox,
        runtime,
    } = dto;
    let policy = decode_sandbox(&name, sandbox)?;
    let runtime = decode_runtime(&name, runtime)?;
    Ok(ProcessRecord {
        spec: AppSpec {
            name,
            script,
            args,
            cwd,
            env: env.into_iter().collect(),
            autorestart,
            min_uptime_ms,
            max_restarts,
            restart_delay_ms,
            depends_on,
            sandbox: policy,
        },
        runtime,
    })
}

fn decode_sandbox(app: &str, dto: SandboxDto) -> Result<SandboxPolicy, DecodeError> {
    let SandboxDto {
        mode,
        network,
        writable_roots,
    } = dto;
    let parsed = SandboxMode::parse(&mode).ok_or_else(|| DecodeError::UnknownSandboxMode {
        app: app.to_string(),
        mode,
    })?;
    Ok(SandboxPolicy {
        mode: parsed,
        network,
        writable_roots,
    })
}

fn decode_runtime(app: &str, dto: RuntimeDto) -> Result<ProcessRuntime, DecodeError> {
    let RuntimeDto {
        pm_id,
        status,
        restart_time,
        unstable_restarts,
        created_at_ms,
        pid,
        started_at_ms,
    } = dto;
    let parsed = ProcessStatus::parse(&status).ok_or_else(|| DecodeError::UnknownStatus {
        app: app.to_string(),
        status,
    })?;
    Ok(ProcessRuntime {
        pm_id,
        name: app.to_string(),
        pid,
        status: parsed,
        restart_time,
        unstable_restarts,
        created_at_ms,
        started_at_ms,
        pending_restart: false,
    })
}

#[cfg(test)]
#[path = "../tests/persistence_dto_tests.rs"]
mod tests;
