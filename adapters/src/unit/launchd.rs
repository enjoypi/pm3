use super::{escape::escape_xml, spec::UnitSpec};
use crate::config::RESTART_CONDITION_ON_FAILURE;

const PLIST_HEADER: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
    "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" ",
    "\"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n",
    "<plist version=\"1.0\">\n"
);

const KEEP_ALIVE_ALWAYS: &str = "<true/>";
const KEEP_ALIVE_ON_FAILURE: &str = concat!(
    "<dict>\n",
    "        <key>SuccessfulExit</key>\n",
    "        <false/>\n",
    "    </dict>"
);

#[must_use]
pub fn render_plist(spec: &UnitSpec) -> String {
    let arguments = spec
        .daemon_args()
        .iter()
        .map(|argument| render_argument(argument))
        .collect::<String>();
    let program = render_argument(&spec.program.to_string_lossy());
    let label = escape_xml(&spec.label);
    let working_directory = escape_xml(&spec.working_directory.to_string_lossy());
    let log_path = escape_xml(&spec.log_path.to_string_lossy());
    let keep_alive = keep_alive_of(&spec.restart_condition);
    let environment = render_environment(&spec.environment_pairs());
    let umask = spec.umask;
    let max_tasks = spec.max_tasks;
    format!(
        "{PLIST_HEADER}<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
{program}{arguments}    </array>
    <key>WorkingDirectory</key>
    <string>{working_directory}</string>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    {keep_alive}
    <key>AbandonProcessGroup</key>
    <true/>
    <key>ProcessType</key>
    <string>Background</string>
    <key>Umask</key>
    <integer>{umask}</integer>
    <key>HardResourceLimits</key>
    <dict>
        <key>NumberOfProcesses</key>
        <integer>{max_tasks}</integer>
    </dict>
    <key>StandardOutPath</key>
    <string>{log_path}</string>
    <key>StandardErrorPath</key>
    <string>{log_path}</string>
    <key>EnvironmentVariables</key>
    <dict>
{environment}    </dict>
</dict>
</plist>
"
    )
}

fn render_environment(vars: &[(&str, &str)]) -> String {
    vars.iter().map(render_variable).collect()
}

fn render_variable(entry: &(&str, &str)) -> String {
    let (name, value) = entry;
    let key = escape_xml(name);
    let text = escape_xml(value);
    format!("        <key>{key}</key>\n        <string>{text}</string>\n")
}

fn render_argument(raw: &str) -> String {
    let escaped = escape_xml(raw);
    format!("        <string>{escaped}</string>\n")
}

fn keep_alive_of(restart_condition: &str) -> &'static str {
    if restart_condition == RESTART_CONDITION_ON_FAILURE {
        return KEEP_ALIVE_ON_FAILURE;
    }
    KEEP_ALIVE_ALWAYS
}

#[cfg(test)]
#[path = "../tests/unit_launchd_tests.rs"]
mod tests;
