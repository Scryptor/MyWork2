//! Печатные документы: данные из заказа, клиента и настроек и рендер в фоне.
//!
//! Значения полей (`mw_docs::fields`) собираются здесь как текст; в разметку Typst они не попадают —
//! генератор читает их из `data.json` внутри документа.

use std::collections::BTreeMap;

use chrono::{Datelike, NaiveDate};
use mw_db::entity::service_point;
use mw_docs::{DocsError, DocumentData, RenderOptions, Rendered, sample_data, standard_template};
use mw_domain::documents::DocumentKind;
use mw_domain::money::{Money, format_money_doc};
use mw_domain::phone::format_phone;
use mw_domain::validation::FieldError;
use sea_orm::EntityTrait;

use crate::{Actor, AppSettings, Client, Core, CoreError, Order};

/// Сколько документов строится одновременно; остальные ждут своей очереди.
pub(crate) const MAX_CONCURRENT_RENDERS: usize = 2;

/// Что напечатать.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DocumentRequest {
    pub kind: DocumentKind,
    /// Заказ, из которого печатается документ ([`DocumentKind::for_order`]); у пустого акта приёма
    /// и калибровки не учитывается.
    pub order_id: Option<i64>,
    /// Образец — для видов из заказа без `order_id`: заказ, клиент и дата — примеры
    /// (`mw_docs::sample_data`), фирма и точка — из настроек, где они заданы.
    pub sample: bool,
    /// Размер наклейки, смещение и DPI принтера — как пришли.
    pub options: RenderOptions,
}

/// В каком виде выгрузить документ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentOutput {
    Pdf,
    /// Страницы PNG с разрешением `dpi` (растр для печати или предпросмотра).
    Png {
        dpi: u32,
    },
}

/// Готовый документ: PDF или PNG по странице.
#[derive(Clone, PartialEq, Eq)]
pub enum DocumentBytes {
    Pdf(Vec<u8>),
    Png(Vec<Vec<u8>>),
}

/// В отладочном выводе — размеры, а не байты.
impl std::fmt::Debug for DocumentBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pdf(pdf) => write!(f, "Pdf({} байт)", pdf.len()),
            Self::Png(pages) => {
                let sizes: Vec<usize> = pages.iter().map(Vec::len).collect();
                write!(f, "Png({sizes:?} байт)")
            }
        }
    }
}

/// Выгруженный документ.
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentFile {
    /// Размер первой страницы, мм (ширина, высота).
    pub page_size_mm: (f64, f64),
    pub page_count: usize,
    pub bytes: DocumentBytes,
}

impl Core {
    /// Документ в PDF или PNG. API документов зовёт только этот метод: рендер и выгрузка в байты
    /// идут одним заданием под ограничителем рендера (растр A4 тяжелее самой компиляции). Данные и
    /// проверки — те же, что у [`Core::render_document`]. Неверный `dpi` у PNG — ошибка проверки
    /// поля `dpi`.
    pub async fn export_document(
        &self,
        actor: &Actor,
        req: DocumentRequest,
        output: DocumentOutput,
    ) -> Result<DocumentFile, CoreError> {
        self.rendered(actor, req, move |doc| {
            let bytes = match output {
                DocumentOutput::Pdf => DocumentBytes::Pdf(doc.pdf()?),
                DocumentOutput::Png { dpi } => DocumentBytes::Png(doc.png_pages(dpi)?),
            };
            Ok(DocumentFile {
                page_size_mm: doc.page_size_mm(),
                page_count: doc.page_count(),
                bytes,
            })
        })
        .await
    }

    /// Строит документ. Из заказа — с теми же правами, что и просмотр заказа (`get_order`): нужен
    /// `order_id` (он важнее `sample`) или `sample`, иначе ошибка проверки. Пустой акт приёма и
    /// калибровка — всегда без заказа. Рендер идёт вне рабочих потоков, не больше
    /// [`MAX_CONCURRENT_RENDERS`] одновременно. Для текста документа (`Rendered::text`); байты для
    /// печати отдаёт [`Core::export_document`].
    pub async fn render_document(
        &self,
        actor: &Actor,
        req: DocumentRequest,
    ) -> Result<Rendered, CoreError> {
        self.rendered(actor, req, Ok).await
    }

    /// Данные документа, затем рендер и `finish` одним заданием под ограничителем рендера.
    async fn rendered<R: Send + 'static>(
        &self,
        actor: &Actor,
        req: DocumentRequest,
        finish: impl FnOnce(Rendered) -> Result<R, DocsError> + Send + 'static,
    ) -> Result<R, CoreError> {
        let data = self.document_data(actor, &req).await?;
        let template = standard_template(req.kind);
        let options = req.options;
        self.render_gate
            .run(move || mw_docs::render(&template, &data, &options).and_then(finish))
            .await?
            .map_err(|e| match e {
                DocsError::Dpi(_) => {
                    CoreError::Validation(vec![FieldError::new("dpi", "out_of_range")])
                }
                e => CoreError::Internal(format!(
                    "документ «{}» не построен: {e}",
                    req.kind.as_str()
                )),
            })
    }

    /// Значения полей документа.
    pub(crate) async fn document_data(
        &self,
        actor: &Actor,
        req: &DocumentRequest,
    ) -> Result<DocumentData, CoreError> {
        let settings = self.settings();
        let today = self.now().date();
        match req.kind {
            DocumentKind::Calibration => Ok(DocumentData::default()),
            // Бланк заполняют от руки: поля заказа и клиента остаются линиями, свои — из настроек.
            DocumentKind::BlankIntakeAct => {
                let point = self.point_name(settings.default_service_point_id).await?;
                Ok(DocumentData {
                    values: own_values(&settings, &point, today),
                    barcode: None,
                    blank: true,
                })
            }
            DocumentKind::IntakeAct
            | DocumentKind::IssueAct
            | DocumentKind::WarrantyIntakeAct
            | DocumentKind::WarrantyIssueAct
            | DocumentKind::Sticker => match (req.order_id, req.sample) {
                (Some(id), _) => {
                    let order = self.get_order(actor, id).await?;
                    let client = self.get_client(actor, order.client_id).await?;
                    let point = self.point_name(order.service_point_id).await?;
                    Ok(order_data(&order, &client, &point, &settings, today))
                }
                // Образец — на своих реквизитах: фирма и точка из настроек, где они заданы.
                (None, true) => {
                    let point = self.point_name(settings.default_service_point_id).await?;
                    let mut data = sample_data(req.kind);
                    let own = firm_values(&settings, &point)
                        .into_iter()
                        .filter(|(_, value)| !value.trim().is_empty());
                    data.values.extend(own);
                    Ok(data)
                }
                (None, false) => Err(CoreError::Validation(vec![FieldError::required(
                    "order_id",
                )])),
            },
        }
    }

    /// Название точки; архивная печатается как есть, нет точки — пусто.
    async fn point_name(&self, id: Option<i64>) -> Result<String, CoreError> {
        let Some(id) = id else {
            return Ok(String::new());
        };
        Ok(service_point::Entity::find_by_id(id)
            .one(&self.db)
            .await?
            .map(|p| p.name)
            .unwrap_or_default())
    }
}

/// Поля фирмы и точки: реквизиты и договоры — простым текстом ([`plain_text`]); сохранённые
/// настройки не меняются.
fn firm_values(settings: &AppSettings, point: &str) -> BTreeMap<String, String> {
    [
        ("firm.name", plain_text(&settings.firm_name)),
        ("firm.phone", plain_text(&settings.firm_phone)),
        ("firm.details", plain_text(&settings.firm_details)),
        ("firm.legal", plain_text(&settings.firm_legal_details)),
        ("firm.intake_terms", plain_text(&settings.intake_terms)),
        ("firm.issue_terms", plain_text(&settings.issue_terms)),
        ("point.name", point.to_owned()),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_owned(), value))
    .collect()
}

/// Свои поля документа: фирма, точка и дата документа.
fn own_values(settings: &AppSettings, point: &str, today: NaiveDate) -> BTreeMap<String, String> {
    let mut values = firm_values(settings, point);
    values.insert("doc.date".to_owned(), date_ru(today));
    values
}

/// Все поля реестра: свои — из настроек, остальные — из заказа и клиента. Пустая сумма (нет
/// предоплаты, скидки, итога) — пустое значение: строку скрывает шаблон.
fn order_data(
    order: &Order,
    client: &Client,
    point: &str,
    settings: &AppSettings,
    today: NaiveDate,
) -> DocumentData {
    let symbol = settings.currency_symbol.as_str();
    let money = |m: Money| format_money_doc(m, symbol);
    let positive = |m: Money| {
        if m > Money::ZERO {
            money(m)
        } else {
            String::new()
        }
    };
    let phones: Vec<String> = client
        .phones
        .iter()
        .map(|p| format_phone(&p.phone))
        .collect();
    let mut values = own_values(settings, point, today);
    values.extend(
        [
            ("order.number", order.id.to_string()),
            ("order.barcode", order.barcode.clone()),
            ("order.received", date_ru(order.received_at.date())),
            (
                "order.issued",
                order
                    .issued_at
                    .map(|at| date_ru(at.date()))
                    .unwrap_or_default(),
            ),
            (
                "order.device",
                device_line(&order.device_type, &order.brand, &order.model),
            ),
            ("order.device_type", order.device_type.clone()),
            ("order.brand", order.brand.clone()),
            ("order.model", order.model.clone()),
            ("order.serial", order.serial.clone()),
            ("order.color", order.color.clone()),
            ("order.completeness", order.completeness.clone()),
            ("order.condition", order.device_condition.clone()),
            ("order.malfunction", order.malfunction.clone()),
            ("order.work_done", order.work_done.clone()),
            ("order.master", order.master.clone()),
            (
                "order.estimate",
                estimate_text(order.estimate_from, order.estimate_to, symbol),
            ),
            ("order.prepayment", positive(order.prepayment)),
            ("order.discount", positive(order.discount)),
            (
                "order.price",
                order.final_price.map(money).unwrap_or_default(),
            ),
            (
                "order.due",
                amount_due(order.final_price, order.prepayment, order.discount)
                    .map(money)
                    .unwrap_or_default(),
            ),
            ("order.warranty", order.warranty.clone()),
            ("client.name", client.full_name.clone()),
            ("client.phone", phones.first().cloned().unwrap_or_default()),
            ("client.phones", phones.join(", ")),
            ("client.address", client.address.clone()),
        ]
        .map(|(key, value)| (key.to_owned(), value)),
    );
    DocumentData {
        values,
        barcode: Some(order.barcode.clone()).filter(|code| !code.is_empty()),
        blank: false,
    }
}

const MONTHS_GENITIVE: [&str; 12] = [
    "января",
    "февраля",
    "марта",
    "апреля",
    "мая",
    "июня",
    "июля",
    "августа",
    "сентября",
    "октября",
    "ноября",
    "декабря",
];

/// «22 ноября 2026».
fn date_ru(date: NaiveDate) -> String {
    let month = MONTHS_GENITIVE
        .get(date.month0() as usize)
        .copied()
        .unwrap_or_default();
    format!("{} {month} {}", date.day(), date.year())
}

/// Тип, бренд и модель через пробел, пустые пропускаются: «Смартфон Samsung Galaxy A52».
fn device_line(device_type: &str, brand: &str, model: &str) -> String {
    [device_type, brand, model]
        .map(str::trim)
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// «от 1 000 до 2 500 руб.»; одна граница или равные — одна сумма; без оценки — пусто. Как
/// `estimateText` в `apps/ui/src/lib/orders/view.ts`, но словами.
fn estimate_text(from: Option<Money>, to: Option<Money>, symbol: &str) -> String {
    match (from, to) {
        (Some(from), Some(to)) if from != to => format!(
            "от {} до {}",
            format_money_doc(from, ""),
            format_money_doc(to, symbol)
        ),
        (Some(one), _) | (None, Some(one)) => format_money_doc(one, symbol),
        (None, None) => String::new(),
    }
}

/// «К оплате» = итог − предоплата − скидка; без итога — нет суммы. Та же формула, что в
/// `apps/ui/src/lib/orders/OrderFields.svelte` (`toPay` из `apps/ui/src/lib/orders/view.ts`).
fn amount_due(final_price: Option<Money>, prepayment: Money, discount: Money) -> Option<Money> {
    final_price.map(|price| {
        Money::from_kopecks(
            price
                .kopecks()
                .saturating_sub(prepayment.kopecks())
                .saturating_sub(discount.kopecks()),
        )
    })
}

/// Текст из настроек для печати: HTML из MyWork 2 — простым текстом с переносами строк, обычный
/// текст — как есть.
///
/// `<br>`, `</p>`, `</div>`, `</li>` — перевод строки (и перевод строки сразу за ними в исходном
/// тексте с ним сливается: MyWork 2 вставлял текст в HTML, где он ничего не значил); остальные теги
/// и комментарии убираются; `&nbsp; &amp; &lt; &gt; &quot; &#NNN; &#xHH;` раскрываются, незнакомое
/// остаётся как написано. Больше двух переводов строк подряд — два, края обрезаются.
pub fn plain_text(html_or_text: &str) -> String {
    let text = html_or_text.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_str();
    while let Some(at) = rest.find(['<', '&']) {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        if let Some((len, breaks)) = tag(rest) {
            rest = &rest[len..];
            if breaks {
                out.push('\n');
                rest = after_source_line_break(rest);
            }
        } else if let Some((len, c)) = entity(rest) {
            out.push(c);
            rest = &rest[len..];
        } else {
            // `<` или `&`, за которыми нет разметки, — обычный знак (оба однобайтовые).
            out.push_str(&rest[..1]);
            rest = &rest[1..];
        }
    }
    out.push_str(rest);
    at_most_two_line_breaks(out.trim())
}

/// Тег или комментарий в начале `s`: его длина и признак «переносит строку». Тег начинается с `<`,
/// за которым латинская буква, `/`, `!` или `?`, и кончается первым `>`; иначе это не тег.
fn tag(s: &str) -> Option<(usize, bool)> {
    let body = s.strip_prefix('<')?;
    if let Some(comment) = body.strip_prefix("!--") {
        let end = comment.find("-->")?;
        return Some(("<!--".len() + end + "-->".len(), false));
    }
    let (closing, name_start) = match body.strip_prefix('/') {
        Some(name_start) => (true, name_start),
        None => (false, body),
    };
    if !name_start.starts_with(|c: char| c.is_ascii_alphabetic() || c == '!' || c == '?') {
        return None;
    }
    let end = body.find('>')?;
    if body[..end].contains('<') {
        return None;
    }
    let name = name_start
        .chars()
        .take_while(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_lowercase();
    let breaks = name == "br" || (closing && matches!(name.as_str(), "p" | "div" | "li"));
    Some(("<".len() + end + ">".len(), breaks))
}

/// Знакомая HTML-сущность в начале `s`: её длина и знак. Управляющие знаки, кроме перевода строки
/// и табуляции, не раскрываются.
fn entity(s: &str) -> Option<(usize, char)> {
    let body = s.strip_prefix('&')?;
    // Самая длинная — `#x10FFFF`.
    let end = body.find(';').filter(|&end| end <= "#x10FFFF".len())?;
    let name = &body[..end];
    let c = match name {
        "nbsp" => '\u{a0}',
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        _ => {
            let code = name.strip_prefix('#')?;
            let value = match code.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => code.parse().ok()?,
            };
            char::from_u32(value).filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))?
        }
    };
    Some(("&".len() + end + ";".len(), c))
}

/// Перевод строки исходного текста сразу за тегом-переносом (пробелы перед ним не в счёт) уже
/// учтён тегом.
fn after_source_line_break(rest: &str) -> &str {
    rest.trim_start_matches([' ', '\t'])
        .strip_prefix('\n')
        .unwrap_or(rest)
}

fn at_most_two_line_breaks(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut run = 0;
    for c in text.chars() {
        run = if c == '\n' { run + 1 } else { 0 };
        if run <= 2 {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use chrono::{TimeZone, Utc};
    use mw_db::entity::role;
    use mw_docs::fields::fields;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    use super::*;
    use crate::{
        ClientPhone, ClientRef, FixedClock, NewClient, NewOrder, NewUser, OrderPatch, SettingsPatch,
    };

    async fn core_on_22_november(db: sea_orm::DatabaseConnection) -> Core {
        let clock = Arc::new(FixedClock::new(
            Utc.with_ymd_and_hms(2026, 11, 22, 9, 0, 0).unwrap(),
        ));
        Core::open(db, clock, "UTC").await.unwrap()
    }

    fn admin() -> Actor {
        Actor::unrestricted("Тест")
    }

    fn request(kind: DocumentKind, order_id: Option<i64>) -> DocumentRequest {
        DocumentRequest {
            kind,
            order_id,
            sample: false,
            options: RenderOptions::default(),
        }
    }

    fn phone(digits: &str) -> ClientPhone {
        ClientPhone {
            phone: digits.into(),
            note: "мама".into(),
        }
    }

    async fn master(core: &Core, name: &str) -> i64 {
        let role_id = role::Entity::find()
            .filter(role::Column::Name.eq("Мастер"))
            .one(&core.db)
            .await
            .unwrap()
            .unwrap()
            .id;
        let user = NewUser {
            login: String::new(),
            display_name: name.into(),
            role_id,
            password: String::new(),
            can_sign_in: false,
            is_master: true,
        };
        core.create_user(&admin(), user).await.unwrap().id
    }

    /// Каждое поле реестра заполняется из заказа: у полностью заполненного выданного заказа пустых нет.
    #[tokio::test]
    async fn every_field_is_filled_from_a_full_order() {
        for (label, db) in mw_db::testing::all_backends().await {
            let core = core_on_22_november(db).await;
            let point = core
                .create_service_point(&admin(), "Центральная")
                .await
                .unwrap();
            let firm = SettingsPatch {
                firm_name: Some("Сервис".into()),
                firm_phone: Some("+7 900 000-00-01".into()),
                firm_details: Some("ул. Мастеров, 5".into()),
                firm_legal_details: Some("ИНН 000000000000".into()),
                intake_terms: Some("Принято на диагностику.".into()),
                issue_terms: Some("Претензий нет.".into()),
                ..Default::default()
            };
            core.update_settings(&admin(), firm).await.unwrap();
            let master_id = master(&core, "Иванов И. И.").await;
            let new = NewOrder {
                client: ClientRef::New(NewClient {
                    full_name: "Петров Пётр".into(),
                    phones: vec![phone("89000000000"), phone("200000")],
                    address: "ул. Примерная, 1".into(),
                    ..Default::default()
                }),
                service_point_id: Some(point.id),
                device_type: "Смартфон".into(),
                brand: "Samsung".into(),
                model: "Galaxy A52".into(),
                serial: "R58R00000AB".into(),
                color: "Чёрный".into(),
                completeness: "Чехол".into(),
                device_condition: "Царапины".into(),
                malfunction: "Не включается".into(),
                master_user_id: Some(master_id),
                estimate_from: Some(Money::from_rubles(1000)),
                estimate_to: Some(Money::from_rubles(2500)),
                prepayment: Money::from_rubles(500),
                warranty: "30 дней".into(),
                ..Default::default()
            };
            let order = core.create_order(&admin(), new).await.unwrap();
            let patch = OrderPatch {
                final_price: Some(Some(Money::from_rubles(2000))),
                discount: Some(Money::from_rubles(200)),
                work_done: Some("Замена разъёма".into()),
                ..Default::default()
            };
            let order = core
                .update_order(&admin(), order.id, order.version, patch)
                .await
                .unwrap();
            let order = core
                .issue_order(&admin(), order.id, order.version)
                .await
                .unwrap();

            let data = core
                .document_data(&admin(), &request(DocumentKind::IssueAct, Some(order.id)))
                .await
                .unwrap();
            let keys: BTreeSet<&str> = data.values.keys().map(String::as_str).collect();
            let all: BTreeSet<&str> = fields().iter().map(|f| f.key).collect();
            assert_eq!(keys, all, "{label}");
            let empty: Vec<&String> = data
                .values
                .iter()
                .filter(|(_, v)| v.trim().is_empty())
                .map(|(k, _)| k)
                .collect();
            assert!(empty.is_empty(), "{label}: пустые {empty:?}");
            let value = |key: &str| data.values.get(key).cloned().unwrap_or_default();
            for (key, expected) in [
                ("order.number", order.id.to_string()),
                ("order.barcode", order.barcode.clone()),
                ("order.received", "22 ноября 2026".into()),
                ("order.issued", "22 ноября 2026".into()),
                ("order.device", "Смартфон Samsung Galaxy A52".into()),
                ("order.condition", "Царапины".into()),
                ("order.master", "Иванов И. И.".into()),
                (
                    "order.estimate",
                    "от 1\u{202f}000 до 2\u{202f}500\u{a0}руб.".into(),
                ),
                ("order.prepayment", "500\u{a0}руб.".into()),
                ("order.discount", "200\u{a0}руб.".into()),
                ("order.price", "2\u{202f}000\u{a0}руб.".into()),
                ("order.due", "1\u{202f}300\u{a0}руб.".into()),
                ("client.phone", "+7 900 000-00-00".into()),
                ("client.phones", "+7 900 000-00-00, 20-00-00".into()),
                ("firm.legal", "ИНН 000000000000".into()),
                ("point.name", "Центральная".into()),
                ("doc.date", "22 ноября 2026".into()),
            ] {
                assert_eq!(value(key), expected, "{label}: {key}");
            }
            assert_eq!(data.barcode.as_deref(), Some(order.barcode.as_str()));
            assert!(!data.blank, "{label}");
        }
    }

    /// Пустой акт: только свои поля фирмы, точка по умолчанию и дата; поля заказа и клиента — линии.
    #[tokio::test]
    async fn blank_act_has_only_the_firm_point_and_date() {
        for (label, db) in mw_db::testing::all_backends().await {
            let core = core_on_22_november(db).await;
            let point = core
                .create_service_point(&admin(), "Центральная")
                .await
                .unwrap();
            let settings = SettingsPatch {
                firm_name: Some("Сервис".into()),
                default_service_point_id: Some(Some(point.id)),
                ..Default::default()
            };
            core.update_settings(&admin(), settings).await.unwrap();
            let data = core
                .document_data(&admin(), &request(DocumentKind::BlankIntakeAct, None))
                .await
                .unwrap();
            let own: BTreeSet<&str> = fields()
                .iter()
                .map(|f| f.key)
                .filter(|k| !k.starts_with("order.") && !k.starts_with("client."))
                .collect();
            let keys: BTreeSet<&str> = data.values.keys().map(String::as_str).collect();
            assert_eq!(keys, own, "{label}");
            assert_eq!(data.values["firm.name"], "Сервис", "{label}");
            assert_eq!(data.values["point.name"], "Центральная", "{label}");
            assert_eq!(data.values["doc.date"], "22 ноября 2026", "{label}");
            assert!(data.blank && data.barcode.is_none(), "{label}");

            let calibration = core
                .document_data(&admin(), &request(DocumentKind::Calibration, Some(1)))
                .await
                .unwrap();
            assert_eq!(calibration, DocumentData::default(), "{label}");
        }
    }

    /// Образец: заказ, клиент и дата — примеры; фирма и точка — из настроек, где они заданы.
    #[tokio::test]
    async fn sample_takes_the_firm_and_the_point_from_the_settings_where_set() {
        for (label, db) in mw_db::testing::all_backends().await {
            let clock = Arc::new(FixedClock::new(
                Utc.with_ymd_and_hms(2027, 3, 8, 9, 0, 0).unwrap(),
            ));
            let core = Core::open(db, clock, "UTC").await.unwrap();
            let point = core
                .create_service_point(&admin(), "Северная")
                .await
                .unwrap();
            let settings = SettingsPatch {
                firm_name: Some("Сервис".into()),
                intake_terms: Some("Принято.<br>Без вскрытия.".into()),
                default_service_point_id: Some(Some(point.id)),
                ..Default::default()
            };
            core.update_settings(&admin(), settings).await.unwrap();
            let sample = DocumentRequest {
                sample: true,
                ..request(DocumentKind::IssueAct, None)
            };
            let data = core.document_data(&admin(), &sample).await.unwrap();
            let expected = sample_data(DocumentKind::IssueAct);
            let keys: BTreeSet<&str> = data.values.keys().map(String::as_str).collect();
            let all: BTreeSet<&str> = fields().iter().map(|f| f.key).collect();
            assert_eq!(keys, all, "{label}");
            for (key, value) in &data.values {
                let wanted = match key.as_str() {
                    "firm.name" => "Сервис",
                    "firm.intake_terms" => "Принято.\nБез вскрытия.",
                    "point.name" => "Северная",
                    _ => expected.values[key].as_str(),
                };
                assert_eq!(value, wanted, "{label}: {key}");
            }
            assert_eq!(data.values["doc.date"], "22 ноября 2026", "{label}");
            assert_eq!(data.barcode, expected.barcode, "{label}");
            assert!(!data.blank, "{label}");
        }
    }

    #[test]
    fn dates_name_the_month_in_the_genitive() {
        let date = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
        assert_eq!(date_ru(date(2026, 11, 22)), "22 ноября 2026");
        assert_eq!(date_ru(date(2027, 1, 1)), "1 января 2027");
        assert_eq!(date_ru(date(2026, 5, 9)), "9 мая 2026");
        assert_eq!(date_ru(date(2026, 12, 31)), "31 декабря 2026");
    }

    #[test]
    fn device_joins_type_brand_and_model_present() {
        assert_eq!(
            device_line("Смартфон", "Samsung", "Galaxy A52"),
            "Смартфон Samsung Galaxy A52"
        );
        assert_eq!(device_line("", " Apple ", "iPhone"), "Apple iPhone");
        assert_eq!(device_line("Ноутбук", "", ""), "Ноутбук");
        assert_eq!(device_line("", "", ""), "");
    }

    #[test]
    fn estimate_is_a_range_one_sum_or_nothing() {
        let rub = |r| Some(Money::from_rubles(r));
        assert_eq!(
            estimate_text(rub(1000), rub(2500), "₽"),
            "от 1\u{202f}000 до 2\u{202f}500\u{a0}руб."
        );
        assert_eq!(
            estimate_text(rub(1000), None, "₽"),
            "1\u{202f}000\u{a0}руб."
        );
        assert_eq!(estimate_text(None, rub(2500), "$"), "2\u{202f}500\u{a0}$");
        assert_eq!(estimate_text(rub(800), rub(800), "₽"), "800\u{a0}руб.");
        assert_eq!(estimate_text(None, None, "₽"), "");
    }

    #[test]
    fn amount_due_is_the_final_price_less_prepayment_and_discount() {
        let rub = Money::from_rubles;
        assert_eq!(
            amount_due(Some(rub(2000)), rub(500), rub(200)),
            Some(rub(1300))
        );
        assert_eq!(
            amount_due(Some(rub(1000)), rub(1500), Money::ZERO),
            Some(Money::from_kopecks(-50_000))
        );
        assert_eq!(
            amount_due(Some(Money::ZERO), Money::ZERO, Money::ZERO),
            Some(Money::ZERO)
        );
        assert_eq!(amount_due(None, rub(500), rub(200)), None);
    }
}
