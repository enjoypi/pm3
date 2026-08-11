#![cfg(unix)]

use tokio::process::Command;

use super::*;

#[tokio::test]
async fn a_finished_command_reports_its_output() {
    let outcome = capture_timed(Command::new("/usr/bin/true"), 5000).await;
    let CommandOutcome::Finished(output) = outcome else {
        panic!("true should finish: {outcome:?}")
    };
    assert!(output.status.success());
}

#[tokio::test]
async fn a_missing_program_reports_a_spawn_failure() {
    let outcome = capture_timed(Command::new("/nonexistent/pm3-timed"), 5000).await;
    assert!(
        matches!(outcome, CommandOutcome::SpawnFailed(_)),
        "got: {outcome:?}"
    );
}

#[tokio::test]
async fn a_command_past_its_budget_reports_a_stall() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "exec sleep 30"]).kill_on_drop(true);
    let outcome = capture_timed(command, 50).await;
    assert!(
        matches!(outcome, CommandOutcome::Stalled),
        "got: {outcome:?}"
    );
}
