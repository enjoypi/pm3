use super::*;
use crate::config_sections::{pm3_section, telemetry_section};

#[test]
fn the_startup_banner_accepts_every_configured_setting() {
    let yaml = format!(
        "{}{}",
        pm3_section("/tmp/pm3-fixture", 1600, "workspace-write"),
        telemetry_section("info"),
    );
    let cfg = crate::parse_config(&yaml).expect("the fixture config should parse");
    log_startup_banner(&cfg, "1.17.0", "/tmp/pm3-fixture/pm3.sock");
}
