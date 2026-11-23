//! Таблица шаблонов документов создаётся на обоих движках: вид — ключ, модель может быть пустой.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use chrono::NaiveDate;
use mw_db::entity::document_template;
use mw_db::testing::all_backends;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};

#[tokio::test]
async fn table_holds_a_model_or_null_and_one_row_per_kind() {
    let at = NaiveDate::from_ymd_opt(2026, 11, 23)
        .and_then(|d| d.and_hms_opt(10, 0, 0))
        .unwrap();
    for (label, db) in all_backends().await {
        let row = |kind: &str, model: Option<&str>| document_template::ActiveModel {
            kind: Set(kind.to_owned()),
            model: Set(model.map(str::to_owned)),
            version: Set(3),
            updated_at: Set(at),
        };
        row("intake_act", Some("{}")).insert(&db).await.unwrap();
        row("issue_act", None).insert(&db).await.unwrap();
        assert!(
            row("intake_act", None).insert(&db).await.is_err(),
            "{label}"
        );
        let reset = document_template::Entity::find_by_id("issue_act")
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            (reset.model, reset.version, reset.updated_at),
            (None, 3, at),
            "{label}"
        );
    }
}
