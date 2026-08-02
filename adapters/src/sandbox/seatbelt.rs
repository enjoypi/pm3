use usecases::{SandboxPolicy, WrappedCommand};

use super::roots::normalize_root;

const BASE_POLICY: &str = include_str!("seatbelt_base_policy.sbpl");
const NETWORK_POLICY: &str = include_str!("seatbelt_network_policy.sbpl");

#[must_use]
pub fn seatbelt_argv(
    sandbox_program: &str,
    policy: &SandboxPolicy,
    program: &str,
    args: &[String],
) -> WrappedCommand {
    let mut sandbox_args = vec!["-p".to_string(), seatbelt_profile(policy), "--".to_string()];
    sandbox_args.push(program.to_string());
    sandbox_args.extend_from_slice(args);
    WrappedCommand {
        program: sandbox_program.to_string(),
        args: sandbox_args,
    }
}

#[must_use]
pub fn seatbelt_profile(policy: &SandboxPolicy) -> String {
    let mut profile = String::with_capacity(BASE_POLICY.len() + NETWORK_POLICY.len());
    profile.push_str(BASE_POLICY);
    if policy.network {
        profile.push('\n');
        profile.push_str(NETWORK_POLICY);
    }
    for root in policy.granted_roots() {
        profile.push_str(&writable_root_rule(root));
    }
    profile
}

fn writable_root_rule(root: &str) -> String {
    let escaped = normalize_root(root)
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    format!("\n(allow file-write* (subpath \"{escaped}\"))\n")
}

#[cfg(test)]
#[path = "../tests/sandbox_seatbelt_tests.rs"]
mod tests;
