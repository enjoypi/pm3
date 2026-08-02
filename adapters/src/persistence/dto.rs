use serde::{Deserialize, Serialize};
use thiserror::Error;
use usecases::{ProcessIdentity, ProcessRecord, ProcessRuntime, ProcessStatus, RuntimeError};

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

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<IdentityDto>,

    #[serde(default)]
    pub schedule_armed: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct IdentityDto {
    pub token: String,
    pub launch_digest: String,
    pub binary_digest: String,
}

#[derive(Debug, Eq, PartialEq, Error)]
pub enum DecodeError {
    #[error("cannot decode app '{app}': unknown status '{status}'")]
    UnknownStatus { app: String, status: String },

    #[error("cannot decode app '{app}': {source}")]
    InconsistentState { app: String, source: RuntimeError },
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
        identity,
        schedule_armed,
    } = runtime;
    let parsed = ProcessStatus::parse(&status).ok_or_else(|| DecodeError::UnknownStatus {
        app: name.clone(),
        status,
    })?;
    let decoded = ProcessRuntime {
        pm_id,
        name,
        pid,
        status: parsed,
        restart_time,
        unstable_restarts,
        created_at_ms,
        started_at_ms,
        identity: identity.map(decode_identity),
        pending_restart: false,
        schedule_armed,
    };
    decoded
        .validate_consistency()
        .map_err(|source| DecodeError::InconsistentState {
            app: decoded.name.clone(),
            source,
        })?;
    Ok(decoded)
}

fn decode_identity(dto: IdentityDto) -> ProcessIdentity {
    let IdentityDto {
        token,
        launch_digest,
        binary_digest,
    } = dto;
    ProcessIdentity {
        token,
        launch_digest,
        binary_digest,
    }
}

fn encode_identity(identity: &ProcessIdentity) -> IdentityDto {
    let ProcessIdentity {
        token,
        launch_digest,
        binary_digest,
    } = identity;
    IdentityDto {
        token: token.clone(),
        launch_digest: launch_digest.clone(),
        binary_digest: binary_digest.clone(),
    }
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
        identity,
        pending_restart: _,
        schedule_armed,
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
            identity: identity.as_ref().map(encode_identity),
            schedule_armed: *schedule_armed,
        },
    }
}

#[cfg(test)]
#[path = "../tests/persistence_dto_tests.rs"]
mod tests;
