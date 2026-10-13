use std::{ffi::OsString, os::unix::fs::PermissionsExt as _, path::PathBuf};

use clap::Parser as _;

use super::*;

fn launch(program: &str, args: &[&str]) -> LaunchSpec {
    LaunchSpec {
        name: "sshd".to_string(),
        program: program.to_string(),
        args: args.iter().map(|arg| (*arg).to_string()).collect(),
        cwd: "/".to_string(),
        env: vec![("PATH".to_string(), "/usr/bin".to_string())],
        stdout_path: "/logs/sshd-out.log".to_string(),
        stderr_path: "/logs/sshd-err.log".to_string(),
        self_responsible: true,
    }
}

fn unloadable(dir: &Path) -> PathBuf {
    let garbage = dir.join("garbage");
    std::fs::write(&garbage, "not a binary").expect("write");
    std::fs::set_permissions(&garbage, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    garbage
}

#[test]
fn the_trampoline_runs_pm3_with_the_program_after_a_separator() {
    let spec = launch("/usr/sbin/sshd", &["-D", "-e"]);
    let trampolined = through_trampoline(Path::new("/opt/pm3"), &spec);
    assert_eq!(trampolined.program, "/opt/pm3");
    assert_eq!(
        trampolined.args,
        ["__disclaim", "--", "/usr/sbin/sshd", "-D", "-e"]
    );
}

#[test]
fn the_trampoline_keeps_everything_but_the_command_line() {
    let spec = launch("/usr/sbin/sshd", &[]);
    let trampolined = through_trampoline(Path::new("/opt/pm3"), &spec);
    assert_eq!(trampolined.env, spec.env);
    assert_eq!(trampolined.cwd, spec.cwd);
    assert_eq!(trampolined.stderr_path, spec.stderr_path);
}

#[test]
fn a_program_the_kernel_cannot_load_returns_instead_of_replacing_the_process() {
    let dir = tempfile::tempdir().expect("temp dir");
    let error = exec_disclaimed(unloadable(dir.path()).as_os_str(), &[OsString::from("-x")]);
    assert_eq!(error.raw_os_error(), Some(libc::ENOEXEC));
}

#[tokio::test]
async fn the_hidden_subcommand_reports_a_program_it_cannot_become() {
    let dir = tempfile::tempdir().expect("temp dir");
    let garbage = unloadable(dir.path());
    let cli = crate::cli::Cli::parse_from([
        OsString::from("pm3"),
        OsString::from(SUBCOMMAND),
        OsString::from(SEPARATOR),
        garbage.clone().into_os_string(),
        OsString::from("-D"),
    ]);
    let error = crate::cli::dispatch(cli).await.unwrap_err();
    assert!(
        matches!(&error, crate::Error::Disclaim { program, .. } if *program == garbage),
        "got: {error}"
    );
}
