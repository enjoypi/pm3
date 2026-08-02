use std::path::Path;

use usecases::{SandboxMode, SandboxPolicy};

use super::*;

fn spec_with_args(cwd: &str, args: &[&str]) -> AppSpec {
    AppSpec {
        args: args.iter().map(|arg| (*arg).to_string()).collect(),
        ..spec_at(cwd, Vec::new())
    }
}

fn spec_at(cwd: &str, writable_roots: Vec<String>) -> AppSpec {
    AppSpec {
        name: "web".to_string(),
        script: "/bin/sh".to_string(),
        args: Vec::new(),
        cwd: cwd.to_string(),
        env: Vec::new(),
        autorestart: true,
        min_uptime_ms: 1000,
        max_restarts: 15,
        restart_delay_ms: 0,
        schedule: None,
        depends_on: Vec::new(),
        sandbox: SandboxPolicy {
            mode: SandboxMode::WorkspaceWrite,
            network: false,
            writable_roots,
            derived_roots: Vec::new(),
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
async fn a_missing_working_directory_is_created() {
    let dir = tempfile::tempdir().expect("temp dir");
    let cwd = dir.path().join("web");
    let mut spec = spec_at(&cwd.to_string_lossy(), Vec::new());
    materialise_workspace(&mut spec).await;
    assert!(cwd.is_dir(), "the working directory should exist");
}

#[tokio::test]
async fn a_working_directory_blocked_by_a_file_is_left_unresolved() {
    let dir = tempfile::tempdir().expect("temp dir");
    let blocked = dir.path().join("web");
    std::fs::write(&blocked, "occupied").expect("occupy the working directory path");
    let mut spec = spec_at(&blocked.to_string_lossy(), Vec::new());
    materialise_workspace(&mut spec).await;
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
    materialise_workspace(&mut spec).await;
    assert_eq!(spec.cwd, real);
}

#[tokio::test]
async fn every_writable_root_is_resolved_to_its_real_path() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (link, real) = linked_dir(dir.path());
    let mut spec = spec_at(&link, vec![link.clone()]);
    materialise_workspace(&mut spec).await;
    assert_eq!(spec.sandbox.writable_roots, vec![real]);
}

#[tokio::test]
async fn a_placeholder_argument_becomes_the_working_directory() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut spec = spec_with_args(&dir.path().to_string_lossy(), &["-d", SVC_CWD_PLACEHOLDER]);
    materialise_workspace(&mut spec).await;
    assert_eq!(spec.args, vec!["-d".to_string(), spec.cwd.clone()]);
}

#[tokio::test]
async fn a_placeholder_keeps_the_rest_of_the_argument() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut spec = spec_with_args(&dir.path().to_string_lossy(), &["${PM3_SVC_CWD}/data.db"]);
    materialise_workspace(&mut spec).await;
    assert_eq!(spec.args, vec![format!("{}/data.db", spec.cwd)]);
}

#[tokio::test]
async fn every_placeholder_argument_is_expanded() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut spec = spec_with_args(
        &dir.path().to_string_lossy(),
        &[SVC_CWD_PLACEHOLDER, SVC_CWD_PLACEHOLDER],
    );
    materialise_workspace(&mut spec).await;
    assert_eq!(spec.args, vec![spec.cwd.clone(), spec.cwd.clone()]);
}

#[tokio::test]
async fn an_argument_without_the_placeholder_is_left_alone() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut spec = spec_with_args(&dir.path().to_string_lossy(), &["--port=8080"]);
    materialise_workspace(&mut spec).await;
    assert_eq!(spec.args, vec!["--port=8080".to_string()]);
}

#[tokio::test]
async fn a_placeholder_expands_to_the_real_path_not_the_symlink() {
    let dir = tempfile::tempdir().expect("temp dir");
    let (link, real) = linked_dir(dir.path());
    let mut spec = spec_with_args(&link, &[SVC_CWD_PLACEHOLDER]);
    materialise_workspace(&mut spec).await;
    assert_eq!(spec.args, vec![real]);
}

#[tokio::test]
async fn a_script_shaped_like_the_placeholder_is_left_alone() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut spec = spec_with_args(&dir.path().to_string_lossy(), &[]);
    spec.script = SVC_CWD_PLACEHOLDER.to_string();
    materialise_workspace(&mut spec).await;
    assert_eq!(spec.script, SVC_CWD_PLACEHOLDER);
}

#[tokio::test]
async fn an_unresolvable_writable_root_is_left_alone() {
    let dir = tempfile::tempdir().expect("temp dir");
    let absent = "/nonexistent/pm3-root".to_string();
    let mut spec = spec_at(&dir.path().to_string_lossy(), vec![absent.clone()]);
    materialise_workspace(&mut spec).await;
    assert_eq!(spec.sandbox.writable_roots, vec![absent]);
}
