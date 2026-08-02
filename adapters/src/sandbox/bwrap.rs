use usecases::{SandboxPolicy, WrappedCommand};

use super::roots::normalize_root;

#[must_use]
pub fn bwrap_argv(
    sandbox_program: &str,
    policy: &SandboxPolicy,
    program: &str,
    args: &[String],
) -> WrappedCommand {
    let mut sandbox_args = vec!["--unshare-user".to_string(), "--unshare-pid".to_string()];
    if !policy.network {
        sandbox_args.push("--unshare-net".to_string());
    }
    sandbox_args.push("--ro-bind".to_string());
    sandbox_args.push("/".to_string());
    sandbox_args.push("/".to_string());
    sandbox_args.push("--dev".to_string());
    sandbox_args.push("/dev".to_string());
    sandbox_args.push("--proc".to_string());
    sandbox_args.push("/proc".to_string());
    for root in policy.granted_roots() {
        let trimmed = normalize_root(root);
        sandbox_args.push("--bind".to_string());
        sandbox_args.push(trimmed.to_string());
        sandbox_args.push(trimmed.to_string());
    }
    sandbox_args.push("--".to_string());
    sandbox_args.push(program.to_string());
    sandbox_args.extend_from_slice(args);
    WrappedCommand {
        program: sandbox_program.to_string(),
        args: sandbox_args,
    }
}

#[cfg(test)]
#[path = "../tests/sandbox_bwrap_tests.rs"]
mod tests;
