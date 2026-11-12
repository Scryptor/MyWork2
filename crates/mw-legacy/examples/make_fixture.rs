//! Собирает синтетическую папку MyWork 2.
//!
//! `cargo run -p mw-legacy --features fixtures --example make_fixture -- <папка> [v24|…|v32] [--extra-orders N]`

use std::path::PathBuf;

use mw_legacy::fixture::{FixtureOptions, FixtureVersion, write_fixture};

fn usage() -> ! {
    eprintln!(
        "Использование: make_fixture <папка> [v24|v25|v26|v27|v29|v30|v31|v32] [--extra-orders N]"
    );
    std::process::exit(2)
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mut args = std::env::args().skip(1);
    let Some(dir) = args.next() else { usage() };
    let mut version = FixtureVersion::V32;
    let mut extra_orders = 0usize;
    while let Some(arg) = args.next() {
        if arg == "--extra-orders" {
            extra_orders = args
                .next()
                .and_then(|n| n.parse().ok())
                .unwrap_or_else(|| usage());
        } else if let Some(v) = FixtureVersion::parse(&arg) {
            version = v;
        } else {
            usage();
        }
    }
    let dir = PathBuf::from(dir);
    if let Err(error) = write_fixture(
        &dir,
        &FixtureOptions {
            version,
            extra_orders,
        },
    )
    .await
    {
        eprintln!("Не удалось собрать папку: {error}");
        std::process::exit(1);
    }
    println!("Готово: {}", dir.display());
}
