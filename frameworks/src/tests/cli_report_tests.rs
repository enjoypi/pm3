#![cfg(unix)]
use super::*;

#[test]
fn config_show_is_a_subcommand_of_config() {
    let cli = parse(&["pm3", "config", "show"]);
    assert!(
        matches!(
            &cli.command,
            Commands::Config {
                command: ConfigCommands::Show
            }
        ),
        "got: {:?}",
        cli.command
    );
}

#[test]
fn the_daemon_runs_in_the_foreground() {
    assert!(matches!(
        parse(&["pm3", "daemon"]).command,
        Commands::Daemon
    ));
}

#[test]
fn the_hidden_sleep_target_takes_a_duration() {
    let cli = parse(&["pm3", "__sleep", "25"]);
    assert!(
        matches!(&cli.command, Commands::Sleep { ms } if *ms == 25),
        "got: {:?}",
        cli.command
    );
}

#[test]
fn an_unknown_subcommand_is_rejected() {
    assert!(Cli::try_parse_from(["pm3", "teleport"]).is_err());
}

#[test]
fn the_err_and_all_log_flags_conflict() {
    assert!(Cli::try_parse_from(["pm3", "logs", "web", "--err", "--all"]).is_err());
}

#[tokio::test]
async fn the_sleep_target_returns_nothing_to_print() {
    let printed = execute(parse(&["pm3", "__sleep", "1"]))
        .await
        .expect("should sleep");
    assert_eq!(printed, None);
}

#[tokio::test]
async fn dispatching_a_command_prints_its_output() {
    let dir = tempfile::tempdir().expect("temp dir");
    let config = crate::test_support::write_config(dir.path(), "/tmp/pm3-cli-check");
    dispatch(parse(&[
        "pm3",
        "--config",
        config.to_str().expect("path"),
        "config",
        "check",
    ]))
    .await
    .expect("should dispatch");
}

#[tokio::test]
async fn dispatching_a_silent_command_prints_nothing() {
    dispatch(parse(&["pm3", "__sleep", "1"]))
        .await
        .expect("should dispatch");
}

#[tokio::test]
async fn showing_a_config_returns_the_resolved_document() {
    let dir = tempfile::tempdir().expect("temp dir");
    let config = crate::test_support::write_config(dir.path(), "/tmp/pm3-cli-show");
    let printed = execute(parse(&[
        "pm3",
        "--config",
        config.to_str().expect("path"),
        "config",
        "show",
    ]))
    .await
    .expect("should show");
    assert!(
        printed.unwrap_or_default().contains("pm3-cli-show"),
        "the resolved document should carry the home"
    );
}

#[tokio::test]
async fn checking_a_missing_config_fails() {
    let outcome = execute(parse(&[
        "pm3",
        "--config",
        "/nonexistent/pm3.yaml",
        "config",
        "check",
    ]))
    .await;
    assert!(outcome.is_err(), "got: {outcome:?}");
}

#[tokio::test]
async fn an_accepted_signal_delivers_and_reports() {
    let fixture = crate::daemon_fixture::running_daemon().await;
    let apps_file = crate::daemon_fixture::sleeper_apps_file(&fixture);
    let config = fixture.config_path.clone();
    execute(parse(&["pm3", "--config", &config, "start", &apps_file]))
        .await
        .expect("should start");

    let signalled = execute(parse(&[
        "pm3", "--config", &config, "signal", "web", "usr1",
    ]))
    .await
    .expect("should signal");
    assert_eq!(signalled.as_deref(), Some("sent USR1 to web"));

    crate::daemon_fixture::stop_daemon(fixture).await;
}

#[tokio::test]
async fn every_app_subcommand_reaches_the_daemon() {
    let fixture = crate::daemon_fixture::running_daemon().await;
    let apps_file = crate::daemon_fixture::sleeper_apps_file(&fixture);
    let config = fixture.config_path.clone();

    let started = execute(parse(&["pm3", "--config", &config, "start", &apps_file]))
        .await
        .expect("should start");
    assert!(
        started.is_none(),
        "a fresh start has nothing to restart: {started:?}"
    );

    let listed = execute(parse(&["pm3", "--config", &config, "list"]))
        .await
        .expect("should list");
    assert!(
        listed.unwrap_or_default().contains("web"),
        "list should show the app"
    );

    let described = execute(parse(&["pm3", "--config", &config, "describe", "web"]))
        .await
        .expect("should describe");
    assert!(
        described.unwrap_or_default().contains("/bin/sh"),
        "describe should show the script"
    );

    let restarted = execute(parse(&["pm3", "--config", &config, "restart", "web"]))
        .await
        .expect("should restart");
    assert_eq!(restarted.as_deref(), Some("restarted web"));

    let reset = execute(parse(&["pm3", "--config", &config, "reset", "web"]))
        .await
        .expect("should reset");
    assert_eq!(reset.as_deref(), Some("reset web"));

    let after_reset = execute(parse(&["pm3", "--config", &config, "describe", "web"]))
        .await
        .expect("should describe");
    let restarts = after_reset
        .unwrap_or_default()
        .lines()
        .find(|line| line.starts_with("restarts"))
        .expect("a restarts row")
        .trim_end()
        .to_string();
    assert!(restarts.ends_with('0'), "got: {restarts}");

    let stopped = execute(parse(&["pm3", "--config", &config, "stop", "web"]))
        .await
        .expect("should stop");
    assert_eq!(stopped.as_deref(), Some("stopped web"));

    let deleted = execute(parse(&["pm3", "--config", &config, "delete", "web"]))
        .await
        .expect("should delete");
    assert_eq!(deleted.as_deref(), Some("deleted web"));

    crate::daemon_fixture::stop_daemon(fixture).await;
}

#[test]
fn service_without_a_subcommand_asks_for_the_status() {
    let cli = parse(&["pm3", "service"]);
    assert!(
        matches!(&cli.command, Commands::Service { command: None }),
        "got: {:?}",
        cli.command
    );
}

#[test]
fn service_install_defaults_to_a_real_run() {
    let cli = parse(&["pm3", "service", "install"]);
    assert!(
        matches!(
            &cli.command,
            Commands::Service {
                command: Some(ServiceCommands::Install {
                    dry_run: false,
                    force: false
                })
            }
        ),
        "got: {:?}",
        cli.command
    );
}

#[test]
fn service_install_takes_a_dry_run_flag() {
    let cli = parse(&["pm3", "service", "install", "--dry-run"]);
    assert!(
        matches!(
            &cli.command,
            Commands::Service {
                command: Some(ServiceCommands::Install {
                    dry_run: true,
                    force: false
                })
            }
        ),
        "got: {:?}",
        cli.command
    );
}

#[test]
fn service_uninstall_takes_a_dry_run_flag() {
    let cli = parse(&["pm3", "service", "uninstall", "--dry-run"]);
    assert!(
        matches!(
            &cli.command,
            Commands::Service {
                command: Some(ServiceCommands::Uninstall { dry_run: true })
            }
        ),
        "got: {:?}",
        cli.command
    );
}

#[tokio::test]
async fn the_service_subcommand_reaches_the_status_report() {
    let dir = tempfile::tempdir().expect("temp dir");
    let home = dir.path().join("home");
    let config = crate::test_support::write_config(dir.path(), &home.to_string_lossy());
    let cli = parse(&["pm3", "--config", &config.to_string_lossy(), "service"]);
    let printed = execute(cli).await.expect("the status query should answer");
    assert!(
        printed.expect("a report").contains("not installed"),
        "an unknown service should read as not installed"
    );
}

#[test]
fn a_successful_command_reports_success() {
    assert_eq!(report(Ok(())), std::process::ExitCode::SUCCESS);
}

#[test]
fn a_failing_command_reports_failure() {
    let error = crate::Error::InlineUsage {
        reason: "no program".to_string(),
    };
    assert_eq!(report(Err(error)), std::process::ExitCode::FAILURE);
}

#[tokio::test]
async fn naming_a_service_without_a_program_explains_the_usage() {
    let err = execute(parse(&["pm3", "start", "--name", "probe"]))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("needs a program"), "got: {err}");
}

#[tokio::test]
async fn starting_without_a_target_explains_the_usage() {
    let err = execute(parse(&["pm3", "start"]))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("exactly one apps file"), "got: {err}");
}

#[tokio::test]
async fn starting_two_apps_files_is_rejected() {
    let err = execute(parse(&["pm3", "start", "a.yaml", "b.yaml"]))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("exactly one apps file"), "got: {err}");
}

#[test]
fn start_accepts_repeated_stop_exit_codes() {
    let cli = parse(&[
        "pm3",
        "start",
        "--name",
        "web",
        "--stop-exit-code",
        "3",
        "--stop-exit-code",
        "0",
        "/bin/true",
    ]);
    let Commands::Start(args) = &cli.command else {
        panic!("expected the start command, got: {:?}", cli.command);
    };
    assert_eq!(args.stop_exit_codes, [3, 0]);
}
