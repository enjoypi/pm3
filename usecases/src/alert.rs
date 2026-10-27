use std::{collections::HashMap, mem::Discriminant};

use crate::{ports::ExitOutcome, supervision::SupervisionEffect};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Alert {
    pub name: String,
    pub kind: AlertKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AlertKind {
    Exited(ExitOutcome),
    ReadyFailed { downstream: Vec<String> },
    LivenessKilled,
    MemoryKilled,
    LaunchFailed,
}

#[derive(Debug)]
pub struct AlertLatch {
    cooldown_ms: u64,
    fired: HashMap<(String, Discriminant<AlertKind>), u64>,
}

impl AlertLatch {
    #[must_use]
    pub fn new(cooldown_ms: u64) -> Self {
        Self {
            cooldown_ms,
            fired: HashMap::new(),
        }
    }

    pub fn fire(&mut self, name: &str, kind: AlertKind, now_ms: u64) -> Option<SupervisionEffect> {
        let key = (name.to_string(), std::mem::discriminant(&kind));
        let cooling = self
            .fired
            .get(&key)
            .is_some_and(|&last_ms| now_ms.saturating_sub(last_ms) < self.cooldown_ms);
        if cooling {
            return None;
        }
        self.fired.insert(key, now_ms);
        Some(SupervisionEffect::Notify(Alert {
            name: name.to_string(),
            kind,
        }))
    }

    pub fn settle(
        &mut self,
        name: &str,
        outcome: ExitOutcome,
        alarming: bool,
        now_ms: u64,
    ) -> Option<SupervisionEffect> {
        if alarming {
            return self.fire(name, AlertKind::Exited(outcome), now_ms);
        }
        self.forget(name);
        None
    }

    pub fn forget(&mut self, name: &str) {
        self.fired.retain(|(fired, _kind), _last_ms| fired != name);
    }
}

#[cfg(test)]
#[path = "tests/alert_tests.rs"]
mod tests;
