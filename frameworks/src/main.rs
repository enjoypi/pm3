use clap::Parser as _;

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let outcome = frameworks::cli::dispatch(frameworks::cli::Cli::parse()).await;
    frameworks::cli::report(outcome)
}
