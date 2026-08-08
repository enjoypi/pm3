use entities::AppSpec;

use crate::ports::Liveness;

const NAME_LABEL: &str = "name";
const PROGRAM_LABEL: &str = "program";
const CWD_LABEL: &str = "cwd";
const ARG_LABEL: &str = "arg";
const ENV_LABEL: &str = "env";
const SANDBOX_LABEL: &str = "sandbox";
const NETWORK_LABEL: &str = "network";
const ROOT_LABEL: &str = "root";
const NETWORK_ALLOWED: &str = "allowed";
const NETWORK_DENIED: &str = "denied";

#[must_use]
pub fn render_identity(spec: &AppSpec) -> String {
    let AppSpec {
        name,
        script,
        args,
        cwd,
        env,
        autorestart: _,
        min_uptime_ms: _,
        max_restarts: _,
        restart_delay_ms: _,
        max_restart_delay_ms: _,
        schedule: _,
        depends_on: _,
        max_memory_kib: _,
        ready_probe: _,
        listen_timeout_ms: _,
        stop_exit_codes: _,
        sandbox,
    } = spec;

    let head = [
        field(NAME_LABEL, name),
        field(PROGRAM_LABEL, script),
        field(CWD_LABEL, cwd),
        field(SANDBOX_LABEL, sandbox.mode.as_str()),
        field(NETWORK_LABEL, network_label(sandbox.network)),
    ]
    .concat();
    let with_args = args.iter().fold(head, |mut text, arg| {
        text.push_str(&field(ARG_LABEL, arg));
        text
    });
    let with_roots = sandbox
        .writable_roots
        .iter()
        .fold(with_args, |mut text, root| {
            text.push_str(&field(ROOT_LABEL, root));
            text
        });
    sorted_env(env).iter().fold(with_roots, |mut text, entry| {
        text.push_str(&field(ENV_LABEL, &entry_line(entry)));
        text
    })
}

#[must_use]
pub fn pid_was_recycled(observed: &Liveness, expected: Option<&str>) -> bool {
    let (Liveness::Alive(current), Some(expected)) = (observed, expected) else {
        return false;
    };
    current != expected
}

const fn network_label(allowed: bool) -> &'static str {
    if allowed {
        NETWORK_ALLOWED
    } else {
        NETWORK_DENIED
    }
}

fn sorted_env(env: &[(String, String)]) -> Vec<&(String, String)> {
    let mut entries: Vec<&(String, String)> = env.iter().collect();
    entries.sort_by(|(left_key, left_value), (right_key, right_value)| {
        left_key
            .cmp(right_key)
            .then_with(|| left_value.cmp(right_value))
    });
    entries
}

fn entry_line(entry: &(String, String)) -> String {
    let (key, value) = entry;
    format!("{key}={value}")
}

fn field(label: &str, value: &str) -> String {
    let length = value.len();
    format!("{label} {length} {value}\n")
}

#[cfg(test)]
#[path = "tests/fingerprint_tests.rs"]
mod tests;
