#[cfg(unix)]
pub const SHELL: &str = "/bin/sh";
#[cfg(windows)]
pub const SHELL: &str = r"C:\Windows\System32\cmd.exe";

#[cfg(unix)]
pub const SHELL_NAME: &str = "sh";
#[cfg(windows)]
pub const SHELL_NAME: &str = "cmd.exe";

#[cfg(unix)]
pub const SEARCH_PATH: &str = "/usr/bin:/bin";
#[cfg(windows)]
pub const SEARCH_PATH: &str = "C:/Windows/System32";

#[cfg(unix)]
pub fn unreachable_parent(dir: &std::path::Path) -> std::path::PathBuf {
    let blocked = dir.join("blocked");
    std::fs::write(&blocked, "not a directory").expect("occupy the parent path");
    blocked
}

#[cfg(windows)]
pub fn unreachable_parent(dir: &std::path::Path) -> std::path::PathBuf {
    dir.join("bad|name")
}

#[cfg(unix)]
pub const EOL: &str = "\n";
#[cfg(windows)]
pub const EOL: &str = "\r\n";

#[cfg(unix)]
pub fn shell_args(unix: &str, _windows: &str) -> Vec<String> {
    vec!["-c".to_string(), unix.to_string()]
}

#[cfg(windows)]
pub fn shell_args(_unix: &str, windows: &str) -> Vec<String> {
    vec!["/C".to_string(), windows.to_string()]
}

#[cfg(windows)]
pub struct SleepingTree(pub tokio::process::Child);

#[cfg(windows)]
impl SleepingTree {
    pub fn spawn() -> Self {
        let child = tokio::process::Command::new(SHELL)
            .args(["/C", r"C:\Windows\System32\PING.EXE -n 60 127.0.0.1"])
            .stdout(std::process::Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn a sleeping tree");
        Self(child)
    }

    pub fn pid(&self) -> u32 {
        self.0.id().expect("a running tree has a root pid")
    }
}

#[cfg(windows)]
impl Drop for SleepingTree {
    fn drop(&mut self) {
        if let Some(pid) = self.0.id() {
            let _ = std::process::Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/T", "/F"])
                .output();
        }
    }
}
