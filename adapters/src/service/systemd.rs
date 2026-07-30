use super::spec::ServiceUnitSpec;

const RESTART_DELAY_SECS: u64 = 2;
const PATH_VARIABLE: &str = "PATH";
const HOME_VARIABLE: &str = "HOME";

#[must_use]
pub fn render_unit(spec: &ServiceUnitSpec) -> String {
    let label = escape_value(&spec.label);
    let exec_start = render_exec_start(spec);
    let working_directory = escape_value(&spec.working_directory.to_string_lossy());
    let log_path = escape_value(&spec.log_path.to_string_lossy());
    let search_path = escape_value(&spec.search_path);
    let home = escape_value(&spec.home);
    format!(
        "[Unit]
Description={label}
After=default.target

[Service]
Type=simple
ExecStart={exec_start}
WorkingDirectory={working_directory}
Restart=on-failure
RestartSec={RESTART_DELAY_SECS}
KillMode=process
Environment=\"{HOME_VARIABLE}={home}\"
Environment=\"{PATH_VARIABLE}={search_path}\"
StandardOutput=append:{log_path}
StandardError=append:{log_path}

[Install]
WantedBy=default.target
"
    )
}

fn render_exec_start(spec: &ServiceUnitSpec) -> String {
    let mut tokens = Vec::with_capacity(4);
    tokens.push(quote_token(&spec.program.to_string_lossy()));
    for argument in spec.daemon_args() {
        tokens.push(quote_token(&argument));
    }
    tokens.join(" ")
}

fn quote_token(raw: &str) -> String {
    let mut quoted = String::with_capacity(raw.len() + 2);
    quoted.push('"');
    for character in raw.chars() {
        match character {
            '\\' => quoted.push_str("\\\\"),
            '"' => quoted.push_str("\\\""),
            '%' => quoted.push_str("%%"),
            other => quoted.push(other),
        }
    }
    quoted.push('"');
    quoted
}

fn escape_value(raw: &str) -> String {
    raw.replace('%', "%%")
}

#[cfg(test)]
#[path = "../tests/service_systemd_tests.rs"]
mod tests;
