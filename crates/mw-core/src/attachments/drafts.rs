//! Черновики приёма: вложения до создания заказа, их перенос в заказ и очистка.

use std::path::PathBuf;

use chrono::Duration;
use mw_db::entity::attachment;
use mw_domain::permission::Permission;
use mw_domain::validation::FieldError;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set,
};

use super::{Attachment, store, to_api};
use crate::{Actor, Core, CoreError};

/// Больше вложений в одном черновике нельзя.
pub const MAX_DRAFT_ATTACHMENTS: u64 = 10;

/// Ключ черновика или повтора приёма: 1–64 символа `[A-Za-z0-9-]`.
pub(crate) fn valid_key(key: &str) -> bool {
    (1..=64).contains(&key.len()) && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn checked_key(field: &'static str, key: &str) -> Result<(), CoreError> {
    if valid_key(key) {
        Ok(())
    } else {
        Err(CoreError::Validation(vec![FieldError::new(
            field, "invalid",
        )]))
    }
}

fn paths_of(data_dir: &std::path::Path, m: &attachment::Model) -> Vec<PathBuf> {
    [Some(m.path.as_str()), m.thumb_path.as_deref()]
        .into_iter()
        .flatten()
        .filter_map(|rel| store::resolve(data_dir, rel))
        .collect()
}

fn draft_condition(key: &str) -> sea_orm::Condition {
    sea_orm::Condition::all()
        .add(attachment::Column::DraftKey.eq(key))
        .add(attachment::Column::OrderId.is_null())
}

impl Core {
    /// Добавляет фото или файл в черновик приёма.
    pub async fn add_draft_attachment(
        &self,
        actor: &Actor,
        draft_key: &str,
        name: &str,
        bytes: Vec<u8>,
    ) -> Result<Attachment, CoreError> {
        actor.require(Permission::OrdersCreate)?;
        checked_key("draft_key", draft_key)?;
        let data_dir = self.data_dir()?.to_path_buf();
        let too_many = || {
            CoreError::Validation(vec![FieldError::with_message(
                "file",
                "too_many",
                "В черновике уже 10 фото",
            )])
        };
        let count = attachment::Entity::find()
            .filter(draft_condition(draft_key))
            .count(&self.db)
            .await?;
        if count >= MAX_DRAFT_ATTACHMENTS {
            return Err(too_many());
        }
        let written = super::prepare_and_write(
            data_dir,
            format!("attachments/drafts/{draft_key}"),
            name.to_owned(),
            bytes,
        )
        .await?;
        let files = written.files.clone();
        let inserted = async {
            let tx = self.begin_write().await?;
            let count = attachment::Entity::find()
                .filter(draft_condition(draft_key))
                .count(&tx)
                .await?;
            if count >= MAX_DRAFT_ATTACHMENTS {
                return Err(too_many());
            }
            let model = written
                .into_active(None, Some(draft_key.to_owned()), self.now())
                .insert(&tx)
                .await?;
            tx.commit().await?;
            Ok::<_, CoreError>(model)
        }
        .await;
        match inserted {
            Ok(m) => Ok(to_api(m, false)),
            Err(e) => {
                store::remove_files(&files);
                Err(e)
            }
        }
    }

    /// Вложения черновика по порядку добавления.
    pub async fn list_draft_attachments(
        &self,
        actor: &Actor,
        draft_key: &str,
    ) -> Result<Vec<Attachment>, CoreError> {
        actor.require(Permission::OrdersCreate)?;
        checked_key("draft_key", draft_key)?;
        let data_dir = self.data_dir()?;
        let rows = attachment::Entity::find()
            .filter(draft_condition(draft_key))
            .order_by_asc(attachment::Column::Id)
            .all(&self.db)
            .await?;
        Ok(rows
            .into_iter()
            .map(|m| {
                let missing = self.is_missing(data_dir, &m);
                to_api(m, missing)
            })
            .collect())
    }

    /// Выбрасывает черновик: записи и их файлы. Неизвестный ключ — не ошибка.
    ///
    /// Удаляются только файлы удалённых записей: в папке черновика могут лежать файлы заказа,
    /// перенос которых в папку заказа не удался.
    pub async fn discard_draft(&self, actor: &Actor, draft_key: &str) -> Result<(), CoreError> {
        actor.require(Permission::OrdersCreate)?;
        checked_key("draft_key", draft_key)?;
        let data_dir = self.data_dir()?.to_path_buf();
        let tx = self.begin_write().await?;
        let rows = attachment::Entity::find()
            .filter(draft_condition(draft_key))
            .all(&tx)
            .await?;
        if rows.is_empty() {
            tx.rollback().await.ok();
        } else {
            attachment::Entity::delete_many()
                .filter(attachment::Column::Id.is_in(rows.iter().map(|m| m.id)))
                .exec(&tx)
                .await?;
            tx.commit().await?;
        }
        remove_rows_files(&data_dir, &rows);
        Ok(())
    }

    /// Удаляет вложения черновиков старше срока: записи и их файлы. Возвращает их число.
    pub async fn cleanup_drafts(&self, older_than: Duration) -> Result<u64, CoreError> {
        let Some(data_dir) = self.data_dir.clone() else {
            return Ok(0);
        };
        // Срок — настоящие часы (`now_utc`), а `created_at` записан по местным: граница
        // переводится в местное время так же, как `now()`.
        let cutoff = self.local_time(self.now_utc() - older_than);
        let tx = self.begin_write().await?;
        let expired = attachment::Entity::find()
            .filter(attachment::Column::DraftKey.is_not_null())
            .filter(attachment::Column::OrderId.is_null())
            .filter(attachment::Column::CreatedAt.lt(cutoff))
            .all(&tx)
            .await?;
        if expired.is_empty() {
            tx.rollback().await.ok();
            return Ok(0);
        }
        attachment::Entity::delete_many()
            .filter(attachment::Column::Id.is_in(expired.iter().map(|m| m.id)))
            .exec(&tx)
            .await?;
        tx.commit().await?;
        remove_rows_files(&data_dir, &expired);
        Ok(expired.len() as u64)
    }

    /// После фиксации заказа: переносит файлы вложений из черновика в папку заказа.
    /// Неудача не отменяет заказ: вложение остаётся по старому пути, а частично сделанный
    /// перенос откатывается, чтобы путь в базе и файл на диске не расходились.
    ///
    /// Перенос и запись путей идут отдельной задачей: оборванный запрос (клиент ушёл, сервер
    /// бросил обработчик) не остановит их на полпути, когда файлы уже перенесены, а пути — нет.
    pub(crate) async fn move_draft_files(&self, order_id: i64, rows: Vec<attachment::Model>) {
        let core = self.clone();
        // Задача на трекере: остановка сервера ждёт её (`Core::wait_background`) до закрытия БД.
        let task = self
            .background
            .spawn(async move { core.move_draft_files_now(order_id, rows).await });
        if let Err(e) = task.await {
            tracing::warn!(order_id, error = %e, "перенос вложений черновика прерван");
        }
    }

    async fn move_draft_files_now(&self, order_id: i64, rows: Vec<attachment::Model>) {
        let Some(data_dir) = self.data_dir.clone() else {
            return;
        };
        let draft_key = rows.first().and_then(|m| m.draft_key.clone());
        let moved = {
            let data_dir = data_dir.clone();
            tokio::task::spawn_blocking(move || move_files(&data_dir, order_id, rows))
                .await
                .unwrap_or_default()
        };
        if moved.is_empty() {
            return;
        }
        #[cfg(test)]
        tests::pause_before_paths(draft_key.as_deref()).await;
        let result = async {
            #[cfg(test)]
            if tests::take_hook(&tests::FAIL_PATH_UPDATE, draft_key.as_deref()) {
                return Err(CoreError::Internal("тестовый сбой записи путей".into()));
            }
            let tx = self.begin_write().await?;
            for m in &moved {
                attachment::ActiveModel {
                    id: Set(m.id),
                    path: Set(m.new_path.clone()),
                    thumb_path: Set(m.new_thumb.clone()),
                    ..Default::default()
                }
                .update(&tx)
                .await?;
            }
            tx.commit().await?;
            Ok::<(), CoreError>(())
        }
        .await;
        match result {
            Ok(()) => {
                if let Some(key) = &draft_key {
                    // Пустая папка черновика не нужна; непустая (часть не перенесена) остаётся.
                    std::fs::remove_dir(data_dir.join("attachments/drafts").join(key)).ok();
                }
            }
            Err(e) => {
                tracing::warn!(order_id, error = %e, "пути вложений после переноса не записаны");
                for m in moved.iter().rev() {
                    undo(&m.done);
                }
            }
        }
    }
}

/// Перенесённое вложение: новые пути для базы и сделанные переименования для отката.
#[derive(Debug)]
pub(crate) struct MovedAttachment {
    pub id: i64,
    pub new_path: String,
    pub new_thumb: Option<String>,
    pub done: Vec<(PathBuf, PathBuf)>,
}

/// Возвращает файлы на прежние места (в обратном порядке).
fn undo(done: &[(PathBuf, PathBuf)]) {
    for (from, to) in done.iter().rev() {
        if let Some(parent) = from.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        if let Err(e) = std::fs::rename(to, from) {
            tracing::warn!(path = %from.display(), error = %e, "перенос вложения не откатился");
        }
    }
}

/// Переносит файлы вложений в папку заказа. Вложение либо переносится целиком (основной
/// файл и миниатюра, если она есть на диске), либо остаётся на месте.
pub(crate) fn move_files(
    data_dir: &std::path::Path,
    order_id: i64,
    rows: Vec<attachment::Model>,
) -> Vec<MovedAttachment> {
    let mut out = Vec::new();
    for m in rows {
        let Some(file_name) = m.path.rsplit('/').next() else {
            continue;
        };
        let new_rel = format!("attachments/orders/{order_id}/{file_name}");
        let (Some(from), Some(to)) = (
            store::resolve(data_dir, &m.path),
            store::resolve(data_dir, &new_rel),
        ) else {
            continue;
        };
        let mut pairs = vec![(from, to)];
        let mut new_thumb = None;
        if let Some(old) = m
            .thumb_path
            .as_deref()
            .and_then(|r| store::resolve(data_dir, r))
            && old.exists()
        {
            let rel = store::thumb_rel(&new_rel);
            if let Some(new) = store::resolve(data_dir, &rel) {
                pairs.push((old, new));
                new_thumb = Some(rel);
            }
        }
        let mut done: Vec<(PathBuf, PathBuf)> = Vec::new();
        let mut failure = None;
        for (from, to) in pairs {
            let step = (|| -> std::io::Result<()> {
                if let Some(parent) = to.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::rename(&from, &to)
            })();
            match step {
                Ok(()) => done.push((from, to)),
                Err(e) => {
                    failure = Some(e);
                    break;
                }
            }
        }
        if let Some(e) = failure {
            undo(&done);
            tracing::warn!(
                attachment_id = m.id, error = %e, "файл черновика не перенесён в заказ"
            );
            continue;
        }
        out.push(MovedAttachment {
            id: m.id,
            new_path: new_rel,
            new_thumb,
            done,
        });
    }
    out
}

/// Удаляет файлы этих записей черновиков, затем их опустевшие папки. Папка, где остались
/// другие файлы (например, файлы заказа, не перенесённые из черновика), остаётся.
pub(super) fn remove_rows_files(data_dir: &std::path::Path, rows: &[attachment::Model]) {
    let files: Vec<PathBuf> = rows.iter().flat_map(|m| paths_of(data_dir, m)).collect();
    store::remove_files(&files);
    let mut keys: Vec<&str> = rows.iter().filter_map(|m| m.draft_key.as_deref()).collect();
    keys.sort_unstable();
    keys.dedup();
    for key in keys {
        if valid_key(key) {
            std::fs::remove_dir(data_dir.join("attachments/drafts").join(key)).ok();
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use std::sync::{Arc, Mutex};

    use tokio::sync::Notify;

    use super::*;

    /// Запись путей после переноса файлов черновика с этим ключом завершится ошибкой (один раз).
    pub(super) static FAIL_PATH_UPDATE: Mutex<Option<String>> = Mutex::new(None);

    /// Перенос черновика с этим ключом остановится перед записью путей: сообщит `reached` и
    /// подождёт `release` (один раз); меток может быть несколько — по одной на ключ.
    struct Pause {
        key: String,
        reached: Arc<Notify>,
        release: Arc<Notify>,
    }
    static PAUSE_BEFORE_PATHS: Mutex<Vec<Pause>> = Mutex::new(Vec::new());

    /// Снимает метку, если она для этого ключа.
    pub(super) fn take_hook(hook: &Mutex<Option<String>>, key: Option<&str>) -> bool {
        let mut guard = hook.lock().unwrap();
        if key.is_some() && guard.as_deref() == key {
            *guard = None;
            true
        } else {
            false
        }
    }

    pub(super) async fn pause_before_paths(key: Option<&str>) {
        let pause = {
            let mut guard = PAUSE_BEFORE_PATHS.lock().unwrap();
            let at = guard.iter().position(|p| Some(p.key.as_str()) == key);
            at.map(|i| guard.remove(i))
        };
        if let Some(p) = pause {
            p.reached.notify_one();
            p.release.notified().await;
        }
    }

    fn row(id: i64, path: &str, thumb: Option<&str>) -> attachment::Model {
        attachment::Model {
            id,
            order_id: None,
            draft_key: Some("k".into()),
            kind: "photo".into(),
            name: "a.jpg".into(),
            caption: String::new(),
            content_type: "image/jpeg".into(),
            size: 1,
            width: None,
            height: None,
            path: path.into(),
            thumb_path: thumb.map(str::to_owned),
            legacy: false,
            created_at: chrono::NaiveDateTime::default(),
            stock_item_id: None,
        }
    }

    fn put(dir: &std::path::Path, rel: &str) {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"x").unwrap();
    }

    #[test]
    fn failed_thumbnail_move_puts_the_main_file_back() {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), "attachments/drafts/k/a.jpg");
        put(dir.path(), "attachments/drafts/k/a.jpg.thumb.jpg");
        // Целевое имя миниатюры занято непустой папкой: переименование не удастся.
        put(dir.path(), "attachments/orders/7/a.jpg.thumb.jpg/inner");
        let rows = vec![row(
            1,
            "attachments/drafts/k/a.jpg",
            Some("attachments/drafts/k/a.jpg.thumb.jpg"),
        )];
        assert!(move_files(dir.path(), 7, rows).is_empty());
        assert!(dir.path().join("attachments/drafts/k/a.jpg").exists());
        assert!(
            dir.path()
                .join("attachments/drafts/k/a.jpg.thumb.jpg")
                .exists()
        );
        assert!(!dir.path().join("attachments/orders/7/a.jpg").exists());
    }

    #[test]
    fn missing_thumbnail_is_not_recorded_and_undo_restores_files() {
        let dir = tempfile::tempdir().unwrap();
        put(dir.path(), "attachments/drafts/k/a.jpg");
        put(dir.path(), "attachments/drafts/k/b.jpg");
        put(dir.path(), "attachments/drafts/k/b.jpg.thumb.jpg");
        let rows = vec![
            row(
                1,
                "attachments/drafts/k/a.jpg",
                Some("attachments/drafts/k/a.jpg.thumb.jpg"),
            ),
            row(
                2,
                "attachments/drafts/k/b.jpg",
                Some("attachments/drafts/k/b.jpg.thumb.jpg"),
            ),
        ];
        let moved = move_files(dir.path(), 7, rows);
        assert_eq!(moved.len(), 2);
        assert_eq!(moved[0].new_thumb, None);
        assert_eq!(
            moved[1].new_thumb.as_deref(),
            Some("attachments/orders/7/b.jpg.thumb.jpg")
        );
        for m in moved.iter().rev() {
            undo(&m.done);
        }
        for rel in [
            "attachments/drafts/k/a.jpg",
            "attachments/drafts/k/b.jpg",
            "attachments/drafts/k/b.jpg.thumb.jpg",
        ] {
            assert!(dir.path().join(rel).exists(), "{rel}");
        }
    }

    #[tokio::test]
    async fn failed_path_update_moves_files_back_even_if_the_draft_folder_is_gone() {
        use crate::{Actor, ClientRef, Core, FixedClock, NewClient, NewOrder};

        for (label, db) in mw_db::testing::all_backends().await {
            let clock = Arc::new(FixedClock::new(chrono::Utc::now()));
            let dir = tempfile::tempdir().unwrap();
            let core = Core::open(db, clock, "UTC")
                .await
                .unwrap()
                .with_attachments_dir(dir.path().to_path_buf());
            let actor = Actor::unrestricted("Тест");
            let img = image::RgbImage::from_pixel(20, 20, image::Rgb([9, 9, 9]));
            let mut png = std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgb8(img)
                .write_to(&mut png, image::ImageFormat::Png)
                .unwrap();
            for _ in 0..2 {
                core.add_draft_attachment(&actor, "fail", "p.png", png.get_ref().clone())
                    .await
                    .unwrap();
            }
            *FAIL_PATH_UPDATE.lock().unwrap() = Some("fail".into());
            let order = core
                .create_order(
                    &actor,
                    NewOrder {
                        client: ClientRef::New(NewClient {
                            full_name: "Ким".into(),
                            ..Default::default()
                        }),
                        device_type: "Телефон".into(),
                        brand: "А".into(),
                        model: "Б".into(),
                        malfunction: "Не включается".into(),
                        attachment_draft: Some("fail".into()),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
            assert!(FAIL_PATH_UPDATE.lock().unwrap().is_none());
            let list = core.list_order_attachments(&actor, order.id).await.unwrap();
            assert_eq!(list.len(), 2, "{label}");
            for a in &list {
                assert!(!a.missing, "{label}");
                core.attachment_file(&actor, a.id, crate::AttachmentVariant::Thumb)
                    .await
                    .unwrap();
            }
        }
    }

    /// Запрос «Принять» оборван (клиент ушёл, сервер бросил обработчик) после фиксации заказа,
    /// пока файлы переносятся: перенос и запись путей всё равно доходят до конца.
    #[tokio::test]
    async fn dropped_request_does_not_stop_the_move_halfway() {
        use crate::{Actor, AttachmentVariant, ClientRef, Core, FixedClock, NewClient, NewOrder};

        for (label, db) in mw_db::testing::all_backends().await {
            let clock = Arc::new(FixedClock::new(chrono::Utc::now()));
            let dir = tempfile::tempdir().unwrap();
            let core = Core::open(db, clock, "UTC")
                .await
                .unwrap()
                .with_attachments_dir(dir.path().to_path_buf());
            let actor = Actor::unrestricted("Тест");
            let img = image::RgbImage::from_pixel(20, 20, image::Rgb([9, 9, 9]));
            let mut png = std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgb8(img)
                .write_to(&mut png, image::ImageFormat::Png)
                .unwrap();
            let a = core
                .add_draft_attachment(&actor, "drop", "p.png", png.into_inner())
                .await
                .unwrap();
            let reached = Arc::new(Notify::new());
            let release = Arc::new(Notify::new());
            PAUSE_BEFORE_PATHS.lock().unwrap().push(Pause {
                key: "drop".into(),
                reached: reached.clone(),
                release: release.clone(),
            });
            let request = {
                let core = core.clone();
                let actor = actor.clone();
                tokio::spawn(async move {
                    core.create_order(
                        &actor,
                        NewOrder {
                            client: ClientRef::New(NewClient {
                                full_name: "Ким".into(),
                                ..Default::default()
                            }),
                            device_type: "Телефон".into(),
                            brand: "А".into(),
                            model: "Б".into(),
                            malfunction: "Не включается".into(),
                            attachment_draft: Some("drop".into()),
                            ..Default::default()
                        },
                    )
                    .await
                })
            };
            reached.notified().await;
            request.abort();
            assert!(request.await.unwrap_err().is_cancelled(), "{label}");
            release.notify_one();
            let mut readable = false;
            for _ in 0..100 {
                if let Ok(f) = core
                    .attachment_file(&actor, a.id, AttachmentVariant::Full)
                    .await
                    && f.path.starts_with(dir.path().join("attachments/orders"))
                {
                    readable = true;
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
            assert!(readable, "{label}: путь вложения не записан после переноса");
        }
    }

    /// Остановка сервера ждёт перенос файлов: `wait_background` не возвращается, пока перенос
    /// оборванного запроса не дошёл до записи путей, и после него файлы уже на месте.
    #[tokio::test]
    async fn wait_background_waits_for_a_dropped_requests_move() {
        use crate::{Actor, AttachmentVariant, ClientRef, Core, FixedClock, NewClient, NewOrder};

        for (label, db) in mw_db::testing::all_backends().await {
            let clock = Arc::new(FixedClock::new(chrono::Utc::now()));
            let dir = tempfile::tempdir().unwrap();
            let core = Core::open(db, clock, "UTC")
                .await
                .unwrap()
                .with_attachments_dir(dir.path().to_path_buf());
            let actor = Actor::unrestricted("Тест");
            let img = image::RgbImage::from_pixel(20, 20, image::Rgb([9, 9, 9]));
            let mut png = std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgb8(img)
                .write_to(&mut png, image::ImageFormat::Png)
                .unwrap();
            let a = core
                .add_draft_attachment(&actor, "wait", "p.png", png.into_inner())
                .await
                .unwrap();
            let reached = Arc::new(Notify::new());
            let release = Arc::new(Notify::new());
            PAUSE_BEFORE_PATHS.lock().unwrap().push(Pause {
                key: "wait".into(),
                reached: reached.clone(),
                release: release.clone(),
            });
            let request = {
                let core = core.clone();
                let actor = actor.clone();
                tokio::spawn(async move {
                    core.create_order(
                        &actor,
                        NewOrder {
                            client: ClientRef::New(NewClient {
                                full_name: "Ким".into(),
                                ..Default::default()
                            }),
                            device_type: "Телефон".into(),
                            brand: "А".into(),
                            model: "Б".into(),
                            malfunction: "Не включается".into(),
                            attachment_draft: Some("wait".into()),
                            ..Default::default()
                        },
                    )
                    .await
                })
            };
            reached.notified().await;
            request.abort();
            request.await.unwrap_err();
            // Зависший перенос не держит остановку вечно: по сроку — `false` и предупреждение.
            assert!(
                !core
                    .wait_background(std::time::Duration::from_millis(100))
                    .await,
                "{label}"
            );
            let waiter = {
                let core = core.clone();
                tokio::spawn(async move {
                    core.wait_background(std::time::Duration::from_secs(30))
                        .await
                })
            };
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            assert!(
                !waiter.is_finished(),
                "{label}: ожидание не дождалось переноса"
            );
            release.notify_one();
            let finished = tokio::time::timeout(std::time::Duration::from_secs(10), waiter)
                .await
                .unwrap()
                .unwrap();
            assert!(finished, "{label}");
            let f = core
                .attachment_file(&actor, a.id, AttachmentVariant::Full)
                .await
                .unwrap();
            assert!(
                f.path.starts_with(dir.path().join("attachments/orders")),
                "{label}"
            );
        }
    }
}
