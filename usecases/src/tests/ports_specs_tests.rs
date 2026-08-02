use super::*;

#[test]
fn a_missing_declaration_keeps_the_reason_as_its_message() {
    let error = SpecResolveError::Missing {
        name: "web".to_string(),
        reason: "cannot find app 'web' in its own service file".to_string(),
    };
    assert_eq!(
        error.to_string(),
        "cannot find app 'web' in its own service file"
    );
}

#[test]
fn an_unusable_declaration_keeps_the_reason_as_its_message() {
    let error = SpecResolveError::Unusable {
        name: "web".to_string(),
        reason: "cannot read apps file '/x': nope".to_string(),
    };
    assert_eq!(error.to_string(), "cannot read apps file '/x': nope");
}

#[test]
fn a_missing_declaration_names_the_app_it_is_about() {
    let error = SpecResolveError::Missing {
        name: "web".to_string(),
        reason: "gone".to_string(),
    };
    assert_eq!(error.app(), "web");
}

#[test]
fn an_unusable_declaration_names_the_app_it_is_about() {
    let error = SpecResolveError::Unusable {
        name: "api".to_string(),
        reason: "broken".to_string(),
    };
    assert_eq!(error.app(), "api");
}
