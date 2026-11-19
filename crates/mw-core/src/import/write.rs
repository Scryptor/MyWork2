//! Проверка пакета, стирание старых данных и запись в одной транзакции.

use std::collections::{HashMap, HashSet};

use mw_db::entity::{
    attachment, audit_log, client, client_phone, dictionary_item, legacy_import, legacy_row, order,
    order_tracking, role, service_point, session, setting, status, user,
};
use mw_db::ids::{IdTable, set_next_id};
use mw_domain::dictionary::DEFAULT_DICTIONARY;
use mw_domain::permission::Permission;
use mw_domain::phone::{normalize_phone, phone_key};
use mw_domain::search::normalize_search;
use mw_domain::status::StatusKind;
use mw_domain::validation::FieldError;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseBackend, EntityTrait, NotSet,
    PaginatorTrait, QueryFilter, Set,
};
use serde_json::{Value, json};

use super::{
    DatabaseKind, ImportBundle, ImportMode, ImportSummary, ImportTarget, ImportedClient,
    LegacyTarget,
};
use crate::auth::PasswordScheme;
use crate::clients::validate_client;
use crate::orders::warranty_columns;
use crate::search_index::{
    SEARCH_INDEX_VERSION, client_search_text, order_search_text, set_order_keys,
};
use crate::settings::{self, AppSettings, put};
use crate::{Actor, Core, CoreError};

/// Больше параметров на один запрос не отправляем (предел PostgreSQL — 65 535).
const MAX_BIND_PARAMS: usize = 30_000;

fn bad(message: impl AsRef<str>) -> CoreError {
    CoreError::Internal(format!("импорт: {}", message.as_ref()))
}

/// Пустой ли список названий-дублей по `normalize_search`.
fn check_unique_names<'a>(
    what: &str,
    names: impl IntoIterator<Item = &'a str>,
) -> Result<(), CoreError> {
    let mut seen = HashSet::new();
    for name in names {
        if normalize_search(name).is_empty() {
            return Err(bad(format!("пустое название: {what}")));
        }
        if !seen.insert(normalize_search(name)) {
            return Err(bad(format!("{what} с одинаковым названием")));
        }
    }
    Ok(())
}

/// Очищенные номера клиента пакета; нарушение правил — ошибка импорта.
fn clean_client_phones(c: &ImportedClient) -> Result<Vec<crate::ClientPhone>, CoreError> {
    if c.phones
        .iter()
        .any(|p| normalize_phone(&p.phone).is_empty())
    {
        return Err(bad(format!("у клиента №{} номер без цифр", c.id)));
    }
    validate_client(&c.full_name, &c.phones).map_err(|errors| {
        let list: Vec<String> = errors
            .iter()
            .map(|e| format!("{}: {}", e.field, e.code))
            .collect();
        bad(format!("клиент №{}: {}", c.id, list.join(", ")))
    })
}

/// Проверка пакета до любых записей; все нарушения — `Internal`.
fn validate_bundle(b: &ImportBundle) -> Result<(), CoreError> {
    check_unique_names("статусы", b.statuses.iter().map(|s| s.name.as_str()))?;
    if !b.statuses.iter().any(|s| s.kind == StatusKind::Issued) {
        return Err(bad("нет статуса вида «Выдан»"));
    }
    check_unique_names("точки", b.service_points.iter().map(String::as_str))?;
    check_unique_names("роли", b.roles.iter().map(|r| r.name.as_str()))?;

    let mut kinds = HashSet::new();
    for (kind, values) in &b.dictionary {
        if !kinds.insert(*kind) {
            return Err(bad(format!(
                "справочник {} встречается дважды",
                kind.as_str()
            )));
        }
        let mut seen = HashSet::new();
        if !values.iter().all(|v| seen.insert(v.as_str())) {
            return Err(bad(format!("повторы в справочнике {}", kind.as_str())));
        }
    }
    if DEFAULT_DICTIONARY.iter().any(|(k, _)| !kinds.contains(k)) {
        return Err(bad("в пакете не все справочники"));
    }

    let role_names: HashSet<&str> = b.roles.iter().map(|r| r.name.as_str()).collect();
    let mut logins = HashSet::new();
    for u in &b.users {
        if normalize_search(&u.login).is_empty() || !logins.insert(normalize_search(&u.login)) {
            return Err(bad("логины пустые или совпадают без учёта регистра"));
        }
        if !role_names.contains(u.role.as_str()) {
            return Err(bad(format!("у пользователя нет роли «{}»", u.role)));
        }
    }
    if !b.users.is_empty()
        && !b.users.iter().any(|u| {
            b.roles
                .iter()
                .any(|r| r.name == u.role && r.permissions.contains(Permission::UsersManage))
        })
    {
        return Err(bad(
            "ни у одного пользователя нет права «Пользователи и права»",
        ));
    }

    let mut client_ids = HashSet::new();
    for c in &b.clients {
        if c.id <= 0 || !client_ids.insert(c.id) {
            return Err(bad(format!(
                "номер клиента {} неверен или повторяется",
                c.id
            )));
        }
        clean_client_phones(c)?;
    }
    let status_names: HashSet<&str> = b.statuses.iter().map(|s| s.name.as_str()).collect();
    let point_names: HashSet<&str> = b.service_points.iter().map(String::as_str).collect();
    let mut order_ids = HashSet::new();
    for o in &b.orders {
        if o.id <= 0 || !order_ids.insert(o.id) {
            return Err(bad(format!(
                "номер заказа {} неверен или повторяется",
                o.id
            )));
        }
        if !client_ids.contains(&o.client_id) {
            return Err(bad(format!(
                "заказ №{}: нет клиента №{}",
                o.id, o.client_id
            )));
        }
        if !status_names.contains(o.status.as_str()) {
            return Err(bad(format!("заказ №{}: нет такого статуса", o.id)));
        }
        if o.service_point
            .as_deref()
            .is_some_and(|p| !point_names.contains(p))
        {
            return Err(bad(format!("заказ №{}: нет такой точки", o.id)));
        }
    }

    for row in &b.legacy_rows {
        let ok = match &row.target {
            None => true,
            Some(LegacyTarget::Order(id)) => order_ids.contains(id),
            Some(LegacyTarget::Client(id)) => client_ids.contains(id),
            Some(LegacyTarget::Role(name)) => role_names.contains(name.as_str()),
            Some(LegacyTarget::User(login)) => b.users.iter().any(|u| &u.login == login),
        };
        if !ok {
            return Err(bad(format!(
                "след {}/{} указывает на несуществующую запись",
                row.source, row.source_key
            )));
        }
    }

    let values = &b.settings.values;
    if values.timezone.is_some() || values.default_service_point_id.is_some() {
        return Err(bad(
            "часовой пояс и точка по умолчанию в пакете не задаются",
        ));
    }
    if b.settings
        .default_service_point
        .as_deref()
        .is_some_and(|p| !point_names.contains(p))
    {
        return Err(bad("точки по умолчанию нет среди точек"));
    }
    settings::validate(values, true).map_err(|e| bad(format!("настройки: {e}")))?;

    if b.next_order_id <= order_ids.iter().copied().max().unwrap_or(0) {
        return Err(bad("следующий номер заказа не больше наибольшего"));
    }
    if b.next_client_id <= client_ids.iter().copied().max().unwrap_or(0) {
        return Err(bad("следующий номер клиента не больше наибольшего"));
    }
    Ok(())
}

/// Состояние приёмника: сколько в нём клиентов, заказов, пользователей и записей журнала.
async fn target_state(conn: &impl ConnectionTrait) -> Result<ImportTarget, CoreError> {
    Ok(ImportTarget {
        clients: client::Entity::find().count(conn).await?,
        orders: order::Entity::find().count(conn).await?,
        users: user::Entity::find().count(conn).await?,
        audit_entries: audit_log::Entity::find().count(conn).await?,
    })
}

/// Вставка пачками: на запрос не больше `MAX_BIND_PARAMS` параметров.
async fn insert_chunked<E>(
    conn: &impl ConnectionTrait,
    rows: Vec<E::ActiveModel>,
) -> Result<(), CoreError>
where
    E: EntityTrait,
    E::ActiveModel: Send,
{
    // Каждая колонка получает параметр: считаем по сущности, а не вручную.
    let columns = <E::Column as sea_orm::Iterable>::iter().count();
    let size = (MAX_BIND_PARAMS / columns.max(1)).max(1);
    let mut rows = rows.into_iter();
    loop {
        let chunk: Vec<_> = rows.by_ref().take(size).collect();
        if chunk.is_empty() {
            return Ok(());
        }
        E::insert_many(chunk).exec_without_returning(conn).await?;
    }
}

async fn wipe(conn: &impl ConnectionTrait) -> Result<(), CoreError> {
    session::Entity::delete_many().exec(conn).await?;
    attachment::Entity::delete_many().exec(conn).await?;
    legacy_row::Entity::delete_many().exec(conn).await?;
    legacy_import::Entity::delete_many().exec(conn).await?;
    order_tracking::Entity::delete_many().exec(conn).await?;
    audit_log::Entity::delete_many().exec(conn).await?;
    order::Entity::delete_many().exec(conn).await?;
    client_phone::Entity::delete_many().exec(conn).await?;
    client::Entity::delete_many().exec(conn).await?;
    user::Entity::delete_many().exec(conn).await?;
    role::Entity::delete_many().exec(conn).await?;
    status::Entity::delete_many().exec(conn).await?;
    dictionary_item::Entity::delete_many().exec(conn).await?;
    service_point::Entity::delete_many().exec(conn).await?;
    setting::Entity::delete_many()
        .filter(setting::Column::Key.ne("timezone"))
        .exec(conn)
        .await?;
    Ok(())
}

/// Справочники первого порядка: статусы, точки, справочники; возвращает карты название → номер.
async fn write_catalogs(
    conn: &impl ConnectionTrait,
    b: &ImportBundle,
) -> Result<(HashMap<String, i64>, HashMap<String, i64>), CoreError> {
    let statuses = b
        .statuses
        .iter()
        .enumerate()
        .map(|(i, s)| status::ActiveModel {
            id: NotSet,
            name: Set(s.name.clone()),
            kind: Set(s.kind.as_str().to_owned()),
            color: Set(s.color.clone()),
            sort_order: Set(i as i32),
            archived: Set(false),
        })
        .collect();
    insert_chunked::<status::Entity>(conn, statuses).await?;
    let points = b
        .service_points
        .iter()
        .enumerate()
        .map(|(i, name)| service_point::ActiveModel {
            id: NotSet,
            name: Set(name.clone()),
            sort_order: Set(i as i32),
            archived: Set(false),
        })
        .collect();
    insert_chunked::<service_point::Entity>(conn, points).await?;
    let items = b
        .dictionary
        .iter()
        .flat_map(|(kind, values)| {
            values
                .iter()
                .enumerate()
                .map(move |(i, value)| dictionary_item::ActiveModel {
                    id: NotSet,
                    kind: Set(kind.as_str().to_owned()),
                    value: Set(value.clone()),
                    sort_order: Set(i as i32),
                    archived: Set(false),
                })
        })
        .collect();
    insert_chunked::<dictionary_item::Entity>(conn, items).await?;

    let status_ids = status::Entity::find()
        .all(conn)
        .await?
        .into_iter()
        .map(|s| (s.name, s.id))
        .collect();
    let point_ids = service_point::Entity::find()
        .all(conn)
        .await?
        .into_iter()
        .map(|p| (p.name, p.id))
        .collect();
    Ok((status_ids, point_ids))
}

/// Роли и пользователи по одному (нужны номера); возвращает карты название → номер и логин → номер.
async fn write_access(
    core: &Core,
    conn: &impl ConnectionTrait,
    b: &ImportBundle,
) -> Result<(HashMap<String, i64>, HashMap<String, i64>), CoreError> {
    let mut role_ids = HashMap::new();
    for r in &b.roles {
        let permissions =
            serde_json::to_string(&r.permissions).map_err(|e| bad(format!("права роли: {e}")))?;
        let m = role::ActiveModel {
            id: NotSet,
            name: Set(r.name.clone()),
            permissions: Set(permissions),
            version: Set(1),
        }
        .insert(conn)
        .await?;
        role_ids.insert(r.name.clone(), m.id);
    }
    let now = core.now();
    let mut user_ids = HashMap::new();
    for u in &b.users {
        let m = user::ActiveModel {
            id: NotSet,
            login: Set(u.login.clone()),
            login_key: Set(normalize_search(&u.login)),
            display_name: Set(u.login.clone()),
            role_id: Set(role_ids[&u.role]),
            password_hash: Set(u.legacy_sha1.clone()),
            password_scheme: Set(PasswordScheme::LegacySha1.as_str().to_owned()),
            active: Set(true),
            created_at: Set(now),
            updated_at: Set(now),
            version: Set(1),
        }
        .insert(conn)
        .await?;
        user_ids.insert(u.login.clone(), m.id);
    }
    Ok((role_ids, user_ids))
}

/// Клиенты и их номера; возвращает число номеров.
async fn write_clients(conn: &impl ConnectionTrait, b: &ImportBundle) -> Result<u64, CoreError> {
    let mut clients = Vec::with_capacity(b.clients.len());
    let mut phones = Vec::new();
    for c in &b.clients {
        let list = clean_client_phones(c)?;
        let numbers: Vec<String> = list.iter().map(|p| p.phone.clone()).collect();
        let main = numbers.first().cloned().unwrap_or_default();
        clients.push(client::ActiveModel {
            id: Set(c.id),
            full_name: Set(c.full_name.clone()),
            phone_key: Set(phone_key(&main)),
            phone: Set(main),
            address: Set(c.address.clone()),
            note: Set(c.note.clone()),
            source: Set(c.source.clone()),
            blacklisted: Set(c.blacklisted),
            created_at: Set(c.created_at),
            updated_at: Set(c.created_at),
            deleted_at: Set(None),
            version: Set(1),
            search_text: Set(client_search_text(&c.full_name, &numbers)),
            name_key: Set(mw_domain::search::normalize_search(&c.full_name)),
        });
        for (i, p) in list.iter().enumerate() {
            phones.push(client_phone::ActiveModel {
                id: NotSet,
                client_id: Set(c.id),
                position: Set(i as i32),
                phone_key: Set(phone_key(&p.phone)),
                phone: Set(p.phone.clone()),
                note: Set(p.note.clone()),
            });
        }
    }
    let phone_count = phones.len() as u64;
    insert_chunked::<client::Entity>(conn, clients).await?;
    insert_chunked::<client_phone::Entity>(conn, phones).await?;
    Ok(phone_count)
}

/// Заказы и трек-номера; возвращает число трек-номеров.
async fn write_orders(
    conn: &impl ConnectionTrait,
    b: &ImportBundle,
    status_ids: &HashMap<String, i64>,
    point_ids: &HashMap<String, i64>,
) -> Result<u64, CoreError> {
    let mut orders = Vec::with_capacity(b.orders.len());
    let mut tracking = Vec::new();
    for o in &b.orders {
        let (warranty_text, warranty_days, warranty_months) = warranty_columns(&o.warranty);
        let mut m = order::Model {
            id: o.id,
            client_id: o.client_id,
            service_point_id: o.service_point.as_ref().map(|p| point_ids[p]),
            status_id: status_ids[&o.status],
            received_at: o.received_at,
            issued_at: o.issued_at,
            prepaid_at: o.prepaid_at,
            device_type: o.device_type.clone(),
            brand: o.brand.clone(),
            model: o.model.clone(),
            serial: o.serial.clone(),
            color: o.color.clone(),
            device_password: String::new(),
            completeness: o.completeness.clone(),
            device_condition: o.device_condition.clone(),
            malfunction: o.malfunction.clone(),
            work_done: o.work_done.clone(),
            internal_comment: o.internal_comment.clone(),
            master: o.master.clone(),
            barcode: o.barcode.clone(),
            estimate_from: o.estimate_from.map(|m| m.kopecks()),
            estimate_to: o.estimate_to.map(|m| m.kopecks()),
            final_price: o.final_price.map(|m| m.kopecks()),
            prepayment: o.prepayment.kopecks(),
            extra_costs: o.extra_costs.kopecks(),
            discount: o.discount.kopecks(),
            warranty_days,
            warranty_text,
            warranty_months,
            need_call: o.need_call,
            need_parts: o.need_parts,
            created_at: o.received_at,
            updated_at: o.received_at,
            deleted_at: o.deleted_at,
            version: 1,
            search_text: String::new(),
            status_changed_at: None,
            device_type_key: String::new(),
            brand_key: String::new(),
            model_key: String::new(),
            master_key: String::new(),
            request_key: None,
        };
        m.search_text = order_search_text(&m);
        set_order_keys(&mut m);
        orders.push(order::ActiveModel::from(m).reset_all());
        for (i, (number, description)) in o.tracking.iter().enumerate() {
            tracking.push(order_tracking::ActiveModel {
                id: NotSet,
                order_id: Set(o.id),
                position: Set(i as i32),
                number: Set(number.clone()),
                description: Set(description.clone()),
            });
        }
    }
    let tracking_count = tracking.len() as u64;
    insert_chunked::<order::Entity>(conn, orders).await?;
    insert_chunked::<order_tracking::Entity>(conn, tracking).await?;
    Ok(tracking_count)
}

/// Настройки: стандартные значения, поверх них пакет, часовой пояс прежний.
async fn write_settings(
    conn: &impl ConnectionTrait,
    b: &ImportBundle,
    point_ids: &HashMap<String, i64>,
    timezone: String,
) -> Result<(), CoreError> {
    let mut s = AppSettings {
        timezone,
        default_service_point_id: b
            .settings
            .default_service_point
            .as_ref()
            .map(|p| point_ids[p]),
        ..AppSettings::default()
    };
    let p = &b.settings.values;
    if let Some(v) = &p.currency_symbol {
        s.currency_symbol = v.clone();
    }
    if let Some(v) = p.master_percent {
        s.master_percent = v;
    }
    if let Some(v) = p.diagnostics_overdue_days {
        s.diagnostics_overdue_days = v;
    }
    if let Some(v) = &p.default_warranty {
        s.default_warranty = v.clone();
    }
    if let Some(v) = p.intake_required_fields {
        s.intake_required_fields = v;
    }
    if let Some(v) = &p.default_master {
        s.default_master = v.clone();
    }
    if let Some(v) = &p.firm_name {
        s.firm_name = v.clone();
    }
    if let Some(v) = &p.firm_phone {
        s.firm_phone = v.clone();
    }
    if let Some(v) = &p.firm_details {
        s.firm_details = v.clone();
    }
    if let Some(v) = &p.firm_legal_details {
        s.firm_legal_details = v.clone();
    }
    if let Some(v) = &p.intake_terms {
        s.intake_terms = v.clone();
    }
    if let Some(v) = &p.issue_terms {
        s.issue_terms = v.clone();
    }
    let Value::Object(map) =
        serde_json::to_value(&s).map_err(|e| bad(format!("настройки: {e}")))?
    else {
        return Err(bad("настройки не превратились в объект"));
    };
    for (key, value) in &map {
        put(conn, key, value).await?;
    }
    put(conn, "search_index_version", &json!(SEARCH_INDEX_VERSION)).await?;
    Ok(())
}

/// Строки таблицы должны совпасть с пакетом.
async fn expect_count<E: EntityTrait>(
    conn: &impl ConnectionTrait,
    what: &str,
    expected: u64,
) -> Result<(), CoreError>
where
    E::Model: Send + Sync,
{
    let actual = E::find().count(conn).await?;
    if actual == expected {
        Ok(())
    } else {
        Err(bad(format!(
            "сверка не сошлась: {what}: в базе {actual}, в пакете {expected}"
        )))
    }
}

impl Core {
    /// Права, нужные для импорта (`settings_manage`, затем `users_manage`); проверяются до копирования файлов.
    pub fn check_import_rights(&self, actor: &Actor) -> Result<(), CoreError> {
        actor.require(Permission::SettingsManage)?;
        actor.require(Permission::UsersManage)
    }

    /// Какая СУБД под `Core`.
    pub fn database_kind(&self) -> DatabaseKind {
        match self.db.get_database_backend() {
            DatabaseBackend::Postgres => DatabaseKind::Postgres,
            _ => DatabaseKind::Sqlite,
        }
    }

    /// Сколько клиентов, заказов, пользователей и записей журнала сейчас в базе.
    pub async fn import_target_state(&self) -> Result<ImportTarget, CoreError> {
        target_state(&self.db).await
    }

    /// Записывает пакет из MyWork 2 в одной транзакции: или всё, или ничего.
    /// `Replace` (только SQLite) под блокировкой писателя снимает копию базы, затем стирает и пишет заново.
    pub async fn import_bundle(
        &self,
        actor: &Actor,
        bundle: ImportBundle,
        mode: ImportMode,
    ) -> Result<ImportSummary, CoreError> {
        self.check_import_rights(actor)?;
        validate_bundle(&bundle)?;
        if matches!(mode, ImportMode::Replace { .. })
            && self.database_kind() != DatabaseKind::Sqlite
        {
            return Err(CoreError::Validation(vec![FieldError::new(
                "mode",
                "replace_unsupported",
            )]));
        }
        let tx = self.begin_write().await?;
        match self.write_bundle(&tx, actor, &bundle, &mode).await {
            Ok(summary) => {
                let loaded = settings::load(&tx).await;
                let loaded = match loaded {
                    Ok(l) => l,
                    Err(e) => {
                        tx.rollback().await.ok();
                        return Err(e);
                    }
                };
                tx.commit().await?;
                *self.settings.write().unwrap_or_else(|e| e.into_inner()) = loaded;
                Ok(summary)
            }
            Err(e) => {
                tx.rollback().await.ok();
                Err(e)
            }
        }
    }

    async fn write_bundle(
        &self,
        tx: &impl ConnectionTrait,
        actor: &Actor,
        b: &ImportBundle,
        mode: &ImportMode,
    ) -> Result<ImportSummary, CoreError> {
        if matches!(mode, ImportMode::IntoEmpty) && !target_state(tx).await?.is_empty() {
            return Err(CoreError::Validation(vec![FieldError::new(
                "database",
                "not_empty",
            )]));
        }
        // Копия снимается под блокировкой писателя: между копией и стиранием база не меняется.
        // Другое соединение пула читает последнее зафиксированное состояние (WAL).
        if let ImportMode::Replace { backup_to } = mode {
            mw_db::backup::vacuum_into(&self.db, backup_to).await?;
        }
        let timezone = settings::load(tx).await?.timezone;
        wipe(tx).await?;

        let (status_ids, point_ids) = write_catalogs(tx, b).await?;
        let (role_ids, user_ids) = write_access(self, tx, b).await?;
        let phone_count = write_clients(tx, b).await?;
        let tracking_count = write_orders(tx, b, &status_ids, &point_ids).await?;

        let now = self.now();
        let mode_code = match mode {
            ImportMode::IntoEmpty => "into_empty",
            ImportMode::Replace { .. } => "replace",
        };
        let import_id = legacy_import::ActiveModel {
            id: NotSet,
            imported_at: Set(now),
            source_path: Set(b.source_path.clone()),
            mode: Set(mode_code.to_owned()),
            report: Set(b.report.to_string()),
        }
        .insert(tx)
        .await?
        .id;

        // Записи пакета идут со своим временем и автором; `legacy.imported` — от `actor`.
        let mut entries: Vec<audit_log::ActiveModel> = b
            .history
            .iter()
            .map(|h| audit_log::ActiveModel {
                id: NotSet,
                at: Set(h.at),
                user_id: Set(None),
                user_name: Set(h.user_name.clone()),
                action: Set(h.action.clone()),
                order_id: Set(h.order_id),
                client_id: Set(h.client_id),
                summary: Set(h.summary.clone()),
                details: Set(h.details.as_ref().map(Value::to_string)),
            })
            .collect();
        // Пользователь `actor` мог быть стёрт заменой: тогда ссылки на него нет.
        let actor_user = match actor.user_id() {
            Some(id) => user::Entity::find_by_id(id).one(tx).await?.map(|u| u.id),
            None => None,
        };
        entries.push(audit_log::ActiveModel {
            id: NotSet,
            at: Set(now),
            user_id: Set(actor_user),
            user_name: Set(actor.name().to_owned()),
            action: Set("legacy.imported".to_owned()),
            order_id: Set(None),
            client_id: Set(None),
            summary: Set(format!(
                "Импорт из MyWork 2: заказов {}, клиентов {}",
                b.orders.len(),
                b.clients.len()
            )),
            details: Set(Some(json!({ "import_id": import_id }).to_string())),
        });
        insert_chunked::<audit_log::Entity>(tx, entries).await?;
        mw_db::backfill::status_changed_at(tx).await?;

        let rows = b
            .legacy_rows
            .iter()
            .map(|r| {
                let target = match &r.target {
                    None => (None, None),
                    Some(LegacyTarget::Order(id)) => (Some("orders"), Some(*id)),
                    Some(LegacyTarget::Client(id)) => (Some("clients"), Some(*id)),
                    Some(LegacyTarget::Role(name)) => (Some("roles"), Some(role_ids[name])),
                    Some(LegacyTarget::User(login)) => (Some("users"), Some(user_ids[login])),
                };
                legacy_row::ActiveModel {
                    id: NotSet,
                    import_id: Set(import_id),
                    source: Set(r.source.clone()),
                    source_key: Set(r.source_key.clone()),
                    target_table: Set(target.0.map(str::to_owned)),
                    target_id: Set(target.1),
                    raw: Set(r.raw.to_string()),
                }
            })
            .collect();
        insert_chunked::<legacy_row::Entity>(tx, rows).await?;
        mw_db::backfill::legacy_client_files(tx).await?;

        write_settings(tx, b, &point_ids, timezone).await?;

        let dictionary_items: u64 = b.dictionary.iter().map(|(_, v)| v.len() as u64).sum();
        expect_count::<status::Entity>(tx, "статусы", b.statuses.len() as u64).await?;
        expect_count::<service_point::Entity>(tx, "точки", b.service_points.len() as u64).await?;
        expect_count::<dictionary_item::Entity>(tx, "справочники", dictionary_items).await?;
        expect_count::<role::Entity>(tx, "роли", b.roles.len() as u64).await?;
        expect_count::<user::Entity>(tx, "пользователи", b.users.len() as u64).await?;
        expect_count::<client::Entity>(tx, "клиенты", b.clients.len() as u64).await?;
        expect_count::<client_phone::Entity>(tx, "телефоны", phone_count).await?;
        expect_count::<order::Entity>(tx, "заказы", b.orders.len() as u64).await?;
        expect_count::<order_tracking::Entity>(tx, "трек-номера", tracking_count).await?;
        expect_count::<audit_log::Entity>(tx, "журнал", b.history.len() as u64 + 1).await?;
        expect_count::<legacy_row::Entity>(tx, "следы", b.legacy_rows.len() as u64).await?;
        expect_count::<legacy_import::Entity>(tx, "импорты", 1).await?;

        // Последним: `setval` в PostgreSQL не откатывается вместе с транзакцией.
        set_next_id(tx, IdTable::Orders, b.next_order_id).await?;
        set_next_id(tx, IdTable::Clients, b.next_client_id).await?;

        Ok(ImportSummary {
            import_id,
            statuses: b.statuses.len() as u64,
            service_points: b.service_points.len() as u64,
            dictionary_items,
            roles: b.roles.len() as u64,
            users: b.users.len() as u64,
            clients: b.clients.len() as u64,
            client_phones: phone_count,
            orders: b.orders.len() as u64,
            deleted_orders: b.orders.iter().filter(|o| o.deleted_at.is_some()).count() as u64,
            tracking_numbers: tracking_count,
            history: b.history.len() as u64,
            legacy_rows: b.legacy_rows.len() as u64,
            next_order_id: b.next_order_id,
            next_client_id: b.next_client_id,
        })
    }
}
