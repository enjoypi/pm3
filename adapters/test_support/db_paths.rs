use std::path::{Path, PathBuf};

pub fn workspace_migrations_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .join("migrations")
}

pub fn sqlite_rwc_url(db_path: &Path) -> String {
    format!("sqlite://{}?mode=rwc", db_path.display())
}
