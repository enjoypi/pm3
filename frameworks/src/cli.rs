use clap::{Args, Parser, Subcommand};

use crate::{
    Error, Result, commands,
    svc::{AMBIGUOUS_TARGET, InlineStart},
};

pub const DEFAULT_CONFIG: &str = "config.yaml";
pub const DEFAULT_LOG_LINES: usize = 20;

#[derive(Debug, Parser)]
#[command(
    name = "pm3",
    version,
    about = "极简进程管理器，每个服务跑在严格沙盒中"
)]
pub struct Cli {
    #[arg(long, global = true, default_value = DEFAULT_CONFIG)]
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

        #[arg(short = 'n', long, default_value_t = DEFAULT_LOG_LINES)]
        lines: usize,

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

    #[arg(long = "env", value_name = "KEY=VALUE", help = "Environment entry")]
    pub env: Vec<String>,

    #[arg(long, help = "Allow the program to reach the network")]
    pub network: bool,

    #[arg(
        long = "writable-dir",
        value_name = "DIR",
        help = "Extra writable directory"
    )]
    pub writable_dirs: Vec<String>,

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
        Commands::Start(args) => run_start(&config, &args).await.map(Some),
        Commands::Stop { selector } => commands::stop_app(&config, &selector).await.map(Some),
        Commands::Restart { selector } => commands::restart_app(&config, &selector).await.map(Some),
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
        Commands::Daemon => crate::daemon::run_daemon(&config).await.map(|()| None),
        Commands::Sleep { ms } => {
            commands::sleep_for(ms).await;
            Ok(None)
        }
    }
}

async fn run_start(config: &str, args: &StartArgs) -> Result<String> {
    let Some(name) = args.name.as_deref() else {
        let [apps_file] = args.target.as_slice() else {
            return Err(Error::InlineUsage {
                reason: AMBIGUOUS_TARGET.to_string(),
            });
        };
        return commands::start_apps(config, apps_file, args.force).await;
    };
    let request = InlineStart {
        name,
        target: &args.target,
        cwd: args.cwd.as_deref(),
        env: &args.env,
        network: args.network,
        writable_dirs: &args.writable_dirs,
        force: args.force,
    };
    commands::start_inline(config, &request).await
}

async fn run_logs(
    config: &str,
    name: &str,
    lines: usize,
    follow: bool,
    polls: u32,
) -> Result<Option<String>> {
    let tail = commands::read_log_tail(config, name, lines).await?;
    if !follow {
        return Ok(Some(tail));
    }
    emit(&tail);
    commands::follow_log(config, name, polls, &emit).await?;
    Ok(None)
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
