use mw_db::entity::dictionary_item;
use mw_domain::dictionary::DictionaryKind;
use mw_domain::permission::Permission;
use mw_domain::search::normalize_search;
use mw_domain::validation::FieldError;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, NotSet, QueryFilter, QueryOrder,
    Set,
};
use serde_json::json;

use crate::statuses::check_permutation;
use crate::{Actor, Core, CoreError, NewAuditEntry, write_audit};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictionaryItem {
    pub id: i64,
    pub kind: DictionaryKind,
    pub value: String,
    pub sort_order: i32,
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
    /// По `sort_order`, затем по значению.
    pub async fn list_dictionary(
        &self,
        kind: DictionaryKind,
    ) -> Result<Vec<DictionaryItem>, CoreError> {
        let mut items = kind_rows(&self.db, kind)
            .await?
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

    pub async fn rename_dictionary_item(
        &self,
        actor: &Actor,
        id: i64,
        value: &str,
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
        let rows = kind_rows(&tx, kind).await?;
        let value = check_value(value, &rows, Some(id))?;
        let old = current.value.clone();
        let mut active: dictionary_item::ActiveModel = current.into();
        active.value = Set(value.clone());
        let model = active.update(&tx).await?;
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
                details: Some(json!({ "id": id, "kind": kind.as_str(), "old": old, "new": value })),
            },
        )
        .await?;
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
