use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use adapters::{
    APPS_PATH, AppConfig, KillSignaler, LogFollower, Pm3Paths, ReplyDto, SERVICES_STOP_ALL_PATH,
    Signaler as _, StartRequestDto, load_and_parse_config, log_paths, logs_dir_of, read_tail,
    wait_until_released,
};

use crate::{
    Error, Result,
    client::{OK_STATUS, UdsClient},
    daemon::{DaemonLaunch, ensure_daemon_running},
    layout::{
        canonicalize, ensure_layout, host_home, read_pid_file, resolve_cfg_dir, resolve_layout,
    },
    svc::{self, InlineStart, Reconciled, SvcContext},
};

pub const FOLLOW_FOREVER: u32 = u32::MAX;
pub const DAEMON_NOT_RUNNING: &str = "the pm3 daemon is not running";

const STOP_ACTION: &str = "stop";
const RESTART_ACTION: &str = "restart";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartReport {
    pub response: String,
    pub changed: Vec<String>,
    pub already_running: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Session {
    pub config: AppConfig,
    pub paths: Pm3Paths,
    pub cfg_dir: PathBuf,
}

impl Session {
    #[must_use]
    pub fn svc_context<'s>(&'s self, home: Option<&'s str>) -> SvcContext<'s> {
        SvcContext {
            cfg_dir: &self.cfg_dir,
            search_path: &self.config.pm3.search_path,
            home,
        }
    }
}

pub fn open_session(config_path: &str) -> Result<Session> {
    let config = load_and_parse_config(config_path)?;
    let home = host_home();
    let paths = resolve_layout(&config.pm3, home.as_deref())?;
    let cfg_dir = resolve_cfg_dir(&config.pm3, home.as_deref())?;
    Ok(Session {
        config,
        paths,
        cfg_dir,
    })
}

pub async fn start_apps(config_path: &str, apps_file: &str, force: bool) -> Result<StartReport> {
    let session = open_session(config_path)?;
    ensure_layout(&session.paths, &session.cfg_dir).await?;
    let resolved = canonical_apps_file(apps_file)?;
    let home = host_home();
    let split =
        svc::split_apps_file(&session.svc_context(home.as_deref()), &resolved, force).await?;
    let asked = ask(config_path, "POST", APPS_PATH, Some(&start_body(&resolved))).await;
    finish_start(asked, split.changed, &split.undo).await
}

pub async fn start_inline(config_path: &str, request: &InlineStart<'_>) -> Result<StartReport> {
    let session = open_session(config_path)?;
    ensure_layout(&session.paths, &session.cfg_dir).await?;
    let home = host_home();
    let prepared = svc::prepare_inline(&session.svc_context(home.as_deref()), request).await?;
    let body = start_body(&prepared.path.to_string_lossy());
    let changed = if prepared.reconciled == Reconciled::Stale {
        vec![request.name.to_string()]
    } else {
        Vec::new()
    };
    let asked = ask(config_path, "POST", APPS_PATH, Some(&body)).await;
    finish_start(asked, changed, &prepared.undo).await
}

async fn finish_start(
    asked: Result<ReplyDto>,
    changed: Vec<String>,
    undo: &svc::SvcUndo,
) -> Result<StartReport> {
    let reply = match asked {
        Ok(reply) => reply,
        Err(error) => {
            undo.run().await;
            return Err(error);
        }
    };
    let ReplyDto {
        report,
        service: _,
        already_running,
    } = reply;
    Ok(StartReport {
        response: report,
        changed,
        already_running,
    })
}

pub async fn list_apps(config_path: &str) -> Result<String> {
    ask_report(config_path, "GET", APPS_PATH, None).await
}

pub async fn describe_app(config_path: &str, selector: &str) -> Result<String> {
    ask_report(config_path, "GET", &app_path(selector), None).await
}

pub async fn stop_app(config_path: &str, selector: &str) -> Result<String> {
    ask_report(
        config_path,
        "POST",
        &app_action(selector, STOP_ACTION),
        None,
    )
    .await
}

pub async fn restart_app(config_path: &str, selector: &str) -> Result<String> {
    ask_report(
        config_path,
        "POST",
        &app_action(selector, RESTART_ACTION),
        None,
    )
    .await
}

pub async fn delete_app(config_path: &str, selector: &str) -> Result<String> {
    let cfg_dir = open_session(config_path)?.cfg_dir;
    let deleted = ask(config_path, "DELETE", &app_path(selector), None).await?;
    svc::forget(&cfg_dir, deleted.service.as_deref().unwrap_or(selector)).await;
    Ok(deleted.report)
}

pub async fn kill_daemon(config_path: &str, with_services: bool) -> Result<String> {
    let session = open_session(config_path)?;
    let pm3 = &session.config.pm3;
    let client = UdsClient::new(session.paths.socket.clone(), pm3.request_timeout_ms);
    if !client.daemon_is_healthy().await {
        return Ok(DAEMON_NOT_RUNNING.to_string());
    }
    let stopped = if with_services {
        Some(ask_report(config_path, "POST", SERVICES_STOP_ALL_PATH, None).await?)
    } else {
        None
    };
    let Some(pid) = read_pid_file(&session.paths).await else {
        return Err(Error::DaemonPidUnknown {
            path: session.paths.pid_file.to_string_lossy().into_owned(),
        });
    };
    KillSignaler::default().terminate(pid).await?;
    let budget_ms = pm3.start_timeout_ms;
    if !wait_until_released(
        &session.paths.socket,
        budget_ms,
        pm3.daemon_poll_interval_ms,
    )
    .await
    {
        return Err(Error::DaemonLingering {
            pid,
            path: session.paths.socket.to_string_lossy().into_owned(),
            timeout_ms: budget_ms,
        });
    }
    Ok(kill_report(stopped.as_deref(), pid))
}

fn kill_report(stopped: Option<&str>, pid: u32) -> String {
    let farewell = format!("stopped the pm3 daemon (pid {pid})");
    stopped.map_or_else(
        || format!("{farewell}; managed services keep running"),
        |services| format!("{services}\n{farewell}"),
    )
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
    let session = open_session(config_path)?;
    let interval = Duration::from_millis(session.config.pm3.log_follow_interval_ms);
    let stdout = stdout_log(&session.paths, name);
    let mut follower = LogFollower::start_at_end(Path::new(&stdout)).await?;
    for _poll in 0..polls {
        for line in follower.poll_appended().await? {
            emit(&line);
        }
        tokio::time::sleep(interval).await;
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
    let resolved = canonicalize(apps_file, |reason| Error::AppsFile {
        path: apps_file.to_string(),
        reason,
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

async fn ask_report(
    config_path: &str,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> Result<String> {
    ask(config_path, method, path, body)
        .await
        .map(|reply| reply.report)
}

async fn ask(config_path: &str, method: &str, path: &str, body: Option<&str>) -> Result<ReplyDto> {
    let session = open_session(config_path)?;
    ensure_layout(&session.paths, &session.cfg_dir).await?;
    let program = std::env::current_exe().unwrap_or_default();
    let launch =
        DaemonLaunch::from_config(&session.paths, config_path, program, &session.config.pm3);
    ensure_daemon_running(&launch).await?;
    let reply = UdsClient::new(
        session.paths.socket.clone(),
        session.config.pm3.request_timeout_ms,
    )
    .request(method, path, body)
    .await?;
    if reply.status != OK_STATUS {
        return Err(Error::Refused {
            status: reply.status,
            body: reply.body,
        });
    }
    decode_reply(&reply.body)
}

fn decode_reply(body: &str) -> Result<ReplyDto> {
    serde_json::from_str(body).map_err(|e| Error::Undecodable {
        reason: e.to_string(),
    })
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
