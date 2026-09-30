use super::*;

#[test]
fn an_alive_liveness_hands_over_its_token() {
    assert_eq!(
        Liveness::Alive("token".to_string()).into_token(),
        Some("token".to_string())
    );
    assert_eq!(Liveness::Gone.into_token(), None);
    assert_eq!(Liveness::Unreadable.into_token(), None);
}
