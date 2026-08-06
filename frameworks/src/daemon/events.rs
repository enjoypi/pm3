use adapters::{DaemonCommand, ExitOutcome};

#[derive(Debug)]
pub enum DaemonEvent {
    Command(DaemonCommand),
    Exited {
        name: String,
        generation: u64,
        outcome: ExitOutcome,
    },
    Restart {
        name: String,
    },
    Fire {
        name: String,
        fire_at_ms: u64,
    },
    ForceKill {
        name: String,
        generation: u64,
        pid: u32,
        token: Option<String>,
    },
    SampleMemory,
    Shutdown,
}
