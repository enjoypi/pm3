use std::{
    ffi::{CString, OsStr, OsString, c_char, c_int, c_short},
    io,
    os::unix::ffi::OsStrExt as _,
    path::Path,
    ptr,
};

use adapters::LaunchSpec;

pub const SUBCOMMAND: &str = "__disclaim";
const SEPARATOR: &str = "--";
const DISCLAIM: c_int = 1;

#[expect(
    unsafe_code,
    reason = "the disclaim attribute is private libSystem API with no binding"
)]
unsafe extern "C" {
    fn responsibility_spawnattrs_setdisclaim(
        attr: *mut libc::posix_spawnattr_t,
        disclaim: c_int,
    ) -> c_int;
}

#[must_use]
pub fn through_trampoline(executable: &Path, spec: &LaunchSpec) -> LaunchSpec {
    let head = [SUBCOMMAND, SEPARATOR, spec.program.as_str()].map(String::from);
    LaunchSpec {
        program: executable.to_string_lossy().into_owned(),
        args: head.into_iter().chain(spec.args.iter().cloned()).collect(),
        ..spec.clone()
    }
}

#[expect(
    unsafe_code,
    clippy::undocumented_unsafe_blocks,
    reason = "posix_spawn with the private disclaim attribute has no safe wrapper; attr is initialised before use, every pointer borrows a CString that outlives the call, the list ends with a null, and environ is the live process environment"
)]
#[expect(
    clippy::cast_possible_truncation,
    reason = "POSIX_SPAWN_SETEXEC is 0x40 and posix_spawnattr_setflags takes a short"
)]
pub fn exec_disclaimed(program: &OsStr, args: &[OsString]) -> io::Error {
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
        responsibility_spawnattrs_setdisclaim(&raw mut attr, DISCLAIM);
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
