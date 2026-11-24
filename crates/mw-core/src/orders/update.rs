use chrono::{NaiveDateTime, SubsecRound};
use mw_db::entity::{order, service_point, status};
use mw_domain::issue::IssueRequirement;
use mw_domain::permission::Permission;
use mw_domain::status::StatusKind;
use mw_domain::validation::FieldError;
use sea_orm::ExprTrait;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder,
};
use serde_json::{Map, Value, json};

use super::master::{available_master, master_unavailable};
use super::types::{NewWarrantyCase, OrderPatch, warranty_columns};
use crate::clients::find_live;
use crate::search_index::{order_search_text, set_order_keys};
use crate::{Actor, Core, CoreError, EditVia, NewAuditEntry, Order, write_audit};
use mw_domain::search::normalize_search;

const ENTITY: &str = "заказ";
const MASK: &str = "***";

pub(super) fn deleted_error() -> CoreError {
    CoreError::Validation(vec![FieldError::new("id", "deleted")])
}

/// Даты приёма и выдачи правит только тот, у кого есть `order_dates_edit`.
fn require_dates_permission(actor: &Actor, patch: &OrderPatch) -> Result<(), CoreError> {
    if patch.received_at.is_some() || patch.issued_at.is_some() {
        actor.require(Permission::OrderDatesEdit)?;
    }
    Ok(())
}

pub(super) async fn load(conn: &impl ConnectionTrait, id: i64) -> Result<order::Model, CoreError> {
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

/// Точка в записи о перемещении: имя в кавычках (архивная — тоже), без точки — «без точки».
async fn point_label(tx: &DatabaseTransaction, id: Option<i64>) -> Result<String, CoreError> {
    let Some(id) = id else {
        return Ok("без точки".to_owned());
    };
    Ok(service_point::Entity::find_by_id(id)
        .one(tx)
        .await?
        .map_or_else(|| format!("№ {id}"), |p| format!("«{}»", p.name)))
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
        // Отказ без права на даты — до блокировки записи.
        require_dates_permission(actor, &patch)?;
        let tx = self.begin_write().await?;
        let (updated, _) = self
            .update_order_in(&tx, actor, id, expected_version, patch, None)
            .await?;
        tx.commit().await?;
        Ok(updated)
    }

    /// Правка заказа внутри открытой транзакции записи: всё, что делает `update_order`, кроме
    /// проверки права `orders_edit` и фиксации (замена значения справочника в заказах идёт через
    /// неё же, с пометкой `via`). Возвращает заказ и признак, что запись изменилась.
    pub(crate) async fn update_order_in(
        &self,
        tx: &DatabaseTransaction,
        actor: &Actor,
        id: i64,
        expected_version: i64,
        patch: OrderPatch,
        via: Option<EditVia>,
    ) -> Result<(Order, bool), CoreError> {
        // Даты хранятся с точностью до секунды, как `Core::now()`.
        let patch = OrderPatch {
            received_at: patch.received_at.map(|t| t.trunc_subsecs(0)),
            issued_at: patch.issued_at.map(|t| t.map(|t| t.trunc_subsecs(0))),
            ..patch
        };
        require_dates_permission(actor, &patch)?;
        let current = load(tx, id).await?;
        if current.deleted_at.is_some() {
            return Err(deleted_error());
        }

        let mut m = current.clone();
        let mut d = Diff::default();
        apply!(d, m, client_id, "клиент", patch.client_id);
        // Точка: перемещение пишется своей записью `order.moved`, среди меток «Изменено» его нет.
        let moved = match patch.service_point_id {
            Some(new) if new != m.service_point_id => {
                Some((std::mem::replace(&mut m.service_point_id, new), new))
            }
            _ => None,
        };
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
        // Мастер: номер пользователя и его имя (снимок) меняются вместе. Тот же номер — правки нет,
        // даже если пользователя с тех пор переименовали или отключили. Имя без номера (не
        // перенесённое старое) снимается и `null`.
        let mut master_invalid = false;
        if let Some(new_id) = patch.master_user_id
            && (new_id != m.master_user_id || (new_id.is_none() && !m.master.is_empty()))
        {
            let name = match new_id {
                Some(id) => match available_master(tx, id).await? {
                    Some(name) => name,
                    None => {
                        master_invalid = true;
                        String::new()
                    }
                },
                None => String::new(),
            };
            d.note("master", "мастер", json!(m.master), json!(name));
            if new_id != m.master_user_id {
                d.note(
                    "master_user_id",
                    "мастер",
                    json!(m.master_user_id),
                    json!(new_id),
                );
            }
            m.master = name;
            m.master_user_id = new_id;
        }
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

        if d.changes.is_empty() && moved.is_none() {
            return Ok((current.into(), false));
        }
        if current.version != expected_version {
            return Err(CoreError::Conflict {
                entity: ENTITY,
                id,
                current_version: current.version,
            });
        }

        let mut errors: Vec<FieldError> = Vec::new();
        if master_invalid {
            errors.push(master_unavailable("master_user_id"));
        }
        if m.client_id != current.client_id && find_live(tx, m.client_id).await?.is_none() {
            errors.push(FieldError::new("client_id", "invalid"));
        }
        if let Some((_, Some(sp))) = moved {
            let ok = service_point::Entity::find_by_id(sp)
                .one(tx)
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
            .col_expr(order::Column::MasterUserId, Expr::value(m.master_user_id))
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
            .exec(tx)
            .await?;
        check_affected(tx, res.rows_affected, id).await?;

        if !d.changes.is_empty() {
            let mut summary = format!("Изменено: {}", d.labels.join(", "));
            let details = json!({ "changes": d.changes });
            // Пометка «из справочника» — только в тексте записи, не среди полей правки.
            if let Some(via) = via {
                summary.push_str(via.suffix_ru());
            }
            write_audit(
                tx,
                now,
                actor,
                NewAuditEntry {
                    action: "order.updated",
                    order_id: Some(id),
                    client_id: Some(m.client_id),
                    stock_item_id: None,
                    summary,
                    details: Some(details),
                },
            )
            .await?;
        }
        if let Some((from, to)) = moved {
            let summary = format!(
                "Перемещён: {} → {}",
                point_label(tx, from).await?,
                point_label(tx, to).await?
            );
            write_audit(
                tx,
                now,
                actor,
                NewAuditEntry {
                    action: "order.moved",
                    order_id: Some(id),
                    client_id: Some(m.client_id),
                    stock_item_id: None,
                    summary,
                    details: Some(json!({ "from": from, "to": to })),
                },
            )
            .await?;
        }
        Ok((load(tx, id).await?.into(), true))
    }

    /// Смена статуса заказа.
    pub async fn change_status(
        &self,
        actor: &Actor,
        id: i64,
        expected_version: i64,
        status_id: i64,
    ) -> Result<Order, CoreError> {
        self.change_status_with(actor, id, expected_version, status_id, true)
            .await
    }

    /// Смена статуса; `stamp_issued_at = false` не ставит дату выдачи при переходе в вид «выдан»
    /// (массовое действие «не ставить дату»). Уже стоящая дата не меняется.
    pub async fn change_status_with(
        &self,
        actor: &Actor,
        id: i64,
        expected_version: i64,
        status_id: i64,
        stamp_issued_at: bool,
    ) -> Result<Order, CoreError> {
        actor.require(Permission::OrdersEdit)?;
        let tx = self.begin_write().await?;
        let updated = self
            .change_status_in(
                &tx,
                actor,
                id,
                expected_version,
                Some(status_id),
                stamp_issued_at,
            )
            .await?;
        tx.commit().await?;
        Ok(updated)
    }

    /// Приём выданного заказа по гарантии: статус «Принят по гарантии» (дата выдачи остаётся)
    /// и запись журнала `order.warranty_case` — в одной транзакции.
    pub async fn accept_warranty_case(
        &self,
        actor: &Actor,
        id: i64,
        expected_version: i64,
        case: NewWarrantyCase,
    ) -> Result<Order, CoreError> {
        actor.require(Permission::OrdersEdit)?;
        let complaint = case.complaint.trim().to_owned();
        if complaint.is_empty() {
            return Err(CoreError::Validation(vec![FieldError::required(
                "complaint",
            )]));
        }
        let tx = self.begin_write().await?;
        let current = load(&tx, id).await?;
        if current.deleted_at.is_some() {
            return Err(deleted_error());
        }
        let current_kind = status::Entity::find_by_id(current.status_id)
            .one(&tx)
            .await?;
        if current_kind.is_none_or(|s| s.kind != StatusKind::Issued.as_str()) {
            return Err(CoreError::Validation(vec![FieldError::with_message(
                "status",
                "invalid",
                "Принять по гарантии можно только выданный заказ",
            )]));
        }
        let now = self.now();
        let at = case.at.unwrap_or(now).trunc_subsecs(0);
        if at > now {
            return Err(CoreError::Validation(vec![FieldError::with_message(
                "at",
                "invalid",
                "Дата обращения ещё не наступила: укажите сегодняшний или прошедший день",
            )]));
        }
        // Обращение выбирают днём: годится любой момент дня выдачи.
        if current
            .issued_at
            .is_some_and(|issued| at.date() < issued.date())
        {
            return Err(CoreError::Validation(vec![FieldError::with_message(
                "at",
                "invalid",
                "Дата обращения раньше выдачи заказа: укажите день выдачи или позже",
            )]));
        }
        let key = normalize_search("Принят по гарантии");
        let warranty = status::Entity::find()
            .filter(status::Column::Archived.eq(false))
            .order_by_asc(status::Column::SortOrder)
            .order_by_asc(status::Column::Id)
            .all(&tx)
            .await?
            .into_iter()
            .find(|s| normalize_search(&s.name) == key)
            .ok_or_else(|| {
                CoreError::Validation(vec![FieldError::with_message(
                    "status",
                    "invalid",
                    "Нет статуса «Принят по гарантии». Добавьте его в настройках",
                )])
            })?;
        let updated = self
            .change_status_in(&tx, actor, id, expected_version, Some(warranty.id), true)
            .await?;
        write_audit(
            &tx,
            now,
            actor,
            NewAuditEntry {
                action: "order.warranty_case",
                order_id: Some(id),
                client_id: Some(current.client_id),
                stock_item_id: None,
                summary: format!("Принят по гарантии: {complaint}"),
                details: Some(json!({ "complaint": complaint, "at": at })),
            },
        )
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
            .change_status_in(&tx, actor, id, expected_version, None, true)
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
                stock_item_id: None,
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
        stamp_issued_at: bool,
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
        let old_status = status::Entity::find_by_id(current.status_id)
            .one(tx)
            .await?;
        if to_issued {
            // «Обязательно при выдаче» задаёт весь набор: проверяется при каждом переходе в вид «Выдан».
            let required = crate::settings::load(tx).await?.issue_required;
            let empty = |req: IssueRequirement| match req {
                IssueRequirement::FinalPrice => current.final_price.is_none(),
                // Мастер — пользователь: текст без него («-» и имена из старой базы) не мастер.
                IssueRequirement::Master => current.master_user_id.is_none(),
                IssueRequirement::WorkDone => current.work_done.trim().is_empty(),
                IssueRequirement::Warranty => current.warranty_text.trim().is_empty(),
            };
            let errors: Vec<FieldError> = [
                IssueRequirement::FinalPrice,
                IssueRequirement::Master,
                IssueRequirement::WorkDone,
                IssueRequirement::Warranty,
            ]
            .into_iter()
            .filter(|r| required.contains(r) && empty(*r))
            .map(|r| {
                FieldError::with_message(
                    r.as_str(),
                    "required_for_issue",
                    "Заполните перед выдачей",
                )
            })
            .collect();
            if !errors.is_empty() {
                return Err(CoreError::Validation(errors));
            }
        }
        let old_name = old_status.map_or_else(|| current.status_id.to_string(), |s| s.name);

        let now = self.now();
        let issued_at: Option<NaiveDateTime> = if to_issued && stamp_issued_at {
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
                stock_item_id: None,
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use std::sync::Arc;
    use std::time::Duration;

    use mw_domain::permission::{Permission, PermissionSet};

    use super::*;
    use crate::{Actor, FixedClock};

    /// Отказ без права на даты приходит сразу: блокировка записи для него не берётся.
    #[tokio::test]
    async fn dates_permission_is_checked_before_the_write_lock() {
        for (label, db) in mw_db::testing::all_backends().await {
            let clock = Arc::new(FixedClock::new(chrono::Utc::now()));
            let core = Core::open(db, clock, "UTC").await.unwrap();
            let mut set = PermissionSet::empty();
            set.insert(Permission::OrdersEdit);
            let editor = Actor::for_tests(None, "Тест", set);
            let held = core.begin_write().await.unwrap();
            let patch = OrderPatch {
                received_at: Some(core.now()),
                ..Default::default()
            };
            let result = tokio::time::timeout(
                Duration::from_secs(2),
                core.update_order(&editor, 1, 1, patch),
            )
            .await;
            held.rollback().await.ok();
            assert!(
                matches!(
                    result,
                    Ok(Err(CoreError::Forbidden(Permission::OrderDatesEdit)))
                ),
                "{label}: отказ ждал блокировку записи: {result:?}"
            );
        }
    }
}
