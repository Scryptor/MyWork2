//! Шкафы склада: список с картой ячеек, добавление, правка, удаление, порядок.

use std::collections::{HashMap, HashSet};

use mw_db::entity::{service_point, stock_item, stock_place, stock_rack};
use mw_domain::permission::Permission;
use mw_domain::stock_place::{
    Grid, MAX_RACK_LABEL_CHARS, cell_code, codes_change, lost_cells, normalize_rack_code,
};
use mw_domain::validation::FieldError;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, NotSet, QueryFilter, QueryOrder,
    QuerySelect, QueryTrait, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::StockFilter;
use super::items::{restock_condition, search_condition};
use crate::statuses::{check_permutation, check_seen_order};
use crate::{Actor, Core, CoreError, NewAuditEntry, write_audit};

/// Название сущности в ошибках «не найдено».
const ENTITY: &str = "шкаф";
/// Сколько кодов ячеек называет отказ.
const REFUSAL_CELLS: usize = 10;

/// Ячейка шкафа на карте.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct StockCell {
    /// Номер полки, с 1.
    pub shelf: i32,
    /// Номер ячейки на полке, с 1.
    pub cell: i32,
    /// Код ячейки, например `B-2-4`.
    pub code: String,
    /// Сколько позиций лежит в ячейке.
    pub positions: i64,
    /// В ячейке есть позиция, подходящая под поиск или «Докупить»; без фильтра — `false`.
    pub found: bool,
}

/// Шкаф склада со всей сеткой ячеек.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct StockRack {
    /// Номер шкафа.
    pub id: i64,
    /// Код: заглавные латинские буквы и цифры, до 8 знаков.
    pub code: String,
    /// Подпись.
    pub label: String,
    /// Сколько полок.
    pub shelves: i32,
    /// Сколько ячеек на каждой полке.
    pub cells_per_shelf: i32,
    /// Точка обслуживания, где стоит шкаф.
    #[schema(required = true)]
    pub service_point_id: Option<i64>,
    /// Порядковый номер в списке.
    pub sort_order: i32,
    /// Все ячейки: полка за полкой, ячейка за ячейкой.
    pub cells: Vec<StockCell>,
}

/// Новый шкаф.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, ts_rs::TS, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewStockRack {
    /// Код шкафа.
    pub code: String,
    /// Подпись, до 100 знаков.
    pub label: String,
    /// Сколько полок, 1…20.
    pub shelves: i32,
    /// Сколько ячеек на полке, 1…20; всего в шкафу не больше 100.
    pub cells_per_shelf: i32,
    /// Точка обслуживания (не в архиве); `null` — не указана.
    #[serde(deserialize_with = "crate::patch::required_nullable")]
    #[schema(required = true)]
    pub service_point_id: Option<i64>,
}

/// Частичная правка шкафа; не переданное поле не меняется.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, ts_rs::TS, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
#[ts(optional_fields)]
pub struct StockRackPatch {
    /// Код шкафа.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub code: Option<String>,
    /// Подпись.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub label: Option<String>,
    /// Сколько полок.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub shelves: Option<i32>,
    /// Сколько ячеек на полке.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub cells_per_shelf: Option<i32>,
    /// Точка обслуживания; `null` — не указана.
    #[serde(default, deserialize_with = "crate::patch::nullable")]
    pub service_point_id: Option<Option<i64>>,
}

fn grid(m: &stock_rack::Model) -> Grid {
    Grid {
        shelves: m.shelves,
        cells_per_shelf: m.cells_per_shelf,
    }
}

async fn load(conn: &impl ConnectionTrait, id: i64) -> Result<stock_rack::Model, CoreError> {
    stock_rack::Entity::find_by_id(id)
        .one(conn)
        .await?
        .ok_or(CoreError::NotFound { entity: ENTITY, id })
}

async fn all_rows(conn: &impl ConnectionTrait) -> Result<Vec<stock_rack::Model>, CoreError> {
    Ok(stock_rack::Entity::find()
        .order_by_asc(stock_rack::Column::SortOrder)
        .order_by_asc(stock_rack::Column::Id)
        .all(conn)
        .await?)
}

/// Занятые ячейки шкафа (с хотя бы одним местом), по полкам и ячейкам.
pub(crate) async fn occupied_cells(
    conn: &impl ConnectionTrait,
    rack_id: i64,
) -> Result<Vec<(i32, i32)>, CoreError> {
    let mut cells: Vec<(i32, i32)> = stock_place::Entity::find()
        .filter(stock_place::Column::RackId.eq(rack_id))
        .all(conn)
        .await?
        .into_iter()
        .map(|p| (p.shelf, p.cell))
        .collect();
    cells.sort_unstable();
    cells.dedup();
    Ok(cells)
}

/// Отказ: в этих ячейках лежат запчасти; называет не больше десяти кодов.
pub(crate) fn occupied_refusal(code: &str, grid: Grid, cells: &[(i32, i32)]) -> CoreError {
    let mut names: Vec<String> = cells
        .iter()
        .take(REFUSAL_CELLS)
        .map(|&(shelf, cell)| cell_code(code, grid, shelf, cell))
        .collect();
    let more = cells.len().saturating_sub(REFUSAL_CELLS);
    let text = if cells.len() == 1 {
        format!(
            "В ячейке {} лежат запчасти — переложите их",
            names.remove(0)
        )
    } else if more > 0 {
        format!(
            "В ячейках {} и ещё {more} лежат запчасти — переложите их",
            names.join(", ")
        )
    } else {
        format!(
            "В ячейках {} лежат запчасти — переложите их",
            names.join(", ")
        )
    };
    CoreError::InUse(text)
}

/// Позиции, подходящие под фильтр; `None` — фильтра нет, «найдено» нигде нет.
fn matching_items(filter: &StockFilter) -> Option<sea_orm::sea_query::SelectStatement> {
    if filter.query.trim().is_empty() && !filter.restock {
        return None;
    }
    let mut select = stock_item::Entity::find()
        .filter(stock_item::Column::DeletedAt.is_null())
        .filter(search_condition(&filter.query));
    if filter.restock {
        select = select.filter(restock_condition());
    }
    Some(
        select
            .select_only()
            .column(stock_item::Column::Id)
            .into_query(),
    )
}

/// Шкафы с картой: счётчики позиций и признак «найдено» по фильтру.
async fn build(
    conn: &impl ConnectionTrait,
    racks: Vec<stock_rack::Model>,
    filter: &StockFilter,
) -> Result<Vec<StockRack>, CoreError> {
    let mut counts: HashMap<(i64, i32, i32), i64> = HashMap::new();
    for p in stock_place::Entity::find().all(conn).await? {
        *counts.entry((p.rack_id, p.shelf, p.cell)).or_default() += 1;
    }
    let mut found: HashSet<(i64, i32, i32)> = HashSet::new();
    if let Some(items) = matching_items(filter) {
        for p in stock_place::Entity::find()
            .filter(stock_place::Column::StockItemId.in_subquery(items))
            .all(conn)
            .await?
        {
            found.insert((p.rack_id, p.shelf, p.cell));
        }
    }
    Ok(racks
        .into_iter()
        .map(|r| {
            let g = grid(&r);
            let cells = g
                .cells()
                .map(|(shelf, cell)| {
                    let key = (r.id, shelf, cell);
                    StockCell {
                        shelf,
                        cell,
                        code: cell_code(&r.code, g, shelf, cell),
                        positions: counts.get(&key).copied().unwrap_or(0),
                        found: found.contains(&key),
                    }
                })
                .collect();
            StockRack {
                id: r.id,
                code: r.code,
                label: r.label,
                shelves: r.shelves,
                cells_per_shelf: r.cells_per_shelf,
                service_point_id: r.service_point_id,
                sort_order: r.sort_order,
                cells,
            }
        })
        .collect())
}

async fn one(conn: &impl ConnectionTrait, id: i64) -> Result<StockRack, CoreError> {
    let rack = load(conn, id).await?;
    Ok(build(conn, vec![rack], &StockFilter::default())
        .await?
        .remove(0))
}

/// Точка есть и не в архиве.
async fn point_is_live(conn: &impl ConnectionTrait, id: i64) -> Result<bool, CoreError> {
    Ok(service_point::Entity::find_by_id(id)
        .one(conn)
        .await?
        .is_some_and(|p| !p.archived))
}

fn snapshot(m: &stock_rack::Model) -> Value {
    json!({
        "code": m.code,
        "label": m.label,
        "shelves": m.shelves,
        "cells_per_shelf": m.cells_per_shelf,
        "service_point_id": m.service_point_id,
    })
}

fn too_long(label: &str) -> bool {
    label.chars().count() > MAX_RACK_LABEL_CHARS
}

impl Core {
    /// Шкафы с картой ячеек (право `stock_view`). `found` — в ячейке есть живая позиция, подходящая
    /// под поиск и «Докупить» фильтра.
    pub async fn list_stock_racks(
        &self,
        actor: &Actor,
        filter: &StockFilter,
    ) -> Result<Vec<StockRack>, CoreError> {
        actor.require(Permission::StockView)?;
        let racks = all_rows(&self.db).await?;
        build(&self.db, racks, filter).await
    }

    /// Новый шкаф в конце списка (право `stock_edit`).
    pub async fn create_stock_rack(
        &self,
        actor: &Actor,
        new: NewStockRack,
    ) -> Result<StockRack, CoreError> {
        actor.require(Permission::StockEdit)?;
        let tx = self.begin_write().await?;
        let rows = all_rows(&tx).await?;
        let mut errors = Vec::new();
        let code = normalize_rack_code(&new.code);
        match &code {
            None => errors.push(FieldError::new("code", "rack_code")),
            Some(c) if rows.iter().any(|r| &r.code == c) => {
                errors.push(FieldError::new("code", "duplicate"));
            }
            Some(_) => {}
        }
        let g = Grid {
            shelves: new.shelves,
            cells_per_shelf: new.cells_per_shelf,
        };
        errors.extend(g.validate());
        let label = new.label.trim().to_owned();
        if too_long(&label) {
            errors.push(FieldError::new("label", "too_long"));
        }
        if let Some(id) = new.service_point_id
            && !point_is_live(&tx, id).await?
        {
            errors.push(FieldError::new("service_point_id", "invalid"));
        }
        let Some(code) = code.filter(|_| errors.is_empty()) else {
            return Err(CoreError::Validation(errors));
        };
        let sort_order = rows.iter().map(|r| r.sort_order).max().map_or(0, |m| m + 1);
        let model = stock_rack::ActiveModel {
            id: NotSet,
            code: Set(code.clone()),
            label: Set(label.clone()),
            shelves: Set(new.shelves),
            cells_per_shelf: Set(new.cells_per_shelf),
            service_point_id: Set(new.service_point_id),
            sort_order: Set(sort_order),
        }
        .insert(&tx)
        .await?;
        let name = if label.is_empty() {
            String::new()
        } else {
            format!(" «{label}»")
        };
        write_audit(
            &tx,
            self.now(),
            actor,
            NewAuditEntry {
                action: "stock_rack.created",
                order_id: None,
                client_id: None,
                stock_item_id: None,
                summary: format!(
                    "Добавлен шкаф {code}{name}, {} × {}",
                    new.shelves, new.cells_per_shelf
                ),
                details: Some(json!({ "id": model.id, "rack": snapshot(&model) })),
            },
        )
        .await?;
        let rack = one(&tx, model.id).await?;
        tx.commit().await?;
        Ok(rack)
    }

    /// Правка шкафа (право `stock_edit`). Ячейки, которых нет в новой сетке, должны быть пусты.
    pub async fn update_stock_rack(
        &self,
        actor: &Actor,
        id: i64,
        patch: StockRackPatch,
    ) -> Result<StockRack, CoreError> {
        actor.require(Permission::StockEdit)?;
        let tx = self.begin_write().await?;
        let current = load(&tx, id).await?;
        let mut m = current.clone();
        let mut errors = Vec::new();
        if let Some(raw) = &patch.code {
            match normalize_rack_code(raw) {
                None => errors.push(FieldError::new("code", "rack_code")),
                Some(code) if code != m.code => {
                    let taken = stock_rack::Entity::find()
                        .filter(stock_rack::Column::Code.eq(code.as_str()))
                        .one(&tx)
                        .await?
                        .is_some();
                    if taken {
                        errors.push(FieldError::new("code", "duplicate"));
                    }
                    m.code = code;
                }
                Some(_) => {}
            }
        }
        let mut grid_touched = false;
        if let Some(shelves) = patch.shelves.filter(|&s| s != m.shelves) {
            m.shelves = shelves;
            grid_touched = true;
        }
        if let Some(cells) = patch.cells_per_shelf.filter(|&c| c != m.cells_per_shelf) {
            m.cells_per_shelf = cells;
            grid_touched = true;
        }
        if grid_touched {
            errors.extend(grid(&m).validate());
        }
        if let Some(label) = patch.label.as_deref().map(str::trim)
            && label != m.label
        {
            if too_long(label) {
                errors.push(FieldError::new("label", "too_long"));
            }
            label.clone_into(&mut m.label);
        }
        if let Some(point) = patch.service_point_id
            && point != m.service_point_id
        {
            if let Some(point_id) = point
                && !point_is_live(&tx, point_id).await?
            {
                errors.push(FieldError::new("service_point_id", "invalid"));
            }
            m.service_point_id = point;
        }
        if !errors.is_empty() {
            return Err(CoreError::Validation(errors));
        }
        if m == current {
            return one(&tx, id).await;
        }
        if grid_touched {
            let lost: HashSet<(i32, i32)> =
                lost_cells(grid(&current), grid(&m)).into_iter().collect();
            let blocked: Vec<(i32, i32)> = occupied_cells(&tx, id)
                .await?
                .into_iter()
                .filter(|c| lost.contains(c))
                .collect();
            if !blocked.is_empty() {
                return Err(occupied_refusal(&current.code, grid(&current), &blocked));
            }
        }
        stock_rack::ActiveModel {
            id: Set(id),
            code: Set(m.code.clone()),
            label: Set(m.label.clone()),
            shelves: Set(m.shelves),
            cells_per_shelf: Set(m.cells_per_shelf),
            service_point_id: Set(m.service_point_id),
            ..Default::default()
        }
        .update(&tx)
        .await?;
        let changed = codes_change(&current.code, grid(&current), &m.code, grid(&m));
        write_audit(
            &tx,
            self.now(),
            actor,
            NewAuditEntry {
                action: "stock_rack.updated",
                order_id: None,
                client_id: None,
                stock_item_id: None,
                summary: format!("Изменён шкаф {}", m.code),
                details: Some(json!({
                    "id": id,
                    "was": snapshot(&current),
                    "became": snapshot(&m),
                    "codes_changed": changed,
                })),
            },
        )
        .await?;
        let rack = one(&tx, id).await?;
        tx.commit().await?;
        Ok(rack)
    }

    /// Удаление пустого шкафа насовсем (право `stock_edit`).
    pub async fn delete_stock_rack(&self, actor: &Actor, id: i64) -> Result<(), CoreError> {
        actor.require(Permission::StockEdit)?;
        let tx = self.begin_write().await?;
        let rack = load(&tx, id).await?;
        let occupied = occupied_cells(&tx, id).await?;
        if !occupied.is_empty() {
            return Err(occupied_refusal(&rack.code, grid(&rack), &occupied));
        }
        stock_rack::Entity::delete_by_id(id).exec(&tx).await?;
        write_audit(
            &tx,
            self.now(),
            actor,
            NewAuditEntry {
                action: "stock_rack.deleted",
                order_id: None,
                client_id: None,
                stock_item_id: None,
                summary: format!("Удалён шкаф {}", rack.code),
                details: Some(json!({ "id": id, "rack": snapshot(&rack) })),
            },
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Новый порядок шкафов (право `stock_edit`): номера всех шкафов; `seen_ids` — порядок, который
    /// видел экран.
    pub async fn reorder_stock_racks(
        &self,
        actor: &Actor,
        ordered_ids: &[i64],
        seen_ids: Option<&[i64]>,
    ) -> Result<(), CoreError> {
        actor.require(Permission::StockEdit)?;
        let tx = self.begin_write().await?;
        let rows = all_rows(&tx).await?;
        check_permutation(ordered_ids, rows.iter().map(|r| r.id))?;
        if rows.iter().map(|r| r.id).eq(ordered_ids.iter().copied()) {
            return Ok(());
        }
        if let Some(seen) = seen_ids {
            check_seen_order(seen, rows.iter().map(|r| (r.id, r.sort_order)))?;
        }
        for (position, id) in (0_i32..).zip(ordered_ids) {
            stock_rack::ActiveModel {
                id: Set(*id),
                sort_order: Set(position),
                ..Default::default()
            }
            .update(&tx)
            .await?;
        }
        write_audit(
            &tx,
            self.now(),
            actor,
            NewAuditEntry {
                action: "stock_racks.reordered",
                order_id: None,
                client_id: None,
                stock_item_id: None,
                summary: "Изменён порядок шкафов".to_owned(),
                details: Some(json!({ "ordered_ids": ordered_ids })),
            },
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
}
