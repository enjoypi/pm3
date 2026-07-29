use clap::Parser;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    frameworks::cli::dispatch(frameworks::cli::Cli::parse()).await
}
