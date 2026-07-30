use super::*;

fn launch() -> LaunchSpec {
    LaunchSpec {
        name: "api".to_string(),
        program: "/usr/bin/node".to_string(),
        args: vec!["server.js".to_string(), "--port=8080".to_string()],
        cwd: "/srv/api".to_string(),
        env: vec![
            ("PORT".to_string(), "8080".to_string()),
            ("HOME".to_string(), "/srv/api".to_string()),
        ],
        stdout_path: "/logs/api-out.log".to_string(),
        stderr_path: "/logs/api-err.log".to_string(),
    }
}

#[test]
fn the_same_launch_always_renders_the_same_text() {
    assert_eq!(render_launch(&launch()), render_launch(&launch()));
}

#[test]
fn the_environment_renders_in_key_order_however_it_was_declared() {
    let reordered = LaunchSpec {
        env: launch().env.into_iter().rev().collect(),
        ..launch()
    };
    assert_eq!(render_launch(&launch()), render_launch(&reordered));
}

#[test]
fn duplicate_environment_keys_render_in_value_order() {
    let one = LaunchSpec {
        env: vec![
            ("PORT".to_string(), "8080".to_string()),
            ("PORT".to_string(), "9090".to_string()),
        ],
        ..launch()
    };
    let other = LaunchSpec {
        env: one.env.iter().cloned().rev().collect(),
        ..launch()
    };
    assert_eq!(render_launch(&one), render_launch(&other));
}

#[test]
fn moving_the_log_files_leaves_the_launch_unchanged() {
    let relocated = LaunchSpec {
        stdout_path: "/elsewhere/api-out.log".to_string(),
        stderr_path: "/elsewhere/api-err.log".to_string(),
        ..launch()
    };
    assert_eq!(render_launch(&launch()), render_launch(&relocated));
}

#[test]
fn a_different_program_renders_differently() {
    let upgraded = LaunchSpec {
        program: "/opt/node/bin/node".to_string(),
        ..launch()
    };
    assert_ne!(render_launch(&launch()), render_launch(&upgraded));
}

#[test]
fn reordering_the_arguments_renders_differently() {
    let swapped = LaunchSpec {
        args: launch().args.into_iter().rev().collect(),
        ..launch()
    };
    assert_ne!(render_launch(&launch()), render_launch(&swapped));
}

#[test]
fn a_different_working_directory_renders_differently() {
    let moved = LaunchSpec {
        cwd: "/srv/other".to_string(),
        ..launch()
    };
    assert_ne!(render_launch(&launch()), render_launch(&moved));
}

#[test]
fn a_different_environment_value_renders_differently() {
    let retuned = LaunchSpec {
        env: vec![("PORT".to_string(), "9090".to_string())],
        ..launch()
    };
    assert_ne!(render_launch(&launch()), render_launch(&retuned));
}

#[test]
fn a_renamed_service_renders_differently() {
    let renamed = LaunchSpec {
        name: "web".to_string(),
        ..launch()
    };
    assert_ne!(render_launch(&launch()), render_launch(&renamed));
}

#[test]
fn an_argument_holding_a_newline_cannot_forge_another_field() {
    let smuggled = LaunchSpec {
        args: vec!["a\ncwd 10 /srv/other".to_string()],
        ..launch()
    };
    let honest = LaunchSpec {
        args: vec!["a".to_string()],
        cwd: "/srv/other".to_string(),
        ..launch()
    };
    assert_ne!(render_launch(&smuggled), render_launch(&honest));
}

#[test]
fn every_field_is_length_prefixed() {
    let rendered = render_launch(&launch());
    assert!(
        rendered.contains("program 13 /usr/bin/node\n"),
        "got: {rendered}"
    );
}
