use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppSelector {
    Id(u32),
    Name(String),
}

impl AppSelector {
    #[must_use]
    pub fn parse(raw: &str) -> Self {
        raw.parse::<u32>()
            .map_or_else(|_| Self::Name(raw.to_string()), Self::Id)
    }

    #[must_use]
    pub fn matches(&self, pm_id: u32, name: &str) -> bool {
        match self {
            Self::Id(id) => *id == pm_id,
            Self::Name(candidate) => candidate == name,
        }
    }
}

impl fmt::Display for AppSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Id(id) => write!(f, "{id}"),
            Self::Name(name) => write!(f, "{name}"),
        }
    }
}

#[cfg(test)]
#[path = "tests/selector_tests.rs"]
mod tests;
