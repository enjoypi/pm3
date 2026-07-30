pub fn apps_section(name: &str, script: &str, cwd: &str) -> String {
    format!(
        r#"apps:
  - name: "{name}"
    script: "{script}"
    cwd: "{cwd}"
"#
    )
}

pub fn every_optional_field_section() -> String {
    r#"    args:
      - server.js
      - --port=8080
    env:
      PORT: "8080"
      RUST_LOG: debug
    depends_on:
      - db
    autorestart: false
    min_uptime_ms: 250
    max_restarts: 3
    restart_delay_ms: 40
    sandbox:
      mode: "read-only"
      network: true
      writable_roots: []
"#
    .to_string()
}
