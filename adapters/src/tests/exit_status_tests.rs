use super::*;

#[test]
fn a_silent_refusal_falls_back_to_the_exit_code() {
    assert_eq!(describe_refusal("   \n", 7), "exited with status 7");
}

#[test]
fn a_noisy_refusal_reports_what_the_program_said() {
    assert_eq!(
        describe_refusal("  no such process\n", 1),
        "no such process"
    );
}

#[tokio::test]
async fn a_clean_exit_reports_zero() {
    let status = tokio::process::Command::new("/usr/bin/true")
        .status()
        .await
        .expect("should run /usr/bin/true");
    assert_eq!(exit_code_of(&status), 0);
}

#[tokio::test]
async fn a_process_killed_by_a_signal_has_no_code() {
    let status = tokio::process::Command::new("/bin/sh")
        .args(["-c", "kill -TERM $$"])
        .status()
        .await
        .expect("should run /bin/sh");
    assert_eq!(exit_code_of(&status), UNKNOWN_EXIT_CODE);
}

#[tokio::test]
async fn a_clean_exit_becomes_a_reported_code() {
    let status = tokio::process::Command::new("/usr/bin/true")
        .status()
        .await
        .expect("should run /usr/bin/true");
    assert_eq!(exit_outcome_of(status), ExitOutcome::Code(0));
}

#[tokio::test]
async fn an_exit_by_signal_becomes_a_failure_pm3_can_see() {
    let status = tokio::process::Command::new("/bin/sh")
        .args(["-c", "kill -TERM $$"])
        .status()
        .await
        .expect("should run /bin/sh");
    let outcome = exit_outcome_of(status);
    assert_eq!(outcome, ExitOutcome::Signalled);
    assert!(outcome.failed());
}
