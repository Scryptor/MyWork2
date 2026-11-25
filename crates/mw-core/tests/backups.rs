//! Резервные копии: копия, список, хранение, автокопия, настройки.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use chrono::{TimeZone, Utc};
use common::*;
use mw_core::backups::{BackupKind, BackupSettingsPatch};
use mw_core::{Core, CoreError, FixedClock};
use mw_domain::permission::Permission;
use mw_domain::validation::FieldError;

const PG_REASON: &str = "Для PostgreSQL резервные копии делает pg_dump по расписанию на сервере";

struct T {
    label: &'static str,
    core: Core,
    clock: Arc<FixedClock>,
    /// Папка данных теста.
    data: PathBuf,
    /// Держит временную папку до конца теста.
    _tmp: tempfile::TempDir,
}

impl T {
    fn sqlite(&self) -> bool {
        self.label == "sqlite"
    }

    fn backups(&self) -> PathBuf {
        self.data.join("backups")
    }

    fn at(&self, day: u32, hour: u32) {
        self.clock
            .set(Utc.with_ymd_and_hms(2026, 3, day, hour, 0, 0).unwrap());
    }
}

async fn cores() -> Vec<T> {
    let mut out = Vec::new();
    for (label, db) in mw_db::testing::all_backends().await {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        std::fs::create_dir_all(&data).unwrap();
        // `VACUUM INTO` из базы в памяти файла не пишет: SQLite-ветка идёт по файловой базе.
        let db = if label == "sqlite" {
            let db = mw_db::connect(&mw_db::sqlite_url(&tmp.path().join("live.db")))
                .await
                .unwrap();
            mw_db::migrate(&db).await.unwrap();
            db
        } else {
            db
        };
        let clock = Arc::new(FixedClock::new(
            Utc.with_ymd_and_hms(2026, 3, 10, 12, 0, 0).unwrap(),
        ));
        let core = Core::open(db, clock.clone(), "UTC")
            .await
            .unwrap()
            .with_attachments_dir(data.clone())
            .with_backups();
        out.push(T {
            label,
            core,
            clock,
            data,
            _tmp: tmp,
        });
    }
    out
}

fn put(root: &Path, rel: &str, bytes: &[u8]) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

fn count_files(dir: &Path) -> u64 {
    let mut n = 0;
    for e in std::fs::read_dir(dir).unwrap() {
        let e = e.unwrap();
        if e.file_type().unwrap().is_dir() {
            n += count_files(&e.path());
        } else {
            n += 1;
        }
    }
    n
}

fn manifest(dir: &Path, name: &str, kind: &str, created: &str, source: &str) {
    put(
        dir,
        &format!("{name}/mywork.db"),
        b"SQLite format 3\0 not a real base",
    );
    put(
        dir,
        &format!("{name}/mywork-backup.json"),
        serde_json::json!({
            "format": 1, "kind": kind, "created_at": created, "app_version": "0.0.1",
            "source": source, "database_bytes": 1, "files": 0, "files_bytes": 0
        })
        .to_string()
        .as_bytes(),
    );
}

fn validation(e: CoreError) -> Vec<(String, String)> {
    match e {
        CoreError::Validation(v) => v
            .into_iter()
            .map(|f: FieldError| (f.field, f.code))
            .collect(),
        other => panic!("ожидалась ошибка проверки, получено {other:?}"),
    }
}

fn data_source(t: &T) -> String {
    std::fs::canonicalize(&t.data)
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

/// Ядро PostgreSQL отказывает в копиях.
async fn check_refusal(t: &T) {
    let overview = t.core.backup_overview(&admin()).await.unwrap();
    assert!(!overview.supported, "{}", t.label);
    assert_eq!(overview.reason.as_deref(), Some(PG_REASON));
    match t
        .core
        .create_backup(&admin(), BackupKind::Manual)
        .await
        .unwrap_err()
    {
        CoreError::Unavailable(m) => assert_eq!(m, PG_REASON),
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        t.core.start_backup(&admin()).await,
        Err(CoreError::Unavailable(_))
    ));
    assert!(matches!(
        t.core
            .update_backup_settings(&admin(), BackupSettingsPatch::default())
            .await,
        Err(CoreError::Unavailable(_))
    ));
    assert!(matches!(
        t.core.delete_backup(&admin(), "x").await,
        Err(CoreError::Unavailable(_))
    ));
    assert!(t.core.run_auto_backup_if_due().await.unwrap().is_none());
}

#[tokio::test]
async fn a_copy_has_the_base_and_the_files() {
    for t in cores().await {
        if !t.sqlite() {
            check_refusal(&t).await;
            continue;
        }
        let who = admin();
        let order = t.core.create_order(&who, sample_new_order()).await.unwrap();
        t.core
            .add_order_attachment(&who, order.id, "Акт.PDF", b"%PDF-1.4 test".to_vec())
            .await
            .unwrap();
        put(&t.data, "attachments/drafts/k/x.jpg", b"draft");
        put(&t.data, "attachments/.import-1/y.jpg", b"import");

        let copy = t
            .core
            .create_backup(&who, BackupKind::Manual)
            .await
            .unwrap();
        assert_eq!(copy.name, "20260310-120000");
        assert_eq!(copy.kind, BackupKind::Manual);
        assert_eq!(copy.created_at.to_string(), "2026-03-10 12:00:00");
        assert!(copy.database_bytes > 0);
        assert_eq!(copy.files, count_files(&t.data.join("attachments/orders")));
        assert!(copy.files >= 1);

        let dir = t.backups().join(&copy.name);
        let db = mw_db::connect(&mw_db::sqlite_url(&dir.join("mywork.db")))
            .await
            .unwrap();
        use sea_orm::EntityTrait;
        let orders = mw_db::entity::order::Entity::find().all(&db).await.unwrap();
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].id, order.id);

        assert_eq!(
            count_files(&dir.join("attachments/orders")),
            count_files(&t.data.join("attachments/orders"))
        );
        assert!(!dir.join("attachments/drafts").exists());
        assert!(!dir.join("attachments/.import-1").exists());
        assert!(
            names(&t.backups())
                .iter()
                .all(|n| !n.starts_with(".partial-"))
        );
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(dir.join("mywork-backup.json")).unwrap())
                .unwrap();
        assert_eq!(manifest["kind"], "manual");
        assert_eq!(manifest["created_at"], "2026-03-10T12:00:00");
        assert_eq!(manifest["source"], data_source(&t));
        assert_eq!(count_audit(&t.core, "backup.created").await, 1);
    }
}

#[tokio::test]
async fn names_follow_the_time_and_never_collide() {
    for t in cores().await {
        if !t.sqlite() {
            check_refusal(&t).await;
            continue;
        }
        let a = t
            .core
            .create_backup(&admin(), BackupKind::Manual)
            .await
            .unwrap();
        let b = t
            .core
            .create_backup(&admin(), BackupKind::Manual)
            .await
            .unwrap();
        assert_eq!(a.name, "20260310-120000");
        assert_eq!(b.name, "20260310-120000-2");
        let overview = t.core.backup_overview(&admin()).await.unwrap();
        assert!(overview.supported);
        assert_eq!(overview.reason, None);
        assert_eq!(
            overview
                .copies
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            ["20260310-120000-2", "20260310-120000"]
        );
        assert_eq!(overview.folder, t.backups().to_string_lossy());
        assert_eq!(overview.folder_problem, None);
        assert!(overview.pending.is_none() && overview.last_restore.is_none());
        assert!(overview.legacy.is_none());
    }
}

#[tokio::test]
async fn auto_copy_is_made_once_a_day_and_keeps_the_last_n() {
    for t in cores().await {
        if !t.sqlite() {
            check_refusal(&t).await;
            continue;
        }
        let who = admin();
        t.core
            .update_backup_settings(
                &who,
                BackupSettingsPatch {
                    keep: Some(2),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let dir = t.backups();
        let manual = {
            t.at(1, 9);
            t.core
                .create_backup(&who, BackupKind::Manual)
                .await
                .unwrap()
        };
        std::fs::create_dir_all(dir.join("20200101-000000")).unwrap();
        put(&dir, "20200101-000000/mywork.db", b"import");
        manifest(
            &dir,
            "foreign-auto",
            "auto",
            "2020-01-01T00:00:00",
            "/somewhere/else",
        );

        let mut made = Vec::new();
        for day in 1..=4 {
            t.at(day, 10);
            let first = t.core.run_auto_backup_if_due().await.unwrap();
            made.push(first.expect("автокопия дня").name);
            assert!(
                t.core.run_auto_backup_if_due().await.unwrap().is_none(),
                "второй раз за день не делается"
            );
        }
        let overview = t.core.backup_overview(&who).await.unwrap();
        let own_auto: Vec<_> = overview
            .copies
            .iter()
            .filter(|c| c.kind == BackupKind::Auto && c.name != "foreign-auto")
            .map(|c| c.name.clone())
            .collect();
        assert_eq!(own_auto, [made[3].clone(), made[2].clone()]);
        let present = names(&dir);
        assert!(present.contains(&manual.name));
        assert!(present.contains(&"20200101-000000".to_owned()));
        assert!(present.contains(&"foreign-auto".to_owned()));
        assert!(!present.contains(&made[0]) && !present.contains(&made[1]));
        // Автокопии в журнал не пишутся: там только ручная копия дня 1.
        assert_eq!(count_audit(&t.core, "backup.created").await, 1);

        t.core
            .update_backup_settings(
                &who,
                BackupSettingsPatch {
                    auto: Some(false),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        t.at(5, 10);
        assert!(t.core.run_auto_backup_if_due().await.unwrap().is_none());
    }
}

#[tokio::test]
async fn settings_are_checked_and_kept_in_the_data_folder() {
    for t in cores().await {
        if !t.sqlite() {
            check_refusal(&t).await;
            continue;
        }
        let who = admin();
        let overview = t.core.backup_overview(&who).await.unwrap();
        assert!(overview.settings.auto);
        assert_eq!(overview.settings.keep, 10);
        assert_eq!(overview.settings.folder, None);
        assert!(!overview.settings.legacy_reviewed);

        for keep in [0u16, 101] {
            let e = t
                .core
                .update_backup_settings(
                    &who,
                    BackupSettingsPatch {
                        keep: Some(keep),
                        ..Default::default()
                    },
                )
                .await
                .unwrap_err();
            assert_eq!(validation(e), [("keep".into(), "out_of_range".into())]);
        }
        let folder_error = |folder: String| BackupSettingsPatch {
            folder: Some(Some(folder)),
            ..Default::default()
        };
        let other = tempfile::tempdir().unwrap();
        let cases = [
            ("relative/dir".to_owned(), "folder_relative"),
            (
                other.path().join("missing").to_string_lossy().into_owned(),
                "folder_missing",
            ),
        ];
        for (folder, code) in cases {
            let e = t
                .core
                .update_backup_settings(&who, folder_error(folder))
                .await
                .unwrap_err();
            assert_eq!(validation(e), [("folder".into(), code.into())]);
        }
        std::fs::create_dir_all(t.data.join("x")).unwrap();
        let e = t
            .core
            .update_backup_settings(
                &who,
                folder_error(t.data.join("x").to_string_lossy().into()),
            )
            .await
            .unwrap_err();
        assert_eq!(
            validation(e),
            [("folder".into(), "folder_inside_data".into())]
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let ro = other.path().join("ro");
            std::fs::create_dir(&ro).unwrap();
            std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o555)).unwrap();
            let is_root = std::fs::File::create(ro.join("probe")).is_ok();
            if !is_root {
                let e = t
                    .core
                    .update_backup_settings(&who, folder_error(ro.to_string_lossy().into()))
                    .await
                    .unwrap_err();
                assert_eq!(
                    validation(e),
                    [("folder".into(), "folder_read_only".into())]
                );
            }
            std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        assert_eq!(count_audit(&t.core, "backup.settings").await, 0);
        let own = other.path().join("copies");
        std::fs::create_dir(&own).unwrap();
        let saved = t
            .core
            .update_backup_settings(&who, folder_error(own.to_string_lossy().into()))
            .await
            .unwrap();
        assert_eq!(saved.folder.as_deref(), Some(own.to_str().unwrap()));
        assert!(t.data.join("backup-settings.json").is_file());
        assert_eq!(count_audit(&t.core, "backup.settings").await, 1);

        // Правка без изменений ничего не пишет.
        t.core
            .update_backup_settings(&who, folder_error(own.to_string_lossy().into()))
            .await
            .unwrap();
        t.core
            .update_backup_settings(
                &who,
                BackupSettingsPatch {
                    keep: Some(10),
                    auto: Some(true),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(count_audit(&t.core, "backup.settings").await, 1);

        // Новое ядро на той же папке читает те же значения.
        let db2 = mw_db::testing::sqlite_memory().await;
        let core2 = Core::open(db2, t.clock.clone(), "UTC")
            .await
            .unwrap()
            .with_attachments_dir(t.data.clone())
            .with_backups();
        let again = core2.backup_overview(&who).await.unwrap();
        assert_eq!(
            again.settings.folder.as_deref(),
            Some(own.to_str().unwrap())
        );
        assert_eq!(again.folder, own.to_string_lossy());

        let reset = t
            .core
            .update_backup_settings(
                &who,
                BackupSettingsPatch {
                    folder: Some(None),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(reset.folder, None);
        let overview = t.core.backup_overview(&who).await.unwrap();
        assert_eq!(overview.folder, t.backups().to_string_lossy());
        assert_eq!(count_audit(&t.core, "backup.settings").await, 2);

        let weak = actor_with(&[Permission::OrdersEdit]);
        assert!(matches!(
            t.core
                .update_backup_settings(&weak, BackupSettingsPatch::default())
                .await,
            Err(CoreError::Forbidden(Permission::SettingsManage))
        ));
        assert!(matches!(
            t.core.backup_overview(&weak).await,
            Err(CoreError::Forbidden(Permission::SettingsManage))
        ));

        // Испорченный файл — умолчания.
        std::fs::write(t.data.join("backup-settings.json"), b"{ not json").unwrap();
        let broken = t.core.backup_overview(&who).await.unwrap();
        assert!(broken.settings.auto);
        assert_eq!(broken.settings.keep, 10);
        assert_eq!(broken.settings.folder, None);
    }
}

#[tokio::test]
async fn a_missing_folder_sends_the_auto_copy_to_the_default_one() {
    for t in cores().await {
        if !t.sqlite() {
            check_refusal(&t).await;
            continue;
        }
        let who = admin();
        let other = tempfile::tempdir().unwrap();
        let chosen = other.path().join("usb");
        std::fs::create_dir(&chosen).unwrap();
        t.core
            .update_backup_settings(
                &who,
                BackupSettingsPatch {
                    folder: Some(Some(chosen.to_string_lossy().into())),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        std::fs::remove_dir_all(&chosen).unwrap();

        let overview = t.core.backup_overview(&who).await.unwrap();
        assert_eq!(overview.folder, t.backups().to_string_lossy());
        assert!(
            overview
                .folder_problem
                .as_deref()
                .unwrap()
                .contains(chosen.to_str().unwrap())
        );

        let copy = t.core.run_auto_backup_if_due().await.unwrap().unwrap();
        assert!(t.backups().join(&copy.name).join("mywork.db").is_file());
        let overview = t.core.backup_overview(&who).await.unwrap();
        let job = overview.job.unwrap();
        assert!(job.note.unwrap().contains(chosen.to_str().unwrap()));
        assert_eq!(job.copy.as_deref(), Some(copy.name.as_str()));
        assert!(job.finished_at.is_some());

        let e = t
            .core
            .create_backup(&who, BackupKind::Manual)
            .await
            .unwrap_err();
        assert_eq!(validation(e), [("folder".into(), "folder_missing".into())]);
    }
}

#[tokio::test]
async fn an_interrupted_copy_is_not_listed_and_is_cleaned_up() {
    for t in cores().await {
        if !t.sqlite() {
            check_refusal(&t).await;
            continue;
        }
        let dir = t.backups();
        put(&dir, ".partial-20260309-100000/mywork.db", b"half");
        put(
            &dir,
            ".partial-20260309-100000/attachments/orders/1/a.jpg",
            b"half",
        );
        put(&dir, "20260309-110000/mywork.db", b"no manifest");
        let before = t.core.backup_overview(&admin()).await.unwrap();
        assert!(before.copies.is_empty());

        let copy = t
            .core
            .create_backup(&admin(), BackupKind::Manual)
            .await
            .unwrap();
        let present = names(&dir);
        assert!(!present.iter().any(|n| n.starts_with(".partial-")));
        assert!(present.contains(&"20260309-110000".to_owned()));
        assert!(present.contains(&copy.name));
        let overview = t.core.backup_overview(&admin()).await.unwrap();
        assert_eq!(overview.copies.len(), 1);
    }
}

#[tokio::test]
async fn deleting_a_copy() {
    for t in cores().await {
        if !t.sqlite() {
            check_refusal(&t).await;
            continue;
        }
        let who = admin();
        let copy = t
            .core
            .create_backup(&who, BackupKind::Manual)
            .await
            .unwrap();
        put(&t.data, "x/keep.txt", b"outside");

        for bad in ["nope", "../x", "x", ""] {
            let e = t.core.delete_backup(&who, bad).await.unwrap_err();
            assert_eq!(
                validation(e),
                [("name".into(), "backup_missing".into())],
                "{bad}"
            );
        }
        assert!(t.data.join("x/keep.txt").exists());

        let weak = actor_with(&[Permission::SettingsManage]);
        assert!(matches!(
            t.core.delete_backup(&weak, &copy.name).await,
            Err(CoreError::Forbidden(Permission::UsersManage))
        ));
        assert!(t.backups().join(&copy.name).exists());

        t.core.delete_backup(&who, &copy.name).await.unwrap();
        assert!(!t.backups().join(&copy.name).exists());
        assert_eq!(count_audit(&t.core, "backup.deleted").await, 1);
    }
}

#[tokio::test]
async fn a_manual_copy_runs_in_the_background() {
    for t in cores().await {
        if !t.sqlite() {
            check_refusal(&t).await;
            continue;
        }
        let job = t.core.start_backup(&admin()).await.unwrap();
        assert_eq!(job.kind, BackupKind::Manual);
        assert!(job.finished_at.is_none());
        // `wait_background` отменяет знак остановки и прервал бы копию: сначала ждём её конец.
        for _ in 0..200 {
            let job = t.core.backup_overview(&admin()).await.unwrap().job.unwrap();
            if job.finished_at.is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(t.core.wait_background(Duration::from_secs(10)).await);
        let overview = t.core.backup_overview(&admin()).await.unwrap();
        let job = overview.job.unwrap();
        let name = job.copy.clone().expect("копия готова");
        assert!(job.finished_at.is_some());
        assert_eq!(job.error, None);
        assert!(overview.copies.iter().any(|c| c.name == name));
        assert_eq!(count_audit(&t.core, "backup.created").await, 1);
    }
}

#[tokio::test]
async fn a_second_copy_while_one_runs_is_refused() {
    for t in cores().await {
        if !t.sqlite() {
            check_refusal(&t).await;
            continue;
        }
        let _ = t.core.start_backup(&admin()).await.unwrap();
        match t.core.create_backup(&admin(), BackupKind::Manual).await {
            Err(CoreError::InUse(m)) => assert_eq!(m, "Копия уже делается — дождитесь её"),
            // Первая копия могла уже закончиться: тогда вторая прошла.
            Ok(_) => {}
            Err(other) => panic!("{other:?}"),
        }
        t.core.wait_background(Duration::from_secs(10)).await;
    }
}
