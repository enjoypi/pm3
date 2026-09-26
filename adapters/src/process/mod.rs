mod kill_signaler;
#[cfg(unix)]
mod ps_probe;
mod ready_probe;
mod sha256_fingerprinter;
mod system_clock;
mod timed;
mod tokio_launcher;
mod watcher;
#[cfg(windows)]
mod win_probe;

#[cfg(unix)]
pub use self::ps_probe::PsProcessProbe as HostProcessProbe;
#[cfg(windows)]
pub use self::win_probe::WinProcessProbe as HostProcessProbe;
pub use self::{
    kill_signaler::KillSignaler,
    ready_probe::HostReadyProber,
    sha256_fingerprinter::Sha256Fingerprinter,
    system_clock::SystemClock,
    timed::{CommandOutcome, capture_timed},
    tokio_launcher::TokioProcessLauncher,
    watcher::{AdoptedWatch, PollCadence, wait_for_exit, wait_until_released},
};
