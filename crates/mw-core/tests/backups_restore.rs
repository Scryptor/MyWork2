//! Восстановление из резервной копии: запрос, отмена, шаги при следующем запуске.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::{TimeZone, Utc};
use common::*;
use mw_core::backups::{
    BackupKind, BackupSettingsPatch, RESTORE_MARKER, RESTORE_RESULT, RESTORE_STAGING, RestoreStop,
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
        self.open_with(clock()).await
    }

    /// То же со своими часами.
    async fn open_with(&self, clock: Arc<FixedClock>) -> (Core, DatabaseConnection) {
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
        let core = Core::open(db.clone(), clock, "UTC")
            .await
            .unwrap()
            .with_attachments_dir(self.data.clone())
            .with_backups();
        (core, db)
    }
}

/// Имя страховочной копии из шага `.restore/phase.json`.
fn safety_of_phase(data: &Path) -> String {
    let phase: serde_json::Value = serde_json::from_slice(
        &std::fs::read(data.join(RESTORE_STAGING).join("phase.json")).unwrap(),
    )
    .unwrap();
    phase["safety"].as_str().unwrap().to_owned()
}

async fn orders_in(base: &Path) -> u64 {
    let db = mw_db::connect(&mw_db::sqlite_url(base)).await.unwrap();
    let n = order::Entity::find().count(&db).await.unwrap();
    db.close().await.unwrap();
    n
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
    let before = files(&dir);
    assert!(before.contains(&"mywork.db".to_owned()));
    assert!(
        before
            .iter()
            .all(|f| !f.ends_with("-wal") && !f.ends_with("-shm")),
        "{before:?}"
    );
    let check = mw_db::backup::inspect_copy(&dir.join("mywork.db"))
        .await
        .unwrap();
    assert!(check.intact && check.unknown_migrations.is_empty());
    assert_eq!(files(&dir), before);
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
        let result = mw_core::backups::BackupRestoreResult {
            ok: false,
            copy: p.core.backup_overview(&who).await.unwrap().copies[0].clone(),
            requested_at: p.core.backup_overview(&who).await.unwrap().copies[0].created_at,
            requested_by: "Тест".to_owned(),
            safety: None,
            error: Some("x".to_owned()),
        };
        std::fs::write(
            env.data.join(RESTORE_RESULT),
            serde_json::to_vec(&result).unwrap(),
        )
        .unwrap();
        assert!(
            p.core
                .backup_overview(&who)
                .await
                .unwrap()
                .last_restore
                .is_some()
        );
        std::fs::create_dir_all(env.data.join(RESTORE_STAGING)).unwrap();
        std::fs::write(env.data.join(RESTORE_STAGING).join("x"), b"x").unwrap();
        p.core.cancel_restore(&who).await.unwrap();
        let overview = p.core.backup_overview(&who).await.unwrap();
        assert!(overview.pending.is_none());
        assert!(overview.last_restore.is_none());
        assert!(!env.data.join(RESTORE_RESULT).exists());
        assert!(!env.data.join(RESTORE_STAGING).exists());
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

/// Копия B с тремя заказами: проверка по содержимому, что восстановилась именно она.
async fn make_copy_of_three(core: &Core) -> String {
    let who = admin();
    core.create_order(&who, sample_new_order()).await.unwrap();
    let n = orders(core).await;
    assert_eq!(n, 3, "в базе должно быть три заказа");
    core.create_backup(&who, BackupKind::Manual)
        .await
        .unwrap()
        .name
}

#[tokio::test]
async fn a_stale_staging_never_decides_a_later_restore() {
    for env in envs().await {
        if !env.sqlite() {
            continue;
        }
        let who = admin();
        // (a) Остановка после `copied`, отмена и запрос другой копии.
        let p = prepare(&env).await;
        p.core.request_restore(&who, &p.name).await.unwrap();
        let data = env.data.clone();
        close(p.core, p.db).await;
        assert!(
            apply_pending_restore_until(&data, RestoreStop::AfterCopied)
                .await
                .is_err()
        );
        assert!(data.join(RESTORE_STAGING).exists());
        let (core, db) = env.open().await;
        core.cancel_restore(&who).await.unwrap();
        assert!(!data.join(RESTORE_STAGING).exists());
        let b = make_copy_of_three(&core).await;
        // Остаток снова на месте: отметка его не принимает.
        core.request_restore(&who, &b).await.unwrap();
        close(core, db).await;
        let result = apply_pending_restore(&data).await.unwrap().unwrap();
        assert!(result.ok);
        assert_eq!(result.copy.name, b);
        let (core, db) = env.open().await;
        assert_eq!(orders(&core).await, 3);
        close(core, db).await;
    }
    for env in envs().await {
        if !env.sqlite() {
            continue;
        }
        let who = admin();
        // (b) Шаг `moved_aside` от законченного восстановления и новый запрос.
        let p = prepare(&env).await;
        p.core.request_restore(&who, &p.name).await.unwrap();
        let data = env.data.clone();
        close(p.core, p.db).await;
        assert!(
            apply_pending_restore_until(&data, RestoreStop::AfterMovedAside)
                .await
                .is_err()
        );
        let stale = std::fs::read(data.join(RESTORE_STAGING).join("phase.json")).unwrap();
        assert!(apply_pending_restore(&data).await.unwrap().unwrap().ok);
        let (core, db) = env.open().await;
        assert_eq!(orders(&core).await, 1);
        let b = make_copy_of_three_from_one(&core).await;
        core.request_restore(&who, &b).await.unwrap();
        close(core, db).await;
        std::fs::create_dir_all(data.join(RESTORE_STAGING)).unwrap();
        std::fs::write(data.join(RESTORE_STAGING).join("phase.json"), stale).unwrap();
        let result = apply_pending_restore(&data).await.unwrap().unwrap();
        assert!(result.ok);
        assert_eq!(result.copy.name, b);
        let (core, db) = env.open().await;
        assert_eq!(orders(&core).await, 3);
        close(core, db).await;
    }
}

async fn make_copy_of_three_from_one(core: &Core) -> String {
    let who = admin();
    core.create_order(&who, sample_new_order()).await.unwrap();
    make_copy_of_three(core).await
}

/// Шаг 2 перенёс базу и упал на вложениях (на Windows папку держит Проводник), потом в папке данных
/// появилась новая база: следующий запуск отказывает и страховочную базу не затирает.
#[tokio::test]
async fn a_new_base_after_a_broken_step_two_never_replaces_the_safety_copy() {
    for env in envs().await {
        if !env.sqlite() {
            refusal(&env).await;
            continue;
        }
        let p = prepare(&env).await;
        p.core.request_restore(&admin(), &p.name).await.unwrap();
        let data = env.data.clone();
        let files_ab = p.files_ab.clone();
        close(p.core, p.db).await;
        assert!(
            apply_pending_restore_until(&data, RestoreStop::AfterCopied)
                .await
                .is_err()
        );
        let safety = env.backups().join(safety_of_phase(&data));
        std::fs::create_dir_all(&safety).unwrap();
        for name in ["mywork.db", "mywork.db-wal", "mywork.db-shm"] {
            if data.join(name).exists() {
                std::fs::rename(data.join(name), safety.join(name)).unwrap();
            }
        }
        // Подключение к папке данных создаёт пустую базу.
        let (core, db) = env.open().await;
        assert_eq!(orders(&core).await, 0);
        close(core, db).await;

        let e = apply_pending_restore(&data).await.unwrap_err();
        let text = e.to_string();
        assert!(text.contains("страховочная копия цела"), "{text}");
        assert_eq!(orders_in(&safety.join("mywork.db")).await, 2);
        assert_eq!(orders_in(&data.join("mywork.db")).await, 0);
        assert_eq!(files(&data.join("attachments")), files_ab);
        assert!(!safety.join("attachments").exists());
        assert!(data.join(RESTORE_MARKER).exists());
    }
}

/// После шага 3 базы на месте нет (её не оказалось в `.restore/`): это ошибка, а не успех — иначе
/// подключение создало бы пустую базу.
#[tokio::test]
async fn a_restore_without_the_base_in_place_is_not_a_success() {
    for env in envs().await {
        if !env.sqlite() {
            refusal(&env).await;
            continue;
        }
        let p = prepare(&env).await;
        p.core.request_restore(&admin(), &p.name).await.unwrap();
        let data = env.data.clone();
        close(p.core, p.db).await;
        assert!(
            apply_pending_restore_until(&data, RestoreStop::AfterMovedAside)
                .await
                .is_err()
        );
        std::fs::remove_file(data.join(RESTORE_STAGING).join("mywork.db")).unwrap();
        let e = apply_pending_restore(&data).await.unwrap_err();
        let text = e.to_string();
        assert!(text.contains("Восстановление не закончено"), "{text}");
        assert!(!data.join("mywork.db").exists());
        assert!(data.join(RESTORE_MARKER).exists());
        assert!(!data.join(RESTORE_RESULT).exists());
    }
}

/// Своя папка копий: страховочная копия ложится в папку по умолчанию, а список её всё равно показывает (общий
/// порядок, новые сверху) — её можно восстановить и удалить. Прочие копии папки по умолчанию не видны; при
/// одинаковом имени берётся копия выбранной папки.
#[tokio::test]
async fn the_safety_copy_is_listed_with_a_folder_of_own() {
    for env in envs().await {
        if !env.sqlite() {
            refusal(&env).await;
            continue;
        }
        let who = admin();
        let (core, db) = env.open().await;
        core.create_order(&who, sample_new_order()).await.unwrap();
        let in_default = core
            .create_backup(&who, BackupKind::Manual)
            .await
            .unwrap()
            .name;
        let own = tempfile::tempdir().unwrap();
        core.update_backup_settings(
            &who,
            BackupSettingsPatch {
                folder: Some(Some(own.path().to_string_lossy().into_owned())),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let copy = core
            .create_backup(&who, BackupKind::Manual)
            .await
            .unwrap()
            .name;
        assert_eq!(copy, in_default, "имена в разных папках совпадают");
        core.create_order(&who, sample_new_order()).await.unwrap();
        close(core, db).await;

        // Восстановление запрошено минутой позже копии.
        let later = Arc::new(FixedClock::new(
            Utc.with_ymd_and_hms(2026, 3, 10, 12, 31, 0).unwrap(),
        ));
        let (core, db) = env.open_with(later.clone()).await;
        core.request_restore(&who, &copy).await.unwrap();
        close(core, db).await;
        let result = apply_pending_restore(&env.data).await.unwrap().unwrap();
        assert!(result.ok, "{:?}", result.error);
        let safety = result.safety.unwrap();
        assert!(env.backups().join(&safety).is_dir());

        let (core, db) = env.open_with(later).await;
        assert_eq!(orders(&core).await, 1);
        let overview = core.backup_overview(&who).await.unwrap();
        assert_eq!(overview.folder, own.path().to_string_lossy());
        let listed: Vec<_> = overview
            .copies
            .iter()
            .map(|c| (c.name.clone(), c.kind))
            .collect();
        assert_eq!(
            listed,
            [
                (safety.clone(), BackupKind::BeforeRestore),
                (copy.clone(), BackupKind::Manual)
            ]
        );

        let pending = core.request_restore(&who, &safety).await.unwrap();
        assert_eq!(pending.copy.name, safety);
        let e = core.delete_backup(&who, &safety).await.unwrap_err();
        assert_eq!(
            validation(e),
            [("name".to_owned(), "backup_pending".to_owned())]
        );
        core.cancel_restore(&who).await.unwrap();
        core.delete_backup(&who, &safety).await.unwrap();
        assert!(!env.backups().join(&safety).exists());

        // Одинаковые имена: в списке и для удаления — копия выбранной папки.
        let manifest_path = env.backups().join(&in_default).join("mywork-backup.json");
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&manifest_path).unwrap()).unwrap();
        manifest["kind"] = "before_restore".into();
        std::fs::write(&manifest_path, manifest.to_string()).unwrap();
        let listed: Vec<_> = core
            .backup_overview(&who)
            .await
            .unwrap()
            .copies
            .into_iter()
            .map(|c| (c.name, c.kind))
            .collect();
        assert_eq!(listed, [(copy.clone(), BackupKind::Manual)]);
        core.delete_backup(&who, &copy).await.unwrap();
        assert!(!own.path().join(&copy).exists());
        assert!(env.backups().join(&in_default).exists());
        close(core, db).await;
    }
}

/// Новый запрос восстановления стирает итог прошлого: иначе на экране обе выноски, и «Скрыть» итога
/// молча отменило бы новый запрос.
#[tokio::test]
async fn a_new_request_clears_the_last_result() {
    for env in envs().await {
        if !env.sqlite() {
            refusal(&env).await;
            continue;
        }
        let p = prepare(&env).await;
        let who = admin();
        p.core.request_restore(&who, &p.name).await.unwrap();
        let data = env.data.clone();
        let name = p.name.clone();
        close(p.core, p.db).await;
        assert!(apply_pending_restore(&data).await.unwrap().unwrap().ok);
        let result = std::fs::read(data.join(RESTORE_RESULT)).unwrap();

        let (core, db) = env.open().await;
        assert!(
            core.backup_overview(&who)
                .await
                .unwrap()
                .last_restore
                .is_some()
        );
        core.request_restore(&who, &name).await.unwrap();
        let overview = core.backup_overview(&who).await.unwrap();
        assert!(overview.pending.is_some());
        assert!(overview.last_restore.is_none());
        assert!(!data.join(RESTORE_RESULT).exists());
        close(core, db).await;

        // Запрос командой — так же.
        std::fs::remove_file(data.join(RESTORE_MARKER)).unwrap();
        std::fs::write(data.join(RESTORE_RESULT), &result).unwrap();
        mw_core::backups::schedule_restore(
            &data,
            &env.backups().join(&name),
            "2026-03-10T12:30:00".parse().unwrap(),
            "Тест",
        )
        .await
        .unwrap();
        assert!(data.join(RESTORE_MARKER).exists());
        assert!(!data.join(RESTORE_RESULT).exists());
    }
}
