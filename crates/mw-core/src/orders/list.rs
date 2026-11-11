//! Список заказов: поиск по тексту заказа и клиента, фильтры, сортировка, штрихкод, нумерация.

use chrono::{Duration, NaiveDateTime};
use mw_db::entity::{client, order, status};
use mw_db::ids::{IdTable, max_id, set_next_id};
use mw_domain::barcode::{normalize_scanned, order_barcode};
use mw_domain::money::Money;
use mw_domain::permission::Permission;
use mw_domain::search::{like_contains, search_tokens};
use mw_domain::status::StatusKind;
use mw_domain::validation::FieldError;
use sea_orm::sea_query::{
    Asterisk, Condition, Expr, ExprTrait, Func, LikeExpr, NullOrdering, Order, Query,
    SelectStatement, SimpleExpr,
};
use sea_orm::{ConnectionTrait, FromQueryResult, StatementBuilder};
use serde_json::json;

use crate::page::validate_page;
use crate::sql::text_sort;
use crate::{Actor, Core, CoreError, NewAuditEntry, Page, write_audit};

/// Фильтр по тому, выдан ли заказ (по виду его статуса).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum IssuedFilter {
    #[default]
    Any,
    /// В работе: вид статуса не `issued`.
    InService,
    Issued,
}

/// Условия списка заказов.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OrderFilter {
    /// Слова поиска по тексту заказа и клиента; все слова должны найтись (И).
    pub query: String,
    /// Пусто — любые статусы.
    pub status_ids: Vec<i64>,
    pub service_point_id: Option<i64>,
    pub client_id: Option<i64>,
    pub need_call: Option<bool>,
    pub need_parts: Option<bool>,
    pub issued: IssuedFilter,
    /// Диагностика дольше `diagnostics_overdue_days` суток.
    pub overdue_diagnostics: bool,
    /// Показывать и удалённые (нужно право `orders_delete`).
    pub include_deleted: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OrderSortField {
    Id,
    #[default]
    ReceivedAt,
    IssuedAt,
    ClientName,
    DeviceType,
    Brand,
    Model,
    Status,
    Master,
    FinalPrice,
}

/// Порядок списка. Второй ключ всегда `id` по убыванию.
///
/// Текст сортируется двоично (по кодовым точкам) на обеих СУБД; пустые `issued_at` и
/// `final_price` всегда в конце, при любом направлении.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderSort {
    pub field: OrderSortField,
    pub descending: bool,
}

impl Default for OrderSort {
    fn default() -> Self {
        Self {
            field: OrderSortField::ReceivedAt,
            descending: true,
        }
    }
}

/// Строка списка заказов.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderListItem {
    pub id: i64,
    pub received_at: NaiveDateTime,
    pub issued_at: Option<NaiveDateTime>,
    pub client_id: i64,
    pub client_name: String,
    pub client_phone: String,
    pub client_blacklisted: bool,
    pub service_point_id: Option<i64>,
    pub status_id: i64,
    pub device_type: String,
    pub brand: String,
    pub model: String,
    pub serial: String,
    pub malfunction: String,
    pub master: String,
    pub estimate_from: Option<Money>,
    pub estimate_to: Option<Money>,
    pub prepayment: Money,
    pub final_price: Option<Money>,
    pub need_call: bool,
    pub need_parts: bool,
    pub barcode: String,
    pub deleted_at: Option<NaiveDateTime>,
    pub version: i64,
}

#[derive(FromQueryResult)]
struct Row {
    id: i64,
    received_at: NaiveDateTime,
    issued_at: Option<NaiveDateTime>,
    client_id: i64,
    client_name: String,
    client_phone: String,
    client_blacklisted: bool,
    service_point_id: Option<i64>,
    status_id: i64,
    device_type: String,
    brand: String,
    model: String,
    serial: String,
    malfunction: String,
    master: String,
    estimate_from: Option<i64>,
    estimate_to: Option<i64>,
    prepayment: i64,
    final_price: Option<i64>,
    need_call: bool,
    need_parts: bool,
    barcode: String,
    deleted_at: Option<NaiveDateTime>,
    version: i64,
}

impl From<Row> for OrderListItem {
    fn from(r: Row) -> Self {
        Self {
            id: r.id,
            received_at: r.received_at,
            issued_at: r.issued_at,
            client_id: r.client_id,
            client_name: r.client_name,
            client_phone: r.client_phone,
            client_blacklisted: r.client_blacklisted,
            service_point_id: r.service_point_id,
            status_id: r.status_id,
            device_type: r.device_type,
            brand: r.brand,
            model: r.model,
            serial: r.serial,
            malfunction: r.malfunction,
            master: r.master,
            estimate_from: r.estimate_from.map(Money::from_kopecks),
            estimate_to: r.estimate_to.map(Money::from_kopecks),
            prepayment: Money::from_kopecks(r.prepayment),
            final_price: r.final_price.map(Money::from_kopecks),
            need_call: r.need_call,
            need_parts: r.need_parts,
            barcode: r.barcode,
            deleted_at: r.deleted_at,
            version: r.version,
        }
    }
}

fn o(c: order::Column) -> Expr {
    Expr::col((order::Entity, c))
}

fn cl(c: client::Column) -> Expr {
    Expr::col((client::Entity, c))
}

fn st(c: status::Column) -> Expr {
    Expr::col((status::Entity, c))
}

fn contains(col: Expr, pattern: &str) -> SimpleExpr {
    col.like(LikeExpr::new(pattern).escape('\\'))
}

/// `FROM orders JOIN clients JOIN statuses WHERE cond`.
fn base(cond: Condition) -> SelectStatement {
    let mut q = Query::select();
    q.from(order::Entity)
        .inner_join(
            client::Entity,
            o(order::Column::ClientId).equals((client::Entity, client::Column::Id)),
        )
        .inner_join(
            status::Entity,
            o(order::Column::StatusId).equals((status::Entity, status::Column::Id)),
        )
        .cond_where(cond);
    q
}

fn sort_key(
    backend: sea_orm::DatabaseBackend,
    field: OrderSortField,
) -> (SimpleExpr, NullOrdering) {
    use OrderSortField::*;
    let text = |e: Expr| (text_sort(backend, e), NullOrdering::Last);
    match field {
        Id => (o(order::Column::Id), NullOrdering::Last),
        ReceivedAt => (o(order::Column::ReceivedAt), NullOrdering::Last),
        IssuedAt => (o(order::Column::IssuedAt), NullOrdering::Last),
        ClientName => text(cl(client::Column::SearchText)),
        DeviceType => text(o(order::Column::DeviceType)),
        Brand => text(o(order::Column::Brand)),
        Model => text(o(order::Column::Model)),
        Status => (st(status::Column::SortOrder), NullOrdering::Last),
        Master => text(o(order::Column::Master)),
        FinalPrice => (o(order::Column::FinalPrice), NullOrdering::Last),
    }
}

impl Core {
    async fn select_items(&self, mut q: SelectStatement) -> Result<Vec<OrderListItem>, CoreError> {
        let backend = self.db.get_database_backend();
        q.expr_as(o(order::Column::Id), "id")
            .expr_as(o(order::Column::ReceivedAt), "received_at")
            .expr_as(o(order::Column::IssuedAt), "issued_at")
            .expr_as(o(order::Column::ClientId), "client_id")
            .expr_as(cl(client::Column::FullName), "client_name")
            .expr_as(cl(client::Column::Phone), "client_phone")
            .expr_as(cl(client::Column::Blacklisted), "client_blacklisted")
            .expr_as(o(order::Column::ServicePointId), "service_point_id")
            .expr_as(o(order::Column::StatusId), "status_id")
            .expr_as(o(order::Column::DeviceType), "device_type")
            .expr_as(o(order::Column::Brand), "brand")
            .expr_as(o(order::Column::Model), "model")
            .expr_as(o(order::Column::Serial), "serial")
            .expr_as(o(order::Column::Malfunction), "malfunction")
            .expr_as(o(order::Column::Master), "master")
            .expr_as(o(order::Column::EstimateFrom), "estimate_from")
            .expr_as(o(order::Column::EstimateTo), "estimate_to")
            .expr_as(o(order::Column::Prepayment), "prepayment")
            .expr_as(o(order::Column::FinalPrice), "final_price")
            .expr_as(o(order::Column::NeedCall), "need_call")
            .expr_as(o(order::Column::NeedParts), "need_parts")
            .expr_as(o(order::Column::Barcode), "barcode")
            .expr_as(o(order::Column::DeletedAt), "deleted_at")
            .expr_as(o(order::Column::Version), "version");
        let stmt = StatementBuilder::build(&q, &backend);
        Ok(Row::find_by_statement(stmt)
            .all(&self.db)
            .await?
            .into_iter()
            .map(OrderListItem::from)
            .collect())
    }

    /// Список заказов. Права не нужны, но `include_deleted` требует `orders_delete`.
    pub async fn list_orders(
        &self,
        actor: &Actor,
        filter: &OrderFilter,
        sort: OrderSort,
        limit: u64,
        offset: u64,
    ) -> Result<Page<OrderListItem>, CoreError> {
        if filter.include_deleted {
            actor.require(Permission::OrdersDelete)?;
        }
        validate_page(limit, offset)?;
        let backend = self.db.get_database_backend();

        let mut cond = Condition::all();
        if !filter.include_deleted {
            cond = cond.add(o(order::Column::DeletedAt).is_null());
        }
        for token in search_tokens(&filter.query) {
            let p = like_contains(&token);
            cond = cond.add(
                Condition::any()
                    .add(contains(o(order::Column::SearchText), &p))
                    .add(contains(cl(client::Column::SearchText), &p)),
            );
        }
        if !filter.status_ids.is_empty() {
            cond = cond.add(o(order::Column::StatusId).is_in(filter.status_ids.iter().copied()));
        }
        if let Some(id) = filter.service_point_id {
            cond = cond.add(o(order::Column::ServicePointId).eq(id));
        }
        if let Some(id) = filter.client_id {
            cond = cond.add(o(order::Column::ClientId).eq(id));
        }
        if let Some(v) = filter.need_call {
            cond = cond.add(o(order::Column::NeedCall).eq(v));
        }
        if let Some(v) = filter.need_parts {
            cond = cond.add(o(order::Column::NeedParts).eq(v));
        }
        let issued = StatusKind::Issued.as_str();
        match filter.issued {
            IssuedFilter::Any => {}
            IssuedFilter::InService => cond = cond.add(st(status::Column::Kind).ne(issued)),
            IssuedFilter::Issued => cond = cond.add(st(status::Column::Kind).eq(issued)),
        }
        if filter.overdue_diagnostics {
            let days = i64::from(self.settings().diagnostics_overdue_days);
            let cutoff = self.now() - Duration::days(days);
            cond = cond
                .add(st(status::Column::Kind).eq(StatusKind::Diagnostics.as_str()))
                .add(o(order::Column::ReceivedAt).lt(cutoff));
        }

        let mut count = base(cond.clone());
        count.expr(Func::count(Expr::col(Asterisk)));
        let row = self
            .db
            .query_one_raw(StatementBuilder::build(&count, &backend))
            .await?
            .ok_or_else(|| sea_orm::DbErr::RecordNotFound("COUNT(*)".to_owned()))?;
        let total = u64::try_from(row.try_get_by_index::<i64>(0)?).unwrap_or(0);

        let mut q = base(cond);
        let (key, nulls) = sort_key(backend, sort.field);
        let direction = if sort.descending {
            Order::Desc
        } else {
            Order::Asc
        };
        q.order_by_expr_with_nulls(key, direction, nulls)
            .order_by_expr(o(order::Column::Id), Order::Desc)
            .limit(limit)
            .offset(offset);
        Ok(Page {
            items: self.select_items(q).await?,
            total,
        })
    }

    /// Поиск по штрихкоду сканера среди неудалённых заказов; старые коды могли совпасть.
    pub async fn find_orders_by_barcode(
        &self,
        _actor: &Actor,
        scanned: &str,
    ) -> Result<Vec<OrderListItem>, CoreError> {
        let code = normalize_scanned(scanned);
        if code.is_empty() {
            return Ok(Vec::new());
        }
        let cond = Condition::all()
            .add(o(order::Column::DeletedAt).is_null())
            .add(o(order::Column::Barcode).eq(code));
        let mut q = base(cond);
        q.order_by_expr(o(order::Column::ReceivedAt), Order::Desc)
            .order_by_expr(o(order::Column::Id), Order::Desc);
        self.select_items(q).await
    }

    /// Следующий созданный заказ получит номер `next` (выше всех существующих).
    pub async fn set_next_order_number(&self, actor: &Actor, next: i64) -> Result<(), CoreError> {
        actor.require(Permission::SettingsManage)?;
        let tx = self.begin_write().await?;
        if next <= max_id(&tx, IdTable::Orders).await? || order_barcode(next).is_none() {
            return Err(CoreError::Validation(vec![FieldError::new(
                "next",
                "out_of_range",
            )]));
        }
        set_next_id(&tx, IdTable::Orders, next).await?;
        write_audit(
            &tx,
            self.now(),
            actor,
            NewAuditEntry {
                action: "orders.next_number_set",
                order_id: None,
                client_id: None,
                summary: format!("Следующий номер заказа: {next}"),
                details: Some(json!({ "next": next })),
            },
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
}
