use std::path::Path;

#[cfg(unix)]
pub const HEALTHY_SYSTEMD: &str =
    "case \"$2\" in\n  is-active) echo active ;;\n  show) echo 4242 ;;\nesac\nexit 0";
#[cfg(windows)]
pub const HEALTHY_SYSTEMD: &str =
    "if \"%2\"==\"is-active\" echo active\nif \"%2\"==\"show\" echo 4242\nexit /b 0";

#[cfg(unix)]
pub const HEALTHY_SCHTASKS: &str =
    "case \"$1\" in\n  /Query) echo 'Status: Running' ;;\nesac\nexit 0";
#[cfg(windows)]
pub const HEALTHY_SCHTASKS: &str = "if \"%1\"==\"/Query\" echo Status: Running\nexit /b 0";

#[cfg(unix)]
pub const REFUSING_ENABLE: &str = "case \"$2\" in\n  enable) exit 1 ;;\nesac\nexit 0";
#[cfg(windows)]
pub const REFUSING_ENABLE: &str = "if \"%2\"==\"enable\" exit /b 1\nexit /b 0";

#[cfg(unix)]
pub const FOREIGN_PID: &str =
    "case \"$2\" in\n  is-active) echo active ;;\n  show) echo 1 ;;\nesac\nexit 0";
#[cfg(windows)]
pub const FOREIGN_PID: &str =
    "if \"%2\"==\"is-active\" echo active\nif \"%2\"==\"show\" echo 1\nexit /b 0";

#[cfg(unix)]
pub const KICKSTART_RECOVERS: &str = "case \"$1\" in\n  list) if [ -f \"$0.kicked\" ]; then echo '\"PID\" = 4242;'; else echo '{}' ; fi ;;\n  kickstart) touch \"$0.kicked\" ;;\nesac\nexit 0";
#[cfg(windows)]
pub const KICKSTART_RECOVERS: &str = "if \"%1\"==\"list\" if exist \"%~f0.kicked\" (echo \"PID\" = 4242;) else (echo {})\nif \"%1\"==\"kickstart\" type nul > \"%~f0.kicked\"\nexit /b 0";

#[cfg(unix)]
pub const KICKSTART_REFUSED: &str =
    "case \"$1\" in\n  list) echo '{}' ;;\n  kickstart) exit 1 ;;\nesac\nexit 0";
#[cfg(windows)]
pub const KICKSTART_REFUSED: &str =
    "if \"%1\"==\"list\" echo {}\nif \"%1\"==\"kickstart\" exit /b 1\nexit /b 0";

#[cfg(unix)]
pub const LOAD_DELETES_ITSELF: &str = "case \"$1\" in\n  load) rm \"$0\" ;;\nesac\nexit 0";

#[cfg(unix)]
pub const SHOW_DELETES_ITSELF: &str =
    "case \"$2\" in\n  is-active) echo active ;;\n  show) rm \"$0\" ;;\nesac\nexit 0";

#[cfg(unix)]
pub const SHOW_STALLS: &str =
    "case \"$2\" in\n  is-active) echo active ;;\n  show) sleep 30 ;;\nesac\nexit 0";
#[cfg(windows)]
pub const SHOW_STALLS: &str = "if \"%2\"==\"is-active\" echo active\nif \"%2\"==\"show\" C:\\Windows\\System32\\PING.EXE -n 31 127.0.0.1 >NUL\nexit /b 0";

#[cfg(unix)]
pub const KICKSTART_DELETES_ITSELF: &str =
    "case \"$1\" in\n  list) echo '{}' ;;\n  kickstart) rm \"$0\" ;;\nesac\nexit 0";

#[cfg(unix)]
pub const ENABLE_CORRUPTS_DUMP: &str = "case \"$2\" in\n  is-active) echo active ;;\n  show) echo 4242 ;;\n  enable) echo 'not: [yaml' > \"$PM3_DUMP\" ;;\nesac\nexit 0";
#[cfg(windows)]
pub const ENABLE_CORRUPTS_DUMP: &str = "if \"%2\"==\"is-active\" echo active\nif \"%2\"==\"show\" echo 4242\nif \"%2\"==\"enable\" (echo not: [yaml)> \"%PM3_DUMP%\"\nexit /b 0";

#[cfg(unix)]
pub const ENABLE_EMPTIES_DUMP: &str = "case \"$2\" in\n  is-active) echo active ;;\n  show) echo 4242 ;;\n  enable) printf 'services: []\\n' > \"$PM3_DUMP\" ;;\nesac\nexit 0";
#[cfg(windows)]
pub const ENABLE_EMPTIES_DUMP: &str = "if \"%2\"==\"is-active\" echo active\nif \"%2\"==\"show\" echo 4242\nif \"%2\"==\"enable\" (echo services: [])> \"%PM3_DUMP%\"\nexit /b 0";

#[cfg(unix)]
const MANAGER_FILE: &str = "fake-manager";
#[cfg(windows)]
const MANAGER_FILE: &str = "fake-manager.cmd";

pub fn fake_manager(dir: &Path, body: &str) -> String {
    let path = dir.join(MANAGER_FILE);
    write_manager(&path, None, body);
    path.to_string_lossy().into_owned()
}

#[cfg(unix)]
pub fn write_manager(path: &Path, dump: Option<&Path>, body: &str) {
    use std::os::unix::fs::PermissionsExt as _;
    let prelude = dump.map_or_else(String::new, |dump| format!("PM3_DUMP={}\n", dump.display()));
    std::fs::write(path, format!("#!/bin/sh\n{prelude}{body}\n")).expect("write the fake manager");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
}

#[cfg(windows)]
pub fn write_manager(path: &Path, dump: Option<&Path>, body: &str) {
    let prelude = dump.map_or_else(String::new, |dump| {
        format!("set \"PM3_DUMP={}\"\n", dump.display())
    });
    let script = format!("@echo off\n{prelude}{body}\n").replace('\n', "\r\n");
    std::fs::write(path, script).expect("write the fake manager");
}
