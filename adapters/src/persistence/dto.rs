use serde::{Deserialize, Serialize};
use thiserror::Error;
use usecases::{ProcessRecord, ProcessRuntime, ProcessStatus};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DumpDocument {
    pub services: Vec<StateDto>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StateDto {
    pub name: String,
    pub runtime: RuntimeDto,
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
}

#[must_use]
pub fn encode_states(records: &[ProcessRecord]) -> DumpDocument {
    DumpDocument {
        services: records.iter().map(encode_state).collect(),
    }
}

pub fn decode_state(dto: StateDto) -> Result<ProcessRuntime, DecodeError> {
    let StateDto { name, runtime } = dto;
    let RuntimeDto {
        pm_id,
        status,
        restart_time,
        unstable_restarts,
        created_at_ms,
        pid,
        started_at_ms,
    } = runtime;
    let parsed = ProcessStatus::parse(&status).ok_or_else(|| DecodeError::UnknownStatus {
        app: name.clone(),
        status,
    })?;
    Ok(ProcessRuntime {
        pm_id,
        name,
        pid,
        status: parsed,
        restart_time,
        unstable_restarts,
        created_at_ms,
        started_at_ms,
        pending_restart: false,
    })
}

fn encode_state(record: &ProcessRecord) -> StateDto {
    let ProcessRecord { spec: _, runtime } = record;
    let ProcessRuntime {
        pm_id,
        name,
        pid,
        status,
        restart_time,
        unstable_restarts,
        created_at_ms,
        started_at_ms,
        pending_restart: _,
    } = runtime;
    StateDto {
        name: name.clone(),
        runtime: RuntimeDto {
            pm_id: *pm_id,
            status: status.as_str().to_string(),
            restart_time: *restart_time,
            unstable_restarts: *unstable_restarts,
            created_at_ms: *created_at_ms,
            pid: *pid,
            started_at_ms: *started_at_ms,
        },
    }
}

#[cfg(test)]
#[path = "../tests/persistence_dto_tests.rs"]
mod tests;
