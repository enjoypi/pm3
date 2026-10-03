use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

use adapters::{LogStream, Pm3Paths, Pm3Roots, log_path, portable_path, resolve_paths};
#[cfg(windows)]
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use tokio::{
    io::{AsyncRead, AsyncReadExt as _, AsyncWrite, AsyncWriteExt as _},
    sync::oneshot,
    task::JoinHandle,
};

use crate::{
    Result,
    client::UdsClient,
    daemon::service::run_daemon_with_shutdown,
    platform::{SHELL, SHELL_FLAG, SLEEPER},
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
    let home = PathBuf::from(portable_path(&dir.path().join("home").to_string_lossy()));
    let paths = resolve_paths(Pm3Roots::single(&home));
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
        "apps:\n  - name: web\n    script: '{SHELL}'\n    cwd: '{cwd}'\n    args:\n      - \"{SHELL_FLAG}\"\n      - '{SLEEPER}'\n"
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

pub const REPLY_200: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok";
const REPLY_500: &[u8] = b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 4\r\n\r\noops";
const REQUEST_SINK: usize = 1024;

pub trait Duplex: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Duplex for T {}

#[cfg(unix)]
pub struct FakeListener(tokio::net::UnixListener);

#[cfg(unix)]
impl FakeListener {
    pub fn bind(socket: &Path) -> impl Future<Output = Self> {
        std::future::ready(Self(
            tokio::net::UnixListener::bind(socket).expect("bind the fake daemon"),
        ))
    }

    #[expect(
        clippy::needless_pass_by_ref_mut,
        reason = "a named pipe listener swaps its pending instance, so both platforms share one signature"
    )]
    pub async fn accept(&mut self) -> Box<dyn Duplex> {
        let (stream, _addr) = self.0.accept().await.expect("accept a request");
        Box::new(stream)
    }
}

#[cfg(windows)]
pub struct FakeListener {
    name: String,
    next: NamedPipeServer,
}

#[cfg(windows)]
impl FakeListener {
    pub async fn bind(socket: &Path) -> Self {
        let secret = crate::layout::pipe_secret(socket)
            .await
            .expect("read the pipe secret");
        let name = crate::layout::pipe_name_of(socket, &secret);
        let next = ServerOptions::new()
            .first_pipe_instance(true)
            .create(&name)
            .expect("bind the fake daemon");
        Self { name, next }
    }

    pub async fn accept(&mut self) -> Box<dyn Duplex> {
        self.next.connect().await.expect("accept a request");
        let fresh = ServerOptions::new()
            .create(&self.name)
            .expect("open the next pipe instance");
        Box::new(std::mem::replace(&mut self.next, fresh))
    }
}

pub async fn answer(stream: &mut dyn Duplex, reply: &[u8]) {
    let mut sink = vec![0_u8; REQUEST_SINK];
    let read = stream.read(&mut sink).await.unwrap_or_default();
    sink.truncate(read);
    stream.write_all(reply).await.ok();
    stream.shutdown().await.ok();
}

pub async fn answer_only_the_health_probe(socket: PathBuf) -> JoinHandle<()> {
    let mut listener = FakeListener::bind(&socket).await;
    tokio::spawn(async move {
        answer(listener.accept().await.as_mut(), REPLY_200).await;
        drop(listener);
        std::fs::remove_file(&socket).ok();
    })
}

pub async fn answer_health_then_refusal(socket: &Path) -> JoinHandle<()> {
    let mut listener = FakeListener::bind(socket).await;
    tokio::spawn(async move {
        for reply in [REPLY_200, REPLY_200, REPLY_500] {
            answer(listener.accept().await.as_mut(), reply).await;
        }
    })
}

#[expect(
    clippy::infinite_loop,
    reason = "a fake daemon answers until its test aborts the task"
)]
pub async fn keep_answering(mut listener: FakeListener, reply: &'static [u8]) {
    loop {
        answer(listener.accept().await.as_mut(), reply).await;
    }
}
