use std::path::PathBuf;

use adapters::{CONFIG_FILE, InlineStart, default_config_path, validate_cron};
use clap::{Args, Parser, Subcommand};

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

    #[command(about = "Show everything known about one app")]
    Describe { selector: String },

    #[command(about = "List every managed app")]
    List,

    #[command(about = "Show the stdout log of a managed app")]
    Logs {
        name: String,

        #[arg(short = 'n', long)]
        lines: Option<usize>,

        #[arg(short = 'f', long)]
        follow: bool,
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
        Commands::Stop { selector } => {
            commands::act_on_app(&config, &selector, commands::STOP_ACTION)
                .await
                .map(Some)
        }
        Commands::Restart { selector } => {
            commands::act_on_app(&config, &selector, commands::RESTART_ACTION)
                .await
                .map(Some)
        }
        Commands::Delete { selector } => commands::delete_app(&config, &selector).await.map(Some),
        Commands::Describe { selector } => {
            commands::describe_app(&config, &selector).await.map(Some)
        }
        Commands::List => commands::list_apps(&config).await.map(Some),
        Commands::Logs {
            name,
            lines,
            follow,
        } => run_logs(&config, &name, lines, follow, commands::FOLLOW_FOREVER).await,
        Commands::Config { command } => run_config(&config, &command).map(Some),
        Commands::Service { command } => crate::service::run_service(&config, command.as_ref())
            .await
            .map(Some),
        Commands::Install { source } => crate::install::run(&config, source).await.map(|()| None),
        Commands::Kill { with_services } => commands::kill_daemon(&config, with_services)
            .await
            .map(Some),
        Commands::Daemon => crate::daemon::run_daemon(&config).await.map(|()| None),
        Commands::Sleep { ms } => {
            commands::sleep_for(ms).await;
            Ok(None)
        }
    }
}

#[must_use]
pub fn default_config(pm3_home_env: Option<&str>, home_env: Option<&str>) -> String {
    default_config_path(pm3_home_env, home_env).map_or_else(
        |_unresolved| CONFIG_FILE.to_string(),
        |path| path.to_string_lossy().into_owned(),
    )
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

async fn run_logs(
    config: &str,
    name: &str,
    lines: Option<usize>,
    follow: bool,
    polls: u32,
) -> Result<Option<String>> {
    let tail = commands::read_log_tail(config, name, lines).await?;
    if !follow {
        return Ok(Some(tail));
    }
    emit(&tail);
    commands::follow_log(config, name, polls, &emit)
        .await
        .map(|()| None)
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
