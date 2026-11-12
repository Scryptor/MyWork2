//! Общие помощники тестов `mw-api`.
//!
//! A `// name` block in the plan's test listings that uses variables from an earlier block continues that block's test function; all other blocks are separate test functions.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Method, Request};
use chrono::{TimeZone, Utc};
use http_body_util::BodyExt;
use mw_api::{ApiConfig, router};
use mw_core::{Actor, Core, FixedClock, NewRole, NewUser, User};
use mw_db::entity::{role, status};
use mw_domain::permission::{Permission, PermissionSet};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use tower::ServiceExt;

/// Пароль всех тестовых пользователей.
pub const TEST_PASSWORD: &str = "пароль-1234";

#[derive(Clone, Copy, Debug)]
pub enum Listen {
    Loopback,
    Network,
}

pub struct TestApi {
    pub label: &'static str,
    pub core: Core,
    pub clock: Arc<FixedClock>,
    router: Router,
}

pub struct TestResponse {
    pub status: u16,
    pub headers: HeaderMap,
    pub body: serde_json::Value,
}

/// По одному API на бэкенд, пояс UTC, часы 2026-11-11T09:00:00Z.
pub async fn apis(listen: Listen) -> Vec<TestApi> {
    let loopback = matches!(listen, Listen::Loopback);
    let mut out = Vec::new();
    for (label, db) in mw_db::testing::all_backends().await {
        let clock = Arc::new(FixedClock::new(
            Utc.with_ymd_and_hms(2026, 11, 11, 9, 0, 0).unwrap(),
        ));
        let core = Core::open(db, clock.clone(), "UTC").await.unwrap();
        let router = router(core.clone(), ApiConfig { loopback });
        out.push(TestApi {
            label,
            core,
            clock,
            router,
        });
    }
    out
}

impl TestApi {
    pub async fn get(&self, path: &str, token: Option<&str>) -> TestResponse {
        self.send(Method::GET, path, token, None).await
    }

    pub async fn send(
        &self,
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<serde_json::Value>,
    ) -> TestResponse {
        self.send_from([10, 0, 0, 1], method, path, token, body)
            .await
    }

    pub async fn send_from(
        &self,
        ip: [u8; 4],
        method: Method,
        path: &str,
        token: Option<&str>,
        body: Option<serde_json::Value>,
    ) -> TestResponse {
        let mut builder = Request::builder()
            .method(method)
            .uri(path)
            .header("host", "127.0.0.1:8080")
            .header("content-type", "application/json");
        if let Some(token) = token {
            builder = builder.header("authorization", format!("Bearer {token}"));
        }
        let body = match body {
            Some(v) => Body::from(serde_json::to_vec(&v).unwrap()),
            None => Body::empty(),
        };
        let mut req = builder.body(body).unwrap();
        req.extensions_mut()
            .insert(ConnectInfo(SocketAddr::from((ip, 50000))));
        self.request(req).await
    }

    /// Отправляет запрос как есть; без `ConnectInfo` подставляет 10.0.0.1:50000.
    pub async fn request(&self, mut req: Request<Body>) -> TestResponse {
        if req.extensions().get::<ConnectInfo<SocketAddr>>().is_none() {
            req.extensions_mut()
                .insert(ConnectInfo(SocketAddr::from(([10, 0, 0, 1], 50000))));
        }
        let resp = self.router.clone().oneshot(req).await.unwrap();
        let status = resp.status().as_u16();
        let headers = resp.headers().clone();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let body = if bytes.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or_else(|_| {
                serde_json::Value::String(String::from_utf8_lossy(&bytes).into_owned())
            })
        };
        TestResponse {
            status,
            headers,
            body,
        }
    }
}

pub fn get_with_host(path: &str, host: &str) -> Request<Body> {
    Request::get(path)
        .header("host", host)
        .body(Body::empty())
        .unwrap()
}

pub async fn role_id_by_name(core: &Core, name: &str) -> i64 {
    role::Entity::find()
        .filter(role::Column::Name.eq(name))
        .one(core.db())
        .await
        .unwrap()
        .unwrap_or_else(|| panic!("нет роли {name}"))
        .id
}

pub async fn status_id_by_name(core: &Core, name: &str) -> i64 {
    status::Entity::find()
        .filter(status::Column::Name.eq(name))
        .one(core.db())
        .await
        .unwrap()
        .unwrap_or_else(|| panic!("нет статуса {name}"))
        .id
}

impl TestApi {
    /// Пользователь с ролью «Администратор» и паролем `TEST_PASSWORD`.
    pub async fn create_admin_user(&self, login: &str) -> User {
        let role_id = role_id_by_name(&self.core, "Администратор").await;
        self.core
            .create_user(
                &Actor::unrestricted("Тест"),
                NewUser {
                    login: login.into(),
                    display_name: login.into(),
                    role_id,
                    password: TEST_PASSWORD.into(),
                },
            )
            .await
            .unwrap()
    }

    /// Пользователь с новой ролью «Роль {login}» из заданных прав.
    pub async fn create_user_with(&self, login: &str, permissions: &[Permission]) -> User {
        let system = Actor::unrestricted("Тест");
        if self.core.is_open_mode().await.unwrap() {
            self.create_admin_user("admin").await;
        }
        let mut set = PermissionSet::empty();
        for p in permissions {
            set.insert(*p);
        }
        let role = self
            .core
            .create_role(
                &system,
                NewRole {
                    name: format!("Роль {login}"),
                    permissions: set,
                },
            )
            .await
            .unwrap();
        self.core
            .create_user(
                &system,
                NewUser {
                    login: login.into(),
                    display_name: login.into(),
                    role_id: role.id,
                    password: TEST_PASSWORD.into(),
                },
            )
            .await
            .unwrap()
    }

    /// Входит под `login` с `TEST_PASSWORD` и возвращает токен.
    pub async fn login(&self, login: &str) -> String {
        let r = self
            .send(
                Method::POST,
                "/api/v1/auth/login",
                None,
                Some(serde_json::json!({
                    "login": login, "password": TEST_PASSWORD, "device_label": "тест"
                })),
            )
            .await;
        assert_eq!(r.status, 200, "{}: {}", self.label, r.body);
        r.body["token"].as_str().unwrap().to_owned()
    }

    /// Токен администратора `admin`; пользователь создаётся при первом вызове.
    pub async fn admin_token(&self) -> String {
        let users = self
            .core
            .list_users(&Actor::unrestricted("Тест"))
            .await
            .unwrap();
        if !users.iter().any(|u| u.login == "admin") {
            self.create_admin_user("admin").await;
        }
        self.login("admin").await
    }

    pub async fn token_with(&self, login: &str, permissions: &[Permission]) -> String {
        self.create_user_with(login, permissions).await;
        self.login(login).await
    }
}

/// Записанный вывод tracing; подписчик действует, пока жив `LogCapture`.
pub struct LogCapture {
    buf: Arc<Mutex<Vec<u8>>>,
    _guard: tracing::subscriber::DefaultGuard,
}

impl LogCapture {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.buf.lock().unwrap()).into_owned()
    }
}

struct BufWriter(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for BufWriter {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Подписчик tracing уровня TRACE, пишущий в буфер (на текущем потоке).
pub fn capture_logs() -> LogCapture {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let writer = buf.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(move || BufWriter(writer.clone()))
        .finish();
    LogCapture {
        buf,
        _guard: tracing::subscriber::set_default(subscriber),
    }
}

/// Полное тело `NewOrder`: новый клиент, «Телефон», неисправность «Не включается»,
/// остальные строки пустые, суммы `null`/0, флаги `false`.
pub fn new_order(name: &str, phone: &str, brand: &str, model: &str) -> serde_json::Value {
    serde_json::json!({
        "client": { "new": {
            "full_name": name, "phone": phone, "address": "", "note": "",
            "source": "", "blacklisted": false
        } },
        "service_point_id": null, "status_id": null,
        "device_type": "Телефон", "brand": brand, "model": model,
        "serial": "", "color": "", "device_password": "", "completeness": "",
        "device_condition": "", "malfunction": "Не включается",
        "internal_comment": "", "master": "",
        "estimate_from": null, "estimate_to": null, "prepayment": 0,
        "warranty_days": null, "need_call": false, "need_parts": false
    })
}

/// POST /api/v1/orders; ждёт 201 и возвращает заказ.
pub async fn create_order(t: &TestApi, token: &str, body: serde_json::Value) -> serde_json::Value {
    let r = t
        .send(Method::POST, "/api/v1/orders", Some(token), Some(body))
        .await;
    assert_eq!(r.status, 201, "{}: {}", t.label, r.body);
    r.body
}

/// Номера заказов из `items` страницы.
pub fn ids(page: &serde_json::Value) -> Vec<i64> {
    page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| o["id"].as_i64().unwrap())
        .collect()
}
