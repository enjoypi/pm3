use std::path::PathBuf;

use adapters::{CONFIG_FILE, InlineStart, default_config_path, validate_cron};
use clap::{Args, CommandFactory as _, Parser, Subcommand};

use crate::{
    Error, Result, commands,
    layout::{host_home, host_pm3_home},
    prompt,
};

pub const MISSING_COMMAND: &str = "--name needs a program to run after it";
pub const AMBIGUOUS_TARGET: &str =
    "without --name, start takes exactly one apps file; pm3 options must come before the program";

#[derive(Debug, Parser)]
#[command(
    name = "pm3",
    version,
    about = "极简进程管理器，每个服务跑在严格沙盒中"
)]
pub struct Cli {
    #[arg(long, global = true, default_value_t = default_config(host_pm3_home().as_deref(), host_home().as_deref()))]
    pub config: String,

    #[command(subcommand)]
    pub command: Commands,
}

#[expect(
    clippy::large_enum_variant,
    reason = "start carries the whole inline declaration once per CLI invocation"
)]
#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(
        about = "Start an apps file, or one program given inline after --name",
        long_about = "pm3 start <APPS_FILE>\npm3 start --name <NAME> [OPTIONS] <PROGRAM> [ARGS...]\n\npm3 options must come before the program: everything after it belongs to the program."
    )]
    Start(StartArgs),

    #[command(about = "Stop a managed app")]
    Stop { selector: String },

    #[command(about = "Restart a managed app")]
    Restart { selector: String },

    #[command(about = "Stop a managed app and forget it")]
    Delete { selector: String },

    #[command(about = "Clear a managed app's restart counters and breaker state")]
    Reset { selector: String },

    #[command(
        about = "Send a signal to a managed app's process group",
        long_about = "Send a signal to a managed app's process group. NAME is one of TERM, INT, QUIT, HUP, USR1, USR2 (case-insensitive)."
    )]
    Signal { selector: String, name: String },

    #[command(about = "Show everything known about one app")]
    Describe {
        selector: String,

        #[arg(long, help = "Print the description as JSON")]
        json: bool,
    },

    #[command(about = "List every managed app")]
    List {
        #[arg(long, help = "Print the listing as JSON")]
        json: bool,
    },

    #[command(about = "Show the logs of managed apps")]
    Logs {
        names: Vec<String>,

        #[arg(short = 'n', long)]
        lines: Option<usize>,

        #[arg(short = 'f', long)]
        follow: bool,

        #[arg(long, help = "Show the stderr log instead of stdout")]
        err: bool,

        #[arg(
            long,
            conflicts_with = "err",
            help = "Merge the stdout and stderr logs"
        )]
        all: bool,

        #[arg(
            long,
            conflicts_with_all = ["follow", "lines"],
            help = "Truncate the selected log files instead of showing them"
        )]
        clear: bool,
    },

    #[command(about = "Configuration management")]
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },

    #[command(about = "Manage the pm3 auto-start service")]
    Service {
        #[command(subcommand)]
        command: Option<ServiceCommands>,
    },

    #[command(
        about = "Install or upgrade pm3 itself",
        long_about = "Backs up the running install, swaps in SOURCE (the running binary by default), reinstalls the auto-start service, and verifies every managed service is reclaimed."
    )]
    Install {
        #[arg(value_name = "SOURCE", help = "New binary to install")]
        source: Option<PathBuf>,
    },

    #[command(
        about = "Stop the pm3 daemon",
        long_about = "Stops the pm3 daemon. Managed services keep running and are reclaimed by the next daemon unless --with-services is given."
    )]
    Kill {
        #[arg(long, help = "Stop every managed service before leaving")]
        with_services: bool,
    },

    #[command(about = "Run the pm3 daemon in the foreground")]
    Daemon,

    #[command(about = "Print the shell completion script for SHELL")]
    Completion { shell: clap_complete::Shell },

    #[command(name = "__sleep", hide = true, about = "Sleep, then exit cleanly")]
    Sleep { ms: u64 },
}

#[derive(Debug, Args)]
pub struct StartArgs {
    #[arg(long, help = "Manage one inline program under this name")]
    pub name: Option<String>,

    #[arg(long, help = "Working directory; defaults to <pm3 home>/<name>")]
    pub cwd: Option<String>,

    #[arg(
        long,
        value_name = "EXPR",
        help = "Five-field cron schedule; '~' picks a random value on every fire"
    )]
    pub cron: Option<String>,

    #[arg(
        long = "no-autorestart",
        help = "Do not restart the program when it exits"
    )]
    pub no_autorestart: bool,

    #[arg(long, help = "Allow the program to reach the network")]
    pub network: bool,

    #[arg(
        long = "writable-dir",
        value_name = "DIR",
        help = "Extra writable directory"
    )]
    pub writable_dirs: Vec<String>,

    #[arg(
        long = "readable-dir",
        value_name = "DIR",
        help = "Extra readable directory; only a confined read scope needs it"
    )]
    pub readable_dirs: Vec<String>,

    #[arg(
        long = "max-memory",
        value_name = "SIZE",
        help = "Restart the program when its resident memory grows past this size, e.g. 300M"
    )]
    pub max_memory: Option<String>,

    #[arg(
        long = "ready-exec",
        value_name = "CMD",
        help = "Probe readiness with this command; repeatable, first one is the program"
    )]
    pub ready_exec: Vec<String>,

    #[arg(
        long = "ready-tcp",
        value_name = "HOST:PORT",
        conflicts_with = "ready_exec",
        help = "Probe readiness by connecting to this endpoint"
    )]
    pub ready_tcp: Option<String>,

    #[arg(
        long = "listen-timeout",
        value_name = "MS",
        help = "Fail the service when it is not ready within this budget"
    )]
    pub listen_timeout_ms: Option<u64>,

    #[arg(
        long = "stop-exit-code",
        value_name = "CODE",
        help = "Treat this exit code as a clean stop; repeatable"
    )]
    pub stop_exit_codes: Vec<i32>,

    #[arg(long, help = "Overwrite an existing service file")]
    pub force: bool,

    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub target: Vec<String>,
}

#[derive(Debug, Subcommand)]
pub enum ServiceCommands {
    #[command(about = "Install the pm3 daemon as a user-level auto-start service")]
    Install {
        #[arg(long)]
        dry_run: bool,

        #[arg(long)]
        force: bool,
    },

    #[command(about = "Deactivate and remove the pm3 auto-start service")]
    Uninstall {
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommands {
    #[command(about = "Validate the configuration file")]
    Check,

    #[command(about = "Show the resolved configuration")]
    Show,
}

pub async fn dispatch(cli: Cli) -> Result<()> {
    let printed = execute(cli).await?;
    if let Some(output) = printed {
        emit(&output);
    }
    Ok(())
}

pub async fn execute(cli: Cli) -> Result<Option<String>> {
    let Cli { config, command } = cli;
    match command {
        Commands::Start(args) => run_start(&config, &args).await,
        Commands::Stop { selector } => act(&config, &selector, commands::STOP_ACTION).await,
        Commands::Restart { selector } => act(&config, &selector, commands::RESTART_ACTION).await,
        Commands::Delete { selector } => commands::delete_app(&config, &selector).await.map(Some),
        Commands::Reset { selector } => act(&config, &selector, commands::RESET_ACTION).await,
        Commands::Signal { selector, name } => commands::signal_app(&config, &selector, &name)
            .await
            .map(Some),
        Commands::Describe { selector, json } => commands::describe_app(&config, &selector, json)
            .await
            .map(Some),
        Commands::List { json } => commands::list_apps(&config, json).await.map(Some),
        Commands::Logs {
            names,
            lines,
            follow,
            err,
            all,
            clear,
        } => {
            let request = crate::logs::LogRequest {
                names,
                lines,
                err,
                all,
                follow,
                action: if clear {
                    crate::logs::LogAction::Clear
                } else {
                    crate::logs::LogAction::Show
                },
                polls: crate::logs::FOLLOW_FOREVER,
            };
            crate::logs::run_logs(&config, &request, &emit).await
        }
        Commands::Config { command } => run_config(&config, &command).map(Some),
        Commands::Service { command } => crate::service::run_service(&config, command.as_ref())
            .await
            .map(Some),
        Commands::Install { source } => crate::install::run(&config, source).await.map(|()| None),
        Commands::Kill { with_services } => commands::kill_daemon(&config, with_services)
            .await
            .map(Some),
        Commands::Daemon => crate::daemon::run_daemon(&config).await.map(|()| None),
        Commands::Completion { shell } => {
            print_completion(shell);
            Ok(None)
        }
        Commands::Sleep { ms } => {
            commands::sleep_for(ms).await;
            Ok(None)
        }
    }
}

fn print_completion(shell: clap_complete::Shell) {
    let mut command = Cli::command();
    clap_complete::generate(shell, &mut command, "pm3", &mut std::io::stdout());
}

#[must_use]
pub fn default_config(pm3_home_env: Option<&str>, home_env: Option<&str>) -> String {
    default_config_path(pm3_home_env, home_env).map_or_else(
        |_unresolved| CONFIG_FILE.to_string(),
        |path| path.to_string_lossy().into_owned(),
    )
}

async fn act(config: &str, selector: &str, action: &str) -> Result<Option<String>> {
    commands::act_on_app(config, selector, action)
        .await
        .map(Some)
}

async fn run_start(config: &str, args: &StartArgs) -> Result<Option<String>> {
    let report = match args.name.as_deref() {
        None => {
            let [apps_file] = args.target.as_slice() else {
                return Err(Error::InlineUsage {
                    reason: AMBIGUOUS_TARGET.to_string(),
                });
            };
            commands::start_apps(config, apps_file, args.force).await?
        }
        Some(name) => {
            if let Some(cron) = args.cron.as_deref() {
                validate_cron(name, cron)?;
            }
            let Some((program, rest)) = args.target.split_first() else {
                return Err(Error::InlineUsage {
                    reason: MISSING_COMMAND.to_string(),
                });
            };
            let request = InlineStart {
                name,
                program,
                args: rest,
                cwd: args.cwd.as_deref(),
                cron: args.cron.as_deref(),
                autorestart: args.no_autorestart.then_some(false),
                network: args.network,
                writable_dirs: &args.writable_dirs,
                readable_dirs: &args.readable_dirs,
                max_memory: args.max_memory.as_deref(),
                ready_exec: &args.ready_exec,
                ready_tcp: args.ready_tcp.as_deref(),
                listen_timeout_ms: args.listen_timeout_ms,
                stop_exit_codes: &args.stop_exit_codes,
                force: args.force,
            };
            commands::start_inline(config, &request).await?
        }
    };
    emit(&report.response);
    offer_restarts(config, &report, &mut confirm_via_stdio).await
}

async fn offer_restarts(
    config: &str,
    report: &commands::StartReport,
    confirm: &mut (dyn FnMut(&str) -> bool + Send),
) -> Result<Option<String>> {
    let pending = prompt::stale_running(&report.changed, &report.already_running);
    let mut lines: Vec<String> = Vec::new();
    for name in pending {
        if confirm(name) {
            lines.push(commands::act_on_app(config, name, commands::RESTART_ACTION).await?);
        } else {
            lines.push(prompt::keep_old_config_hint(name));
        }
    }
    Ok((!lines.is_empty()).then(|| lines.join("\n")))
}

fn confirm_via_stdio(name: &str) -> bool {
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    prompt::confirm_restart(name, &mut input, &mut output)
}

fn run_config(config: &str, command: &ConfigCommands) -> Result<String> {
    match command {
        ConfigCommands::Check => commands::check_config(config),
        ConfigCommands::Show => commands::show_config(config),
    }
}

#[must_use]
pub fn report(outcome: Result<()>) -> std::process::ExitCode {
    match outcome {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            emit_error(&error);
            std::process::ExitCode::FAILURE
        }
    }
}

#[expect(clippy::print_stdout, reason = "CLI command output")]
fn emit(output: &str) {
    println!("{output}");
}

#[expect(clippy::print_stderr, reason = "CLI error output")]
fn emit_error(error: &Error) {
    eprintln!("{error}");
}

#[cfg(test)]
#[path = "tests/cli_tests.rs"]
mod tests;
