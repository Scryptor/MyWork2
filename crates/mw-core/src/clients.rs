use chrono::NaiveDateTime;
use std::collections::{HashMap, HashSet};

use mw_db::entity::{client, client_phone, order};
use mw_domain::permission::Permission;
use mw_domain::phone::{normalize_phone, phone_key};
use mw_domain::search::{like_contains, normalize_search, search_tokens};
use mw_domain::validation::FieldError;
use sea_orm::ExprTrait;
use sea_orm::sea_query::{Expr, Order, Query};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseTransaction, EntityTrait, NotSet,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Select, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::page::validate_page;
use crate::search_index::client_search_text;
use crate::sql::text_sort;
use crate::{Actor, Core, CoreError, EditVia, NewAuditEntry, Page, write_audit};

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
    pub(crate) fn from_parts(m: client::Model, phones: Vec<ClientPhone>) -> Self {
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

pub(crate) fn search_text(full_name: &str, phones: &[ClientPhone]) -> String {
    let numbers: Vec<String> = phones.iter().map(|p| p.phone.clone()).collect();
    client_search_text(full_name, &numbers)
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
pub(crate) async fn load_phones(
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
pub(crate) async fn write_phones(
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
        name_key: Set(normalize_search(&full_name)),
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

/// Поле сортировки списка клиентов.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ClientSortField {
    /// Имя (по нормализованному ключу).
    Name,
    /// Дата последнего заказа (по умолчанию); клиенты без заказов в конце.
    #[default]
    LastOrder,
    /// Число заказов.
    OrdersCount,
}

/// Порядок списка клиентов.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientSort {
    pub field: ClientSortField,
    pub descending: bool,
}

impl Default for ClientSort {
    fn default() -> Self {
        Self {
            field: ClientSortField::LastOrder,
            descending: true,
        }
    }
}

/// Условия списка клиентов.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClientFilter {
    /// Слова поиска по имени и телефонам; пусто — все.
    pub query: String,
    /// Только клиенты, у которых есть похожий (порядок тогда по ключу имени).
    pub duplicates: bool,
    pub sort: ClientSort,
}

/// Строка списка клиентов.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct ClientListItem {
    /// Номер клиента.
    pub id: i64,
    /// ФИО клиента.
    pub full_name: String,
    /// Телефоны; первый — основной.
    pub phones: Vec<ClientPhone>,
    /// Откуда клиент узнал о сервисе.
    pub source: String,
    /// Клиент в чёрном списке.
    pub blacklisted: bool,
    /// Сколько у клиента заказов (удалённые не считаются).
    pub orders_count: u64,
    /// Когда принят последний заказ (местное время предприятия); пусто, если заказов нет.
    #[schema(required = true)]
    pub last_order_at: Option<NaiveDateTime>,
}

/// Число заказов и дата последнего у каждого клиента из списка.
async fn order_stats(
    conn: &impl ConnectionTrait,
    ids: &[i64],
) -> Result<HashMap<i64, (u64, Option<NaiveDateTime>)>, CoreError> {
    let mut map = HashMap::new();
    if ids.is_empty() {
        return Ok(map);
    }
    let rows = order::Entity::find()
        .select_only()
        .column(order::Column::ClientId)
        .expr_as(Expr::col(order::Column::Id).count(), "n")
        .expr_as(Expr::col(order::Column::ReceivedAt).max(), "last")
        .filter(order::Column::ClientId.is_in(ids.iter().copied()))
        .filter(order::Column::DeletedAt.is_null())
        .group_by(order::Column::ClientId)
        .into_tuple::<(i64, i64, Option<NaiveDateTime>)>()
        .all(conn)
        .await?;
    for (id, n, last) in rows {
        map.insert(id, (n as u64, last));
    }
    Ok(map)
}

/// Строки списка из моделей; порядок сохраняется.
pub(crate) async fn list_items(
    conn: &impl ConnectionTrait,
    models: Vec<client::Model>,
) -> Result<Vec<ClientListItem>, CoreError> {
    let ids: Vec<i64> = models.iter().map(|m| m.id).collect();
    let mut phones = load_phones(conn, &ids).await?;
    let stats = order_stats(conn, &ids).await?;
    Ok(models
        .into_iter()
        .map(|m| {
            let (orders_count, last_order_at) = stats.get(&m.id).copied().unwrap_or((0, None));
            ClientListItem {
                id: m.id,
                phones: phones.remove(&m.id).unwrap_or_default(),
                full_name: m.full_name,
                source: m.source,
                blacklisted: m.blacklisted,
                orders_count,
                last_order_at,
            }
        })
        .collect())
}

/// Подзапросы для сортировки: число заказов и дата последнего заказа клиента.
const ORDERS_COUNT_SQL: &str =
    "(SELECT COUNT(*) FROM orders o WHERE o.client_id = clients.id AND o.deleted_at IS NULL)";
const LAST_ORDER_SQL: &str = "(SELECT MAX(o.received_at) FROM orders o WHERE o.client_id = clients.id AND o.deleted_at IS NULL)";

/// У клиента есть другой живой клиент с тем же непустым ключом имени или общим телефоном
/// (сравниваются `phone_key` — последние 10 цифр; номера короче 6 цифр не считаются).
/// Два отдельных `EXISTS`, каждый по своему индексу (`name_key`, `phone_key`): общий подзапрос
/// по всем клиентам делал фильтр квадратичным.
const HAS_SIMILAR_SQL: &str = "((clients.name_key <> '' AND EXISTS (SELECT 1 FROM clients c2 \
        WHERE c2.name_key = clients.name_key AND c2.id <> clients.id AND c2.deleted_at IS NULL)) \
    OR EXISTS (SELECT 1 FROM client_phones p1 \
        JOIN client_phones p2 ON p2.phone_key = p1.phone_key AND p2.client_id <> p1.client_id \
        JOIN clients c2 ON c2.id = p2.client_id AND c2.deleted_at IS NULL \
        WHERE p1.client_id = clients.id AND LENGTH(p1.phone_key) >= 6))";

pub(crate) async fn find_live(
    conn: &impl ConnectionTrait,
    id: i64,
) -> Result<Option<client::Model>, CoreError> {
    Ok(client::Entity::find_by_id(id)
        .filter(client::Column::DeletedAt.is_null())
        .one(conn)
        .await?)
}

/// Новые значения полей клиента.
pub(crate) struct ClientFields<'a> {
    pub full_name: &'a str,
    pub phones: &'a [ClientPhone],
    pub address: &'a str,
    pub note: &'a str,
    pub source: &'a str,
    pub blacklisted: bool,
}

/// Записывает поля клиента (и производные: основной телефон, `search_text`, `name_key`) с проверкой
/// версии; версия растёт на 1. Устаревшая версия — `Conflict`, нет клиента — `NotFound`.
/// Строки `client_phones` не трогает.
pub(crate) async fn apply_client_fields(
    conn: &impl ConnectionTrait,
    id: i64,
    expected_version: i64,
    now: NaiveDateTime,
    f: ClientFields<'_>,
) -> Result<(), CoreError> {
    let main = f
        .phones
        .first()
        .map(|p| p.phone.clone())
        .unwrap_or_default();
    let res = client::Entity::update_many()
        .col_expr(client::Column::FullName, Expr::value(f.full_name))
        .col_expr(client::Column::Phone, Expr::value(main.clone()))
        .col_expr(client::Column::PhoneKey, Expr::value(phone_key(&main)))
        .col_expr(client::Column::Address, Expr::value(f.address))
        .col_expr(client::Column::Note, Expr::value(f.note))
        .col_expr(client::Column::Source, Expr::value(f.source))
        .col_expr(client::Column::Blacklisted, Expr::value(f.blacklisted))
        .col_expr(
            client::Column::SearchText,
            Expr::value(search_text(f.full_name, f.phones)),
        )
        .col_expr(
            client::Column::NameKey,
            Expr::value(normalize_search(f.full_name)),
        )
        .col_expr(client::Column::UpdatedAt, Expr::value(now))
        .col_expr(
            client::Column::Version,
            Expr::col(client::Column::Version).add(1),
        )
        .filter(client::Column::Id.eq(id))
        .filter(client::Column::Version.eq(expected_version))
        .filter(client::Column::DeletedAt.is_null())
        .exec(conn)
        .await?;
    if res.rows_affected == 0 {
        return Err(match find_live(conn, id).await? {
            Some(m) => CoreError::Conflict {
                entity: ENTITY,
                id,
                current_version: m.version,
            },
            None => CoreError::NotFound { entity: ENTITY, id },
        });
    }
    Ok(())
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
                stock_item_id: None,
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
        let (updated, _) = self
            .update_client_in(&tx, actor, id, expected_version, patch, None)
            .await?;
        tx.commit().await?;
        Ok(updated)
    }

    /// Правка клиента внутри открытой транзакции записи: всё, что делает `update_client`, кроме
    /// проверки прав и фиксации (замена источника клиента из справочника идёт через неё же, с
    /// пометкой `via`). Возвращает клиента и признак, что запись изменилась.
    pub(crate) async fn update_client_in(
        &self,
        tx: &DatabaseTransaction,
        actor: &Actor,
        id: i64,
        expected_version: i64,
        patch: ClientPatch,
        via: Option<EditVia>,
    ) -> Result<(Client, bool), CoreError> {
        let current = find_live(tx, id)
            .await?
            .ok_or(CoreError::NotFound { entity: ENTITY, id })?;
        let full_name = patch
            .full_name
            .map_or_else(|| current.full_name.clone(), |v| v.trim().to_owned());
        let current_phones = load_phones(tx, &[id])
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
            // Пустая правка допустима, только если исходные данные корректны:
            // запись без цифр с пометкой не должна пропадать молча.
            if let Err(errors) = &validated
                && errors.iter().any(|e| e.field == "phones")
            {
                return Err(CoreError::Validation(errors.clone()));
            }
            return Ok((Client::from_parts(current, current_phones), false));
        }
        if current.version != expected_version {
            return Err(CoreError::Conflict {
                entity: ENTITY,
                id,
                current_version: current.version,
            });
        }
        let phones = validated.map_err(CoreError::Validation)?;
        let mut summary = format!(
            "Изменён клиент «{}»: {}",
            display_name(&current.full_name, &current_phones),
            names.join(", ")
        );
        if let Some(via) = via {
            summary.push_str(via.suffix_ru());
        }

        let now = self.now();
        apply_client_fields(
            tx,
            id,
            expected_version,
            now,
            ClientFields {
                full_name: &full_name,
                phones: &phones,
                address: &address,
                note: &note,
                source: &source,
                blacklisted,
            },
        )
        .await?;
        write_audit(
            tx,
            now,
            actor,
            NewAuditEntry {
                action: "client.updated",
                order_id: None,
                client_id: Some(id),
                stock_item_id: None,
                summary,
                details: Some(Value::Object(changed)),
            },
        )
        .await?;
        write_phones(tx, id, &phones).await?;
        let updated = find_live(tx, id)
            .await?
            .ok_or(CoreError::NotFound { entity: ENTITY, id })?;
        Ok((Client::from_parts(updated, phones), true))
    }

    pub async fn list_clients(
        &self,
        actor: &Actor,
        filter: &ClientFilter,
        limit: u64,
        offset: u64,
    ) -> Result<Page<ClientListItem>, CoreError> {
        actor.require_any(&[Permission::ClientsView, Permission::OrdersCreate])?;
        validate_page(limit, offset)?;
        let select = client_filtered(filter);
        let total = select.clone().count(&self.db).await?;
        let models = client_ordered(select, self.db.get_database_backend(), filter)
            .limit(limit)
            .offset(offset)
            .all(&self.db)
            .await?;
        let items = list_items(&self.db, models).await?;
        Ok(Page { items, total })
    }

    /// Поиск клиентов по словам в порядке по умолчанию.
    pub async fn find_clients(
        &self,
        actor: &Actor,
        query: &str,
        limit: u64,
        offset: u64,
    ) -> Result<Page<ClientListItem>, CoreError> {
        let filter = ClientFilter {
            query: query.to_owned(),
            ..Default::default()
        };
        self.list_clients(actor, &filter, limit, offset).await
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

/// Условия списка клиентов; общие для списка и выгрузки.
pub(crate) fn client_filtered(filter: &ClientFilter) -> Select<client::Entity> {
    let mut select = client::Entity::find().filter(client::Column::DeletedAt.is_null());
    for token in search_tokens(&filter.query) {
        select = select.filter(
            Expr::col(client::Column::SearchText)
                .like(sea_orm::sea_query::LikeExpr::new(like_contains(&token)).escape('\\')),
        );
    }
    if filter.duplicates {
        select = select.filter(Expr::cust(HAS_SIMILAR_SQL));
    }
    select
}

/// Порядок списка и выгрузки клиентов; последние ключи — поисковый текст и номер.
pub(crate) fn client_ordered(
    select: Select<client::Entity>,
    backend: sea_orm::DatabaseBackend,
    filter: &ClientFilter,
) -> Select<client::Entity> {
    let dir = |descending: bool| if descending { Order::Desc } else { Order::Asc };
    let select = if filter.duplicates {
        select.order_by_asc(text_sort(backend, client::Column::NameKey.into_expr()))
    } else {
        let s = filter.sort;
        match s.field {
            ClientSortField::Name => select.order_by(
                text_sort(backend, client::Column::NameKey.into_expr()),
                dir(s.descending),
            ),
            ClientSortField::OrdersCount => {
                select.order_by(Expr::cust(ORDERS_COUNT_SQL), dir(s.descending))
            }
            ClientSortField::LastOrder => select
                .order_by(Expr::cust(format!("{LAST_ORDER_SQL} IS NULL")), Order::Asc)
                .order_by(Expr::cust(LAST_ORDER_SQL), dir(s.descending)),
        }
    };
    select
        .order_by_asc(text_sort(backend, client::Column::SearchText.into_expr()))
        .order_by_asc(client::Column::Id)
}

pub(crate) fn display_name<'a>(full_name: &'a str, phones: &'a [ClientPhone]) -> &'a str {
    if full_name.is_empty() {
        phones.first().map_or("", |p| p.phone.as_str())
    } else {
        full_name
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use sea_orm::{ConnectionTrait, DbBackend, Statement};

    use super::HAS_SIMILAR_SQL;

    /// План SQLite для фильтра «Похожие»: строки `detail` из `EXPLAIN QUERY PLAN`.
    async fn sqlite_plan(sql: &str) -> Vec<String> {
        let db = mw_db::testing::sqlite_memory().await;
        db.query_all_raw(Statement::from_string(
            DbBackend::Sqlite,
            format!("EXPLAIN QUERY PLAN {sql}"),
        ))
        .await
        .unwrap()
        .iter()
        .map(|row| row.try_get_by_index::<String>(3).unwrap())
        .collect()
    }

    /// Каждый клиент ищет похожих по индексам, а не перебором всех клиентов: перебор внутри
    /// `EXISTS` делал фильтр квадратичным (секунды на тысячах клиентов).
    #[tokio::test]
    async fn similar_filter_uses_indexes_inside_exists() {
        let plan = sqlite_plan(&format!(
            "SELECT clients.id FROM clients WHERE clients.deleted_at IS NULL AND {HAS_SIMILAR_SQL}"
        ))
        .await;
        let text = plan.join("\n");
        let scans: Vec<&String> = plan
            .iter()
            .filter(|d| d.starts_with("SCAN") && !d.starts_with("SCAN clients"))
            .collect();
        assert!(scans.is_empty(), "перебор в подзапросе:\n{text}");
        assert_eq!(
            plan.iter()
                .filter(|d| d.starts_with("SCAN clients"))
                .count(),
            1,
            "клиенты перебираются один раз, снаружи:\n{text}"
        );
        assert!(!text.contains("SCAN c2"), "{text}");
    }
}
