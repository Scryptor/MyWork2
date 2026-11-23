use clap::Parser;
use mw_server::{Cli, RENDER_WORKER_ARG, init_logging, run, run_render_worker};

fn main() {
    // Режим процесса-рендерера — первым делом, до разбора аргументов, журнала и рантайма: в stdout
    // рендерер пишет только кадры протокола.
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == RENDER_WORKER_ARG)
    {
        std::process::exit(run_render_worker());
    }
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(e) => {
            eprintln!("Ошибка: {e}");
            std::process::exit(1);
        }
    };
    // Код выхода возвращает `main_inner`: охранник журнала внутри неё успевает дописать файл,
    // а `process::exit` его пропустил бы. Выход — `process::exit` и при успехе: рантайм при сбросе
    // ждал бы без конца блокирующую работу запроса, оставленного остановкой (`Server::serve`).
    let code = runtime.block_on(main_inner());
    std::process::exit(code);
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
