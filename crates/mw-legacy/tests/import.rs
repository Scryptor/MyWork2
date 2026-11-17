//! Импорт папки MyWork 2 целиком: файлы, отчёт, вход, повторный импорт.
#![allow(clippy::unwrap_used)]
mod common;

use common::*;
use mw_core::{Actor, CoreError, DatabaseKind, NewClient};
use mw_domain::permission::{Permission, PermissionSet};
use mw_legacy::fixture::FIXTURE_SMS_TOKEN;
use mw_legacy::fixture::FixtureVersion::{self, V30, V32};
use mw_legacy::{LegacyError, Severity, WarningCode, import_folder};

type Res = Result<(), Box<dyn std::error::Error>>;

#[tokio::test]
async fn every_schema_version_imports_without_losing_rows() -> Res {
    for version in FixtureVersion::ALL {
        for t in cores().await {
            let (src, data) = (fixture_dir(version).await, tempfile::tempdir()?);
            let r =
                import_folder(&t.core, &admin(), &options(src.path(), data.path(), false)).await?;
            let n = source_counts(src.path()).await;
            let label = format!("{version:?} {}", t.label);
            assert_eq!(
                (r.summary.orders, r.summary.history),
                (n.catalog, n.states_map + n.history_bd),
                "{label}"
            );
            assert_eq!(
                r.summary.clients,
                if n.clients_map > 0 {
                    n.clients_map + 2
                } else {
                    7
                },
                "{label}"
            );
            assert_eq!(r.summary.next_order_id, 16, "{label}");
            assert_eq!(
                money_totals(&t.core).await,
                [150_000, 50_000, 1_500_150_000, 0, 0],
                "{label}"
            );
            assert_eq!(
                t.core.create_order(&admin(), sample_new_order()).await?.id,
                16,
                "{label}"
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn v32_files_and_warnings_round_trip() -> Res {
    for t in cores().await {
        let (src, data) = (fixture_dir(V32).await, tempfile::tempdir()?);
        let r = import_folder(&t.core, &admin(), &options(src.path(), data.path(), false)).await?;
        let att = data.path().join("attachments");
        assert_eq!(
            std::fs::read(att.join("orders/1/docs/act.pdf"))?,
            b"%PDF-fixture"
        );
        assert_eq!(std::fs::read(att.join("orders/3/video.mp4"))?, b"video-3");
        assert_eq!(
            std::fs::read(att.join("stock/1/abc123.jpg"))?,
            b"stock-photo"
        );
        assert!(
            att.join("legacy/ClientFiles/2/404/lost.jpg").exists()
                && att.join("legacy/ClientFiles/2/note.txt").exists()
        );
        assert!(!std::fs::read_dir(&att)?.any(|e| {
            e.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".import")
        }));
        assert_eq!((r.files.copied, r.files.missing), (6, 1), "{}", t.label);
        assert_eq!(
            (
                r.count(WarningCode::FileMissing),
                r.count(WarningCode::ClientFilesOrphan)
            ),
            (1, 2)
        );
        assert_eq!(
            legacy_target(&t.core, "ClientFiles", "1/1/photo.jpg").await,
            Some(("orders".to_owned(), 1))
        );
        assert_eq!(
            r.warnings
                .iter()
                .filter(|w| w.code.severity() == Severity::Warning)
                .count(),
            27,
            "{}",
            t.label
        );
    }
    Ok(())
}

#[tokio::test]
async fn imported_client_files_are_listed_as_attachments() -> Res {
    use mw_core::AttachmentKind::{File, Photo};
    for t in cores().await {
        let (src, data) = (fixture_dir(V32).await, tempfile::tempdir()?);
        let core = t
            .core
            .clone()
            .with_attachments_dir(data.path().to_path_buf());
        import_folder(&core, &admin(), &options(src.path(), data.path(), false)).await?;
        let shown = |list: Vec<mw_core::Attachment>| {
            list.into_iter()
                .map(|a| (a.kind, a.name, a.legacy, a.missing))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            shown(core.list_order_attachments(&admin(), 1).await?),
            vec![
                (Photo, "photo.jpg".to_owned(), true, false),
                (File, "act.pdf".to_owned(), true, false)
            ],
            "{}",
            t.label
        );
        assert_eq!(
            shown(core.list_order_attachments(&admin(), 3).await?),
            vec![(File, "video.mp4".to_owned(), true, false)],
            "{}",
            t.label
        );
    }
    Ok(())
}

#[tokio::test]
async fn source_folder_is_untouched_by_full_import() -> Res {
    for t in cores().await {
        let (src, data) = (fixture_dir(V32).await, tempfile::tempdir()?);
        let before = hash_tree(src.path());
        import_folder(&t.core, &admin(), &options(src.path(), data.path(), false)).await?;
        assert_eq!(hash_tree(src.path()), before, "{}", t.label);
    }
    Ok(())
}

#[tokio::test]
async fn sms_token_is_never_stored_and_summary_has_no_personal_data() -> Res {
    for t in cores().await {
        let (src, data) = (fixture_dir(V32).await, tempfile::tempdir()?);
        let r = import_folder(&t.core, &admin(), &options(src.path(), data.path(), false)).await?;
        assert!(!dump_all_tables(&t.core).await.contains(FIXTURE_SMS_TOKEN));
        assert!(!r.full_text().contains(FIXTURE_SMS_TOKEN));
        assert!(!r.to_json().to_string().contains(FIXTURE_SMS_TOKEN));
        let s = r.summary_text();
        for personal in ["ИВАНОВ", "89211234567", "Ленина", "ПЕТРОВ", "Ирина"]
        {
            assert!(!s.contains(personal), "{personal}");
        }
        assert!(
            s.contains("Перенесено") && s.contains("заказов — 11"),
            "{s}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn legacy_users_log_in_after_import() -> Res {
    for t in cores().await {
        let data = tempfile::tempdir()?;
        let src = fixture_dir(V32).await;
        import_folder(&t.core, &admin(), &options(src.path(), data.path(), false)).await?;
        t.core.login("ирина", "1234", "x").await?;
        t.core.login("ирина (2)", "пароль", "x").await?;
        t.core.login("Пётр", "qwerty", "x").await?;
        assert!(matches!(
            t.core.login("Олег", "zzz", "x").await,
            Err(CoreError::InvalidCredentials)
        ));
        assert!(!t.core.is_open_mode().await?);
    }
    Ok(())
}

#[tokio::test]
async fn open_mode_kept_when_old_program_had_no_users() -> Res {
    for t in cores().await {
        let data = tempfile::tempdir()?;
        let src = fixture_dir(V30).await;
        import_folder(&t.core, &admin(), &options(src.path(), data.path(), false)).await?;
        assert!(t.core.is_open_mode().await?);
    }
    Ok(())
}

#[tokio::test]
async fn second_import_needs_replace_and_replace_keeps_a_backup() -> Res {
    let (src, data) = (fixture_dir(V32).await, tempfile::tempdir()?);
    let core = file_sqlite_core(&data.path().join("mw.sqlite")).await;
    let r1 = import_folder(&core, &admin(), &options(src.path(), data.path(), false)).await?;
    assert!(matches!(
        import_folder(&core, &admin(), &options(src.path(), data.path(), false)).await,
        Err(LegacyError::TargetNotEmpty(_))
    ));
    core.create_order(&admin(), sample_new_order()).await?;
    let r2 = import_folder(&core, &admin(), &options(src.path(), data.path(), true)).await?;
    let backup = r2.backup.clone().unwrap();
    assert_eq!(order_count_in_file(&backup.join("mywork.db")).await, 12);
    assert!(backup.join("attachments/orders/1/photo.jpg").exists());
    assert!(data.path().join("attachments/orders/1/photo.jpg").exists());
    assert_eq!(
        (r2.summary.orders, core.import_target_state().await?.orders),
        (11, 11)
    );
    assert_ne!(r1.summary.import_id, r2.summary.import_id);
    Ok(())
}

#[tokio::test]
async fn failed_import_leaves_attachments_untouched() -> Res {
    for t in cores().await {
        let (src, data) = (fixture_dir(V32).await, tempfile::tempdir()?);
        t.core
            .create_client(
                &admin(),
                NewClient {
                    full_name: "Уже есть".into(),
                    ..Default::default()
                },
            )
            .await?;
        assert!(
            import_folder(&t.core, &admin(), &options(src.path(), data.path(), false))
                .await
                .is_err()
        );
        let att = data.path().join("attachments");
        assert!(!att.exists() || std::fs::read_dir(att)?.next().is_none());
    }
    Ok(())
}

#[tokio::test]
async fn large_fixture_imports_on_every_backend() -> Res {
    for t in cores().await {
        let (src, data) = (fixture_dir_with(V32, 3000).await, tempfile::tempdir()?);
        let r = import_folder(&t.core, &admin(), &options(src.path(), data.path(), false)).await?;
        assert_eq!(r.summary.orders, 3011, "{}", t.label);
    }
    Ok(())
}

fn no_staging_left(dir: &std::path::Path) -> bool {
    !dir.exists()
        || std::fs::read_dir(dir).unwrap().all(|e| {
            !e.unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".import")
        })
}

#[tokio::test]
async fn empty_attachment_folders_do_not_block_placing_files() -> Res {
    for t in cores().await {
        let (src, data) = (fixture_dir(V32).await, tempfile::tempdir()?);
        let att = data.path().join("attachments");
        std::fs::create_dir_all(att.join("orders"))?;
        std::fs::create_dir_all(att.join("stock"))?;
        import_folder(&t.core, &admin(), &options(src.path(), data.path(), false)).await?;
        assert!(att.join("orders/1/photo.jpg").exists(), "{}", t.label);
        assert!(att.join("stock/1/abc123.jpg").exists(), "{}", t.label);
        assert!(no_staging_left(&att));
    }
    Ok(())
}

#[tokio::test]
async fn missing_rights_are_refused_before_any_file_is_copied() -> Res {
    for t in cores().await {
        let (src, data) = (fixture_dir(V32).await, tempfile::tempdir()?);
        let mut set = PermissionSet::empty();
        set.insert(Permission::SettingsManage);
        let weak = Actor::for_tests(None, "Тест", set);
        let r = import_folder(&t.core, &weak, &options(src.path(), data.path(), false)).await;
        assert!(
            matches!(
                r,
                Err(LegacyError::Core(CoreError::Forbidden(
                    Permission::UsersManage
                )))
            ),
            "{}",
            t.label
        );
        let att = data.path().join("attachments");
        assert!(!att.exists() || std::fs::read_dir(&att)?.next().is_none());
    }
    Ok(())
}

#[tokio::test]
async fn replace_on_postgres_is_refused_without_staging() -> Res {
    for t in cores().await {
        if t.core.database_kind() != DatabaseKind::Postgres {
            continue;
        }
        let (src, data) = (fixture_dir(V32).await, tempfile::tempdir()?);
        let r = import_folder(&t.core, &admin(), &options(src.path(), data.path(), true)).await;
        assert!(matches!(r, Err(LegacyError::ReplaceUnsupported)));
        assert!(no_staging_left(&data.path().join("attachments")));
        assert!(!data.path().join("attachments").exists());
    }
    Ok(())
}

#[tokio::test]
async fn nothing_is_written_into_the_old_folder() -> Res {
    for t in cores().await {
        let src = fixture_dir(V32).await;
        let before = hash_tree(src.path());
        let mut o = options(src.path(), src.path(), false);
        o.attachments_dir = src.path().join("new/attachments");
        let r = import_folder(&t.core, &admin(), &o).await;
        assert!(
            matches!(r, Err(LegacyError::InsideSource(_))),
            "{}",
            t.label
        );
        let data = tempfile::tempdir()?;
        let mut o = options(src.path(), data.path(), false);
        o.backups_dir = src.path().join("ClientFiles/backups");
        let r = import_folder(&t.core, &admin(), &o).await;
        assert!(
            matches!(r, Err(LegacyError::InsideSource(_))),
            "{}",
            t.label
        );
        assert_eq!(hash_tree(src.path()), before);
        assert!(!src.path().join("new").exists());
    }
    Ok(())
}
