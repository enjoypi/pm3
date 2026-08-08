use super::spec::SpecError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReadyProbe {
    Exec { command: Vec<String> },
    Tcp { host: String, port: u16 },
}

pub fn validate_probe(app: &str, probe: &ReadyProbe) -> Result<(), SpecError> {
    match probe {
        ReadyProbe::Exec { command } => {
            if command
                .first()
                .is_none_or(|program| program.trim().is_empty())
            {
                return Err(SpecError::EmptyReadyProbe(app.to_string()));
            }
        }
        ReadyProbe::Tcp { host, port } => {
            if host.trim().is_empty() || *port == 0 {
                return Err(SpecError::InvalidReadyEndpoint {
                    app: app.to_string(),
                    endpoint: format!("{host}:{port}"),
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "../tests/process_ready_tests.rs"]
mod tests;
