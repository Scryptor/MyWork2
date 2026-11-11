use mw_db::entity::{order, service_point, status};
use mw_domain::barcode::order_barcode;
use mw_domain::money::Money;
use mw_domain::permission::Permission;
use mw_domain::status::StatusKind;
use mw_domain::validation::FieldError;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, NotSet, QueryFilter, QueryOrder,
    Set,
};
use serde_json::json;

use super::types::order_search_text;
use crate::clients::{find_live, insert_client};
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
    pub async fn create_order(&self, actor: &Actor, new: NewOrder) -> Result<Order, CoreError> {
        actor.require(Permission::OrdersCreate)?;
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
                if c.full_name.trim().is_empty()
                    && mw_domain::phone::normalize_phone(c.phone.trim()).is_empty()
                {
                    errors.push(FieldError::required("full_name"));
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
            warranty_days: Set(new.warranty_days.map(i32::from)),
            need_call: Set(new.need_call),
            need_parts: Set(new.need_parts),
            created_at: Set(now),
            updated_at: Set(now),
            deleted_at: Set(None),
            version: Set(1),
            search_text: Set(String::new()),
        }
        .insert(&tx)
        .await?;

        let barcode = order_barcode(inserted.id).ok_or_else(|| {
            CoreError::Validation(vec![FieldError::new("order_id", "out_of_range")])
        })?;
        let mut model = inserted;
        model.barcode = barcode;
        model.search_text = order_search_text(&model);
        order::ActiveModel {
            id: Set(model.id),
            barcode: Set(model.barcode.clone()),
            search_text: Set(model.search_text.clone()),
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
        tx.commit().await?;
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
