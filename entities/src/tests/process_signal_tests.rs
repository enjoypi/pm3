use super::*;

#[test]
fn a_bare_signal_name_is_accepted() {
    assert_eq!(parse_signal_name("HUP").as_deref(), Ok("HUP"));
}

#[test]
fn a_lowercase_signal_name_is_normalized() {
    assert_eq!(parse_signal_name("usr1").as_deref(), Ok("USR1"));
}

#[test]
fn an_unknown_signal_name_is_rejected() {
    let err = parse_signal_name("KILL9").unwrap_err();
    assert_eq!(
        err.to_string(),
        "unknown signal 'KILL9'; valid signals: TERM, INT, QUIT, HUP, USR1, USR2"
    );
}

#[test]
fn every_documented_signal_round_trips() {
    for name in VALID_SIGNALS {
        assert_eq!(parse_signal_name(name).as_deref(), Ok(name));
    }
}
