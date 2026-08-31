use super::*;
use crate::{service::forget, service_fixtures::*};

#[tokio::test]
async fn an_inline_request_writes_one_config_file() {
    let home = home();
    let written = prepared(&home, false).await;
    assert_eq!(written.path, home.cfg_dir.join("sleeper.yaml"));
    assert_eq!(written.reconciled, Reconciled::Stale);
    let contents = std::fs::read_to_string(&written.path).expect("read the config file");
    assert!(contents.contains("name: \"sleeper\""), "got: {contents}");
    assert!(contents.contains(SHELL), "got: {contents}");
}

#[tokio::test]
async fn an_inline_request_leaves_the_working_directory_to_the_daemon() {
    let home = home();
    let path = prepared(&home, false).await.path;
    let written = std::fs::read_to_string(&path).expect("read the config file");
    assert!(!written.contains("cwd:"), "got: {written}");
}

#[tokio::test]
async fn a_bare_program_is_stored_without_resolving_it() {
    let home = home();
    let path = prepare_inline(&context(&home), &request("sh", &[], None, false))
        .await
        .expect("the inline request should resolve")
        .path;
    let written = std::fs::read_to_string(&path).expect("read the config file");
    assert!(written.contains("script: \"sh\""), "got: {written}");
}

#[tokio::test]
async fn an_explicit_working_directory_folds_the_home_away() {
    let home = home();
    let args = shell_args();
    let asked = request(SHELL, &args, Some("/home/dev/work"), false);
    let path = prepare_inline(&context(&home), &asked)
        .await
        .expect("the inline request should resolve")
        .path;
    let written = std::fs::read_to_string(&path).expect("read the config file");
    assert!(written.contains("cwd: \"${HOME}/work\""), "got: {written}");
}

#[tokio::test]
async fn program_arguments_fold_the_home_away() {
    let home = home();
    let mut args = shell_args();
    args.push("/home/dev/.config/mihomo/rule.yaml".to_string());
    let path = prepare_inline(&context(&home), &request(SHELL, &args, None, false))
        .await
        .expect("the inline request should resolve")
        .path;
    let written = std::fs::read_to_string(&path).expect("read the config file");
    assert!(
        written.contains("${HOME}/.config/mihomo/rule.yaml"),
        "got: {written}"
    );
}

#[tokio::test]
async fn a_bare_service_cwd_token_is_stored_as_a_braced_placeholder() {
    let home = home();
    let mut args = shell_args();
    args.push("PM3_SERVICE_CWD".to_string());
    let path = prepare_inline(&context(&home), &request(SHELL, &args, None, false))
        .await
        .expect("the inline request should resolve")
        .path;
    let written = std::fs::read_to_string(&path).expect("read the config file");
    assert!(written.contains("\"${PM3_SERVICE_CWD}\""), "got: {written}");
}

#[tokio::test]
async fn splitting_an_apps_file_folds_the_service_cwd_token() {
    let home = home();
    let apps_file = write_apps_file(
        &home,
        "apps:\n  - name: web\n    script: /bin/sh\n    args:\n      - \"PM3_SERVICE_CWD/data\"\n",
    );
    let split = split_apps_file(&context(&home), &apps_file.to_string_lossy(), false)
        .await
        .expect("the apps file should split");
    assert_eq!(split.changed, vec!["web".to_string()]);
    let written =
        std::fs::read_to_string(home.cfg_dir.join("web.yaml")).expect("read the config file");
    assert!(
        written.contains("\"${PM3_SERVICE_CWD}/data\""),
        "got: {written}"
    );
}

#[tokio::test]
async fn a_program_missing_from_the_search_path_is_reported() {
    let home = home();
    let err = prepare_inline(
        &context(&home),
        &request("pm3-not-a-real-program", &[], None, false),
    )
    .await
    .unwrap_err()
    .to_string();
    assert_eq!(err, "cannot find 'pm3-not-a-real-program' on PATH");
}

#[tokio::test]
async fn an_inline_start_never_writes_an_environment() {
    let home = home();
    let prepared = prepared(&home, false).await;
    let written = std::fs::read_to_string(&prepared.path).expect("read the config file");
    assert!(
        !written.contains("env:"),
        "environment values belong in the sidecar file: {written}"
    );
}

#[tokio::test]
async fn writing_the_same_config_twice_changes_nothing() {
    let home = home();
    let first = prepared(&home, false).await;
    let before = std::fs::read_to_string(&first.path).expect("read the config file");
    let second = prepared(&home, false).await;
    assert_eq!(first.path, second.path);
    assert_eq!(second.reconciled, Reconciled::Unchanged);
    assert_eq!(
        std::fs::read_to_string(&second.path).expect("read the config file"),
        before
    );
}

#[tokio::test]
async fn a_forced_rewrite_reports_a_stale_config() {
    let home = home();
    prepared(&home, false).await;
    std::fs::write(
        home.cfg_dir.join("sleeper.yaml"),
        "apps:\n  - name: sleeper\n    script: /bin/echo\n",
    )
    .expect("edit the config file");
    let rewritten = prepared(&home, true).await;
    assert_eq!(rewritten.reconciled, Reconciled::Stale);
}

#[tokio::test]
async fn a_changed_config_needs_force() {
    let home = home();
    let path = prepared(&home, false).await.path;
    std::fs::write(&path, "apps:\n  - name: sleeper\n    script: /bin/echo\n")
        .expect("edit the config file");
    let err = prepare_inline(&context(&home), &request(SHELL, &shell_args(), None, false))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("without --force"), "got: {err}");
    assert!(err.contains("-    script: /bin/echo"), "got: {err}");
}

#[tokio::test]
async fn force_overwrites_a_changed_config() {
    let home = home();
    let path = prepared(&home, false).await.path;
    std::fs::write(&path, "apps: []\n").expect("edit the config file");
    prepared(&home, true).await;
    let written = std::fs::read_to_string(&path).expect("read the config file");
    assert!(written.contains(SHELL), "got: {written}");
}

#[tokio::test]
async fn a_config_directory_that_is_missing_is_reported() {
    let dir = tempfile::tempdir().expect("temp dir");
    let missing = dir.path().join("config/pm3");
    let context = ServiceContext {
        cfg_dir: &missing,
        search_path: SEARCH_PATH,
        home: Some(FAKE_HOME),
    };
    let err = prepare_inline(&context, &request(SHELL, &shell_args(), None, false))
        .await
        .unwrap_err()
        .to_string();
    assert!(
        err.starts_with("cannot write the service file"),
        "got: {err}"
    );
}

#[tokio::test]
async fn an_apps_file_is_split_into_one_config_file_per_app() {
    let home = home();
    let apps_file = write_apps_file(
        &home,
        "apps:\n  - name: web\n    script: /bin/sh\n  - name: db\n    script: /bin/sh\n",
    );
    let split = split_apps_file(&context(&home), &apps_file.to_string_lossy(), false)
        .await
        .expect("the apps file should split");
    assert_eq!(split.changed, vec!["web".to_string(), "db".to_string()]);
    assert!(home.cfg_dir.join("web.yaml").is_file());
    assert!(home.cfg_dir.join("db.yaml").is_file());
}

#[tokio::test]
async fn splitting_an_unchanged_apps_file_reports_no_changes() {
    let home = home();
    let apps_file = write_apps_file(&home, "apps:\n  - name: web\n    script: /bin/sh\n");
    split_apps_file(&context(&home), &apps_file.to_string_lossy(), false)
        .await
        .expect("the apps file should split");
    let split = split_apps_file(&context(&home), &apps_file.to_string_lossy(), false)
        .await
        .expect("the apps file should split again");
    assert!(split.changed.is_empty(), "got: {:?}", split.changed);
}

#[tokio::test]
async fn splitting_folds_the_home_out_of_every_app() {
    let home = home();
    let apps_file = write_apps_file(
        &home,
        "apps:\n  - name: web\n    script: /bin/sh\n    cwd: \"/home/dev/web\"\n    args:\n      - \"/home/dev/app.js\"\n",
    );
    let split = split_apps_file(&context(&home), &apps_file.to_string_lossy(), false)
        .await
        .expect("the apps file should split");
    assert_eq!(split.changed, vec!["web".to_string()]);
    let written =
        std::fs::read_to_string(home.cfg_dir.join("web.yaml")).expect("read the config file");
    assert!(written.contains("cwd: \"${HOME}/web\""), "got: {written}");
    assert!(written.contains("${HOME}/app.js"), "got: {written}");
}

#[tokio::test]
async fn splitting_an_unreadable_apps_file_is_reported() {
    let home = home();
    let err = split_apps_file(&context(&home), "/nonexistent/pm3-apps.yaml", false)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot read apps file"), "got: {err}");
}

#[tokio::test]
async fn splitting_over_a_changed_config_needs_force() {
    let home = home();
    let apps_file = write_apps_file(&home, "apps:\n  - name: web\n    script: /bin/sh\n");
    std::fs::write(home.cfg_dir.join("web.yaml"), "apps: []\n").expect("seed a conflict");
    let err = split_apps_file(&context(&home), &apps_file.to_string_lossy(), false)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("without --force"), "got: {err}");
}

#[tokio::test]
async fn undoing_a_fresh_split_removes_every_file_it_wrote() {
    let home = home();
    let apps_file = write_apps_file(
        &home,
        "apps:\n  - name: web\n    script: /bin/sh\n  - name: db\n    script: /bin/sh\n",
    );
    let split = split_apps_file(&context(&home), &apps_file.to_string_lossy(), false)
        .await
        .expect("the apps file should split");
    split.undo.run().await;
    assert!(!home.cfg_dir.join("web.yaml").exists());
    assert!(!home.cfg_dir.join("db.yaml").exists());
}

#[tokio::test]
async fn undoing_a_forced_split_restores_the_previous_config() {
    let home = home();
    let apps_file = write_apps_file(&home, "apps:\n  - name: web\n    script: /bin/sh\n");
    let service = home.cfg_dir.join("web.yaml");
    std::fs::write(&service, "apps: []\n").expect("seed the previous config");
    let split = split_apps_file(&context(&home), &apps_file.to_string_lossy(), true)
        .await
        .expect("a forced split should overwrite");
    split.undo.run().await;
    assert_eq!(
        std::fs::read_to_string(&service).expect("read the config file"),
        "apps: []\n"
    );
}

#[tokio::test]
async fn an_undo_that_cannot_reach_its_file_leaves_the_rest_of_the_rollback_running() {
    let home = home();
    let apps_file = write_apps_file(
        &home,
        "apps:\n  - name: web\n    script: /bin/sh\n  - name: db\n    script: /bin/sh\n",
    );
    let split = split_apps_file(&context(&home), &apps_file.to_string_lossy(), false)
        .await
        .expect("the apps file should split");
    std::fs::remove_file(home.cfg_dir.join("web.yaml")).expect("take the file away first");
    split.undo.run().await;
    assert!(
        !home.cfg_dir.join("db.yaml").exists(),
        "a failed step must not abort the rollback"
    );
}

#[tokio::test]
async fn undoing_an_unchanged_split_touches_nothing() {
    let home = home();
    let apps_file = write_apps_file(&home, "apps:\n  - name: web\n    script: /bin/sh\n");
    split_apps_file(&context(&home), &apps_file.to_string_lossy(), false)
        .await
        .expect("the apps file should split");
    let split = split_apps_file(&context(&home), &apps_file.to_string_lossy(), false)
        .await
        .expect("the apps file should split again");
    split.undo.run().await;
    assert!(
        home.cfg_dir.join("web.yaml").is_file(),
        "an unchanged split has nothing to roll back"
    );
}

#[tokio::test]
async fn a_split_that_hits_a_conflict_rolls_back_what_it_already_wrote() {
    let home = home();
    let apps_file = write_apps_file(
        &home,
        "apps:\n  - name: web\n    script: /bin/sh\n  - name: db\n    script: /bin/sh\n",
    );
    std::fs::write(home.cfg_dir.join("db.yaml"), "apps: []\n").expect("seed a conflict");
    let err = split_apps_file(&context(&home), &apps_file.to_string_lossy(), false)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("without --force"), "got: {err}");
    assert!(
        !home.cfg_dir.join("web.yaml").exists(),
        "the first app must not survive a refused split"
    );
    assert_eq!(
        std::fs::read_to_string(home.cfg_dir.join("db.yaml")).expect("read the config file"),
        "apps: []\n"
    );
}

#[tokio::test]
async fn forgetting_a_service_removes_its_config() {
    let home = home();
    let path = prepared(&home, false).await.path;
    forget(&home.cfg_dir, NAME).await;
    assert!(!path.exists(), "the config file should be gone");
}

#[tokio::test]
async fn forgetting_a_service_removes_its_environment_too() {
    let home = home();
    prepared(&home, false).await;
    let secrets = crate::env_file_of(&home.cfg_dir, NAME).expect("a safe service name");
    std::fs::write(&secrets, "TUNNEL_TOKEN=eyJhIjoiZjQ2\n").expect("write the environment file");
    forget(&home.cfg_dir, NAME).await;
    assert!(!secrets.exists(), "a deleted service keeps no secrets");
}

#[tokio::test]
async fn forgetting_an_unknown_service_is_quiet() {
    let home = home();
    forget(&home.cfg_dir, "ghost").await;
    assert!(!home.cfg_dir.join("ghost.yaml").exists());
}

#[tokio::test]
async fn splitting_an_apps_file_folds_home_out_of_the_writable_roots() {
    let home = home();
    let apps_file = write_apps_file(
        &home,
        "apps:\n  - name: web\n    script: /bin/sh\n    sandbox:\n      writable_roots:\n        - \"/home/dev/prj\"\n",
    );
    split_apps_file(&context(&home), &apps_file.to_string_lossy(), false)
        .await
        .expect("the apps file should split");
    let written =
        std::fs::read_to_string(home.cfg_dir.join("web.yaml")).expect("read the config file");
    assert!(written.contains("\"${HOME}/prj\""), "got: {written}");
}

#[tokio::test]
async fn splitting_an_apps_file_that_declares_an_environment_is_refused() {
    let home = home();
    let apps_file = write_apps_file(
        &home,
        "apps:\n  - name: web\n    script: /bin/sh\n    env:\n      TUNNEL_TOKEN: \"eyJhIjoiZjQ2\"\n",
    );
    let refused = split_apps_file(&context(&home), &apps_file.to_string_lossy(), false)
        .await
        .expect_err("an apps file may not declare an environment")
        .to_string();
    assert!(refused.contains("'web.env'"), "{refused}");
    assert!(!refused.contains("eyJhIjoiZjQ2"), "{refused}");
    assert!(
        !home.cfg_dir.join("web.yaml").exists(),
        "nothing may be written when the declaration is refused"
    );
}

#[tokio::test]
async fn splitting_an_apps_file_leaves_a_sandbox_without_roots_alone() {
    let home = home();
    let apps_file = write_apps_file(
        &home,
        "apps:\n  - name: web\n    script: /bin/sh\n    sandbox:\n      network: true\n",
    );
    split_apps_file(&context(&home), &apps_file.to_string_lossy(), false)
        .await
        .expect("the apps file should split");
    let written =
        std::fs::read_to_string(home.cfg_dir.join("web.yaml")).expect("read the config file");
    assert!(written.contains("network: true"), "got: {written}");
}
