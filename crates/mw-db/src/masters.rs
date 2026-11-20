//! Мастера — пользователи: одно правило для миграции данных, импорта MyWork 2 и начальной настройки.
//!
//! Имена мастеров — из заказов без мастера-пользователя и переданные списком — сводятся без учёта
//! регистра (`mw_domain::search::normalize_search`). Имя без единой буквы («-», «--», «?», «0») —
//! не мастер: такой заказ остаётся без `master_user_id`. Для каждого имени пользователь с тем же
//! отображаемым именем или логином получает отметку «Мастер», а если такого нет, создаётся
//! пользователь без входа с отметкой «Мастер» и ролью «Мастер». Его имя — первое написание из списка,
//! иначе самое частое в заказах (при равенстве — не заглавными буквами, затем первое по номеру
//! заказа; `mw_domain::spelling::Spellings::best_name`); края обрезаны, повторные пробелы внутри
//! схлопнуты. Заказы получают ссылку `master_user_id`; текст `master`, версия и время изменения
//! заказа не меняются. Ничего не удаляется; логины, пароли, роли и вход прежних пользователей не
//! меняются.
//!
//! Модуль — часть выпущенной миграции `m20261120_000004_masters_to_users`: любая правка должна
//! оставаться совместимой со схемой базы на шаге этой миграции (читать и писать только колонки,
//! которые к ней уже есть; сущности не используются), а повторный запуск — ничего не менять. Правила
//! написания и пропуска имён без букв добавлены после выпуска: они действуют на следующие запуски
//! (импорт, начальная настройка, база, ещё не обновлённая). Уже перенесённая база не трогается:
//! миграция на ней больше не выполняется, а при повторном запуске заказы со ссылкой на мастера не
//! читаются и имена из списка находят прежних пользователей (ключ имени — тот же `normalize_search`).

use std::collections::{BTreeMap, HashMap};

use chrono::NaiveDateTime;
use mw_domain::search::normalize_search;
use mw_domain::spelling::{Spellings, has_letter, tidy};
use sea_orm::sea_query::{Alias, Expr, ExprTrait, Query};
use sea_orm::{ConnectionTrait, DbErr};

use crate::entity::user::no_login_key;

/// Готовая роль мастера: название и права (как в миграции готовых ролей).
pub const MASTER_ROLE: &str = "Мастер";
/// Права роли «Мастер» в JSON; заморожено литералом.
pub const MASTER_ROLE_PERMISSIONS: &str = r#"["orders_edit","clients_view"]"#;

/// Сколько номеров заказов в одном `IN (…)`.
const LINK_CHUNK: usize = 1000;

fn a(name: &str) -> Alias {
    Alias::new(name)
}

/// Что сделало правило.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MasterUsers {
    /// Ключ имени (`normalize_search`) → номер пользователя-мастера; для всех имён.
    pub by_key: HashMap<String, i64>,
    /// Созданные пользователи без входа: (номер, имя).
    pub created: Vec<(i64, String)>,
    /// Прежние пользователи, получившие отметку «Мастер»: (номер, имя).
    pub marked: Vec<(i64, String)>,
    /// Сколько заказов получили ссылку на мастера.
    pub orders_linked: u64,
    /// Роли «Мастер» не было — добавлена.
    pub role_created: bool,
}

impl MasterUsers {
    /// Пользователь-мастер для имени (без учёта регистра); пустое или незнакомое имя — `None`.
    pub fn user_for(&self, name: &str) -> Option<i64> {
        self.by_key.get(&normalize_search(name)).copied()
    }
}

/// Одно имя мастера во всех написаниях.
#[derive(Default)]
struct Group {
    /// Первое написание из переданного списка (справочник, файл, настройка).
    listed: Option<String>,
    /// Написания из заказов с числом заказов, в порядке номеров заказов.
    spellings: Spellings,
}

impl Group {
    /// Имя нового пользователя: из списка, иначе самое частое в заказах (при равенстве — не
    /// заглавными буквами, затем первое по номеру заказа).
    fn name(&self) -> String {
        if let Some(listed) = &self.listed {
            return listed.clone();
        }
        self.spellings.best_name().unwrap_or_default().to_owned()
    }
}

struct UserRow {
    id: i64,
    login: String,
    display_name: String,
    active: bool,
    is_master: bool,
}

impl UserRow {
    fn label(&self) -> String {
        if self.display_name.is_empty() {
            self.login.clone()
        } else {
            self.display_name.clone()
        }
    }
}

async fn users(conn: &impl ConnectionTrait) -> Result<Vec<UserRow>, DbErr> {
    let mut select = Query::select();
    select
        .columns([
            a("id"),
            a("login"),
            a("display_name"),
            a("active"),
            a("is_master"),
        ])
        .from(a("users"))
        .order_by(a("id"), sea_orm::sea_query::Order::Asc);
    conn.query_all(&select)
        .await?
        .iter()
        .map(|r| {
            Ok(UserRow {
                id: r.try_get_by_index(0)?,
                login: r.try_get_by_index(1)?,
                display_name: r.try_get_by_index(2)?,
                active: r.try_get_by_index(3)?,
                is_master: r.try_get_by_index(4)?,
            })
        })
        .collect()
}

/// Пользователи по ключам имени: отображаемое имя и логин (`normalize_search`).
#[derive(Default)]
struct UserKeys {
    by_name: HashMap<String, Vec<usize>>,
    by_login: HashMap<String, Vec<usize>>,
}

impl UserKeys {
    fn add(&mut self, i: usize, u: &UserRow) {
        let name = normalize_search(&u.display_name);
        if !name.is_empty() {
            self.by_name.entry(name).or_default().push(i);
        }
        let login = normalize_search(&u.login);
        if !login.is_empty() {
            self.by_login.entry(login).or_default().push(i);
        }
    }

    /// Пользователь для ключа имени: совпадает отображаемое имя или логин. Сначала уже мастер,
    /// затем активный, затем совпавший по имени, затем с меньшим номером.
    fn matching(&self, users: &[UserRow], key: &str) -> Option<usize> {
        let found = |map: &HashMap<String, Vec<usize>>, by_name: bool| {
            map.get(key)
                .into_iter()
                .flatten()
                .map(move |&i| (i, by_name))
                .collect::<Vec<_>>()
        };
        found(&self.by_name, true)
            .into_iter()
            .chain(found(&self.by_login, false))
            .min_by_key(|&(i, by_name)| {
                let u = &users[i];
                (!u.is_master, !u.active, !by_name, u.id)
            })
            .map(|(i, _)| i)
    }
}

/// Номер роли «Мастер» (без учёта регистра); нет такой — добавляется с готовыми правами.
/// Второе значение — роль добавлена.
pub async fn ensure_master_role(conn: &impl ConnectionTrait) -> Result<(i64, bool), DbErr> {
    let mut select = Query::select();
    select
        .columns([a("id"), a("name")])
        .from(a("roles"))
        .order_by(a("id"), sea_orm::sea_query::Order::Asc);
    let key = normalize_search(MASTER_ROLE);
    for row in conn.query_all(&select).await? {
        let name: String = row.try_get_by_index(1)?;
        if normalize_search(&name) == key {
            return Ok((row.try_get_by_index(0)?, false));
        }
    }
    let mut insert = Query::insert();
    insert
        .into_table(a("roles"))
        .columns([a("name"), a("permissions"), a("version")])
        .values([
            MASTER_ROLE.into(),
            MASTER_ROLE_PERMISSIONS.into(),
            1_i64.into(),
        ])
        .map_err(|e| DbErr::Custom(format!("роль «{MASTER_ROLE}»: {e}")))?
        .returning_col(a("id"));
    let row = conn
        .query_one(&insert)
        .await?
        .ok_or_else(|| DbErr::Custom(format!("роль «{MASTER_ROLE}» не добавлена")))?;
    Ok((row.try_get_by_index(0)?, true))
}

/// Пользователь без входа с отметкой «Мастер»: без логина (ключ — `no_login_key`), без пароля.
async fn insert_master(
    conn: &impl ConnectionTrait,
    name: &str,
    role_id: i64,
    now: NaiveDateTime,
) -> Result<i64, DbErr> {
    let mut insert = Query::insert();
    insert
        .into_table(a("users"))
        .columns([
            a("login"),
            a("login_key"),
            a("display_name"),
            a("role_id"),
            a("password_hash"),
            a("password_scheme"),
            a("active"),
            a("created_at"),
            a("updated_at"),
            a("version"),
            a("can_sign_in"),
            a("is_master"),
        ])
        .values([
            "".into(),
            no_login_key()?.into(),
            name.into(),
            role_id.into(),
            "".into(),
            "argon2".into(),
            true.into(),
            now.into(),
            now.into(),
            1_i64.into(),
            false.into(),
            true.into(),
        ])
        .map_err(|e| DbErr::Custom(format!("мастер «{name}»: {e}")))?
        .returning_col(a("id"));
    let row = conn
        .query_one(&insert)
        .await?
        .ok_or_else(|| DbErr::Custom(format!("мастер «{name}» не добавлен")))?;
    row.try_get_by_index(0)
}

/// Правило «мастера → пользователи» (см. описание модуля). `names` — имена из списка (справочник,
/// файл мастеров, мастер по умолчанию, начальная настройка): их написание идёт новому пользователю
/// раньше написаний из заказов. `now` — местное время предприятия для новых и отмеченных записей.
/// Повторный вызов ничего не меняет.
pub async fn masters_to_users(
    conn: &impl ConnectionTrait,
    names: &[String],
    now: NaiveDateTime,
) -> Result<MasterUsers, DbErr> {
    let mut groups: BTreeMap<String, Group> = BTreeMap::new();
    for name in names {
        let name = tidy(name);
        if !has_letter(&name) {
            continue;
        }
        groups
            .entry(normalize_search(&name))
            .or_default()
            .listed
            .get_or_insert(name);
    }
    // Заказы без мастера-пользователя: номер и имя текстом (все, в том числе удалённые), по номерам.
    let mut select = Query::select();
    select
        .columns([a("id"), a("master")])
        .from(a("orders"))
        .and_where(Expr::col(a("master_user_id")).is_null())
        .and_where(Expr::col(a("master")).ne(""))
        .order_by(a("id"), sea_orm::sea_query::Order::Asc);
    let mut orders: Vec<(i64, String)> = Vec::new();
    for row in conn.query_all(&select).await? {
        let id: i64 = row.try_get_by_index(0)?;
        let master: String = row.try_get_by_index(1)?;
        let master = tidy(&master);
        if !has_letter(&master) {
            continue;
        }
        let key = normalize_search(&master);
        groups
            .entry(key.clone())
            .or_default()
            .spellings
            .add(&master, 1);
        orders.push((id, key));
    }

    let mut out = MasterUsers::default();
    if groups.is_empty() {
        return Ok(out);
    }
    let mut users = users(conn).await?;
    let mut keys = UserKeys::default();
    for (i, u) in users.iter().enumerate() {
        keys.add(i, u);
    }
    let mut role_id = None;
    for (key, group) in &groups {
        let id = match keys.matching(&users, key) {
            Some(i) => {
                let user = &mut users[i];
                if !user.is_master {
                    let mut update = Query::update();
                    update
                        .table(a("users"))
                        .value(a("is_master"), true)
                        .value(a("updated_at"), now)
                        .value(a("version"), Expr::col(a("version")).add(1))
                        .and_where(Expr::col(a("id")).eq(user.id));
                    conn.execute(&update).await?;
                    user.is_master = true;
                    out.marked.push((user.id, user.label()));
                }
                user.id
            }
            None => {
                let role = match role_id {
                    Some(id) => id,
                    None => {
                        let (id, created) = ensure_master_role(conn).await?;
                        out.role_created = created;
                        role_id = Some(id);
                        id
                    }
                };
                let name = group.name();
                let id = insert_master(conn, &name, role, now).await?;
                let row = UserRow {
                    id,
                    login: String::new(),
                    display_name: name.clone(),
                    active: true,
                    is_master: true,
                };
                keys.add(users.len(), &row);
                users.push(row);
                out.created.push((id, name));
                id
            }
        };
        out.by_key.insert(key.clone(), id);
    }

    let mut by_user: BTreeMap<i64, Vec<i64>> = BTreeMap::new();
    for (order_id, key) in orders {
        if let Some(user_id) = out.by_key.get(&key) {
            by_user.entry(*user_id).or_default().push(order_id);
        }
    }
    for (user_id, ids) in by_user {
        for chunk in ids.chunks(LINK_CHUNK) {
            let mut update = Query::update();
            update
                .table(a("orders"))
                .value(a("master_user_id"), user_id)
                .and_where(Expr::col(a("id")).is_in(chunk.iter().copied()))
                .and_where(Expr::col(a("master_user_id")).is_null());
            out.orders_linked += conn.execute(&update).await?.rows_affected();
        }
    }
    Ok(out)
}
