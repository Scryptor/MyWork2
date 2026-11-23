// Без консольного окна на Windows в сборке для людей.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Режим процесса-рендерера — до Tauri и журнала: в stdout рендерер пишет только кадры протокола.
    if std::env::args_os()
        .nth(1)
        .is_some_and(|arg| arg == mw_server::RENDER_WORKER_ARG)
    {
        std::process::exit(mw_server::run_render_worker());
    }
    mw_desktop::run();
}
