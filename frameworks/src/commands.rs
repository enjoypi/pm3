use std::{path::Path, time::Duration};

use adapters::{
    APPS_PATH, AppConfig, LogFollower, Pm3Paths, StartRequestDto, load_and_parse_config, log_paths,
    logs_dir_of, read_tail,
};

use crate::{
    Error, Result,
    client::{OK_STATUS, UdsClient},
    daemon::{DaemonLaunch, ensure_daemon_running},
    layout::{ensure_layout, host_home, resolve_layout},
};

pub const FOLLOW_FOREVER: u32 = u32::MAX;

const FOLLOW_INTERVAL_MS: u64 = 200;
const STOP_ACTION: &str = "stop";
const RESTART_ACTION: &str = "restart";

#[derive(Clone, Debug)]
pub struct Session {
    pub config: AppConfig,
    pub paths: Pm3Paths,
}

pub fn open_session(config_path: &str) -> Result<Session> {
    let config = load_and_parse_config(config_path)?;
    let paths = resolve_layout(&config.pm3, host_home().as_deref())?;
    Ok(Session { config, paths })
}

pub async fn start_apps(config_path: &str, apps_file: &str) -> Result<String> {
    let body = start_body(&canonical_apps_file(apps_file)?);
    ask(config_path, "POST", APPS_PATH, Some(&body)).await
}

pub async fn list_apps(config_path: &str) -> Result<String> {
    ask(config_path, "GET", APPS_PATH, None).await
}

pub async fn describe_app(config_path: &str, selector: &str) -> Result<String> {
    ask(config_path, "GET", &app_path(selector), None).await
}

pub async fn stop_app(config_path: &str, selector: &str) -> Result<String> {
    ask(
        config_path,
        "POST",
        &app_action(selector, STOP_ACTION),
        None,
    )
    .await
}

pub async fn restart_app(config_path: &str, selector: &str) -> Result<String> {
    ask(
        config_path,
        "POST",
        &app_action(selector, RESTART_ACTION),
        None,
    )
    .await
}

pub async fn delete_app(config_path: &str, selector: &str) -> Result<String> {
    ask(config_path, "DELETE", &app_path(selector), None).await
}

pub async fn read_log_tail(config_path: &str, name: &str, lines: usize) -> Result<String> {
    let stdout = stdout_log(&open_session(config_path)?.paths, name);
    let tail = read_tail(Path::new(&stdout), lines).await?;
    Ok(tail.join("\n"))
}

pub async fn follow_log(
    config_path: &str,
    name: &str,
    polls: u32,
    emit: &(dyn Fn(&str) + Send + Sync),
) -> Result<()> {
    let stdout = stdout_log(&open_session(config_path)?.paths, name);
    let mut follower = LogFollower::start_at_end(Path::new(&stdout)).await?;
    for _poll in 0..polls {
        for line in follower.poll_appended().await? {
            emit(&line);
        }
        tokio::time::sleep(Duration::from_millis(FOLLOW_INTERVAL_MS)).await;
    }
    Ok(())
}

pub fn check_config(config_path: &str) -> Result<String> {
    Ok(adapters::check_config(config_path)?)
}

pub fn show_config(config_path: &str) -> Result<String> {
    Ok(adapters::show_config(config_path)?)
}

pub async fn sleep_for(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

#[must_use]
pub fn stdout_log(paths: &Pm3Paths, name: &str) -> String {
    log_paths(&logs_dir_of(&paths.root), name).stdout
}

pub fn canonical_apps_file(apps_file: &str) -> Result<String> {
    let resolved = std::fs::canonicalize(apps_file).map_err(|e| Error::AppsFile {
        path: apps_file.to_string(),
        reason: e.to_string(),
    })?;
    Ok(resolved.to_string_lossy().into_owned())
}

#[must_use]
pub fn start_body(apps_file: &str) -> String {
    let request = StartRequestDto {
        apps_file: apps_file.to_string(),
    };
    serde_json::to_string(&request)
        .expect("internal error: StartRequestDto serialization is infallible")
}

async fn ask(config_path: &str, method: &str, path: &str, body: Option<&str>) -> Result<String> {
    let session = open_session(config_path)?;
    ensure_layout(&session.paths).await?;
    let program = std::env::current_exe().unwrap_or_default();
    let launch =
        DaemonLaunch::from_config(&session.paths, config_path, program, &session.config.pm3);
    ensure_daemon_running(&launch).await?;
    let reply = UdsClient::new(session.paths.socket.clone())
        .request(method, path, body)
        .await?;
    if reply.status != OK_STATUS {
        return Err(Error::Refused {
            status: reply.status,
            body: reply.body,
        });
    }
    Ok(reply.body)
}

fn app_path(selector: &str) -> String {
    format!("{APPS_PATH}/{selector}")
}

fn app_action(selector: &str, action: &str) -> String {
    format!("{APPS_PATH}/{selector}/{action}")
}

#[cfg(test)]
#[path = "tests/commands_tests.rs"]
mod tests;
