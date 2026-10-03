#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use self::common::{
    Home, PM3, daemon_log, home, pm3, shutdown_daemon, stderr_of, stdout_of, wait_for_log,
};

const TASK: &str = "ticker";

fn start_task(home: &Home, cron: &str, extra: &[&str]) -> std::process::Output {
    let mut args = vec!["start", "--name", TASK, "--cron", cron, "--no-autorestart"];
    args.extend_from_slice(extra);
    args.extend_from_slice(&[PM3, "__sleep", "100"]);
    pm3(home, &args)
}

fn field_of(row: &str, index: usize) -> String {
    row.split_whitespace()
        .nth(index)
        .unwrap_or_default()
        .to_string()
}

fn task_row(home: &Home) -> String {
    stdout_of(&pm3(home, &["list"]))
        .lines()
        .find(|line| line.contains(TASK))
        .unwrap_or_default()
        .to_string()
}

#[test]
fn a_scheduled_task_is_registered_without_running() {
    let home = home();
    let started = start_task(&home, "* * * * *", &[]);
    assert!(started.status.success(), "{}", stderr_of(&started));
    assert!(
        stdout_of(&started).contains("scheduled ticker"),
        "{}",
        stdout_of(&started)
    );

    let row = task_row(&home);
    assert_eq!(field_of(&row, 2), "stopped", "got row: {row}");
    assert_eq!(
        field_of(&row, 5),
        "-",
        "a pending task reports no resources: {row}"
    );
    shutdown_daemon(&home);
}

#[test]
fn a_scheduled_task_advertises_its_next_fire() {
    let home = home();
    let started = start_task(&home, "* * * * *", &[]);
    assert!(started.status.success(), "{}", stderr_of(&started));

    let next = field_of(&task_row(&home), 6);
    assert!(
        next.contains(':'),
        "the next column reads as HH:MM, the header carries the offset, got: {next}"
    );
    assert_eq!(
        common::described_field(&home, TASK, "schedule"),
        "* * * * *"
    );
    assert!(
        common::described_field(&home, TASK, "next fire").contains("UTC+"),
        "describe should stamp the zone"
    );
    shutdown_daemon(&home);
}

#[test]
fn stopping_a_scheduled_task_clears_its_next_fire() {
    let home = home();
    let started = start_task(&home, "* * * * *", &[]);
    assert!(started.status.success(), "{}", stderr_of(&started));
    assert_ne!(field_of(&task_row(&home), 6), "-");

    let stopped = pm3(&home, &["stop", TASK]);
    assert!(stopped.status.success(), "{}", stderr_of(&stopped));
    assert_eq!(
        field_of(&task_row(&home), 6),
        "-",
        "a stopped task must drop its timer"
    );
    shutdown_daemon(&home);
}

#[test]
fn a_random_schedule_keeps_its_draw_when_restarted_within_its_window() {
    let home = home();
    let started = start_task(&home, "0 0 1 ~ *", &[]);
    assert!(started.status.success(), "{}", stderr_of(&started));

    let first = common::described_field(&home, TASK, "next fire");
    for _ in 0..4 {
        let restarted = pm3(&home, &["restart", TASK]);
        assert!(restarted.status.success(), "{}", stderr_of(&restarted));
        assert_eq!(
            common::described_field(&home, TASK, "next fire"),
            first,
            "a restart inside a window must not redraw it"
        );
    }
    shutdown_daemon(&home);
}

#[test]
fn an_unparsable_schedule_is_refused_before_the_daemon_sees_it() {
    let home = home();
    let refused = start_task(&home, "nonsense", &[]);
    assert!(!refused.status.success(), "{}", stdout_of(&refused));
    assert!(
        stderr_of(&refused).contains("cannot parse schedule"),
        "{}",
        stderr_of(&refused)
    );
    shutdown_daemon(&home);
}

#[test]
fn a_schedule_out_of_range_is_refused_before_the_daemon_sees_it() {
    let home = home();
    let refused = start_task(&home, "0~99 * * * *", &[]);
    assert!(!refused.status.success(), "{}", stdout_of(&refused));
    assert!(
        stderr_of(&refused).contains("bounds must fall within"),
        "{}",
        stderr_of(&refused)
    );
    shutdown_daemon(&home);
}

#[test]
fn a_due_schedule_fires_the_task_and_arms_the_next_cycle() {
    let home = home();
    let started = start_task(&home, "*/2 * * * * *", &[]);
    assert!(started.status.success(), "{}", stderr_of(&started));

    let journal = wait_for_log(&daemon_log(&home), "\"action\":\"spawn\"");
    assert!(
        journal.contains("\"action\":\"arm\""),
        "a fire must arm the following cycle"
    );

    shutdown_daemon(&home);
}
