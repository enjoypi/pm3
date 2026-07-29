#![cfg(has_database)]

use super::*;
#[cfg(has_http)]
use crate::test_helpers::{
    ephemeral_port, serve_immediate_shutdown_retrying_bind, server_only_yaml,
};
use crate::test_helpers::{
    full_yaml, sqlite_rwc_url, telemetry_only_yaml, tokio_block_on, workspace_migrations_dir,
    write_config,
};

#[cfg(has_http)]
#[tokio::test]
async fn run_serve_with_shutdown_immediate_with_db() {
    let dir = tempfile::tempdir().expect("tempdir");
    let url = sqlite_rwc_url(&dir.path().join("serve.db"));
    let migrations = workspace_migrations_dir();
    let migrations_path = migrations.to_str().expect("path");
    serve_immediate_shutdown_retrying_bind(&dir, |port| {
        full_yaml("127.0.0.1", port, &url, migrations_path)
    })
    .await
    .expect("immediate shutdown ok with db");
}

#[cfg(has_http)]
#[tokio::test]
async fn run_serve_with_shutdown_without_database_skips_pool() {
    let dir = tempfile::tempdir().expect("tempdir");
    serve_immediate_shutdown_retrying_bind(&dir, |port| server_only_yaml("127.0.0.1", port))
        .await
        .expect("serve without database should start and shut down");
}

#[cfg(has_http)]
#[tokio::test]
async fn run_serve_with_shutdown_db_create_pool_failure() {
    let port = ephemeral_port();
    let dir = tempfile::tempdir().expect("tempdir");
    let url = "sqlite:///nonexistent_root_path/db.db?mode=rw";
    let migrations = workspace_migrations_dir();
    let path = write_config(
        &dir,
        &full_yaml("127.0.0.1", port, url, migrations.to_str().expect("path")),
    );
    assert!(
        run_serve_with_shutdown(&path, false, async {})
            .await
            .is_err()
    );
}

#[cfg(has_http)]
#[tokio::test]
async fn run_serve_with_shutdown_unreadable_migrations_dir_stops_startup() {
    let port = ephemeral_port();
    let dir = tempfile::tempdir().expect("tempdir");
    let url = sqlite_rwc_url(&dir.path().join("serve_bad_migrations.db"));
    let path = write_config(
        &dir,
        &full_yaml("127.0.0.1", port, &url, "/nonexistent/migrations"),
    );
    let err = run_serve_with_shutdown(&path, false, async {})
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("cannot find migrations directory"),
        "got: {err}"
    );
}

#[tokio::test]
async fn with_db_pool_invalid_config_path_returns_error() {
    let result = with_db_pool("/nonexistent/db.yaml", None, |_pool, _mpath, _t| async {
        Ok(())
    })
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn with_db_pool_create_pool_failure_returns_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let url = format!("sqlite://{}/missing.db?mode=ro", dir.path().display());
    let migrations = workspace_migrations_dir();
    let path = write_config(
        &dir,
        &full_yaml("127.0.0.1", 38922, &url, migrations.to_str().expect("path")),
    );
    let result = with_db_pool(&path, None, |_pool, _mpath, _t| async { Ok(()) }).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn run_db_migrate_invalid_migrations_path_returns_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("bad_mig.db");
    let url = sqlite_rwc_url(&db_path);
    let path = write_config(
        &dir,
        &full_yaml("127.0.0.1", 38920, &url, "/nonexistent/m1"),
    );
    let err = run_db_migrate(&path, Some("/nonexistent/m2"))
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("cannot find migrations directory"),
        "got: {err}"
    );
}

#[tokio::test]
async fn run_db_status_invalid_migrations_path_returns_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("bad_status.db");
    let url = sqlite_rwc_url(&db_path);
    let path = write_config(
        &dir,
        &full_yaml("127.0.0.1", 38921, &url, "/nonexistent/m1"),
    );
    let err = run_db_status(&path, Some("/nonexistent/m2"))
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("cannot find migrations directory"),
        "got: {err}"
    );
}

#[tokio::test]
async fn with_db_pool_no_database_section_returns_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = write_config(&dir, &telemetry_only_yaml());
    let result = with_db_pool(&path, None, |_pool, _mpath, _t| async { Ok(()) }).await;
    let err = result.unwrap_err();
    assert!(err.to_string().contains("database"), "got: {err}");
}

#[tokio::test]
async fn with_db_pool_propagates_inner_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("inner_err.db");
    let url = sqlite_rwc_url(&db_path);
    let migrations = workspace_migrations_dir();
    let path = write_config(
        &dir,
        &full_yaml("127.0.0.1", 38901, &url, migrations.to_str().expect("path")),
    );
    let result = with_db_pool(&path, None, |_pool, _mpath, _t| async {
        Err(anyhow::anyhow!("inner failure"))
    })
    .await;
    assert!(result.unwrap_err().to_string().contains("inner failure"));
}

#[tokio::test]
async fn run_db_migrate_ok() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("migrate.db");
    let url = sqlite_rwc_url(&db_path);
    let migrations = workspace_migrations_dir();
    let path = write_config(
        &dir,
        &full_yaml("127.0.0.1", 38902, &url, migrations.to_str().expect("path")),
    );
    run_db_migrate(&path, None).await.expect("migrate ok");
}

#[tokio::test]
async fn run_db_migrate_with_path_override() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("migrate_override.db");
    let url = sqlite_rwc_url(&db_path);
    let path = write_config(
        &dir,
        &full_yaml("127.0.0.1", 38903, &url, "/nonexistent/migrations"),
    );
    let migrations = workspace_migrations_dir();
    run_db_migrate(&path, Some(migrations.to_str().expect("path")))
        .await
        .expect("migrate with override ok");
}

#[tokio::test]
async fn run_db_status_ok() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("status.db");
    let url = sqlite_rwc_url(&db_path);
    let migrations = workspace_migrations_dir();
    let path = write_config(
        &dir,
        &full_yaml("127.0.0.1", 38904, &url, migrations.to_str().expect("path")),
    );
    run_db_migrate(&path, None).await.expect("migrate first");
    run_db_status(&path, None).await.expect("status ok");
}

#[test]
fn dispatch_db_migrate() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("dispatch_migrate.db");
    let url = sqlite_rwc_url(&db_path);
    let migrations = workspace_migrations_dir();
    let path = write_config(
        &dir,
        &full_yaml("127.0.0.1", 38906, &url, migrations.to_str().expect("path")),
    );
    let cli = Cli {
        config: path,
        command: Commands::Db {
            migrations_path: None,
            command: DbCommands::Migrate,
        },
    };
    tokio_block_on(dispatch(cli)).expect("ok");
}

#[test]
fn dispatch_db_status() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("dispatch_status.db");
    let url = sqlite_rwc_url(&db_path);
    let migrations = workspace_migrations_dir();
    let path_str = write_config(
        &dir,
        &full_yaml("127.0.0.1", 38907, &url, migrations.to_str().expect("path")),
    );
    let migrate_cli = Cli {
        config: path_str.clone(),
        command: Commands::Db {
            migrations_path: None,
            command: DbCommands::Migrate,
        },
    };
    tokio_block_on(dispatch(migrate_cli)).expect("migrate ok");
    let status_cli = Cli {
        config: path_str,
        command: Commands::Db {
            migrations_path: None,
            command: DbCommands::Status,
        },
    };
    tokio_block_on(dispatch(status_cli)).expect("status ok");
}
