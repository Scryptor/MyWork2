use chrono::NaiveDateTime;
use mw_db::entity::client;
use mw_domain::permission::Permission;
use mw_domain::phone::{normalize_phone, phone_key};
use mw_domain::search::{index_text, like_contains, search_tokens};
use mw_domain::validation::FieldError;
use sea_orm::ExprTrait;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, NotSet, PaginatorTrait,
    QueryFilter, QueryOrder, QuerySelect, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::page::validate_page;
use crate::sql::text_sort;
use crate::{Actor, Core, CoreError, NewAuditEntry, Page, write_audit};

const ENTITY: &str = "клиент";

/// Клиент.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct Client {
    /// Номер клиента.
    pub id: i64,
    /// ФИО клиента.
    pub full_name: String,
    /// Телефон.
    pub phone: String,
    /// Адрес.
    pub address: String,
    /// Заметка.
    pub note: String,
    /// Откуда клиент узнал о сервисе.
    pub source: String,
    /// Клиент в чёрном списке.
    pub blacklisted: bool,
    /// Когда создано (местное время предприятия).
    pub created_at: NaiveDateTime,
    /// Когда изменено (местное время предприятия).
    pub updated_at: NaiveDateTime,
    /// Версия записи для проверки конфликтов правок.
    pub version: i64,
}

impl From<client::Model> for Client {
    fn from(m: client::Model) -> Self {
        Self {
            id: m.id,
            full_name: m.full_name,
            phone: m.phone,
            address: m.address,
            note: m.note,
            source: m.source,
            blacklisted: m.blacklisted,
            created_at: m.created_at,
            updated_at: m.updated_at,
            version: m.version,
        }
    }
}

/// Данные нового клиента.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema,
)]
#[serde(deny_unknown_fields)]
pub struct NewClient {
    /// ФИО клиента.
    pub full_name: String,
    /// Телефон.
    pub phone: String,
    /// Адрес.
    pub address: String,
    /// Заметка.
    pub note: String,
    /// Откуда клиент узнал о сервисе.
    pub source: String,
    /// Клиент в чёрном списке.
    pub blacklisted: bool,
}

/// Частичное изменение клиента; отсутствующее поле не меняется.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, ts_rs::TS, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
#[ts(optional_fields)]
pub struct ClientPatch {
    /// ФИО клиента.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub full_name: Option<String>,
    /// Телефон.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub phone: Option<String>,
    /// Адрес.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub address: Option<String>,
    /// Заметка.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub note: Option<String>,
    /// Откуда клиент узнал о сервисе.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub source: Option<String>,
    /// Клиент в чёрном списке.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub blacklisted: Option<bool>,
}

fn search_text(full_name: &str, phone: &str) -> String {
    index_text(&[full_name, phone])
}

fn required_name() -> CoreError {
    CoreError::Validation(vec![FieldError::required("full_name")])
}

/// Общая вставка клиента без проверки прав и журнала (используется и при приёме заказа).
pub(crate) async fn insert_client(
    conn: &impl ConnectionTrait,
    now: NaiveDateTime,
    new: NewClient,
) -> Result<client::Model, CoreError> {
    let full_name = new.full_name.trim().to_owned();
    let phone = normalize_phone(new.phone.trim());
    if full_name.is_empty() && phone.is_empty() {
        return Err(required_name());
    }
    Ok(client::ActiveModel {
        id: NotSet,
        search_text: Set(search_text(&full_name, &phone)),
        phone_key: Set(phone_key(&phone)),
        full_name: Set(full_name),
        phone: Set(phone),
        address: Set(new.address.trim().to_owned()),
        note: Set(new.note.trim().to_owned()),
        source: Set(new.source.trim().to_owned()),
        blacklisted: Set(new.blacklisted),
        created_at: Set(now),
        updated_at: Set(now),
        deleted_at: Set(None),
        version: Set(1),
    }
    .insert(conn)
    .await?)
}

pub(crate) async fn find_live(
    conn: &impl ConnectionTrait,
    id: i64,
) -> Result<Option<client::Model>, CoreError> {
    Ok(client::Entity::find_by_id(id)
        .filter(client::Column::DeletedAt.is_null())
        .one(conn)
        .await?)
}

impl Core {
    pub async fn create_client(&self, actor: &Actor, new: NewClient) -> Result<Client, CoreError> {
        actor.require_any(&[Permission::ClientsCreate, Permission::OrdersCreate])?;
        let tx = self.begin_write().await?;
        let model = insert_client(&tx, self.now(), new).await?;
        write_audit(
            &tx,
            self.now(),
            actor,
            NewAuditEntry {
                action: "client.created",
                order_id: None,
                client_id: Some(model.id),
                summary: format!("Добавлен клиент «{}»", display_name(&model)),
                details: Some(json!({ "id": model.id })),
            },
        )
        .await?;
        tx.commit().await?;
        Ok(model.into())
    }

    pub async fn get_client(&self, _actor: &Actor, id: i64) -> Result<Client, CoreError> {
        find_live(&self.db, id)
            .await?
            .map(Client::from)
            .ok_or(CoreError::NotFound { entity: ENTITY, id })
    }

    pub async fn update_client(
        &self,
        actor: &Actor,
        id: i64,
        expected_version: i64,
        patch: ClientPatch,
    ) -> Result<Client, CoreError> {
        actor.require_any(&[Permission::ClientsEdit, Permission::OrdersEdit])?;
        let tx = self.begin_write().await?;
        let current = find_live(&tx, id)
            .await?
            .ok_or(CoreError::NotFound { entity: ENTITY, id })?;
        let full_name = patch
            .full_name
            .map_or_else(|| current.full_name.clone(), |v| v.trim().to_owned());
        let phone = patch
            .phone
            .map_or_else(|| current.phone.clone(), |v| normalize_phone(v.trim()));
        let address = patch
            .address
            .map_or_else(|| current.address.clone(), |v| v.trim().to_owned());
        let note = patch
            .note
            .map_or_else(|| current.note.clone(), |v| v.trim().to_owned());
        let source = patch
            .source
            .map_or_else(|| current.source.clone(), |v| v.trim().to_owned());
        let blacklisted = patch.blacklisted.unwrap_or(current.blacklisted);

        let mut changed = Map::new();
        let mut names: Vec<&str> = Vec::new();
        let mut note_change = |field: &'static str, differs: bool, value: Value| {
            if differs {
                changed.insert(field.to_owned(), value);
                names.push(field);
            }
        };
        note_change(
            "full_name",
            full_name != current.full_name,
            json!(full_name),
        );
        note_change("phone", phone != current.phone, json!(phone));
        note_change("address", address != current.address, json!(address));
        note_change("note", note != current.note, json!(note));
        note_change("source", source != current.source, json!(source));
        note_change(
            "blacklisted",
            blacklisted != current.blacklisted,
            json!(blacklisted),
        );
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
        if full_name.is_empty() && phone.is_empty() {
            return Err(required_name());
        }
        let summary = format!(
            "Изменён клиент «{}»: {}",
            display_name(&current),
            names.join(", ")
        );

        let now = self.now();
        let res = client::Entity::update_many()
            .col_expr(client::Column::FullName, Expr::value(full_name.clone()))
            .col_expr(client::Column::Phone, Expr::value(phone.clone()))
            .col_expr(client::Column::PhoneKey, Expr::value(phone_key(&phone)))
            .col_expr(client::Column::Address, Expr::value(address))
            .col_expr(client::Column::Note, Expr::value(note))
            .col_expr(client::Column::Source, Expr::value(source))
            .col_expr(client::Column::Blacklisted, Expr::value(blacklisted))
            .col_expr(
                client::Column::SearchText,
                Expr::value(search_text(&full_name, &phone)),
            )
            .col_expr(client::Column::UpdatedAt, Expr::value(now))
            .col_expr(
                client::Column::Version,
                Expr::col(client::Column::Version).add(1),
            )
            .filter(client::Column::Id.eq(id))
            .filter(client::Column::Version.eq(expected_version))
            .filter(client::Column::DeletedAt.is_null())
            .exec(&tx)
            .await?;
        if res.rows_affected == 0 {
            return Err(match find_live(&tx, id).await? {
                Some(m) => CoreError::Conflict {
                    entity: ENTITY,
                    id,
                    current_version: m.version,
                },
                None => CoreError::NotFound { entity: ENTITY, id },
            });
        }
        write_audit(
            &tx,
            now,
            actor,
            NewAuditEntry {
                action: "client.updated",
                order_id: None,
                client_id: Some(id),
                summary,
                details: Some(Value::Object(changed)),
            },
        )
        .await?;
        let updated = find_live(&tx, id)
            .await?
            .ok_or(CoreError::NotFound { entity: ENTITY, id })?;
        tx.commit().await?;
        Ok(updated.into())
    }

    pub async fn find_clients(
        &self,
        actor: &Actor,
        query: &str,
        limit: u64,
        offset: u64,
    ) -> Result<Page<Client>, CoreError> {
        actor.require_any(&[Permission::ClientsView, Permission::OrdersCreate])?;
        validate_page(limit, offset)?;
        let mut select = client::Entity::find().filter(client::Column::DeletedAt.is_null());
        for token in search_tokens(query) {
            select = select.filter(
                Expr::col(client::Column::SearchText)
                    .like(sea_orm::sea_query::LikeExpr::new(like_contains(&token)).escape('\\')),
            );
        }
        let total = select.clone().count(&self.db).await?;
        let items = select
            .order_by_asc(text_sort(
                self.db.get_database_backend(),
                client::Column::SearchText.into_expr(),
            ))
            .order_by_asc(client::Column::Id)
            .limit(limit)
            .offset(offset)
            .all(&self.db)
            .await?
            .into_iter()
            .map(Client::from)
            .collect();
        Ok(Page { items, total })
    }

    pub async fn find_clients_by_phone(
        &self,
        actor: &Actor,
        phone: &str,
    ) -> Result<Vec<Client>, CoreError> {
        actor.require_any(&[Permission::ClientsView, Permission::OrdersCreate])?;
        let key = phone_key(phone);
        if key.is_empty() {
            return Ok(Vec::new());
        }
        Ok(client::Entity::find()
            .filter(client::Column::DeletedAt.is_null())
            .filter(client::Column::PhoneKey.eq(key))
            .order_by_asc(text_sort(
                self.db.get_database_backend(),
                client::Column::SearchText.into_expr(),
            ))
            .order_by_asc(client::Column::Id)
            .all(&self.db)
            .await?
            .into_iter()
            .map(Client::from)
            .collect())
    }
}

fn display_name(m: &client::Model) -> &str {
    if m.full_name.is_empty() {
        &m.phone
    } else {
        &m.full_name
    }
}
