use super::*;

#[test]
fn spec_errors_convert_and_render_transparently() {
    let source = SpecError::EmptyName;
    let expected = source.to_string();
    let wrapped = EntityError::from(source);
    assert_eq!(wrapped.to_string(), expected);
}

#[test]
fn dependency_errors_convert_and_render_transparently() {
    let source = DependencyError::Cycle {
        involved: vec!["a".to_string()],
    };
    let expected = source.to_string();
    let wrapped = EntityError::from(source);
    assert_eq!(wrapped.to_string(), expected);
}

#[test]
fn policy_errors_convert_and_render_transparently() {
    let source = PolicyError::EmptyWritableRoot;
    let expected = source.to_string();
    let wrapped = EntityError::from(source);
    assert_eq!(wrapped.to_string(), expected);
}

#[test]
fn runtime_errors_convert_and_render_transparently() {
    let source = RuntimeError::RunningWithoutPid {
        app: "api".to_string(),
        status: "online".to_string(),
    };
    let expected = source.to_string();
    let wrapped = EntityError::from(source);
    assert_eq!(wrapped.to_string(), expected);
}
