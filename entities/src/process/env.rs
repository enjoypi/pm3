use std::collections::BTreeMap;

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum EnvScope {
    Injected,
    Global,
    #[default]
    App,
}

impl EnvScope {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Injected => "pm3",
            Self::Global => "global",
            Self::App => "app",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnvValue {
    pub key: String,
    pub value: String,
    pub scope: EnvScope,
}

impl EnvValue {
    #[must_use]
    pub fn new(key: &str, value: &str, scope: EnvScope) -> Self {
        Self {
            key: key.to_string(),
            value: value.to_string(),
            scope,
        }
    }

    #[must_use]
    pub fn injected(key: &str, value: &str) -> Self {
        Self::new(key, value, EnvScope::Injected)
    }

    #[must_use]
    pub fn global(key: &str, value: &str) -> Self {
        Self::new(key, value, EnvScope::Global)
    }

    #[must_use]
    pub fn app(key: &str, value: &str) -> Self {
        Self::new(key, value, EnvScope::App)
    }
}

#[must_use]
pub fn merge_environment(layers: &[&[EnvValue]]) -> Vec<EnvValue> {
    let mut merged: BTreeMap<&str, &EnvValue> = BTreeMap::new();
    for layer in layers {
        for entry in *layer {
            merged.insert(entry.key.as_str(), entry);
        }
    }
    merged.into_values().cloned().collect()
}

#[cfg(test)]
#[path = "../tests/process_env_tests.rs"]
mod tests;
