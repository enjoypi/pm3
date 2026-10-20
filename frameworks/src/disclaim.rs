use std::{
    ffi::{CString, OsStr, OsString, c_char, c_int, c_short},
    io,
    os::unix::ffi::OsStrExt as _,
    path::Path,
    ptr,
};

use adapters::{LaunchError, LaunchSpec, Priority, resolve_executable};

pub const SUBCOMMAND: &str = "__disclaim";
pub const DISCLAIM: c_int = 1;
pub const INHERIT: c_int = 0;
pub const NO_CLAMP: u64 = 0;
const KEEP_RESPONSIBLE: c_int = 0;
const SEPARATOR: &str = "--";
const DISCLAIM_FLAG: &str = "--disclaim";
const PROCESS_TYPE_FLAG: &str = "--process-type";
const QOS_CLAMP_FLAG: &str = "--qos-clamp";
const DAEMON_STANDARD: c_int = 0x300;
const DAEMON_INTERACTIVE: c_int = 0x400;
const DAEMON_BACKGROUND: c_int = 0x500;
const CLAMP_UTILITY: u64 = 1;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SpawnTraits {
    pub disclaim: c_int,
    pub process_type: c_int,
    pub qos_clamp: u64,
}

impl SpawnTraits {
    #[must_use]
    pub const fn of(self_responsible: bool, priority: Priority) -> Self {
        let disclaim = if self_responsible {
            DISCLAIM
        } else {
            KEEP_RESPONSIBLE
        };
        let (process_type, qos_clamp) = match priority {
            Priority::Background => (DAEMON_BACKGROUND, NO_CLAMP),
            Priority::Standard => (DAEMON_STANDARD, CLAMP_UTILITY),
            Priority::Interactive => (DAEMON_INTERACTIVE, NO_CLAMP),
        };
        Self {
            disclaim,
            process_type,
            qos_clamp,
        }
    }
}

#[expect(
    unsafe_code,
    reason = "the disclaim, process type and qos clamp attributes are private libSystem API with no binding"
)]
unsafe extern "C" {
    fn responsibility_spawnattrs_setdisclaim(
        attr: *mut libc::posix_spawnattr_t,
        disclaim: c_int,
    ) -> c_int;
    fn posix_spawnattr_setprocesstype_np(
        attr: *mut libc::posix_spawnattr_t,
        process_type: c_int,
    ) -> c_int;
    fn posix_spawnattr_set_qos_clamp_np(attr: *mut libc::posix_spawnattr_t, clamp: u64) -> c_int;
}

pub fn wrap(executable: &Path, spec: &LaunchSpec) -> Result<LaunchSpec, LaunchError> {
    if resolve_executable(&spec.program, None).is_none() {
        return Err(LaunchError::Spawn {
            app: spec.name.clone(),
            reason: format!("'{}' is not an executable file", spec.program),
        });
    }
    let traits = SpawnTraits::of(spec.self_responsible, spec.priority);
    let head = [
        SUBCOMMAND.to_string(),
        DISCLAIM_FLAG.to_string(),
        traits.disclaim.to_string(),
        PROCESS_TYPE_FLAG.to_string(),
        traits.process_type.to_string(),
        QOS_CLAMP_FLAG.to_string(),
        traits.qos_clamp.to_string(),
        SEPARATOR.to_string(),
        spec.program.clone(),
    ];
    Ok(LaunchSpec {
        program: executable.to_string_lossy().into_owned(),
        args: head.into_iter().chain(spec.args.iter().cloned()).collect(),
        ..spec.clone()
    })
}

#[expect(
    unsafe_code,
    clippy::undocumented_unsafe_blocks,
    reason = "posix_spawn with private spawn attributes has no safe wrapper; attr is initialised before use, every pointer borrows a CString that outlives the call, the list ends with a null, and environ is the live process environment"
)]
#[expect(
    clippy::cast_possible_truncation,
    reason = "POSIX_SPAWN_SETEXEC is 0x40 and posix_spawnattr_setflags takes a short"
)]
pub fn exec_disclaimed(program: &OsStr, args: &[OsString], traits: SpawnTraits) -> io::Error {
    let path = c_string(program);
    let owned: Vec<CString> = args.iter().map(|arg| c_string(arg)).collect();
    let mut pointers: Vec<*mut c_char> = std::iter::once(&path)
        .chain(&owned)
        .map(|arg| arg.as_ptr().cast_mut())
        .collect();
    pointers.push(ptr::null_mut());
    let mut attr: libc::posix_spawnattr_t = ptr::null_mut();
    let mut pid: libc::pid_t = 0;
    let code = unsafe {
        libc::posix_spawnattr_init(&raw mut attr);
        libc::posix_spawnattr_setflags(&raw mut attr, libc::POSIX_SPAWN_SETEXEC as c_short);
        responsibility_spawnattrs_setdisclaim(&raw mut attr, traits.disclaim);
        posix_spawnattr_setprocesstype_np(&raw mut attr, traits.process_type);
        posix_spawnattr_set_qos_clamp_np(&raw mut attr, traits.qos_clamp);
        libc::posix_spawn(
            &raw mut pid,
            path.as_ptr(),
            ptr::null(),
            &raw const attr,
            pointers.as_ptr(),
            *libc::_NSGetEnviron(),
        )
    };
    io::Error::from_raw_os_error(code)
}

fn c_string(text: &OsStr) -> CString {
    CString::new(text.as_bytes()).unwrap_or_default()
}

#[cfg(test)]
#[path = "tests/disclaim_tests.rs"]
mod tests;
