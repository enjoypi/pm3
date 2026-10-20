use tokio::runtime::{Builder, Runtime};

use crate::cli::Commands;

pub fn runtime_for(command: &Commands) -> std::io::Result<Runtime> {
    let mut builder = Builder::new_multi_thread();
    builder.enable_all();
    if matches!(command, Commands::Daemon) {
        demote(&mut builder);
    }
    builder.build()
}

#[cfg(target_os = "macos")]
fn demote(builder: &mut Builder) {
    demote_current_thread();
    builder.on_thread_start(demote_current_thread);
}

#[cfg(not(target_os = "macos"))]
const fn demote(_builder: &mut Builder) {}

#[cfg(target_os = "macos")]
#[expect(
    unsafe_code,
    clippy::undocumented_unsafe_blocks,
    reason = "pthread_set_qos_class_self_np only changes the calling thread's own QoS class"
)]
fn demote_current_thread() {
    unsafe {
        libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_BACKGROUND, 0);
    }
}
