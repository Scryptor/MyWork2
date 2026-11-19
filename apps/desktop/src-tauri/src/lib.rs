//! Программа MyWork для компьютера: окно Tauri над встроенным сервером.
//!
//! Сервер (`mw-server`) слушает 127.0.0.1 и раздаёт интерфейс; окно открывает его адрес, и интерфейс
//! говорит с сервером по HTTP, как в браузере. Через Tauri идут только выбор папки (`tauri-plugin-dialog`)
//! и события окна. Одна копия на папку данных (`mw_server::lock`), закрытие окна останавливает сервер штатно.

pub mod launch;
pub mod navigation;

use mw_server::LogGuard;
use mw_server::lock::{self, DataDirLock, LockError};
use navigation::Navigation;
use std::collections::HashMap;
use std::io::Write;
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
/// Файл сбоев в папке журнала: пишется сразу, без очереди журнала, — успевает и перед аварийным выходом.
const CRASH_FILE: &str = "crash.txt";
/// Предел `crash.txt`: сверх него старые записи отбрасываются, остаются новые (повторяющийся сбой не
/// заполняет диск).
const CRASH_LIMIT: usize = 1024 * 1024;
/// Начало записи о сбое в `crash.txt`: по нему файл обрезается целыми записями.
const CRASH_MARK: &str = "=== ";

/// То, что живёт, пока работает программа; отпускается по порядку в `RunEvent::Exit`.
struct Desktop {
    running: Mutex<Option<launch::Running>>,
    lock: Mutex<Option<DataDirLock>>,
    log: Mutex<Option<LogGuard>>,
}

/// С чем программа входит в цикл окон.
enum Start {
    /// Папка данных своя: поднять сервер и открыть окно.
    Ready { data_dir: PathBuf, log_dir: PathBuf },
    /// Папку данных держит другая копия.
    Busy(PathBuf),
    /// Запуск невозможен: текст для человека и папка журнала, если она есть.
    Failed(String, Option<PathBuf>),
}

static APP: OnceLock<AppHandle> = OnceLock::new();
static MAIN_THREAD: OnceLock<ThreadId> = OnceLock::new();
/// Окно о сбое показывается один раз: повторяющийся сбой (например, в опрашиваемом запросе) не засыпает
/// человека окнами, всё остальное — в журнале и в `crash.txt`.
static CRASH_SHOWN: AtomicBool = AtomicBool::new(false);

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Запускает программу; возвращается только если окно не удалось создать.
pub fn run() {
    let _ = MAIN_THREAD.set(std::thread::current().id());
    let data_dir = std::env::current_exe()
        .map_err(|e| format!("не удалось узнать, где лежит программа: {e}"))
        .and_then(|exe| {
            let exe_dir = exe.parent().map(Path::to_path_buf).unwrap_or_default();
            launch::data_dir(&exe_dir).map_err(|e| e.to_string())
        });
    // Замок — до журнала в файл и до сервера: вторая копия не пишет в общий журнал и не открывает базу.
    let (start, lock) = match data_dir {
        Ok(data_dir) => match lock::acquire(&data_dir) {
            Ok(lock) => (
                Start::Ready {
                    log_dir: data_dir.join("logs"),
                    data_dir,
                },
                Some(lock),
            ),
            Err(LockError::Busy { dir, .. }) => (Start::Busy(dir), None),
            Err(e) => (Start::Failed(e.to_string(), None), None),
        },
        Err(e) => (Start::Failed(e, None), None),
    };
    let log_dir = match &start {
        Start::Ready { log_dir, .. } => Some(log_dir.clone()),
        _ => None,
    };
    let log = match mw_server::init_logging(log_dir.as_deref()) {
        Ok(guard) => Some(guard),
        Err(e) => {
            let guard = mw_server::init_logging(None).ok();
            tracing::warn!("журнал в файл отключён: {e}");
            guard
        }
    };
    install_crash_report(log_dir.map(|dir| dir.join(CRASH_FILE)));
    tracing::info!("MyWork {} для компьютера", env!("CARGO_PKG_VERSION"));

    let desktop = Desktop {
        running: Mutex::new(None),
        lock: Mutex::new(lock),
        log: Mutex::new(log),
    };
    let built = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(desktop)
        .setup(move |app| {
            let _ = APP.set(app.handle().clone());
            match start {
                Start::Ready { data_dir, log_dir } => {
                    tracing::info!("папка данных {}", data_dir.display());
                    if let Err(e) = open(app, &data_dir) {
                        startup_failed(app.handle(), &e.to_string(), Some(&log_dir));
                    }
                }
                Start::Busy(dir) => {
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
                }
                Start::Failed(problem, log_dir) => {
                    startup_failed(app.handle(), &problem, log_dir.as_deref());
                }
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
    let own = origin.clone();
    // Куда сохраняется каждая принятая загрузка: macOS в конце пути не сообщает.
    let saved = Mutex::new(HashMap::<Url, PathBuf>::new());
    WebviewWindowBuilder::new(app, MAIN_WINDOW, WebviewUrl::External(url))
        .title(TITLE)
        .inner_size(width, height)
        .min_inner_size(WINDOW_MIN_SIZE.0, WINDOW_MIN_SIZE.1)
        .center()
        // Масштаб страницы выключен: размер программы меняют её собственные клавиши (Ctrl/⌘ + − 0).
        .zoom_hotkeys_enabled(false)
        .on_navigation(move |to| match navigation::navigation(to, &origin) {
            Navigation::Allow => true,
            Navigation::System => {
                open_in_system(to);
                false
            }
            Navigation::Block => {
                tracing::warn!("переход не открыт: схема {}", to.scheme());
                false
            }
        })
        // «Сохранить отчёт» и файлы-вложения: WebKit без обработчика загрузку отменяет.
        .on_download(move |webview, event| match event {
            DownloadEvent::Requested { url, destination } => {
                if !navigation::download_allowed(&url, &own) {
                    tracing::warn!("загрузка не со своего сервера отклонена");
                    return false;
                }
                // Имя файла может назвать клиента: в журнал не пишется.
                tracing::info!("файл сохраняется в папку загрузок");
                locked(&saved).insert(url, destination.clone());
                true
            }
            DownloadEvent::Finished { url, path, success } => {
                // Только загрузки, принятые выше.
                let Some(destination) = locked(&saved).remove(&url) else {
                    return true;
                };
                let path = path.unwrap_or(destination);
                let (kind, text) = if success {
                    (
                        MessageDialogKind::Info,
                        format!("Файл сохранён: {}", path.display()),
                    )
                } else {
                    tracing::warn!("файл не сохранён");
                    (
                        MessageDialogKind::Warning,
                        "Не удалось сохранить файл".to_string(),
                    )
                };
                webview
                    .dialog()
                    .message(text)
                    .title(TITLE)
                    .kind(kind)
                    .show(|_| {});
                true
            }
            _ => true,
        })
        .build()?;
    Ok(())
}

/// Звонок, письмо, внешний сайт — программой системы. Адрес может назвать клиента (номер, почта): в журнал
/// идёт только схема.
fn open_in_system(url: &Url) {
    if let Err(e) = open::that_detached(url.as_str()) {
        tracing::warn!("система не открыла адрес со схемой {}: {e}", url.scheme());
    }
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

/// Окно с ошибкой запуска (и путём к журналу, если он есть); по нажатию программа завершается.
fn startup_failed(app: &AppHandle, problem: &str, log_dir: Option<&Path>) {
    tracing::error!("программа не запущена: {problem}");
    let text = match log_dir {
        Some(dir) => format!(
            "Не удалось запустить программу: {problem}\n\nПодробности — в журнале: {}",
            dir.display()
        ),
        None => format!("Не удалось запустить программу: {problem}"),
    };
    let handle = app.clone();
    app.dialog()
        .message(text)
        .title(TITLE)
        .kind(MessageDialogKind::Error)
        .show(move |_| handle.exit(1));
}

/// Выход (последнее окно закрыто, ⌘Q, `exit`): окно прячется, чтобы не висеть замершим, сервер
/// останавливается штатно — дожидается начатого (до 30 с, запись импорта — до фиксации) и закрывает базу;
/// затем отпускается замок и дописывается журнал.
fn shut_down(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW)
        && let Err(e) = window.hide()
    {
        tracing::warn!("окно не спрятано: {e}");
    }
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

/// Сбой (паника): сразу в `crash.txt` (без очереди журнала — успевает и перед аварийным выходом), в журнал
/// и, если сбой не в главном потоке, одним окном с путём к файлу. Главный поток, упав, окно показать не может.
fn install_crash_report(crash_file: Option<PathBuf>) {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Some(path) = &crash_file {
            write_crash(path, info);
        }
        tracing::error!("сбой программы: {info}");
        default(info);
        let on_main = MAIN_THREAD.get() == Some(&std::thread::current().id());
        if let Some(app) = APP.get()
            && !on_main
            && !CRASH_SHOWN.swap(true, Ordering::SeqCst)
        {
            let details = match &crash_file {
                Some(path) => format!("Подробности — в файле {}", path.display()),
                None => "Подробности — в выводе программы".to_string(),
            };
            app.dialog()
                .message(format!(
                    "Внутренняя ошибка программы: {}\n\n{details}",
                    panic_text(info)
                ))
                .title(TITLE)
                .kind(MessageDialogKind::Error)
                .show(|_| {});
        }
    }));
}

/// Дописывает сбой в файл: время (UTC, как в журнале), место и текст, стек вызовов.
fn write_crash(path: &Path, info: &std::panic::PanicHookInfo<'_>) {
    let when = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let backtrace = std::backtrace::Backtrace::force_capture();
    let entry = format!("{CRASH_MARK}{when} сбой программы: {info}\n{backtrace}\n\n");
    let _ = append_crash(path, &entry, CRASH_LIMIT);
}

/// Дописывает запись в файл сбоев не больше `limit` байт: если не помещается, отбрасываются самые старые
/// записи целиком (запись больше предела остаётся одна).
fn append_crash(path: &Path, entry: &str, limit: usize) -> std::io::Result<()> {
    let old = match std::fs::read(path) {
        Ok(old) => old,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e),
    };
    if old.len() + entry.len() <= limit {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        return file.write_all(entry.as_bytes());
    }
    // Остаются записи, что помещаются вместе с новой: с первого начала записи в последних `room` байтах.
    let room = limit.saturating_sub(entry.len());
    let from = old.len() - room.min(old.len());
    let starts_entry = |at: usize| {
        old[at..].starts_with(CRASH_MARK.as_bytes()) && (at == 0 || old[at - 1] == b'\n')
    };
    let kept = (from..old.len())
        .find(|&at| starts_entry(at))
        .map_or(&[][..], |at| &old[at..]);
    let mut text = Vec::with_capacity(kept.len() + entry.len());
    text.extend_from_slice(kept);
    text.extend_from_slice(entry.as_bytes());
    // Целиком или никак: временный файл рядом и замена переименованием.
    let temporary = path.with_extension("txt.tmp");
    std::fs::write(&temporary, &text).and_then(|()| std::fs::rename(&temporary, path))
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

    #[test]
    fn the_crash_file_keeps_the_newest_whole_entries_within_its_limit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CRASH_FILE);
        let entry = |n: usize| format!("{CRASH_MARK}сбой {n:02}\nстек\n\n");
        let limit = 5 * entry(0).len();
        for n in 0..5 {
            append_crash(&path, &entry(n), limit).unwrap();
        }
        let all: String = (0..5).map(entry).collect();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), all);
        for n in 5..12 {
            append_crash(&path, &entry(n), limit).unwrap();
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(text.len() <= limit, "{n}: {}", text.len());
            assert!(text.starts_with(CRASH_MARK), "{n}: {text}");
            assert!(text.ends_with(&entry(n)), "{n}: {text}");
        }
        let newest: String = (7..12).map(entry).collect();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), newest);
        // Запись больше предела остаётся одна; временного файла нет.
        let huge = format!("{CRASH_MARK}{}\n\n", "x".repeat(limit));
        append_crash(&path, &huge, limit).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), huge);
        append_crash(&path, &entry(12), limit).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), entry(12));
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from(CRASH_FILE)]);
    }

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
