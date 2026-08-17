use std::fmt::Write as _;

use usecases::{SandboxPolicy, WrappedCommand, covers_path, normalize_root};

const BASE_POLICY: &str = include_str!("seatbelt_base_policy.sbpl");
const FULL_READ_POLICY: &str = include_str!("seatbelt_full_read_policy.sbpl");
const MINIMAL_READ_POLICY: &str = include_str!("seatbelt_minimal_read_policy.sbpl");
const NETWORK_POLICY: &str = include_str!("seatbelt_network_policy.sbpl");

const PARAMETER_FLAG: &str = "-D";
const PROFILE_FLAG: &str = "-p";
const ARGUMENT_TERMINATOR: &str = "--";
const READABLE_PARAMETER: &str = "READABLE";
const WRITABLE_PARAMETER: &str = "WRITABLE";
const HIDDEN_PARAMETER: &str = "HIDDEN";
const READ_ACTION: &str = "allow file-read* file-test-existence";
const ANCESTOR_ACTION: &str = "allow file-read-metadata";
const WRITE_ACTION: &str = "allow file-read* file-test-existence file-write*";
const FILESYSTEM_ROOT: &str = "/";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeatbeltProfile {
    pub profile: String,
    pub parameters: Vec<(String, String)>,
}

#[must_use]
pub fn seatbelt_argv(
    sandbox_program: &str,
    policy: &SandboxPolicy,
    program: &str,
    args: &[String],
) -> WrappedCommand {
    let rendered = seatbelt_profile(policy, program);
    let mut sandbox_args = Vec::new();
    for (name, value) in &rendered.parameters {
        sandbox_args.push(PARAMETER_FLAG.to_string());
        sandbox_args.push(format!("{name}={value}"));
    }
    sandbox_args.push(PROFILE_FLAG.to_string());
    sandbox_args.push(rendered.profile);
    sandbox_args.push(ARGUMENT_TERMINATOR.to_string());
    sandbox_args.push(program.to_string());
    sandbox_args.extend_from_slice(args);
    WrappedCommand {
        program: sandbox_program.to_string(),
        args: sandbox_args,
    }
}

#[must_use]
pub fn seatbelt_profile(policy: &SandboxPolicy, program: &str) -> SeatbeltProfile {
    let readable = readable_roots_of(policy, program);
    let writable = policy.granted_roots();
    let hidden = policy.hidden_paths();
    let mut profile = String::with_capacity(BASE_POLICY.len() + MINIMAL_READ_POLICY.len());
    profile.push_str(BASE_POLICY);
    profile.push_str(&read_policy_of(policy, &hidden));
    profile.push_str(&rules(READ_ACTION, READABLE_PARAMETER, &readable, &hidden));
    profile.push_str(&rules(WRITE_ACTION, WRITABLE_PARAMETER, &writable, &hidden));
    if policy.network {
        profile.push_str(NETWORK_POLICY);
    }
    let parameters = named(READABLE_PARAMETER, &readable)
        .chain(named(WRITABLE_PARAMETER, &writable))
        .chain(named(HIDDEN_PARAMETER, &hidden))
        .collect();
    SeatbeltProfile {
        profile,
        parameters,
    }
}

fn readable_roots_of<'p>(policy: &'p SandboxPolicy, program: &'p str) -> Vec<&'p str> {
    if !policy.read.confines_reads() {
        return Vec::new();
    }
    policy
        .readable_roots
        .iter()
        .map(String::as_str)
        .chain([program])
        .collect()
}

fn read_policy_of(policy: &SandboxPolicy, hidden: &[&str]) -> String {
    if policy.read.confines_reads() {
        return MINIMAL_READ_POLICY.to_string();
    }
    let carveout = carveout_for(FILESYSTEM_ROOT, hidden);
    if carveout.is_empty() {
        return FULL_READ_POLICY.to_string();
    }
    format!("\n({READ_ACTION} (require-all (subpath \"{FILESYSTEM_ROOT}\"){carveout}))\n")
}

fn carveout_for(granted: &str, hidden: &[&str]) -> String {
    hidden
        .iter()
        .enumerate()
        .fold(String::new(), |mut text, (index, root)| {
            if covers_path(granted, root) {
                let _ = write!(
                    text,
                    " (require-not (subpath (param \"{HIDDEN_PARAMETER}_{index}\")))"
                );
            }
            text
        })
}

fn rules(action: &str, parameter: &str, granted: &[&str], hidden: &[&str]) -> String {
    granted
        .iter()
        .enumerate()
        .fold(String::new(), |mut text, (index, root)| {
            let carveout = carveout_for(root, hidden);
            let _ = writeln!(
                text,
                "\n({action} (require-all (subpath (param \"{parameter}_{index}\")){carveout}))"
            );
            let _ = writeln!(
                text,
                "({ANCESTOR_ACTION} (path-ancestors (param \"{parameter}_{index}\")))"
            );
            text
        })
}

fn named<'r>(
    parameter: &'r str,
    roots: &'r [&'r str],
) -> impl Iterator<Item = (String, String)> + 'r {
    roots.iter().enumerate().map(move |(index, root)| {
        (
            format!("{parameter}_{index}"),
            normalize_root(root).to_string(),
        )
    })
}

#[cfg(test)]
#[path = "../tests/sandbox_seatbelt_tests.rs"]
mod tests;
