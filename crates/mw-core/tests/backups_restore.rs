//! Восстановление из резервной копии: запрос, отмена, шаги при следующем запуске.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{TimeZone, Utc};
use common::*;
use mw_core::backups::{
    BackupKind, RESTORE_MARKER, RESTORE_RESULT, RESTORE_STAGING, RestoreStop,
    apply_pending_restore, apply_pending_restore_until,
};
use mw_core::{Core, CoreError, FixedClock};
use mw_db::entity::order;
use mw_domain::permission::Permission;
use mw_domain::validation::FieldError;
use sea_orm::{ConnectionTrait, DatabaseConnection, EntityTrait, PaginatorTrait, Statement};

const PG_REASON: &str = "Для PostgreSQL резервные копии делает pg_dump по расписанию на сервере";

fn clock() -> Arc<FixedClock> {
    Arc::new(FixedClock::new(
        Utc.with_ymd_and_hms(2026, 3, 10, 12, 30, 0).unwrap(),
    ))
}

fn noise() -> Vec<u8> {
    (0..4096u32).map(|i| (i * 37 % 251) as u8).collect()
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

/// Все файлы под `dir`, пути относительные.
fn files(dir: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            if e.file_type().unwrap().is_dir() {
                walk(root, &e.path(), out);
            } else {
                out.push(
                    e.path()
                        .strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

/// Ядро над файловой SQLite в `<data>/mywork.db` (или над PostgreSQL) с копиями.
struct Env {
    label: &'static str,
    /// Держит временную папку до конца теста.
    _tmp: tempfile::TempDir,
    data: PathBuf,
    pg: Option<DatabaseConnection>,
}

async fn envs() -> Vec<Env> {
    let mut out = Vec::new();
    for (label, db) in mw_db::testing::all_backends().await {
        let tmp = tempfile::tempdir().unwrap();
        let data = tmp.path().join("data");
        std::fs::create_dir_all(&data).unwrap();
        let pg = (label != "sqlite").then_some(db);
        out.push(Env {
            label,
            _tmp: tmp,
            data,
            pg,
        });
    }
    out
}

impl Env {
    fn sqlite(&self) -> bool {
        self.label == "sqlite"
    }

    fn backups(&self) -> PathBuf {
        self.data.join("backups")
    }

    /// Открывает ядро; SQLite — файл в папке данных (миграции применяются при открытии).
    async fn open(&self) -> (Core, DatabaseConnection) {
        let db = match &self.pg {
            Some(db) => db.clone(),
            None => {
                let db = mw_db::connect(&mw_db::sqlite_url(&self.data.join("mywork.db")))
                    .await
                    .unwrap();
                mw_db::migrate(&db).await.unwrap();
                db
            }
        };
        let core = Core::open(db.clone(), clock(), "UTC")
            .await
            .unwrap()
            .with_attachments_dir(self.data.clone())
            .with_backups();
        (core, db)
    }
}

async fn close(core: Core, db: DatabaseConnection) {
    drop(core);
    db.close().await.unwrap();
}

async fn orders(core: &Core) -> u64 {
    order::Entity::find().count(core.db()).await.unwrap()
}

struct Prepared {
    core: Core,
    db: DatabaseConnection,
    name: String,
    files_a: Vec<String>,
    files_ab: Vec<String>,
}

/// Заказ 1 с вложением A, ручная копия, заказ 2 с вложением B.
async fn prepare(env: &Env) -> Prepared {
    let (core, db) = env.open().await;
    let who = admin();
    let one = core.create_order(&who, sample_new_order()).await.unwrap();
    core.add_order_attachment(&who, one.id, "a.pdf", b"%PDF-1.4 A".to_vec())
        .await
        .unwrap();
    let name = core
        .create_backup(&who, BackupKind::Manual)
        .await
        .unwrap()
        .name;
    let files_a = files(&env.data.join("attachments"));
    let two = core.create_order(&who, sample_new_order()).await.unwrap();
    core.add_order_attachment(&who, two.id, "b.pdf", b"%PDF-1.4 B".to_vec())
        .await
        .unwrap();
    let files_ab = files(&env.data.join("attachments"));
    assert_eq!(files_ab.len(), files_a.len() + 1);
    Prepared {
        core,
        db,
        name,
        files_a,
        files_ab,
    }
}

async fn check_restored(env: &Env, files_a: &[String], files_ab: &[String], safety: &str) {
    let (core, db) = env.open().await;
    assert_eq!(orders(&core).await, 1);
    assert_eq!(files(&env.data.join("attachments")), files_a);
    let dir = env.backups().join(safety);
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("mywork-backup.json")).unwrap()).unwrap();
    assert_eq!(manifest["kind"], "before_restore");
    assert_eq!(manifest["created_at"], "2026-03-10T12:30:00");
    let old = mw_db::connect(&mw_db::sqlite_url(&dir.join("mywork.db")))
        .await
        .unwrap();
    assert_eq!(order::Entity::find().count(&old).await.unwrap(), 2);
    old.close().await.unwrap();
    assert_eq!(files(&dir.join("attachments")), files_ab);
    assert!(!env.data.join(RESTORE_MARKER).exists());
    assert!(!env.data.join(RESTORE_STAGING).exists());
    let overview = core.backup_overview(&admin()).await.unwrap();
    assert!(overview.pending.is_none());
    let last = overview.last_restore.unwrap();
    assert!(last.ok);
    assert_eq!(last.safety.as_deref(), Some(safety));
    assert_eq!(
        overview
            .copies
            .iter()
            .filter(|c| c.kind == BackupKind::BeforeRestore)
            .count(),
        1
    );
    close(core, db).await;
}

async fn refusal(env: &Env) {
    let (core, _db) = env.open().await;
    match core.request_restore(&admin(), "x").await.unwrap_err() {
        CoreError::Unavailable(m) => assert_eq!(m, PG_REASON),
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        core.cancel_restore(&admin()).await,
        Err(CoreError::Unavailable(_))
    ));
}

#[tokio::test]
async fn restore_brings_back_the_base_and_the_files() {
    for env in envs().await {
        if !env.sqlite() {
            refusal(&env).await;
            continue;
        }
        let p = prepare(&env).await;
        let pending = p.core.request_restore(&admin(), &p.name).await.unwrap();
        assert_eq!(pending.copy.name, p.name);
        assert_eq!(pending.requested_by, "Тест");
        assert_eq!(pending.requested_at.to_string(), "2026-03-10 12:30:00");
        let overview = p.core.backup_overview(&admin()).await.unwrap();
        assert_eq!(overview.pending.unwrap().copy.name, p.name);
        assert_eq!(count_audit(&p.core, "backup.restore_requested").await, 1);
        let data = env.data.clone();
        let (name, files_a, files_ab) = (p.name.clone(), p.files_a.clone(), p.files_ab.clone());
        close(p.core, p.db).await;

        let result = apply_pending_restore(&data).await.unwrap().unwrap();
        assert!(result.ok, "{:?}", result.error);
        assert_eq!(result.copy.name, name);
        assert_eq!(result.requested_by, "Тест");
        let safety = result.safety.clone().unwrap();
        // Второй запуск ничего не делает.
        assert!(apply_pending_restore(&data).await.unwrap().is_none());

        check_restored(&env, &files_a, &files_ab, &safety).await;
        let (core, db) = env.open().await;
        core.record_restore(&result).await.unwrap();
        assert_eq!(count_audit(&core, "backup.restored").await, 1);
        let text = audit_json(&core, "backup.restored").await;
        assert!(text.contains("Данные восстановлены из копии от 10.03.2026 12:30"));
        close(core, db).await;
    }
}

#[tokio::test]
async fn an_interrupted_restore_finishes_on_the_next_start() {
    for stop in [RestoreStop::AfterCopied, RestoreStop::AfterMovedAside] {
        for env in envs().await {
            if !env.sqlite() {
                refusal(&env).await;
                continue;
            }
            let p = prepare(&env).await;
            p.core.request_restore(&admin(), &p.name).await.unwrap();
            let data = env.data.clone();
            let (files_a, files_ab) = (p.files_a.clone(), p.files_ab.clone());
            close(p.core, p.db).await;

            assert!(apply_pending_restore_until(&data, stop).await.is_err());
            assert!(data.join(RESTORE_MARKER).exists());
            let result = apply_pending_restore(&data).await.unwrap().unwrap();
            assert!(result.ok, "{:?}", result.error);
            let safety = result.safety.unwrap();
            check_restored(&env, &files_a, &files_ab, &safety).await;
        }
    }
}

#[tokio::test]
async fn a_damaged_or_newer_copy_is_not_restored() {
    for env in envs().await {
        if !env.sqlite() {
            refusal(&env).await;
            continue;
        }
        let p = prepare(&env).await;
        let who = admin();
        let copy_db = env.backups().join(&p.name).join("mywork.db");

        // Испорчена до запроса.
        let good = std::fs::read(&copy_db).unwrap();
        std::fs::write(&copy_db, noise()).unwrap();
        let e = p.core.request_restore(&who, &p.name).await.unwrap_err();
        assert_eq!(
            validation(e),
            [("name".to_owned(), "backup_damaged".to_owned())]
        );
        std::fs::write(&copy_db, &good).unwrap();

        // Нет такой копии.
        let e = p.core.request_restore(&who, "nope").await.unwrap_err();
        assert_eq!(
            validation(e),
            [("name".to_owned(), "backup_missing".to_owned())]
        );

        // Лишняя миграция.
        let newer = p
            .core
            .create_backup(&who, BackupKind::Manual)
            .await
            .unwrap();
        let newer_db = env.backups().join(&newer.name).join("mywork.db");
        let conn = mw_db::connect(&mw_db::sqlite_url(&newer_db)).await.unwrap();
        conn.execute_raw(Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            "INSERT INTO seaql_migrations (version, applied_at) VALUES ('m29991231_000001_future', 1)",
        ))
        .await
        .unwrap();
        conn.close().await.unwrap();
        let e = p.core.request_restore(&who, &newer.name).await.unwrap_err();
        assert_eq!(
            validation(e),
            [("name".to_owned(), "backup_too_new".to_owned())]
        );

        // Описание от более новой версии формата.
        let future = p
            .core
            .create_backup(&who, BackupKind::Manual)
            .await
            .unwrap();
        let manifest_path = env.backups().join(&future.name).join("mywork-backup.json");
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
        manifest["format"] = 2.into();
        std::fs::write(&manifest_path, manifest.to_string()).unwrap();
        let e = p
            .core
            .request_restore(&who, &future.name)
            .await
            .unwrap_err();
        assert_eq!(
            validation(e),
            [("name".to_owned(), "backup_too_new".to_owned())]
        );
        assert!(!env.data.join(RESTORE_MARKER).exists());

        // Исправна при запросе, испорчена до запуска.
        p.core.request_restore(&who, &p.name).await.unwrap();
        let data = env.data.clone();
        close(p.core, p.db).await;
        std::fs::write(&copy_db, noise()).unwrap();
        let result = apply_pending_restore(&data).await.unwrap().unwrap();
        assert!(!result.ok);
        assert_eq!(
            result.error.as_deref(),
            Some("Копия повреждена — выберите другую")
        );
        assert_eq!(result.safety, None);
        assert!(!data.join(RESTORE_MARKER).exists());
        assert!(!data.join(RESTORE_STAGING).exists());
        assert!(data.join(RESTORE_RESULT).exists());

        let (core, db) = env.open().await;
        assert_eq!(orders(&core).await, 2);
        let overview = core.backup_overview(&who).await.unwrap();
        assert!(
            overview
                .copies
                .iter()
                .all(|c| c.kind != BackupKind::BeforeRestore)
        );
        assert!(overview.pending.is_none());
        assert!(!overview.last_restore.unwrap().ok);
        close(core, db).await;
    }
}

#[tokio::test]
async fn a_vanished_copy_is_a_failed_restore() {
    for env in envs().await {
        if !env.sqlite() {
            continue;
        }
        let p = prepare(&env).await;
        p.core.request_restore(&admin(), &p.name).await.unwrap();
        let data = env.data.clone();
        let path = env.backups().join(&p.name);
        close(p.core, p.db).await;
        std::fs::remove_dir_all(&path).unwrap();
        let result = apply_pending_restore(&data).await.unwrap().unwrap();
        assert!(!result.ok);
        let error = result.error.unwrap();
        assert!(error.starts_with("Копии больше нет: "), "{error}");
        assert!(!data.join(RESTORE_MARKER).exists());
        let (core, db) = env.open().await;
        assert_eq!(orders(&core).await, 2);
        close(core, db).await;
    }
}

#[tokio::test]
async fn cancel_and_delete_with_a_pending_restore() {
    for env in envs().await {
        if !env.sqlite() {
            refusal(&env).await;
            continue;
        }
        let p = prepare(&env).await;
        let who = admin();
        p.core.request_restore(&who, &p.name).await.unwrap();
        let e = p.core.delete_backup(&who, &p.name).await.unwrap_err();
        assert_eq!(
            validation(e),
            [("name".to_owned(), "backup_pending".to_owned())]
        );
        std::fs::write(env.data.join(RESTORE_RESULT), b"{}").ok();
        p.core.cancel_restore(&who).await.unwrap();
        let overview = p.core.backup_overview(&who).await.unwrap();
        assert!(overview.pending.is_none());
        assert!(overview.last_restore.is_none());
        assert_eq!(count_audit(&p.core, "backup.restore_cancelled").await, 1);
        // Повторная отмена без отметки ничего не пишет.
        p.core.cancel_restore(&who).await.unwrap();
        assert_eq!(count_audit(&p.core, "backup.restore_cancelled").await, 1);
        p.core.delete_backup(&who, &p.name).await.unwrap();
        close(p.core, p.db).await;
    }
}

#[tokio::test]
async fn unreadable_marker_files_are_treated_as_absent() {
    for env in envs().await {
        if !env.sqlite() {
            continue;
        }
        let (core, db) = env.open().await;
        std::fs::write(env.data.join(RESTORE_MARKER), b"{ not json").unwrap();
        std::fs::write(env.data.join(RESTORE_RESULT), b"[1,").unwrap();
        let overview = core.backup_overview(&admin()).await.unwrap();
        assert!(overview.pending.is_none() && overview.last_restore.is_none());
        let data = env.data.clone();
        close(core, db).await;
        assert!(apply_pending_restore(&data).await.unwrap().is_none());
    }
}

#[tokio::test]
async fn restore_needs_both_rights() {
    for env in envs().await {
        let (core, _db) = env.open().await;
        for who in [
            actor_with(&[Permission::SettingsManage]),
            actor_with(&[Permission::UsersManage]),
        ] {
            let missing = if who.permissions().contains(Permission::UsersManage) {
                Permission::SettingsManage
            } else {
                Permission::UsersManage
            };
            assert!(matches!(
                core.request_restore(&who, "x").await,
                Err(CoreError::Forbidden(p)) if p == missing
            ));
            assert!(matches!(
                core.cancel_restore(&who).await,
                Err(CoreError::Forbidden(p)) if p == missing
            ));
        }
    }
}
