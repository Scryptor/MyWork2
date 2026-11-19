use mw_db::entity::{attachment, order, service_point, status};
use mw_domain::barcode::order_barcode;
use mw_domain::money::Money;
use mw_domain::permission::Permission;
use mw_domain::status::StatusKind;
use mw_domain::validation::FieldError;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, NotSet, QueryFilter, QueryOrder,
    Set,
};
use serde_json::json;

use super::types::warranty_columns;
use crate::attachments::valid_key;
use crate::clients::{find_live, insert_client};
use crate::search_index::{order_search_text, set_order_keys};
use crate::{Actor, ClientRef, Core, CoreError, NewAuditEntry, NewOrder, Order, write_audit};

const ENTITY: &str = "заказ";

fn trimmed(s: &str) -> String {
    s.trim().to_owned()
}

async fn default_status_id(conn: &impl ConnectionTrait) -> Result<Option<i64>, CoreError> {
    for kind in [StatusKind::Diagnostics, StatusKind::Open] {
        let found = status::Entity::find()
            .filter(status::Column::Kind.eq(kind.as_str()))
            .filter(status::Column::Archived.eq(false))
            .order_by_asc(status::Column::SortOrder)
            .order_by_asc(status::Column::Id)
            .one(conn)
            .await?;
        if let Some(s) = found {
            return Ok(Some(s.id));
        }
    }
    Ok(None)
}

impl Core {
    /// Приём устройства: клиент, заказ, штрихкод и запись журнала в одной транзакции.
    /// Повтор с тем же `request_key` возвращает уже созданный заказ.
    pub async fn create_order(&self, actor: &Actor, new: NewOrder) -> Result<Order, CoreError> {
        self.create_order_marked(actor, new).await.map(|(o, _)| o)
    }

    /// То же, что `create_order`, плюс признак повтора: `true`, если вернулся заказ,
    /// созданный раньше с тем же `request_key`.
    pub async fn create_order_marked(
        &self,
        actor: &Actor,
        new: NewOrder,
    ) -> Result<(Order, bool), CoreError> {
        actor.require(Permission::OrdersCreate)?;
        for (field, key) in [
            ("request_key", &new.request_key),
            ("attachment_draft", &new.attachment_draft),
        ] {
            if key.as_deref().is_some_and(|k| !valid_key(k)) {
                return Err(CoreError::Validation(vec![FieldError::new(
                    field, "invalid",
                )]));
            }
        }
        let Some(key) = new.request_key.clone() else {
            return Ok((self.create_order_once(actor, new).await?, false));
        };
        if let Some(found) = self.order_by_request_key(&key).await? {
            return Ok((found, true));
        }
        match self.create_order_once(actor, new).await {
            Ok(created) => Ok((created, false)),
            // Параллельный приём с тем же ключом: победитель уже записан.
            Err(e @ CoreError::Db(_)) => match self.order_by_request_key(&key).await? {
                Some(found) => Ok((found, true)),
                None => Err(e),
            },
            Err(e) => Err(e),
        }
    }

    /// Заказ по ключу приёма. Удалённый заказ держит ключ: повтор не возвращает его
    /// и не создаёт новый, а просит начать приём заново.
    async fn order_by_request_key(&self, key: &str) -> Result<Option<Order>, CoreError> {
        let found = order::Entity::find()
            .filter(order::Column::RequestKey.eq(key))
            .one(&self.db)
            .await?;
        match found {
            Some(m) if m.deleted_at.is_some() => {
                Err(CoreError::Validation(vec![FieldError::with_message(
                    "request_key",
                    "invalid",
                    "Этот приём уже был удалён — начните новый",
                )]))
            }
            other => Ok(other.map(Order::from)),
        }
    }

    async fn create_order_once(&self, actor: &Actor, new: NewOrder) -> Result<Order, CoreError> {
        let required = self.settings().intake_required_fields;
        let device_type = trimmed(&new.device_type);
        let brand = trimmed(&new.brand);
        let model = trimmed(&new.model);
        let malfunction = trimmed(&new.malfunction);

        let tx = self.begin_write().await?;
        let mut errors: Vec<FieldError> = Vec::new();
        if required {
            for (field, value) in [
                ("device_type", &device_type),
                ("brand", &brand),
                ("model", &model),
                ("malfunction", &malfunction),
            ] {
                if value.is_empty() {
                    errors.push(FieldError::required(field));
                }
            }
        }
        for (field, value) in [
            ("estimate_from", new.estimate_from.map(|m| m.kopecks())),
            ("estimate_to", new.estimate_to.map(|m| m.kopecks())),
            ("prepayment", Some(new.prepayment.kopecks())),
        ] {
            if value.is_some_and(|v| v < 0) {
                errors.push(FieldError::new(field, "out_of_range"));
            }
        }
        if let (Some(from), Some(to)) = (new.estimate_from, new.estimate_to)
            && from > to
        {
            errors.push(FieldError::new("estimate_to", "estimate_range"));
        }
        match &new.client {
            ClientRef::Existing(id) => {
                if find_live(&tx, *id).await?.is_none() {
                    errors.push(FieldError::new("client_id", "invalid"));
                }
            }
            ClientRef::New(c) => {
                if let Err(e) = crate::clients::validate_client(&c.full_name, &c.phones) {
                    errors.extend(e);
                }
            }
        }
        if let Some(id) = new.service_point_id {
            let ok = service_point::Entity::find_by_id(id)
                .one(&tx)
                .await?
                .is_some_and(|p| !p.archived);
            if !ok {
                errors.push(FieldError::new("service_point_id", "invalid"));
            }
        }
        let status_id = match new.status_id {
            Some(id) => {
                let ok = status::Entity::find_by_id(id)
                    .one(&tx)
                    .await?
                    .is_some_and(|s| !s.archived && s.kind != StatusKind::Issued.as_str());
                if !ok {
                    errors.push(FieldError::new("status_id", "invalid"));
                }
                id
            }
            None => match default_status_id(&tx).await? {
                Some(id) => id,
                None => {
                    errors.push(FieldError::new("status_id", "invalid"));
                    0
                }
            },
        };
        if !errors.is_empty() {
            return Err(CoreError::Validation(errors));
        }

        let now = self.now();
        let client_id = match new.client {
            ClientRef::Existing(id) => id,
            ClientRef::New(c) => insert_client(&tx, now, c).await?.id,
        };
        let prepayment = new.prepayment.kopecks();
        let (warranty_text, warranty_days, warranty_months) = warranty_columns(&new.warranty);
        let inserted = order::ActiveModel {
            id: NotSet,
            client_id: Set(client_id),
            service_point_id: Set(new.service_point_id),
            status_id: Set(status_id),
            received_at: Set(now),
            issued_at: Set(None),
            prepaid_at: Set((prepayment > 0).then_some(now)),
            device_type: Set(device_type),
            brand: Set(brand),
            model: Set(model),
            serial: Set(trimmed(&new.serial)),
            color: Set(trimmed(&new.color)),
            device_password: Set(trimmed(&new.device_password)),
            completeness: Set(trimmed(&new.completeness)),
            device_condition: Set(trimmed(&new.device_condition)),
            malfunction: Set(malfunction),
            work_done: Set(String::new()),
            internal_comment: Set(trimmed(&new.internal_comment)),
            master: Set(trimmed(&new.master)),
            barcode: Set(String::new()),
            estimate_from: Set(new.estimate_from.map(Money::kopecks)),
            estimate_to: Set(new.estimate_to.map(Money::kopecks)),
            final_price: Set(None),
            prepayment: Set(prepayment),
            extra_costs: Set(0),
            discount: Set(0),
            warranty_days: Set(warranty_days),
            warranty_text: Set(warranty_text),
            warranty_months: Set(warranty_months),
            need_call: Set(new.need_call),
            need_parts: Set(new.need_parts),
            created_at: Set(now),
            updated_at: Set(now),
            deleted_at: Set(None),
            version: Set(1),
            search_text: Set(String::new()),
            status_changed_at: Set(None),
            device_type_key: Set(String::new()),
            brand_key: Set(String::new()),
            model_key: Set(String::new()),
            master_key: Set(String::new()),
            request_key: Set(new.request_key.clone()),
        }
        .insert(&tx)
        .await?;

        let barcode = order_barcode(inserted.id).ok_or_else(|| {
            CoreError::Validation(vec![FieldError::new("order_id", "out_of_range")])
        })?;
        let mut model = inserted;
        model.barcode = barcode;
        model.search_text = order_search_text(&model);
        set_order_keys(&mut model);
        order::ActiveModel {
            id: Set(model.id),
            barcode: Set(model.barcode.clone()),
            search_text: Set(model.search_text.clone()),
            device_type_key: Set(model.device_type_key.clone()),
            brand_key: Set(model.brand_key.clone()),
            model_key: Set(model.model_key.clone()),
            master_key: Set(model.master_key.clone()),
            ..Default::default()
        }
        .update(&tx)
        .await?;

        write_audit(
            &tx,
            now,
            actor,
            NewAuditEntry {
                action: "order.created",
                order_id: Some(model.id),
                client_id: Some(client_id),
                summary: format!("Заказ № {} принят", model.id),
                details: Some(json!({ "order_id": model.id, "client_id": client_id })),
            },
        )
        .await?;
        let moved = match &new.attachment_draft {
            Some(key) => {
                let rows = attachment::Entity::find()
                    .filter(attachment::Column::DraftKey.eq(key.as_str()))
                    .filter(attachment::Column::OrderId.is_null())
                    .all(&tx)
                    .await?;
                if !rows.is_empty() {
                    attachment::Entity::update_many()
                        .col_expr(attachment::Column::OrderId, Expr::value(model.id))
                        .col_expr(
                            attachment::Column::DraftKey,
                            Expr::value(Option::<String>::None),
                        )
                        .filter(attachment::Column::DraftKey.eq(key.as_str()))
                        .filter(attachment::Column::OrderId.is_null())
                        .exec(&tx)
                        .await?;
                }
                rows
            }
            None => Vec::new(),
        };
        tx.commit().await?;
        if !moved.is_empty() {
            self.move_draft_files(model.id, moved).await;
        }
        Ok(model.into())
    }

    /// Заказ по номеру, включая удалённый (с `deleted_at`).
    pub async fn get_order(&self, _actor: &Actor, id: i64) -> Result<Order, CoreError> {
        order::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .map(Order::from)
            .ok_or(CoreError::NotFound { entity: ENTITY, id })
    }
}
