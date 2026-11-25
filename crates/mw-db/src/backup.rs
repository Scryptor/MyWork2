//! Копия базы SQLite одним файлом.

use std::path::Path;

use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, DbErr, Statement};

/// Согласованная копия SQLite-базы (`VACUUM INTO`). Создаёт папку назначения.
/// Если файл уже есть, возвращает ошибку и не трогает его.
pub async fn vacuum_into(db: &DatabaseConnection, path: &Path) -> Result<(), DbErr> {
    if db.get_database_backend() != DatabaseBackend::Sqlite {
        return Err(DbErr::Custom(
            "копия файлом возможна только для SQLite".to_owned(),
        ));
    }
    if path.exists() {
        return Err(DbErr::Custom(format!(
            "файл копии уже существует: {}",
            path.display()
        )));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| DbErr::Custom(format!("не удалось создать папку копии: {e}")))?;
    }
    let target = path
        .to_str()
        .ok_or_else(|| DbErr::Custom("путь копии не является текстом UTF-8".to_owned()))?;
    db.execute_raw(Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "VACUUM INTO ?",
        [target.into()],
    ))
    .await?;
    Ok(())
}

/// Что нашлось в файле копии базы.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyCheck {
    /// `PRAGMA quick_check` ответил `ok`, и есть таблица миграций.
    pub intact: bool,
    /// Миграции копии, которых эта программа не знает (копия от более новой версии).
    pub unknown_migrations: Vec<String>,
}

/// Осматривает файл копии базы, ничего в нём не меняя: одно соединение, без создания файла и
/// без смены `journal_mode`. Нет файла — ошибка; повреждённый или не тот файл — `intact: false`.
pub async fn inspect_copy(path: &Path) -> Result<CopyCheck, DbErr> {
    use sea_orm::sqlx::sqlite::SqliteConnectOptions;
    use sea_orm::sqlx::{ConnectOptions, Connection, Row};
    use sea_orm_migration::MigratorTrait;

    if !path.is_file() {
        return Err(DbErr::Custom(format!(
            "файла копии нет: {}",
            path.display()
        )));
    }
    let broken = CopyCheck {
        intact: false,
        unknown_migrations: Vec::new(),
    };
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(false)
        .read_only(true);
    let Ok(mut conn) = options.connect().await else {
        return Ok(broken);
    };
    let checked = async {
        let first: String = sea_orm::sqlx::query("PRAGMA quick_check")
            .fetch_optional(&mut conn)
            .await?
            .map(|row| row.try_get(0))
            .transpose()?
            .unwrap_or_default();
        if first != "ok" {
            return Ok::<_, sea_orm::sqlx::Error>(None);
        }
        let rows = sea_orm::sqlx::query("SELECT version FROM seaql_migrations")
            .fetch_all(&mut conn)
            .await?;
        let mut versions = Vec::new();
        for row in rows {
            versions.push(row.try_get::<String, _>(0)?);
        }
        Ok(Some(versions))
    }
    .await;
    conn.close().await.ok();
    let Ok(Some(versions)) = checked else {
        return Ok(broken);
    };
    let known: Vec<String> = crate::migration::Migrator::migrations()
        .iter()
        .map(|m| m.name().to_owned())
        .collect();
    let mut unknown_migrations: Vec<String> = versions
        .into_iter()
        .filter(|v| !known.contains(v))
        .collect();
    unknown_migrations.sort();
    Ok(CopyCheck {
        intact: true,
        unknown_migrations,
    })
}

/// Сводит копию базы к одному файлу: переносит журнал WAL в основной файл и возвращает
/// обычный журнал. Одно обычное соединение на запись, без `connect` (он включает WAL).
/// Повторный вызов безопасен.
pub async fn make_single_file(path: &Path) -> Result<(), DbErr> {
    use sea_orm::sqlx::sqlite::SqliteConnectOptions;
    use sea_orm::sqlx::{ConnectOptions, Connection};

    let map = |e: sea_orm::sqlx::Error| DbErr::Custom(e.to_string());
    let mut conn = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(false)
        .connect()
        .await
        .map_err(map)?;
    let done = async {
        sea_orm::sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&mut conn)
            .await?;
        sea_orm::sqlx::query("PRAGMA journal_mode=DELETE")
            .execute(&mut conn)
            .await?;
        Ok::<_, sea_orm::sqlx::Error>(())
    }
    .await;
    conn.close().await.map_err(map)?;
    done.map_err(map)
}
