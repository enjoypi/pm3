use std::{ffi::OsString, os::unix::fs::PermissionsExt as _, path::PathBuf};

use clap::Parser as _;

use super::*;

fn launch(program: &str, args: &[&str], self_responsible: bool, priority: Priority) -> LaunchSpec {
    LaunchSpec {
        name: "sshd".to_string(),
        program: program.to_string(),
        args: args.iter().map(|arg| (*arg).to_string()).collect(),
        cwd: "/".to_string(),
        env: vec![("PATH".to_string(), "/usr/bin".to_string())],
        stdout_path: "/logs/sshd-out.log".to_string(),
        stderr_path: "/logs/sshd-err.log".to_string(),
        self_responsible,
        priority,
    }
}

fn unloadable(dir: &Path) -> PathBuf {
    let garbage = dir.join("garbage");
    std::fs::write(&garbage, "not a binary").expect("write");
    std::fs::set_permissions(&garbage, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    garbage
}

fn parsed_traits(argv: &[&str]) -> SpawnTraits {
    let cli = crate::cli::Cli::parse_from(std::iter::once("pm3").chain(argv.iter().copied()));
    let crate::cli::Commands::Disclaim {
        disclaim,
        process_type,
        qos_clamp,
        ..
    } = cli.command
    else {
        panic!("expected the trampoline, got {:?}", cli.command);
    };
    SpawnTraits {
        disclaim,
        process_type,
        qos_clamp,
    }
}

#[test]
fn the_trampoline_spells_out_every_spawn_attribute_before_the_program() {
    let spec = launch("/bin/sh", &["-D", "-e"], true, Priority::Interactive);
    let trampolined = wrap(Path::new("/opt/pm3"), &spec).expect("a launchable program");
    assert_eq!(trampolined.program, "/opt/pm3");
    assert_eq!(
        trampolined.args,
        [
            "__disclaim",
            "--disclaim",
            "1",
            "--process-type",
            "1024",
            "--qos-clamp",
            "0",
            "--",
            "/bin/sh",
            "-D",
            "-e"
        ]
    );
}

#[test]
fn the_trampoline_keeps_everything_but_the_command_line() {
    let spec = launch("/bin/sh", &[], false, Priority::Background);
    let trampolined = wrap(Path::new("/opt/pm3"), &spec).expect("a launchable program");
    assert_eq!(trampolined.env, spec.env);
    assert_eq!(trampolined.cwd, spec.cwd);
    assert_eq!(trampolined.stderr_path, spec.stderr_path);
}

#[test]
fn the_trampoline_refuses_a_program_it_could_never_become() {
    let dir = tempfile::tempdir().expect("temp dir");
    let unrunnable = dir.path().join("not-executable");
    std::fs::write(&unrunnable, "").expect("write");
    let spec = launch(
        &unrunnable.to_string_lossy(),
        &[],
        false,
        Priority::Background,
    );
    let error = wrap(Path::new("/opt/pm3"), &spec).unwrap_err();
    assert!(
        error.to_string().contains("is not an executable file"),
        "got: {error}"
    );
}

#[test]
fn each_priority_maps_to_its_launchd_process_type() {
    assert_eq!(
        SpawnTraits::of(false, Priority::Background),
        SpawnTraits {
            disclaim: 0,
            process_type: 0x500,
            qos_clamp: 0
        }
    );
    assert_eq!(
        SpawnTraits::of(false, Priority::Standard),
        SpawnTraits {
            disclaim: 0,
            process_type: 0x300,
            qos_clamp: 1
        }
    );
    assert_eq!(
        SpawnTraits::of(true, Priority::Interactive),
        SpawnTraits {
            disclaim: 1,
            process_type: 0x400,
            qos_clamp: 0
        }
    );
}

#[test]
fn the_trampoline_reads_back_the_attributes_it_was_given() {
    let spec = launch("/bin/sh", &["-D"], false, Priority::Standard);
    let trampolined = wrap(Path::new("/opt/pm3"), &spec).expect("a launchable program");
    let argv: Vec<&str> = trampolined.args.iter().map(String::as_str).collect();
    assert_eq!(
        parsed_traits(&argv),
        SpawnTraits::of(false, Priority::Standard)
    );
}

#[test]
fn a_bare_trampoline_still_disclaims_and_inherits_the_rest() {
    assert_eq!(
        parsed_traits(&[SUBCOMMAND, SEPARATOR, "/usr/sbin/sshd"]),
        SpawnTraits {
            disclaim: DISCLAIM,
            process_type: INHERIT,
            qos_clamp: NO_CLAMP
        }
    );
}

#[test]
fn a_program_the_kernel_cannot_load_returns_instead_of_replacing_the_process() {
    let dir = tempfile::tempdir().expect("temp dir");
    let error = exec_disclaimed(
        unloadable(dir.path()).as_os_str(),
        &[OsString::from("-x")],
        SpawnTraits::of(true, Priority::Standard),
    );
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
