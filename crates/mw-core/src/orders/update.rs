use chrono::{NaiveDateTime, SubsecRound};
use mw_db::entity::{order, status};
use mw_domain::permission::Permission;
use mw_domain::status::StatusKind;
use mw_domain::validation::FieldError;
use sea_orm::ExprTrait;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder,
};
use serde_json::{Map, Value, json};

use super::types::{OrderPatch, warranty_columns};
use crate::clients::find_live;
use crate::search_index::{order_search_text, set_order_keys};
use crate::{Actor, Core, CoreError, NewAuditEntry, Order, write_audit};

const ENTITY: &str = "заказ";
const MASK: &str = "***";

fn deleted_error() -> CoreError {
    CoreError::Validation(vec![FieldError::new("id", "deleted")])
}

async fn load(conn: &impl ConnectionTrait, id: i64) -> Result<order::Model, CoreError> {
    order::Entity::find_by_id(id)
        .one(conn)
        .await?
        .ok_or(CoreError::NotFound { entity: ENTITY, id })
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

/// Применяет значение патча к полю и записывает различие.
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
    /// Частичная правка заказа с оптимистической блокировкой и точным журналом различий.
    pub async fn update_order(
        &self,
        actor: &Actor,
        id: i64,
        expected_version: i64,
        patch: OrderPatch,
    ) -> Result<Order, CoreError> {
        actor.require(Permission::OrdersEdit)?;
        // Даты хранятся с точностью до секунды, как `Core::now()`.
        let patch = OrderPatch {
            received_at: patch.received_at.map(|t| t.trunc_subsecs(0)),
            issued_at: patch.issued_at.map(|t| t.map(|t| t.trunc_subsecs(0))),
            ..patch
        };
        if patch.received_at.is_some() || patch.issued_at.is_some() {
            actor.require(Permission::OrderDatesEdit)?;
        }
        let tx = self.begin_write().await?;
        let current = load(&tx, id).await?;
        if current.deleted_at.is_some() {
            return Err(deleted_error());
        }

        let mut m = current.clone();
        let mut d = Diff::default();
        apply!(d, m, client_id, "клиент", patch.client_id);
        apply!(d, m, service_point_id, "точка", patch.service_point_id);
        apply!(d, m, received_at, "дата приёма", patch.received_at);
        apply!(d, m, issued_at, "дата выдачи", patch.issued_at);
        apply!(
            d,
            m,
            device_type,
            "тип устройства",
            trimmed(patch.device_type)
        );
        apply!(d, m, brand, "бренд", trimmed(patch.brand));
        apply!(d, m, model, "модель", trimmed(patch.model));
        apply!(d, m, serial, "серийный номер", trimmed(patch.serial));
        apply!(d, m, color, "цвет", trimmed(patch.color));
        // Пароль устройства: сравниваем настоящие значения, в журнал пишем маску.
        if let Some(new) = trimmed(patch.device_password)
            && new != m.device_password
        {
            d.note(
                "device_password",
                "пароль устройства",
                json!(MASK),
                json!(MASK),
            );
            m.device_password = new;
        }
        apply!(
            d,
            m,
            completeness,
            "комплектность",
            trimmed(patch.completeness)
        );
        apply!(
            d,
            m,
            device_condition,
            "состояние",
            trimmed(patch.device_condition)
        );
        apply!(
            d,
            m,
            malfunction,
            "неисправность",
            trimmed(patch.malfunction)
        );
        apply!(
            d,
            m,
            work_done,
            "выполненные работы",
            trimmed(patch.work_done)
        );
        apply!(
            d,
            m,
            internal_comment,
            "комментарий",
            trimmed(patch.internal_comment)
        );
        apply!(d, m, master, "мастер", trimmed(patch.master));
        apply!(
            d,
            m,
            estimate_from,
            "ориентировочная стоимость",
            patch.estimate_from.map(|v| v.map(|x| x.kopecks()))
        );
        apply!(
            d,
            m,
            estimate_to,
            "ориентировочная стоимость",
            patch.estimate_to.map(|v| v.map(|x| x.kopecks()))
        );
        apply!(
            d,
            m,
            final_price,
            "итоговая стоимость",
            patch.final_price.map(|v| v.map(|x| x.kopecks()))
        );
        apply!(
            d,
            m,
            prepayment,
            "предоплата",
            patch.prepayment.map(|v| v.kopecks())
        );
        apply!(
            d,
            m,
            extra_costs,
            "затраты",
            patch.extra_costs.map(|v| v.kopecks())
        );
        apply!(
            d,
            m,
            discount,
            "скидка",
            patch.discount.map(|v| v.kopecks())
        );
        // Текст гарантии и срок, выведенный из него, меняются вместе; в журнале одно поле.
        if let Some(new) = trimmed(patch.warranty)
            && new != m.warranty_text
        {
            d.note("warranty", "гарантия", json!(m.warranty_text), json!(new));
            (m.warranty_text, m.warranty_days, m.warranty_months) = warranty_columns(&new);
        }
        apply!(d, m, need_call, "нужно позвонить", patch.need_call);
        apply!(d, m, need_parts, "требует заказа", patch.need_parts);

        if d.changes.is_empty() {
            return Ok(current.into());
        }
        if current.version != expected_version {
            return Err(CoreError::Conflict {
                entity: ENTITY,
                id,
                current_version: current.version,
            });
        }

        let mut errors: Vec<FieldError> = Vec::new();
        if m.client_id != current.client_id && find_live(&tx, m.client_id).await?.is_none() {
            errors.push(FieldError::new("client_id", "invalid"));
        }
        if m.service_point_id != current.service_point_id
            && let Some(sp) = m.service_point_id
        {
            let ok = mw_db::entity::service_point::Entity::find_by_id(sp)
                .one(&tx)
                .await?
                .is_some_and(|p| !p.archived);
            if !ok {
                errors.push(FieldError::new("service_point_id", "invalid"));
            }
        }
        // Значения вне диапазона проверяем только у полей, которые правка действительно меняет:
        // уже сохранённые старые значения не мешают посторонним правкам.
        for (field, value) in [
            ("estimate_from", m.estimate_from),
            ("estimate_to", m.estimate_to),
            ("prepayment", Some(m.prepayment)),
            ("extra_costs", Some(m.extra_costs)),
            ("discount", Some(m.discount)),
            ("final_price", m.final_price),
        ] {
            if d.changes.contains_key(field) && value.is_some_and(|v| v < 0) {
                errors.push(FieldError::new(field, "out_of_range"));
            }
        }
        if let (Some(from), Some(to)) = (m.estimate_from, m.estimate_to)
            && from > to
        {
            errors.push(FieldError::new("estimate_to", "estimate_range"));
        }
        if !errors.is_empty() {
            return Err(CoreError::Validation(errors));
        }

        let now = self.now();
        if m.prepayment != current.prepayment {
            if m.prepayment > 0 && m.prepaid_at.is_none() {
                m.prepaid_at = Some(now);
            } else if m.prepayment == 0 {
                m.prepaid_at = None;
            }
        }
        m.search_text = order_search_text(&m);
        set_order_keys(&mut m);

        let res = order::Entity::update_many()
            .col_expr(order::Column::ClientId, Expr::value(m.client_id))
            .col_expr(
                order::Column::ServicePointId,
                Expr::value(m.service_point_id),
            )
            .col_expr(order::Column::ReceivedAt, Expr::value(m.received_at))
            .col_expr(order::Column::IssuedAt, Expr::value(m.issued_at))
            .col_expr(order::Column::PrepaidAt, Expr::value(m.prepaid_at))
            .col_expr(
                order::Column::DeviceType,
                Expr::value(m.device_type.clone()),
            )
            .col_expr(order::Column::Brand, Expr::value(m.brand.clone()))
            .col_expr(order::Column::Model, Expr::value(m.model.clone()))
            .col_expr(order::Column::Serial, Expr::value(m.serial.clone()))
            .col_expr(order::Column::Color, Expr::value(m.color.clone()))
            .col_expr(
                order::Column::DevicePassword,
                Expr::value(m.device_password.clone()),
            )
            .col_expr(
                order::Column::Completeness,
                Expr::value(m.completeness.clone()),
            )
            .col_expr(
                order::Column::DeviceCondition,
                Expr::value(m.device_condition.clone()),
            )
            .col_expr(
                order::Column::Malfunction,
                Expr::value(m.malfunction.clone()),
            )
            .col_expr(order::Column::WorkDone, Expr::value(m.work_done.clone()))
            .col_expr(
                order::Column::InternalComment,
                Expr::value(m.internal_comment.clone()),
            )
            .col_expr(order::Column::Master, Expr::value(m.master.clone()))
            .col_expr(order::Column::EstimateFrom, Expr::value(m.estimate_from))
            .col_expr(order::Column::EstimateTo, Expr::value(m.estimate_to))
            .col_expr(order::Column::FinalPrice, Expr::value(m.final_price))
            .col_expr(order::Column::Prepayment, Expr::value(m.prepayment))
            .col_expr(order::Column::ExtraCosts, Expr::value(m.extra_costs))
            .col_expr(order::Column::Discount, Expr::value(m.discount))
            .col_expr(
                order::Column::WarrantyText,
                Expr::value(m.warranty_text.clone()),
            )
            .col_expr(order::Column::WarrantyDays, Expr::value(m.warranty_days))
            .col_expr(
                order::Column::WarrantyMonths,
                Expr::value(m.warranty_months),
            )
            .col_expr(order::Column::NeedCall, Expr::value(m.need_call))
            .col_expr(order::Column::NeedParts, Expr::value(m.need_parts))
            .col_expr(
                order::Column::SearchText,
                Expr::value(m.search_text.clone()),
            )
            .col_expr(
                order::Column::DeviceTypeKey,
                Expr::value(m.device_type_key.clone()),
            )
            .col_expr(order::Column::BrandKey, Expr::value(m.brand_key.clone()))
            .col_expr(order::Column::ModelKey, Expr::value(m.model_key.clone()))
            .col_expr(order::Column::MasterKey, Expr::value(m.master_key.clone()))
            .col_expr(order::Column::UpdatedAt, Expr::value(now))
            .col_expr(
                order::Column::Version,
                Expr::col(order::Column::Version).add(1),
            )
            .filter(order::Column::Id.eq(id))
            .filter(order::Column::Version.eq(expected_version))
            .filter(order::Column::DeletedAt.is_null())
            .exec(&tx)
            .await?;
        check_affected(&tx, res.rows_affected, id).await?;

        write_audit(
            &tx,
            now,
            actor,
            NewAuditEntry {
                action: "order.updated",
                order_id: Some(id),
                client_id: Some(m.client_id),
                summary: format!("Изменено: {}", d.labels.join(", ")),
                details: Some(json!({ "changes": d.changes })),
            },
        )
        .await?;
        let updated = load(&tx, id).await?;
        tx.commit().await?;
        Ok(updated.into())
    }

    /// Смена статуса заказа.
    pub async fn change_status(
        &self,
        actor: &Actor,
        id: i64,
        expected_version: i64,
        status_id: i64,
    ) -> Result<Order, CoreError> {
        actor.require(Permission::OrdersEdit)?;
        let tx = self.begin_write().await?;
        let updated = self
            .change_status_in(&tx, actor, id, expected_version, Some(status_id))
            .await?;
        tx.commit().await?;
        Ok(updated)
    }

    /// Выдача клиенту: переход в первый неархивный статус вида «выдан».
    pub async fn issue_order(
        &self,
        actor: &Actor,
        id: i64,
        expected_version: i64,
    ) -> Result<Order, CoreError> {
        actor.require(Permission::OrdersEdit)?;
        let tx = self.begin_write().await?;
        let updated = self
            .change_status_in(&tx, actor, id, expected_version, None)
            .await?;
        tx.commit().await?;
        Ok(updated)
    }

    /// Мягкое удаление заказа.
    pub async fn delete_order(
        &self,
        actor: &Actor,
        id: i64,
        expected_version: i64,
    ) -> Result<(), CoreError> {
        actor.require(Permission::OrdersDelete)?;
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
        let res = order::Entity::update_many()
            .col_expr(order::Column::DeletedAt, Expr::value(Some(now)))
            .col_expr(order::Column::UpdatedAt, Expr::value(now))
            .col_expr(
                order::Column::Version,
                Expr::col(order::Column::Version).add(1),
            )
            .filter(order::Column::Id.eq(id))
            .filter(order::Column::Version.eq(expected_version))
            .filter(order::Column::DeletedAt.is_null())
            .exec(&tx)
            .await?;
        check_affected(&tx, res.rows_affected, id).await?;
        write_audit(
            &tx,
            now,
            actor,
            NewAuditEntry {
                action: "order.deleted",
                order_id: Some(id),
                client_id: Some(current.client_id),
                summary: format!("Заказ № {id} удалён"),
                details: None,
            },
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// `target = None` — первый неархивный статус вида «выдан».
    async fn change_status_in(
        &self,
        tx: &DatabaseTransaction,
        actor: &Actor,
        id: i64,
        expected_version: i64,
        target: Option<i64>,
    ) -> Result<Order, CoreError> {
        let current = load(tx, id).await?;
        if current.deleted_at.is_some() {
            return Err(deleted_error());
        }
        let invalid = || CoreError::Validation(vec![FieldError::new("status_id", "invalid")]);
        let new_status = match target {
            Some(sid) => status::Entity::find_by_id(sid).one(tx).await?,
            None => {
                status::Entity::find()
                    .filter(status::Column::Kind.eq(StatusKind::Issued.as_str()))
                    .filter(status::Column::Archived.eq(false))
                    .order_by_asc(status::Column::SortOrder)
                    .order_by_asc(status::Column::Id)
                    .one(tx)
                    .await?
            }
        };
        let new_status = new_status.filter(|s| !s.archived).ok_or_else(invalid)?;
        if new_status.id == current.status_id {
            return Ok(current.into());
        }
        if current.version != expected_version {
            return Err(CoreError::Conflict {
                entity: ENTITY,
                id,
                current_version: current.version,
            });
        }
        let to_issued = new_status.kind == StatusKind::Issued.as_str();
        if to_issued {
            let mut errors = Vec::new();
            if current.master.trim().is_empty() {
                errors.push(FieldError::required("master"));
            }
            if current.work_done.trim().is_empty() {
                errors.push(FieldError::required("work_done"));
            }
            if !errors.is_empty() {
                return Err(CoreError::Validation(errors));
            }
        }
        let old_name = status::Entity::find_by_id(current.status_id)
            .one(tx)
            .await?
            .map_or_else(|| current.status_id.to_string(), |s| s.name);

        let now = self.now();
        let issued_at: Option<NaiveDateTime> = if to_issued {
            current.issued_at.or(Some(now))
        } else {
            current.issued_at
        };
        let res = order::Entity::update_many()
            .col_expr(order::Column::StatusId, Expr::value(new_status.id))
            .col_expr(order::Column::IssuedAt, Expr::value(issued_at))
            .col_expr(order::Column::StatusChangedAt, Expr::value(Some(now)))
            .col_expr(order::Column::UpdatedAt, Expr::value(now))
            .col_expr(
                order::Column::Version,
                Expr::col(order::Column::Version).add(1),
            )
            .filter(order::Column::Id.eq(id))
            .filter(order::Column::Version.eq(expected_version))
            .filter(order::Column::DeletedAt.is_null())
            .exec(tx)
            .await?;
        check_affected(tx, res.rows_affected, id).await?;
        write_audit(
            tx,
            now,
            actor,
            NewAuditEntry {
                action: "order.status_changed",
                order_id: Some(id),
                client_id: Some(current.client_id),
                summary: format!("Статус: «{old_name}» → «{}»", new_status.name),
                details: Some(json!({ "from": current.status_id, "to": new_status.id })),
            },
        )
        .await?;
        Ok(load(tx, id).await?.into())
    }
}

/// Ноль затронутых строк: конфликт версий, если заказ есть, иначе «не найден».
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
