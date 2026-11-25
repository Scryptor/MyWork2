//! Excel-файл отчёта по заказам: листы «Итоги», разрез и «Заказы».

use mw_db::entity::{service_point, user};
use mw_domain::money::Money;
use mw_domain::permission::Permission;
use sea_orm::EntityTrait;

use super::xlsx::{Cell, Column, Sheet, workbook};
use super::{EXPORT_ROWS_MAX, ExportFile, XLSX_CONTENT_TYPE, dmy, ensure_rows};
use crate::reports::orders::{ReportOrderRow, groups};
use crate::{Actor, Core, CoreError, OrderReport, ReportFilter, ReportGroupBy, ReportTotals};

const ASCII_NAME: &str = "report.xlsx";

/// Подписи заданных фильтров для листа «Итоги»; пустой фильтр не попадает.
#[derive(Default)]
struct FilterLabels {
    service_point: Option<String>,
    master: Option<String>,
    device_type: Option<String>,
    brand: Option<String>,
    source: Option<String>,
}

/// Период словами: «01.11.2026–30.11.2026», «с 01.11.2026», «по 30.11.2026»; без границ — `None`.
fn period(filter: &ReportFilter) -> Option<String> {
    match (filter.from, filter.to) {
        (Some(from), Some(to)) => Some(format!("{}\u{2013}{}", dmy(from), dmy(to))),
        (Some(from), None) => Some(format!("с {}", dmy(from))),
        (None, Some(to)) => Some(format!("по {}", dmy(to))),
        (None, None) => None,
    }
}

fn file_name(filter: &ReportFilter) -> String {
    let period = period(filter).unwrap_or_else(|| "за всё время".to_owned());
    format!("Отчёт по заказам {period}.xlsx")
}

/// Значение текстового фильтра без пробелов по краям; пустое — «нет фильтра».
fn given(value: &Option<String>) -> Option<&str> {
    value.as_deref().map(str::trim).filter(|v| !v.is_empty())
}

fn count(n: u64) -> Cell {
    Cell::Int(i64::try_from(n).unwrap_or(i64::MAX))
}

fn totals_cells(t: &ReportTotals) -> Vec<Cell> {
    vec![
        count(t.orders),
        count(t.without_repair),
        Cell::Money(t.revenue),
        Cell::Money(t.costs),
        Cell::Money(t.profit),
        Cell::Money(t.master_pay),
    ]
}

fn summary_sheet(filter: &ReportFilter, report: &OrderReport, labels: &FilterLabels) -> Sheet {
    let text = |s: &str| Cell::Text(s.to_owned());
    let t = &report.totals;
    let mut rows = vec![vec![
        text("Период"),
        text(&period(filter).unwrap_or_else(|| "За всё время".to_owned())),
    ]];
    for (title, label) in [
        ("Точка", &labels.service_point),
        ("Мастер", &labels.master),
        ("Тип устройства", &labels.device_type),
        ("Бренд", &labels.brand),
        ("Источник", &labels.source),
    ] {
        if let Some(label) = label {
            rows.push(vec![text(title), text(label)]);
        }
    }
    let money = |title: &str, m: Money| vec![text(title), Cell::Money(m)];
    rows.extend([
        vec![text("Заказов"), count(t.orders)],
        vec![text("Из них без ремонта"), count(t.without_repair)],
        money("Выручка", t.revenue),
        money("Скидки", t.discounts),
        money("Затраты", t.costs),
        money("Прибыль", t.profit),
        money("Оплата мастерам", t.master_pay),
        vec![
            text("Процент мастера"),
            Cell::Text(format!("{} %", report.master_percent)),
        ],
    ]);
    Sheet {
        name: "Итоги".into(),
        columns: vec![Column::new("Показатель"), Column::new("Значение")],
        rows,
        total: None,
    }
}

fn group_sheet(by: ReportGroupBy, report: &OrderReport) -> Sheet {
    let (name, first) = match by {
        ReportGroupBy::Master => ("По мастерам", "Мастер"),
        ReportGroupBy::ServicePoint => ("По точкам", "Точка"),
        ReportGroupBy::DeviceType => ("По типам", "Тип устройства"),
        ReportGroupBy::Brand => ("По брендам", "Бренд"),
        ReportGroupBy::Source => ("По источникам", "Источник"),
    };
    let row = |label: &str, t: &ReportTotals| {
        let mut cells = vec![Cell::Text(label.to_owned())];
        cells.extend(totals_cells(t));
        cells
    };
    Sheet {
        name: name.into(),
        columns: [
            first,
            "Заказов",
            "Без ремонта",
            "Выручка",
            "Затраты",
            "Прибыль",
            "Оплата мастеру",
        ]
        .map(Column::new)
        .into(),
        rows: report
            .groups
            .iter()
            .map(|g| row(&g.label, &g.totals))
            .collect(),
        total: Some(row("Итого", &report.totals)),
    }
}

fn orders_sheet(rows: &[ReportOrderRow]) -> Sheet {
    let text = |s: &str| Cell::Text(s.to_owned());
    let point = |r: &ReportOrderRow| {
        if r.service_point_archived {
            format!("{} (в архиве)", r.service_point)
        } else {
            r.service_point.clone()
        }
    };
    Sheet {
        name: "Заказы".into(),
        columns: [
            "№ заказа",
            "Принят",
            "Выдан",
            "Дней в сервисе",
            "Клиент",
            "Телефон",
            "Тип устройства",
            "Бренд",
            "Модель",
            "Неисправность",
            "Выполненные работы",
            "Мастер",
            "Точка",
            "Источник",
            "Итог",
            "Скидка",
            "Затраты",
            "Прибыль",
            "Оплата мастеру",
            "Без ремонта",
        ]
        .map(Column::new)
        .into(),
        rows: rows
            .iter()
            .map(|r| {
                vec![
                    Cell::Int(r.id),
                    Cell::Time(r.received_at),
                    r.issued_at.map_or(Cell::Empty, Cell::Time),
                    r.issued_at.map_or(Cell::Empty, |at| {
                        Cell::Int((at.date() - r.received_at.date()).num_days())
                    }),
                    text(&r.client_name),
                    text(&r.phone),
                    text(&r.device_type),
                    text(&r.brand),
                    text(&r.model),
                    text(&r.malfunction),
                    text(&r.work_done),
                    text(&r.master),
                    Cell::Text(point(r)),
                    text(&r.source),
                    Cell::Money(r.price),
                    Cell::Money(r.discount),
                    Cell::Money(r.costs),
                    Cell::Money(r.profit),
                    Cell::Money(r.master_pay),
                    if r.without_repair {
                        text("да")
                    } else {
                        Cell::Empty
                    },
                ]
            })
            .collect(),
        total: None,
    }
}

impl Core {
    /// Подписи фильтров: как у групп (самое частое написание, «(в архиве)»); если заказов нет —
    /// название из справочника или то, что передано.
    async fn report_filter_labels(
        &self,
        filter: &ReportFilter,
        rows: &[ReportOrderRow],
    ) -> Result<FilterLabels, CoreError> {
        // Фильтр оставил в отчёте одну группу этого разреза.
        let from_rows = |by: ReportGroupBy| {
            groups(by, rows)
                .ok()
                .and_then(|g| g.into_iter().next())
                .map(|g| g.label)
        };
        let mut labels = FilterLabels::default();
        if let Some(id) = filter.service_point_id {
            labels.service_point = Some(match from_rows(ReportGroupBy::ServicePoint) {
                Some(label) => label,
                None => service_point::Entity::find_by_id(id)
                    .one(&self.db)
                    .await?
                    .map_or_else(|| id.to_string(), |p| p.name),
            });
        }
        if let Some(id) = filter.master_user_id {
            labels.master = Some(match from_rows(ReportGroupBy::Master) {
                Some(label) => label,
                None => user::Entity::find_by_id(id)
                    .one(&self.db)
                    .await?
                    .map_or_else(|| id.to_string(), |u| u.display_name),
            });
        }
        for (value, by, slot) in [
            (
                &filter.device_type,
                ReportGroupBy::DeviceType,
                &mut labels.device_type,
            ),
            (&filter.brand, ReportGroupBy::Brand, &mut labels.brand),
            (&filter.source, ReportGroupBy::Source, &mut labels.source),
        ] {
            if let Some(raw) = given(value) {
                *slot = Some(from_rows(by).unwrap_or_else(|| raw.to_owned()));
            }
        }
        Ok(labels)
    }

    /// Отчёт по заказам файлом Excel; нужны права «Отчёты» и «Выгрузка» (лист «Заказы» несёт
    /// клиентов и телефоны). Листы: «Итоги», разрез и «Заказы».
    pub async fn order_report_xlsx(
        &self,
        actor: &Actor,
        filter: &ReportFilter,
    ) -> Result<ExportFile, CoreError> {
        actor.require(Permission::ReportsView)?;
        actor.require(Permission::DataExport)?;
        let (report, rows) = self.report_with_rows(actor, filter).await?;
        ensure_rows(rows.len(), EXPORT_ROWS_MAX)?;
        let labels = self.report_filter_labels(filter, &rows).await?;
        let sheets = [
            summary_sheet(filter, &report, &labels),
            group_sheet(filter.group_by, &report),
            orders_sheet(&rows),
        ];
        let currency = self.settings().currency_symbol.clone();
        let bytes = self
            .export_gate
            .run(move || workbook(&sheets, &currency))
            .await??;
        Ok(ExportFile {
            name: file_name(filter),
            ascii_name: ASCII_NAME,
            content_type: XLSX_CONTENT_TYPE,
            bytes,
        })
    }
}
