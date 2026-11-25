//! Позиции склада: список, сводка, позиция, добавление, правка, удаление, история.

use mw_db::entity::{stock_item, stock_place, stock_rack};
use mw_domain::permission::Permission;
use mw_domain::search::{like_contains, search_tokens};
use mw_domain::stock_place::{Grid, PlaceTarget, place_candidates, place_in_rack, rack_part};
use mw_domain::validation::FieldError;
use sea_orm::sea_query::{Condition, Expr, ExprTrait, LikeExpr, Order};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseTransaction, EntityTrait, NotSet,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, QueryTrait, Select, Set,
};
use serde_json::{Map, Value, json};

use super::places::{
    add_to_cell, item_with_places, load_places, reduce_places, reduced_json, reduced_tail,
    resolve_cell,
};
use super::{
    ENTITY, NewStockItem, StockFilter, StockItem, StockItemPatch, StockListing, StockSortField,
    StockSummary, deleted_error, display_name, stock_item as to_item, validate,
};
use crate::page::validate_page;
use crate::search_index::set_stock_keys;
use crate::sql::text_sort;
use crate::{
    Actor, AuditEntry, AuditFilter, Core, CoreError, EditVia, NewAuditEntry, Page, PageRequest,
    write_audit,
};

async fn load(conn: &impl ConnectionTrait, id: i64) -> Result<stock_item::Model, CoreError> {
    stock_item::Entity::find_by_id(id)
        .one(conn)
        .await?
        .ok_or(CoreError::NotFound { entity: ENTITY, id })
}

/// Неудалённые позиции.
fn live() -> Select<stock_item::Entity> {
    stock_item::Entity::find().filter(stock_item::Column::DeletedAt.is_null())
}

/// «Докупить»: порог задан, а остаток ниже него (как `mw_domain::stock::needs_restock`).
pub(super) fn restock_condition() -> Condition {
    Condition::all()
        .add(Expr::col(stock_item::Column::RestockThreshold).gt(0))
        .add(
            Expr::col(stock_item::Column::Quantity)
                .lt(Expr::col(stock_item::Column::RestockThreshold)),
        )
}

/// Запрос из одного целого числа — ещё и номер позиции.
fn item_number(query: &str) -> Option<i64> {
    let query = query.trim();
    if query.is_empty() || !query.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    query.parse().ok()
}

/// Все слова запроса есть в тексте позиции, или запрос — её номер.
pub(super) fn search_condition(query: &str) -> Condition {
    let mut words = Condition::all();
    for token in search_tokens(query) {
        words = words.add(
            Expr::col(stock_item::Column::SearchText)
                .like(LikeExpr::new(like_contains(&token)).escape('\\')),
        );
    }
    match item_number(query) {
        Some(id) => Condition::any()
            .add(words)
            .add(stock_item::Column::Id.eq(id)),
        None => words,
    }
}

/// Место, по которому ищут: шкаф, цель в нём и канонический код.
pub(super) struct PlaceScope {
    pub rack_id: i64,
    pub target: PlaceTarget,
    pub code: String,
    /// Слова `query` действуют вместе с местом (место задано явно).
    pub with_words: bool,
}

/// Шкаф и цель по коду (как есть или с русской раскладки); первое совпадение — ответ.
pub(crate) async fn resolve_place(
    conn: &impl ConnectionTrait,
    code: &str,
) -> Result<Option<(i64, PlaceTarget, String)>, CoreError> {
    let candidates = place_candidates(code);
    if candidates.is_empty() {
        return Ok(None);
    }
    let racks = stock_rack::Entity::find().all(conn).await?;
    for candidate in candidates {
        let Some(rack) = racks.iter().find(|r| r.code == rack_part(&candidate)) else {
            continue;
        };
        let grid = Grid {
            shelves: rack.shelves,
            cells_per_shelf: rack.cells_per_shelf,
        };
        if let Some(target) = place_in_rack(&candidate, &rack.code, grid) {
            return Ok(Some((rack.id, target, candidate)));
        }
    }
    Ok(None)
}

/// Место поиска: явное `place` (неузнанное — ошибка) или весь `query`, если он — код места.
pub(super) async fn place_scope(
    conn: &impl ConnectionTrait,
    filter: &StockFilter,
) -> Result<Option<PlaceScope>, CoreError> {
    let (code, with_words) = match &filter.place {
        Some(code) => (code.as_str(), true),
        None if !filter.as_text && !filter.query.trim().is_empty() => {
            (filter.query.as_str(), false)
        }
        None => return Ok(None),
    };
    match resolve_place(conn, code).await? {
        Some((rack_id, target, code)) => Ok(Some(PlaceScope {
            rack_id,
            target,
            code,
            with_words,
        })),
        None if with_words => Err(CoreError::Validation(vec![FieldError::new(
            "place",
            "no_such_place",
        )])),
        None => Ok(None),
    }
}

/// Места выбранного шкафа или ячейки.
pub(super) fn place_rows(scope: &PlaceScope) -> Select<stock_place::Entity> {
    let mut rows =
        stock_place::Entity::find().filter(stock_place::Column::RackId.eq(scope.rack_id));
    if let PlaceTarget::Cell { shelf, cell } = scope.target {
        rows = rows
            .filter(stock_place::Column::Shelf.eq(shelf))
            .filter(stock_place::Column::Cell.eq(cell));
    }
    rows
}

/// Позиции с остатком в месте.
fn place_condition(scope: &PlaceScope) -> Condition {
    Condition::all().add(
        stock_item::Column::Id.in_subquery(
            place_rows(scope)
                .select_only()
                .column(stock_place::Column::StockItemId)
                .into_query(),
        ),
    )
}

/// Условие списка: место (если узнано) и слова, затем «Докупить».
pub(super) fn filter_condition(filter: &StockFilter, scope: Option<&PlaceScope>) -> Condition {
    let mut cond = Condition::all();
    match scope {
        Some(s) => {
            cond = cond.add(place_condition(s));
            if s.with_words {
                cond = cond.add(search_condition(&filter.query));
            }
        }
        None => cond = cond.add(search_condition(&filter.query)),
    }
    if filter.restock {
        cond = cond.add(restock_condition());
    }
    cond
}

/// Накопитель различий: поле, русское название, было, стало.
#[derive(Default)]
struct Diff {
    changes: Map<String, Value>,
    labels: Vec<&'static str>,
}

impl Diff {
    fn note(&mut self, field: &str, label: &'static str, old: Value, new: Value) {
        self.changes.insert(field.to_owned(), json!([old, new]));
        if !self.labels.contains(&label) {
            self.labels.push(label);
        }
    }
}

/// Применяет значение правки к полю и записывает различие.
macro_rules! apply {
    ($diff:ident, $m:ident, $field:ident, $label:expr, $new:expr) => {
        if let Some(new) = $new {
            if new != $m.$field {
                $diff.note(stringify!($field), $label, json!($m.$field), json!(new));
                $m.$field = new;
            }
        }
    };
}

fn trimmed(v: Option<String>) -> Option<String> {
    v.map(|s| s.trim().to_owned())
}

impl Core {
    /// Список склада (право `stock_view`): поиск, «Докупить», сортировка, страница.
    pub async fn list_stock(
        &self,
        actor: &Actor,
        filter: &StockFilter,
        limit: u64,
        offset: u64,
    ) -> Result<Page<StockItem>, CoreError> {
        let l = self.search_stock(actor, filter, limit, offset).await?;
        Ok(Page {
            items: l.items,
            total: l.total,
        })
    }

    /// Поиск склада (право `stock_view`): как `list_stock`, плюс узнанное место. Код места в
    /// `query` (и с русской раскладки) — только позиции оттуда; `place` — явно, вместе со словами.
    pub async fn search_stock(
        &self,
        actor: &Actor,
        filter: &StockFilter,
        limit: u64,
        offset: u64,
    ) -> Result<StockListing, CoreError> {
        actor.require(Permission::StockView)?;
        validate_page(limit, offset)?;
        let backend = self.db.get_database_backend();
        let scope = place_scope(&self.db, filter).await?;
        let select = live().filter(filter_condition(filter, scope.as_ref()));
        let total = select.clone().count(&self.db).await?;
        let dir = if filter.sort.descending {
            Order::Desc
        } else {
            Order::Asc
        };
        let name_key = || text_sort(backend, stock_item::Column::NameKey.into_expr());
        let select = match filter.sort.field {
            StockSortField::Id => select.order_by(stock_item::Column::Id, dir),
            StockSortField::Name => select.order_by(name_key(), dir),
            StockSortField::Quantity => select.order_by(stock_item::Column::Quantity, dir),
            StockSortField::Price => select.order_by(stock_item::Column::Price, dir),
        };
        let rows = select
            .order_by_asc(name_key())
            .order_by_asc(stock_item::Column::Id)
            .limit(limit)
            .offset(offset)
            .all(&self.db)
            .await?;
        let ids: Vec<i64> = rows.iter().map(|m| m.id).collect();
        let mut places = load_places(&self.db, &ids).await?;
        let items = rows
            .into_iter()
            .map(|m| {
                let own = places.remove(&m.id).unwrap_or_default();
                to_item(m, own)
            })
            .collect();
        Ok(StockListing {
            items,
            total,
            place: scope.map(|s| s.code),
        })
    }

    /// Сводка склада (право `stock_view`): сколько позиций надо докупить.
    pub async fn stock_summary(&self, actor: &Actor) -> Result<StockSummary, CoreError> {
        actor.require(Permission::StockView)?;
        let restock = live().filter(restock_condition()).count(&self.db).await?;
        Ok(StockSummary { restock })
    }

    /// Позиция склада (право `stock_view`); удалённой — «не найдена».
    pub async fn get_stock_item(&self, actor: &Actor, id: i64) -> Result<StockItem, CoreError> {
        actor.require(Permission::StockView)?;
        let model = live()
            .filter(stock_item::Column::Id.eq(id))
            .one(&self.db)
            .await?
            .ok_or(CoreError::NotFound { entity: ENTITY, id })?;
        let places = load_places(&self.db, &[id])
            .await?
            .remove(&id)
            .unwrap_or_default();
        Ok(to_item(model, places))
    }

    /// Новая позиция склада (право `stock_create`).
    pub async fn create_stock_item(
        &self,
        actor: &Actor,
        new: NewStockItem,
    ) -> Result<StockItem, CoreError> {
        actor.require(Permission::StockCreate)?;
        let now = self.now();
        let mut m = stock_item::Model {
            id: 0,
            name: new.name.trim().to_owned(),
            device_type: new.device_type.trim().to_owned(),
            part_type: new.part_type.trim().to_owned(),
            brand: new.brand.trim().to_owned(),
            model: new.model.trim().to_owned(),
            color: new.color.trim().to_owned(),
            note: new.note.trim().to_owned(),
            quantity: new.quantity,
            price: new.price.kopecks(),
            restock_threshold: new.restock_threshold,
            search_text: String::new(),
            name_key: String::new(),
            created_at: now,
            updated_at: now,
            deleted_at: None,
            version: 1,
        };
        validate(&m, |_| true)?;
        let place = new.place.filter(|_| m.quantity > 0);
        // Наименование проверено непустым: выводимое не зависит от номера, которого ещё нет.
        set_stock_keys(&mut m);
        let tx = self.begin_write().await?;
        let placed = match place {
            Some(at) => {
                let code = resolve_cell(&tx, at).await?.ok_or_else(|| {
                    CoreError::Validation(vec![FieldError::new("place", "no_such_place")])
                })?;
                Some((at, code))
            }
            None => None,
        };
        let model = stock_item::ActiveModel {
            id: NotSet,
            name: Set(m.name),
            device_type: Set(m.device_type),
            part_type: Set(m.part_type),
            brand: Set(m.brand),
            model: Set(m.model),
            color: Set(m.color),
            note: Set(m.note),
            quantity: Set(m.quantity),
            price: Set(m.price),
            restock_threshold: Set(m.restock_threshold),
            search_text: Set(m.search_text),
            name_key: Set(m.name_key),
            created_at: Set(now),
            updated_at: Set(now),
            deleted_at: Set(None),
            version: Set(1),
        }
        .insert(&tx)
        .await?;
        let item_id = model.id;
        let details = match &placed {
            Some((at, code)) => {
                add_to_cell(&tx, item_id, *at, model.quantity).await?;
                json!({ "id": item_id, "placed": { "code": code, "quantity": model.quantity } })
            }
            None => json!({ "id": item_id }),
        };
        let item = item_with_places(&tx, item_id).await?;
        write_audit(
            &tx,
            now,
            actor,
            NewAuditEntry {
                action: "stock.created",
                order_id: None,
                client_id: None,
                stock_item_id: Some(item.id),
                summary: format!("Добавлена позиция склада «{}»", item.display_name),
                details: Some(details),
            },
        )
        .await?;
        tx.commit().await?;
        Ok(item)
    }

    /// Частичная правка позиции (право `stock_edit`) с проверкой версии и журналом различий.
    pub async fn update_stock_item(
        &self,
        actor: &Actor,
        id: i64,
        expected_version: i64,
        patch: StockItemPatch,
    ) -> Result<StockItem, CoreError> {
        actor.require(Permission::StockEdit)?;
        let tx = self.begin_write().await?;
        let (updated, _) = self
            .update_stock_item_in(&tx, actor, id, expected_version, patch, None)
            .await?;
        tx.commit().await?;
        Ok(updated)
    }

    /// Правка позиции внутри открытой транзакции записи: всё, что делает `update_stock_item`,
    /// кроме проверки права и фиксации (замена типа запчасти из справочника идёт через неё же, с
    /// пометкой `via`). Возвращает позицию и признак, что запись изменилась.
    pub(crate) async fn update_stock_item_in(
        &self,
        tx: &DatabaseTransaction,
        actor: &Actor,
        id: i64,
        expected_version: i64,
        patch: StockItemPatch,
        via: Option<EditVia>,
    ) -> Result<(StockItem, bool), CoreError> {
        let current = load(tx, id).await?;
        if current.deleted_at.is_some() {
            return Err(deleted_error());
        }
        let mut m = current.clone();
        let mut d = Diff::default();
        apply!(d, m, name, "наименование", trimmed(patch.name));
        apply!(
            d,
            m,
            device_type,
            "тип устройства",
            trimmed(patch.device_type)
        );
        apply!(d, m, part_type, "тип запчасти", trimmed(patch.part_type));
        apply!(d, m, brand, "бренд", trimmed(patch.brand));
        apply!(d, m, model, "модель", trimmed(patch.model));
        apply!(d, m, color, "цвет", trimmed(patch.color));
        apply!(d, m, note, "примечание", trimmed(patch.note));
        apply!(d, m, quantity, "количество", patch.quantity);
        apply!(d, m, price, "цена", patch.price.map(|p| p.kopecks()));
        apply!(
            d,
            m,
            restock_threshold,
            "докупить при",
            patch.restock_threshold
        );

        if d.changes.is_empty() {
            let places = load_places(tx, &[id])
                .await?
                .remove(&id)
                .unwrap_or_default();
            return Ok((to_item(current, places), false));
        }
        if current.version != expected_version {
            return Err(CoreError::Conflict {
                entity: ENTITY,
                id,
                current_version: current.version,
            });
        }
        // Пределы проверяются только у полей, которые правка меняет.
        validate(&m, |field| d.changes.contains_key(field))?;

        let now = self.now();
        set_stock_keys(&mut m);
        let res = stock_item::Entity::update_many()
            .col_expr(stock_item::Column::Name, Expr::value(m.name.clone()))
            .col_expr(
                stock_item::Column::DeviceType,
                Expr::value(m.device_type.clone()),
            )
            .col_expr(
                stock_item::Column::PartType,
                Expr::value(m.part_type.clone()),
            )
            .col_expr(stock_item::Column::Brand, Expr::value(m.brand.clone()))
            .col_expr(stock_item::Column::Model, Expr::value(m.model.clone()))
            .col_expr(stock_item::Column::Color, Expr::value(m.color.clone()))
            .col_expr(stock_item::Column::Note, Expr::value(m.note.clone()))
            .col_expr(stock_item::Column::Quantity, Expr::value(m.quantity))
            .col_expr(stock_item::Column::Price, Expr::value(m.price))
            .col_expr(
                stock_item::Column::RestockThreshold,
                Expr::value(m.restock_threshold),
            )
            .col_expr(
                stock_item::Column::SearchText,
                Expr::value(m.search_text.clone()),
            )
            .col_expr(stock_item::Column::NameKey, Expr::value(m.name_key.clone()))
            .col_expr(stock_item::Column::UpdatedAt, Expr::value(now))
            .col_expr(
                stock_item::Column::Version,
                Expr::col(stock_item::Column::Version).add(1),
            )
            .filter(stock_item::Column::Id.eq(id))
            .filter(stock_item::Column::Version.eq(expected_version))
            .filter(stock_item::Column::DeletedAt.is_null())
            .exec(tx)
            .await?;
        check_affected(tx, res.rows_affected, id).await?;
        let reduced = if d.changes.contains_key("quantity") {
            reduce_places(tx, id, m.quantity).await?
        } else {
            Vec::new()
        };

        let mut summary = format!("Изменено: {}", d.labels.join(", "));
        // Пометка «из справочника» — только в тексте записи, не среди полей правки.
        if let Some(via) = via {
            summary.push_str(via.suffix_ru());
        }
        let mut details = json!({ "changes": d.changes });
        if !reduced.is_empty() {
            summary.push_str(&reduced_tail(&reduced));
            details["places_reduced"] = reduced_json(&reduced);
        }
        write_audit(
            tx,
            now,
            actor,
            NewAuditEntry {
                action: "stock.updated",
                order_id: None,
                client_id: None,
                stock_item_id: Some(id),
                summary,
                details: Some(details),
            },
        )
        .await?;
        Ok((item_with_places(tx, id).await?, true))
    }

    /// Мягкое удаление позиции (право `stock_delete`).
    pub async fn delete_stock_item(
        &self,
        actor: &Actor,
        id: i64,
        expected_version: i64,
    ) -> Result<(), CoreError> {
        actor.require(Permission::StockDelete)?;
        let tx = self.begin_write().await?;
        let current = load(&tx, id).await?;
        if current.deleted_at.is_some() {
            return Err(deleted_error());
        }
        if current.version != expected_version {
            return Err(CoreError::Conflict {
                entity: ENTITY,
                id,
                current_version: current.version,
            });
        }
        let now = self.now();
        let res = stock_item::Entity::update_many()
            .col_expr(stock_item::Column::DeletedAt, Expr::value(Some(now)))
            .col_expr(stock_item::Column::UpdatedAt, Expr::value(now))
            .col_expr(
                stock_item::Column::Version,
                Expr::col(stock_item::Column::Version).add(1),
            )
            .filter(stock_item::Column::Id.eq(id))
            .filter(stock_item::Column::Version.eq(expected_version))
            .filter(stock_item::Column::DeletedAt.is_null())
            .exec(&tx)
            .await?;
        check_affected(&tx, res.rows_affected, id).await?;
        // Удалённая позиция не занимает ячейки: места снимаются все, в журнале остаётся, где лежала.
        let freed = reduce_places(&tx, id, 0).await?;
        let details =
            (!freed.is_empty()).then(|| json!({ "places_reduced": reduced_json(&freed) }));
        write_audit(
            &tx,
            now,
            actor,
            NewAuditEntry {
                action: "stock.deleted",
                order_id: None,
                client_id: None,
                stock_item_id: Some(id),
                summary: format!("Удалена позиция склада «{}»", display_name(&current)),
                details,
            },
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Записи журнала позиции, новые сверху (право `stock_view`, без `audit_view`). Удалённая
    /// позиция отдаётся, как и история удалённого заказа: в ней видно, кто её удалил. Нет
    /// позиции — «не найдена».
    pub async fn stock_item_history(
        &self,
        actor: &Actor,
        id: i64,
        page: PageRequest,
    ) -> Result<Page<AuditEntry>, CoreError> {
        actor.require(Permission::StockView)?;
        validate_page(page.limit, page.offset)?;
        load(&self.db, id).await?;
        let filter = AuditFilter {
            stock_item_id: Some(id),
            ..Default::default()
        };
        self.list_audit(actor, &filter, page.limit, page.offset)
            .await
    }
}

/// Ноль затронутых строк: «удалена» или конфликт версий, если позиция есть, иначе «не найдена».
async fn check_affected(conn: &impl ConnectionTrait, rows: u64, id: i64) -> Result<(), CoreError> {
    if rows > 0 {
        return Ok(());
    }
    let m = load(conn, id).await?;
    if m.deleted_at.is_some() {
        return Err(deleted_error());
    }
    Err(CoreError::Conflict {
        entity: ENTITY,
        id,
        current_version: m.version,
    })
}

#[cfg(test)]
mod tests {
    use super::item_number;

    #[test]
    fn only_a_whole_number_is_an_item_number() {
        assert_eq!(item_number(" 41 "), Some(41));
        assert_eq!(item_number("0041"), Some(41));
        for query in [
            "",
            " ",
            "41a",
            "4 1",
            "-41",
            "+41",
            "41.0",
            "99999999999999999999",
        ] {
            assert_eq!(item_number(query), None, "{query:?}");
        }
    }
}
