use chrono_tz::Tz;
use mw_db::entity::setting;
use mw_domain::permission::Permission;
use mw_domain::validation::FieldError;
use sea_orm::sea_query::OnConflict;
use sea_orm::{ConnectionTrait, EntityTrait, Set};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::core::begin_write_on;
use crate::{Actor, Core, CoreError, NewAuditEntry, write_audit};

/// Настройки предприятия. Хранятся по строке на поле в таблице `settings` (ключ — имя поля,
/// значение — JSON).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct AppSettings {
    /// Знак валюты.
    pub currency_symbol: String,
    /// Процент мастера, 0–100.
    pub master_percent: u8,
    /// Через сколько суток диагностика считается просроченной.
    pub diagnostics_overdue_days: u16,
    /// Гарантия по умолчанию, в сутках; пусто — без гарантии.
    #[schema(required = true)]
    pub default_warranty_days: Option<u16>,
    /// Требовать заполнения обязательных полей при приёме.
    pub intake_required_fields: bool,
    /// Часовой пояс предприятия (имя IANA).
    pub timezone: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            currency_symbol: "₽".to_owned(),
            master_percent: 50,
            diagnostics_overdue_days: 4,
            default_warranty_days: Some(30),
            intake_required_fields: true,
            timezone: "UTC".to_owned(),
        }
    }
}

/// Частичное изменение настроек; не переданное поле не меняется. Гарантия: `null` — «без гарантии».
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, ts_rs::TS, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
#[ts(optional_fields)]
pub struct SettingsPatch {
    /// Знак валюты.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub currency_symbol: Option<String>,
    /// Процент мастера, 0–100.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub master_percent: Option<u8>,
    /// Через сколько суток диагностика считается просроченной.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub diagnostics_overdue_days: Option<u16>,
    /// Гарантия по умолчанию, в сутках; пусто — без гарантии.
    #[serde(default, deserialize_with = "crate::patch::nullable")]
    pub default_warranty_days: Option<Option<u16>>,
    /// Требовать заполнения обязательных полей при приёме.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub intake_required_fields: Option<bool>,
    /// Часовой пояс предприятия (имя IANA).
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub timezone: Option<String>,
}

pub(crate) fn parse_timezone(name: &str) -> Tz {
    name.parse().unwrap_or(Tz::UTC)
}

fn get<T: serde::de::DeserializeOwned>(rows: &[setting::Model], key: &str) -> Option<T> {
    let row = rows.iter().find(|r| r.key == key)?;
    serde_json::from_str(&row.value).ok()
}

/// Читает все настройки; отсутствующие или нечитаемые поля заменяются умолчаниями.
async fn load(conn: &impl ConnectionTrait) -> Result<AppSettings, CoreError> {
    let rows = setting::Entity::find().all(conn).await?;
    let d = AppSettings::default();
    Ok(AppSettings {
        currency_symbol: get(&rows, "currency_symbol").unwrap_or(d.currency_symbol),
        master_percent: get(&rows, "master_percent").unwrap_or(d.master_percent),
        diagnostics_overdue_days: get(&rows, "diagnostics_overdue_days")
            .unwrap_or(d.diagnostics_overdue_days),
        default_warranty_days: get(&rows, "default_warranty_days")
            .unwrap_or(d.default_warranty_days),
        intake_required_fields: get(&rows, "intake_required_fields")
            .unwrap_or(d.intake_required_fields),
        timezone: get(&rows, "timezone").unwrap_or(d.timezone),
    })
}

async fn put(conn: &impl ConnectionTrait, key: &str, value: &Value) -> Result<(), CoreError> {
    let row = setting::ActiveModel {
        key: Set(key.to_owned()),
        value: Set(value.to_string()),
    };
    setting::Entity::insert(row)
        .on_conflict(
            OnConflict::column(setting::Column::Key)
                .update_column(setting::Column::Value)
                .to_owned(),
        )
        .exec_without_returning(conn)
        .await?;
    Ok(())
}

/// Загружает настройки; если пояса ещё нет в БД, сохраняет системный (неверный → UTC).
pub(crate) async fn ensure_timezone(
    db: &sea_orm::DatabaseConnection,
    system_timezone: &str,
) -> Result<AppSettings, CoreError> {
    let tx = begin_write_on(db).await?;
    if setting::Entity::find_by_id("timezone")
        .one(&tx)
        .await?
        .is_none()
    {
        let name = if system_timezone.parse::<Tz>().is_ok() {
            system_timezone
        } else {
            "UTC"
        };
        put(&tx, "timezone", &json!(name)).await?;
    }
    let loaded = load(&tx).await?;
    tx.commit().await?;
    Ok(loaded)
}

fn validate(p: &SettingsPatch) -> Result<(), CoreError> {
    let mut errors = Vec::new();
    if let Some(c) = &p.currency_symbol {
        let n = c.chars().count();
        if n == 0 {
            errors.push(FieldError::required("currency_symbol"));
        } else if n > 8 {
            errors.push(FieldError::new("currency_symbol", "out_of_range"));
        }
    }
    if p.master_percent.is_some_and(|v| v > 100) {
        errors.push(FieldError::new("master_percent", "out_of_range"));
    }
    if p.diagnostics_overdue_days
        .is_some_and(|v| !(1..=365).contains(&v))
    {
        errors.push(FieldError::new("diagnostics_overdue_days", "out_of_range"));
    }
    if matches!(p.default_warranty_days, Some(Some(v)) if v > 3650) {
        errors.push(FieldError::new("default_warranty_days", "out_of_range"));
    }
    if p.timezone
        .as_deref()
        .is_some_and(|t| t.parse::<Tz>().is_err())
    {
        errors.push(FieldError::new("timezone", "invalid"));
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(CoreError::Validation(errors))
    }
}

impl Core {
    pub async fn update_settings(
        &self,
        actor: &Actor,
        patch: SettingsPatch,
    ) -> Result<AppSettings, CoreError> {
        actor.require(Permission::SettingsManage)?;
        validate(&patch)?;

        let tx = self.begin_write().await?;
        let current = load(&tx).await?;
        let mut changes = serde_json::Map::new();
        let mut names = Vec::new();
        let mut apply = |key: &'static str, label: &'static str, differs: bool, value: Value| {
            if differs {
                names.push(label);
                changes.insert(key.to_owned(), value);
            }
        };
        if let Some(v) = &patch.currency_symbol {
            apply(
                "currency_symbol",
                "валюта",
                *v != current.currency_symbol,
                json!(v),
            );
        }
        if let Some(v) = patch.master_percent {
            apply(
                "master_percent",
                "процент мастера",
                v != current.master_percent,
                json!(v),
            );
        }
        if let Some(v) = patch.diagnostics_overdue_days {
            apply(
                "diagnostics_overdue_days",
                "срок диагностики",
                v != current.diagnostics_overdue_days,
                json!(v),
            );
        }
        if let Some(v) = patch.default_warranty_days {
            apply(
                "default_warranty_days",
                "гарантия по умолчанию",
                v != current.default_warranty_days,
                json!(v),
            );
        }
        if let Some(v) = patch.intake_required_fields {
            apply(
                "intake_required_fields",
                "обязательные поля приёма",
                v != current.intake_required_fields,
                json!(v),
            );
        }
        if let Some(v) = &patch.timezone {
            apply("timezone", "часовой пояс", *v != current.timezone, json!(v));
        }
        if changes.is_empty() {
            return Ok(current);
        }
        for (key, value) in &changes {
            put(&tx, key, value).await?;
        }
        let new = load(&tx).await?;
        write_audit(
            &tx,
            self.now(),
            actor,
            NewAuditEntry {
                action: "settings.updated",
                order_id: None,
                client_id: None,
                summary: format!("Изменены настройки: {}", names.join(", ")),
                details: Some(Value::Object(changes)),
            },
        )
        .await?;
        tx.commit().await?;
        *self.settings.write().unwrap_or_else(|e| e.into_inner()) = new.clone();
        Ok(new)
    }
}
