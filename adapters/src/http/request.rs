use thiserror::Error;
use usecases::{AppSelector, SpecError, validate_app_name};

use super::{
    dto::{ReplyDto, SignalRequestDto, StartRequestDto},
    routes::APPS_PATH,
};

#[derive(Debug, Error)]
#[error("cannot decode the pm3 daemon reply: {reason}")]
pub struct ReplyDecodeError {
    pub reason: String,
}

#[must_use]
pub fn encode_start_request(services: &[String]) -> String {
    let request = StartRequestDto {
        services: services.to_vec(),
    };
    serde_json::to_string(&request)
        .expect("internal error: StartRequestDto serialization is infallible")
}

#[must_use]
pub fn encode_signal_request(signal: &str) -> String {
    let request = SignalRequestDto {
        signal: signal.to_string(),
    };
    serde_json::to_string(&request)
        .expect("internal error: SignalRequestDto serialization is infallible")
}

pub fn decode_reply(body: &str) -> Result<ReplyDto, ReplyDecodeError> {
    serde_json::from_str(body).map_err(|error| ReplyDecodeError {
        reason: error.to_string(),
    })
}

pub fn app_path(selector: &str) -> Result<String, SpecError> {
    let addressed = addressable(selector)?;
    Ok(format!("{APPS_PATH}/{addressed}"))
}

pub fn app_action_path(selector: &str, action: &str) -> Result<String, SpecError> {
    let addressed = addressable(selector)?;
    Ok(format!("{APPS_PATH}/{addressed}/{action}"))
}

fn addressable(selector: &str) -> Result<&str, SpecError> {
    match AppSelector::parse(selector) {
        AppSelector::Id(_) => Ok(selector),
        AppSelector::Name(name) => validate_app_name(&name).map(|()| selector),
    }
}

#[cfg(test)]
#[path = "../tests/http_request_tests.rs"]
mod tests;
