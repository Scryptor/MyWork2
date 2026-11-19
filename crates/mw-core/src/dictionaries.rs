use mw_db::entity::dictionary_item;
use mw_domain::dictionary::DictionaryKind;
use mw_domain::permission::Permission;
use mw_domain::search::normalize_search;
use mw_domain::validation::FieldError;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, NotSet, QueryFilter, QueryOrder,
    Set,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::statuses::check_permutation;
use crate::{Actor, Core, CoreError, NewAuditEntry, write_audit};

/// Значение справочника.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct DictionaryItem {
    /// Номер значения.
    pub id: i64,
    /// Вид справочника.
    pub kind: DictionaryKind,
    /// Текст значения.
    pub value: String,
    /// Порядковый номер в списке.
    pub sort_order: i32,
    /// Скрыто из выбора (в архиве).
    pub archived: bool,
}

/// Частичное изменение значения справочника; не переданное поле не меняется.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, ts_rs::TS, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
#[ts(optional_fields)]
pub struct DictionaryItemPatch {
    /// Текст значения.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub value: Option<String>,
    /// Скрыто из выбора (в архиве).
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub archived: Option<bool>,
}

fn to_item(m: dictionary_item::Model) -> Result<DictionaryItem, CoreError> {
    let kind = m
        .kind
        .parse::<DictionaryKind>()
        .map_err(|e| CoreError::Internal(format!("справочник №{}: {e:?}", m.id)))?;
    Ok(DictionaryItem {
        id: m.id,
        kind,
        value: m.value,
        sort_order: m.sort_order,
        archived: m.archived,
    })
}

async fn kind_rows(
    conn: &impl ConnectionTrait,
    kind: DictionaryKind,
) -> Result<Vec<dictionary_item::Model>, CoreError> {
    Ok(dictionary_item::Entity::find()
        .filter(dictionary_item::Column::Kind.eq(kind.as_str()))
        .order_by_asc(dictionary_item::Column::SortOrder)
        .order_by_asc(dictionary_item::Column::Id)
        .all(conn)
        .await?)
}

/// По `sort_order`, затем по значению — порядок, в котором значения видит `list_dictionary`.
fn sorted_items(rows: Vec<dictionary_item::Model>) -> Result<Vec<DictionaryItem>, CoreError> {
    let mut items = rows
        .into_iter()
        .map(to_item)
        .collect::<Result<Vec<_>, _>>()?;
    items.sort_by(|a, b| {
        a.sort_order
            .cmp(&b.sort_order)
            .then_with(|| normalize_search(&a.value).cmp(&normalize_search(&b.value)))
    });
    Ok(items)
}

fn check_value(
    value: &str,
    others: &[dictionary_item::Model],
    own_id: Option<i64>,
) -> Result<String, CoreError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(CoreError::Validation(vec![FieldError::required("value")]));
    }
    let key = normalize_search(value);
    if others
        .iter()
        .any(|o| Some(o.id) != own_id && normalize_search(&o.value) == key)
    {
        return Err(CoreError::Validation(vec![FieldError::new(
            "value",
            "duplicate",
        )]));
    }
    Ok(value.to_owned())
}

impl Core {
    /// По `sort_order`, затем по значению; архивные — только по запросу.
    pub async fn list_dictionary(
        &self,
        kind: DictionaryKind,
        include_archived: bool,
    ) -> Result<Vec<DictionaryItem>, CoreError> {
        let mut rows = kind_rows(&self.db, kind).await?;
        if !include_archived {
            rows.retain(|r| !r.archived);
        }
        sorted_items(rows)
    }

    /// Доступно и из настроек, и из приёма («+ своё»).
    pub async fn add_dictionary_item(
        &self,
        actor: &Actor,
        kind: DictionaryKind,
        value: &str,
    ) -> Result<DictionaryItem, CoreError> {
        actor.require_any(&[Permission::SettingsManage, Permission::OrdersCreate])?;
        let tx = self.begin_write().await?;
        let rows = kind_rows(&tx, kind).await?;
        let value = check_value(value, &rows, None)?;
        let sort_order = rows.iter().map(|r| r.sort_order).max().map_or(0, |m| m + 1);
        let model = dictionary_item::ActiveModel {
            id: NotSet,
            kind: Set(kind.as_str().to_owned()),
            value: Set(value.clone()),
            sort_order: Set(sort_order),
            archived: Set(false),
        }
        .insert(&tx)
        .await?;
        write_audit(
            &tx,
            self.now(),
            actor,
            NewAuditEntry {
                action: "dictionary.added",
                order_id: None,
                client_id: None,
                summary: format!("Справочник «{}»: добавлено «{value}»", kind.label_ru()),
                details: Some(json!({ "id": model.id, "kind": kind.as_str(), "value": value })),
            },
        )
        .await?;
        tx.commit().await?;
        to_item(model)
    }

    /// Переименование и архивация значения; правка без изменений ничего не пишет.
    pub async fn update_dictionary_item(
        &self,
        actor: &Actor,
        id: i64,
        patch: DictionaryItemPatch,
    ) -> Result<DictionaryItem, CoreError> {
        actor.require(Permission::SettingsManage)?;
        let tx = self.begin_write().await?;
        let current = dictionary_item::Entity::find_by_id(id)
            .one(&tx)
            .await?
            .ok_or(CoreError::NotFound {
                entity: "значение справочника",
                id,
            })?;
        let kind = current
            .kind
            .parse::<DictionaryKind>()
            .map_err(|e| CoreError::Internal(format!("справочник №{id}: {e:?}")))?;
        let renamed = match patch.value.as_deref() {
            Some(v) if v.trim() != current.value => {
                let rows = kind_rows(&tx, kind).await?;
                Some(check_value(v, &rows, Some(id))?)
            }
            Some(v) if v.trim().is_empty() => {
                return Err(CoreError::Validation(vec![FieldError::required("value")]));
            }
            _ => None,
        };
        let archived = patch.archived.filter(|a| *a != current.archived);
        if renamed.is_none() && archived.is_none() {
            return to_item(current);
        }
        let old = current.value.clone();
        let mut active: dictionary_item::ActiveModel = current.into();
        if let Some(v) = &renamed {
            active.value = Set(v.clone());
        }
        if let Some(a) = archived {
            active.archived = Set(a);
        }
        let model = active.update(&tx).await?;
        if let Some(value) = renamed {
            write_audit(
                &tx,
                self.now(),
                actor,
                NewAuditEntry {
                    action: "dictionary.renamed",
                    order_id: None,
                    client_id: None,
                    summary: format!(
                        "Справочник «{}»: «{old}» переименовано в «{value}»",
                        kind.label_ru()
                    ),
                    details: Some(
                        json!({ "id": id, "kind": kind.as_str(), "old": old, "new": value }),
                    ),
                },
            )
            .await?;
        }
        if let Some(a) = archived {
            write_audit(
                &tx,
                self.now(),
                actor,
                NewAuditEntry {
                    action: if a {
                        "dictionary.archived"
                    } else {
                        "dictionary.restored"
                    },
                    order_id: None,
                    client_id: None,
                    summary: format!(
                        "Справочник «{}»: «{}» {}",
                        kind.label_ru(),
                        model.value,
                        if a {
                            "в архиве"
                        } else {
                            "возвращено из архива"
                        }
                    ),
                    details: Some(json!({ "id": id, "kind": kind.as_str(), "archived": a })),
                },
            )
            .await?;
        }
        tx.commit().await?;
        to_item(model)
    }

    pub async fn delete_dictionary_item(&self, actor: &Actor, id: i64) -> Result<(), CoreError> {
        actor.require(Permission::SettingsManage)?;
        let tx = self.begin_write().await?;
        let current = dictionary_item::Entity::find_by_id(id)
            .one(&tx)
            .await?
            .ok_or(CoreError::NotFound {
                entity: "значение справочника",
                id,
            })?;
        let kind = current
            .kind
            .parse::<DictionaryKind>()
            .map_err(|e| CoreError::Internal(format!("справочник №{id}: {e:?}")))?;
        dictionary_item::Entity::delete_by_id(id).exec(&tx).await?;
        write_audit(
            &tx,
            self.now(),
            actor,
            NewAuditEntry {
                action: "dictionary.deleted",
                order_id: None,
                client_id: None,
                summary: format!(
                    "Справочник «{}»: удалено «{}»",
                    kind.label_ru(),
                    current.value
                ),
                details: Some(json!({ "id": id, "kind": current.kind, "value": current.value })),
            },
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Принимает перестановку всех id значений этого справочника.
    pub async fn reorder_dictionary(
        &self,
        actor: &Actor,
        kind: DictionaryKind,
        ordered_ids: &[i64],
    ) -> Result<(), CoreError> {
        actor.require(Permission::SettingsManage)?;
        let tx = self.begin_write().await?;
        let rows = kind_rows(&tx, kind).await?;
        check_permutation(ordered_ids, rows.iter().map(|r| r.id))?;
        let current = sorted_items(rows)?;
        if current.iter().map(|i| i.id).eq(ordered_ids.iter().copied()) {
            return Ok(());
        }
        for (position, id) in (0_i32..).zip(ordered_ids) {
            dictionary_item::ActiveModel {
                id: Set(*id),
                sort_order: Set(position),
                ..Default::default()
            }
            .update(&tx)
            .await?;
        }
        write_audit(
            &tx,
            self.now(),
            actor,
            NewAuditEntry {
                action: "dictionary.reordered",
                order_id: None,
                client_id: None,
                summary: format!("Изменён порядок справочника «{}»", kind.label_ru()),
                details: Some(json!({ "kind": kind.as_str(), "ordered_ids": ordered_ids })),
            },
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
}
