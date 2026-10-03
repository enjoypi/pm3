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

#[cfg(unix)]
pub fn script(dir: &std::path::Path, name: &str, unix: &str, _windows: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt as _;

    let path = dir.join(name);
    std::fs::write(&path, format!("#!/bin/sh\n{unix}\n")).expect("write the script");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .expect("make the script executable");
    path
}

#[cfg(windows)]
pub fn script(dir: &std::path::Path, name: &str, _unix: &str, windows: &str) -> std::path::PathBuf {
    let path = dir.join(format!("{name}.cmd"));
    std::fs::write(&path, format!("@echo off\r\n{windows}\r\n")).expect("write the script");
    path
}

#[cfg(unix)]
pub fn full_mode_of(path: &std::path::Path) -> u32 {
    use std::os::unix::fs::PermissionsExt as _;

    std::fs::metadata(path)
        .expect("the path should exist")
        .permissions()
        .mode()
        & 0o7777
}

#[cfg(unix)]
pub fn mode_of(path: &std::path::Path) -> u32 {
    full_mode_of(path) & 0o777
}

#[cfg(unix)]
pub const ABSOLUTE_SHELL: &str = "/bin/sh";
#[cfg(windows)]
pub const ABSOLUTE_SHELL: &str = "C:/Windows/System32/cmd.exe";

#[cfg(unix)]
pub fn link_dir(target: &std::path::Path, link: &std::path::Path) {
    std::os::unix::fs::symlink(target, link).expect("link the directory");
}

#[cfg(windows)]
pub fn link_dir(target: &std::path::Path, link: &std::path::Path) {
    let status = std::process::Command::new(SHELL)
        .args(["/C", "mklink", "/J"])
        .arg(native(link))
        .arg(native(target))
        .stdout(std::process::Stdio::null())
        .status()
        .expect("run mklink");
    assert!(status.success(), "mklink /J should link the directory");
}

#[cfg(windows)]
fn native(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('/', "\\")
}

pub fn real_text(path: &std::path::Path) -> String {
    crate::portable_real_path(&path.canonicalize().expect("canonicalize the path"))
}

pub fn text(path: &std::path::Path) -> String {
    crate::portable_path(&path.to_string_lossy())
}
