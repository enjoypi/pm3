use super::*;
use crate::ports_test_helpers::FakePorts;

#[tokio::test]
async fn the_fake_is_always_ready() {
    let ports = FakePorts::new(0);
    let probe = ReadyProbe::Tcp {
        host: "127.0.0.1".to_string(),
        port: 8080,
    };
    assert_eq!(ports.check_ready(&probe).await, Readiness::Ready);
}
