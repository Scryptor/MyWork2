//! История заказа: записи журнала без права на просмотр всего журнала.

use mw_db::entity::{audit_log, order};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect};

use crate::page::validate_page;
use crate::{Actor, AuditEntry, Core, CoreError, Page, PageRequest};

impl Core {
    /// Записи журнала заказа, новые сверху. Права `audit_view` не нужно (достаточно войти);
    /// нет заказа — «не найден».
    pub async fn order_history(
        &self,
        _actor: &Actor,
        id: i64,
        page: PageRequest,
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
