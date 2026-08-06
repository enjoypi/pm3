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

#[test]
fn a_path_covers_itself() {
    assert!(covers_path("/srv/api", "/srv/api/"));
}

#[test]
fn a_parent_covers_its_child() {
    assert!(covers_path("/home/me", "/home/me/.config/pm3"));
}

#[test]
fn a_sibling_sharing_a_prefix_covers_nothing() {
    assert!(!covers_path("/home/mel", "/home/me/.config/pm3"));
}

#[test]
fn a_child_does_not_cover_its_parent() {
    assert!(!covers_path("/home/me/.config", "/home/me"));
}

#[test]
fn the_filesystem_root_covers_everything() {
    assert!(covers_path("/", "/home/me/.config/pm3"));
}
