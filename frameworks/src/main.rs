use clap::Parser as _;

fn main() -> std::process::ExitCode {
    let cli = frameworks::cli::Cli::parse();
    let runtime = frameworks::runtime_for(&cli.command).expect("cannot build the tokio runtime");
    runtime.block_on(async {
        let outcome = frameworks::cli::dispatch(cli).await;
        frameworks::cli::report(outcome)
    })
}
