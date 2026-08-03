use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TimerState {
    fires: HashMap<String, u64>,
    restarts: HashSet<String>,
    generations: HashMap<String, u64>,
    next_generation: u64,
}

impl TimerState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn next_fire_of(&self, name: &str) -> Option<u64> {
        self.fires.get(name).copied()
    }

    #[must_use]
    pub fn fire_is_due(&self, name: &str, fire_at_ms: u64) -> bool {
        self.next_fire_of(name) == Some(fire_at_ms)
    }

    pub fn arm(&mut self, name: &str, fire_at_ms: u64) {
        self.fires.insert(name.to_string(), fire_at_ms);
    }

    pub fn disarm(&mut self, name: &str) {
        self.fires.remove(name);
    }

    pub fn disarm_all(&mut self) -> Vec<String> {
        self.fires.drain().map(|(name, _fire)| name).collect()
    }

    pub fn queue_restart(&mut self, name: &str) {
        self.restarts.insert(name.to_string());
    }

    pub fn claim_restart(&mut self, name: &str) -> bool {
        self.restarts.remove(name)
    }

    pub fn cancel_all_restarts(&mut self) -> Vec<String> {
        self.restarts.drain().collect()
    }

    pub fn bump(&mut self, name: &str) -> u64 {
        self.next_generation = self.next_generation.saturating_add(1);
        self.generations
            .insert(name.to_string(), self.next_generation);
        self.next_generation
    }

    #[must_use]
    pub fn is_current(&self, name: &str, generation: u64) -> bool {
        self.current_generation(name) == generation
    }

    #[must_use]
    pub fn current_generation(&self, name: &str) -> u64 {
        self.generations.get(name).copied().unwrap_or_default()
    }
}

#[cfg(test)]
#[path = "tests/timer_state_tests.rs"]
mod tests;
