use super::*;
use crate::{
    service::{InlineStart, prepare_inline, split_apps_file},
    service_fixtures::*,
};

#[tokio::test]
async fn a_partial_rollback_removes_only_the_named_service_file() {
    let home = home();
    let apps_file = home.dir.path().join("apps.yaml");
    std::fs::write(
        &apps_file,
        "apps:\n  - name: web\n    script: /bin/sh\n  - name: api\n    script: /bin/sh\n",
    )
    .expect("write the apps file");
    let split = split_apps_file(&context(&home), &apps_file.to_string_lossy(), false)
        .await
        .expect("the apps file should split");

    split.undo.run_for(&["api".to_string()]).await;

    assert!(home.cfg_dir.join("web.yaml").is_file());
    assert!(!home.cfg_dir.join("api.yaml").exists());
}

#[tokio::test]
async fn forgetting_an_unsafe_name_touches_nothing() {
    let home = home();
    let escaped = home.dir.path().join("escape.yaml");
    std::fs::write(&escaped, "name: escape\n").expect("seed the file");
    forget(&home.cfg_dir, "../escape").await;
    assert!(escaped.exists());
}

#[tokio::test]
async fn splitting_refuses_an_unsafe_app_name_before_writing_anything() {
    let home = home();
    let apps_file = home.dir.path().join("apps.yaml");
    std::fs::write(
        &apps_file,
        "apps:\n  - name: ../escape\n    script: /bin/sh\n",
    )
    .expect("write the apps file");
    let err = split_apps_file(&context(&home), &apps_file.to_string_lossy(), false)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot accept app name"), "got: {err}");
    assert!(!home.dir.path().join("escape.yaml").exists());
}

#[tokio::test]
async fn an_inline_request_refuses_an_unsafe_app_name() {
    let home = home();
    let args = shell_args();
    let request = InlineStart {
        name: "../escape",
        ..request(SHELL, &args, None, false)
    };
    let err = prepare_inline(&context(&home), &request)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot accept app name"), "got: {err}");
    assert!(!home.dir.path().join("escape.yaml").exists());
}

#[tokio::test]
async fn a_config_file_that_is_not_there_yet_reads_as_stale() {
    let home = home();
    let path = home.dir.path().join("config.yaml");
    let reconciled = reconcile(&path, "pm3:\n", false)
        .await
        .expect("a missing file is stale, not a conflict");
    assert_eq!(reconciled, Reconciled::Stale);
}

#[tokio::test]
async fn a_config_file_that_already_says_the_same_thing_is_unchanged() {
    let home = home();
    let path = home.dir.path().join("config.yaml");
    std::fs::write(&path, "pm3:\n").expect("seed the config file");
    let reconciled = reconcile(&path, "pm3:\n", false)
        .await
        .expect("identical content is unchanged");
    assert_eq!(reconciled, Reconciled::Unchanged);
}

#[tokio::test]
async fn a_config_file_pm3_cannot_read_is_refused_instead_of_overwritten() {
    let home = home();
    let path = home.dir.path().join("config.yaml");
    std::fs::create_dir_all(&path).expect("block the config path with a directory");
    let err = reconcile(&path, "pm3:\n", false)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot read the service file"), "got: {err}");
}

#[tokio::test]
async fn an_empty_config_file_still_needs_force_to_be_overwritten() {
    let home = home();
    let path = home.dir.path().join("config.yaml");
    std::fs::write(&path, "").expect("seed an empty config file");
    let err = reconcile(&path, "pm3:\n", false)
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("without --force"), "got: {err}");
}

#[tokio::test]
async fn forgetting_a_service_file_pm3_cannot_delete_is_reported() {
    let home = home();
    let blocked = home.cfg_dir.join("sleeper.yaml");
    std::fs::create_dir_all(&blocked).expect("block the service file path");
    std::fs::write(blocked.join("occupied"), "state").expect("fill the blocked path");
    forget(&home.cfg_dir, NAME).await;
    assert!(
        blocked.is_dir(),
        "the blocked path must survive the attempt"
    );
}

#[tokio::test]
async fn forgetting_a_service_file_that_is_already_gone_is_quiet() {
    let home = home();
    forget(&home.cfg_dir, NAME).await;
    assert!(!home.cfg_dir.join("sleeper.yaml").exists());
}

#[tokio::test]
async fn a_service_file_pm3_cannot_read_stops_the_write() {
    let home = home();
    let blocked = home.cfg_dir.join("sleeper.yaml");
    std::fs::create_dir_all(&blocked).expect("block the service file path");
    let args = shell_args();
    let err = prepare_inline(&context(&home), &request(SHELL, &args, None, false))
        .await
        .unwrap_err()
        .to_string();
    assert!(err.contains("cannot read the service file"), "got: {err}");
}
