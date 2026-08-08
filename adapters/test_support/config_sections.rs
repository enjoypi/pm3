pub fn pm3_section(home: &str, kill_timeout_ms: u64, sandbox_mode: &str) -> String {
    format!(
        r#"pm3:
  home: "{home}"
  cfg_dir: "{home}/service"
  search_path: "/usr/bin:/bin"
  stop_signal: "TERM"
  kill_timeout_ms: {kill_timeout_ms}
  start_timeout_ms: 5000
  drain_timeout_secs: 5
  request_timeout_ms: 30000
  command_timeout_ms: 5000
  daemon_poll_interval_ms: 50
  daemon_poll_max_interval_ms: 1000
  memory_poll_interval_ms: 30000
  log_follow_interval_ms: 200
  log_tail_lines: 20
  log_rotate_max_bytes: 0
  log_rotate_interval_ms: 60000
  ready_timeout_ms: 30000
  ready_poll_interval_ms: 200
  daemon_channel_depth: 32
  request_body_limit_bytes: 131072
  restart:
    autorestart: true
    min_uptime_ms: 1000
    max_restarts: 15
    restart_delay_ms: 0
    max_restart_delay_ms: 15000
  sandbox:
    mode: "{sandbox_mode}"
    read: "minimal"
    network: false
    seatbelt_program: "/usr/bin/sandbox-exec"
    bwrap_program: "bwrap"
    minimal_read_roots:
      - "/usr"
      - "/bin"
    forbidden_writable_roots:
      - "/"
      - "/etc"
  service:
    label: "pm3-fixture"
    restart_delay_secs: 2
    restart_condition: "always"
    max_tasks: 4096
    cpu_quota_percent: 0
    launchctl_path: "/bin/launchctl"
    systemctl_path: "/usr/bin/systemctl"
    loginctl_path: "/usr/bin/loginctl"
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
