use adapters::DaemonCommand;
use tokio::sync::mpsc;

use super::{actor::Daemon, events::DaemonEvent};

pub async fn run(
    mut daemon: Daemon,
    commands: mpsc::Receiver<DaemonCommand>,
    mut events: mpsc::Receiver<DaemonEvent>,
) {
    tokio::spawn(forward_commands(commands, daemon.events.clone()));
    loop {
        let event = events
            .recv()
            .await
            .expect("internal error: the daemon holds an event sender, so the queue stays open");
        let last = matches!(event, DaemonEvent::Shutdown);
        daemon.apply(event).await;
        if last {
            return;
        }
    }
}

async fn forward_commands(
    mut commands: mpsc::Receiver<DaemonCommand>,
    events: mpsc::Sender<DaemonEvent>,
) {
    while let Some(command) = commands.recv().await {
        events.send(DaemonEvent::Command(command)).await.ok();
    }
}
