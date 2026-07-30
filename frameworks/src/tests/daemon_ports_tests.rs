use std::path::Path;

use adapters::{AppSpec, ProcessRuntime, SandboxMode, SpecSource, service_file_of};

use super::*;
use crate::test_support::pm3_config_with_home;

const EPOCH_2023_MS: u64 = 1_700_000_000_000;
const POLL_MS: u64 = 10;

fn unconfined_policy() -> SandboxPolicy {
    SandboxPolicy {
        mode: SandboxMode::DangerFullAccess,
        network: true,
        writable_roots: Vec::new(),
    }
}

fn ports_in(dir: &Path) -> DaemonPorts {
    DaemonPorts::new(dir.join("dump.yaml"), spec_source_in(dir), None, POLL_MS)
}

fn spec_source_in(dir: &Path) -> SpecSource {
    let cfg_dir = dir.join("svc");
    std::fs::create_dir_all(&cfg_dir).expect("create the service directory");
    SpecSource {
        cfg_dir,
        config: pm3_config_with_home(&dir.to_string_lossy()),
        home_dir: dir.to_string_lossy().into_owned(),
        logs_dir: dir.join("logs").to_string_lossy().into_owned(),
        tmp_dir: None,
    }
}

fn register_service(dir: &Path, name: &str) {
    std::fs::write(
        service_file_of(&dir.join("svc"), name),
        format!("apps:\n  - name: {name}\n    script: /bin/echo\n"),
    )
    .expect("write the service file");
}

fn launch_spec(dir: &Path, program: &str, args: &[&str]) -> LaunchSpec {
    LaunchSpec {
        name: "web".to_string(),
        program: program.to_string(),
        args: args.iter().map(|arg| (*arg).to_string()).collect(),
        cwd: dir.to_string_lossy().into_owned(),
        env: Vec::new(),
        stdout_path: dir.join("web-out.log").to_string_lossy().into_owned(),
        stderr_path: dir.join("web-err.log").to_string_lossy().into_owned(),
    }
}

fn stored_record() -> ProcessRecord {
    ProcessRecord {
        spec: AppSpec {
            name: "web".to_string(),
            script: "/bin/echo".to_string(),
            args: Vec::new(),
            cwd: "/tmp".to_string(),
            env: Vec::new(),
            autorestart: true,
            min_uptime_ms: 1000,
            max_restarts: 15,
            restart_delay_ms: 0,
            depends_on: Vec::new(),
            sandbox: unconfined_policy(),
        },
        runtime: ProcessRuntime::new(0, "web".to_string(), EPOCH_2023_MS),
    }
}

#[tokio::test]
async fn the_clock_reports_wall_clock_time() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert!(ports_in(dir.path()).now_ms() > EPOCH_2023_MS);
}

#[tokio::test]
async fn an_unconfined_app_is_wrapped_by_nothing() {
    let dir = tempfile::tempdir().expect("temp dir");
    let wrapped = ports_in(dir.path())
        .wrap("web", &unconfined_policy(), "/bin/echo", &[])
        .expect("should wrap");
    assert_eq!(wrapped.program, "/bin/echo");
}

#[tokio::test]
async fn the_dump_store_rejoins_the_saved_state_with_its_service_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let ports = ports_in(dir.path());
    register_service(dir.path(), "web");
    ports.save(&[stored_record()]).await.expect("save");
    let loaded = ports.load().await.expect("load");
    assert_eq!(loaded[0].runtime, stored_record().runtime);
}

#[tokio::test]
async fn a_spawned_child_is_tracked_and_reaped() {
    let dir = tempfile::tempdir().expect("temp dir");
    let ports = ports_in(dir.path());
    let spec = launch_spec(dir.path(), "/bin/echo", &["hello"]);
    let process = ports.spawn(&spec).await.expect("spawn");
    assert_eq!(ports.tracked_pids().await, vec![process.pid]);
    let outcome = ports.wait(process.pid).await.expect("reap");
    assert!(outcome.clean(), "got: {outcome:?}");
    assert!(ports.tracked_pids().await.is_empty());
}

#[tokio::test]
async fn terminating_a_child_stops_it() {
    let dir = tempfile::tempdir().expect("temp dir");
    let ports = ports_in(dir.path());
    let spec = launch_spec(dir.path(), "/bin/sh", &["-c", "sleep 30"]);
    let process = ports.spawn(&spec).await.expect("spawn");
    ports.terminate(process.pid).await.expect("terminate");
    let outcome = ports.wait(process.pid).await.expect("reap");
    assert_eq!(outcome.exit_code, None);
}

#[tokio::test]
async fn force_killing_a_child_stops_it() {
    let dir = tempfile::tempdir().expect("temp dir");
    let ports = ports_in(dir.path());
    let spec = launch_spec(dir.path(), "/bin/sh", &["-c", "sleep 30"]);
    let process = ports.spawn(&spec).await.expect("spawn");
    ports.force_kill(process.pid).await.expect("force kill");
    let outcome = ports.wait(process.pid).await.expect("reap");
    assert_eq!(outcome.exit_code, None);
}
