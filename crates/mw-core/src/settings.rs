use std::collections::HashSet;

use chrono_tz::Tz;
use mw_db::entity::{service_point, setting};
use mw_domain::dictionary::DictionaryKind;
use mw_domain::issue::IssueRequirement;
use mw_domain::permission::Permission;
use mw_domain::search::normalize_search;
use mw_domain::validation::FieldError;
use sea_orm::sea_query::OnConflict;
use sea_orm::{ColumnTrait, ConnectionTrait, DatabaseTransaction, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::core::begin_write_on;
use crate::{Actor, Core, CoreError, EditVia, NewAuditEntry, write_audit};

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
    /// Через сколько суток готовый, но не выданный заказ считается «давно готовым».
    pub ready_stale_days: u16,
    /// Гарантия по умолчанию — печатный текст («30 дней»).
    pub default_warranty: String,
    /// Требовать заполнения обязательных полей при приёме.
    pub intake_required_fields: bool,
    /// Точка по умолчанию (неархивная); пусто — не выбрана.
    #[schema(required = true)]
    pub default_service_point_id: Option<i64>,
    /// Мастер по умолчанию (имя).
    pub default_master: String,
    /// Название фирмы для печатных форм.
    pub firm_name: String,
    /// Телефон фирмы.
    pub firm_phone: String,
    /// Данные фирмы (адрес, часы работы).
    pub firm_details: String,
    /// Реквизиты фирмы.
    pub firm_legal_details: String,
    /// Условия приёма (текст акта приёма).
    pub intake_terms: String,
    /// Условия выдачи (текст акта выдачи).
    pub issue_terms: String,
    /// Часовой пояс предприятия (имя IANA).
    pub timezone: String,
    /// Что должно быть заполнено в заказе перед выдачей; пусто — ничего сверх обычного.
    pub issue_required: Vec<IssueRequirement>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            currency_symbol: "₽".to_owned(),
            master_percent: 50,
            diagnostics_overdue_days: 4,
            ready_stale_days: 30,
            default_warranty: "30 дней".to_owned(),
            intake_required_fields: true,
            default_service_point_id: None,
            default_master: String::new(),
            firm_name: String::new(),
            firm_phone: String::new(),
            firm_details: String::new(),
            firm_legal_details: String::new(),
            intake_terms: String::new(),
            issue_terms: String::new(),
            timezone: "UTC".to_owned(),
            issue_required: Vec::new(),
        }
    }
}

/// Частичное изменение настроек; не переданное поле не меняется. Точка по умолчанию: `null` — сбросить.
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
    /// Через сколько суток готовый, но не выданный заказ считается «давно готовым».
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub ready_stale_days: Option<u16>,
    /// Гарантия по умолчанию — печатный текст.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub default_warranty: Option<String>,
    /// Требовать заполнения обязательных полей при приёме.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub intake_required_fields: Option<bool>,
    /// Точка по умолчанию; `null` — сбросить.
    #[serde(default, deserialize_with = "crate::patch::nullable")]
    pub default_service_point_id: Option<Option<i64>>,
    /// Мастер по умолчанию (имя).
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub default_master: Option<String>,
    /// Название фирмы для печатных форм.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub firm_name: Option<String>,
    /// Телефон фирмы.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub firm_phone: Option<String>,
    /// Данные фирмы (адрес, часы работы).
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub firm_details: Option<String>,
    /// Реквизиты фирмы.
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub firm_legal_details: Option<String>,
    /// Условия приёма (текст акта приёма).
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub intake_terms: Option<String>,
    /// Условия выдачи (текст акта выдачи).
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub issue_terms: Option<String>,
    /// Часовой пояс предприятия (имя IANA).
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub timezone: Option<String>,
    /// Что должно быть заполнено в заказе перед выдачей (без повторов).
    #[serde(default, deserialize_with = "crate::patch::present")]
    #[schema(nullable = false)]
    pub issue_required: Option<Vec<IssueRequirement>>,
}

/// Оставляет в правке только поля, значение которых отличается от текущего.
fn keep_differences(p: &mut SettingsPatch, c: &AppSettings) {
    fn keep<T: PartialEq>(v: &mut Option<T>, cur: &T) {
        if v.as_ref() == Some(cur) {
            *v = None;
        }
    }
    keep(&mut p.currency_symbol, &c.currency_symbol);
    keep(&mut p.master_percent, &c.master_percent);
    keep(&mut p.diagnostics_overdue_days, &c.diagnostics_overdue_days);
    keep(&mut p.ready_stale_days, &c.ready_stale_days);
    keep(&mut p.default_warranty, &c.default_warranty);
    keep(&mut p.intake_required_fields, &c.intake_required_fields);
    keep(&mut p.default_service_point_id, &c.default_service_point_id);
    keep(&mut p.default_master, &c.default_master);
    keep(&mut p.firm_name, &c.firm_name);
    keep(&mut p.firm_phone, &c.firm_phone);
    keep(&mut p.firm_details, &c.firm_details);
    keep(&mut p.firm_legal_details, &c.firm_legal_details);
    keep(&mut p.intake_terms, &c.intake_terms);
    keep(&mut p.issue_terms, &c.issue_terms);
    keep(&mut p.timezone, &c.timezone);
    keep(&mut p.issue_required, &c.issue_required);
}

pub(crate) fn parse_timezone(name: &str) -> Tz {
    name.parse().unwrap_or(Tz::UTC)
}

fn get<T: serde::de::DeserializeOwned>(rows: &[setting::Model], key: &str) -> Option<T> {
    let row = rows.iter().find(|r| r.key == key)?;
    serde_json::from_str(&row.value).ok()
}

/// Читает все настройки; отсутствующие или нечитаемые поля заменяются умолчаниями.
pub(crate) async fn load(conn: &impl ConnectionTrait) -> Result<AppSettings, CoreError> {
    let rows = setting::Entity::find().all(conn).await?;
    let d = AppSettings::default();
    let mut loaded = AppSettings {
        currency_symbol: get(&rows, "currency_symbol").unwrap_or(d.currency_symbol),
        master_percent: get(&rows, "master_percent").unwrap_or(d.master_percent),
        diagnostics_overdue_days: get(&rows, "diagnostics_overdue_days")
            .unwrap_or(d.diagnostics_overdue_days),
        ready_stale_days: get(&rows, "ready_stale_days").unwrap_or(d.ready_stale_days),
        default_warranty: get(&rows, "default_warranty").unwrap_or(d.default_warranty),
        intake_required_fields: get(&rows, "intake_required_fields")
            .unwrap_or(d.intake_required_fields),
        default_service_point_id: get(&rows, "default_service_point_id")
            .unwrap_or(d.default_service_point_id),
        default_master: get(&rows, "default_master").unwrap_or(d.default_master),
        firm_name: get(&rows, "firm_name").unwrap_or(d.firm_name),
        firm_phone: get(&rows, "firm_phone").unwrap_or(d.firm_phone),
        firm_details: get(&rows, "firm_details").unwrap_or(d.firm_details),
        firm_legal_details: get(&rows, "firm_legal_details").unwrap_or(d.firm_legal_details),
        intake_terms: get(&rows, "intake_terms").unwrap_or(d.intake_terms),
        issue_terms: get(&rows, "issue_terms").unwrap_or(d.issue_terms),
        timezone: get(&rows, "timezone").unwrap_or(d.timezone),
        issue_required: get(&rows, "issue_required").unwrap_or(d.issue_required),
    };
    // Точка по умолчанию, которая в архиве или которой нет, считается не выбранной.
    if let Some(id) = loaded.default_service_point_id {
        let live = service_point::Entity::find_by_id(id)
            .filter(service_point::Column::Archived.eq(false))
            .one(conn)
            .await?
            .is_some();
        if !live {
            loaded.default_service_point_id = None;
        }
    }
    Ok(loaded)
}

pub(crate) async fn put(
    conn: &impl ConnectionTrait,
    key: &str,
    value: &Value,
) -> Result<(), CoreError> {
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

/// Мастер и гарантия по умолчанию следуют за значением справочника: при переименовании или
/// объединении настройка, совпадающая (по `normalize_search`) с одним из прежних значений `from`,
/// получает значение `to` — в той же транзакции, с записью журнала. Возвращает новые настройки для
/// кэша `Core` (его обновляют после фиксации) или `None`, если ничего не изменилось.
pub(crate) async fn follow_dictionary_value(
    tx: &DatabaseTransaction,
    core: &Core,
    actor: &Actor,
    kind: DictionaryKind,
    from: &HashSet<String>,
    to: &str,
) -> Result<Option<AppSettings>, CoreError> {
    let (key, label) = match kind {
        DictionaryKind::Master => ("default_master", "мастер по умолчанию"),
        DictionaryKind::Warranty => ("default_warranty", "гарантия по умолчанию"),
        _ => return Ok(None),
    };
    let current = load(tx).await?;
    let value = match kind {
        DictionaryKind::Master => &current.default_master,
        _ => &current.default_warranty,
    };
    let key_of = normalize_search(value);
    if value == to || key_of.is_empty() || !from.contains(&key_of) {
        return Ok(None);
    }
    put(tx, key, &json!(to)).await?;
    let via = EditVia::Dictionary;
    write_audit(
        tx,
        core.now(),
        actor,
        NewAuditEntry {
            action: "settings.updated",
            order_id: None,
            client_id: None,
            summary: format!("Изменены настройки: {label}{}", via.suffix_ru()),
            details: Some(json!({ key: to })),
        },
    )
    .await?;
    Ok(Some(load(tx).await?))
}

impl Core {
    /// Фиксирует транзакцию записи и кладёт в кэш настройки, прочитанные в ней (`None` — кэш не
    /// трогаем). Кэш обновляется, пока держится `settings_gate`, взятый ДО фиксации: следующий
    /// писатель, начавшийся сразу после фиксации, дойдёт до своего `commit_with_settings` только
    /// после того, как мы положим своё значение, поэтому кэш всегда равен последнему
    /// зафиксированному.
    pub(crate) async fn commit_with_settings(
        &self,
        tx: DatabaseTransaction,
        settings: Option<AppSettings>,
    ) -> Result<(), CoreError> {
        let Some(settings) = settings else {
            tx.commit().await?;
            return Ok(());
        };
        let _gate = self.settings_gate.lock().await;
        tx.commit().await?;
        #[cfg(test)]
        tests::pause_after_commit(settings.master_percent).await;
        *self.settings.write().unwrap_or_else(|e| e.into_inner()) = settings;
        Ok(())
    }
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

/// `point_exists` — найдена ли неархивная точка, на которую указывает патч (если указывает).
pub(crate) fn validate(p: &SettingsPatch, point_exists: bool) -> Result<(), CoreError> {
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
    if p.ready_stale_days.is_some_and(|v| !(1..=365).contains(&v)) {
        errors.push(FieldError::new("ready_stale_days", "out_of_range"));
    }
    if matches!(p.default_service_point_id, Some(Some(_))) && !point_exists {
        errors.push(FieldError::new("default_service_point_id", "invalid"));
    }
    if p.timezone
        .as_deref()
        .is_some_and(|t| t.parse::<Tz>().is_err())
    {
        errors.push(FieldError::new("timezone", "invalid"));
    }
    if let Some(list) = &p.issue_required {
        let unique: HashSet<_> = list.iter().collect();
        if unique.len() != list.len() {
            errors.push(FieldError::new("issue_required", "invalid"));
        }
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
        let mut patch = patch;
        for text in [
            &mut patch.default_warranty,
            &mut patch.default_master,
            &mut patch.firm_name,
            &mut patch.firm_phone,
            &mut patch.firm_details,
            &mut patch.firm_legal_details,
            &mut patch.intake_terms,
            &mut patch.issue_terms,
        ]
        .into_iter()
        .flatten()
        {
            *text = text.trim().to_owned();
        }

        let tx = self.begin_write().await?;
        let current = load(&tx).await?;
        // Правило «правка без изменений»: проверяем только то, что действительно отличается.
        keep_differences(&mut patch, &current);
        let point_exists = match patch.default_service_point_id {
            Some(Some(id)) => service_point::Entity::find_by_id(id)
                .filter(service_point::Column::Archived.eq(false))
                .one(&tx)
                .await?
                .is_some(),
            _ => true,
        };
        validate(&patch, point_exists)?;
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
        if let Some(v) = patch.ready_stale_days {
            apply(
                "ready_stale_days",
                "срок «давно готов»",
                v != current.ready_stale_days,
                json!(v),
            );
        }
        if let Some(v) = &patch.default_warranty {
            apply(
                "default_warranty",
                "гарантия по умолчанию",
                *v != current.default_warranty,
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
        if let Some(v) = patch.default_service_point_id {
            apply(
                "default_service_point_id",
                "точка по умолчанию",
                v != current.default_service_point_id,
                json!(v),
            );
        }
        if let Some(v) = &patch.default_master {
            apply(
                "default_master",
                "мастер по умолчанию",
                *v != current.default_master,
                json!(v),
            );
        }
        if let Some(v) = &patch.firm_name {
            apply(
                "firm_name",
                "название фирмы",
                *v != current.firm_name,
                json!(v),
            );
        }
        if let Some(v) = &patch.firm_phone {
            apply(
                "firm_phone",
                "телефон фирмы",
                *v != current.firm_phone,
                json!(v),
            );
        }
        if let Some(v) = &patch.firm_details {
            apply(
                "firm_details",
                "данные фирмы",
                *v != current.firm_details,
                json!(v),
            );
        }
        if let Some(v) = &patch.firm_legal_details {
            apply(
                "firm_legal_details",
                "реквизиты",
                *v != current.firm_legal_details,
                json!(v),
            );
        }
        if let Some(v) = &patch.intake_terms {
            apply(
                "intake_terms",
                "условия приёма",
                *v != current.intake_terms,
                json!(v),
            );
        }
        if let Some(v) = &patch.issue_terms {
            apply(
                "issue_terms",
                "условия выдачи",
                *v != current.issue_terms,
                json!(v),
            );
        }
        if let Some(v) = &patch.timezone {
            apply("timezone", "часовой пояс", *v != current.timezone, json!(v));
        }
        if let Some(v) = &patch.issue_required {
            apply(
                "issue_required",
                "обязательно при выдаче",
                *v != current.issue_required,
                json!(v),
            );
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
        self.commit_with_settings(tx, Some(new.clone())).await?;
        Ok(new)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use tokio::sync::Notify;

    use crate::{Actor, Core, FixedClock, SettingsPatch};

    /// Запись с этим `master_percent` остановится после `commit`, до записи в кэш: сообщит
    /// `reached` и подождёт `release` (один раз).
    struct Pause {
        percent: u8,
        reached: Arc<Notify>,
        release: Arc<Notify>,
    }
    static PAUSE_AFTER_COMMIT: Mutex<Vec<Pause>> = Mutex::new(Vec::new());

    pub(super) async fn pause_after_commit(percent: u8) {
        let pause = {
            let mut guard = PAUSE_AFTER_COMMIT.lock().unwrap();
            let at = guard.iter().position(|p| p.percent == percent);
            at.map(|i| guard.remove(i))
        };
        if let Some(p) = pause {
            p.reached.notify_one();
            p.release.notified().await;
        }
    }

    async fn write(core: &Core, percent: u8) {
        core.update_settings(
            &Actor::unrestricted("Тест"),
            SettingsPatch {
                master_percent: Some(percent),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    }

    /// Запись A зафиксирована, но ещё не положила значение в кэш; запись B начинается после
    /// её фиксации. Кэш после обеих равен последнему зафиксированному (B), то есть базе.
    #[tokio::test]
    async fn cache_equals_the_last_committed_write_even_if_an_earlier_one_is_slow() {
        for (label, db) in mw_db::testing::all_backends().await {
            let clock = Arc::new(FixedClock::new(chrono::Utc::now()));
            let core = Core::open(db.clone(), clock.clone(), "UTC").await.unwrap();
            let (a, b) = (61u8, 62u8);
            let reached = Arc::new(Notify::new());
            let release = Arc::new(Notify::new());
            PAUSE_AFTER_COMMIT.lock().unwrap().push(Pause {
                percent: a,
                reached: reached.clone(),
                release: release.clone(),
            });
            let first = {
                let core = core.clone();
                tokio::spawn(async move { write(&core, a).await })
            };
            reached.notified().await;
            // A в базе, в кэше ещё нет. B берёт блокировку записи (свободна) и доходит до
            // фиксации: с `settings_gate` она ждёт A, без него — обгоняет и кладёт своё значение.
            let second = {
                let core = core.clone();
                tokio::spawn(async move { write(&core, b).await })
            };
            tokio::time::sleep(Duration::from_millis(300)).await;
            release.notify_one();
            first.await.unwrap();
            second.await.unwrap();
            let stored = Core::open(db, clock, "UTC").await.unwrap();
            assert_eq!(stored.settings().master_percent, b, "{label}");
            assert_eq!(core.settings().master_percent, b, "{label}: кэш устарел");
        }
    }
}
