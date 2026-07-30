use entities::{SandboxMode, SandboxPolicy};

use super::*;

fn spec() -> AppSpec {
    AppSpec {
        name: "api".to_string(),
        script: "/usr/bin/node".to_string(),
        args: vec!["server.js".to_string(), "--port=8080".to_string()],
        cwd: "/srv/api".to_string(),
        env: vec![
            ("PORT".to_string(), "8080".to_string()),
            ("HOME".to_string(), "/srv/api".to_string()),
        ],
        autorestart: true,
        min_uptime_ms: 1000,
        max_restarts: 15,
        restart_delay_ms: 0,
        depends_on: Vec::new(),
        sandbox: SandboxPolicy {
            mode: SandboxMode::WorkspaceWrite,
            network: false,
            writable_roots: vec!["/srv/api/state".to_string()],
            derived_roots: vec!["/srv/pm3/logs".to_string()],
        },
    }
}

fn with_sandbox(policy: SandboxPolicy) -> AppSpec {
    AppSpec {
        sandbox: policy,
        ..spec()
    }
}

#[test]
fn the_same_spec_always_renders_the_same_text() {
    assert_eq!(render_identity(&spec()), render_identity(&spec()));
}

#[test]
fn the_environment_renders_in_key_order_however_it_was_declared() {
    let reordered = AppSpec {
        env: spec().env.into_iter().rev().collect(),
        ..spec()
    };
    assert_eq!(render_identity(&spec()), render_identity(&reordered));
}

#[test]
fn duplicate_environment_keys_render_in_value_order() {
    let one = AppSpec {
        env: vec![
            ("PORT".to_string(), "8080".to_string()),
            ("PORT".to_string(), "9090".to_string()),
        ],
        ..spec()
    };
    let other = AppSpec {
        env: one.env.iter().cloned().rev().collect(),
        ..spec()
    };
    assert_eq!(render_identity(&one), render_identity(&other));
}

#[test]
fn roots_pm3_derived_from_its_own_environment_leave_the_identity_unchanged() {
    let elsewhere = with_sandbox(SandboxPolicy {
        derived_roots: vec![
            "/private/var/folders/xy/T".to_string(),
            "/srv/pm3/logs".to_string(),
        ],
        ..spec().sandbox
    });
    assert_eq!(render_identity(&spec()), render_identity(&elsewhere));
}

#[test]
fn a_daemon_without_a_temporary_directory_reads_the_same_identity() {
    let bare = with_sandbox(SandboxPolicy {
        derived_roots: Vec::new(),
        ..spec().sandbox
    });
    assert_eq!(render_identity(&spec()), render_identity(&bare));
}

#[test]
fn the_restart_policy_leaves_the_identity_unchanged() {
    let retuned = AppSpec {
        autorestart: false,
        min_uptime_ms: 5000,
        max_restarts: 1,
        restart_delay_ms: 250,
        ..spec()
    };
    assert_eq!(render_identity(&spec()), render_identity(&retuned));
}

#[test]
fn a_different_program_renders_differently() {
    let upgraded = AppSpec {
        script: "/opt/node/bin/node".to_string(),
        ..spec()
    };
    assert_ne!(render_identity(&spec()), render_identity(&upgraded));
}

#[test]
fn reordering_the_arguments_renders_differently() {
    let swapped = AppSpec {
        args: spec().args.into_iter().rev().collect(),
        ..spec()
    };
    assert_ne!(render_identity(&spec()), render_identity(&swapped));
}

#[test]
fn a_different_working_directory_renders_differently() {
    let moved = AppSpec {
        cwd: "/srv/other".to_string(),
        ..spec()
    };
    assert_ne!(render_identity(&spec()), render_identity(&moved));
}

#[test]
fn a_different_environment_value_renders_differently() {
    let retuned = AppSpec {
        env: vec![("PORT".to_string(), "9090".to_string())],
        ..spec()
    };
    assert_ne!(render_identity(&spec()), render_identity(&retuned));
}

#[test]
fn a_renamed_service_renders_differently() {
    let renamed = AppSpec {
        name: "web".to_string(),
        ..spec()
    };
    assert_ne!(render_identity(&spec()), render_identity(&renamed));
}

#[test]
fn a_declared_writable_root_renders_differently() {
    let widened = with_sandbox(SandboxPolicy {
        writable_roots: vec!["/srv".to_string()],
        ..spec().sandbox
    });
    assert_ne!(render_identity(&spec()), render_identity(&widened));
}

#[test]
fn a_different_sandbox_mode_renders_differently() {
    let opened = with_sandbox(SandboxPolicy {
        mode: SandboxMode::DangerFullAccess,
        ..spec().sandbox
    });
    assert_ne!(render_identity(&spec()), render_identity(&opened));
}

#[test]
fn granting_the_network_renders_differently() {
    let online = with_sandbox(SandboxPolicy {
        network: true,
        ..spec().sandbox
    });
    assert_ne!(render_identity(&spec()), render_identity(&online));
}

#[test]
fn an_argument_holding_a_newline_cannot_forge_another_field() {
    let smuggled = AppSpec {
        args: vec!["a\ncwd 10 /srv/other".to_string()],
        ..spec()
    };
    let honest = AppSpec {
        args: vec!["a".to_string()],
        cwd: "/srv/other".to_string(),
        ..spec()
    };
    assert_ne!(render_identity(&smuggled), render_identity(&honest));
}

#[test]
fn every_field_is_length_prefixed() {
    let rendered = render_identity(&spec());
    assert!(
        rendered.contains("program 13 /usr/bin/node\n"),
        "got: {rendered}"
    );
}
