use std::io::Cursor;

use super::*;

fn changed(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_string()).collect()
}

#[test]
fn a_changed_service_that_is_running_is_offered_a_restart() {
    let names = changed(&["web"]);
    let pending = stale_running(&names, &changed(&["web"]));
    assert_eq!(pending, vec!["web"]);
}

#[test]
fn a_changed_service_that_was_just_started_is_not_offered_a_restart() {
    let names = changed(&["db"]);
    let pending = stale_running(&names, &changed(&["web"]));
    assert!(pending.is_empty(), "got: {pending:?}");
}

#[test]
fn an_unchanged_running_service_is_not_offered_a_restart() {
    let names = changed(&[]);
    let pending = stale_running(&names, &changed(&["web"]));
    assert!(pending.is_empty(), "got: {pending:?}");
}

#[test]
fn a_service_name_that_prefixes_another_does_not_match() {
    let names = changed(&["web"]);
    let pending = stale_running(&names, &changed(&["web2"]));
    assert!(pending.is_empty(), "got: {pending:?}");
}

#[test]
fn answering_y_confirms_the_restart() {
    let mut input = Cursor::new("y\n");
    let mut output: Vec<u8> = Vec::new();
    assert!(confirm_restart("web", &mut input, &mut output));
}

#[test]
fn answering_yes_in_any_case_confirms_the_restart() {
    let mut input = Cursor::new("YES\n");
    let mut output: Vec<u8> = Vec::new();
    assert!(confirm_restart("web", &mut input, &mut output));
}

#[test]
fn a_blank_answer_declines_the_restart() {
    let mut input = Cursor::new("\n");
    let mut output: Vec<u8> = Vec::new();
    assert!(!confirm_restart("web", &mut input, &mut output));
}

#[test]
fn an_explicit_no_declines_the_restart() {
    let mut input = Cursor::new("n\n");
    let mut output: Vec<u8> = Vec::new();
    assert!(!confirm_restart("web", &mut input, &mut output));
}

#[test]
fn an_eof_answer_declines_the_restart() {
    let mut input = Cursor::new("");
    let mut output: Vec<u8> = Vec::new();
    assert!(!confirm_restart("web", &mut input, &mut output));
}

#[test]
fn the_prompt_names_the_service_and_the_default() {
    let mut input = Cursor::new("n\n");
    let mut output: Vec<u8> = Vec::new();
    confirm_restart("web", &mut input, &mut output);
    let shown = String::from_utf8(output).expect("prompt output is utf-8");
    assert_eq!(shown, "config changed for 'web'; restart to apply? [y/N] ");
}

#[test]
fn an_answer_without_a_newline_is_followed_by_one() {
    let mut input = Cursor::new("y");
    let mut output: Vec<u8> = Vec::new();
    assert!(confirm_restart("web", &mut input, &mut output));
    let shown = String::from_utf8(output).expect("prompt output is utf-8");
    assert!(shown.ends_with("? [y/N] \n"), "got: {shown:?}");
}

#[test]
fn a_failing_reader_declines_the_restart() {
    let mut input = FailingReader;
    let mut output: Vec<u8> = Vec::new();
    assert!(!confirm_restart("web", &mut input, &mut output));
}

#[test]
fn the_hint_names_the_service_and_the_restart_command() {
    assert_eq!(
        keep_old_config_hint("web"),
        "'web' keeps running with the previous config; run 'pm3 restart web' to apply the new one"
    );
}

struct FailingReader;

impl std::io::BufRead for FailingReader {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        Err(std::io::Error::other("reader broke"))
    }

    fn consume(&mut self, _amount: usize) {}
}

impl std::io::Read for FailingReader {
    fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("reader broke"))
    }
}
