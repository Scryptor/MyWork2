use mw_db::entity::{role, user};
use mw_domain::permission::{Permission, PermissionSet};
use mw_domain::search::normalize_search;
use mw_domain::validation::FieldError;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, NotSet, PaginatorTrait,
    QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, json};

use crate::{Actor, Core, CoreError, NewAuditEntry, write_audit};

use super::users;

const ENTITY: &str = "роль";

/// Роль: набор прав.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct Role {
    /// Номер роли.
    pub id: i64,
    /// Название роли.
    pub name: String,
    /// Набор прав.
    pub permissions: PermissionSet,
    /// Версия записи для проверки конфликтов правок.
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

/// Данные новой роли.
#[derive(Debug, Clone, Default, Deserialize, ts_rs::TS, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewRole {
    /// Название роли.
    pub name: String,
    /// Набор прав.
    pub permissions: PermissionSet,
}

/// Частичное изменение роли; отсутствующее поле не меняется.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, ts_rs::TS, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
#[ts(optional_fields)]
pub struct RolePatch {
    /// Название роли.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub name: Option<String>,
    /// Набор прав.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
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

/// Есть ли право `users_manage` у какой-нибудь роли, кроме `id`.
fn another_role_manages_users(rows: &[role::Model], id: i64) -> bool {
    rows.iter()
        .any(|r| r.id != id && parse_permissions(&r.permissions).contains(Permission::UsersManage))
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
            // Даже когда войти некому (режим без входа), без роли с этим правом не создать
            // администратора (`mw-server create-admin`, первый вход).
            if old_permissions.contains(Permission::UsersManage)
                && !permissions.contains(Permission::UsersManage)
                && !another_role_manages_users(&rows, id)
            {
                return Err(CoreError::InUse(format!(
                    "Нельзя снять право «{}» с последней роли, где оно есть",
                    Permission::UsersManage.label_ru()
                )));
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

    /// Удаляет роль по версии записи. Роль, в которой есть пользователи (любые: отключённые и
    /// без входа тоже), не удаляется — `InUse` с их числом. Последняя роль с правом
    /// `users_manage` не удаляется даже пустой: без неё не создать администратора
    /// (`mw-server create-admin`, первый пользователь в режиме без входа).
    pub async fn delete_role(
        &self,
        actor: &Actor,
        id: i64,
        expected_version: i64,
    ) -> Result<(), CoreError> {
        actor.require(Permission::UsersManage)?;
        let tx = self.begin_write().await?;
        let current = role::Entity::find_by_id(id)
            .one(&tx)
            .await?
            .ok_or(CoreError::NotFound { entity: ENTITY, id })?;
        if current.version != expected_version {
            return Err(CoreError::Conflict {
                entity: ENTITY,
                id,
                current_version: current.version,
            });
        }
        let members = user::Entity::find()
            .filter(user::Column::RoleId.eq(id))
            .count(&tx)
            .await?;
        if members > 0 {
            return Err(CoreError::InUse(format!(
                "В роли есть пользователи: {members}"
            )));
        }
        if parse_permissions(&current.permissions).contains(Permission::UsersManage)
            && !another_role_manages_users(&all_roles(&tx).await?, id)
        {
            return Err(CoreError::InUse(format!(
                "Нельзя удалить последнюю роль с правом «{}»",
                Permission::UsersManage.label_ru()
            )));
        }
        role::Entity::delete_by_id(id).exec(&tx).await?;
        write_audit(
            &tx,
            self.now(),
            actor,
            NewAuditEntry {
                action: "role.deleted",
                order_id: None,
                client_id: None,
                summary: format!("Удалена роль «{}»", current.name),
                details: Some(json!({
                    "id": id,
                    "name": current.name,
                    "permissions": parse_permissions(&current.permissions),
                })),
            },
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
}
