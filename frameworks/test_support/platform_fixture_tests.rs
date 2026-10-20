#[cfg(unix)]
pub const SHELL: &str = "/bin/sh";
#[cfg(windows)]
pub const SHELL: &str = r"C:\Windows\System32\cmd.exe";

#[cfg(unix)]
pub const SHELL_FLAG: &str = "-c";
#[cfg(windows)]
pub const SHELL_FLAG: &str = "/C";

#[cfg(unix)]
pub const SLEEPER: &str = "sleep 30";
#[cfg(windows)]
pub const SLEEPER: &str = r"C:\Windows\System32\PING.EXE -n 31 127.0.0.1 >NUL";

pub const CRASHER: &str = "exit 1";

#[cfg(unix)]
pub const SUCCEEDER: &str = "true";
#[cfg(windows)]
pub const SUCCEEDER: &str = "exit 0";

#[cfg(unix)]
pub const KILLED: adapters::ExitOutcome = adapters::ExitOutcome::Signalled;
#[cfg(windows)]
pub const KILLED: adapters::ExitOutcome = adapters::ExitOutcome::Code(1);

#[cfg(unix)]
pub fn abs(path: &str) -> String {
    path.to_string()
}

#[cfg(windows)]
pub fn abs(path: &str) -> String {
    format!("C:{path}")
}

#[cfg(unix)]
pub fn true_program() -> String {
    "/usr/bin/true".to_string()
}

#[cfg(unix)]
pub fn false_program() -> String {
    "/usr/bin/false".to_string()
}

#[cfg(windows)]
struct Stubs {
    _dir: tempfile::TempDir,
    agreeing: String,
    refusing: String,
}

#[cfg(windows)]
static STUBS: std::sync::LazyLock<Stubs> = std::sync::LazyLock::new(|| {
    let dir = tempfile::tempdir().expect("create the stub directory");
    let write = |name: &str, code: u8| {
        let path = dir.path().join(format!("{name}.cmd"));
        std::fs::write(&path, format!("@exit /b {code}\r\n")).expect("write the stub");
        path.to_string_lossy().into_owned()
    };
    let agreeing = write("true", 0);
    let refusing = write("false", 1);
    Stubs {
        _dir: dir,
        agreeing,
        refusing,
    }
});

#[cfg(windows)]
pub fn true_program() -> String {
    STUBS.agreeing.clone()
}

#[cfg(windows)]
pub fn false_program() -> String {
    STUBS.refusing.clone()
}

#[cfg(target_os = "macos")]
struct Trampoline {
    _dir: tempfile::TempDir,
    program: std::path::PathBuf,
}

#[cfg(target_os = "macos")]
static TRAMPOLINE: std::sync::LazyLock<Trampoline> = std::sync::LazyLock::new(|| {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = tempfile::tempdir().expect("create the trampoline directory");
    let program = dir.path().join("pm3-trampoline");
    std::fs::write(
        &program,
        "#!/bin/sh\nwhile [ \"$1\" != -- ]; do shift; done\nshift\nexec \"$@\"\n",
    )
    .expect("write the trampoline");
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    Trampoline { _dir: dir, program }
});

#[cfg(target_os = "macos")]
pub fn trampoline_program() -> std::path::PathBuf {
    TRAMPOLINE.program.clone()
}
