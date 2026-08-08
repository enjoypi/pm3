use super::*;
use crate::ports_test_helpers::FakePorts;

#[test]
fn an_alive_liveness_hands_over_its_token() {
    assert_eq!(
        Liveness::Alive("token".to_string()).into_token(),
        Some("token".to_string())
    );
    assert_eq!(Liveness::Gone.into_token(), None);
    assert_eq!(Liveness::Unreadable.into_token(), None);
}

#[tokio::test]
async fn the_fake_reports_only_the_seeded_resources() {
    let ports = FakePorts::new(0);
    ports.seed_resource(
        7,
        ResourceSample {
            rss_kib: 4096,
            cpu_tenths: 15,
        },
    );
    let sampled = ports.resource_usage(&[7, 8]).await;
    assert_eq!(
        sampled.get(&7),
        Some(&ResourceSample {
            rss_kib: 4096,
            cpu_tenths: 15,
        })
    );
    assert_eq!(sampled.get(&8), None);
}
