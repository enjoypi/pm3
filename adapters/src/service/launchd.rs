use super::spec::ServiceUnitSpec;

const PLIST_HEADER: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
    "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" ",
    "\"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n",
    "<plist version=\"1.0\">\n"
);

const PATH_VARIABLE: &str = "PATH";
const HOME_VARIABLE: &str = "HOME";

#[must_use]
pub fn render_plist(spec: &ServiceUnitSpec) -> String {
    let arguments = spec
        .daemon_args()
        .iter()
        .map(|argument| render_argument(argument))
        .collect::<String>();
    let program = render_argument(&spec.program.to_string_lossy());
    let label = escape_xml(&spec.label);
    let working_directory = escape_xml(&spec.working_directory.to_string_lossy());
    let log_path = escape_xml(&spec.log_path.to_string_lossy());
    let search_path = escape_xml(&spec.search_path);
    let home = escape_xml(&spec.home);
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
    <true/>
    <key>AbandonProcessGroup</key>
    <true/>
    <key>ProcessType</key>
    <string>Background</string>
    <key>StandardOutPath</key>
    <string>{log_path}</string>
    <key>StandardErrorPath</key>
    <string>{log_path}</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>{HOME_VARIABLE}</key>
        <string>{home}</string>
        <key>{PATH_VARIABLE}</key>
        <string>{search_path}</string>
    </dict>
</dict>
</plist>
"
    )
}

fn render_argument(raw: &str) -> String {
    let escaped = escape_xml(raw);
    format!("        <string>{escaped}</string>\n")
}

fn escape_xml(raw: &str) -> String {
    let mut escaped = String::with_capacity(raw.len());
    for character in raw.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            other => escaped.push(other),
        }
    }
    escaped
}

#[cfg(test)]
#[path = "../tests/service_launchd_tests.rs"]
mod tests;
