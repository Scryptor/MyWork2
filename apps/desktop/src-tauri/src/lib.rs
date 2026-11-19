//! Программа MyWork для компьютера: окно Tauri над встроенным сервером.
//!
//! Сервер (`mw-server`) слушает 127.0.0.1 и раздаёт интерфейс; окно открывает его адрес, и интерфейс
//! говорит с сервером по HTTP, как в браузере. Через Tauri идут только выбор папки (`tauri-plugin-dialog`)
//! и события окна. Одна копия на папку данных (`single`), закрытие окна останавливает сервер штатно.

pub mod launch;
pub mod single;

use mw_server::LogGuard;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};
use std::thread::ThreadId;
use tauri::webview::DownloadEvent;
use tauri::{App, AppHandle, Manager, RunEvent, Url, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

/// Заголовок окна и сообщений.
const TITLE: &str = "MyWork";
/// Метка окна; её же называет `capabilities/default.json`.
const MAIN_WINDOW: &str = "main";
/// Размер окна при запуске (логические точки; положение и размер пока не запоминаются); на маленьком
/// экране — меньше.
const WINDOW_SIZE: (f64, f64) = (1280.0, 800.0);
const WINDOW_MIN_SIZE: (f64, f64) = (480.0, 400.0);

/// То, что живёт, пока работает программа; отпускается по порядку в `RunEvent::Exit`.
struct Desktop {
    running: Mutex<Option<launch::Running>>,
    lock: Mutex<Option<single::DataDirLock>>,
    log: Mutex<Option<LogGuard>>,
}

static APP: OnceLock<AppHandle> = OnceLock::new();
static MAIN_THREAD: OnceLock<ThreadId> = OnceLock::new();
/// Окно о сбое показывается один раз: повторяющийся сбой (например, в опрашиваемом запросе) не засыпает
/// человека окнами, всё остальное — в журнале.
static CRASH_SHOWN: AtomicBool = AtomicBool::new(false);

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Запускает программу; возвращается только если окно не удалось создать.
pub fn run() {
    let _ = MAIN_THREAD.set(std::thread::current().id());
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    let data_dir = launch::data_dir(&exe_dir);
    let log_dir = data_dir.join("logs");
    let log = match mw_server::init_logging(Some(&log_dir)) {
        Ok(guard) => Some(guard),
        Err(e) => {
            let guard = mw_server::init_logging(None).ok();
            tracing::warn!("журнал в файл отключён: {e}");
            guard
        }
    };
    install_crash_report(log_dir.clone());
    tracing::info!(
        "MyWork {} для компьютера, папка данных {}",
        env!("CARGO_PKG_VERSION"),
        data_dir.display()
    );
    // Замок берётся до сервера: вторая копия не открывает базу первой.
    let lock = single::acquire(&data_dir);

    let desktop = Desktop {
        running: Mutex::new(None),
        lock: Mutex::new(None),
        log: Mutex::new(log),
    };
    let built = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(desktop)
        .setup(move |app| {
            let _ = APP.set(app.handle().clone());
            let problem = match lock {
                Ok(lock) => {
                    *locked(&app.state::<Desktop>().lock) = Some(lock);
                    open(app, &data_dir).err().map(|e| e.to_string())
                }
                Err(single::LockError::Busy(dir)) => {
                    tracing::warn!("папка данных {} занята другой копией", dir.display());
                    let handle = app.handle().clone();
                    app.dialog()
                        .message(format!(
                            "Программа уже запущена с папкой данных {}.\nПереключитесь на её окно.",
                            dir.display()
                        ))
                        .title(TITLE)
                        .kind(MessageDialogKind::Info)
                        .show(move |_| handle.exit(0));
                    None
                }
                Err(e) => Some(e.to_string()),
            };
            if let Some(problem) = problem {
                tracing::error!("программа не запущена: {problem}");
                fail(
                    app.handle(),
                    format!(
                        "Не удалось запустить программу: {problem}\n\nПодробности — в журнале: {}",
                        log_dir.display()
                    ),
                );
            }
            Ok(())
        })
        .build(tauri::generate_context!());
    match built {
        Ok(app) => app.run(|app, event| {
            if let RunEvent::Exit = event {
                shut_down(app);
            }
        }),
        Err(e) => {
            tracing::error!("окно программы не создано: {e}");
            eprintln!("Ошибка: окно программы не создано: {e}");
        }
    }
}

/// Поднимает сервер и открывает окно на его адресе.
fn open(app: &App, data_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let ui_dir = app.path().resource_dir()?.join("ui");
    let running = tauri::async_runtime::block_on(launch::start(data_dir, &ui_dir))?;
    let url: Url = running.url.parse()?;
    tracing::info!("окно открывает {url}");
    *locked(&app.state::<Desktop>().running) = Some(running);

    let (width, height) = window_size(app);
    let origin = url.origin();
    let saved = Mutex::new(HashMap::<Url, PathBuf>::new());
    WebviewWindowBuilder::new(app, MAIN_WINDOW, WebviewUrl::External(url))
        .title(TITLE)
        .inner_size(width, height)
        .min_inner_size(WINDOW_MIN_SIZE.0, WINDOW_MIN_SIZE.1)
        .center()
        // Масштаб страницы выключен: размер программы меняют её собственные клавиши (Ctrl/⌘ + − 0).
        .zoom_hotkeys_enabled(false)
        // Окно показывает только свой сервер: права окна (`capabilities`) не уходят чужим страницам.
        .on_navigation(move |to| to.scheme() == "about" || to.origin() == origin)
        // «Сохранить отчёт» и файлы-вложения: WebKit без обработчика загрузку отменяет.
        .on_download(move |webview, event| {
            match event {
                DownloadEvent::Requested { url, destination } => {
                    tracing::info!("файл сохраняется в {}", destination.display());
                    locked(&saved).insert(url, destination.clone());
                }
                DownloadEvent::Finished { url, path, success } => {
                    let path = locked(&saved).remove(&url).or(path);
                    let (kind, text) = match (success, path) {
                        (true, Some(path)) => (
                            MessageDialogKind::Info,
                            format!("Файл сохранён: {}", path.display()),
                        ),
                        (true, None) => (MessageDialogKind::Info, "Файл сохранён".to_string()),
                        (false, _) => {
                            tracing::warn!("файл не сохранён: {url}");
                            (
                                MessageDialogKind::Warning,
                                "Не удалось сохранить файл".to_string(),
                            )
                        }
                    };
                    webview
                        .dialog()
                        .message(text)
                        .title(TITLE)
                        .kind(kind)
                        .show(|_| {});
                }
                _ => {}
            }
            true
        })
        .build()?;
    Ok(())
}

/// `WINDOW_SIZE`, но не больше 90 % рабочей области основного монитора.
fn window_size(app: &App) -> (f64, f64) {
    let (width, height) = WINDOW_SIZE;
    match app.primary_monitor() {
        Ok(Some(monitor)) => {
            let scale = monitor.scale_factor();
            let area = monitor.work_area().size;
            (
                width.min(f64::from(area.width) / scale * 0.9),
                height.min(f64::from(area.height) / scale * 0.9),
            )
        }
        _ => (width, height),
    }
}

/// Окно с ошибкой запуска; по нажатию программа завершается.
fn fail(app: &AppHandle, text: String) {
    let handle = app.clone();
    app.dialog()
        .message(text)
        .title(TITLE)
        .kind(MessageDialogKind::Error)
        .show(move |_| handle.exit(1));
}

/// Выход (последнее окно закрыто, ⌘Q, `exit`): сервер останавливается штатно — дожидается начатого
/// (до 30 с, запись импорта — до фиксации) и закрывает базу; затем отпускается замок и дописывается журнал.
fn shut_down(app: &AppHandle) {
    let desktop = app.state::<Desktop>();
    let running = locked(&desktop.running).take();
    if let Some(running) = running {
        tracing::info!("программа закрывается: остановка сервера");
        if let Err(e) = tauri::async_runtime::block_on(running.shutdown()) {
            tracing::error!("сервер остановлен с ошибкой: {e}");
        }
    }
    drop(locked(&desktop.lock).take());
    drop(locked(&desktop.log).take());
}

/// Сбой (паника) — в журнал и, если программа уже запущена и сбой не в главном потоке, окном с путём
/// к журналу. Главный поток, упав, окно показать не может: тогда остаётся запись в журнале.
fn install_crash_report(log_dir: PathBuf) {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!("сбой программы: {info}");
        default(info);
        let on_main = MAIN_THREAD.get() == Some(&std::thread::current().id());
        if let Some(app) = APP.get()
            && !on_main
            && !CRASH_SHOWN.swap(true, Ordering::SeqCst)
        {
            app.dialog()
                .message(format!(
                    "Внутренняя ошибка программы: {}\n\nПодробности — в журнале: {}",
                    panic_text(info),
                    log_dir.display()
                ))
                .title(TITLE)
                .kind(MessageDialogKind::Error)
                .show(|_| {});
        }
    }));
}

fn panic_text(info: &std::panic::PanicHookInfo<'_>) -> String {
    let payload = info.payload();
    payload
        .downcast_ref::<&str>()
        .map(|text| (*text).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "без описания".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use tauri::utils::acl::RemoteUrlPattern;

    /// Права окна — узкие: только адрес встроенного сервера, только выбор папки и события окна.
    #[test]
    fn the_window_rights_reach_only_the_embedded_server_and_only_folder_and_window_events() {
        let capability: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
        assert_eq!(capability["windows"], serde_json::json!([MAIN_WINDOW]));
        assert_eq!(
            capability["permissions"],
            serde_json::json!([
                "core:event:allow-listen",
                "core:event:allow-unlisten",
                "dialog:allow-open"
            ])
        );
        let urls = capability["remote"]["urls"].as_array().unwrap();
        let patterns: Vec<RemoteUrlPattern> = urls
            .iter()
            .map(|url| RemoteUrlPattern::from_str(url.as_str().unwrap()).unwrap())
            .collect();
        let allowed = |url: &str| {
            let url = Url::parse(url).unwrap();
            patterns.iter().any(|pattern| pattern.test(&url))
        };
        assert!(allowed("http://127.0.0.1:50946/"));
        assert!(allowed("http://127.0.0.1:8080/setup?from=orders"));
        assert!(!allowed("http://localhost:50946/"));
        assert!(!allowed("http://127.0.0.2:50946/"));
        assert!(!allowed("https://127.0.0.1:50946/"));
        assert!(!allowed("http://example.com/"));
    }
}
