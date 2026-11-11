//! Логи не содержат паролей, токенов и строк поиска.
//!
//! Один `#[tokio::test]` на текущем потоке: все задачи идут в потоке с подписчиком.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use axum::http::Method;
use common::{Listen, TEST_PASSWORD, apis, capture_logs};
use serde_json::json;

#[tokio::test]
async fn logs_never_contain_passwords_tokens_or_search_text() {
    let logs = capture_logs();
    let mut secrets = vec![
        TEST_PASSWORD.to_owned(),
        "Неверный-Секрет-654".to_owned(),
        "Конфиденциально".to_owned(),
        "%D0%9A%D0%BE%D0%BD".to_owned(),
        "9211234567".to_owned(),
    ];
    for t in apis(Listen::Network).await {
        t.create_admin_user("Ирина").await;
        t.send(
            Method::POST,
            "/api/v1/auth/login",
            None,
            Some(
                json!({ "login": "ирина", "password": "Неверный-Секрет-654", "device_label": "" }),
            ),
        )
        .await;
        let token = t.login("Ирина").await;
        t.get(
            "/api/v1/server-info?query=%D0%9A%D0%BE%D0%BD%D1%84%D0%B8%D0%B4%D0%B5%D0%BD%D1%86%D0%B8%D0%B0%D0%BB%D1%8C%D0%BD%D0%BE&phone=9211234567",
            Some(&token),
        )
        .await;
        t.get("/api/v1/auth/me", Some(&token)).await;
        secrets.push(token);
    }
    let text = logs.text();
    for s in &secrets {
        assert!(!text.contains(s.as_str()), "в логе найдено «{s}»");
    }
    assert!(
        text.contains("/api/v1/auth/me"),
        "запросы всё же журналируются"
    );
}
