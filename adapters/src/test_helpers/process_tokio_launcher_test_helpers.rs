use std::path::Path;

use super::*;

pub const APP_NAME: &str = "web";
pub const ECHO_PROGRAM: &str = "/bin/echo";
pub const SHELL_PROGRAM: &str = "/bin/sh";
pub const PWD_PROGRAM: &str = "/bin/pwd";
pub const OUT_LOG: &str = "web-out.log";
pub const ERR_LOG: &str = "web-err.log";

pub fn spec_in(dir: &Path, program: &str, args: &[&str]) -> LaunchSpec {
    LaunchSpec {
        name: APP_NAME.to_string(),
        program: program.to_string(),
        args: args.iter().map(|arg| (*arg).to_string()).collect(),
        cwd: text(dir),
        env: Vec::new(),
        stdout_path: text(&dir.join(OUT_LOG)),
        stderr_path: text(&dir.join(ERR_LOG)),
    }
}

pub fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub async fn read_log(dir: &Path, name: &str) -> String {
    tokio::fs::read_to_string(dir.join(name))
        .await
        .expect("should read the log file")
}

pub async fn run_to_completion(spec: &LaunchSpec) -> Option<ExitOutcome> {
    let launcher = TokioProcessLauncher::default();
    let process = launcher.spawn(spec).await.expect("should spawn");
    launcher.wait(process.pid).await
}
