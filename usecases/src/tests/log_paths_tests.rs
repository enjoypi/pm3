use super::*;

#[test]
fn paths_use_the_out_and_err_suffixes() {
    let paths = log_paths("/home/u/.pm3/logs", "api");
    assert_eq!(paths.stdout, "/home/u/.pm3/logs/api-out.log");
    assert_eq!(paths.stderr, "/home/u/.pm3/logs/api-err.log");
}

#[test]
fn a_trailing_slash_does_not_double_up() {
    let paths = log_paths("/home/u/.pm3/logs/", "api");
    assert_eq!(paths.stdout, "/home/u/.pm3/logs/api-out.log");
}

#[test]
fn repeated_trailing_slashes_collapse() {
    let paths = log_paths("/logs///", "api");
    assert_eq!(paths.stderr, "/logs/api-err.log");
}

#[test]
fn a_single_stream_path_picks_the_matching_suffix() {
    assert_eq!(
        log_path("/home/u/.pm3/logs", "api", LogStream::Stdout),
        "/home/u/.pm3/logs/api-out.log"
    );
    assert_eq!(
        log_path("/home/u/.pm3/logs", "api", LogStream::Stderr),
        "/home/u/.pm3/logs/api-err.log"
    );
}
