//! История заказа: записи журнала без права на просмотр всего журнала.

use chrono::NaiveDateTime;
use mw_db::entity::{audit_log, order};
use sea_orm::{
    ColumnTrait, Condition, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
};

use crate::page::validate_page;
use crate::{Actor, AuditEntry, Core, CoreError, Page, PageRequest};

impl Core {
    /// Записи журнала заказа, новые сверху. Права `audit_view` не нужно (достаточно войти);
    /// нет заказа — «не найден».
    pub async fn order_history(
        &self,
        actor: &Actor,
        id: i64,
        page: PageRequest,
    ) -> Result<Page<AuditEntry>, CoreError> {
        self.order_history_before(actor, id, page, None).await
    }

    /// То же, но только записи после курсора `(время, номер)` в порядке показа (время и номер по
    /// убыванию): листание по курсору не повторяет страницу, когда сверху успели прибавиться новые
    /// записи, и не сбивается, если время записи меньше, чем у более старой по номеру (перевод
    /// часов). `total` — всех записей заказа.
    pub async fn order_history_before(
        &self,
        _actor: &Actor,
        id: i64,
        page: PageRequest,
        before: Option<(NaiveDateTime, i64)>,
    ) -> Result<Page<AuditEntry>, CoreError> {
        validate_page(page.limit, page.offset)?;
        // Удалённый заказ, как и в `get_order`, отдаётся: в истории видно, кто его удалил.
        if order::Entity::find_by_id(id).one(&self.db).await?.is_none() {
            return Err(CoreError::NotFound {
                entity: "заказ",
                id,
            });
        }
        let select = audit_log::Entity::find().filter(audit_log::Column::OrderId.eq(id));
        let total = select.clone().count(&self.db).await?;
        let select = match before {
            Some((at, b)) => select.filter(
                Condition::any().add(audit_log::Column::At.lt(at)).add(
                    Condition::all()
                        .add(audit_log::Column::At.eq(at))
                        .add(audit_log::Column::Id.lt(b)),
                ),
            ),
            None => select,
        };
        let items = select
            .order_by_desc(audit_log::Column::At)
            .order_by_desc(audit_log::Column::Id)
            .limit(page.limit)
            .offset(page.offset)
            .all(&self.db)
            .await?
            .into_iter()
            .map(AuditEntry::from)
            .collect();
        Ok(Page { items, total })
    }
}
