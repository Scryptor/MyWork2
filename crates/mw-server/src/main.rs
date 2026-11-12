use clap::Parser;
use mw_server::{Cli, run};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_env("MW_LOG").unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn")),
        )
        .init();
    if let Err(e) = run(Cli::parse()).await {
        eprintln!("Ошибка: {e}");
        std::process::exit(1);
    }
}
