use super::*;

fn named(name: &str) -> AppSelector {
    AppSelector::Name(name.to_string())
}

#[test]
fn every_request_names_its_action() {
    let actions = [
        (
            SupervisionRequest::Start {
                services: Vec::new(),
            },
            "start",
        ),
        (SupervisionRequest::List, "list"),
        (SupervisionRequest::Describe(named("web")), "describe"),
        (SupervisionRequest::Stop(named("web")), "stop"),
        (SupervisionRequest::Restart(named("web")), "restart"),
        (SupervisionRequest::Delete(named("web")), "delete"),
        (SupervisionRequest::Reset(named("web")), "reset"),
        (
            SupervisionRequest::Signal {
                selector: named("web"),
                signal: "HUP".to_string(),
            },
            "signal",
        ),
        (SupervisionRequest::StopAll, "stop_all"),
    ];
    for (request, action) in actions {
        assert_eq!(request.action(), action, "got: {request:?}");
    }
}

#[test]
fn a_start_request_targets_every_service_it_names() {
    let request = SupervisionRequest::Start {
        services: vec!["web".to_string(), "api".to_string()],
    };
    assert_eq!(request.target(), "web,api");
}

#[test]
fn a_table_wide_request_targets_nothing_in_particular() {
    assert_eq!(SupervisionRequest::List.target(), "");
    assert_eq!(SupervisionRequest::StopAll.target(), "");
}

#[test]
fn a_single_app_request_targets_its_selector() {
    let selectors = [
        SupervisionRequest::Describe(named("web")),
        SupervisionRequest::Stop(named("web")),
        SupervisionRequest::Restart(named("web")),
        SupervisionRequest::Delete(named("web")),
        SupervisionRequest::Reset(named("web")),
        SupervisionRequest::Signal {
            selector: named("web"),
            signal: "HUP".to_string(),
        },
    ];
    for request in selectors {
        assert_eq!(request.target(), "web", "got: {request:?}");
    }
}

#[test]
fn a_usecase_failure_renders_transparently() {
    let failure = SupervisionFailure::Usecase(UsecaseError::NotFound("web".to_string()));
    assert_eq!(failure.to_string(), "cannot find app 'web'");
}

#[test]
fn a_spec_failure_renders_transparently() {
    let failure = SupervisionFailure::Spec(SpecResolveError::Missing {
        name: "web".to_string(),
        reason: "gone".to_string(),
    });
    assert_eq!(failure.to_string(), "gone");
}
