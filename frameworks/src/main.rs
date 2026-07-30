use clap::Parser as _;

#[cfg(not(unix))]
compile_error!("pm3 manages unix processes and unix sockets; only unix targets are supported");

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let outcome = frameworks::cli::dispatch(frameworks::cli::Cli::parse()).await;
    frameworks::cli::report(outcome)
}
