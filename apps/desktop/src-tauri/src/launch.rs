//! Встроенный сервер программы для компьютера: папка данных, адрес на 127.0.0.1, штатная остановка.

use mw_server::{Config, Server, ServerError, data_dir_name, os_data_dir, portable_data_dir};
use std::net::{Ipv4Addr, SocketAddr};
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

/// Файл настроек этого компьютера в папке данных (спецификация, «Где лежат данные»).
pub const LOCAL_FILE: &str = "local.json";

/// Файл рядом с программой, переводящий её в портативный режим (данные — в `data` рядом).
pub use mw_server::PORTABLE_FILE;

/// Порт первого запуска — случайный отсюда, вне эфемерного диапазона ОС (49152–65535 на Windows и macOS,
/// 32768–60999 на Linux): там порт берут исходящие соединения других программ, и запомненный адрес окна
/// чаще оказывался бы занят.
const FIRST_PORTS: RangeInclusive<u16> = 20000..=29999;
/// Сколько случайных портов пробуется, прежде чем взять любой свободный (`:0`).
const RANDOM_TRIES: usize = 8;

/// Папка данных программы этой сборки: см. `data_dir_for`.
pub fn data_dir(exe_dir: &Path) -> Result<PathBuf, ServerError> {
    data_dir_for(exe_dir, os_data_dir(), cfg!(debug_assertions))
}

/// Папка данных: `data` рядом с программой при `portable.txt` (кроме пакета macOS, см.
/// `mw_server::portable_data_dir`); иначе в папке данных ОС — `mw_server::data_dir_name(dev)`: `MyWork3`, а у
/// отладочной сборки (`dev`) — `MyWork3-dev`. Без папки ОС (нет `HOME` / `APPDATA`) — ошибка: молча писать
/// рядом с программой или в текущую папку нельзя.
pub fn data_dir_for(
    exe_dir: &Path,
    os_dir: Option<PathBuf>,
    dev: bool,
) -> Result<PathBuf, ServerError> {
    if let Some(portable) = portable_data_dir(exe_dir) {
        return Ok(portable);
    }
    os_dir.map(|dir| dir.join(data_dir_name(dev))).ok_or_else(|| {
        ServerError::Config(format!(
            "не удалось определить папку данных: система не назвала свою папку (не заданы HOME / APPDATA). \
             Чтобы хранить данные рядом с программой, положите рядом с ней файл {PORTABLE_FILE}"
        ))
    })
}

/// Работающий встроенный сервер.
pub struct Running {
    /// Адрес для окна: `http://127.0.0.1:<порт>/`.
    pub url: String,
    stop: oneshot::Sender<()>,
    serving: JoinHandle<Result<(), ServerError>>,
}

impl Running {
    /// Штатная остановка: новые запросы не принимаются, начатое (перенос файлов, запись импорта)
    /// доходит до конца или отменяется (`Server::serve` ждёт до 30 с), база закрывается.
    pub async fn shutdown(self) -> Result<(), ServerError> {
        // Сервер, уже завершившийся сам, сигнал не ждёт: ошибку отправки не к чему применить.
        let _ = self.stop.send(());
        self.serving.await.unwrap_or_else(|e| {
            Err(ServerError::Config(format!(
                "сервер завершился аварийно: {e}"
            )))
        })
    }
}

/// Поднимает сервер с SQLite в папке данных и интерфейсом из `ui_dir` на 127.0.0.1. Порт — прежний
/// (из `local.json`), если он свободен, иначе случайный из `FIRST_PORTS`, иначе любой свободный: у окна тот же
/// адрес, а с ним и источник, к которому браузерное хранилище привязывает настройки вида, сохранённые виды и вход.
pub async fn start(data_dir: &Path, ui_dir: &Path) -> Result<Running, ServerError> {
    let remembered = remembered_port(data_dir);
    let port = choose_port(remembered, random_number, port_is_free);
    let server = match Server::start(&config(data_dir, ui_dir, port)).await {
        // Порт заняли между проверкой и привязкой.
        Err(ServerError::Io { source, .. }) if port != 0 && port_taken(source.kind()) => {
            tracing::info!("порт {port} занят, сервер возьмёт любой свободный");
            Server::start(&config(data_dir, ui_dir, 0)).await?
        }
        started => started?,
    };
    let addr = server.local_addr();
    if Some(addr.port()) != remembered {
        remember_port(data_dir, addr.port());
    }
    let (stop, stopped) = oneshot::channel::<()>();
    let serving = tokio::spawn(server.serve(async {
        let _ = stopped.await;
    }));
    Ok(Running {
        url: format!("http://{addr}/"),
        stop,
        serving,
    })
}

/// Порт для сервера: прежний, если свободен; иначе первый свободный из `RANDOM_TRIES` случайных в
/// `FIRST_PORTS`; иначе `0` (любой свободный, его назовёт ОС).
fn choose_port(
    remembered: Option<u16>,
    mut random: impl FnMut() -> u64,
    mut free: impl FnMut(u16) -> bool,
) -> u16 {
    if let Some(port) = remembered
        && free(port)
    {
        return port;
    }
    if remembered.is_some() {
        tracing::info!("прежний порт занят, сервер возьмёт другой");
    }
    let span = u64::from(FIRST_PORTS.end() - FIRST_PORTS.start()) + 1;
    (0..RANDOM_TRIES)
        .filter_map(|_| u16::try_from(random() % span).ok())
        .map(|offset| FIRST_PORTS.start() + offset)
        .find(|&port| free(port))
        .unwrap_or(0)
}

/// Свободен ли порт на 127.0.0.1: пробная привязка тут же отпускается.
fn port_is_free(port: u16) -> bool {
    std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port)).is_ok()
}

/// Случайное число без отдельной библиотеки: у каждого `RandomState` свои случайные ключи.
fn random_number() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    std::collections::hash_map::RandomState::new()
        .build_hasher()
        .finish()
}

fn config(data_dir: &Path, ui_dir: &Path, port: u16) -> Config {
    Config {
        listen: SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
        data_dir: data_dir.to_path_buf(),
        database_url: None,
        timezone: None,
        ui_dir: Some(ui_dir.to_path_buf()),
    }
}

/// Порт занят другой программой (`AddrInUse`) или закрыт для программ системой (Windows резервирует
/// диапазоны портов, там привязка даёт `PermissionDenied`).
fn port_taken(kind: std::io::ErrorKind) -> bool {
    matches!(
        kind,
        std::io::ErrorKind::AddrInUse | std::io::ErrorKind::PermissionDenied
    )
}

fn read_local(data_dir: &Path) -> Option<serde_json::Map<String, serde_json::Value>> {
    let text = std::fs::read_to_string(data_dir.join(LOCAL_FILE)).ok()?;
    match serde_json::from_str(&text) {
        Ok(serde_json::Value::Object(map)) => Some(map),
        _ => None,
    }
}

fn remembered_port(data_dir: &Path) -> Option<u16> {
    let port = read_local(data_dir)?.get("port")?.as_u64()?;
    u16::try_from(port).ok().filter(|&port| port != 0)
}

/// Запоминает порт в `local.json`, сохраняя прочие настройки этого компьютера. Неразборчивый файл
/// не перезаписывается: в нём могут быть настройки человека. Ошибка записи — только в журнал.
fn remember_port(data_dir: &Path, port: u16) {
    let path = data_dir.join(LOCAL_FILE);
    let mut local = if path.exists() {
        match read_local(data_dir) {
            Some(local) => local,
            None => {
                tracing::warn!("{} не разобран, порт не запомнен", path.display());
                return;
            }
        }
    } else {
        serde_json::Map::new()
    };
    local.insert("port".into(), port.into());
    let text = serde_json::Value::Object(local).to_string();
    // Целиком или никак: временный файл рядом и замена переименованием.
    let temporary = data_dir.join(format!("{LOCAL_FILE}.tmp"));
    let written =
        std::fs::write(&temporary, text).and_then(|()| std::fs::rename(&temporary, &path));
    if let Err(e) = written {
        let _ = std::fs::remove_file(&temporary);
        tracing::warn!("не удалось запомнить порт в {}: {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;

    fn addr_of(url: &str) -> std::net::SocketAddr {
        url.trim_start_matches("http://")
            .trim_end_matches('/')
            .parse()
            .unwrap()
    }

    async fn get(url: &str, path: &str) -> (u16, String) {
        let mut stream = TcpStream::connect(addr_of(url)).await.unwrap();
        let request =
            format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).await.unwrap();
        let text = String::from_utf8_lossy(&raw).to_string();
        let status = text.split(' ').nth(1).unwrap().parse().unwrap();
        let body = text.split_once("\r\n\r\n").map(|x| x.1).unwrap_or("");
        (status, body.to_string())
    }

    fn ui_dir(root: &Path) -> PathBuf {
        let ui = root.join("ui");
        std::fs::create_dir_all(&ui).unwrap();
        std::fs::write(
            ui.join("index.html"),
            "<!doctype html><title>MyWork</title>",
        )
        .unwrap();
        ui
    }

    #[test]
    fn portable_txt_next_to_the_program_keeps_the_data_beside_it_in_any_build() {
        let exe = tempfile::tempdir().unwrap();
        std::fs::write(exe.path().join("portable.txt"), "").unwrap();
        for dev in [false, true] {
            assert_eq!(
                data_dir_for(exe.path(), Some("/os".into()), dev).unwrap(),
                exe.path().join("data")
            );
            assert_eq!(
                data_dir_for(exe.path(), None, dev).unwrap(),
                exe.path().join("data")
            );
        }
    }

    #[test]
    fn a_debug_build_has_its_own_folder_so_unreleased_migrations_never_reach_the_installed_program()
    {
        let exe = tempfile::tempdir().unwrap();
        assert_eq!(
            data_dir_for(exe.path(), Some("/os".into()), false).unwrap(),
            PathBuf::from("/os").join("MyWork3")
        );
        assert_eq!(
            data_dir_for(exe.path(), Some("/os".into()), true).unwrap(),
            PathBuf::from("/os").join("MyWork3-dev")
        );
        // Эта сборка тестов — отладочная (или выпускная при `--release`): `data_dir` следует ей.
        let name = if cfg!(debug_assertions) {
            "MyWork3-dev"
        } else {
            "MyWork3"
        };
        assert_eq!(
            data_dir(exe.path()).unwrap(),
            mw_server::os_data_dir().unwrap().join(name)
        );
    }

    #[test]
    fn portable_txt_inside_a_macos_bundle_is_ignored_so_the_data_survives_replacing_the_program() {
        let root = tempfile::tempdir().unwrap();
        let exe = root
            .path()
            .join("MyWork.app")
            .join("Contents")
            .join("MacOS");
        std::fs::create_dir_all(&exe).unwrap();
        std::fs::write(exe.join(PORTABLE_FILE), "").unwrap();
        assert_eq!(
            data_dir_for(&exe, Some("/os".into()), false).unwrap(),
            PathBuf::from("/os").join("MyWork3")
        );
        assert!(data_dir_for(&exe, None, false).is_err());
    }

    /// Случайные числа по списку (по кругу).
    fn numbers(list: &'static [u64]) -> impl FnMut() -> u64 {
        let mut next = list.iter().cycle();
        move || *next.next().unwrap()
    }

    #[test]
    fn the_first_port_is_random_outside_the_ephemeral_range() {
        let mut tried = Vec::new();
        let port = choose_port(None, numbers(&[7, 10_003]), |port| {
            tried.push(port);
            true
        });
        assert_eq!(port, 20_007);
        assert_eq!(tried, vec![20_007]);
        // Числа любого размера дают порт из диапазона.
        for n in [0, 9_999, 10_000, u64::MAX] {
            let port = choose_port(None, move || n, |_| true);
            assert!(FIRST_PORTS.contains(&port), "{n}: {port}");
        }
    }

    #[test]
    fn a_free_remembered_port_is_kept_and_a_taken_one_gives_a_random_one() {
        let port = choose_port(Some(50_946), numbers(&[1]), |_| true);
        assert_eq!(port, 50_946);
        let mut tried = Vec::new();
        let port = choose_port(Some(50_946), numbers(&[1, 2, 3]), |port| {
            tried.push(port);
            port == 20_003
        });
        assert_eq!(port, 20_003);
        assert_eq!(tried, vec![50_946, 20_001, 20_002, 20_003]);
    }

    #[test]
    fn when_every_random_port_is_taken_the_system_picks_one() {
        let mut tried = 0;
        let port = choose_port(Some(50_946), numbers(&[5]), |_| {
            tried += 1;
            false
        });
        assert_eq!(port, 0);
        assert_eq!(tried, 1 + RANDOM_TRIES);
    }

    #[test]
    fn without_a_folder_of_the_system_the_start_fails_clearly_instead_of_writing_beside_the_program()
     {
        let exe = tempfile::tempdir().unwrap();
        let error = data_dir_for(exe.path(), None, false)
            .unwrap_err()
            .to_string();
        assert!(error.contains("папку данных"), "{error}");
        assert!(error.contains("portable.txt"), "{error}");
        assert!(!exe.path().join("data").exists());
    }

    #[test]
    fn local_settings_are_replaced_whole_through_a_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(LOCAL_FILE), r#"{"printer": "Принтер"}"#).unwrap();
        remember_port(dir.path(), 50946);
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(
            names,
            vec![std::ffi::OsString::from(LOCAL_FILE)],
            "временный файл убран"
        );
        let local: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.path().join(LOCAL_FILE)).unwrap())
                .unwrap();
        assert_eq!(
            local,
            serde_json::json!({"port": 50946, "printer": "Принтер"})
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn serves_the_api_and_the_interface_and_stops_closing_the_database() {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("данные");
        let ui = ui_dir(root.path());
        let running = start(&data, &ui).await.unwrap();
        assert!(
            running.url.starts_with("http://127.0.0.1:"),
            "{}",
            running.url
        );
        assert!(running.url.ends_with('/'), "{}", running.url);
        let port = addr_of(&running.url).port();
        assert!(
            FIRST_PORTS.contains(&port),
            "первый порт — из {FIRST_PORTS:?}: {port}"
        );
        assert!(data.join("mywork.db").is_file());

        let (status, body) = get(&running.url, "/api/v1/server-info").await;
        assert_eq!(status, 200, "{body}");
        assert!(body.contains("\"auth_required\":false"), "{body}");
        let (status, body) = get(&running.url, "/").await;
        assert_eq!(status, 200, "{body}");
        assert!(body.contains("<title>MyWork</title>"), "{body}");

        let addr = addr_of(&running.url);
        assert!(
            data.join("mywork.db-wal").exists(),
            "база открыта в режиме WAL"
        );
        running.shutdown().await.unwrap();
        assert!(TcpStream::connect(addr).await.is_err(), "порт свободен");
        // Последнее соединение SQLite, закрываясь, переносит журнал в базу и удаляет его.
        assert!(!data.join("mywork.db-wal").exists(), "база закрыта");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_next_start_takes_the_same_address_so_the_window_keeps_its_settings() {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("data");
        let ui = ui_dir(root.path());
        let first = start(&data, &ui).await.unwrap();
        let url = first.url.clone();
        first.shutdown().await.unwrap();
        let local: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(data.join(LOCAL_FILE)).unwrap()).unwrap();
        assert_eq!(local["port"], addr_of(&url).port());

        let second = start(&data, &ui).await.unwrap();
        assert_eq!(second.url, url);
        second.shutdown().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_taken_port_gives_a_new_one_and_other_local_settings_stay() {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("data");
        let ui = ui_dir(root.path());
        let busy = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let busy_port = busy.local_addr().unwrap().port();
        std::fs::create_dir_all(&data).unwrap();
        std::fs::write(
            data.join(LOCAL_FILE),
            format!(r#"{{"port": {busy_port}, "printer": "Принтер"}}"#),
        )
        .unwrap();

        let running = start(&data, &ui).await.unwrap();
        let port = addr_of(&running.url).port();
        assert_ne!(port, busy_port);
        running.shutdown().await.unwrap();
        let local: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(data.join(LOCAL_FILE)).unwrap()).unwrap();
        assert_eq!(local["port"], port);
        assert_eq!(local["printer"], "Принтер");
        drop(busy);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_unreadable_local_file_is_left_as_it_is() {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join("data");
        let ui = ui_dir(root.path());
        std::fs::create_dir_all(&data).unwrap();
        std::fs::write(data.join(LOCAL_FILE), "{ правка руками").unwrap();

        let running = start(&data, &ui).await.unwrap();
        let (status, _) = get(&running.url, "/api/v1/server-info").await;
        assert_eq!(status, 200);
        running.shutdown().await.unwrap();
        assert_eq!(
            std::fs::read_to_string(data.join(LOCAL_FILE)).unwrap(),
            "{ правка руками"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_missing_interface_is_a_clear_error() {
        let root = tempfile::tempdir().unwrap();
        let error = match start(&root.path().join("data"), &root.path().join("нет")).await {
            Ok(_) => panic!("сервер поднялся без интерфейса"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains("папка интерфейса не найдена"), "{error}");
    }
}
