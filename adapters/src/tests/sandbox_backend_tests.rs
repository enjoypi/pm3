use super::*;

fn programs() -> SandboxProgramSet {
    SandboxProgramSet {
        seatbelt: "/usr/bin/sandbox-exec".to_string(),
        bwrap: "bwrap".to_string(),
    }
}

#[test]
fn seatbelt_runs_the_program_the_config_names() {
    assert_eq!(
        programs().program(SandboxBackend::Seatbelt),
        "/usr/bin/sandbox-exec"
    );
}

#[test]
fn bwrap_runs_the_program_the_config_names() {
    assert_eq!(programs().program(SandboxBackend::Bwrap), "bwrap");
}

#[test]
fn the_program_set_reads_both_backends_from_the_config() {
    let sandbox = crate::config::SandboxConfig {
        mode: "workspace-write".to_string(),
        read: "minimal".to_string(),
        network: false,
        seatbelt_program: "/opt/sandbox-exec".to_string(),
        bwrap_program: "/opt/bwrap".to_string(),
        minimal_read_roots: vec!["/usr".to_string()],
        forbidden_writable_roots: Vec::new(),
    };
    let set = SandboxProgramSet::from_config(&sandbox);
    assert_eq!(set.seatbelt, "/opt/sandbox-exec");
    assert_eq!(set.bwrap, "/opt/bwrap");
}

#[test]
fn a_backend_that_is_not_installed_resolves_to_nothing() {
    assert!(
        SandboxBackend::Bwrap
            .resolve(&programs(), Some("/nonexistent"))
            .is_none()
    );
}

#[test]
fn a_resolved_backend_carries_the_absolute_path_of_its_program() {
    let host = SandboxBackend::Bwrap
        .resolve(&programs(), Some("/bin"))
        .map(|found| found.program);
    assert_eq!(host.is_some(), std::path::Path::new("/bin/bwrap").is_file());
}

#[test]
fn an_installed_backend_resolves_to_the_program_on_the_search_path() {
    let dir = tempfile::tempdir().expect("temp dir");
    let program = crate::platform::script(dir.path(), "fake-bwrap", "", "");
    let name = program
        .file_name()
        .expect("the script has a file name")
        .to_string_lossy()
        .into_owned();
    let set = SandboxProgramSet {
        seatbelt: String::new(),
        bwrap: name,
    };
    let search = dir.path().to_string_lossy().into_owned();
    assert_eq!(
        SandboxBackend::Bwrap.resolve(&set, Some(&search)),
        Some(HostSandbox {
            backend: SandboxBackend::Bwrap,
            program: program.to_string_lossy().into_owned(),
        })
    );
}
