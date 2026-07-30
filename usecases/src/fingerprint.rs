use crate::ports::LaunchSpec;

const NAME_LABEL: &str = "name";
const PROGRAM_LABEL: &str = "program";
const CWD_LABEL: &str = "cwd";
const ARG_LABEL: &str = "arg";
const ENV_LABEL: &str = "env";

#[must_use]
pub fn render_launch(launch: &LaunchSpec) -> String {
    let LaunchSpec {
        name,
        program,
        args,
        cwd,
        env,
        stdout_path: _,
        stderr_path: _,
    } = launch;

    let head = [
        field(NAME_LABEL, name),
        field(PROGRAM_LABEL, program),
        field(CWD_LABEL, cwd),
    ]
    .concat();
    let with_args = args.iter().fold(head, |mut text, arg| {
        text.push_str(&field(ARG_LABEL, arg));
        text
    });
    sorted_env(env).iter().fold(with_args, |mut text, entry| {
        text.push_str(&field(ENV_LABEL, &entry_line(entry)));
        text
    })
}

fn sorted_env(env: &[(String, String)]) -> Vec<&(String, String)> {
    let mut entries: Vec<&(String, String)> = env.iter().collect();
    entries.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
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
