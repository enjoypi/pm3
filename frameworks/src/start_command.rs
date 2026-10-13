use adapters::{InlineStart, validate_cron};

use crate::{
    Error, Result,
    cli::{StartArgs, emit},
    commands, prompt,
};

const MISSING_COMMAND: &str = "--name needs a program to run after it";
const AMBIGUOUS_TARGET: &str =
    "without --name, start takes exactly one apps file; pm3 options must come before the program";

pub async fn run_start(config: &str, args: &StartArgs) -> Result<Option<String>> {
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
            };
            commands::start_inline(config, &request, args.force).await?
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
            lines.push(commands::act_on_app(config, name, adapters::RESTART_ACTION).await?);
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

#[cfg(test)]
#[path = "tests/start_command_tests.rs"]
mod tests;
