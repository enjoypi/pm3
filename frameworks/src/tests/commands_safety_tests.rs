use super::*;

#[tokio::test]
async fn a_half_started_batch_keeps_the_service_file_of_what_started() {
    let fixture = running_daemon().await;
    let (apps_file, cfg_dir) = half_startable_apps_file(&fixture);
    let outcome = start_apps(&fixture.config_path, &apps_file, false).await;
    assert!(outcome.is_err(), "a refused service must fail the command");
    assert!(cfg_dir.join("web.yaml").is_file(), "web did start");
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn a_half_started_batch_rolls_back_only_the_service_file_it_refused() {
    let fixture = running_daemon().await;
    let (apps_file, cfg_dir) = half_startable_apps_file(&fixture);
    start_apps(&fixture.config_path, &apps_file, false)
        .await
        .expect_err("a refused service must fail the command");
    assert!(
        !cfg_dir.join("broken.yaml").exists(),
        "broken never started"
    );
    stop_daemon(fixture).await;
}

#[tokio::test]
async fn a_half_started_batch_names_what_it_could_not_start() {
    let fixture = running_daemon().await;
    let (apps_file, _cfg_dir) = half_startable_apps_file(&fixture);
    let err = start_apps(&fixture.config_path, &apps_file, false)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot start broken"), "got: {err}");
    stop_daemon(fixture).await;
}

fn half_startable_apps_file(fixture: &Fixture) -> (String, PathBuf) {
    let cwd = fixture.paths.root.to_string_lossy().into_owned();
    let unrunnable = fixture.dir.path().join("not-executable");
    std::fs::write(&unrunnable, "").expect("write a file nobody can execute");
    let body = format!(
        "apps:\n  - name: web\n    script: /bin/sh\n    cwd: \"{cwd}\"\n    args:\n      - \"-c\"\n      - \"sleep 30\"\n  - name: broken\n    script: \"{}\"\n    cwd: \"{cwd}\"\n    depends_on:\n      - web\n",
        unrunnable.display()
    );
    let apps_file = crate::test_support::write_apps_file(fixture.dir.path(), &body);
    let cfg_dir = fixture.paths.root.join("svc");
    (apps_file.to_string_lossy().into_owned(), cfg_dir)
}

#[tokio::test]
async fn following_a_log_path_that_is_not_a_file_fails() {
    let fixture = running_daemon().await;
    let blocked = stdout_log(&fixture.paths, "blocked").expect("a safe service name");
    std::fs::create_dir_all(&blocked).expect("block the log path with a directory");
    let outcome = follow_log(&fixture.config_path, "blocked", 1, &|_line| {}).await;
    assert!(outcome.is_err(), "got: {outcome:?}");
    stop_daemon(fixture).await;
}
