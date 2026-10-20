#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum Priority {
    #[default]
    Background,
    Standard,
    Interactive,
}

impl Priority {
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "background" => Some(Self::Background),
            "standard" => Some(Self::Standard),
            "interactive" => Some(Self::Interactive),
            _ => None,
        }
    }
}

#[cfg(test)]
#[path = "../tests/process_priority_tests.rs"]
mod tests;
