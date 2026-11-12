use chrono::NaiveDateTime;
use std::collections::{HashMap, HashSet};

use mw_db::entity::{client, client_phone};
use mw_domain::permission::Permission;
use mw_domain::phone::{normalize_phone, phone_key};
use mw_domain::search::{index_text, like_contains, search_tokens};
use mw_domain::validation::FieldError;
use sea_orm::ExprTrait;
use sea_orm::sea_query::{Expr, Query};
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

/// Сколько телефонов можно записать клиенту.
pub const MAX_CLIENT_PHONES: usize = 10;

/// Телефон клиента с пометкой («жена», «раб»); первый в списке основной.
#[derive(
    Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema,
)]
#[serde(deny_unknown_fields)]
pub struct ClientPhone {
    /// Номер; при записи остаются только цифры.
    pub phone: String,
    /// Пометка к номеру.
    pub note: String,
}

/// Клиент.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct Client {
    /// Номер клиента.
    pub id: i64,
    /// ФИО клиента.
    pub full_name: String,
    /// Телефоны; первый — основной.
    pub phones: Vec<ClientPhone>,
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

impl Client {
    fn from_parts(m: client::Model, phones: Vec<ClientPhone>) -> Self {
        Self {
            id: m.id,
            full_name: m.full_name,
            phones,
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
    /// Телефоны; первый — основной.
    pub phones: Vec<ClientPhone>,
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
    /// Телефоны; список заменяется целиком.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub phones: Option<Vec<ClientPhone>>,
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

fn search_text(full_name: &str, phones: &[ClientPhone]) -> String {
    let mut parts: Vec<&str> = vec![full_name];
    parts.extend(phones.iter().map(|p| p.phone.as_str()));
    index_text(&parts)
}

/// Оставляет в номерах цифры, обрезает пометки, отбрасывает записи без цифр.
fn clean_phones(raw: &[ClientPhone]) -> Vec<ClientPhone> {
    raw.iter()
        .filter_map(|p| {
            let digits = normalize_phone(p.phone.trim());
            (!digits.is_empty()).then(|| ClientPhone {
                phone: digits,
                note: p.note.trim().to_owned(),
            })
        })
        .collect()
}

/// Проверяет ФИО и список телефонов; возвращает очищенный список (только цифры, без пустых записей).
pub(crate) fn validate_client(
    full_name: &str,
    raw: &[ClientPhone],
) -> Result<Vec<ClientPhone>, Vec<FieldError>> {
    let mut errors = Vec::new();
    let phones = clean_phones(raw);
    let invalid = raw
        .iter()
        .any(|p| normalize_phone(p.phone.trim()).is_empty() && !p.note.trim().is_empty());
    if full_name.trim().is_empty() && phones.is_empty() {
        errors.push(FieldError::required("full_name"));
    }
    if invalid {
        errors.push(FieldError::new("phones", "invalid"));
    } else if phones.len() > MAX_CLIENT_PHONES {
        errors.push(FieldError::new("phones", "out_of_range"));
    } else {
        let mut seen = HashSet::new();
        if !phones.iter().all(|p| seen.insert(phone_key(&p.phone))) {
            errors.push(FieldError::new("phones", "duplicate"));
        }
    }
    if errors.is_empty() {
        Ok(phones)
    } else {
        Err(errors)
    }
}

/// Номера клиентов одним запросом, в порядке позиций.
async fn load_phones(
    conn: &impl ConnectionTrait,
    ids: &[i64],
) -> Result<HashMap<i64, Vec<ClientPhone>>, CoreError> {
    let mut map: HashMap<i64, Vec<ClientPhone>> = HashMap::new();
    if ids.is_empty() {
        return Ok(map);
    }
    let rows = client_phone::Entity::find()
        .filter(client_phone::Column::ClientId.is_in(ids.iter().copied()))
        .order_by_asc(client_phone::Column::ClientId)
        .order_by_asc(client_phone::Column::Position)
        .all(conn)
        .await?;
    for r in rows {
        map.entry(r.client_id).or_default().push(ClientPhone {
            phone: r.phone,
            note: r.note,
        });
    }
    Ok(map)
}

/// Клиенты с номерами; порядок сохраняется.
async fn with_phones(
    conn: &impl ConnectionTrait,
    models: Vec<client::Model>,
) -> Result<Vec<Client>, CoreError> {
    let ids: Vec<i64> = models.iter().map(|m| m.id).collect();
    let mut map = load_phones(conn, &ids).await?;
    Ok(models
        .into_iter()
        .map(|m| {
            let phones = map.remove(&m.id).unwrap_or_default();
            Client::from_parts(m, phones)
        })
        .collect())
}

/// Заменяет строки `client_phones` клиента.
async fn write_phones(
    conn: &impl ConnectionTrait,
    client_id: i64,
    phones: &[ClientPhone],
) -> Result<(), CoreError> {
    client_phone::Entity::delete_many()
        .filter(client_phone::Column::ClientId.eq(client_id))
        .exec(conn)
        .await?;
    if phones.is_empty() {
        return Ok(());
    }
    client_phone::Entity::insert_many(phones.iter().enumerate().map(|(i, p)| {
        client_phone::ActiveModel {
            id: NotSet,
            client_id: Set(client_id),
            position: Set(i as i32),
            phone: Set(p.phone.clone()),
            phone_key: Set(phone_key(&p.phone)),
            note: Set(p.note.clone()),
        }
    }))
    .exec(conn)
    .await?;
    Ok(())
}

/// Общая вставка клиента без проверки прав и журнала (используется и при приёме заказа).
pub(crate) async fn insert_client(
    conn: &impl ConnectionTrait,
    now: NaiveDateTime,
    new: NewClient,
) -> Result<Client, CoreError> {
    let full_name = new.full_name.trim().to_owned();
    let phones = validate_client(&full_name, &new.phones).map_err(CoreError::Validation)?;
    let main = phones.first().map(|p| p.phone.clone()).unwrap_or_default();
    let model = client::ActiveModel {
        id: NotSet,
        search_text: Set(search_text(&full_name, &phones)),
        phone_key: Set(phone_key(&main)),
        full_name: Set(full_name),
        phone: Set(main),
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
    .await?;
    write_phones(conn, model.id, &phones).await?;
    Ok(Client::from_parts(model, phones))
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
        let name = display_name(&model.full_name, &model.phones).to_owned();
        write_audit(
            &tx,
            self.now(),
            actor,
            NewAuditEntry {
                action: "client.created",
                order_id: None,
                client_id: Some(model.id),
                summary: format!("Добавлен клиент «{}»", name),
                details: Some(json!({ "id": model.id })),
            },
        )
        .await?;
        tx.commit().await?;
        Ok(model)
    }

    pub async fn get_client(&self, _actor: &Actor, id: i64) -> Result<Client, CoreError> {
        let model = find_live(&self.db, id)
            .await?
            .ok_or(CoreError::NotFound { entity: ENTITY, id })?;
        Ok(with_phones(&self.db, vec![model]).await?.remove(0))
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
        let current_phones = load_phones(&tx, &[id])
            .await?
            .remove(&id)
            .unwrap_or_default();
        let raw_phones = patch.phones.unwrap_or_else(|| current_phones.clone());
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

        let validated = validate_client(&full_name, &raw_phones);
        let phones = validated
            .clone()
            .unwrap_or_else(|_| clean_phones(&raw_phones));
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

        note_change("phones", phones != current_phones, json!(phones));
        note_change("address", address != current.address, json!(address));
        note_change("note", note != current.note, json!(note));
        note_change("source", source != current.source, json!(source));
        note_change(
            "blacklisted",
            blacklisted != current.blacklisted,
            json!(blacklisted),
        );
        if changed.is_empty() {
            return Ok(Client::from_parts(current, current_phones));
        }
        if current.version != expected_version {
            return Err(CoreError::Conflict {
                entity: ENTITY,
                id,
                current_version: current.version,
            });
        }
        let phones = validated.map_err(CoreError::Validation)?;
        let summary = format!(
            "Изменён клиент «{}»: {}",
            display_name(&current.full_name, &current_phones),
            names.join(", ")
        );

        let now = self.now();
        let main = phones.first().map(|p| p.phone.clone()).unwrap_or_default();
        let res = client::Entity::update_many()
            .col_expr(client::Column::FullName, Expr::value(full_name.clone()))
            .col_expr(client::Column::Phone, Expr::value(main.clone()))
            .col_expr(client::Column::PhoneKey, Expr::value(phone_key(&main)))
            .col_expr(client::Column::Address, Expr::value(address))
            .col_expr(client::Column::Note, Expr::value(note))
            .col_expr(client::Column::Source, Expr::value(source))
            .col_expr(client::Column::Blacklisted, Expr::value(blacklisted))
            .col_expr(
                client::Column::SearchText,
                Expr::value(search_text(&full_name, &phones)),
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
        write_phones(&tx, id, &phones).await?;
        let updated = find_live(&tx, id)
            .await?
            .ok_or(CoreError::NotFound { entity: ENTITY, id })?;
        tx.commit().await?;
        Ok(Client::from_parts(updated, phones))
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
        let models = select
            .order_by_asc(text_sort(
                self.db.get_database_backend(),
                client::Column::SearchText.into_expr(),
            ))
            .order_by_asc(client::Column::Id)
            .limit(limit)
            .offset(offset)
            .all(&self.db)
            .await?;
        let items = with_phones(&self.db, models).await?;
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
        let by_key = Query::select()
            .column(client_phone::Column::ClientId)
            .from(client_phone::Entity)
            .and_where(Expr::col(client_phone::Column::PhoneKey).eq(key))
            .to_owned();
        let models = client::Entity::find()
            .filter(client::Column::DeletedAt.is_null())
            .filter(client::Column::Id.in_subquery(by_key))
            .order_by_asc(text_sort(
                self.db.get_database_backend(),
                client::Column::SearchText.into_expr(),
            ))
            .order_by_asc(client::Column::Id)
            .all(&self.db)
            .await?;
        with_phones(&self.db, models).await
    }
}

fn display_name<'a>(full_name: &'a str, phones: &'a [ClientPhone]) -> &'a str {
    if full_name.is_empty() {
        phones.first().map_or("", |p| p.phone.as_str())
    } else {
        full_name
    }
}
