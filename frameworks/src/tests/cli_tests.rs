#![cfg(unix)]
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
fn reset_takes_a_selector() {
    let cli = parse(&["pm3", "reset", "web"]);
    assert!(
        matches!(&cli.command, Commands::Reset { selector } if selector == "web"),
        "got: {:?}",
        cli.command
    );
}

#[test]
fn send_signal_takes_the_signal_first_then_the_selector() {
    let cli = parse(&["pm3", "sendSignal", "hup", "web"]);
    assert!(
        matches!(&cli.command, Commands::SendSignal { signal, selector } if signal == "hup" && selector == "web"),
        "got: {:?}",
        cli.command
    );
}

#[test]
fn shutdown_takes_the_with_services_flag() {
    let cli = parse(&["pm3", "shutdown", "--with-services"]);
    assert!(
        matches!(
            &cli.command,
            Commands::Shutdown {
                with_services: true
            }
        ),
        "got: {:?}",
        cli.command
    );
}

#[test]
fn the_old_command_names_are_rejected() {
    for legacy in ["kill", "signal", "service"] {
        assert!(
            Cli::try_parse_from(["pm3", legacy]).is_err(),
            "{legacy} should no longer parse"
        );
    }
}

#[test]
fn startup_registers_by_default() {
    let cli = parse(&["pm3", "startup"]);
    assert!(
        matches!(
            &cli.command,
            Commands::Startup {
                dry_run: false,
                force: false,
                status: false
            }
        ),
        "got: {:?}",
        cli.command
    );
}

#[test]
fn startup_status_conflicts_with_the_registration_flags() {
    assert!(Cli::try_parse_from(["pm3", "startup", "--status", "--force"]).is_err());
    assert!(Cli::try_parse_from(["pm3", "startup", "--status", "--dry-run"]).is_err());
    assert!(Cli::try_parse_from(["pm3", "startup", "--status"]).is_ok());
}

#[test]
fn unstartup_takes_a_dry_run_flag() {
    let cli = parse(&["pm3", "unstartup", "--dry-run"]);
    assert!(
        matches!(&cli.command, Commands::Unstartup { dry_run: true }),
        "got: {:?}",
        cli.command
    );
}

#[test]
fn list_answers_to_its_pm2_aliases() {
    for alias in ["l", "ls", "ps", "status"] {
        assert!(
            matches!(
                parse(&["pm3", alias]).command,
                Commands::List { json: false }
            ),
            "{alias} should parse as list"
        );
    }
}

#[test]
fn describe_answers_to_its_pm2_aliases() {
    for alias in ["desc", "info", "show"] {
        assert!(
            matches!(
                parse(&["pm3", alias, "web"]).command,
                Commands::Describe { .. }
            ),
            "{alias} should parse as describe"
        );
    }
}

#[test]
fn the_help_groups_every_visible_command_and_hides_the_rest() {
    let help = Cli::command().render_help().to_string();
    assert!(help.contains("Apps:"), "got:\n{help}");
    assert!(help.contains("pm3 itself:"), "got:\n{help}");
    assert!(help.contains("Options:"), "got:\n{help}");
    for visible in [
        "start",
        "stop",
        "restart",
        "delete",
        "reset",
        "sendSignal",
        "describe",
        "list",
        "logs",
        "install",
        "startup",
        "unstartup",
        "shutdown",
        "config",
        "completion",
    ] {
        assert!(
            help.lines()
                .any(|line| line.starts_with(&format!("  {visible}"))),
            "{visible} missing from the help:\n{help}"
        );
    }
    for hidden in ["daemon", "__sleep", "kill"] {
        assert!(
            !help
                .lines()
                .any(|line| line.starts_with(&format!("  {hidden}"))),
            "{hidden} leaked into the help:\n{help}"
        );
    }
}

#[test]
fn start_takes_the_pm2_style_flags() {
    let cli = parse(&[
        "pm3",
        "start",
        "-n",
        "web",
        "--max-memory-restart",
        "300M",
        "--stop-exit-codes",
        "42",
        "--cron-restart",
        "0 * * * *",
        "-f",
        "/bin/true",
    ]);
    let Commands::Start(args) = &cli.command else {
        panic!(
            "start should parse into its own arguments: {:?}",
            cli.command
        )
    };
    assert_eq!(args.name.as_deref(), Some("web"));
    assert_eq!(args.max_memory.as_deref(), Some("300M"));
    assert_eq!(args.stop_exit_codes, [42]);
    assert_eq!(args.cron.as_deref(), Some("0 * * * *"));
    assert!(args.force, "-f is the short form of --force");
}

#[test]
fn completion_takes_a_shell() {
    let cli = parse(&["pm3", "completion", "bash"]);
    assert!(
        matches!(&cli.command, Commands::Completion { shell } if *shell == clap_complete::Shell::Bash),
        "got: {:?}",
        cli.command
    );
}

#[test]
fn completion_rejects_an_unknown_shell() {
    assert!(Cli::try_parse_from(["pm3", "completion", "tcsh"]).is_err());
}

#[tokio::test]
async fn completion_prints_the_script_and_has_no_report() {
    let printed = execute(parse(&["pm3", "completion", "zsh"]))
        .await
        .expect("should generate");
    assert!(printed.is_none(), "got: {printed:?}");
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
fn logs_stream_by_default() {
    let cli = parse(&["pm3", "logs", "web"]);
    let Commands::Logs(args) = &cli.command else {
        panic!(
            "logs should parse into its own arguments: {:?}",
            cli.command
        )
    };
    assert_eq!(args.names, ["web".to_string()]);
    assert!(args.lines.is_none());
    assert!(!args.nostream && !args.err && !args.all && !args.clear);
}

#[test]
fn logs_accept_a_line_count_and_the_nostream_flag() {
    let cli = parse(&["pm3", "logs", "web", "-n", "5", "--nostream"]);
    assert!(
        matches!(
            &cli.command,
            Commands::Logs(args) if args.lines == Some(5) && args.nostream
        ),
        "got: {:?}",
        cli.command
    );
}

#[test]
fn logs_clear_conflicts_with_a_line_count() {
    assert!(Cli::try_parse_from(["pm3", "logs", "web", "--clear", "-n", "5"]).is_err());
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

#[path = "cli_report_tests.rs"]
mod reports;
