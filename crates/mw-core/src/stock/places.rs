//! Где лежит позиция: раскладка по ячейкам шкафов.
//!
//! Сумма мест позиции не больше её остатка. Проверка и запись идут одной транзакцией
//! `begin_write()`. Когда остаток падает (правка, списание в заказ, удаление), лишнее снимается с
//! ячеек по порядку кодов: «Не разложено» уходит первым, потому что сумма мест и так не больше
//! нового остатка, пока он её не пересёк. Возврат запчасти мест не трогает.

use std::collections::{HashMap, HashSet};

use mw_db::entity::{stock_item, stock_place, stock_rack};
use mw_domain::permission::Permission;
use mw_domain::stock_place::{Grid, cell_code, compare_places, take_in_order};
use mw_domain::validation::FieldError;
use sea_orm::sea_query::{Expr, ExprTrait};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, NotSet, QueryFilter, Set,
};
use serde_json::json;

use super::{ENTITY, StockCellRef, StockItem, StockMove, StockPlace, StockPut, stock_item as item};
use crate::{Actor, Core, CoreError, NewAuditEntry, write_audit};

fn invalid(field: &str, code: &str) -> CoreError {
    CoreError::Validation(vec![FieldError::new(field, code)])
}

/// Места позиций и их шкафы: двумя запросами; места каждой позиции — по порядку кодов.
pub(crate) async fn load_places(
    conn: &impl ConnectionTrait,
    item_ids: &[i64],
) -> Result<HashMap<i64, Vec<StockPlace>>, CoreError> {
    let mut out: HashMap<i64, Vec<StockPlace>> = HashMap::new();
    if item_ids.is_empty() {
        return Ok(out);
    }
    let rows = stock_place::Entity::find()
        .filter(stock_place::Column::StockItemId.is_in(item_ids.iter().copied()))
        .all(conn)
        .await?;
    if rows.is_empty() {
        return Ok(out);
    }
    let rack_ids: HashSet<i64> = rows.iter().map(|r| r.rack_id).collect();
    let racks: HashMap<i64, stock_rack::Model> = stock_rack::Entity::find()
        .filter(stock_rack::Column::Id.is_in(rack_ids))
        .all(conn)
        .await?
        .into_iter()
        .map(|r| (r.id, r))
        .collect();
    let mut keyed: Vec<(i64, String, StockPlace)> = Vec::with_capacity(rows.len());
    for r in rows {
        let rack = racks.get(&r.rack_id).ok_or_else(|| {
            CoreError::Internal(format!("у места позиции нет шкафа №{}", r.rack_id))
        })?;
        let code = cell_code(&rack.code, grid(rack), r.shelf, r.cell);
        keyed.push((
            r.stock_item_id,
            rack.code.clone(),
            StockPlace {
                rack_id: r.rack_id,
                shelf: r.shelf,
                cell: r.cell,
                code,
                quantity: r.quantity,
            },
        ));
    }
    keyed.sort_by(|a, b| compare_places((&a.1, a.2.shelf, a.2.cell), (&b.1, b.2.shelf, b.2.cell)));
    for (item_id, _, place) in keyed {
        out.entry(item_id).or_default().push(place);
    }
    Ok(out)
}

fn grid(m: &stock_rack::Model) -> Grid {
    Grid {
        shelves: m.shelves,
        cells_per_shelf: m.cells_per_shelf,
    }
}

/// Снимает с мест позиции лишнее над новым остатком: по порядку кодов, пустые строки удаляются.
/// Возвращает снятое: код ячейки и сколько штук.
pub(crate) async fn reduce_places(
    conn: &impl ConnectionTrait,
    item_id: i64,
    quantity: i64,
) -> Result<Vec<(String, i64)>, CoreError> {
    let places = load_places(conn, &[item_id])
        .await?
        .remove(&item_id)
        .unwrap_or_default();
    let amounts: Vec<i64> = places
        .iter()
        .map(|p| std::cmp::max(p.quantity, 0))
        .collect();
    let takes = take_in_order(&amounts, std::cmp::max(quantity, 0));
    let mut taken = Vec::new();
    for (place, take) in places.iter().zip(takes) {
        if take <= 0 {
            continue;
        }
        set_amount(conn, item_id, place, place.quantity - take).await?;
        taken.push((place.code.clone(), take));
    }
    Ok(taken)
}

/// Задаёт количество в месте; ноль удаляет строку.
async fn set_amount(
    conn: &impl ConnectionTrait,
    item_id: i64,
    at: &StockPlace,
    quantity: i64,
) -> Result<(), CoreError> {
    let filter = |q: sea_orm::Select<stock_place::Entity>| {
        q.filter(stock_place::Column::StockItemId.eq(item_id))
            .filter(stock_place::Column::RackId.eq(at.rack_id))
            .filter(stock_place::Column::Shelf.eq(at.shelf))
            .filter(stock_place::Column::Cell.eq(at.cell))
    };
    let row = filter(stock_place::Entity::find()).one(conn).await?;
    let Some(row) = row else {
        return Ok(());
    };
    if quantity <= 0 {
        stock_place::Entity::delete_by_id(row.id).exec(conn).await?;
    } else {
        stock_place::ActiveModel {
            id: Set(row.id),
            quantity: Set(quantity),
            ..Default::default()
        }
        .update(conn)
        .await?;
    }
    Ok(())
}

/// Сводка «из ячеек: A-2 × 1, B-1-1 × 2» для хвоста записи журнала.
pub(crate) fn reduced_tail(reduced: &[(String, i64)]) -> String {
    let list: Vec<String> = reduced.iter().map(|(c, q)| format!("{c} × {q}")).collect();
    format!(" (из ячеек: {})", list.join(", "))
}

/// Снятое с мест в виде JSON для `places_reduced`.
pub(crate) fn reduced_json(reduced: &[(String, i64)]) -> serde_json::Value {
    reduced
        .iter()
        .map(|(code, quantity)| json!({ "code": code, "quantity": quantity }))
        .collect()
}

/// Шкаф ячейки и её код; `None` — шкафа нет или ячейки нет в его сетке.
pub(crate) async fn resolve_cell(
    conn: &impl ConnectionTrait,
    at: StockCellRef,
) -> Result<Option<String>, CoreError> {
    let Some(rack) = stock_rack::Entity::find_by_id(at.rack_id).one(conn).await? else {
        return Ok(None);
    };
    let g = grid(&rack);
    Ok(g.contains(at.shelf, at.cell)
        .then(|| cell_code(&rack.code, g, at.shelf, at.cell)))
}

/// Сколько штук позиции лежит в ячейке.
async fn amount_at(
    conn: &impl ConnectionTrait,
    item_id: i64,
    at: StockCellRef,
) -> Result<i64, CoreError> {
    Ok(stock_place::Entity::find()
        .filter(stock_place::Column::StockItemId.eq(item_id))
        .filter(stock_place::Column::RackId.eq(at.rack_id))
        .filter(stock_place::Column::Shelf.eq(at.shelf))
        .filter(stock_place::Column::Cell.eq(at.cell))
        .one(conn)
        .await?
        .map_or(0, |r| r.quantity))
}

/// Добавляет штуки в ячейку (строка создаётся, если её не было).
pub(crate) async fn add_to_cell(
    conn: &impl ConnectionTrait,
    item_id: i64,
    at: StockCellRef,
    quantity: i64,
) -> Result<(), CoreError> {
    let existing = stock_place::Entity::find()
        .filter(stock_place::Column::StockItemId.eq(item_id))
        .filter(stock_place::Column::RackId.eq(at.rack_id))
        .filter(stock_place::Column::Shelf.eq(at.shelf))
        .filter(stock_place::Column::Cell.eq(at.cell))
        .one(conn)
        .await?;
    match existing {
        Some(row) => {
            stock_place::ActiveModel {
                id: Set(row.id),
                quantity: Set(row.quantity + quantity),
                ..Default::default()
            }
            .update(conn)
            .await?;
        }
        None => {
            stock_place::ActiveModel {
                id: NotSet,
                stock_item_id: Set(item_id),
                rack_id: Set(at.rack_id),
                shelf: Set(at.shelf),
                cell: Set(at.cell),
                quantity: Set(quantity),
            }
            .insert(conn)
            .await?;
        }
    }
    Ok(())
}

/// Живая позиция; удалённой и несуществующей нет.
async fn live_item(conn: &impl ConnectionTrait, id: i64) -> Result<stock_item::Model, CoreError> {
    stock_item::Entity::find_by_id(id)
        .one(conn)
        .await?
        .filter(|m| m.deleted_at.is_none())
        .ok_or(CoreError::NotFound { entity: ENTITY, id })
}

/// Позиция вместе с местами.
pub(crate) async fn item_with_places(
    conn: &impl ConnectionTrait,
    id: i64,
) -> Result<StockItem, CoreError> {
    let m = stock_item::Entity::find_by_id(id)
        .one(conn)
        .await?
        .ok_or(CoreError::NotFound { entity: ENTITY, id })?;
    let places = load_places(conn, &[id])
        .await?
        .remove(&id)
        .unwrap_or_default();
    Ok(item(m, places))
}

fn check_quantity(quantity: i64) -> Result<(), CoreError> {
    if quantity < 1 {
        return Err(invalid("quantity", "out_of_range"));
    }
    Ok(())
}

impl Core {
    /// Версия позиции растёт, время правки обновляется.
    async fn touch_item(
        &self,
        conn: &impl ConnectionTrait,
        id: i64,
    ) -> Result<chrono::NaiveDateTime, CoreError> {
        let now = self.now();
        stock_item::Entity::update_many()
            .col_expr(
                stock_item::Column::Version,
                Expr::col(stock_item::Column::Version).add(1),
            )
            .col_expr(stock_item::Column::UpdatedAt, Expr::value(now))
            .filter(stock_item::Column::Id.eq(id))
            .exec(conn)
            .await?;
        Ok(now)
    }

    /// Положить штуки из «Не разложено» в ячейку (право `stock_edit`).
    pub async fn put_stock(
        &self,
        actor: &Actor,
        id: i64,
        put: StockPut,
    ) -> Result<StockItem, CoreError> {
        actor.require(Permission::StockEdit)?;
        check_quantity(put.quantity)?;
        let tx = self.begin_write().await?;
        let current = live_item(&tx, id).await?;
        let code = resolve_cell(&tx, put.place)
            .await?
            .ok_or_else(|| invalid("place", "no_such_place"))?;
        let placed: i64 = stock_place::Entity::find()
            .filter(stock_place::Column::StockItemId.eq(id))
            .all(&tx)
            .await?
            .iter()
            .map(|p| p.quantity)
            .sum();
        if put.quantity > current.quantity - placed {
            return Err(invalid("quantity", "places_exceed"));
        }
        add_to_cell(&tx, id, put.place, put.quantity).await?;
        let now = self.touch_item(&tx, id).await?;
        write_audit(
            &tx,
            now,
            actor,
            NewAuditEntry {
                action: "stock.placed",
                order_id: None,
                client_id: None,
                stock_item_id: Some(id),
                summary: format!("Положено в {code}: {} шт.", put.quantity),
                details: Some(json!({ "place": code, "quantity": put.quantity })),
            },
        )
        .await?;
        let out = item_with_places(&tx, id).await?;
        tx.commit().await?;
        Ok(out)
    }

    /// Переложить штуки из одной ячейки в другую (право `stock_edit`).
    pub async fn move_stock(
        &self,
        actor: &Actor,
        id: i64,
        mv: StockMove,
    ) -> Result<StockItem, CoreError> {
        actor.require(Permission::StockEdit)?;
        check_quantity(mv.quantity)?;
        let tx = self.begin_write().await?;
        live_item(&tx, id).await?;
        let from = resolve_cell(&tx, mv.from)
            .await?
            .ok_or_else(|| invalid("from", "no_such_place"))?;
        let to = resolve_cell(&tx, mv.to)
            .await?
            .ok_or_else(|| invalid("to", "no_such_place"))?;
        if mv.from == mv.to {
            return item_with_places(&tx, id).await;
        }
        let have = amount_at(&tx, id, mv.from).await?;
        if mv.quantity > have {
            return Err(invalid("quantity", "places_exceed"));
        }
        let source = StockPlace {
            rack_id: mv.from.rack_id,
            shelf: mv.from.shelf,
            cell: mv.from.cell,
            code: from.clone(),
            quantity: have,
        };
        set_amount(&tx, id, &source, have - mv.quantity).await?;
        add_to_cell(&tx, id, mv.to, mv.quantity).await?;
        let now = self.touch_item(&tx, id).await?;
        write_audit(
            &tx,
            now,
            actor,
            NewAuditEntry {
                action: "stock.moved",
                order_id: None,
                client_id: None,
                stock_item_id: Some(id),
                summary: format!("Переложено из {from} в {to}: {} шт.", mv.quantity),
                details: Some(json!({ "from": from, "to": to, "quantity": mv.quantity })),
            },
        )
        .await?;
        let out = item_with_places(&tx, id).await?;
        tx.commit().await?;
        Ok(out)
    }

    /// Убрать всё из ячейки в «Не разложено» (право `stock_edit`); пустая ячейка — без изменений.
    pub async fn unplace_stock(
        &self,
        actor: &Actor,
        id: i64,
        at: StockCellRef,
    ) -> Result<StockItem, CoreError> {
        actor.require(Permission::StockEdit)?;
        let tx = self.begin_write().await?;
        live_item(&tx, id).await?;
        let code = resolve_cell(&tx, at)
            .await?
            .ok_or_else(|| invalid("place", "no_such_place"))?;
        let have = amount_at(&tx, id, at).await?;
        if have == 0 {
            return item_with_places(&tx, id).await;
        }
        let source = StockPlace {
            rack_id: at.rack_id,
            shelf: at.shelf,
            cell: at.cell,
            code: code.clone(),
            quantity: have,
        };
        set_amount(&tx, id, &source, 0).await?;
        let now = self.touch_item(&tx, id).await?;
        write_audit(
            &tx,
            now,
            actor,
            NewAuditEntry {
                action: "stock.unplaced",
                order_id: None,
                client_id: None,
                stock_item_id: Some(id),
                summary: format!("Убрано из {code}: {have} шт."),
                details: Some(json!({ "place": code, "quantity": have })),
            },
        )
        .await?;
        let out = item_with_places(&tx, id).await?;
        tx.commit().await?;
        Ok(out)
    }
}
