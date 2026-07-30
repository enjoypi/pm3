use tokio::process::Command;
use usecases::ProcessProbe;

pub const PS_PROGRAM: &str = "/bin/ps";

const WIDE_FLAG: &str = "-ww";
const FORMAT_FLAG: &str = "-o";
const START_TIME_FORMAT: &str = "lstart=";
const PID_FLAG: &str = "-p";
const LOCALE_VARIABLE: &str = "LC_ALL";
const FIXED_LOCALE: &str = "C";

#[derive(Clone, Debug)]
pub struct PsProcessProbe {
    program: String,
}

impl PsProcessProbe {
    #[must_use]
    pub const fn with_program(program: String) -> Self {
        Self { program }
    }
}

impl Default for PsProcessProbe {
    fn default() -> Self {
        Self {
            program: PS_PROGRAM.to_string(),
        }
    }
}

impl ProcessProbe for PsProcessProbe {
    async fn identity(&self, pid: u32) -> Option<String> {
        let output = Command::new(&self.program)
            .args([WIDE_FLAG, FORMAT_FLAG, START_TIME_FORMAT, PID_FLAG])
            .arg(pid.to_string())
            .env(LOCALE_VARIABLE, FIXED_LOCALE)
            .output()
            .await
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if token.is_empty() {
            return None;
        }
        tracing::debug!(pid, token, action = "probe", "probed a managed process");
        Some(token)
    }
}

#[cfg(test)]
#[path = "../tests/process_ps_probe_tests.rs"]
mod tests;
