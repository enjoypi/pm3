use super::{escape::escape_xml, spec::UnitSpec};

const TASK_HEADER: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
    "<Task version=\"1.2\" xmlns=\"http://schemas.microsoft.com/windows/2004/02/mit/task\">\n"
);

const RESTART_MINIMUM_SECS: u64 = 60;
const RESTART_COUNT: u64 = 999;

const HOME_VARIABLE: &str = "HOME";
const PATH_VARIABLE: &str = "PATH";

#[must_use]
pub fn render_task_xml(spec: &UnitSpec) -> String {
    let label = escape_xml(&spec.label);
    let wrapper = escape_xml(&spec.wrapper_path().to_string_lossy());
    let working_directory = escape_xml(&spec.working_directory.to_string_lossy());
    let interval = spec.restart_delay_secs.max(RESTART_MINIMUM_SECS);
    format!(
        "{TASK_HEADER}  <RegistrationInfo>
    <Description>{label}</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal>
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
pub fn render_wrapper(spec: &UnitSpec) -> String {
    let mut script = String::from("@echo off\r\n");
    script.push_str(&render_assignment(HOME_VARIABLE, &spec.home));
    script.push_str(&render_assignment(PATH_VARIABLE, &spec.search_path));
    for (name, value) in &spec.pm3_env {
        script.push_str(&render_assignment(name, value));
    }
    let program = spec.program.to_string_lossy();
    let config = spec.config_path.to_string_lossy();
    let log_path = spec.log_path.to_string_lossy();
    let command_line = format!(
        "\"{program}\" daemon --config \"{config}\" >> \"{log_path}\" 2>&1\r\nexit /b 1\r\n"
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
