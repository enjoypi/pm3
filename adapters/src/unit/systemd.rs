use super::spec::UnitSpec;

#[must_use]
pub fn render_unit(spec: &UnitSpec) -> String {
    let label = escape_value(&spec.label);
    let exec_start = render_exec_start(spec);
    let working_directory = escape_value(&spec.working_directory.to_string_lossy());
    let log_path = escape_value(&spec.log_path.to_string_lossy());
    let restart_delay_secs = spec.restart_delay_secs;
    let restart = escape_value(&spec.restart_condition);
    let environment = render_environment(&spec.environment_pairs());
    let umask = format!("{:04o}", spec.umask);
    let max_tasks = spec.max_tasks;
    let cpu_quota = render_cpu_quota(spec.cpu_quota_percent);
    let network_wait = render_network_wait(spec.wait_for_network);
    format!(
        "[Unit]
Description={label}
After=default.target
{network_wait}
[Service]
Type=simple
ExecStart={exec_start}
WorkingDirectory={working_directory}
Restart={restart}
RestartSec={restart_delay_secs}
KillMode=process
UMask={umask}
LimitCORE=0
TasksMax={max_tasks}
{cpu_quota}{environment}StandardOutput=append:{log_path}
StandardError=append:{log_path}

[Install]
WantedBy=default.target
"
    )
}

fn render_cpu_quota(percent: u64) -> String {
    if percent == 0 {
        return String::new();
    }
    format!("CPUQuota={percent}%\n")
}

fn render_environment(vars: &[(&str, &str)]) -> String {
    vars.iter().map(render_variable).collect()
}

fn render_variable(entry: &(&str, &str)) -> String {
    let (name, value) = entry;
    let key = escape_value(name);
    let text = escape_value(value);
    format!("Environment=\"{key}={text}\"\n")
}

fn render_exec_start(spec: &UnitSpec) -> String {
    let mut tokens = Vec::with_capacity(4);
    tokens.push(quote_token(&spec.program.to_string_lossy()));
    for argument in spec.daemon_args() {
        tokens.push(quote_token(&argument));
    }
    tokens.join(" ")
}

fn quote_token(raw: &str) -> String {
    let escaped = escape_value(raw);
    format!("\"{escaped}\"")
}

fn escape_value(raw: &str) -> String {
    let mut escaped = String::with_capacity(raw.len());
    for character in raw.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '%' => escaped.push_str("%%"),
            other => escaped.push(other),
        }
    }
    escaped
}

const fn render_network_wait(wait_for_network: bool) -> &'static str {
    if wait_for_network {
        "Wants=network-online.target\nAfter=network-online.target\n"
    } else {
        ""
    }
}

#[cfg(test)]
#[path = "../tests/unit_systemd_tests.rs"]
mod tests;
