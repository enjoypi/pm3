use rustix::process::{Resource, Rlimit, getrlimit, setrlimit};

pub fn lift_open_file_limit() {
    let Rlimit { maximum, .. } = getrlimit(Resource::Nofile);
    let lifted = Rlimit {
        current: maximum,
        maximum,
    };
    let outcome = setrlimit(Resource::Nofile, lifted);
    tracing::info!(soft = ?maximum, ?outcome, "lifted the open file limit to its hard cap");
}

#[cfg(test)]
#[path = "tests/fd_limit_tests.rs"]
mod tests;
