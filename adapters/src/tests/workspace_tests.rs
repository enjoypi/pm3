#![cfg(unix)]
use std::path::Path;

use usecases::{PolicyError, ReadScope, SandboxMode, SandboxPolicy};

use super::*;

fn spec_with_args(cwd: &str, args: &[&str]) -> AppSpec {
    AppSpec {
        args: args.iter().map(|arg| (*arg).to_string()).collect(),
        ..spec_at(cwd, Vec::new())
    }
}

fn spec_at(cwd: &str, writable_roots: Vec<String>) -> AppSpec {
    AppSpec {
        max_memory_kib: None,
        ready_probe: None,
        listen_timeout_ms: None,
        stop_exit_codes: Vec::new(),
        name: "web".to_string(),
        script: "/bin/sh".to_string(),
        args: Vec::new(),
        cwd: cwd.to_string(),
        env: Vec::new(),
        autorestart: true,
        min_uptime_ms: 1000,
        max_restarts: 15,
        restart_delay_ms: 0,
        max_restart_delay_ms: 15000,
        schedule: None,
        depends_on: Vec::new(),
        sandbox: SandboxPolicy {
            mode: SandboxMode::WorkspaceWrite,
            read: ReadScope::Minimal,
            network: false,
            writable_roots,
            readable_roots: Vec::new(),
            derived_roots: Vec::new(),
            unreadable_roots: Vec::new(),
        },
    }
}

fn linked_dir(root: &Path) -> (String, String) {
    let real = root.join("real");
    std::fs::create_dir_all(&real).expect("create the real directory");
    let link = root.join("link");
    std::os::unix::fs::symlink(&real, &link).expect("link to the real directory");
    (
        link.to_string_lossy().into_owned(),
        real.canonicalize()
            .expect("canonical real directory")
            .to_string_lossy()
            .into_owned(),
    )
}

#[tokio::test]
async fn a_writable_root_resolving_into_a_hidden_root_is_refused() {
    let dir = tempfile::tempdir().expect("temp dir");
    let hidden = dir.path().join("home");
    std::fs::create_dir_all(&hidden).expect("create the hidden directory");
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&hidden, &link).expect("link into the hidden directory");
    let cwd = dir.path().join("web");
    let mut spec = spec_at(
        &cwd.to_string_lossy(),
        vec![link.to_string_lossy().into_owned()],
    );
    spec.sandbox.unreadable_roots = vec![
        hidden
            .canonicalize()
            .expect("canonical hidden directory")
            .to_string_lossy()
            .into_owned(),
    ];
    let error = materialise_workspace(&mut spec)
        .await
        .expect_err("a symlink resolving into a hidden root must be refused");
    assert!(
        matches!(error, PolicyError::WritableRootCoversHiddenRoot { .. }),
        "got: {error}"
    );
}

#[tokio::test]
async fn a_hidden_root_is_resolved_to_its_real_path() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (link, real) = linked_dir(dir.path());
    let cwd = dir.path().join("web");
    let mut spec = spec_at(&cwd.to_string_lossy(), Vec::new());
    spec.sandbox.unreadable_roots = vec![link];
    materialise_workspace(&mut spec)
        .await
        .expect("the workspace should materialise");
    assert_eq!(spec.sandbox.unreadable_roots, vec![real]);
}

#[tokio::test]
async fn a_missing_working_directory_is_created() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cwd = dir.path().join("web");
    let mut spec = spec_at(&cwd.to_string_lossy(), Vec::new());
    materialise_workspace(&mut spec)
        .await
        .expect("materialise should succeed");
    assert!(cwd.is_dir(), "the working directory should exist");
}

#[tokio::test]
async fn a_working_directory_blocked_by_a_file_is_left_unresolved() {
    let dir = tempfile::tempdir().expect("temp dir");
    let blocked = dir.path().join("web");
    std::fs::write(&blocked, "occupied").expect("occupy the working directory path");
    let mut spec = spec_at(&blocked.to_string_lossy(), Vec::new());
    materialise_workspace(&mut spec)
        .await
        .expect("materialise should succeed");
    assert!(
        Path::new(&spec.cwd).is_file(),
        "the blocked path must not silently become a directory"
    );
}

#[tokio::test]
async fn the_working_directory_is_resolved_to_its_real_path() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (link, real) = linked_dir(dir.path());
    let mut spec = spec_at(&link, Vec::new());
    materialise_workspace(&mut spec)
        .await
        .expect("materialise should succeed");
    assert_eq!(spec.cwd, real);
}

#[tokio::test]
async fn a_declared_writable_root_keeps_the_text_the_operator_wrote() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (link, _real) = linked_dir(dir.path());
    let mut spec = spec_at(&link, vec![link.clone()]);
    materialise_workspace(&mut spec)
        .await
        .expect("materialise should succeed");
    assert_eq!(spec.sandbox.writable_roots, vec![link]);
}

#[tokio::test]
async fn the_real_path_of_a_declared_writable_root_is_granted_as_well() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (link, real) = linked_dir(dir.path());
    let mut spec = spec_at(&link, vec![link.clone()]);
    materialise_workspace(&mut spec)
        .await
        .expect("materialise should succeed");
    assert!(
        spec.sandbox.granted_roots().contains(&real.as_str()),
        "got: {:?}",
        spec.sandbox.granted_roots()
    );
}

#[tokio::test]
async fn a_writable_root_pm3_already_derived_is_not_granted_twice() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (link, real) = linked_dir(dir.path());
    let mut spec = spec_at(&link, vec![link.clone()]);
    spec.sandbox.derived_roots = vec![link.clone()];
    materialise_workspace(&mut spec)
        .await
        .expect("materialise should succeed");
    let repeats = spec
        .sandbox
        .derived_roots
        .iter()
        .filter(|root| **root == real)
        .count();
    assert_eq!(repeats, 1, "got: {:?}", spec.sandbox.derived_roots);
}

#[tokio::test]
async fn a_writable_root_that_already_reads_as_its_real_path_is_not_granted_twice() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (_link, real) = linked_dir(dir.path());
    let mut spec = spec_at(&real, vec![real.clone()]);
    materialise_workspace(&mut spec)
        .await
        .expect("materialise should succeed");
    let granted = spec.sandbox.granted_roots();
    let repeats = granted.iter().filter(|root| **root == real).count();
    assert_eq!(repeats, 1, "got: {granted:?}");
}

#[tokio::test]
async fn a_placeholder_argument_becomes_the_working_directory() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut spec = spec_with_args(
        &dir.path().to_string_lossy(),
        &["-d", SERVICE_CWD_PLACEHOLDER],
    );
    materialise_workspace(&mut spec)
        .await
        .expect("materialise should succeed");
    assert_eq!(spec.args, vec!["-d".to_string(), spec.cwd.clone()]);
}

#[tokio::test]
async fn a_placeholder_keeps_the_rest_of_the_argument() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut spec = spec_with_args(
        &dir.path().to_string_lossy(),
        &["${PM3_SERVICE_CWD}/data.db"],
    );
    materialise_workspace(&mut spec)
        .await
        .expect("materialise should succeed");
    assert_eq!(spec.args, vec![format!("{}/data.db", spec.cwd)]);
}

#[tokio::test]
async fn every_placeholder_argument_is_expanded() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut spec = spec_with_args(
        &dir.path().to_string_lossy(),
        &[SERVICE_CWD_PLACEHOLDER, SERVICE_CWD_PLACEHOLDER],
    );
    materialise_workspace(&mut spec)
        .await
        .expect("materialise should succeed");
    assert_eq!(spec.args, vec![spec.cwd.clone(), spec.cwd.clone()]);
}

#[tokio::test]
async fn an_argument_without_the_placeholder_is_left_alone() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut spec = spec_with_args(&dir.path().to_string_lossy(), &["--port=8080"]);
    materialise_workspace(&mut spec)
        .await
        .expect("materialise should succeed");
    assert_eq!(spec.args, vec!["--port=8080".to_string()]);
}

#[tokio::test]
async fn a_placeholder_expands_to_the_real_path_not_the_symlink() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (link, real) = linked_dir(dir.path());
    let mut spec = spec_with_args(&link, &[SERVICE_CWD_PLACEHOLDER]);
    materialise_workspace(&mut spec)
        .await
        .expect("materialise should succeed");
    assert_eq!(spec.args, vec![real]);
}

#[tokio::test]
async fn a_script_shaped_like_the_placeholder_is_left_alone() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut spec = spec_with_args(&dir.path().to_string_lossy(), &[]);
    spec.script = SERVICE_CWD_PLACEHOLDER.to_string();
    materialise_workspace(&mut spec)
        .await
        .expect("materialise should succeed");
    assert_eq!(spec.script, SERVICE_CWD_PLACEHOLDER);
}

#[tokio::test]
async fn an_unresolvable_writable_root_is_left_alone() {
    let dir = tempfile::tempdir().expect("temp dir");
    let absent = "/nonexistent/pm3-root".to_string();
    let mut spec = spec_at(&dir.path().to_string_lossy(), vec![absent.clone()]);
    materialise_workspace(&mut spec)
        .await
        .expect("materialise should succeed");
    assert_eq!(spec.sandbox.writable_roots, vec![absent]);
}
