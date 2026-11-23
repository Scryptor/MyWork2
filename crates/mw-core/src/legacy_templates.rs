//! Текст старых шаблонов актов MyWork 2 для редактора.
//!
//! Импорт сохраняет файлы `Shablon_*.htm` в `legacy_rows` (`source = "file"`, `source_key` — имя
//! файла, `raw.text` — уже декодированный текст). Здесь HTML превращается в текст, а метки вида
//! `NUMZAKAZPRINT` — в поля подстановки `{заказ.номер}`. Дальше текст вставляет в новый шаблон человек.

use mw_db::entity::legacy_row;
use mw_docs::fields::fields;
use mw_domain::documents::DocumentKind;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};

use crate::documents::plain_text;
use crate::{Actor, Core, CoreError};

/// Текст старого шаблона акта, переведённый в текст блока «Текст».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct LegacyTemplate {
    /// Имя файла в старой папке.
    pub file: String,
    /// Текст с полями в фигурных скобках; строки разделены переводами строк.
    pub text: String,
}

/// Файл старой программы, из которого берётся текст вида.
fn legacy_file(kind: DocumentKind) -> Option<&'static str> {
    Some(match kind {
        DocumentKind::IntakeAct => "Shablon_Act_Priema.htm",
        DocumentKind::IssueAct => "Shablon_Act_vidachi.htm",
        DocumentKind::WarrantyIntakeAct => "Shablon_act_priemaGarantii.htm",
        DocumentKind::WarrantyIssueAct => "Shablon_act_vidachiGarantii.htm",
        _ => return None,
    })
}

/// Во что превращается метка: строки из подписи и ключа поля (подпись `None` — только поле).
type Expansion = &'static [(Option<&'static str>, &'static str)];

/// Метки MyWork 2 и ключи полей `mw_docs::fields`. Имена полей по-русски здесь не пишутся.
const MARKERS: &[(&str, Expansion)] = &[
    ("NUMZAKAZPRINT", &[(None, "order.number")]),
    ("DATAPRIEMAPRINT", &[(None, "order.received")]),
    ("DATAVIDACHIPRINT", &[(None, "order.issued")]),
    ("FIOPRINT", &[(None, "client.name")]),
    ("PHONEPRINT", &[(None, "client.phone")]),
    ("ADRESSCLIENTAIFNOTNULLPRINT", &[(None, "client.address")]),
    ("TYPEUSTRPRINT", &[(None, "order.device")]),
    ("SERIALNPIRNT", &[(None, "order.serial")]),
    ("KOMPLEKTNOSTPRINT", &[(None, "order.completeness")]),
    ("VNESHVIPRINT", &[(None, "order.condition")]),
    ("NEISPRAVNOSTPRINT", &[(None, "order.malfunction")]),
    ("VIPRABOTPRINT", &[(None, "order.work_done")]),
    ("PREDOPLATAPRINT", &[(None, "order.prepayment")]),
    ("PREDSTOIMOSTPRINT", &[(None, "order.estimate")]),
    ("GARANTYPRINT", &[(None, "order.warranty")]),
    ("FIRMNAMEPRINT", &[(None, "firm.name")]),
    ("FIRMTELPRINT", &[(None, "firm.phone")]),
    ("DANNIEOFIRMEPRINT", &[(None, "firm.details")]),
    ("URDANNIEPRINT", &[(None, "firm.legal")]),
    ("DOGOVORPRIEMPRINT", &[(None, "firm.intake_terms")]),
    ("DOGOVORVIDACHAPRINT", &[(None, "firm.issue_terms")]),
    (
        "PRICEFULLPRINT",
        &[
            (Some("Сумма предоплаты: "), "order.prepayment"),
            (Some("К оплате: "), "order.due"),
        ],
    ),
    ("SKIDKAPRINT", &[(Some("Скидка: "), "order.discount")]),
];

/// Подстановка для слова; `None` — слово не метка (или у метки нет поля: оставляем как есть).
fn expand(word: &str) -> Option<String> {
    let (_, parts) = MARKERS.iter().find(|(marker, _)| *marker == word)?;
    let mut lines = Vec::new();
    for (label, key) in *parts {
        let field = fields().iter().find(|f| f.key == *key)?;
        lines.push(format!("{}{{{}}}", label.unwrap_or(""), field.placeholder));
    }
    Some(lines.join("\n"))
}

/// Заменяет метки целыми словами: слово — самая длинная подряд идущая цепочка букв, цифр и `_`,
/// поэтому `XNUMZAKAZPRINT` и `NUMZAKAZPRINT1` остаются как есть, а `PRICEFULLPRINT` не путается
/// с более короткими метками. Вызывается после `plain_text`: Word рвёт метку тегами, а там они
/// уже склеены. Фигурные скобки в старом тексте не экранируются: у записи полей нет экранирования,
/// непохожее на поле остаётся буквами.
fn map_markers(text: &str) -> String {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(is_word) {
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        let end = rest.find(|c: char| !is_word(c)).unwrap_or(rest.len());
        let word = &rest[..end];
        match expand(word) {
            Some(replacement) => out.push_str(&replacement),
            None => out.push_str(word),
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

impl Core {
    /// Текст старого шаблона вида из `legacy_rows`; `None` — файла нет, вид без файла (наклейка,
    /// пустой бланк, калибровка) или импорта не было. Из нескольких записей берётся новейшая.
    /// Доступно любому вошедшему.
    pub async fn legacy_document_template(
        &self,
        _actor: &Actor,
        kind: DocumentKind,
    ) -> Result<Option<LegacyTemplate>, CoreError> {
        let Some(file) = legacy_file(kind) else {
            return Ok(None);
        };
        let row = legacy_row::Entity::find()
            .filter(legacy_row::Column::Source.eq("file"))
            .filter(legacy_row::Column::SourceKey.eq(file))
            .order_by_desc(legacy_row::Column::Id)
            .one(&self.db)
            .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let raw: serde_json::Value = serde_json::from_str(&row.raw)
            .map_err(|e| CoreError::Internal(format!("след файла {file} не читается: {e}")))?;
        let Some(text) = raw.get("text").and_then(|t| t.as_str()) else {
            return Ok(None);
        };
        Ok(Some(LegacyTemplate {
            file: file.to_owned(),
            text: map_markers(&plain_text(text)),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_marker_field_exists() {
        for (marker, _) in MARKERS {
            assert!(
                expand(marker).is_some(),
                "{marker}: нет поля в mw_docs::fields"
            );
        }
    }

    #[test]
    fn words_are_replaced_whole() {
        assert_eq!(map_markers("XFIOPRINT FIOPRINT_ FIOPRINT"), {
            let f = expand("FIOPRINT").unwrap();
            format!("XFIOPRINT FIOPRINT_ {f}")
        });
    }
}
