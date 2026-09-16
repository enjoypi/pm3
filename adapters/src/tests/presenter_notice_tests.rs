use super::*;

#[test]
fn the_offset_label_wraps_the_local_offset() {
    let label = local_offset_label();
    assert!(
        label.starts_with('(') && label.ends_with(')'),
        "got: {label}"
    );
    assert!(
        label.contains('+') || label.contains('-'),
        "an offset always carries a sign, got: {label}"
    );
}
