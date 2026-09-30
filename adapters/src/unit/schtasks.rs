use super::{
    escape::escape_xml,
    spec::{CONFIG_FLAG, DAEMON_SUBCOMMAND, UnitSpec},
};

const UTF16_DECLARATION: &str = "<?xml version=\"1.0\" encoding=\"UTF-16\"?>\n";
const TASK_OPENING: &str =
    "<Task version=\"1.2\" xmlns=\"http://schemas.microsoft.com/windows/2004/02/mit/task\">\n";
const UTF16_LE_BOM: [u8; 2] = [0xFF, 0xFE];

const RESTART_MINIMUM_SECS: u64 = 60;
const RESTART_COUNT: u64 = 999;

#[must_use]
pub fn render_task_xml(spec: &UnitSpec) -> String {
    let label = escape_xml(&spec.label);
    let account = escape_xml(&spec.account);
    let wrapper = escape_xml(&spec.wrapper_path().to_string_lossy());
    let working_directory = escape_xml(&spec.working_directory.to_string_lossy());
    let interval = spec.restart_delay_secs.max(RESTART_MINIMUM_SECS);
    format!(
        "{UTF16_DECLARATION}{TASK_OPENING}  <RegistrationInfo>
    <Description>{label}</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{account}</UserId>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal>
      <UserId>{account}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>false</AllowHardTerminate>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <RestartOnFailure>
      <Interval>PT{interval}S</Interval>
      <Count>{RESTART_COUNT}</Count>
    </RestartOnFailure>
  </Settings>
  <Actions>
    <Exec>
      <Command>{wrapper}</Command>
      <WorkingDirectory>{working_directory}</WorkingDirectory>
    </Exec>
  </Actions>
</Task>
"
    )
}

#[must_use]
pub fn encode_for_disk(contents: &str) -> Vec<u8> {
    if contents.starts_with(UTF16_DECLARATION) {
        return UTF16_LE_BOM
            .into_iter()
            .chain(contents.encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
    }
    contents.as_bytes().to_vec()
}

#[must_use]
pub fn render_wrapper(spec: &UnitSpec) -> String {
    let mut script = String::from("@echo off\r\n");
    for (name, value) in spec.environment_pairs() {
        script.push_str(&render_assignment(name, value));
    }
    let program = spec.program.to_string_lossy();
    let config = spec.config_path.to_string_lossy();
    let log_path = spec.log_path.to_string_lossy();
    let command_line = format!(
        "\"{program}\" {DAEMON_SUBCOMMAND} {CONFIG_FLAG} \"{config}\" >> \"{log_path}\" 2>&1\r\nexit /b 1\r\n"
    );
    script.push_str(&command_line);
    script
}

fn render_assignment(name: &str, value: &str) -> String {
    let escaped = escape_batch(value);
    format!("set \"{name}={escaped}\"\r\n")
}

fn escape_batch(raw: &str) -> String {
    raw.replace('%', "%%")
}

#[cfg(test)]
#[path = "../tests/unit_schtasks_tests.rs"]
mod tests;
