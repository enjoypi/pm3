pub mod backend;
pub mod bwrap;
pub mod seatbelt;

mod wrapper;

pub use self::{
    backend::{HostSandbox, SandboxBackend, SandboxProgramSet},
    bwrap::bwrap_argv,
    seatbelt::{seatbelt_argv, seatbelt_profile},
    wrapper::SandboxCommandWrapper,
};

const ARGUMENT_TERMINATOR: &str = "--";

fn finish_wrapped(
    sandbox_program: &str,
    mut sandbox_args: Vec<String>,
    program: &str,
    args: &[String],
) -> usecases::WrappedCommand {
    sandbox_args.push(ARGUMENT_TERMINATOR.to_string());
    sandbox_args.push(program.to_string());
    sandbox_args.extend_from_slice(args);
    usecases::WrappedCommand {
        program: sandbox_program.to_string(),
        args: sandbox_args,
    }
}
