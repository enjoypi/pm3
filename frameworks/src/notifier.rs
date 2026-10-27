use adapters::{DesktopNotifier, NotifyBackend, Pm3Config};

#[cfg(target_os = "macos")]
const PREFERRED_NOTIFIERS: [NotifyBackend; 1] = [NotifyBackend::Osascript];
#[cfg(all(unix, not(target_os = "macos")))]
const PREFERRED_NOTIFIERS: [NotifyBackend; 1] = [NotifyBackend::NotifySend];
#[cfg(windows)]
const PREFERRED_NOTIFIERS: [NotifyBackend; 0] = [];

#[must_use]
pub fn desktop_notifier(config: &Pm3Config) -> DesktopNotifier {
    DesktopNotifier::new(
        &config.notify,
        &PREFERRED_NOTIFIERS,
        config.command_timeout_ms,
        config.search_path.clone(),
    )
}
