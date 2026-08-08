#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use self::common::{home, pm3, shutdown_daemon, sleeper_apps, stdout_of, wait_for_listing};

#[test]
fn the_listing_reports_memory_and_cpu_for_a_running_service() {
    let home = home();
    let apps = sleeper_apps(&home, "web");
    let started = pm3(&home, &["start", apps.to_str().expect("path")]);
    assert!(started.status.success(), "{}", stdout_of(&started));

    let listing = wait_for_listing(&home, "web");
    let row = listing
        .lines()
        .find(|line| line.contains("web"))
        .expect("web should be listed");
    let cells: Vec<&str> = row.split_whitespace().collect();
    let rss = cells.get(6).expect("rss column present");
    assert!(rss.ends_with('K') || rss.ends_with('M'), "got: {rss}");
    let cpu = cells.get(7).expect("cpu column present");
    assert!(cpu.ends_with('%'), "got: {cpu}");
    shutdown_daemon(&home);
}

#[test]
fn the_listing_renders_json_when_asked() {
    let home = home();
    let apps = sleeper_apps(&home, "web");
    let started = pm3(&home, &["start", apps.to_str().expect("path")]);
    assert!(started.status.success(), "{}", stdout_of(&started));

    let shown = stdout_of(&pm3(&home, &["list", "--json"]));
    assert!(shown.contains("\"name\":\"web\""), "got: {shown}");
    assert!(shown.contains("\"status\":\"online\""), "got: {shown}");
    assert!(!shown.contains("env"), "got: {shown}");
    let described = stdout_of(&pm3(&home, &["describe", "web", "--json"]));
    assert!(described.starts_with('{'), "got: {described}");
    shutdown_daemon(&home);
}
