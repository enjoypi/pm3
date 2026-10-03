use super::*;
use crate::config_sections::fixture_config;

#[test]
fn the_startup_banner_accepts_every_configured_setting() {
    let cfg = fixture_config("/tmp/pm3-fixture", "workspace-write");
    log_startup_banner(&cfg, "1.17.0", "/tmp/pm3-fixture/pm3.sock");
}
