mod desktop;
mod message;

pub use self::{
    desktop::{DesktopNotifier, NotifyBackend},
    message::render_alert,
};
