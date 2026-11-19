use clap::Parser;
use mw_server::{Cli, init_logging, run};

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    // Файл журнала — только у сервера; служебные команды пишут в вывод.
    let log_dir = match cli.config() {
        Ok(config) if cli.is_serve() => cli.resolve_log_dir(&config.data_dir),
        _ => None,
    };
    let (_guard, log_problem) = match init_logging(log_dir.as_deref()) {
        Ok(guard) => (guard, None),
        Err(e) => match init_logging(None) {
            Ok(guard) => (guard, Some(e)),
            Err(e) => {
                eprintln!("Ошибка: {e}");
                std::process::exit(1);
            }
        },
    };
    if let Some(problem) = log_problem {
        tracing::warn!("журнал в файл отключён: {problem}");
    }
    if let Err(e) = run(cli).await {
        eprintln!("Ошибка: {e}");
        std::process::exit(1);
    }
}
