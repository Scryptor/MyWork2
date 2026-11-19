//! Прошлые импорты из MyWork 2: строки `legacy_imports` с отчётом (право как у импорта).

use chrono::NaiveDateTime;
use mw_db::entity::legacy_import;
use sea_orm::{EntityTrait, QueryOrder};
use serde::{Deserialize, Serialize};

use crate::{Actor, Core, CoreError};

/// Как выполнен импорт.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum LegacyImportMode {
    /// В пустую базу.
    IntoEmpty,
    /// С заменой прежних данных (перед этим снята копия).
    Replace,
}

impl LegacyImportMode {
    /// Код в `legacy_imports.mode`; незнакомый код читается как `into_empty`.
    fn from_code(code: &str) -> Self {
        match code {
            "replace" => Self::Replace,
            _ => Self::IntoEmpty,
        }
    }
}

/// Один прошлый импорт; `report` — JSON, который собрал `mw-legacy` (со сводкой `summary`).
#[derive(Debug, Clone, PartialEq)]
pub struct LegacyImportRecord {
    pub id: i64,
    pub imported_at: NaiveDateTime,
    pub source_path: String,
    pub mode: LegacyImportMode,
    pub report: serde_json::Value,
}

impl From<legacy_import::Model> for LegacyImportRecord {
    fn from(m: legacy_import::Model) -> Self {
        let report = serde_json::from_str(&m.report).unwrap_or_else(|e| {
            tracing::warn!(import_id = m.id, error = %e, "отчёт импорта не разобран");
            serde_json::Value::Null
        });
        LegacyImportRecord {
            id: m.id,
            imported_at: m.imported_at,
            source_path: m.source_path,
            mode: LegacyImportMode::from_code(&m.mode),
            report,
        }
    }
}

impl Core {
    /// Прошлые импорты, новые первыми (права `settings_manage` и `users_manage`).
    pub async fn list_legacy_imports(
        &self,
        actor: &Actor,
    ) -> Result<Vec<LegacyImportRecord>, CoreError> {
        self.check_import_rights(actor)?;
        Ok(legacy_import::Entity::find()
            .order_by_desc(legacy_import::Column::Id)
            .all(&self.db)
            .await?
            .into_iter()
            .map(LegacyImportRecord::from)
            .collect())
    }

    /// Один прошлый импорт (права `settings_manage` и `users_manage`).
    pub async fn legacy_import(
        &self,
        actor: &Actor,
        id: i64,
    ) -> Result<LegacyImportRecord, CoreError> {
        self.check_import_rights(actor)?;
        legacy_import::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .map(LegacyImportRecord::from)
            .ok_or(CoreError::NotFound {
                entity: "импорт",
                id,
            })
    }
}
