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
                // Роль с правом есть, но все пользователи — в роли «Мастера».
                let mut b = sample_bundle();
                b.users.remove(0);
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

#[tokio::test]
async fn past_imports_are_listed_and_read_with_the_summary_in_the_report() {
    use mw_core::{ImportSummary, LegacyImportMode};
    for t in cores().await {
        assert!(
            t.core
                .list_legacy_imports(&admin())
                .await
                .unwrap()
                .is_empty()
        );
        let s = t
            .core
            .import_bundle(&admin(), sample_bundle(), ImportMode::IntoEmpty)
            .await
            .unwrap();
        let list = t.core.list_legacy_imports(&admin()).await.unwrap();
        assert_eq!(list.len(), 1, "{}", t.label);
        let r = &list[0];
        assert_eq!(
            (r.id, r.source_path.as_str(), r.mode),
            (s.import_id, "fixture/MyWork2", LegacyImportMode::IntoEmpty)
        );
        assert_eq!(r.imported_at, t.core.now());
        // Отчёт пакета остаётся как был, к нему добавляется сводка записанного.
        assert_eq!(r.report["warnings"], serde_json::json!([]));
        let summary: ImportSummary = serde_json::from_value(r.report["summary"].clone()).unwrap();
        assert_eq!(summary, s);
        assert_eq!(
            &t.core.legacy_import(&admin(), s.import_id).await.unwrap(),
            r
        );
        assert!(matches!(
            t.core.legacy_import(&admin(), s.import_id + 100).await,
            Err(CoreError::NotFound { .. })
        ));
        let weak = actor_with(&[Permission::SettingsManage]);
        assert!(matches!(
            t.core.list_legacy_imports(&weak).await,
            Err(CoreError::Forbidden(Permission::UsersManage))
        ));
        assert!(matches!(
            t.core.legacy_import(&weak, s.import_id).await,
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

#[tokio::test]
async fn import_splits_large_order_lists_into_batches() {
    for t in cores().await {
        let mut b = sample_bundle();
        let template = b.orders[2].clone();
        b.orders.truncate(2);
        for id in 100..1100 {
            let mut o = template.clone();
            o.id = id;
            o.barcode = format!("{id:012}");
            o.tracking = Vec::new();
            b.orders.push(o);
        }
        b.next_order_id = 2000;
        let s = t
            .core
            .import_bundle(&admin(), b, ImportMode::IntoEmpty)
            .await
            .unwrap();
        assert_eq!(s.orders, 1002, "{}", t.label);
        assert_eq!(t.core.import_target_state().await.unwrap().orders, 1002);
    }
}

#[tokio::test]
async fn import_fills_status_changed_at_from_the_last_legacy_status() {
    for t in cores().await {
        t.core
            .import_bundle(&admin(), sample_bundle(), ImportMode::IntoEmpty)
            .await
            .unwrap();
        let changed = |id: i64| {
            let t = &t;
            async move {
                mw_db::entity::order::Entity::find_by_id(id)
                    .one(t.core.db())
                    .await
                    .unwrap()
                    .unwrap()
                    .status_changed_at
            }
        };
        let expected = chrono::NaiveDate::from_ymd_opt(2015, 3, 5)
            .unwrap()
            .and_hms_opt(14, 20, 0)
            .unwrap();
        assert_eq!(changed(5).await, Some(expected), "{}", t.label);
        assert_eq!(changed(10).await, None, "{}", t.label);
    }
}

#[tokio::test]
async fn import_turns_master_names_into_master_users() {
    for t in cores().await {
        let s = t
            .core
            .import_bundle(&admin(), sample_bundle(), ImportMode::IntoEmpty)
            .await
            .unwrap();
        // «Петров» (файл мастеров, мастер по умолчанию и «ПЕТРОВ» в заказах) — новый мастер без
        // входа; «олег» — пользователь MyWork 2 «Олег», он получил отметку и не задвоен. Роли — две
        // из пакета и готовые «Мастер» и «Приёмщик».
        assert_eq!(
            (s.users, s.masters_created, s.masters_marked, s.roles),
            (2, 1, 1, 4),
            "{}",
            t.label
        );
        let users = t.core.list_users(&admin()).await.unwrap();
        assert_eq!(users.len(), 3, "{}", t.label);
        let by_name = |name: &str| users.iter().find(|u| u.display_name == name).unwrap();
        let (irina, oleg, petrov) = (by_name("Ирина"), by_name("Олег"), by_name("Петров"));
        assert_eq!(
            (irina.is_master, irina.can_sign_in),
            (false, true),
            "{}",
            t.label
        );
        assert_eq!(
            (oleg.is_master, oleg.can_sign_in, oleg.login.as_str()),
            (true, true, "Олег")
        );
        assert_eq!(
            (petrov.is_master, petrov.can_sign_in, petrov.login.as_str()),
            (true, false, "")
        );
        assert_eq!(petrov.role_id, role_id_by_name(&t.core, "Мастер").await);
        let roles = t.core.list_roles(&admin()).await.unwrap();
        let master_role = roles.iter().find(|r| r.name == "Мастер").unwrap();
        assert_eq!(
            master_role.permissions,
            PermissionSet::from_codes(["orders_edit", "clients_view"])
        );
        for id in [5, 6, 10] {
            let o = t.core.get_order(&admin(), id).await.unwrap();
            assert_eq!(
                (o.master.as_str(), o.master_user_id),
                ("ПЕТРОВ", Some(petrov.id)),
                "{}: заказ {id}",
                t.label
            );
        }
        assert_eq!(t.core.settings().default_master_user_id, Some(petrov.id));
        assert!(
            t.core
                .list_dictionary(mw_domain::dictionary::DictionaryKind::Master, true)
                .await
                .unwrap()
                .is_empty()
        );
        // Вход MyWork 2 у отмеченного пользователя прежний.
        assert!(t.core.login("Олег", "qwerty", "test").await.is_ok());
    }
}

#[tokio::test]
async fn masters_in_the_dictionary_of_a_bundle_are_refused() {
    for t in cores().await {
        let mut b = sample_bundle();
        for (kind, values) in &mut b.dictionary {
            if *kind == mw_domain::dictionary::DictionaryKind::Master {
                values.push("ПЕТРОВ".into());
            }
        }
        let err = t
            .core
            .import_bundle(&admin(), b, ImportMode::IntoEmpty)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("мастера"), "{}: {err}", t.label);
    }
}

/// Импорт стирает роли базы, но готовые «Мастер» и «Приёмщик» остаются в программе: их нет в пакете
/// — добавляются с готовыми правами; группа MyWork 2 с тем же названием (без учёта регистра и «ё»)
/// не задваивается и сохраняет свои права.
#[tokio::test]
async fn import_keeps_the_builtin_roles() {
    for t in cores().await {
        // Нет ни одного нового мастера: роль «Мастер» не нужна правилу мастеров, но остаётся.
        let mut b = sample_bundle();
        b.masters.clear();
        b.settings.default_master = None;
        for o in &mut b.orders {
            o.master = String::new();
        }
        let s = t
            .core
            .import_bundle(&admin(), b, ImportMode::IntoEmpty)
            .await
            .unwrap();
        assert_eq!(s.roles, 4, "{}", t.label);
        let roles = t.core.list_roles(&admin()).await.unwrap();
        let builtin = |key: &str| {
            roles
                .iter()
                .filter(|r| mw_domain::search::normalize_search(&r.name) == key)
                .collect::<Vec<_>>()
        };
        let clerk = builtin("приемщик");
        assert_eq!(clerk.len(), 1, "{}", t.label);
        assert_eq!(
            (clerk[0].name.as_str(), &clerk[0].permissions),
            (
                "Приёмщик",
                &PermissionSet::from_codes([
                    "orders_create",
                    "orders_edit",
                    "clients_view",
                    "clients_create",
                    "clients_edit"
                ])
            ),
            "{}",
            t.label
        );
        let master = builtin("мастер");
        assert_eq!(master.len(), 1, "{}", t.label);
        assert_eq!(
            master[0].permissions,
            PermissionSet::from_codes(["orders_edit", "clients_view"])
        );
        assert_eq!(roles.len(), 4);
    }
    for t in cores().await {
        // Группы MyWork 2 «ПРИЕМЩИК» и «мастер» — те же роли: своих прав не теряют, не задваиваются.
        let mut b = sample_bundle();
        b.roles.push(mw_core::NewRole {
            name: "ПРИЕМЩИК".into(),
            permissions: PermissionSet::from_codes(["clients_view"]),
        });
        b.roles[1].name = "мастер".into();
        b.users[1].role = "мастер".into();
        for row in &mut b.legacy_rows {
            if let Some(mw_core::LegacyTarget::Role(name)) = &mut row.target {
                *name = "мастер".into();
            }
        }
        let s = t
            .core
            .import_bundle(&admin(), b, ImportMode::IntoEmpty)
            .await
            .unwrap();
        assert_eq!(s.roles, 3, "{}", t.label);
        let roles = t.core.list_roles(&admin()).await.unwrap();
        let names: Vec<_> = roles.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(
            names,
            ["Администратор", "мастер", "ПРИЕМЩИК"],
            "{}",
            t.label
        );
        assert_eq!(
            roles[2].permissions,
            PermissionSet::from_codes(["clients_view"])
        );
        assert_eq!(
            roles[1].permissions,
            PermissionSet::from_codes(["orders_edit"])
        );
    }
}
