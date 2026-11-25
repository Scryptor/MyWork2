//! Excel-файл со списком заказов: те же строки и порядок, что у `Core::list_orders`.

use std::collections::HashMap;

use mw_db::entity::{order, order_part, service_point, status};
use mw_domain::money::Money;
use mw_domain::permission::Permission;
use mw_domain::phone::format_phone;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

use super::xlsx::{Cell, Column, Sheet, workbook};
use super::{EXPORT_ROWS_MAX, ExportFile, XLSX_CONTENT_TYPE, dmy, ensure_rows};
use crate::sql::ID_CHUNK;
use crate::{Actor, Core, CoreError, OrderFilter, OrderListItem, OrderSort};

const ASCII_NAME: &str = "orders.xlsx";

const COLUMNS: [&str; 24] = [
    "№ заказа",
    "Принят",
    "Статус",
    "Выдан",
    "Точка",
    "Клиент",
    "Телефон",
    "Тип устройства",
    "Бренд",
    "Модель",
    "Серийный номер",
    "Неисправность",
    "Выполненные работы",
    "Мастер",
    "Оценка от",
    "Оценка до",
    "Итог",
    "Скидка",
    "Предоплата",
    "Расходы",
    "Запчасти",
    "К оплате",
    "Гарантия",
    "Штрихкод",
];

fn overflow() -> CoreError {
    CoreError::Internal("сумма заказа не помещается в число".to_owned())
}

fn checked(kopecks: Option<i64>) -> Result<Money, CoreError> {
    kopecks.map(Money::from_kopecks).ok_or_else(overflow)
}

fn optional(money: Option<Money>) -> Cell {
    money.map_or(Cell::Empty, Cell::Money)
}

impl Core {
    /// Стоимость запчастей (цена × количество) по заказам из списка.
    async fn parts_cost(&self, ids: &[i64]) -> Result<HashMap<i64, i64>, CoreError> {
        let mut sums: HashMap<i64, i64> = HashMap::new();
        for chunk in ids.chunks(ID_CHUNK) {
            for p in order_part::Entity::find()
                .filter(order_part::Column::OrderId.is_in(chunk.iter().copied()))
                .all(&self.db)
                .await?
            {
                let line = p.price.checked_mul(p.quantity).ok_or_else(overflow)?;
                let sum = sums.entry(p.order_id).or_default();
                *sum = sum.checked_add(line).ok_or_else(overflow)?;
            }
        }
        Ok(sums)
    }

    /// Остальные поля заказов, которых нет в строке списка.
    async fn order_models(&self, ids: &[i64]) -> Result<HashMap<i64, order::Model>, CoreError> {
        let mut map = HashMap::new();
        for chunk in ids.chunks(ID_CHUNK) {
            for m in order::Entity::find()
                .filter(order::Column::Id.is_in(chunk.iter().copied()))
                .all(&self.db)
                .await?
            {
                map.insert(m.id, m);
            }
        }
        Ok(map)
    }

    /// Заказы файлом Excel: условия и порядок как у списка, без страниц; нужно право
    /// «Выгрузка». Больше `EXPORT_ROWS_MAX` строк — ошибка `rows`/`too_many_rows`.
    pub async fn export_orders_xlsx(
        &self,
        actor: &Actor,
        filter: &OrderFilter,
        sort: OrderSort,
    ) -> Result<ExportFile, CoreError> {
        actor.require(Permission::DataExport)?;
        let max = u64::try_from(EXPORT_ROWS_MAX).unwrap_or(u64::MAX);
        let items = self
            .list_orders_capped(actor, filter, sort, max.saturating_add(1))
            .await?;
        ensure_rows(items.len(), EXPORT_ROWS_MAX)?;
        let ids: Vec<i64> = items.iter().map(|i| i.id).collect();
        let models = self.order_models(&ids).await?;
        let parts = self.parts_cost(&ids).await?;
        let statuses: HashMap<i64, String> = status::Entity::find()
            .all(&self.db)
            .await?
            .into_iter()
            .map(|s| (s.id, s.name))
            .collect();
        let points: HashMap<i64, String> = service_point::Entity::find()
            .all(&self.db)
            .await?
            .into_iter()
            .map(|p| (p.id, p.name))
            .collect();

        let mut rows = Vec::with_capacity(items.len());
        for item in &items {
            let model = models.get(&item.id).ok_or_else(|| {
                CoreError::Internal(format!("заказ № {} пропал во время выгрузки", item.id))
            })?;
            let parts_cost = Money::from_kopecks(parts.get(&item.id).copied().unwrap_or(0));
            rows.push(order_row(item, model, &statuses, &points, parts_cost)?);
        }
        let sheet = Sheet {
            name: "Заказы".into(),
            columns: COLUMNS.map(Column::new).into(),
            rows,
            total: None,
        };
        let currency = self.settings().currency_symbol.clone();
        let sheets = [sheet];
        let bytes = self
            .export_gate
            .run(move || workbook(&sheets, &currency))
            .await??;
        Ok(ExportFile {
            name: format!("Заказы {}.xlsx", dmy(self.now().date())),
            ascii_name: ASCII_NAME,
            content_type: XLSX_CONTENT_TYPE,
            bytes,
        })
    }
}

fn order_row(
    item: &OrderListItem,
    m: &order::Model,
    statuses: &HashMap<i64, String>,
    points: &HashMap<i64, String>,
    parts_cost: Money,
) -> Result<Vec<Cell>, CoreError> {
    let text = |s: &str| Cell::Text(s.to_owned());
    // К оплате: без итоговой цены — пусто.
    let to_pay = match item.final_price {
        Some(price) => Cell::Money(checked(
            price
                .kopecks()
                .checked_sub(item.prepayment.kopecks())
                .and_then(|v| v.checked_sub(item.discount.kopecks())),
        )?),
        None => Cell::Empty,
    };
    Ok(vec![
        Cell::Int(item.id),
        Cell::Time(item.received_at),
        text(statuses.get(&item.status_id).map_or("", String::as_str)),
        item.issued_at.map_or(Cell::Empty, Cell::Time),
        text(
            item.service_point_id
                .and_then(|id| points.get(&id))
                .map_or("", String::as_str),
        ),
        text(&item.client_name),
        text(&format_phone(&item.client_phone)),
        text(&item.device_type),
        text(&item.brand),
        text(&item.model),
        text(&item.serial),
        text(&item.malfunction),
        text(&m.work_done),
        text(&item.master),
        optional(item.estimate_from),
        optional(item.estimate_to),
        optional(item.final_price),
        Cell::Money(item.discount),
        Cell::Money(item.prepayment),
        Cell::Money(Money::from_kopecks(m.extra_costs)),
        Cell::Money(parts_cost),
        to_pay,
        text(&m.warranty_text),
        text(&item.barcode),
    ])
}
