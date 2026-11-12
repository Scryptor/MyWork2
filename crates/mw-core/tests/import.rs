//! Запись импорта из MyWork 2: одна транзакция, исходные номера, продолжение нумерации.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use std::sync::Arc;

use chrono::{TimeZone, Utc};
use common::*;
use mw_core::{
    AuditFilter, ClientPhone, Core, CoreError, FixedClock, ImportMode, NewClient, SystemClock,
};
use mw_domain::money::Money;
use mw_domain::permission::Permission;
use mw_domain::permission::PermissionSet;
use mw_domain::warranty::WarrantyTerm;
use sea_orm::{EntityTrait, PaginatorTrait};

#[tokio::test]
async fn import_into_empty_database_keeps_numbers_and_continues_them() {
    for t in cores().await {
        let s = t
            .core
            .import_bundle(&admin(), sample_bundle(), ImportMode::IntoEmpty)
            .await
            .unwrap();
        assert_eq!(
            (
                s.orders,
                s.deleted_orders,
                s.clients,
                s.client_phones,
                s.history,
                s.legacy_rows
            ),
            (3, 1, 2, 2, 2, 5),
            "{}",
            t.label
        );
        let o = t.core.get_order(&admin(), 10).await.unwrap();
        assert_eq!(
            (
                o.client_id,
                status_name(&t.core, o.status_id).await.as_str()
            ),
            (9, "Диагностика")
        );
        assert_eq!(
            (o.estimate_from, o.estimate_to),
            (
                Some(Money::from_rubles(1500)),
                Some(Money::from_rubles(1500))
            )
        );
        assert_eq!(
            t.core.get_order(&admin(), 5).await.unwrap().warranty_term,
            Some(WarrantyTerm::days(30))
        );
        assert!(
            t.core
                .get_order(&admin(), 6)
                .await
                .unwrap()
                .deleted_at
                .is_some()
        );
        assert_eq!(
            ids(t
                .core
                .find_orders_by_barcode(&admin(), "123456789012")
                .await
                .unwrap()),
            vec![5]
        );
        assert_eq!(
            ids(t
                .core
                .find_clients_by_phone(&admin(), "89117654321")
                .await
                .unwrap()),
            vec![7]
        );
        // Новый клиент идёт первым: заказ с новым клиентом заберёт следующий номер себе.
        assert_eq!(
            t.core
                .create_client(
                    &admin(),
                    NewClient {
                        full_name: "Новый".into(),
                        ..Default::default()
                    }
                )
                .await
                .unwrap()
                .id,
            15,
            "{}",
            t.label
        );
        assert_eq!(
            t.core
                .create_order(&admin(), sample_new_order())
                .await
                .unwrap()
                .id,
            16,
            "{}",
            t.label
        );
        let st = t.core.settings();
        assert_eq!(
            (
                st.currency_symbol.as_str(),
                st.timezone.as_str(),
                st.firm_name.as_str()
            ),
            ("руб.", "UTC", "СЕРВИС")
        );
        assert_eq!(
            st.default_service_point_id,
            Some(service_point_id_by_name(&t.core, "Ленина, 1").await)
        );
        let h = t
            .core
            .list_audit(
                &admin(),
                &AuditFilter {
                    order_id: Some(5),
                    ..Default::default()
                },
                50,
                0,
            )
            .await
            .unwrap();
        assert_eq!(
            h.items
                .iter()
                .map(|e| e.action.as_str())
                .collect::<Vec<_>>(),
            ["legacy.history", "legacy.status"]
        );
        assert_eq!(h.items[0].user_name, "ADMIN");
        assert_eq!(count_audit(&t.core, "legacy.imported").await, 1);
        assert_eq!(
            legacy_target(&t.core, "Catalog", "5").await,
            Some(("orders".to_owned(), 5))
        );
        assert_eq!(
            legacy_target(&t.core, "Users", "1").await.map(|x| x.0),
            Some("users".to_owned())
        );
        assert_eq!(legacy_target(&t.core, "Stock", "1").await, None);
        assert_eq!(t.core.list_statuses(true).await.unwrap().len(), 9);
    }
}

#[tokio::test]
async fn legacy_passwords_work_after_import() {
    for t in cores().await {
        t.core
            .import_bundle(&admin(), sample_bundle(), ImportMode::IntoEmpty)
            .await
            .unwrap();
        assert!(
            t.core
                .login("ирина", "1234", "x")
                .await
                .unwrap()
                .permissions
                .contains(Permission::UsersManage)
        );
        assert!(matches!(
            t.core.login("Олег", "1234", "x").await,
            Err(CoreError::InvalidCredentials)
        ));
        assert!(!t.core.is_open_mode().await.unwrap());
    }
}

#[tokio::test]
async fn import_refuses_non_empty_database_and_changes_nothing() {
    for t in cores().await {
        t.core
            .create_client(
                &admin(),
                NewClient {
                    full_name: "Уже есть".into(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_validation(
            t.core
                .import_bundle(&admin(), sample_bundle(), ImportMode::IntoEmpty)
                .await,
            &[("database", "not_empty")],
        );
        assert_eq!(t.core.import_target_state().await.unwrap().clients, 1);
        assert_eq!(t.core.list_statuses(true).await.unwrap().len(), 8);
    }
}

#[tokio::test]
async fn broken_bundle_rolls_back_everything() {
    for t in cores().await {
        let broken = [
            {
                let mut b = sample_bundle();
                b.orders[2].status = "Нет такого".into();
                b
            },
            {
                let mut b = sample_bundle();
                b.roles[0].permissions = PermissionSet::from_codes(["orders_create"]);
                b
            },
            {
                let mut b = sample_bundle();
                b.next_order_id = 10;
                b
            },
            {
                let mut b = sample_bundle();
                b.clients[0].phones.push(ClientPhone {
                    phone: "+7 921 1234567".into(),
                    note: String::new(),
                });
                b
            },
        ];
        for b in broken {
            assert!(
                matches!(
                    t.core
                        .import_bundle(&admin(), b, ImportMode::IntoEmpty)
                        .await,
                    Err(CoreError::Internal(_))
                ),
                "{}",
                t.label
            );
            assert!(t.core.import_target_state().await.unwrap().is_empty());
            assert_eq!(t.core.list_statuses(true).await.unwrap().len(), 8);
        }
    }
}

#[tokio::test]
async fn failure_inside_the_transaction_rolls_back_everything() {
    // Пакет проходит проверку, но нарушает уникальность в базе: вторая запись `legacy_rows`
    // с тем же ключом падает уже после записи заказов, пользователей и журнала.
    for t in cores().await {
        let mut b = sample_bundle();
        let dup = b.legacy_rows[0].clone();
        b.legacy_rows.push(dup);
        assert!(
            t.core
                .import_bundle(&admin(), b, ImportMode::IntoEmpty)
                .await
                .is_err()
        );
        assert!(t.core.import_target_state().await.unwrap().is_empty());
        assert_eq!(t.core.list_statuses(true).await.unwrap().len(), 8);
        assert!(t.core.is_open_mode().await.unwrap());
    }
}

#[tokio::test]
async fn import_needs_settings_and_users_rights() {
    for t in cores().await {
        assert!(matches!(
            t.core
                .import_bundle(
                    &actor_with(&[Permission::SettingsManage]),
                    sample_bundle(),
                    ImportMode::IntoEmpty
                )
                .await,
            Err(CoreError::Forbidden(Permission::UsersManage))
        ));
    }
}

async fn file_sqlite_core(path: &std::path::Path) -> Core {
    let db = mw_db::connect(&mw_db::sqlite_url(path)).await.unwrap();
    mw_db::migrate(&db).await.unwrap();
    let clock = Arc::new(FixedClock::new(
        Utc.with_ymd_and_hms(2026, 11, 11, 9, 0, 0).unwrap(),
    ));
    Core::open(db, clock, "UTC").await.unwrap()
}

async fn order_count_in_file(path: &std::path::Path) -> u64 {
    let db = mw_db::connect(&mw_db::sqlite_url(path)).await.unwrap();
    mw_db::entity::order::Entity::find()
        .count(&db)
        .await
        .unwrap()
}

#[tokio::test]
async fn replace_mode_backs_up_sqlite_and_is_refused_on_postgres() {
    let dir = tempfile::tempdir().unwrap();
    let core = file_sqlite_core(&dir.path().join("mywork.db")).await;
    core.import_bundle(&admin(), sample_bundle(), ImportMode::IntoEmpty)
        .await
        .unwrap();
    core.create_order(&admin(), sample_new_order())
        .await
        .unwrap();
    let backup = dir.path().join("backups").join("before.db");
    core.import_bundle(
        &admin(),
        sample_bundle(),
        ImportMode::Replace {
            backup_to: backup.clone(),
        },
    )
    .await
    .unwrap();
    assert_eq!(order_count_in_file(&backup).await, 4);
    assert_eq!(core.import_target_state().await.unwrap().orders, 3);
    assert!(
        core.import_bundle(
            &admin(),
            sample_bundle(),
            ImportMode::Replace { backup_to: backup }
        )
        .await
        .is_err()
    );
    assert_eq!(core.import_target_state().await.unwrap().orders, 3);
    if let Some(pg) = mw_db::testing::postgres_isolated().await {
        let core = Core::open(pg, Arc::new(SystemClock), "UTC").await.unwrap();
        assert_validation(
            core.import_bundle(
                &admin(),
                sample_bundle(),
                ImportMode::Replace {
                    backup_to: dir.path().join("x.db"),
                },
            )
            .await,
            &[("mode", "replace_unsupported")],
        );
    }
}
