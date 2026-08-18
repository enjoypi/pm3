use std::fmt;

use entities::RESERVED_ALL_SELECTOR;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppSelector {
    All,
    Id(u32),
    Name(String),
}

impl AppSelector {
    #[must_use]
    pub fn parse(raw: &str) -> Self {
        if raw == RESERVED_ALL_SELECTOR {
            return Self::All;
        }
        raw.parse::<u32>()
            .map_or_else(|_| Self::Name(raw.to_string()), Self::Id)
    }

    #[must_use]
    pub fn matches(&self, pm_id: u32, name: &str) -> bool {
        match self {
            Self::All => false,
            Self::Id(id) => *id == pm_id,
            Self::Name(candidate) => candidate == name,
        }
    }
}

impl fmt::Display for AppSelector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::All => write!(f, "{RESERVED_ALL_SELECTOR}"),
            Self::Id(id) => write!(f, "{id}"),
            Self::Name(name) => write!(f, "{name}"),
        }
    }
}

#[cfg(test)]
#[path = "tests/selector_tests.rs"]
mod tests;
