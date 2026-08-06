use usecases::{SandboxPolicy, WrappedCommand, covers_path, normalize_root};

const UNSHARE_USER: &str = "--unshare-user";
const UNSHARE_PID: &str = "--unshare-pid";
const UNSHARE_IPC: &str = "--unshare-ipc";
const UNSHARE_UTS: &str = "--unshare-uts";
const UNSHARE_CGROUP_TRY: &str = "--unshare-cgroup-try";
const UNSHARE_NET: &str = "--unshare-net";
const READ_ONLY_BIND: &str = "--ro-bind";
const READ_ONLY_BIND_TRY: &str = "--ro-bind-try";
const WRITABLE_BIND: &str = "--bind";
const TMPFS: &str = "--tmpfs";
const DEVICES: &str = "--dev";
const DEVICES_PATH: &str = "/dev";
const PROCESSES: &str = "--proc";
const PROCESSES_PATH: &str = "/proc";
const FILESYSTEM_ROOT: &str = "/";
const ARGUMENT_TERMINATOR: &str = "--";
const PATH_SEPARATOR: char = '/';

#[must_use]
pub fn bwrap_argv(
    sandbox_program: &str,
    minimal_read_roots: &[String],
    policy: &SandboxPolicy,
    program: &str,
    args: &[String],
) -> WrappedCommand {
    let mut sandbox_args = vec![
        UNSHARE_USER.to_string(),
        UNSHARE_PID.to_string(),
        UNSHARE_IPC.to_string(),
        UNSHARE_UTS.to_string(),
        UNSHARE_CGROUP_TRY.to_string(),
    ];
    if !policy.network {
        sandbox_args.push(UNSHARE_NET.to_string());
    }
    push_read_layer(&mut sandbox_args, minimal_read_roots, policy, program);
    push_pair(&mut sandbox_args, DEVICES, DEVICES_PATH);
    push_pair(&mut sandbox_args, PROCESSES, PROCESSES_PATH);
    for hidden in policy.hidden_paths() {
        push_pair(&mut sandbox_args, TMPFS, normalize_root(hidden));
    }
    let granted = shallowest_first(policy.granted_roots());
    for root in &granted {
        push_bind(&mut sandbox_args, WRITABLE_BIND, normalize_root(root));
    }
    for hidden in nested_in(&policy.hidden_paths(), &granted) {
        push_pair(&mut sandbox_args, TMPFS, hidden);
    }
    sandbox_args.push(ARGUMENT_TERMINATOR.to_string());
    sandbox_args.push(program.to_string());
    sandbox_args.extend_from_slice(args);
    WrappedCommand {
        program: sandbox_program.to_string(),
        args: sandbox_args,
    }
}

fn push_read_layer(
    sandbox_args: &mut Vec<String>,
    minimal_read_roots: &[String],
    policy: &SandboxPolicy,
    program: &str,
) {
    if !policy.read.confines_reads() {
        push_bind(sandbox_args, READ_ONLY_BIND, FILESYSTEM_ROOT);
        return;
    }
    push_pair(sandbox_args, TMPFS, FILESYSTEM_ROOT);
    let declared = minimal_read_roots
        .iter()
        .chain(policy.readable_roots.iter());
    for root in declared {
        push_bind(sandbox_args, READ_ONLY_BIND_TRY, normalize_root(root));
    }
    push_bind(sandbox_args, READ_ONLY_BIND_TRY, program);
}

fn push_pair(sandbox_args: &mut Vec<String>, flag: &str, value: &str) {
    sandbox_args.push(flag.to_string());
    sandbox_args.push(value.to_string());
}

fn push_bind(sandbox_args: &mut Vec<String>, flag: &str, path: &str) {
    sandbox_args.push(flag.to_string());
    sandbox_args.push(path.to_string());
    sandbox_args.push(path.to_string());
}

fn nested_in<'r>(hidden: &[&'r str], granted: &[&str]) -> Vec<&'r str> {
    hidden
        .iter()
        .map(|path| normalize_root(path))
        .filter(|path| granted.iter().any(|root| encloses(root, path)))
        .collect()
}

fn encloses(root: &str, path: &str) -> bool {
    covers_path(root, path) && normalize_root(root) != path
}

fn shallowest_first(roots: Vec<&str>) -> Vec<&str> {
    let mut ordered = roots;
    ordered.sort_by_key(|root| normalize_root(root).matches(PATH_SEPARATOR).count());
    ordered
}

#[cfg(test)]
#[path = "../tests/sandbox_bwrap_tests.rs"]
mod tests;
