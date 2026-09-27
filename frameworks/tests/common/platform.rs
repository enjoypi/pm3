#[cfg(unix)]
pub const SHELL: &str = "/bin/sh";
#[cfg(windows)]
pub const SHELL: &str = r"C:\Windows\System32\cmd.exe";

#[cfg(unix)]
pub const SHELL_FLAG: &str = "-c";
#[cfg(windows)]
pub const SHELL_FLAG: &str = "/C";

#[cfg(unix)]
pub const SEARCH_PATH: &str = "/usr/bin:/bin:/opt/homebrew/bin";
#[cfg(windows)]
pub const SEARCH_PATH: &str = "C:/Windows/System32";

#[cfg(unix)]
pub const SLEEPER: &str = "sleep 30";
#[cfg(windows)]
pub const SLEEPER: &str = r"C:\Windows\System32\PING.EXE -n 31 127.0.0.1 >NUL";

#[cfg(unix)]
pub fn abs(path: &str) -> String {
    path.to_string()
}

#[cfg(windows)]
pub fn abs(path: &str) -> String {
    format!("C:{path}")
}

#[cfg(unix)]
pub fn process_is_alive(pid: u32) -> bool {
    std::process::Command::new("/bin/kill")
        .args(["-0", &pid.to_string()])
        .output()
        .expect("should probe the process")
        .status
        .success()
}

#[cfg(windows)]
pub fn process_is_alive(pid: u32) -> bool {
    let listed = std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
        .output()
        .expect("should probe the process");
    String::from_utf8_lossy(&listed.stdout).contains(&format!("\"{pid}\""))
}

#[cfg(unix)]
pub fn signal(pid: u32, name: &str) {
    let status = std::process::Command::new("/bin/kill")
        .args([name, &pid.to_string()])
        .status()
        .expect("should signal the daemon");
    assert!(status.success(), "kill {name} {pid} should succeed");
}

#[cfg(unix)]
pub fn chatty_command(out: &str, err: &str) -> String {
    format!("echo {out}; echo {err} >&2; exec {SLEEPER}")
}

#[cfg(windows)]
pub fn chatty_command(out: &str, err: &str) -> String {
    format!("echo {out}& echo {err}>&2& {SLEEPER}")
}

#[cfg(unix)]
pub fn captured(command: &mut std::process::Command) -> std::process::Output {
    command.output().expect("pm3 should run")
}

#[cfg(windows)]
pub fn captured(command: &mut std::process::Command) -> std::process::Output {
    let stdout = tempfile::tempfile().expect("a stdout capture file");
    let stderr = tempfile::tempfile().expect("a stderr capture file");
    let status = command
        .stdout(stdout.try_clone().expect("share the stdout capture"))
        .stderr(stderr.try_clone().expect("share the stderr capture"))
        .status()
        .expect("pm3 should run");
    std::process::Output {
        status,
        stdout: read_back(stdout),
        stderr: read_back(stderr),
    }
}

#[cfg(windows)]
fn read_back(mut file: std::fs::File) -> Vec<u8> {
    use std::io::{Read as _, Seek as _};
    file.rewind().expect("rewind the capture");
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).expect("read the capture");
    bytes
}

#[cfg(unix)]
pub fn flood_command(lines: u32) -> String {
    format!("i=0; while [ $i -lt {lines} ]; do echo line; i=$((i+1)); done; exec {SLEEPER}")
}

#[cfg(windows)]
pub fn flood_command(lines: u32) -> String {
    format!("(for /L %i in (1,1,{lines}) do @echo line)& {SLEEPER}")
}

#[cfg(unix)]
pub fn record_then_sleep(word: &str, file: &str) -> String {
    format!("echo {word} >> ./{file}; {SLEEPER}")
}

#[cfg(windows)]
pub fn record_then_sleep(word: &str, file: &str) -> String {
    format!("echo {word}>> {file}& {SLEEPER}")
}

#[cfg(unix)]
pub const NAP: &str = "sleep 1";
#[cfg(windows)]
pub const NAP: &str = r"C:\Windows\System32\PING.EXE -n 2 127.0.0.1 >NUL";

#[cfg(unix)]
pub const EXEC_SLEEPER: &str = "exec sleep 30";
#[cfg(windows)]
pub const EXEC_SLEEPER: &str = SLEEPER;

#[cfg(unix)]
pub fn report_variable(variable: &str, file: &str, then: &str) -> String {
    format!("printf '%s' \"${variable}\" > {file}; {then}")
}

#[cfg(windows)]
pub fn report_variable(variable: &str, file: &str, then: &str) -> String {
    format!("<nul set /p =%{variable}%> {file}& {then}")
}
