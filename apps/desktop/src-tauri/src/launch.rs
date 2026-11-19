//! Встроенный сервер программы для компьютера: папка данных, адрес на 127.0.0.1, штатная остановка.

use mw_server::{Config, Server, ServerError, os_data_dir, resolve_data_dir};
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

/// Файл настроек этого компьютера в папке данных (спецификация, «Где лежат данные»).
pub const LOCAL_FILE: &str = "local.json";

/// Папка данных программы: `data` рядом с программой при `portable.txt`, иначе папка данных ОС + `MyWork3`.
/// Если ОС свою папку не назвала (нет `HOME` / `APPDATA`), данные тоже лежат в `data` рядом с программой.
pub fn data_dir(exe_dir: &Path) -> PathBuf {
    resolve_data_dir(None, Some(exe_dir), os_data_dir()).unwrap_or_else(|_| exe_dir.join("data"))
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
/// (из `local.json`), если он свободен, иначе любой свободный: у окна тот же адрес, а с ним и источник,
/// к которому браузерное хранилище привязывает настройки вида, сохранённые виды и вход.
pub async fn start(data_dir: &Path, ui_dir: &Path) -> Result<Running, ServerError> {
    let port = remembered_port(data_dir)
        .filter(|&port| port_is_free(port))
        .unwrap_or(0);
    let config = Config {
        listen: SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
        data_dir: data_dir.to_path_buf(),
        database_url: None,
        timezone: None,
        ui_dir: Some(ui_dir.to_path_buf()),
    };
    let server = Server::start(&config).await?;
    let addr = server.local_addr();
    if addr.port() != port {
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

/// Свободен ли порт на 127.0.0.1 прямо сейчас (проверка привязкой; занять его сразу после может кто-то
/// другой — тогда `Server::start` сообщит, что адрес занят).
fn port_is_free(port: u16) -> bool {
    std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port)).is_ok()
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
    if let Err(e) = std::fs::write(&path, text) {
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
    fn portable_txt_next_to_the_program_keeps_the_data_beside_it() {
        let exe = tempfile::tempdir().unwrap();
        assert_eq!(
            data_dir(exe.path()),
            mw_server::os_data_dir().unwrap().join("MyWork3")
        );
        std::fs::write(exe.path().join("portable.txt"), "").unwrap();
        assert_eq!(data_dir(exe.path()), exe.path().join("data"));
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
