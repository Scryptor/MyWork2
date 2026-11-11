use mw_db::entity::role;
use mw_domain::permission::{Permission, PermissionSet};
use mw_domain::search::normalize_search;
use mw_domain::validation::FieldError;
use sea_orm::{ActiveModelTrait, ConnectionTrait, EntityTrait, NotSet, QueryOrder, Set};
use serde_json::{Map, json};

use crate::{Actor, Core, CoreError, NewAuditEntry, write_audit};

use super::users;

const ENTITY: &str = "роль";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Role {
    pub id: i64,
    pub name: String,
    pub permissions: PermissionSet,
    pub version: i64,
}

impl From<role::Model> for Role {
    fn from(m: role::Model) -> Self {
        Self {
            id: m.id,
            name: m.name,
            permissions: parse_permissions(&m.permissions),
            version: m.version,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct NewRole {
    pub name: String,
    pub permissions: PermissionSet,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RolePatch {
    pub name: Option<String>,
    pub permissions: Option<PermissionSet>,
}

pub(super) fn parse_permissions(json: &str) -> PermissionSet {
    serde_json::from_str(json).unwrap_or_default()
}

fn permissions_json(set: &PermissionSet) -> Result<String, CoreError> {
    serde_json::to_string(set).map_err(|e| CoreError::Internal(format!("права роли: {e}")))
}

pub(super) async fn all_roles(conn: &impl ConnectionTrait) -> Result<Vec<role::Model>, CoreError> {
    Ok(role::Entity::find()
        .order_by_asc(role::Column::Id)
        .all(conn)
        .await?)
}

fn check_name(
    name: &str,
    others: &[role::Model],
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
    pub async fn list_roles(&self, actor: &Actor) -> Result<Vec<Role>, CoreError> {
        actor.require(Permission::UsersManage)?;
        Ok(all_roles(&self.db)
            .await?
            .into_iter()
            .map(Role::from)
            .collect())
    }

    pub async fn create_role(&self, actor: &Actor, new: NewRole) -> Result<Role, CoreError> {
        actor.require(Permission::UsersManage)?;
        let tx = self.begin_write().await?;
        let rows = all_roles(&tx).await?;
        let name = check_name(&new.name, &rows, None)?;
        let model = role::ActiveModel {
            id: NotSet,
            name: Set(name.clone()),
            permissions: Set(permissions_json(&new.permissions)?),
            version: Set(1),
        }
        .insert(&tx)
        .await?;
        write_audit(
            &tx,
            self.now(),
            actor,
            NewAuditEntry {
                action: "role.created",
                order_id: None,
                client_id: None,
                summary: format!("Создана роль «{name}»"),
                details: Some(json!({ "id": model.id, "permissions": new.permissions })),
            },
        )
        .await?;
        tx.commit().await?;
        Ok(model.into())
    }

    pub async fn update_role(
        &self,
        actor: &Actor,
        id: i64,
        expected_version: i64,
        patch: RolePatch,
    ) -> Result<Role, CoreError> {
        actor.require(Permission::UsersManage)?;
        let tx = self.begin_write().await?;
        let rows = all_roles(&tx).await?;
        let current = rows
            .iter()
            .find(|r| r.id == id)
            .cloned()
            .ok_or(CoreError::NotFound { entity: ENTITY, id })?;
        let new_name = patch.name.as_deref().map(str::trim);
        let old_permissions = parse_permissions(&current.permissions);
        let permissions = patch
            .permissions
            .clone()
            .unwrap_or_else(|| old_permissions.clone());

        let mut changed = Map::new();
        if let Some(n) = new_name
            && n != current.name
        {
            changed.insert("name".into(), json!(n));
        }
        if permissions != old_permissions {
            changed.insert("permissions".into(), json!(permissions));
        }
        if changed.is_empty() {
            return Ok(current.into());
        }
        if current.version != expected_version {
            return Err(CoreError::Conflict {
                entity: ENTITY,
                id,
                current_version: current.version,
            });
        }
        let name = match &patch.name {
            Some(n) if changed.contains_key("name") => check_name(n, &rows, Some(id))?,
            _ => current.name.clone(),
        };

        if permissions != old_permissions {
            let users = users::all_users(&tx).await?;
            let before_perms = users::role_permissions(&rows);
            let mut after_perms = before_perms.clone();
            after_perms.insert(id, permissions.clone());
            let accounts = users::accounts(&users);
            if users::would_lock_out((&accounts, &before_perms), (&accounts, &after_perms)) {
                return Err(CoreError::Validation(vec![FieldError::new(
                    "permissions",
                    "last_admin",
                )]));
            }
        }

        let mut active: role::ActiveModel = current.into();
        active.name = Set(name.clone());
        active.permissions = Set(permissions_json(&permissions)?);
        active.version = Set(expected_version + 1);
        let model = active.update(&tx).await?;
        write_audit(
            &tx,
            self.now(),
            actor,
            NewAuditEntry {
                action: "role.updated",
                order_id: None,
                client_id: None,
                summary: format!("Изменена роль «{name}»"),
                details: Some(json!({ "id": id, "changed": changed })),
            },
        )
        .await?;
        tx.commit().await?;
        Ok(model.into())
    }
}
