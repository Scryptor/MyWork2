//! Настройки резервных копий MyWork 2 предлагаются после импорта.
#![allow(clippy::unwrap_used)]
mod common;

use std::sync::Arc;

use chrono::{TimeZone, Utc};
use common::*;
use mw_core::{BackupLegacySettings, Core, CoreError, FixedClock};
use mw_legacy::fixture::FixtureVersion::V32;
use mw_legacy::import_folder;

/// Ядро на файловой базе SQLite (копии делают `VACUUM INTO`) или на PostgreSQL.
async fn backup_cores(tmp: &std::path::Path) -> Vec<(&'static str, Core)> {
    let mut out = Vec::new();
    for (label, db) in mw_db::testing::all_backends().await {
        let db = if label == "sqlite" {
            let db = mw_db::connect(&mw_db::sqlite_url(&tmp.join(format!("{label}.db"))))
                .await
                .unwrap();
            mw_db::migrate(&db).await.unwrap();
            db
        } else {
            db
        };
        let clock = Arc::new(FixedClock::new(
            Utc.with_ymd_and_hms(2026, 11, 11, 9, 0, 0).unwrap(),
        ));
        let data = tmp.join(format!("data-{label}"));
        std::fs::create_dir_all(&data).unwrap();
        let core = Core::open(db, clock, "UTC")
            .await
            .unwrap()
            .with_attachments_dir(data)
            .with_backups();
        out.push((label, core));
    }
    out
}

#[tokio::test]
async fn backup_settings_of_mywork2_are_offered() {
    let tmp = tempfile::tempdir().unwrap();
    for (label, core) in backup_cores(tmp.path()).await {
        let data = core.data_dir_path().unwrap().to_path_buf();
        let src = fixture_dir(V32).await;
        import_folder(&core, &admin(), &options(src.path(), &data, false))
            .await
            .unwrap();
        let overview = core.backup_overview(&admin()).await.unwrap();
        if label != "sqlite" {
            assert_eq!(overview.legacy, None, "{label}");
            assert!(matches!(
                core.choose_legacy_backup(&admin(), true).await,
                Err(CoreError::Unavailable(_))
            ));
            continue;
        }
        assert_eq!(
            overview.legacy,
            Some(BackupLegacySettings {
                auto: true,
                folders: vec!["D:\\Backup".to_owned()],
            }),
            "{label}"
        );
        let settings = core.choose_legacy_backup(&admin(), true).await.unwrap();
        assert!(settings.auto && settings.legacy_reviewed, "{label}");
        assert_eq!(settings.folder, None, "{label}");
        assert_eq!(core.backup_overview(&admin()).await.unwrap().legacy, None);
    }
}

#[tokio::test]
async fn declining_marks_the_offer_as_answered() {
    let tmp = tempfile::tempdir().unwrap();
    for (label, core) in backup_cores(tmp.path()).await {
        if label != "sqlite" {
            continue;
        }
        let data = core.data_dir_path().unwrap().to_path_buf();
        let src = fixture_dir(V32).await;
        import_folder(&core, &admin(), &options(src.path(), &data, false))
            .await
            .unwrap();
        let settings = core.choose_legacy_backup(&admin(), false).await.unwrap();
        let expected = mw_core::BackupSettings {
            legacy_reviewed: true,
            ..Default::default()
        };
        assert_eq!(settings, expected);
        let overview = core.backup_overview(&admin()).await.unwrap();
        assert_eq!((overview.settings, overview.legacy), (expected, None));
    }
}
