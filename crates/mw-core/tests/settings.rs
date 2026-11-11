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
            (s.default_warranty_days, s.intake_required_fields),
            (Some(30), true)
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
                    default_warranty_days: Some(None),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(s.default_warranty_days, None);
        assert_eq!(t.core.settings().default_warranty_days, None);
        assert_eq!(count_audit(&t.core, "settings.updated").await, 1);
    }
    Ok(())
}
