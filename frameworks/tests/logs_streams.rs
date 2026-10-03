#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use self::common::{
    app_error_log, app_log, chatty_command, home, pm3, shell_app, shutdown_daemon, stdout_of,
    wait_for_log, write_apps,
};

fn chatty_apps(home: &common::Home) -> std::path::PathBuf {
    let web = shell_app(home, "web", &chatty_command("web-out", "web-err"));
    let api = shell_app(home, "api", &chatty_command("api-out", "api-err"));
    write_apps(home, &format!("apps:\n{web}{api}"))
}

fn start_chatty(home: &common::Home) {
    let apps = chatty_apps(home);
    common::start_ok(home, &apps);
    wait_for_log(&app_log(home, "web"), "web-out");
    wait_for_log(&app_error_log(home, "web"), "web-err");
    wait_for_log(&app_log(home, "api"), "api-out");
    wait_for_log(&app_error_log(home, "api"), "api-err");
}

#[test]
fn logs_with_err_reads_the_stderr_stream() {
    let home = home();
    start_chatty(&home);
    let shown = stdout_of(&pm3(&home, &["logs", "web", "--err", "--nostream"]));
    assert!(shown.contains("web-err"), "got: {shown}");
    assert!(!shown.contains("web-out"), "got: {shown}");
    shutdown_daemon(&home);
}

#[test]
fn logs_without_a_name_aggregates_every_declared_service() {
    let home = home();
    start_chatty(&home);
    let shown = stdout_of(&pm3(&home, &["logs", "--nostream"]));
    assert!(shown.contains("api | api-out"), "got: {shown}");
    assert!(shown.contains("web | web-out"), "got: {shown}");
    shutdown_daemon(&home);
}

#[test]
fn logs_with_several_names_prefixes_each_service() {
    let home = home();
    start_chatty(&home);
    let shown = stdout_of(&pm3(&home, &["logs", "web", "api", "--nostream"]));
    assert!(shown.contains("web | web-out"), "got: {shown}");
    assert!(shown.contains("api | api-out"), "got: {shown}");
    shutdown_daemon(&home);
}

#[test]
fn logs_with_all_merges_both_streams() {
    let home = home();
    start_chatty(&home);
    let shown = stdout_of(&pm3(&home, &["logs", "web", "--all", "--nostream"]));
    assert!(shown.contains("web [out] | web-out"), "got: {shown}");
    assert!(shown.contains("web [err] | web-err"), "got: {shown}");
    shutdown_daemon(&home);
}

#[test]
fn logs_with_clear_truncates_the_selected_log() {
    let home = home();
    start_chatty(&home);
    let cleared = stdout_of(&pm3(&home, &["logs", "web", "--clear"]));
    assert!(cleared.contains("cleared "), "got: {cleared}");
    assert_eq!(
        std::fs::metadata(app_log(&home, "web"))
            .expect("stat the cleared log")
            .len(),
        0
    );
    let err_log = std::fs::metadata(app_error_log(&home, "web"))
        .expect("stat the untouched err log")
        .len();
    assert!(err_log > 0, "the err log must stay, got: {err_log}");
    shutdown_daemon(&home);
}
