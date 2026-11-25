//! Настройки копий из MyWork 2: читаются из сохранённых строк `Config.ini` (`legacy_rows`).

use mw_db::entity::legacy_row;
use mw_domain::permission::Permission;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

use super::{BackupLegacySettings, BackupSettings, settings};
use crate::{Actor, Core, CoreError};

/// Значение флажка `EveryDayBackup`: нет строки — MyWork 2 делал копию каждый день.
pub(crate) fn legacy_auto(value: Option<&str>) -> bool {
    matches!(value, Some("Checked") | None)
}

/// Значение строки `Config.ini`, сохранённой импортом.
struct IniRow {
    section: String,
    key: String,
    value: String,
}

impl Core {
    /// Настройки копий из последнего импорта; `None` — таких строк нет.
    pub(super) async fn read_legacy_backup_settings(
        &self,
    ) -> Result<Option<BackupLegacySettings>, CoreError> {
        let rows = legacy_row::Entity::find()
            .filter(legacy_row::Column::Source.eq("Config.ini"))
            .order_by_desc(legacy_row::Column::ImportId)
            .order_by_asc(legacy_row::Column::Id)
            .all(&self.db)
            .await?;
        let Some(latest) = rows.first().map(|r| r.import_id) else {
            return Ok(None);
        };
        let rows = rows
            .into_iter()
            .take_while(|r| r.import_id == latest)
            .filter_map(|r| {
                let raw: serde_json::Value = serde_json::from_str(&r.raw).ok()?;
                let text = |name| raw.get(name)?.as_str().map(str::to_owned);
                Some(IniRow {
                    section: text("section")?,
                    key: text("key")?,
                    value: text("value")?,
                })
            });
        let mut auto = None;
        let mut folders: Vec<String> = Vec::new();
        for row in rows {
            if row.section.eq_ignore_ascii_case("CHECKBOX")
                && row.key.eq_ignore_ascii_case("EveryDayBackup")
            {
                auto.get_or_insert(row.value);
            } else if row.key.eq_ignore_ascii_case("backupPath") {
                let value = row.value.trim();
                if !value.is_empty() && !folders.iter().any(|f| f == value) {
                    folders.push(value.to_owned());
                }
            }
        }
        if auto.is_none() && folders.is_empty() {
            // Строки не про копии: нечего предлагать. Пустое значение пути не считается строкой.
            return Ok(None);
        }
        Ok(Some(BackupLegacySettings {
            auto: legacy_auto(auto.as_deref()),
            folders,
        }))
    }

    /// Отвечает на предложение: берёт настройки MyWork 2 или оставляет свои; предложение
    /// больше не показывается. Папкой становится первый путь, который годится как папка копий.
    pub async fn choose_legacy_backup(
        &self,
        actor: &Actor,
        apply: bool,
    ) -> Result<BackupSettings, CoreError> {
        actor.require(Permission::SettingsManage)?;
        let data_dir = self.backups_data_dir()?.to_path_buf();
        let old = settings::read(&data_dir);
        let mut new = old.clone();
        new.legacy_reviewed = true;
        if apply && let Some(legacy) = self.read_legacy_backup_settings().await? {
            new.auto = legacy.auto;
            if let Some(folder) = legacy
                .folders
                .into_iter()
                .find(|f| settings::check_folder(&data_dir, f).is_ok())
            {
                new.folder = Some(folder);
            }
        }
        if new != old {
            self.store_backup_settings(actor, &data_dir, &old, &new)
                .await?;
        }
        Ok(new)
    }
}

#[cfg(test)]
mod tests {
    use super::legacy_auto;

    #[test]
    fn legacy_auto_reads_the_checkbox() {
        assert!(legacy_auto(Some("Checked")));
        assert!(legacy_auto(None));
        for other in ["Unchecked", "checked", ""] {
            assert!(!legacy_auto(Some(other)), "{other}");
        }
    }
}
