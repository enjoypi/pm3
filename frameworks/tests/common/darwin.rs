use std::mem::{MaybeUninit, size_of};

pub const BACKGROUND_BAND: i32 = 4;
pub const STANDARD_BAND: i32 = 20;
const PROC_PIDLISTTHREADS: libc::c_int = 6;
const MAX_THREADS: usize = 256;

#[expect(
    unsafe_code,
    reason = "the responsible pid is private libSystem API with no binding"
)]
unsafe extern "C" {
    safe fn responsibility_get_pid_responsible_for_pid(pid: libc::pid_t) -> libc::pid_t;
}

#[must_use]
pub fn responsible_pid(pid: libc::pid_t) -> libc::pid_t {
    responsibility_get_pid_responsible_for_pid(pid)
}

#[must_use]
pub fn base_priority(pid: libc::pid_t) -> i32 {
    let info: libc::proc_taskinfo = pid_info(pid, libc::PROC_PIDTASKINFO, 0);
    info.pti_priority
}

#[must_use]
pub fn thread_priorities(pid: libc::pid_t) -> Vec<i32> {
    let handles: [u64; MAX_THREADS] = pid_info(pid, PROC_PIDLISTTHREADS, 0);
    handles
        .iter()
        .take_while(|handle| **handle != 0)
        .map(|handle| {
            let info: libc::proc_threadinfo = pid_info(pid, libc::PROC_PIDTHREADINFO, *handle);
            info.pth_priority
        })
        .collect()
}

#[expect(
    unsafe_code,
    clippy::undocumented_unsafe_blocks,
    reason = "proc_pidinfo fills a plain C struct of the size it is given; T is only ever such a struct or an array of u64"
)]
fn pid_info<T>(pid: libc::pid_t, flavor: libc::c_int, arg: u64) -> T {
    let mut info = MaybeUninit::<T>::zeroed();
    let size = libc::c_int::try_from(size_of::<T>()).expect("the buffer fits a c_int");
    let filled = unsafe { libc::proc_pidinfo(pid, flavor, arg, info.as_mut_ptr().cast(), size) };
    assert!(filled > 0, "proc_pidinfo({pid}, {flavor}) failed");
    unsafe { info.assume_init() }
}
