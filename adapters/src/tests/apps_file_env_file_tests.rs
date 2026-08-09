#![cfg(unix)]
use super::*;

const NAME: &str = "cloudflared";
const SHOWN: &str = "/srv/pm3/service/cloudflared";
const TOKEN_KEY: &str = "TUNNEL_TOKEN";
const TOKEN_VALUE: &str = "eyJhIjoiZjQ2";
const HOME: &str = "/home/dev";

fn parsed(text: &str) -> Vec<(String, String)> {
    parse_env_file(SHOWN, Some(HOME), text).expect("the environment file should parse")
}

fn refused(text: &str) -> EnvFileError {
    parse_env_file(SHOWN, Some(HOME), text).expect_err("the environment file should be refused")
}

fn value_of(text: &str) -> String {
    let entries = parsed(text);
    let (_, value) = entries.first().expect("one entry").clone();
    value
}

fn written(text: &str, mode: u32) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = env_file_of(dir.path(), NAME).expect("the name should be safe");
    std::fs::write(&path, text).expect("write the environment file");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
        .expect("set the starting permissions");
    (dir, path)
}

fn mode_of(path: &std::path::Path) -> u32 {
    std::fs::metadata(path)
        .expect("read the metadata")
        .permissions()
        .mode()
        & 0o777
}

#[test]
fn the_file_sits_beside_the_service_file() {
    let path = env_file_of(std::path::Path::new("/srv/pm3/service"), NAME)
        .expect("the name should be safe");
    assert_eq!(
        path,
        std::path::Path::new("/srv/pm3/service/cloudflared.env")
    );
}

#[test]
fn an_unsafe_service_name_never_becomes_a_path() {
    let refused = env_file_of(std::path::Path::new("/srv/pm3/service"), "../../.bashrc")
        .expect_err("the name should be refused");
    assert!(
        refused.to_string().contains("escape the service directory"),
        "{refused}"
    );
}

#[test]
fn a_single_pair_is_read() {
    assert_eq!(
        parsed("TUNNEL_TOKEN=eyJhIjoiZjQ2"),
        [(TOKEN_KEY.to_string(), TOKEN_VALUE.to_string())]
    );
}

#[test]
fn the_entries_come_back_in_key_order() {
    let entries = parsed("SECOND=2\nFIRST=1\n");
    let keys: Vec<&str> = entries.iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(keys, ["FIRST", "SECOND"]);
}

#[test]
fn blank_lines_and_comments_are_skipped() {
    let entries =
        parsed("# a comment\n\n   \n  # an indented comment\nTUNNEL_TOKEN=eyJhIjoiZjQ2\n");
    assert_eq!(entries.len(), 1);
}

#[test]
fn only_the_first_separator_splits_the_line() {
    assert_eq!(
        value_of("DSN=user=admin&pass=secret"),
        "user=admin&pass=secret"
    );
}

#[test]
fn an_empty_value_is_allowed() {
    assert_eq!(value_of("TUNNEL_TOKEN="), "");
}

#[test]
fn the_surrounding_whitespace_is_dropped() {
    assert_eq!(value_of("  TUNNEL_TOKEN  =  eyJhIjoiZjQ2  "), TOKEN_VALUE);
}

#[test]
fn double_fences_keep_the_whitespace_inside() {
    assert_eq!(value_of("TUNNEL_TOKEN=\"  padded  \""), "  padded  ");
}

#[test]
fn double_fences_decode_the_escapes() {
    assert_eq!(
        value_of("TUNNEL_TOKEN=\"a\\nb\\rc\\td\\\"e\\\\f\""),
        "a\nb\rc\td\"e\\f"
    );
}

#[test]
fn double_fences_decode_a_hex_escape() {
    assert_eq!(value_of("TUNNEL_TOKEN=\"a\\x07b\""), "a\u{7}b");
}

#[test]
fn a_short_hex_escape_stays_verbatim() {
    assert_eq!(value_of("TUNNEL_TOKEN=\"a\\x7\""), "a\\x7");
}

#[test]
fn a_bad_hex_escape_stays_verbatim() {
    assert_eq!(value_of("TUNNEL_TOKEN=\"a\\xzzb\""), "a\\xzzb");
}

#[test]
fn an_unknown_escape_yields_the_escaped_character() {
    assert_eq!(value_of("TUNNEL_TOKEN=\"a\\qb\""), "aqb");
}

#[test]
fn a_trailing_backslash_is_kept() {
    assert_eq!(value_of("TUNNEL_TOKEN=\"ab\\\""), "ab\\");
}

#[test]
fn single_fences_keep_their_body_untouched() {
    assert_eq!(value_of("TUNNEL_TOKEN='a\\nb'"), "a\\nb");
}

#[test]
fn a_bare_home_expands_so_a_search_path_needs_no_absolute_prefix() {
    assert_eq!(
        value_of("PATH=$HOME/.cargo/bin:/usr/bin:/bin"),
        "/home/dev/.cargo/bin:/usr/bin:/bin"
    );
}

#[test]
fn a_braced_home_expands_too() {
    assert_eq!(
        value_of("PATH=${HOME}/bin:${HOME}/.local/bin"),
        "/home/dev/bin:/home/dev/.local/bin"
    );
}

#[test]
fn a_home_at_the_very_end_expands() {
    assert_eq!(value_of("WORKDIR=$HOME"), "/home/dev");
}

#[test]
fn a_longer_variable_name_is_left_alone() {
    assert_eq!(
        value_of("PATH=$HOMEBREW_PREFIX/bin:$HOME/bin"),
        "$HOMEBREW_PREFIX/bin:/home/dev/bin",
        "only the home itself expands, never a variable that merely starts with it"
    );
}

#[test]
fn every_other_dollar_stays_verbatim() {
    assert_eq!(value_of("TUNNEL_TOKEN=p$4ss${WORD}"), "p$4ss${WORD}");
}

#[test]
fn single_fences_hand_a_dollar_home_through_untouched() {
    assert_eq!(
        value_of("TUNNEL_TOKEN='$HOME'"),
        "$HOME",
        "single quotes are the escape hatch for a password that spells out $HOME"
    );
}

#[test]
fn a_host_without_a_home_expands_nothing() {
    let entries =
        parse_env_file(SHOWN, None, "PATH=$HOME/bin").expect("the environment file should parse");
    let (_, value) = entries.first().expect("one entry").clone();
    assert_eq!(value, "$HOME/bin");
}

#[test]
fn a_lone_fence_is_not_a_fenced_value() {
    assert_eq!(value_of("TUNNEL_TOKEN=\""), "\"");
}

#[test]
fn a_line_without_a_separator_is_refused() {
    let refused = refused("TUNNEL_TOKEN\n");
    assert_eq!(
        refused.to_string(),
        format!("cannot parse the environment file '{SHOWN}' at line 1: expected KEY=VALUE")
    );
}

#[test]
fn a_blank_key_is_refused() {
    let refused = refused("=eyJhIjoiZjQ2\n");
    assert!(refused.to_string().contains("at line 1"), "{refused}");
}

#[test]
fn a_key_starting_with_a_digit_is_refused() {
    let refused = refused("1TOKEN=eyJhIjoiZjQ2\n");
    assert_eq!(
        refused.to_string(),
        format!(
            "cannot accept the key '1TOKEN' in the environment file '{SHOWN}' at line 1: use letters, digits and '_', and do not start with a digit"
        )
    );
}

#[test]
fn a_key_with_a_dash_is_refused() {
    let refused = refused("TUNNEL-TOKEN=eyJhIjoiZjQ2\n");
    assert!(refused.to_string().contains("TUNNEL-TOKEN"), "{refused}");
}

#[test]
fn an_exported_key_is_refused_because_of_the_space() {
    let refused = refused("export TUNNEL_TOKEN=eyJhIjoiZjQ2\n");
    assert!(
        refused.to_string().contains("export TUNNEL_TOKEN"),
        "{refused}"
    );
}

#[test]
fn a_repeated_key_is_refused() {
    let refused = refused("TUNNEL_TOKEN=one\nTUNNEL_TOKEN=two\n");
    assert_eq!(
        refused.to_string(),
        format!(
            "cannot accept the key 'TUNNEL_TOKEN' twice in the environment file '{SHOWN}': line 2 repeats it"
        )
    );
}

#[test]
fn no_refusal_ever_shows_the_value() {
    let refused = refused(&format!("TUNNEL_TOKEN=one\nTUNNEL_TOKEN={TOKEN_VALUE}\n"));
    assert!(!refused.to_string().contains(TOKEN_VALUE), "{refused}");
}

#[tokio::test]
async fn a_missing_file_means_no_environment() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = env_file_of(dir.path(), NAME).expect("the name should be safe");
    let loaded = load_env_file(&path, Some(HOME))
        .await
        .expect("a missing file is fine");
    assert!(loaded.is_empty());
}

#[tokio::test]
async fn a_present_file_is_loaded() {
    let (_dir, path) = written("TUNNEL_TOKEN=eyJhIjoiZjQ2\n", 0o600);
    let loaded = load_env_file(&path, Some(HOME))
        .await
        .expect("the file should load");
    assert_eq!(loaded, [(TOKEN_KEY.to_string(), TOKEN_VALUE.to_string())]);
}

#[tokio::test]
async fn loading_tightens_a_world_readable_file() {
    let (_dir, path) = written("TUNNEL_TOKEN=eyJhIjoiZjQ2\n", 0o644);
    load_env_file(&path, Some(HOME))
        .await
        .expect("the file should load");
    assert_eq!(mode_of(&path), 0o600);
}

#[tokio::test]
async fn an_already_private_file_keeps_its_mode() {
    let (_dir, path) = written("TUNNEL_TOKEN=eyJhIjoiZjQ2\n", 0o600);
    load_env_file(&path, Some(HOME))
        .await
        .expect("the file should load");
    assert_eq!(mode_of(&path), 0o600);
}

#[tokio::test]
async fn an_unreadable_file_is_reported_without_its_contents() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, "not a directory").expect("write the blocker");
    let path = env_file_of(&blocker, NAME).expect("the name should be safe");
    let refused = load_env_file(&path, Some(HOME))
        .await
        .expect_err("a file below a file should be refused");
    assert!(
        refused
            .to_string()
            .starts_with("cannot read the environment file"),
        "{refused}"
    );
}

#[tokio::test]
async fn a_linked_file_is_read_but_its_target_keeps_its_mode() {
    let (_dir, target) = written("TUNNEL_TOKEN=eyJhIjoiZjQ2\n", 0o644);
    let beside = tempfile::tempdir().expect("create temp dir");
    let link = env_file_of(beside.path(), NAME).expect("the name should be safe");
    std::os::unix::fs::symlink(&target, &link).expect("link the shared secret into place");
    let loaded = load_env_file(&link, Some(HOME))
        .await
        .expect("the file should load");
    assert_eq!(loaded, [(TOKEN_KEY.to_string(), TOKEN_VALUE.to_string())]);
    assert_eq!(
        mode_of(&target),
        0o644,
        "pm3 must not re-permission a file that belongs to someone else"
    );
}

#[tokio::test]
async fn a_file_that_cannot_be_tightened_only_warns() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let path = env_file_of(dir.path(), NAME).expect("the name should be safe");
    secure_file(&path, &path.to_string_lossy()).await;
    assert!(!path.exists());
}
