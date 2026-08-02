use super::spec::UnitSpec;

const PATH_VARIABLE: &str = "PATH";
const HOME_VARIABLE: &str = "HOME";

#[must_use]
pub fn render_unit(spec: &UnitSpec) -> String {
    let label = escape_value(&spec.label);
    let exec_start = render_exec_start(spec);
    let working_directory = escape_value(&spec.working_directory.to_string_lossy());
    let log_path = escape_value(&spec.log_path.to_string_lossy());
    let search_path = escape_value(&spec.search_path);
    let home = escape_value(&spec.home);
    let restart_delay_secs = spec.restart_delay_secs;
    let restart = escape_value(&spec.restart_condition);
    format!(
        "[Unit]
Description={label}
After=default.target

[Service]
Type=simple
ExecStart={exec_start}
WorkingDirectory={working_directory}
Restart={restart}
RestartSec={restart_delay_secs}
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

fn render_exec_start(spec: &UnitSpec) -> String {
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

#[cfg(test)]
#[path = "../tests/unit_systemd_tests.rs"]
mod tests;
