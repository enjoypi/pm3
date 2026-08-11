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
fn signal_takes_a_selector_and_a_signal_name() {
    let cli = parse(&["pm3", "signal", "web", "hup"]);
    assert!(
        matches!(&cli.command, Commands::Signal { selector, name } if selector == "web" && name == "hup"),
        "got: {:?}",
        cli.command
    );
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
                all,
                clear
            }
            if names == &["web".to_string()] && lines.is_none() && !follow && !err && !all && !clear
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

#[path = "cli_report_tests.rs"]
mod reports;
