use clap::Parser;
use mw_server::{Cli, init_logging, run};

#[tokio::main]
async fn main() {
    // Код выхода возвращает `main_inner`: охранник журнала внутри неё успевает дописать файл,
    // а `process::exit` его пропустил бы.
    let code = main_inner().await;
    if code != 0 {
        std::process::exit(code);
    }
}

async fn main_inner() -> i32 {
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
                return 1;
            }
        },
    };
    if let Some(problem) = log_problem {
        tracing::warn!("журнал в файл отключён: {problem}");
    }
    match run(cli).await {
        Ok(()) => 0,
        Err(e) => {
            tracing::error!("{e}");
            eprintln!("Ошибка: {e}");
            1
        }
    }
}
