mod common;

use std::sync::Arc;

use chrono::{TimeZone, Utc};
use common::*;
use mw_core::{Core, CoreError, FixedClock, SettingsPatch};
use mw_domain::permission::Permission;
use mw_domain::validation::FieldError;

#[tokio::test]
async fn defaults_and_timezone_saved_once() -> Result<(), CoreError> {
    for (label, db) in mw_db::testing::all_backends().await {
        let clock = Arc::new(FixedClock::new(
            Utc.with_ymd_and_hms(2026, 11, 11, 7, 30, 0).unwrap(),
        ));
        let core = Core::open(db.clone(), clock.clone(), "Europe/Moscow").await?;
        let s = core.settings();
        assert_eq!(
            (
                s.currency_symbol.as_str(),
                s.master_percent,
                s.diagnostics_overdue_days
            ),
            ("₽", 50, 4),
            "{label}"
        );
        assert_eq!(
            (s.default_warranty.as_str(), s.intake_required_fields),
            ("30 дней", true)
        );
        assert_eq!(core.now().to_string(), "2026-11-11 10:30:00");
        let reopened = Core::open(db, clock, "UTC").await?;
        assert_eq!(reopened.settings().timezone, "Europe/Moscow");
    }
    Ok(())
}

#[tokio::test]
async fn update_settings_checks_rights_validates_and_audits() -> Result<(), CoreError> {
    for t in cores().await {
        let no_rights = actor_with(&[]);
        let err = t
            .core
            .update_settings(
                &no_rights,
                SettingsPatch {
                    master_percent: Some(40),
                    ..Default::default()
                },
            )
            .await;
        assert!(
            matches!(err, Err(CoreError::Forbidden(Permission::SettingsManage))),
            "{}",
            t.label
        );
        let bad = SettingsPatch {
            master_percent: Some(101),
            timezone: Some("Mars/Olympus".into()),
            ..Default::default()
        };
        match t.core.update_settings(&admin(), bad).await {
            Err(CoreError::Validation(f)) => assert_eq!(
                f,
                vec![
                    FieldError::new("master_percent", "out_of_range"),
                    FieldError::new("timezone", "invalid")
                ]
            ),
            other => panic!("{}: {other:?}", t.label),
        }
        let s = t
            .core
            .update_settings(
                &admin(),
                SettingsPatch {
                    default_warranty: Some("6 месяцев".into()),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(s.default_warranty, "6 месяцев");
        assert_eq!(t.core.settings().default_warranty, "6 месяцев");
        assert_eq!(count_audit(&t.core, "settings.updated").await, 1);
    }
    Ok(())
}

#[tokio::test]
async fn reopening_keeps_single_timezone_row_with_first_value() -> Result<(), CoreError> {
    use mw_db::entity::setting;
    use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
    for (label, db) in mw_db::testing::all_backends().await {
        let clock = Arc::new(FixedClock::new(
            Utc.with_ymd_and_hms(2026, 11, 11, 7, 30, 0).unwrap(),
        ));
        Core::open(db.clone(), clock.clone(), "Europe/Moscow").await?;
        let second = Core::open(db.clone(), clock, "Asia/Tokyo").await?;
        let rows = setting::Entity::find()
            .filter(setting::Column::Key.eq("timezone"))
            .count(&db)
            .await?;
        assert_eq!(rows, 1, "{label}");
        assert_eq!(second.settings().timezone, "Europe/Moscow", "{label}");
    }
    Ok(())
}

#[tokio::test]
async fn new_settings_have_defaults_and_validate_service_point() -> Result<(), CoreError> {
    for t in cores().await {
        let s = t.core.settings();
        assert_eq!(
            (
                s.default_warranty.as_str(),
                s.default_service_point_id,
                s.default_master.as_str(),
                s.firm_name.as_str()
            ),
            ("30 дней", None, "", ""),
            "{}",
            t.label
        );
        let p = t.core.create_service_point(&admin(), "Мира, 5").await?;
        let s = t
            .core
            .update_settings(
                &admin(),
                SettingsPatch {
                    default_service_point_id: Some(Some(p.id)),
                    intake_terms: Some("Строка 1\nСтрока 2\n".into()),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(
            (s.default_service_point_id, s.intake_terms.as_str()),
            (Some(p.id), "Строка 1\nСтрока 2"),
            "{}",
            t.label
        );
        assert_validation(
            t.core
                .update_settings(
                    &admin(),
                    SettingsPatch {
                        default_service_point_id: Some(Some(9_999)),
                        ..Default::default()
                    },
                )
                .await,
            &[("default_service_point_id", "invalid")],
        );
        let cleared = t
            .core
            .update_settings(
                &admin(),
                SettingsPatch {
                    default_service_point_id: Some(None),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(cleared.default_service_point_id, None, "{}", t.label);
    }
    Ok(())
}

#[tokio::test]
async fn archived_default_point_is_invalid_to_set_again() -> Result<(), CoreError> {
    use mw_core::ServicePointPatch;
    for t in cores().await {
        let p = t.core.create_service_point(&admin(), "Мира, 5").await?;
        let set = |id| SettingsPatch {
            default_service_point_id: Some(Some(id)),
            ..Default::default()
        };
        t.core.update_settings(&admin(), set(p.id)).await?;
        t.core
            .update_service_point(
                &admin(),
                p.id,
                ServicePointPatch {
                    archived: Some(true),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(t.core.settings().default_service_point_id, None);
        let before = count_audit(&t.core, "settings.updated").await;
        assert_validation(
            t.core.update_settings(&admin(), set(p.id)).await,
            &[("default_service_point_id", "invalid")],
        );
        assert_eq!(count_audit(&t.core, "settings.updated").await, before);
        let other = t.core.create_service_point(&admin(), "Ленина, 1").await?;
        t.core
            .update_service_point(
                &admin(),
                other.id,
                ServicePointPatch {
                    archived: Some(true),
                    ..Default::default()
                },
            )
            .await?;
        assert_validation(
            t.core.update_settings(&admin(), set(other.id)).await,
            &[("default_service_point_id", "invalid")],
        );
        assert_validation(
            t.core
                .update_settings(
                    &admin(),
                    SettingsPatch {
                        default_service_point_id: Some(Some(other.id)),
                        master_percent: Some(101),
                        timezone: Some("Mars/Olympus".into()),
                        ..Default::default()
                    },
                )
                .await,
            &[
                ("master_percent", "out_of_range"),
                ("default_service_point_id", "invalid"),
                ("timezone", "invalid"),
            ],
        );
    }
    Ok(())
}

#[tokio::test]
async fn archived_default_point_reads_as_none() -> Result<(), CoreError> {
    use mw_core::ServicePointPatch;
    for t in cores().await {
        let point = t.core.create_service_point(&admin(), "Мира, 5").await?.id;
        let s = t
            .core
            .update_settings(
                &admin(),
                SettingsPatch {
                    default_service_point_id: Some(Some(point)),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(s.default_service_point_id, Some(point), "{}", t.label);
        let archive = |archived: bool| ServicePointPatch {
            archived: Some(archived),
            ..Default::default()
        };
        t.core
            .update_service_point(&admin(), point, archive(true))
            .await?;
        assert_eq!(t.core.settings().default_service_point_id, None);
        let reopened = Core::open(t.core.db().clone(), t.clock.clone(), "UTC").await?;
        assert_eq!(reopened.settings().default_service_point_id, None);
        t.core
            .update_service_point(&admin(), point, archive(false))
            .await?;
        assert_eq!(t.core.settings().default_service_point_id, Some(point));
        set_setting_directly(
            &t.core,
            "default_service_point_id",
            serde_json::json!(987_654),
        )
        .await;
        let reopened = Core::open(t.core.db().clone(), t.clock.clone(), "UTC").await?;
        assert_eq!(reopened.settings().default_service_point_id, None);
    }
    Ok(())
}

/// Параллельные записи настроек: кэш равен последнему зафиксированному, то есть тому,
/// что лежит в базе.
#[tokio::test]
async fn parallel_settings_writes_leave_the_cache_equal_to_the_database() {
    for (label, db) in mw_db::testing::all_backends().await {
        let clock = Arc::new(FixedClock::new(
            Utc.with_ymd_and_hms(2026, 11, 11, 7, 30, 0).unwrap(),
        ));
        let core = Core::open(db.clone(), clock.clone(), "UTC").await.unwrap();
        for round in 0..5u8 {
            let mut tasks = Vec::new();
            for i in 1..=12u8 {
                let core = core.clone();
                tasks.push(tokio::spawn(async move {
                    core.update_settings(
                        &admin(),
                        SettingsPatch {
                            master_percent: Some(round * 12 + i),
                            ..Default::default()
                        },
                    )
                    .await
                    .unwrap();
                }));
            }
            for t in tasks {
                t.await.unwrap();
            }
            let stored = Core::open(db.clone(), clock.clone(), "UTC").await.unwrap();
            assert_eq!(
                core.settings().master_percent,
                stored.settings().master_percent,
                "{label} round {round}"
            );
        }
    }
}
