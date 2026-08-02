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
