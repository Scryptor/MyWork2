//! Контакты клиентов для телефонной книги: CSV для Google Контактов и vCard.

use chrono::{NaiveDate, NaiveTime};
use mw_domain::contacts::{Contact, contact_phone, contacts_csv, contacts_vcard};
use mw_domain::permission::Permission;
use mw_domain::phone::phone_key;
use mw_domain::validation::FieldError;
use sea_orm::{ColumnTrait, QueryFilter, QuerySelect};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use super::{EXPORT_ROWS_MAX, ExportFile, dmy, ensure_rows};
use crate::clients::{client_filtered, client_ordered, display_name, load_phones};
use crate::{Actor, ClientFilter, ClientPhone, Core, CoreError};
use mw_db::entity::client;

/// Формат файла с контактами.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ContactsFormat {
    /// CSV для импорта в Google Контакты.
    Csv,
    /// vCard 3.0 для iPhone и Android.
    Vcard,
}

impl Core {
    /// Контакты клиентов файлом: условия и порядок как у списка; нужны права «Выгрузка» и
    /// «Просмотр клиентов». `created_from` — только клиенты, добавленные с 00:00 этого дня
    /// (местное время предприятия, как `created_at`). Номер, уже выданный более раннему клиенту
    /// (по `phone_key`), пропускается, клиент без номеров тоже; ничего не осталось — ошибка
    /// `rows`/`nothing_to_export`; больше `EXPORT_ROWS_MAX` клиентов — `rows`/`too_many_rows`.
    pub async fn export_contacts(
        &self,
        actor: &Actor,
        filter: &ClientFilter,
        created_from: Option<NaiveDate>,
        format: ContactsFormat,
    ) -> Result<ExportFile, CoreError> {
        actor.require(Permission::DataExport)?;
        actor.require(Permission::ClientsView)?;
        let mut select = client_filtered(filter);
        if let Some(day) = created_from {
            select = select.filter(client::Column::CreatedAt.gte(day.and_time(NaiveTime::MIN)));
        }
        let max = u64::try_from(EXPORT_ROWS_MAX).unwrap_or(u64::MAX);
        let models = client_ordered(select, self.db.get_database_backend(), filter)
            .limit(max.saturating_add(1))
            .all(&self.db)
            .await?;
        ensure_rows(models.len(), EXPORT_ROWS_MAX)?;
        let ids: Vec<i64> = models.iter().map(|m| m.id).collect();
        let mut phones = load_phones(&self.db, &ids).await?;
        let mut seen = HashSet::new();
        let mut contacts = Vec::new();
        for m in models {
            let stored: Vec<ClientPhone> = phones.remove(&m.id).unwrap_or_default();
            let numbers: Vec<String> = stored
                .iter()
                .filter_map(|p| {
                    let number = contact_phone(&p.phone)?;
                    seen.insert(phone_key(&p.phone)).then_some(number)
                })
                .collect();
            if !numbers.is_empty() {
                contacts.push(Contact {
                    name: display_name(&m.full_name, &stored).to_owned(),
                    phones: numbers,
                });
            }
        }
        if contacts.is_empty() {
            return Err(CoreError::Validation(vec![FieldError::new(
                "rows",
                "nothing_to_export",
            )]));
        }
        let day = dmy(self.now().date());
        Ok(match format {
            ContactsFormat::Csv => ExportFile {
                name: format!("Контакты {day}.csv"),
                ascii_name: "contacts.csv",
                content_type: "text/csv; charset=utf-8",
                bytes: contacts_csv(&contacts).into_bytes(),
            },
            ContactsFormat::Vcard => ExportFile {
                name: format!("Контакты {day}.vcf"),
                ascii_name: "contacts.vcf",
                content_type: "text/vcard; charset=utf-8",
                bytes: contacts_vcard(&contacts).into_bytes(),
            },
        })
    }
}
