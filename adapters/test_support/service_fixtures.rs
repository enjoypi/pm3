use std::path::PathBuf;

use crate::service::{InlineStart, PreparedService, ServiceContext, prepare_inline};

pub const NAME: &str = "sleeper";
pub const SHELL: &str = "/bin/sh";
pub const SEARCH_PATH: &str = "/usr/bin:/bin";
pub const FAKE_HOME: &str = "/home/dev";

pub struct Home {
    pub dir: tempfile::TempDir,
    pub cfg_dir: PathBuf,
}

pub fn home() -> Home {
    let dir = tempfile::tempdir().expect("temp dir");
    let cfg_dir = dir.path().join("config/pm3");
    std::fs::create_dir_all(&cfg_dir).expect("prepare the config directory");
    Home { dir, cfg_dir }
}

pub fn context(home: &Home) -> ServiceContext<'_> {
    ServiceContext {
        cfg_dir: &home.cfg_dir,
        search_path: SEARCH_PATH,
        home: Some(FAKE_HOME),
    }
}

pub fn request<'s>(
    program: &'s str,
    args: &'s [String],
    cwd: Option<&'s str>,
    force: bool,
) -> InlineStart<'s> {
    InlineStart {
        name: NAME,
        program,
        args,
        cwd,
        env: &[],
        cron: None,
        autorestart: None,
        network: false,
        writable_dirs: &[],
        force,
    }
}

pub fn shell_args() -> Vec<String> {
    vec!["-c".to_string(), "sleep 1".to_string()]
}

pub async fn prepared(home: &Home, force: bool) -> PreparedService {
    prepare_inline(&context(home), &request(SHELL, &shell_args(), None, force))
        .await
        .expect("the inline request should resolve")
}
