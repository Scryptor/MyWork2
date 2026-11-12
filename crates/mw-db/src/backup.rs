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
