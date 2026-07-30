use clap::{Parser, Subcommand};

use crate::{Result, commands};

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
    #[command(about = "Start every app declared in an apps file")]
    Start { apps_file: String },

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

    #[command(about = "Run the pm3 daemon in the foreground")]
    Daemon,

    #[command(name = "__sleep", hide = true, about = "Sleep, then exit cleanly")]
    Sleep { ms: u64 },
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
        Commands::Start { apps_file } => commands::start_apps(&config, &apps_file).await.map(Some),
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
        Commands::Daemon => crate::daemon::run_daemon(&config).await.map(|()| None),
        Commands::Sleep { ms } => {
            commands::sleep_for(ms).await;
            Ok(None)
        }
    }
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

#[expect(clippy::print_stdout, reason = "CLI command output")]
fn emit(output: &str) {
    println!("{output}");
}

#[cfg(test)]
#[path = "tests/cli_tests.rs"]
mod tests;
