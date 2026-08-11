#![cfg(unix)]
use std::{path::PathBuf, sync::Mutex, time::Duration};

use adapters::{LogStream, Pm3Paths, log_path, resolve_paths};
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    sync::oneshot,
    task::JoinHandle,
};

use crate::{
    Result,
    client::UdsClient,
    daemon::run_daemon_with_shutdown,
    test_support::{REQUEST_TIMEOUT_MS, write_apps_file, write_config},
};

const PROBE_INTERVAL: Duration = Duration::from_millis(20);

pub struct Fixture {
    pub dir: tempfile::TempDir,
    pub paths: Pm3Paths,
    pub config_path: String,
    pub shutdown: oneshot::Sender<()>,
    pub daemon: JoinHandle<Result<()>>,
}

pub async fn running_daemon() -> Fixture {
    let dir = tempfile::tempdir().expect("temp dir");
    let home = dir.path().join("home");
    let paths = resolve_paths(&home);
    let config_path = write_config(dir.path(), &home.to_string_lossy())
        .to_string_lossy()
        .into_owned();
    let (shutdown, wait) = oneshot::channel::<()>();
    let spawned = config_path.clone();
    let daemon = tokio::spawn(async move {
        run_daemon_with_shutdown(
            &spawned,
            Box::pin(async move {
                wait.await.ok();
            }),
        )
        .await
    });

    let client = UdsClient::new(paths.socket.clone(), REQUEST_TIMEOUT_MS);
    loop {
        if client.daemon_is_healthy().await {
            break;
        }
        tokio::time::sleep(PROBE_INTERVAL).await;
    }
    Fixture {
        dir,
        paths,
        config_path,
        shutdown,
        daemon,
    }
}

pub async fn stop_daemon(fixture: Fixture) {
    let Fixture {
        dir,
        paths: _paths,
        config_path: _config_path,
        shutdown,
        daemon,
    } = fixture;
    shutdown.send(()).expect("signal shutdown");
    daemon.await.expect("join").expect("serve ok");
    drop(dir);
}

pub fn sleeper_apps_file(fixture: &Fixture) -> String {
    let cwd = fixture.paths.root.to_string_lossy();
    let body = format!(
        "apps:\n  - name: web\n    script: /bin/sh\n    cwd: \"{cwd}\"\n    args:\n      - \"-c\"\n      - \"sleep 30\"\n"
    );
    write_apps_file(fixture.dir.path(), &body)
        .to_string_lossy()
        .into_owned()
}

pub fn seed_log(fixture: &Fixture, name: &str, stream: LogStream, content: &str) -> String {
    let path = log_path(&fixture.paths.logs_dir.to_string_lossy(), name, stream);
    std::fs::create_dir_all(&fixture.paths.logs_dir).expect("create the log directory");
    std::fs::write(&path, content).expect("seed the log");
    path
}

#[derive(Debug, Default)]
pub struct Collected {
    lines: Mutex<Vec<String>>,
}

impl Collected {
    pub fn push(&self, line: &str) {
        self.lines
            .lock()
            .expect("the collector lock stays healthy")
            .push(line.to_string());
    }

    pub fn taken(&self) -> Vec<String> {
        self.lines
            .lock()
            .expect("the collector lock stays healthy")
            .clone()
    }
}

const REPLY_200: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok";
const REPLY_500: &[u8] = b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 4\r\n\r\noops";
const REQUEST_SINK: usize = 1024;

pub fn answer_only_the_health_probe(socket: PathBuf) -> JoinHandle<()> {
    let listener = tokio::net::UnixListener::bind(&socket).expect("bind the probe answerer");
    tokio::spawn(async move {
        let (mut stream, _addr) = listener.accept().await.expect("accept the probe");
        let mut sink = vec![0_u8; REQUEST_SINK];
        let read = stream.read(&mut sink).await.unwrap_or_default();
        sink.truncate(read);
        stream.write_all(REPLY_200).await.ok();
        stream.shutdown().await.ok();
        drop(listener);
        std::fs::remove_file(&socket).ok();
    })
}

pub fn answer_health_then_refusal(socket: &std::path::Path) -> JoinHandle<()> {
    let listener = tokio::net::UnixListener::bind(socket).expect("bind the answerer");
    tokio::spawn(async move {
        for reply in [REPLY_200, REPLY_200, REPLY_500] {
            let (mut stream, _addr) = listener.accept().await.expect("accept a request");
            let mut sink = vec![0_u8; REQUEST_SINK];
            let read = stream.read(&mut sink).await.unwrap_or_default();
            sink.truncate(read);
            stream.write_all(reply).await.ok();
            stream.shutdown().await.ok();
        }
    })
}
