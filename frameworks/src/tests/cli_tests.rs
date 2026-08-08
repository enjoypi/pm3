use super::*;

#[test]
fn the_default_config_lives_in_the_default_pm3_home() {
    assert_eq!(
        default_config(None, Some("/home/dev")),
        "/home/dev/.pm3/config.yaml"
    );
}

#[test]
fn the_default_config_falls_back_to_the_working_directory_without_a_home() {
    assert_eq!(default_config(None, None), CONFIG_FILE);
}

fn parse(args: &[&str]) -> Cli {
    Cli::parse_from(args)
}

fn running_web_report() -> commands::StartReport {
    commands::StartReport {
        response: "web is already running (id 0, pid 1)".to_string(),
        changed: vec!["web".to_string()],
        already_running: vec!["web".to_string()],
    }
}

#[tokio::test]
async fn an_accepted_restart_runs_and_reports() {
    let fixture = crate::daemon_fixture::running_daemon().await;
    let apps_file = crate::daemon_fixture::sleeper_apps_file(&fixture);
    commands::start_apps(&fixture.config_path, &apps_file, false)
        .await
        .expect("should start");
    let offered = offer_restarts(&fixture.config_path, &running_web_report(), &mut |_| true)
        .await
        .expect("should offer");
    assert_eq!(offered.as_deref(), Some("restarted web"));
    crate::daemon_fixture::stop_daemon(fixture).await;
}

#[tokio::test]
async fn a_declined_restart_reports_the_hint() {
    let offered = offer_restarts("/nonexistent/pm3.yaml", &running_web_report(), &mut |_| {
        false
    })
    .await
    .expect("should offer");
    assert_eq!(
        offered.as_deref(),
        Some(
            "'web' keeps running with the previous config; run 'pm3 restart web' to apply the new one"
        )
    );
}

#[tokio::test]
async fn a_failing_restart_aborts_the_offer() {
    let outcome = offer_restarts("/nonexistent/pm3.yaml", &running_web_report(), &mut |_| {
        true
    })
    .await;
    assert!(outcome.is_err(), "got: {outcome:?}");
}

#[tokio::test]
async fn nothing_changed_offers_nothing() {
    let report = commands::StartReport {
        response: "started web (id 0, pid 1)".to_string(),
        changed: Vec::new(),
        already_running: Vec::new(),
    };
    let offered = offer_restarts("/nonexistent/pm3.yaml", &report, &mut |_| true)
        .await
        .expect("should offer");
    assert_eq!(offered, None);
}

#[test]
fn the_config_path_defaults_to_the_pm3_home() {
    assert_eq!(
        parse(&["pm3", "list"]).config,
        default_config(None, host_home().as_deref())
    );
}

#[test]
fn the_config_path_can_be_overridden() {
    let cli = parse(&["pm3", "--config", "/srv/pm3.yaml", "list"]);
    assert_eq!(cli.config, "/srv/pm3.yaml");
}

#[test]
fn start_takes_an_apps_file() {
    let cli = parse(&["pm3", "start", "apps.yaml"]);
    assert!(
        matches!(&cli.command, Commands::Start(args) if args.target == ["apps.yaml"] && args.name.is_none()),
        "got: {:?}",
        cli.command
    );
}

#[test]
fn start_keeps_the_program_flags_for_the_program() {
    let cli = parse(&[
        "pm3",
        "start",
        "--name",
        "mihomo-rule",
        "--network",
        "mihomo",
        "-d",
        "/data",
        "-f",
        "/etc/rule.yaml",
    ]);
    let Commands::Start(args) = &cli.command else {
        panic!(
            "start should parse into its own arguments: {:?}",
            cli.command
        )
    };
    assert_eq!(args.name.as_deref(), Some("mihomo-rule"));
    assert!(args.network, "--network belongs to pm3");
    assert_eq!(
        args.target,
        ["mihomo", "-d", "/data", "-f", "/etc/rule.yaml"]
    );
}

#[test]
fn start_collects_repeated_pm3_options() {
    let cli = parse(&[
        "pm3",
        "start",
        "--name",
        "app",
        "--writable-dir",
        "/srv",
        "--writable-dir",
        "/opt/data",
        "--cwd",
        "/work",
        "--force",
        "/bin/sh",
    ]);
    let Commands::Start(args) = &cli.command else {
        panic!(
            "start should parse into its own arguments: {:?}",
            cli.command
        )
    };
    assert_eq!(args.writable_dirs, ["/srv", "/opt/data"]);
    assert_eq!(args.cwd.as_deref(), Some("/work"));
    assert!(args.force, "--force belongs to pm3");
}

#[test]
fn stop_takes_a_selector() {
    let cli = parse(&["pm3", "stop", "web"]);
    assert!(
        matches!(&cli.command, Commands::Stop { selector } if selector == "web"),
        "got: {:?}",
        cli.command
    );
}

#[test]
fn restart_takes_a_selector() {
    let cli = parse(&["pm3", "restart", "3"]);
    assert!(
        matches!(&cli.command, Commands::Restart { selector } if selector == "3"),
        "got: {:?}",
        cli.command
    );
}

#[test]
fn delete_takes_a_selector() {
    let cli = parse(&["pm3", "delete", "web"]);
    assert!(
        matches!(&cli.command, Commands::Delete { selector } if selector == "web"),
        "got: {:?}",
        cli.command
    );
}

#[test]
fn describe_takes_a_selector() {
    let cli = parse(&["pm3", "describe", "web"]);
    assert!(
        matches!(&cli.command, Commands::Describe { selector, json: _ } if selector == "web"),
        "got: {:?}",
        cli.command
    );
}

#[test]
fn list_takes_no_argument() {
    assert!(matches!(
        parse(&["pm3", "list"]).command,
        Commands::List { json: false }
    ));
}

#[test]
fn logs_leave_the_line_count_to_the_config_without_following() {
    let cli = parse(&["pm3", "logs", "web"]);
    assert!(
        matches!(
            &cli.command,
            Commands::Logs {
                names,
                lines,
                follow,
                err,
                all
            }
            if names == &["web".to_string()] && lines.is_none() && !follow && !err && !all
        ),
        "got: {:?}",
        cli.command
    );
}

#[test]
fn logs_accept_a_line_count_and_the_follow_flag() {
    let cli = parse(&["pm3", "logs", "web", "-n", "5", "-f"]);
    assert!(
        matches!(
            &cli.command,
            Commands::Logs { lines, follow, .. } if *lines == Some(5) && *follow
        ),
        "got: {:?}",
        cli.command
    );
}

#[test]
fn config_check_is_a_subcommand_of_config() {
    let cli = parse(&["pm3", "config", "check"]);
    assert!(
        matches!(
            &cli.command,
            Commands::Config {
                command: ConfigCommands::Check
            }
        ),
        "got: {:?}",
        cli.command
    );
}

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
