pub fn pm3_section(home: &str, kill_timeout_ms: u64, sandbox_mode: &str) -> String {
    format!(
        r#"pm3:
  home: "{home}"
  kill_timeout_ms: {kill_timeout_ms}
  start_timeout_ms: 5000
  drain_timeout_secs: 5
  daemon_poll_interval_ms: 50
  restart:
    min_uptime_ms: 1000
    max_restarts: 15
    restart_delay_ms: 0
  sandbox:
    mode: "{sandbox_mode}"
    network: false
"#
    )
}

pub fn telemetry_section(log_level: &str) -> String {
    format!(
        r#"telemetry:
  service_name: "pm3"
  log_level: "{log_level}"
  log_format: "json"
"#
    )
}
