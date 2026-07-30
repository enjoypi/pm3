use std::path::Path;

use usecases::{SandboxMode, SandboxPolicy};

use super::*;

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
        depends_on: Vec::new(),
        sandbox: SandboxPolicy {
            mode: SandboxMode::WorkspaceWrite,
            network: false,
            writable_roots,
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
async fn an_unresolvable_writable_root_is_left_alone() {
    let dir = tempfile::tempdir().expect("temp dir");
    let absent = "/nonexistent/pm3-root".to_string();
    let mut spec = spec_at(&dir.path().to_string_lossy(), vec![absent.clone()]);
    materialise_workspace(&mut spec).await;
    assert_eq!(spec.sandbox.writable_roots, vec![absent]);
}
