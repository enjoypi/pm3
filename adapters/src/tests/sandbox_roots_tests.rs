use super::*;

#[test]
fn a_trailing_slash_is_trimmed() {
    assert_eq!(normalize_root("/srv/api/"), "/srv/api");
}

#[test]
fn the_filesystem_root_is_not_trimmed_into_emptiness() {
    assert_eq!(normalize_root("/"), "/");
}

#[test]
fn repeated_slashes_still_leave_the_root() {
    assert_eq!(normalize_root("///"), "/");
}
