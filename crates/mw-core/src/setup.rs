//! Первый запуск: признак «ещё не настроено» и мастер начальной настройки.

use mw_db::entity::{client, legacy_import, order, service_point, setting};
use mw_domain::permission::Permission;
use mw_domain::search::normalize_search;
use mw_domain::validation::FieldError;
use sea_orm::{ActiveModelTrait, ConnectionTrait, EntityTrait, NotSet, PaginatorTrait, Set};
use serde::Deserialize;
use serde_json::json;

use crate::settings::{self, put};
use crate::{Actor, Core, CoreError, NewAuditEntry, write_audit};

/// Ключ отметки «настроено» в таблице `settings`; не поле `AppSettings`.
pub(crate) const SETUP_DONE_KEY: &str = "setup_done";

/// Начальная настройка при первом запуске.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, ts_rs::TS, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SetupRequest {
    /// Название фирмы (печатные формы).
    pub firm_name: String,
    /// Первая точка обслуживания; она же точка по умолчанию.
    pub point_name: String,
    /// Мастера: каждое имя — пользователь без входа с отметкой «Мастер» и ролью «Мастер» (есть
    /// пользователь с таким именем или логином — он получает отметку). Повторы (без учёта регистра)
    /// и пустые строки отбрасываются.
    pub masters: Vec<String>,
}

async fn setup_marked(conn: &impl ConnectionTrait) -> Result<bool, CoreError> {
    Ok(setting::Entity::find_by_id(SETUP_DONE_KEY)
        .one(conn)
        .await?
        .is_some())
}

async fn setup_required_on(conn: &impl ConnectionTrait) -> Result<bool, CoreError> {
    if setup_marked(conn).await? {
        return Ok(false);
    }
    let used = order::Entity::find().count(conn).await? > 0
        || client::Entity::find().count(conn).await? > 0
        || legacy_import::Entity::find().count(conn).await? > 0;
    Ok(!used)
}

impl Core {
    /// Нужна ли начальная настройка: нет отметки «настроено» и в базе нет заказов, клиентов и импортов.
    pub async fn setup_required(&self) -> Result<bool, CoreError> {
        setup_required_on(&self.db).await
    }

    /// Только для запуска тестов (`--assume-setup`): ставит отметку «настроено», если её нет,
    /// с записью в журнале. Остальное в базе не меняет.
    pub async fn assume_setup_done(&self) -> Result<(), CoreError> {
        let tx = self.begin_write().await?;
        if setup_marked(&tx).await? {
            return Ok(());
        }
        put(&tx, SETUP_DONE_KEY, &json!(true)).await?;
        write_audit(
            &tx,
            self.now(),
            &Actor::unrestricted("Запуск для тестов"),
            NewAuditEntry {
                action: "setup.assumed",
                order_id: None,
                client_id: None,
                stock_item_id: None,
                summary: "Отмечено: программа настроена (запуск для тестов)".to_owned(),
                details: None,
            },
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Начальная настройка одной транзакцией; повтор или непустая база — `AlreadySetUp`.
    pub async fn run_setup(&self, actor: &Actor, request: SetupRequest) -> Result<(), CoreError> {
        actor.require(Permission::SettingsManage)?;
        let firm_name = request.firm_name.trim().to_owned();
        let point_name = request.point_name.trim().to_owned();
        let mut errors = Vec::new();
        if firm_name.is_empty() {
            errors.push(FieldError::required("firm_name"));
        }
        if point_name.is_empty() {
            errors.push(FieldError::required("point_name"));
        }
        if !errors.is_empty() {
            return Err(CoreError::Validation(errors));
        }

        let tx = self.begin_write().await?;
        // Отметка есть или в базе уже работали (старая база без отметки): настройка не нужна.
        if !setup_required_on(&tx).await? {
            return Err(CoreError::AlreadySetUp);
        }
        let points = service_point::Entity::find().all(&tx).await?;
        let point_key = normalize_search(&point_name);
        let point = match points
            .iter()
            .find(|p| !p.archived && normalize_search(&p.name) == point_key)
        {
            Some(existing) => existing.clone(),
            None => {
                let sort_order = points
                    .iter()
                    .map(|p| p.sort_order)
                    .max()
                    .map_or(0, |m| m + 1);
                service_point::ActiveModel {
                    id: NotSet,
                    name: Set(point_name.clone()),
                    sort_order: Set(sort_order),
                    archived: Set(false),
                }
                .insert(&tx)
                .await?
            }
        };

        let masters = mw_db::masters::masters_to_users(&tx, &request.masters, self.now()).await?;
        let added: Vec<&str> = masters
            .created
            .iter()
            .chain(&masters.marked)
            .map(|(_, name)| name.as_str())
            .collect();

        put(&tx, "firm_name", &json!(firm_name)).await?;
        put(&tx, "default_service_point_id", &json!(point.id)).await?;
        put(&tx, SETUP_DONE_KEY, &json!(true)).await?;
        write_audit(
            &tx,
            self.now(),
            actor,
            NewAuditEntry {
                action: "setup.completed",
                order_id: None,
                client_id: None,
                stock_item_id: None,
                summary: "Выполнена начальная настройка".to_owned(),
                details: Some(json!({
                    "firm_name": firm_name,
                    "service_point_id": point.id,
                    "masters": added,
                })),
            },
        )
        .await?;
        let loaded = settings::load(&tx).await?;
        self.commit_with_settings(tx, Some(loaded)).await
    }
}
