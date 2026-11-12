use mw_db::entity::service_point;
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

/// Точка обслуживания.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct ServicePoint {
    /// Номер точки обслуживания.
    pub id: i64,
    /// Название.
    pub name: String,
    /// Порядковый номер в списке.
    pub sort_order: i32,
    /// Скрыт из выбора (в архиве).
    pub archived: bool,
}

impl From<service_point::Model> for ServicePoint {
    fn from(m: service_point::Model) -> Self {
        Self {
            id: m.id,
            name: m.name,
            sort_order: m.sort_order,
            archived: m.archived,
        }
    }
}

/// Частичное изменение точки обслуживания; `None` — не менять.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, ts_rs::TS, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
#[ts(optional_fields)]
pub struct ServicePointPatch {
    /// Название.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub name: Option<String>,
    /// Скрыт из выбора (в архиве).
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub archived: Option<bool>,
}

async fn all_rows(conn: &impl ConnectionTrait) -> Result<Vec<service_point::Model>, CoreError> {
    Ok(service_point::Entity::find()
        .order_by_asc(service_point::Column::SortOrder)
        .order_by_asc(service_point::Column::Id)
        .all(conn)
        .await?)
}

fn check_name(
    name: &str,
    others: &[service_point::Model],
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

impl Core {
    pub async fn list_service_points(
        &self,
        include_archived: bool,
    ) -> Result<Vec<ServicePoint>, CoreError> {
        let mut query = service_point::Entity::find();
        if !include_archived {
            query = query.filter(service_point::Column::Archived.eq(false));
        }
        Ok(query
            .order_by_asc(service_point::Column::SortOrder)
            .order_by_asc(service_point::Column::Id)
            .all(&self.db)
            .await?
            .into_iter()
            .map(ServicePoint::from)
            .collect())
    }

    pub async fn create_service_point(
        &self,
        actor: &Actor,
        name: &str,
    ) -> Result<ServicePoint, CoreError> {
        actor.require(Permission::SettingsManage)?;
        let tx = self.begin_write().await?;
        let rows = all_rows(&tx).await?;
        let name = check_name(name, &rows, None)?;
        let sort_order = rows.iter().map(|r| r.sort_order).max().map_or(0, |m| m + 1);
        let model = service_point::ActiveModel {
            id: NotSet,
            name: Set(name.clone()),
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
                action: "service_point.created",
                order_id: None,
                client_id: None,
                summary: format!("Добавлена точка обслуживания «{name}»"),
                details: Some(json!({ "id": model.id })),
            },
        )
        .await?;
        tx.commit().await?;
        Ok(model.into())
    }

    pub async fn update_service_point(
        &self,
        actor: &Actor,
        id: i64,
        patch: ServicePointPatch,
    ) -> Result<ServicePoint, CoreError> {
        actor.require(Permission::SettingsManage)?;
        let tx = self.begin_write().await?;
        let rows = all_rows(&tx).await?;
        let current = rows
            .iter()
            .find(|r| r.id == id)
            .cloned()
            .ok_or(CoreError::NotFound {
                entity: "точка обслуживания",
                id,
            })?;
        let mut changed = serde_json::Map::new();
        if let Some(n) = patch.name.as_deref().map(str::trim)
            && n != current.name
        {
            changed.insert("name".into(), json!(n));
        }
        if let Some(a) = patch.archived
            && a != current.archived
        {
            changed.insert("archived".into(), json!(a));
        }
        if changed.is_empty() {
            return Ok(current.into());
        }
        let new_name = match &patch.name {
            Some(n) if n.trim() != current.name => check_name(n, &rows, Some(id))?,
            Some(_) => current.name.clone(),
            None => current.name.clone(),
        };
        let mut active: service_point::ActiveModel = current.into();
        active.name = Set(new_name.clone());
        if let Some(a) = patch.archived {
            active.archived = Set(a);
        }
        let model = active.update(&tx).await?;
        write_audit(
            &tx,
            self.now(),
            actor,
            NewAuditEntry {
                action: "service_point.updated",
                order_id: None,
                client_id: None,
                summary: format!("Изменена точка обслуживания «{new_name}»"),
                details: Some(json!({ "id": id, "changed": changed })),
            },
        )
        .await?;
        tx.commit().await?;
        Ok(model.into())
    }

    /// Принимает перестановку всех id точек обслуживания (включая архивные).
    pub async fn reorder_service_points(
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
            service_point::ActiveModel {
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
                action: "service_points.reordered",
                order_id: None,
                client_id: None,
                summary: "Изменён порядок точек обслуживания".to_owned(),
                details: Some(json!({ "ordered_ids": ordered_ids })),
            },
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
}
