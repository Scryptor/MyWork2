use mw_db::entity::status;
use mw_domain::permission::Permission;
use mw_domain::search::normalize_search;
use mw_domain::status::StatusKind;
use mw_domain::validation::FieldError;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, NotSet, QueryFilter, QueryOrder,
    Set,
};
use serde_json::json;

use crate::{Actor, Core, CoreError, NewAuditEntry, write_audit};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub id: i64,
    pub name: String,
    pub kind: StatusKind,
    pub color: String,
    pub sort_order: i32,
    pub archived: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewStatus {
    pub name: String,
    pub kind: StatusKind,
    pub color: String,
}

/// Частичное изменение статуса; `None` — не менять.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatusPatch {
    pub name: Option<String>,
    pub kind: Option<StatusKind>,
    pub color: Option<String>,
    pub archived: Option<bool>,
}

fn to_status(m: status::Model) -> Result<Status, CoreError> {
    let kind = m
        .kind
        .parse::<StatusKind>()
        .map_err(|e| CoreError::Internal(format!("статус №{}: {e:?}", m.id)))?;
    Ok(Status {
        id: m.id,
        name: m.name,
        kind,
        color: m.color,
        sort_order: m.sort_order,
        archived: m.archived,
    })
}

fn is_live_issued(m: &status::Model) -> bool {
    m.kind == StatusKind::Issued.as_str() && !m.archived
}

/// Имя без пробелов по краям; пустое — `required`, совпавшее с чужим — `duplicate`.
fn check_name(
    name: &str,
    others: &[status::Model],
    own_id: Option<i64>,
) -> Result<String, CoreError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(CoreError::Validation(vec![FieldError::required("name")]));
    }
    let key = normalize_search(name);
    if others
        .iter()
        .any(|o| Some(o.id) != own_id && normalize_search(&o.name) == key)
    {
        return Err(CoreError::Validation(vec![FieldError::new(
            "name",
            "duplicate",
        )]));
    }
    Ok(name.to_owned())
}

async fn all_rows(conn: &impl ConnectionTrait) -> Result<Vec<status::Model>, CoreError> {
    Ok(status::Entity::find()
        .order_by_asc(status::Column::SortOrder)
        .order_by_asc(status::Column::Id)
        .all(conn)
        .await?)
}

impl Core {
    pub async fn list_statuses(&self, include_archived: bool) -> Result<Vec<Status>, CoreError> {
        let mut query = status::Entity::find();
        if !include_archived {
            query = query.filter(status::Column::Archived.eq(false));
        }
        query
            .order_by_asc(status::Column::SortOrder)
            .order_by_asc(status::Column::Id)
            .all(&self.db)
            .await?
            .into_iter()
            .map(to_status)
            .collect()
    }

    pub async fn create_status(&self, actor: &Actor, new: NewStatus) -> Result<Status, CoreError> {
        actor.require(Permission::SettingsManage)?;
        let tx = self.begin_write().await?;
        let rows = all_rows(&tx).await?;
        let name = check_name(&new.name, &rows, None)?;
        let sort_order = rows.iter().map(|r| r.sort_order).max().map_or(0, |m| m + 1);
        let model = status::ActiveModel {
            id: NotSet,
            name: Set(name.clone()),
            kind: Set(new.kind.as_str().to_owned()),
            color: Set(new.color.trim().to_owned()),
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
                action: "status.created",
                order_id: None,
                client_id: None,
                summary: format!("Добавлен статус «{name}»"),
                details: Some(json!({ "id": model.id, "kind": new.kind.as_str() })),
            },
        )
        .await?;
        tx.commit().await?;
        to_status(model)
    }

    pub async fn update_status(
        &self,
        actor: &Actor,
        id: i64,
        patch: StatusPatch,
    ) -> Result<Status, CoreError> {
        actor.require(Permission::SettingsManage)?;
        let tx = self.begin_write().await?;
        let rows = all_rows(&tx).await?;
        let current = rows
            .iter()
            .find(|r| r.id == id)
            .cloned()
            .ok_or(CoreError::NotFound {
                entity: "статус",
                id,
            })?;
        let new_name = patch.name.as_deref().map(str::trim);
        let kind = patch
            .kind
            .map_or(current.kind.clone(), |k| k.as_str().to_owned());
        let archived = patch.archived.unwrap_or(current.archived);
        let color = patch
            .color
            .as_deref()
            .map_or(current.color.clone(), |c| c.trim().to_owned());

        let mut changed = serde_json::Map::new();
        if let Some(n) = new_name
            && n != current.name
        {
            changed.insert("name".into(), json!(n));
        }
        if kind != current.kind {
            changed.insert("kind".into(), json!(kind));
        }
        if color != current.color {
            changed.insert("color".into(), json!(color));
        }
        if archived != current.archived {
            changed.insert("archived".into(), json!(archived));
        }
        if changed.is_empty() {
            return to_status(current);
        }
        let name = match &patch.name {
            Some(n) => check_name(n, &rows, Some(id))?,
            None => current.name.clone(),
        };

        let after = status::Model {
            kind: kind.clone(),
            archived,
            ..current.clone()
        };
        if is_live_issued(&current)
            && !is_live_issued(&after)
            && !rows.iter().any(|r| r.id != id && is_live_issued(r))
        {
            let field = if archived && !current.archived {
                "archived"
            } else {
                "kind"
            };
            return Err(CoreError::Validation(vec![FieldError::new(
                field,
                "last_issued_status",
            )]));
        }

        let mut active: status::ActiveModel = current.into();
        active.name = Set(name.clone());
        active.kind = Set(kind);
        active.color = Set(color);
        active.archived = Set(archived);
        let model = active.update(&tx).await?;
        write_audit(
            &tx,
            self.now(),
            actor,
            NewAuditEntry {
                action: "status.updated",
                order_id: None,
                client_id: None,
                summary: format!("Изменён статус «{name}»"),
                details: Some(json!({ "id": id, "changed": changed })),
            },
        )
        .await?;
        tx.commit().await?;
        to_status(model)
    }

    /// Принимает перестановку всех id статусов (включая архивные); `sort_order` = позиция.
    pub async fn reorder_statuses(
        &self,
        actor: &Actor,
        ordered_ids: &[i64],
    ) -> Result<(), CoreError> {
        actor.require(Permission::SettingsManage)?;
        let tx = self.begin_write().await?;
        let rows = all_rows(&tx).await?;
        check_permutation(ordered_ids, rows.iter().map(|r| r.id))?;
        if rows.iter().map(|r| r.id).eq(ordered_ids.iter().copied()) {
            return Ok(());
        }
        for (position, id) in (0_i32..).zip(ordered_ids) {
            status::ActiveModel {
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
                action: "statuses.reordered",
                order_id: None,
                client_id: None,
                summary: "Изменён порядок статусов".to_owned(),
                details: Some(json!({ "ordered_ids": ordered_ids })),
            },
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
}

/// `ordered_ids` должен содержать каждый существующий id ровно один раз.
pub(crate) fn check_permutation(
    ordered_ids: &[i64],
    existing: impl Iterator<Item = i64>,
) -> Result<(), CoreError> {
    let mut want: Vec<i64> = existing.collect();
    let mut got = ordered_ids.to_vec();
    want.sort_unstable();
    got.sort_unstable();
    if want == got {
        Ok(())
    } else {
        Err(CoreError::Validation(vec![FieldError::new(
            "ordered_ids",
            "invalid",
        )]))
    }
}
