pub fn server_section(host: &str, port: u16, drain_timeout_secs: u64) -> String {
    format!(
        r#"server:
  host: "{host}"
  port: {port}
  drain_timeout_secs: {drain_timeout_secs}
"#
    )
}

pub fn telemetry_section(log_level: &str) -> String {
    format!(
        r#"telemetry:
  service_name: "skel_rs"
  log_level: "{log_level}"
  log_format: "json"
"#
    )
}

pub fn health_check_section(host: &str, connect_timeout_secs: u64) -> String {
    format!(
        r#"health_check:
  host: "{host}"
  connect_timeout_secs: {connect_timeout_secs}
"#
    )
}

pub fn database_section(url: &str, migrations_path: &str, max_connections: u32) -> String {
    format!(
        r#"database:
  url: "{url}"
  migrations_path: "{migrations_path}"
  pool:
    max_connections: {max_connections}
    min_connections: 1
    acquire_timeout_secs: 5
    idle_timeout_secs: 300
    max_lifetime_secs: 1800
    health_check_timeout_secs: 3
"#
    )
}
