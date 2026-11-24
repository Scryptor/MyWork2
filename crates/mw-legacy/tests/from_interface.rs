//! Импорт из интерфейса: проверка папки, ход по этапам, отчёты прошлых импортов, уборка промежуточных папок.
#![allow(clippy::unwrap_used)]
mod common;

use std::path::Path;
use std::sync::{Arc, Mutex};

use common::*;
use mw_core::{Actor, AttachmentVariant, Core, NewClient};
use mw_legacy::fixture::FixtureVersion::V32;
use mw_legacy::{
    ImportOptions, ImportStage, LegacyError, REPORT_GROUP_ENTRIES, Severity, StartedFrom,
    WarningCode, check_folder, finish_interrupted_imports, import_error_message, import_folder,
    import_item, report_view, write_report_files,
};
use tokio_util::sync::CancellationToken;

type Res = Result<(), Box<dyn std::error::Error>>;

#[tokio::test]
async fn check_folder_counts_match_the_import() -> Res {
    for t in cores().await {
        let (src, outer) = (fixture_dir(V32).await, tempfile::tempdir()?);
        let data = outer.path().join("MyWork3");
        let before = hash_tree(src.path());
        let c = check_folder(src.path(), &data, false).await;
        assert!(c.ok && c.problem.is_none(), "{c:?}");
        assert_eq!(c.schema_version.as_deref(), Some("колонок в заказах — 32"));
        assert_eq!(
            hash_tree(src.path()),
            before,
            "проверка не меняет папку MyWork 2"
        );
        assert!(!data.exists(), "проверка ничего не создаёт в папке данных");

        let r = import_folder(&t.core, &admin(), &options(src.path(), &data, false)).await?;
        assert_eq!(
            (c.orders, c.clients, c.files, c.files_bytes),
            (
                r.summary.orders,
                r.summary.clients,
                r.files.copied,
                r.files.bytes
            ),
            "{}",
            t.label
        );
        // Файлы клиентов (пять) и найденные фото склада: у позиции 1 — одно, у позиции 3 — три.
        assert_eq!((c.orders, c.files), (11, 9));
    }
    Ok(())
}

#[tokio::test]
async fn check_folder_explains_what_is_wrong() {
    let problem = |c: mw_legacy::FolderCheck| {
        assert!(!c.ok, "{c:?}");
        assert_eq!(
            (
                c.orders,
                c.clients,
                c.files,
                c.files_bytes,
                c.schema_version
            ),
            (0, 0, 0, 0, None)
        );
        c.problem.unwrap()
    };
    let empty = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let text = problem(check_folder(empty.path(), &data.path().join("d"), false).await);
    assert!(text.contains("В папке нет baza.sqlite"), "{text}");

    let src = fixture_dir(V32).await;
    let inside = src.path().join("MyWork3");
    let text = problem(check_folder(src.path(), &inside, false).await);
    assert!(
        text.contains("Папка данных MyWork 3 лежит внутри папки MyWork 2"),
        "{text}"
    );
    assert!(!inside.exists());

    let text = problem(check_folder(data.path(), data.path(), false).await);
    assert!(text.contains("Выбрана папка данных MyWork 3"), "{text}");

    let text = problem(check_folder(&empty.path().join("нет такой"), data.path(), false).await);
    assert!(text.contains("Папка не найдена"), "{text}");

    let text = problem(check_folder(Path::new("MyWork2"), data.path(), false).await);
    assert!(text.contains("Укажите полный путь"), "{text}");

    std::fs::write(empty.path().join("baza.sqlite"), b"not a database").unwrap();
    problem(check_folder(empty.path(), data.path(), false).await);
}

#[tokio::test]
async fn progress_reports_every_stage_in_order_and_done_last() -> Res {
    use ImportStage::*;
    for t in cores().await {
        let (src, data) = (fixture_dir(V32).await, tempfile::tempdir()?);
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        let mut o = options(src.path(), data.path(), false);
        o.progress = Some(Arc::new(move |stage, percent| {
            sink.lock().unwrap().push((stage, percent));
        }));
        import_folder(&t.core, &admin(), &o).await?;
        let seen = seen.lock().unwrap().clone();
        let mut stages: Vec<ImportStage> = seen.iter().map(|(s, _)| *s).collect();
        stages.dedup();
        assert_eq!(
            stages,
            [Reading, Converting, Copying, Writing, Placing, Done],
            "{}",
            t.label
        );
        assert_eq!(seen.last(), Some(&(Done, 100)));
        assert!(seen.iter().all(|(_, p)| *p <= 100));
        for pair in seen.windows(2) {
            if pair[0].0 == pair[1].0 {
                assert!(pair[0].1 <= pair[1].1, "{seen:?}");
            }
        }
        // Копирование показывает ход по файлам, а не только начало и конец.
        assert!(seen.iter().filter(|(s, _)| *s == Copying).count() > 2);

        // Ошибка: этапа «готово» нет.
        let failed = Arc::new(Mutex::new(Vec::new()));
        let sink = failed.clone();
        let other = tempfile::tempdir()?;
        let mut o = options(src.path(), other.path(), false);
        o.progress = Some(Arc::new(move |stage, percent| {
            sink.lock().unwrap().push((stage, percent));
        }));
        assert!(import_folder(&t.core, &admin(), &o).await.is_err());
        assert!(!failed.lock().unwrap().iter().any(|(s, _)| *s == Done));
    }
    Ok(())
}

#[tokio::test]
async fn stored_report_gives_the_list_item_groups_and_the_command_text() -> Res {
    for t in cores().await {
        let (src, data) = (fixture_dir(V32).await, tempfile::tempdir()?);
        let r = import_folder(&t.core, &admin(), &options(src.path(), data.path(), false)).await?;
        let (text_file, _) = write_report_files(data.path(), t.core.now(), &r)?;
        let records = t.core.list_legacy_imports(&admin()).await?;
        assert_eq!(records.len(), 1, "{}", t.label);

        let item = import_item(&records[0]);
        assert_eq!(
            (item.id, item.orders, item.warnings),
            (r.summary.import_id, 11, 33),
            "{}",
            t.label
        );

        let view = report_view(&records[0], Some(&data.path().join("attachments")));
        assert_eq!(
            (view.orders, view.clients, view.users, view.stock_items),
            (
                r.summary.orders,
                r.summary.clients,
                r.summary.users,
                r.summary.stock_items
            )
        );
        assert_eq!(view.stock_items, 4);
        assert_eq!(view.files, r.files);
        assert_eq!(view.warnings_text, std::fs::read_to_string(&text_file)?);
        let total: u64 = view.groups.iter().map(|g| g.count).sum();
        assert_eq!(total, r.warnings.len() as u64);
        // Сначала «требуют внимания», потом сведения; порядок — как в таблице кодов.
        let order: Vec<usize> = view
            .groups
            .iter()
            .map(|g| WarningCode::ALL.iter().position(|c| *c == g.code).unwrap())
            .collect();
        assert!(order.windows(2).all(|p| p[0] < p[1]), "{order:?}");
        let dates = view
            .groups
            .iter()
            .find(|g| g.code == WarningCode::DateUnparsed)
            .unwrap();
        assert_eq!(
            (dates.title.as_str(), dates.severity),
            ("Не разобрана дата", Severity::Warning)
        );
        assert!(!dates.order_ids.is_empty());
        // Даты истории — не заказы: они в записях группы рядом с номерами.
        let sources: Vec<&str> = dates.entries.iter().map(|e| e.source.as_str()).collect();
        assert_eq!(sources, ["StatesMap", "HistoryBD"]);
        assert_eq!(dates.entries_count, 2);
        for group in &view.groups {
            assert!(group.entries_count <= group.count, "{:?}", group.code);
            assert_eq!(
                group.entries.len() as u64,
                group.entries_count.min(REPORT_GROUP_ENTRIES as u64),
                "{:?}",
                group.code
            );
        }
        // Номера в группах — настоящие заказы этого импорта.
        for group in &view.groups {
            for id in &group.order_ids {
                t.core.get_order(&admin(), *id).await?;
            }
        }
        let next = view
            .groups
            .iter()
            .find(|g| g.code == WarningCode::NextNumber)
            .unwrap();
        assert!(
            next.order_ids.is_empty(),
            "ключ «Catalog» — не номер заказа"
        );
    }
    Ok(())
}

/// Промежуточные папки `.import-*` в папке вложений.
fn staging_dirs(att: &Path) -> Vec<std::path::PathBuf> {
    let Ok(entries) = std::fs::read_dir(att) else {
        return Vec::new();
    };
    let mut dirs: Vec<_> = entries
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .filter(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(".import-")
        })
        .collect();
    dirs.sort();
    dirs
}

/// Импорт «падает» (паника, как обрыв процесса) в начале этапа `stage`.
async fn crash_at(core: &Core, mut options: ImportOptions, stage: ImportStage) {
    options.progress = Some(Arc::new(move |s, _| {
        if s == stage {
            panic!("проверка: обрыв импорта на этапе {s:?}");
        }
    }));
    let core = core.clone();
    let r = tokio::spawn(async move { import_folder(&core, &admin(), &options).await }).await;
    assert!(r.unwrap_err().is_panic());
}

#[tokio::test]
async fn abandoned_staging_is_removed_at_start() -> Res {
    for t in cores().await {
        let att = tempfile::tempdir()?;
        let abandoned = att.path().join(".import-20261111-090000");
        std::fs::create_dir_all(abandoned.join("orders/1"))?;
        std::fs::write(abandoned.join("orders/1/a.jpg"), b"x")?;
        std::fs::create_dir_all(att.path().join("orders/2"))?;
        std::fs::write(att.path().join("orders/2/b.jpg"), b"y")?;
        std::fs::write(att.path().join(".import-file"), b"z")?;
        let cleanup = finish_interrupted_imports(&t.core, att.path()).await?;
        assert_eq!(
            cleanup.removed,
            std::slice::from_ref(&abandoned),
            "{}",
            t.label
        );
        assert!(cleanup.kept.is_empty() && cleanup.placed.is_empty());
        assert!(!abandoned.exists());
        assert!(att.path().join("orders/2/b.jpg").exists());
        assert!(att.path().join(".import-file").exists());
        let none = finish_interrupted_imports(&t.core, &att.path().join("missing")).await?;
        assert_eq!(none, mw_legacy::StagingCleanup::default());
    }
    Ok(())
}

#[tokio::test]
async fn import_interrupted_after_the_commit_is_placed_at_start() -> Res {
    for t in cores().await {
        let (src, data) = (fixture_dir(V32).await, tempfile::tempdir()?);
        let core = t
            .core
            .clone()
            .with_attachments_dir(data.path().to_path_buf());
        let att = data.path().join("attachments");
        crash_at(
            &core,
            options(src.path(), data.path(), false),
            ImportStage::Placing,
        )
        .await;
        let staging = staging_dirs(&att);
        assert_eq!(staging.len(), 1, "{}", t.label);
        assert!(!att.join("orders").exists());
        let cleanup = finish_interrupted_imports(&core, &att).await?;
        assert_eq!(cleanup.placed, staging, "{}", t.label);
        assert!(staging_dirs(&att).is_empty());
        assert!(att.join("orders/1/photo.jpg").exists() && att.join("stock/1/abc123.jpg").exists());
        let files = core.list_order_attachments(&admin(), 1).await?;
        assert!(
            !files.is_empty() && files.iter().all(|a| !a.missing),
            "{}",
            t.label
        );
        for a in files {
            let file = core
                .attachment_file(&admin(), a.id, AttachmentVariant::Full)
                .await?;
            assert!(file.path.exists());
        }
    }
    Ok(())
}

#[tokio::test]
async fn interrupted_import_waits_while_its_place_is_taken() -> Res {
    for t in cores().await {
        let (src, data) = (fixture_dir(V32).await, tempfile::tempdir()?);
        let att = data.path().join("attachments");
        crash_at(
            &t.core,
            options(src.path(), data.path(), false),
            ImportStage::Placing,
        )
        .await;
        std::fs::create_dir_all(att.join("orders/9"))?;
        std::fs::write(att.join("orders/9/other.jpg"), b"x")?;
        let staging = staging_dirs(&att);
        let cleanup = finish_interrupted_imports(&t.core, &att).await?;
        assert_eq!(cleanup.kept, staging, "{}", t.label);
        assert!(cleanup.placed.is_empty() && cleanup.removed.is_empty());
        assert!(staging[0].join("orders/1/photo.jpg").exists());
        assert!(staging[0].join("stock/1/abc123.jpg").exists() && !att.join("stock").exists());
        assert!(att.join("orders/9/other.jpg").exists());
    }
    Ok(())
}

// Новый импорт поверх прерванного возможен только заменой — только в SQLite.
#[tokio::test]
async fn staging_of_a_superseded_import_is_removed_at_start() -> Res {
    let (src, data) = (fixture_dir(V32).await, tempfile::tempdir()?);
    let att = data.path().join("attachments");
    let core = file_sqlite_core(&data.path().join("mw.sqlite")).await;
    crash_at(
        &core,
        options(src.path(), data.path(), false),
        ImportStage::Placing,
    )
    .await;
    let crashed = staging_dirs(&att);
    assert_eq!(crashed.len(), 1);
    let r = import_folder(&core, &admin(), &options(src.path(), data.path(), true)).await?;
    assert_eq!(
        staging_dirs(&att),
        crashed,
        "новый импорт убрал только свою папку"
    );
    let cleanup = finish_interrupted_imports(&core, &att).await?;
    assert_eq!(cleanup.removed, crashed);
    assert!(staging_dirs(&att).is_empty());
    assert!(att.join("orders/1/photo.jpg").exists());
    assert_eq!(
        core.list_legacy_imports(&admin()).await?[0].id,
        r.summary.import_id
    );
    Ok(())
}

#[tokio::test]
async fn cancel_while_copying_leaves_nothing_behind() -> Res {
    for t in cores().await {
        let (src, data) = (fixture_dir(V32).await, tempfile::tempdir()?);
        let att = data.path().join("attachments");
        let token = CancellationToken::new();
        let mut o = options(src.path(), data.path(), false);
        o.cancel = Some(token.clone());
        o.progress = Some(Arc::new(move |stage, percent| {
            if stage == ImportStage::Copying && percent > 0 {
                token.cancel();
            }
        }));
        let r = import_folder(&t.core, &admin(), &o).await;
        assert!(
            matches!(r, Err(LegacyError::Cancelled)),
            "{}: {r:?}",
            t.label
        );
        assert_eq!(
            import_error_message(&r.unwrap_err(), StartedFrom::Interface),
            "Импорт прерван: программа закрывается"
        );
        assert!(t.core.import_target_state().await?.is_empty());
        assert!(staging_dirs(&att).is_empty() && !att.join("orders").exists());
    }
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn staging_of_an_import_written_to_the_database_survives_cleanup() -> Res {
    for t in cores().await {
        let (src, data) = (fixture_dir(V32).await, tempfile::tempdir()?);
        let att = data.path().join("attachments");
        std::fs::create_dir_all(&att)?;
        // Висячая ссылка: базу записать можно, а поставить файлы на место — нет.
        std::os::unix::fs::symlink(data.path().join("nowhere"), att.join("orders"))?;
        let r = import_folder(&t.core, &admin(), &options(src.path(), data.path(), false)).await;
        let Err(e @ LegacyError::FilesNotPlaced { .. }) = r else {
            panic!("{}: {r:?}", t.label);
        };
        // Интерфейс не предлагает переносить файлы руками: перезапуск доустановит их сам.
        assert_eq!(
            import_error_message(&e, StartedFrom::Interface),
            "Файлы не удалось поставить на место — перезапустите программу, она закончит перенос сама"
        );
        let LegacyError::FilesNotPlaced { staging, .. } = e else {
            unreachable!()
        };
        assert!(
            import_error_message(
                &LegacyError::FilesNotPlaced {
                    staging: staging.clone(),
                    report: Box::new(dummy_report())
                },
                StartedFrom::Command
            )
            .contains(&staging.display().to_string())
        );
        let cleanup = finish_interrupted_imports(&t.core, &att).await?;
        assert_eq!(cleanup.kept, std::slice::from_ref(&staging), "{}", t.label);
        assert!(cleanup.removed.is_empty() && cleanup.placed.is_empty());
        assert!(staging.join("orders/1/photo.jpg").exists());
        // Место освободилось — следующий запуск доустанавливает.
        std::fs::remove_file(att.join("orders"))?;
        let cleanup = finish_interrupted_imports(&t.core, &att).await?;
        assert_eq!(
            cleanup.placed,
            std::slice::from_ref(&staging),
            "{}",
            t.label
        );
        assert!(att.join("orders/1/photo.jpg").exists() && !staging.exists());
    }
    Ok(())
}

/// Отчёт без содержимого: для сообщения об ошибке нужен только путь.
#[cfg(unix)]
fn dummy_report() -> mw_legacy::ImportReport {
    mw_legacy::ImportReport {
        started_at: chrono::NaiveDateTime::default(),
        source: "/old".into(),
        schema: Default::default(),
        summary: Default::default(),
        files: Default::default(),
        backup: None,
        warnings: Vec::new(),
    }
}

/// Пользователь загружает файл в заказ, пока программа работает после прерванной установки.
async fn upload(core: &Core, order: i64) -> i64 {
    core.add_order_attachment(&admin(), order, "после-импорта.txt", b"new".to_vec())
        .await
        .unwrap()
        .id
}

/// Файл вложения на месте.
async fn attachment_present(core: &Core, order: i64, id: i64) -> bool {
    core.list_order_attachments(&admin(), order)
        .await
        .unwrap()
        .iter()
        .any(|a| a.id == id && !a.missing)
}

#[tokio::test]
async fn uploads_after_an_interrupted_import_stay_in_place_at_start() -> Res {
    for t in cores().await {
        let (src, data) = (fixture_dir(V32).await, tempfile::tempdir()?);
        let core = t
            .core
            .clone()
            .with_attachments_dir(data.path().to_path_buf());
        let att = data.path().join("attachments");
        crash_at(
            &core,
            options(src.path(), data.path(), false),
            ImportStage::Placing,
        )
        .await;
        let id = upload(&core, 1).await;
        let cleanup = finish_interrupted_imports(&core, &att).await?;
        assert_eq!(cleanup.kept.len(), 1, "{}", t.label);
        assert!(attachment_present(&core, 1, id).await, "{}", t.label);
    }
    // Замена (только SQLite): прежней `orders` не было, её создала загрузка — в копию она не уходит.
    let (src, data) = (fixture_dir(V32).await, tempfile::tempdir()?);
    let att = data.path().join("attachments");
    let core = file_sqlite_core(&data.path().join("mw.sqlite"))
        .await
        .with_attachments_dir(data.path().to_path_buf());
    core.create_order(&admin(), sample_new_order()).await?;
    crash_at(
        &core,
        options(src.path(), data.path(), true),
        ImportStage::Placing,
    )
    .await;
    let id = upload(&core, 1).await;
    let cleanup = finish_interrupted_imports(&core, &att).await?;
    assert_eq!(cleanup.kept.len(), 1);
    assert!(attachment_present(&core, 1, id).await);
    let backups: Vec<_> = std::fs::read_dir(data.path().join("backups"))?
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(backups.len(), 1);
    assert!(!backups[0].join("attachments/orders").exists());
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn staging_left_after_placement_does_not_touch_user_folders_at_next_start() -> Res {
    use std::os::unix::fs::PermissionsExt;
    for t in cores().await {
        let (src, data) = (fixture_dir(V32).await, tempfile::tempdir()?);
        let core = t
            .core
            .clone()
            .with_attachments_dir(data.path().to_path_buf());
        let att = data.path().join("attachments");
        crash_at(
            &core,
            options(src.path(), data.path(), false),
            ImportStage::Placing,
        )
        .await;
        let staging = staging_dirs(&att).remove(0);
        // Папку, из которой нельзя удалить файл, не удалить целиком.
        let locked = staging.join("locked");
        std::fs::create_dir_all(&locked)?;
        std::fs::write(locked.join("f"), b"x")?;
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555))?;
        let unlock = || std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755));
        if std::fs::write(locked.join("probe"), b"").is_ok() {
            unlock()?;
            eprintln!("права папок не действуют (запуск от root): проверка пропущена");
            return Ok(());
        }
        let first = finish_interrupted_imports(&core, &att).await?;
        assert_eq!(first.placed, std::slice::from_ref(&staging), "{}", t.label);
        assert!(staging.exists() && att.join("orders/1/photo.jpg").exists());
        let id = upload(&core, 1).await;
        let second = finish_interrupted_imports(&core, &att).await?;
        assert!(second.placed.is_empty(), "{}: {second:?}", t.label);
        assert!(att.join("orders/1/photo.jpg").exists() && att.join("stock/1/abc123.jpg").exists());
        assert!(attachment_present(&core, 1, id).await, "{}", t.label);
        unlock()?;
        let third = finish_interrupted_imports(&core, &att).await?;
        assert_eq!(third.removed, std::slice::from_ref(&staging), "{}", t.label);
        assert!(attachment_present(&core, 1, id).await);
    }
    Ok(())
}

#[tokio::test]
async fn not_empty_target_gets_a_hint_for_the_command_and_for_the_interface() -> Res {
    for t in cores().await {
        let (src, data) = (fixture_dir(V32).await, tempfile::tempdir()?);
        t.core
            .create_client(
                &Actor::unrestricted("Тест"),
                NewClient {
                    full_name: "Уже есть".into(),
                    ..Default::default()
                },
            )
            .await?;
        let e = import_folder(&t.core, &admin(), &options(src.path(), data.path(), false))
            .await
            .unwrap_err();
        let command = import_error_message(&e, StartedFrom::Command);
        let interface = import_error_message(&e, StartedFrom::Interface);
        for text in [&command, &interface] {
            assert!(
                text.contains("Импорт делается только в пустую базу"),
                "{text}"
            );
        }
        assert!(command.contains("--replace"), "{command}");
        assert!(
            interface.contains("с заменой") && !interface.contains("--replace"),
            "{interface}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn occupied_attachments_folder_is_a_problem_for_check_and_import_alike() -> Res {
    for t in cores().await {
        for name in ["orders", "stock", "legacy"] {
            let (src, data) = (fixture_dir(V32).await, tempfile::tempdir()?);
            let occupied = data.path().join("attachments").join(name);
            std::fs::create_dir_all(occupied.join("1"))?;
            std::fs::write(occupied.join("1/user.jpg"), b"user")?;

            let c = check_folder(src.path(), data.path(), false).await;
            assert!(!c.ok, "{name}: {c:?}");
            let text = c.problem.unwrap();
            assert!(text.contains(&occupied.display().to_string()), "{text}");

            let r =
                import_folder(&t.core, &admin(), &options(src.path(), data.path(), false)).await;
            let Err(e @ LegacyError::AttachmentsNotEmpty(_)) = r else {
                panic!("{}: {r:?}", t.label);
            };
            assert_eq!(
                import_error_message(&e, StartedFrom::Interface),
                text,
                "{}",
                t.label
            );
            assert!(
                t.core.import_target_state().await?.is_empty(),
                "{}",
                t.label
            );

            // При замене прежние файлы уходят в копию, проверка их не считает помехой.
            if t.core.database_kind() == mw_core::DatabaseKind::Sqlite {
                assert!(
                    check_folder(src.path(), data.path(), true).await.ok,
                    "{name}"
                );
            }
        }
    }
    Ok(())
}
