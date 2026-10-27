use usecases::{Alert, AlertKind, ExitOutcome};

use crate::config::NotifyConfig;

#[must_use]
#[expect(
    clippy::literal_string_with_formatting_args,
    reason = "配置文案的占位符由 str::replace 填充，不是格式化参数"
)]
pub fn render_alert(config: &NotifyConfig, alert: &Alert) -> (String, String) {
    let title = config.title.replace("{name}", &alert.name);
    let messages = &config.messages;
    let body = match &alert.kind {
        AlertKind::Exited(ExitOutcome::Code(code)) => {
            messages.exited_code.replace("{code}", &code.to_string())
        }
        AlertKind::Exited(ExitOutcome::Signalled | ExitOutcome::Unobserved) => {
            messages.exited_signal.clone()
        }
        AlertKind::ReadyFailed { downstream } if downstream.is_empty() => {
            messages.ready_failed.clone()
        }
        AlertKind::ReadyFailed { downstream } => messages
            .ready_failed_downstream
            .replace("{downstream}", &downstream.join(", ")),
        AlertKind::LivenessKilled => messages.liveness_killed.clone(),
        AlertKind::MemoryKilled => messages.memory_killed.clone(),
        AlertKind::LaunchFailed => messages.launch_failed.clone(),
    };
    (title, body)
}

#[cfg(test)]
#[path = "../tests/notify_message_tests.rs"]
mod tests;
