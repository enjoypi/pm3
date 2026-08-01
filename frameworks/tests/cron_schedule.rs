#![allow(
    clippy::tests_outside_test_module,
    reason = "integration tests in tests/ are inherently outside #[cfg(test)]"
)]

mod common;

use self::common::{
    Home, PM3, daemon_log, home, pm3, shutdown_daemon, stderr_of, stdout_of, verbose_home,
    wait_for_log,
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

fn described(home: &Home, label: &str) -> String {
    stdout_of(&pm3(home, &["describe", TASK]))
        .lines()
        .find(|line| line.trim_start().starts_with(label))
        .map(|line| line.trim_start_matches(label).trim().to_string())
        .unwrap_or_default()
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
    assert_eq!(field_of(&row, 3), "stopped", "got row: {row}");
    assert_eq!(field_of(&row, 2), "-", "a pending task holds no pid: {row}");
    shutdown_daemon(&home);
}

#[test]
fn a_scheduled_task_advertises_its_next_fire() {
    let home = home();
    let started = start_task(&home, "* * * * *", &[]);
    assert!(started.status.success(), "{}", stderr_of(&started));

    let next = field_of(&task_row(&home), 6);
    assert!(
        next.contains(':') && (next.contains('+') || next.contains('-')),
        "the next column should read as HH:MM±HH:MM, got: {next}"
    );
    assert_eq!(described(&home, "schedule"), "* * * * *");
    assert!(
        described(&home, "next fire").contains("UTC+"),
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
fn a_random_schedule_re_rolls_when_the_cycle_restarts() {
    let home = home();
    let started = start_task(&home, "~ * * * *", &[]);
    assert!(started.status.success(), "{}", stderr_of(&started));

    let first = described(&home, "next fire");
    let mut moved = false;
    for _ in 0..16 {
        let restarted = pm3(&home, &["restart", TASK]);
        assert!(restarted.status.success(), "{}", stderr_of(&restarted));
        if described(&home, "next fire") != first {
            moved = true;
            break;
        }
    }
    assert!(
        moved,
        "a tilde schedule must land on a new minute eventually"
    );
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
    let home = verbose_home();
    let started = start_task(&home, "*/2 * * * * *", &[]);
    assert!(started.status.success(), "{}", stderr_of(&started));

    let journal = wait_for_log(&daemon_log(&home), "\"action\":\"spawn\"");
    assert!(
        journal.contains("\"action\":\"arm\""),
        "a fire must arm the following cycle"
    );

    shutdown_daemon(&home);
}
