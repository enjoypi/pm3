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

#[test]
fn a_rooted_path_is_absolute() {
    assert!(is_absolute_path("/srv/api"));
}

#[test]
fn a_bare_name_is_not_absolute() {
    assert!(!is_absolute_path("srv/api"));
}

#[cfg(windows)]
#[test]
fn a_drive_letter_path_is_absolute_on_windows() {
    assert!(is_absolute_path("C:/Users/dev"));
}

#[cfg(windows)]
#[test]
fn a_drive_path_with_a_backslash_is_left_to_the_edge_to_normalize() {
    assert!(!is_absolute_path(r"C:\Users\dev"));
}

#[cfg(windows)]
#[test]
fn a_digit_before_the_colon_is_no_drive() {
    assert!(!is_absolute_path("1:/Users/dev"));
}
