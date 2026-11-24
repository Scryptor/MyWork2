//! Шаблоны печатных документов: хранение в БД, версии, журнал и импорт файла.
//!
//! Сохранённая модель заменяет стандартную при печати. Стандартную в БД не хранят: сохранение
//! стандартной модели сбрасывает строку в `NULL`, а неразбираемая или не прошедшая проверку модель
//! печатается как стандартная, чтобы документы не пропали из-за порчи данных.

use std::collections::HashSet;
use std::sync::Mutex;

use chrono::NaiveDateTime;
use mw_db::entity::document_template;
use mw_docs::standard_template;
use mw_domain::document_template::{Template, TemplateFile};
use mw_domain::documents::DocumentKind;
use mw_domain::permission::Permission;
use mw_domain::validation::FieldError;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{Actor, Core, CoreError, NewAuditEntry, write_audit};

const ENTITY: &str = "document_template";

/// Шаблон документа с его стандартным образцом.
#[derive(Debug, Clone, PartialEq, Serialize, ts_rs::TS, utoipa::ToSchema)]
pub struct DocumentTemplate {
    /// Позиция вида в `DocumentKind::ALL`, начиная с 1.
    pub id: i64,
    pub kind: DocumentKind,
    /// Действующий шаблон: сохранённый или стандартный.
    pub template: Template,
    /// Стандартный шаблон вида — к чему сбрасывать.
    pub standard: Template,
    /// Сохранён шаблон предприятия, а не стандартный.
    pub custom: bool,
    /// Номер правки; 0 — шаблон ещё не сохраняли.
    pub version: i64,
    /// Когда сохранён, местное время предприятия; `None` — не сохраняли.
    #[schema(required = true)]
    pub updated_at: Option<NaiveDateTime>,
}

/// Строка списка шаблонов.
#[derive(Debug, Clone, PartialEq, Serialize, ts_rs::TS, utoipa::ToSchema)]
pub struct DocumentTemplateInfo {
    pub kind: DocumentKind,
    pub custom: bool,
    pub version: i64,
    #[schema(required = true)]
    pub updated_at: Option<NaiveDateTime>,
}

/// Что сохранить: модель и номер правки, на которой она основана.
#[derive(Debug, Clone, PartialEq, Deserialize, ts_rs::TS, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TemplateSave {
    pub template: Template,
    pub version: i64,
}

/// Поле подстановки для меню «вставить поле».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ts_rs::TS, utoipa::ToSchema)]
pub struct DocumentField {
    /// Ключ в данных документа (`order.number`): им пользуется `v("…")` в своём Typst-коде.
    pub key: String,
    /// Имя в тексте шаблона, без скобок.
    pub placeholder: String,
    /// Название для человека.
    pub label: String,
}

fn not_editable() -> CoreError {
    CoreError::Validation(vec![FieldError::new("kind", "not_editable")])
}

fn ensure_editable(kind: DocumentKind) -> Result<(), CoreError> {
    if kind.editable() {
        Ok(())
    } else {
        Err(not_editable())
    }
}

/// `true` при первом сообщении о непригодном шаблоне этого вида и этой `version` за время работы
/// процесса: читают шаблон часто, и повторять одно и то же предупреждение незачем.
fn first_warning(kind: DocumentKind, version: i64) -> bool {
    static SEEN: Mutex<Option<HashSet<(DocumentKind, i64)>>> = Mutex::new(None);
    SEEN.lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(HashSet::new)
        .insert((kind, version))
}

/// Сохранённая модель, если она читается и проходит проверку; иначе `None` (печатается стандартная).
fn usable_model(row: &document_template::Model, kind: DocumentKind) -> Option<Template> {
    let json = row.model.as_deref()?;
    let parsed = serde_json::from_str::<Template>(json)
        .map_err(|e| e.to_string())
        .and_then(|t| match t.validate() {
            Ok(()) if t.kind == kind => Ok(t),
            Ok(()) => Err("вид модели не совпадает с видом строки".to_owned()),
            Err(errors) => Err(format!("{errors:?}")),
        });
    match parsed {
        Ok(template) => Some(template),
        Err(error) => {
            if first_warning(kind, row.version) {
                tracing::warn!(
                    kind = kind.as_str(),
                    version = row.version,
                    %error,
                    "сохранённый шаблон документа непригоден, печатается стандартный"
                );
            }
            None
        }
    }
}

fn assemble(kind: DocumentKind, row: Option<&document_template::Model>) -> DocumentTemplate {
    let standard = standard_template(kind);
    let custom = row.and_then(|r| usable_model(r, kind));
    DocumentTemplate {
        id: kind.ordinal(),
        kind,
        custom: custom.is_some(),
        template: custom.unwrap_or_else(|| standard.clone()),
        standard,
        version: row.map_or(0, |r| r.version),
        updated_at: row.map(|r| r.updated_at),
    }
}

impl Core {
    /// Виды, которые можно менять, в порядке `DocumentKind::ALL`.
    pub async fn list_document_templates(
        &self,
        _actor: &Actor,
    ) -> Result<Vec<DocumentTemplateInfo>, CoreError> {
        let rows = document_template::Entity::find().all(&self.db).await?;
        Ok(DocumentKind::ALL
            .into_iter()
            .filter(|k| k.editable())
            .map(|kind| {
                let row = rows.iter().find(|r| r.kind == kind.as_str());
                let t = assemble(kind, row);
                DocumentTemplateInfo {
                    kind,
                    custom: t.custom,
                    version: t.version,
                    updated_at: t.updated_at,
                }
            })
            .collect())
    }

    pub async fn get_document_template(
        &self,
        _actor: &Actor,
        kind: DocumentKind,
    ) -> Result<DocumentTemplate, CoreError> {
        ensure_editable(kind)?;
        let row = document_template::Entity::find_by_id(kind.as_str())
            .one(&self.db)
            .await?;
        Ok(assemble(kind, row.as_ref()))
    }

    /// Сохраняет шаблон. Модель, равная стандартной, сбрасывает строку в `NULL`. Правка без
    /// изменений ничего не пишет, даже при устаревшей `version`.
    pub async fn save_document_template(
        &self,
        actor: &Actor,
        kind: DocumentKind,
        save: TemplateSave,
    ) -> Result<DocumentTemplate, CoreError> {
        actor.require(Permission::SettingsManage)?;
        ensure_editable(kind)?;
        if save.template.kind != kind {
            return Err(CoreError::Validation(vec![FieldError::new(
                "kind", "mismatch",
            )]));
        }
        save.template.validate().map_err(CoreError::Validation)?;
        let model = if save.template == standard_template(kind) {
            None
        } else {
            Some(
                serde_json::to_string(&save.template)
                    .map_err(|e| CoreError::Internal(format!("шаблон не записан: {e}")))?,
            )
        };

        let tx = self.begin_write().await?;
        let row = document_template::Entity::find_by_id(kind.as_str())
            .one(&tx)
            .await?;
        let current = row.as_ref().map_or(0, |r| r.version);
        if row.as_ref().and_then(|r| r.model.as_ref()) == model.as_ref() {
            return Ok(assemble(kind, row.as_ref()));
        }
        if save.version != current {
            return Err(CoreError::Conflict {
                entity: ENTITY,
                id: kind.ordinal(),
                current_version: current,
            });
        }
        let now = self.now();
        let active = document_template::ActiveModel {
            kind: Set(kind.as_str().to_owned()),
            model: Set(model.clone()),
            version: Set(current + 1),
            updated_at: Set(now),
        };
        let saved = if row.is_some() {
            active.update(&tx).await?
        } else {
            active.insert(&tx).await?
        };
        let summary = if model.is_some() {
            format!("Изменён шаблон «{}»", kind.label_ru())
        } else {
            format!("Шаблон «{}» сброшен к стандартному", kind.label_ru())
        };
        write_audit(
            &tx,
            now,
            actor,
            NewAuditEntry {
                action: "document_template.updated",
                order_id: None,
                client_id: None,
                stock_item_id: None,
                summary,
                details: Some(json!({ "kind": kind.as_str() })),
            },
        )
        .await?;
        tx.commit().await?;
        Ok(assemble(kind, Some(&saved)))
    }

    /// Разбирает файл `.mwtemplate` для этого вида; ничего не записывает.
    pub async fn import_document_template(
        &self,
        actor: &Actor,
        kind: DocumentKind,
        file: serde_json::Value,
    ) -> Result<Template, CoreError> {
        actor.require(Permission::SettingsManage)?;
        ensure_editable(kind)?;
        TemplateFile::from_value(file, kind).map_err(CoreError::Validation)
    }

    /// Typst-код, который печатает то же, что блок шаблона по `path` (право «настройки»). Шаблон
    /// может быть несохранённым черновиком. Блок, который в код не превратить, или путь мимо блока —
    /// `path`/`code_unavailable`. Ничего не рендерит и не читает из базы.
    pub async fn document_block_code(
        &self,
        actor: &Actor,
        kind: DocumentKind,
        template: Template,
        path: Vec<usize>,
    ) -> Result<String, CoreError> {
        actor.require(Permission::SettingsManage)?;
        ensure_editable(kind)?;
        if template.kind != kind {
            return Err(CoreError::Validation(vec![FieldError::new(
                "kind", "mismatch",
            )]));
        }
        template.validate().map_err(CoreError::Validation)?;
        mw_docs::block_code(&template, &path)
            .ok_or_else(|| CoreError::Validation(vec![FieldError::new("path", "code_unavailable")]))
    }

    /// Поля подстановки в порядке меню.
    pub fn document_fields(&self) -> Vec<DocumentField> {
        mw_docs::fields::fields()
            .iter()
            .map(|f| DocumentField {
                key: f.key.to_owned(),
                placeholder: f.placeholder.to_owned(),
                label: f.label.to_owned(),
            })
            .collect()
    }

    /// Шаблон для печати: сохранённый и годный, иначе стандартный.
    pub(crate) async fn template_for_print(
        &self,
        kind: DocumentKind,
    ) -> Result<Template, CoreError> {
        if !kind.editable() {
            return Ok(standard_template(kind));
        }
        let row = document_template::Entity::find_by_id(kind.as_str())
            .one(&self.db)
            .await?;
        Ok(assemble(kind, row.as_ref()).template)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_broken_template_is_warned_about_once_per_kind_and_version() {
        // Виды и версии, которых не берёт ни один другой тест процесса.
        assert!(first_warning(DocumentKind::Sticker, 9_001));
        assert!(!first_warning(DocumentKind::Sticker, 9_001));
        assert!(first_warning(DocumentKind::Sticker, 9_002));
        assert!(first_warning(DocumentKind::IntakeAct, 9_001));
    }
}
