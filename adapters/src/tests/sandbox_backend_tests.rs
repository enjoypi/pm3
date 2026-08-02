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
        network: false,
        seatbelt_program: "/opt/sandbox-exec".to_string(),
        bwrap_program: "/opt/bwrap".to_string(),
    };
    let set = SandboxProgramSet::from_config(&sandbox);
    assert_eq!(set.seatbelt, "/opt/sandbox-exec");
    assert_eq!(set.bwrap, "/opt/bwrap");
}

#[test]
fn each_backend_has_a_log_friendly_name() {
    assert_eq!(SandboxBackend::Seatbelt.as_str(), "seatbelt");
    assert_eq!(SandboxBackend::Bwrap.as_str(), "bwrap");
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
