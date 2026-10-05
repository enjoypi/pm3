use super::*;

#[test]
fn the_soft_open_file_limit_reaches_the_hard_cap() {
    lift_open_file_limit();
    let Rlimit { current, maximum } = getrlimit(Resource::Nofile);
    assert_eq!(current, maximum, "soft limit must equal the hard cap");
}
