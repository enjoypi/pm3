use super::*;

fn body_of(kind: AlertKind) -> String {
    let alert = Alert {
        name: "api".to_string(),
        kind,
    };
    render_alert(&NotifyConfig::default(), &alert).1
}

#[test]
fn the_title_names_the_service() {
    let alert = Alert {
        name: "api".to_string(),
        kind: AlertKind::LaunchFailed,
    };

    let (title, _body) = render_alert(&NotifyConfig::default(), &alert);

    assert_eq!(title, "pm3: api");
}

#[test]
fn an_exit_code_is_spelled_out() {
    assert_eq!(
        body_of(AlertKind::Exited(ExitOutcome::Code(3))),
        "异常退出，退出码 3"
    );
}

#[test]
fn an_exit_without_a_code_shares_one_message() {
    let signalled = body_of(AlertKind::Exited(ExitOutcome::Signalled));
    let unobserved = body_of(AlertKind::Exited(ExitOutcome::Unobserved));

    assert_eq!(signalled, "被信号终止或异常消失");
    assert_eq!(unobserved, signalled);
}

#[test]
fn a_ready_failure_lists_the_cancelled_downstream() {
    let lone = body_of(AlertKind::ReadyFailed {
        downstream: Vec::new(),
    });
    let cascaded = body_of(AlertKind::ReadyFailed {
        downstream: vec!["web".to_string(), "api".to_string()],
    });

    assert_eq!(lone, "就绪检查失败");
    assert_eq!(cascaded, "就绪检查失败，下游未启动：web, api");
}

#[test]
fn each_restart_reason_has_its_own_message() {
    assert_eq!(
        body_of(AlertKind::LivenessKilled),
        "存活检查连续失败，已重启"
    );
    assert_eq!(body_of(AlertKind::MemoryKilled), "内存超限，已重启");
    assert_eq!(body_of(AlertKind::LaunchFailed), "自动启动失败");
}
