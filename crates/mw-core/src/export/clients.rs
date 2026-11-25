//! Excel-файл со списком клиентов: те же строки и порядок, что у `Core::list_clients`.

use mw_domain::permission::Permission;
use mw_domain::phone::format_phone;
use sea_orm::QuerySelect;

use super::xlsx::{Cell, Column, Sheet, workbook};
use super::{EXPORT_ROWS_MAX, ExportFile, XLSX_CONTENT_TYPE, dmy, ensure_rows};
use crate::clients::{client_filtered, client_ordered, list_items};
use crate::{Actor, ClientFilter, Core, CoreError};

const ASCII_NAME: &str = "clients.xlsx";

const COLUMNS: [&str; 9] = [
    "№ клиента",
    "ФИО",
    "Телефоны",
    "Адрес",
    "Откуда узнал",
    "Чёрный список",
    "Заказов",
    "Примечание",
    "Добавлен",
];

impl Core {
    /// Клиенты файлом Excel: условия и порядок как у списка, без страниц; нужны права
    /// «Просмотр клиентов» и «Выгрузка». Больше `EXPORT_ROWS_MAX` строк — ошибка
    /// `rows`/`too_many_rows`.
    pub async fn export_clients_xlsx(
        &self,
        actor: &Actor,
        filter: &ClientFilter,
    ) -> Result<ExportFile, CoreError> {
        actor.require(Permission::DataExport)?;
        actor.require(Permission::ClientsView)?;
        let max = u64::try_from(EXPORT_ROWS_MAX).unwrap_or(u64::MAX);
        let models = client_ordered(
            client_filtered(filter),
            self.db.get_database_backend(),
            filter,
        )
        .limit(max.saturating_add(1))
        .all(&self.db)
        .await?;
        ensure_rows(models.len(), EXPORT_ROWS_MAX)?;
        let items = list_items(&self.db, models.clone()).await?;
        let text = |s: &str| Cell::Text(s.to_owned());
        let rows = models
            .iter()
            .zip(&items)
            .map(|(m, item)| {
                let phones = item
                    .phones
                    .iter()
                    .map(|p| format_phone(&p.phone))
                    .collect::<Vec<_>>()
                    .join(", ");
                vec![
                    Cell::Int(m.id),
                    text(&m.full_name),
                    Cell::Text(phones),
                    text(&m.address),
                    text(&m.source),
                    if m.blacklisted {
                        text("да")
                    } else {
                        Cell::Empty
                    },
                    Cell::Int(i64::try_from(item.orders_count).unwrap_or(i64::MAX)),
                    text(&m.note),
                    Cell::Day(m.created_at.date()),
                ]
            })
            .collect();
        let sheets = [Sheet {
            name: "Клиенты".into(),
            columns: COLUMNS.map(Column::new).into(),
            rows,
            total: None,
        }];
        let currency = self.settings().currency_symbol.clone();
        let bytes = self
            .export_gate
            .run(move || workbook(&sheets, &currency))
            .await??;
        Ok(ExportFile {
            name: format!("Клиенты {}.xlsx", dmy(self.now().date())),
            ascii_name: ASCII_NAME,
            content_type: XLSX_CONTENT_TYPE,
            bytes,
        })
    }
}
