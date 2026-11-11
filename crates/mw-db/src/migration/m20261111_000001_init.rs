//! Начальная схема и стартовые данные (статусы, роль «Администратор», справочники).

use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20261111_000001_init"
    }
}

fn a(name: &str) -> Alias {
    Alias::new(name)
}

fn id() -> ColumnDef {
    ColumnDef::new(a("id"))
        .big_integer()
        .not_null()
        .auto_increment()
        .primary_key()
        .take()
}

/// Текст без значения: `TEXT NOT NULL DEFAULT ''`.
fn text(name: &str) -> ColumnDef {
    ColumnDef::new(a(name)).text().not_null().default("").take()
}

fn text_required(name: &str) -> ColumnDef {
    ColumnDef::new(a(name)).text().not_null().take()
}

fn flag(name: &str, default: bool) -> ColumnDef {
    ColumnDef::new(a(name))
        .boolean()
        .not_null()
        .default(default)
        .take()
}

fn int_default(name: &str) -> ColumnDef {
    ColumnDef::new(a(name))
        .integer()
        .not_null()
        .default(0)
        .take()
}

fn int_required(name: &str) -> ColumnDef {
    ColumnDef::new(a(name)).integer().not_null().take()
}

fn money_default(name: &str) -> ColumnDef {
    ColumnDef::new(a(name))
        .big_integer()
        .not_null()
        .default(0)
        .take()
}

fn money_null(name: &str) -> ColumnDef {
    ColumnDef::new(a(name)).big_integer().null().take()
}

fn ts(name: &str) -> ColumnDef {
    ColumnDef::new(a(name)).timestamp().not_null().take()
}

fn ts_null(name: &str) -> ColumnDef {
    ColumnDef::new(a(name)).timestamp().null().take()
}

fn version() -> ColumnDef {
    ColumnDef::new(a("version"))
        .big_integer()
        .not_null()
        .default(1)
        .take()
}

fn fk(
    name: &str,
    from: &str,
    column: &str,
    to: &str,
    action: ForeignKeyAction,
) -> ForeignKeyCreateStatement {
    ForeignKey::create()
        .name(name)
        .from(a(from), a(column))
        .to(a(to), a("id"))
        .on_delete(action)
        .to_owned()
}

fn index(table: &str, column: &str) -> IndexCreateStatement {
    Index::create()
        .name(format!("idx_{table}_{column}"))
        .table(a(table))
        .col(a(column))
        .to_owned()
}

fn unique(table: &str, columns: &[&str]) -> IndexCreateStatement {
    let mut stmt = Index::create();
    stmt.name(format!("uq_{table}_{}", columns.join("_")))
        .table(a(table))
        .unique();
    for column in columns {
        stmt.col(a(column));
    }
    stmt.to_owned()
}

fn seed_err(e: impl std::fmt::Display) -> DbErr {
    DbErr::Migration(format!("стартовые данные: {e}"))
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    /// SQLite по умолчанию не оборачивает миграцию в транзакцию; нужна атомарность.
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

    async fn up(&self, m: &SchemaManager) -> Result<(), DbErr> {
        use ForeignKeyAction::{Cascade, Restrict, SetNull};

        m.create_table(
            Table::create()
                .table(a("service_points"))
                .col(id())
                .col(text_required("name"))
                .col(int_default("sort_order"))
                .col(flag("archived", false))
                .to_owned(),
        )
        .await?;
        m.create_index(unique("service_points", &["name"])).await?;

        m.create_table(
            Table::create()
                .table(a("statuses"))
                .col(id())
                .col(text_required("name"))
                .col(text_required("kind"))
                .col(text("color"))
                .col(int_required("sort_order"))
                .col(flag("archived", false))
                .to_owned(),
        )
        .await?;
        m.create_index(unique("statuses", &["name"])).await?;

        m.create_table(
            Table::create()
                .table(a("dictionary_items"))
                .col(id())
                .col(text_required("kind"))
                .col(text_required("value"))
                .col(int_default("sort_order"))
                .to_owned(),
        )
        .await?;
        m.create_index(unique("dictionary_items", &["kind", "value"]))
            .await?;

        m.create_table(
            Table::create()
                .table(a("clients"))
                .col(id())
                .col(text("full_name"))
                .col(text("phone"))
                .col(text("phone_key"))
                .col(text("address"))
                .col(text("note"))
                .col(text("source"))
                .col(flag("blacklisted", false))
                .col(ts("created_at"))
                .col(ts("updated_at"))
                .col(ts_null("deleted_at"))
                .col(version())
                .col(text("search_text"))
                .to_owned(),
        )
        .await?;
        m.create_index(index("clients", "phone_key")).await?;

        let mut orders = Table::create();
        orders
            .table(a("orders"))
            .col(id())
            .col(ColumnDef::new(a("client_id")).big_integer().not_null())
            .col(ColumnDef::new(a("service_point_id")).big_integer().null())
            .col(ColumnDef::new(a("status_id")).big_integer().not_null())
            .col(ts("received_at"))
            .col(ts_null("issued_at"))
            .col(ts_null("prepaid_at"));
        for name in [
            "device_type",
            "brand",
            "model",
            "serial",
            "color",
            "device_password",
            "completeness",
            "device_condition",
            "malfunction",
            "work_done",
            "internal_comment",
            "master",
            "barcode",
        ] {
            orders.col(text(name));
        }
        orders
            .col(money_null("estimate_from"))
            .col(money_null("estimate_to"))
            .col(money_null("final_price"))
            .col(money_default("prepayment"))
            .col(money_default("extra_costs"))
            .col(money_default("discount"))
            .col(ColumnDef::new(a("warranty_days")).integer().null())
            .col(flag("need_call", false))
            .col(flag("need_parts", false))
            .col(ts("created_at"))
            .col(ts("updated_at"))
            .col(ts_null("deleted_at"))
            .col(version())
            .col(text("search_text"))
            .foreign_key(&mut fk(
                "fk_orders_client",
                "orders",
                "client_id",
                "clients",
                Restrict,
            ))
            .foreign_key(&mut fk(
                "fk_orders_service_point",
                "orders",
                "service_point_id",
                "service_points",
                Restrict,
            ))
            .foreign_key(&mut fk(
                "fk_orders_status",
                "orders",
                "status_id",
                "statuses",
                Restrict,
            ));
        m.create_table(orders.to_owned()).await?;
        for column in [
            "client_id",
            "status_id",
            "barcode",
            "received_at",
            "service_point_id",
        ] {
            m.create_index(index("orders", column)).await?;
        }

        m.create_table(
            Table::create()
                .table(a("roles"))
                .col(id())
                .col(text_required("name"))
                .col(text_required("permissions"))
                .col(version())
                .to_owned(),
        )
        .await?;
        m.create_index(unique("roles", &["name"])).await?;

        m.create_table(
            Table::create()
                .table(a("users"))
                .col(id())
                .col(text_required("login"))
                .col(text_required("login_key"))
                .col(text("display_name"))
                .col(ColumnDef::new(a("role_id")).big_integer().not_null())
                .col(text("password_hash"))
                .col(text_required("password_scheme"))
                .col(flag("active", true))
                .col(ts("created_at"))
                .col(ts("updated_at"))
                .col(version())
                .foreign_key(&mut fk(
                    "fk_users_role",
                    "users",
                    "role_id",
                    "roles",
                    Restrict,
                ))
                .to_owned(),
        )
        .await?;
        m.create_index(unique("users", &["login_key"])).await?;

        m.create_table(
            Table::create()
                .table(a("sessions"))
                .col(id())
                .col(ColumnDef::new(a("user_id")).big_integer().not_null())
                .col(text_required("token_hash"))
                .col(text("device_label"))
                .col(ts("created_at"))
                .col(ts("last_seen_at"))
                .foreign_key(&mut fk(
                    "fk_sessions_user",
                    "sessions",
                    "user_id",
                    "users",
                    Cascade,
                ))
                .to_owned(),
        )
        .await?;
        m.create_index(unique("sessions", &["token_hash"])).await?;
        m.create_index(index("sessions", "user_id")).await?;

        m.create_table(
            Table::create()
                .table(a("settings"))
                .col(ColumnDef::new(a("key")).text().not_null().primary_key())
                .col(text_required("value"))
                .to_owned(),
        )
        .await?;

        // `order_id` и `client_id` без внешних ключей: старый журнал ссылается и на удалённые заказы.
        m.create_table(
            Table::create()
                .table(a("audit_log"))
                .col(id())
                .col(ts("at"))
                .col(ColumnDef::new(a("user_id")).big_integer().null())
                .col(text("user_name"))
                .col(text_required("action"))
                .col(ColumnDef::new(a("order_id")).big_integer().null())
                .col(ColumnDef::new(a("client_id")).big_integer().null())
                .col(text("summary"))
                .col(ColumnDef::new(a("details")).text().null())
                .foreign_key(&mut fk(
                    "fk_audit_log_user",
                    "audit_log",
                    "user_id",
                    "users",
                    SetNull,
                ))
                .to_owned(),
        )
        .await?;
        for column in ["order_id", "client_id", "at", "user_id"] {
            m.create_index(index("audit_log", column)).await?;
        }

        seed(m).await
    }
}

// Начальные данные ЗАМОРОЖЕНЫ: это литералы, а не ссылки на константы `mw-domain`. Выпущенная
// миграция не должна менять поведение при правке констант; новые данные — только новой миграцией.

/// (название, вид, цвет); порядок = `sort_order`.
const SEED_STATUSES: [(&str, &str, &str); 8] = [
    ("Диагностика", "diagnostics", "amber"),
    ("Согласование с клиентом", "open", "violet"),
    ("Согласовано", "open", "indigo"),
    ("Принят в работу", "open", "blue"),
    ("Ждёт запчасть", "open", "red"),
    ("Принят по гарантии", "open", "teal"),
    ("Готов", "ready", "green"),
    ("Выдан", "issued", "gray"),
];

/// Права роли «Администратор» (JSON-массив в порядке объявления).
const SEED_ADMIN_PERMISSIONS: &str = r#"["orders_create","orders_edit","orders_delete","order_dates_edit","reports_view","stock_view","stock_create","stock_edit","stock_delete","clients_view","clients_create","clients_edit","clients_delete","clients_merge","settings_manage","users_manage","audit_view"]"#;

/// (вид справочника, значения); порядок значений = `sort_order`.
const SEED_DICTIONARY: &[(&str, &[&str])] = &[
    (
        "device_type",
        &[
            "Телефон",
            "Планшет",
            "Ноутбук",
            "Компьютер",
            "Телевизор",
            "Игровая приставка",
            "Часы",
            "Наушники",
        ],
    ),
    (
        "brand",
        &[
            "Apple", "Samsung", "Xiaomi", "Huawei", "Honor", "Realme", "Lenovo", "Asus", "HP",
            "Sony",
        ],
    ),
    (
        "color",
        &[
            "Чёрный",
            "Белый",
            "Серый",
            "Серебристый",
            "Золотой",
            "Синий",
            "Красный",
            "Зелёный",
        ],
    ),
    (
        "completeness",
        &[
            "Устройство",
            "Чехол",
            "Зарядка",
            "Кабель",
            "Сим-карта",
            "Карта памяти",
            "Коробка",
        ],
    ),
    (
        "condition",
        &[
            "Без видимых повреждений",
            "Царапины",
            "Сколы",
            "Трещина экрана",
            "Следы влаги",
        ],
    ),
    (
        "malfunction",
        &[
            "Не включается",
            "Не заряжается",
            "Разбит экран",
            "Залит",
            "Нет звука",
            "Не ловит сеть",
            "Быстро разряжается",
        ],
    ),
    (
        "work_done",
        &[
            "Диагностика",
            "Замена дисплея",
            "Замена аккумулятора",
            "Замена разъёма зарядки",
            "Чистка",
            "Прошивка",
        ],
    ),
    ("master", &[]),
    (
        "client_source",
        &[
            "Рекомендация",
            "Интернет",
            "Авито",
            "Вывеска",
            "Повторное обращение",
        ],
    ),
];

async fn seed(m: &SchemaManager<'_>) -> Result<(), DbErr> {
    for (order, (name, kind, color)) in (0_i32..).zip(SEED_STATUSES) {
        let mut insert = Query::insert();
        insert
            .into_table(a("statuses"))
            .columns([a("name"), a("kind"), a("color"), a("sort_order")])
            .values([name.into(), kind.into(), color.into(), order.into()])
            .map_err(seed_err)?;
        m.exec_stmt(insert).await?;
    }

    let mut role = Query::insert();
    role.into_table(a("roles"))
        .columns([a("name"), a("permissions")])
        .values(["Администратор".into(), SEED_ADMIN_PERMISSIONS.into()])
        .map_err(seed_err)?;
    m.exec_stmt(role).await?;

    for (kind, values) in SEED_DICTIONARY {
        for (order, value) in (0_i32..).zip(values.iter()) {
            let mut insert = Query::insert();
            insert
                .into_table(a("dictionary_items"))
                .columns([a("kind"), a("value"), a("sort_order")])
                .values([(*kind).into(), (*value).into(), order.into()])
                .map_err(seed_err)?;
            m.exec_stmt(insert).await?;
        }
    }
    Ok(())
}
