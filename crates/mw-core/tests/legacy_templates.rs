#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Текст старых шаблонов MyWork 2: метки заменяются полями, разметка — текстом.

mod common;

use common::*;
use mw_core::{ImportMode, ImportedLegacyRow, LegacyTemplate};
use mw_docs::fields::fields;
use mw_domain::documents::DocumentKind;
use serde_json::json;

/// Синтетический акт приёма в том виде, в каком его хранит `legacy_rows` (после cp1251).
const INTAKE_HTML: &str = "<html><body><p>Акт N <b>NUMZAKAZ</b><b>PRINT</b><o:p></o:p></p>\r\n<p>Клиент: FIOPRINT</p>\r\n<p>PRICEFULLPRINT</p>\r\n<p>Гарантия на чистку — 14 дней</p></body></html>";

fn file_row(name: &str, text: &str) -> ImportedLegacyRow {
    ImportedLegacyRow {
        source: "file".to_owned(),
        source_key: name.to_owned(),
        target: None,
        raw: json!({ "encoding": "windows-1251", "text": text }),
    }
}

fn placeholder(key: &str) -> String {
    let f = fields().iter().find(|f| f.key == key).unwrap();
    format!("{{{}}}", f.placeholder)
}

async fn with_files(files: &[(&str, &str)]) -> Vec<TestCore> {
    let mut out = Vec::new();
    for t in cores().await {
        let mut bundle = sample_bundle();
        for (name, text) in files {
            bundle.legacy_rows.push(file_row(name, text));
        }
        t.core
            .import_bundle(&admin(), bundle, ImportMode::IntoEmpty)
            .await
            .unwrap();
        out.push(t);
    }
    out
}

async fn legacy(t: &TestCore, kind: DocumentKind) -> Option<LegacyTemplate> {
    t.core
        .legacy_document_template(&admin(), kind)
        .await
        .unwrap()
}

#[tokio::test]
async fn legacy_intake_template_is_text_with_fields() {
    for t in with_files(&[("Shablon_Act_Priema.htm", INTAKE_HTML)]).await {
        let l = legacy(&t, DocumentKind::IntakeAct).await.unwrap();
        assert_eq!(l.file, "Shablon_Act_Priema.htm", "{}", t.label);
        assert!(l.text.contains(&placeholder("order.number")), "{}", l.text);
        assert!(l.text.contains(&placeholder("client.name")), "{}", l.text);
        assert!(
            l.text.contains("Гарантия на чистку — 14 дней"),
            "{}",
            l.text
        );
        for bad in ["NUMZAKAZPRINT", "FIOPRINT", "<", "o:p"] {
            assert!(!l.text.contains(bad), "{}: {bad}", l.text);
        }
        assert_eq!(l.text.lines().count(), 5, "{}", l.text);
    }
}

#[tokio::test]
async fn price_and_discount_expand() {
    let html = "<p>PRICEFULLPRINT</p><p>SKIDKAPRINT</p>";
    for t in with_files(&[("Shablon_Act_Priema.htm", html)]).await {
        let l = legacy(&t, DocumentKind::IntakeAct).await.unwrap();
        let expected = format!(
            "Сумма предоплаты: {}\nК оплате: {}\nСкидка: {}",
            placeholder("order.prepayment"),
            placeholder("order.due"),
            placeholder("order.discount")
        );
        assert_eq!(l.text, expected, "{}", t.label);
    }
}

#[tokio::test]
async fn markers_match_whole_words_longest_first() {
    // Метки рядом (`PRICEFULLPRINT` и похожие по началу), метка в буквах, метка в цифрах.
    let html = "<p>PRICEFULLPRINT PRICEPRINT XNUMZAKAZPRINT NUMZAKAZPRINTX NUMZAKAZPRINT1 \
                NUMZAKAZPRINT,FIOPRINT; ADRESSCLIENTAIFNOTNULLPRINT ADRESSCLIENTPRINT</p>";
    for t in with_files(&[("Shablon_Act_Priema.htm", html)]).await {
        let l = legacy(&t, DocumentKind::IntakeAct).await.unwrap();
        let number = placeholder("order.number");
        let want = format!(
            "Сумма предоплаты: {}\nК оплате: {} PRICEPRINT XNUMZAKAZPRINT NUMZAKAZPRINTX \
             NUMZAKAZPRINT1 {number},{}; {} ADRESSCLIENTPRINT",
            placeholder("order.prepayment"),
            placeholder("order.due"),
            placeholder("client.name"),
            placeholder("client.address"),
        );
        assert_eq!(l.text, want, "{}", t.label);
    }
}

#[tokio::test]
async fn every_marker_maps_to_its_field() {
    let table = [
        ("NUMZAKAZPRINT", "order.number"),
        ("DATAPRIEMAPRINT", "order.received"),
        ("DATAVIDACHIPRINT", "order.issued"),
        ("FIOPRINT", "client.name"),
        ("PHONEPRINT", "client.phone"),
        ("ADRESSCLIENTAIFNOTNULLPRINT", "client.address"),
        ("TYPEUSTRPRINT", "order.device"),
        ("SERIALNPIRNT", "order.serial"),
        ("KOMPLEKTNOSTPRINT", "order.completeness"),
        ("VNESHVIPRINT", "order.condition"),
        ("NEISPRAVNOSTPRINT", "order.malfunction"),
        ("VIPRABOTPRINT", "order.work_done"),
        ("PREDOPLATAPRINT", "order.prepayment"),
        ("PREDSTOIMOSTPRINT", "order.estimate"),
        ("GARANTYPRINT", "order.warranty"),
        ("FIRMNAMEPRINT", "firm.name"),
        ("FIRMTELPRINT", "firm.phone"),
        ("DANNIEOFIRMEPRINT", "firm.details"),
        ("URDANNIEPRINT", "firm.legal"),
        ("DOGOVORPRIEMPRINT", "firm.intake_terms"),
        ("DOGOVORVIDACHAPRINT", "firm.issue_terms"),
    ];
    let html = table
        .iter()
        .map(|(m, _)| format!("<p>{m}</p>"))
        .collect::<String>();
    let want = table
        .iter()
        .map(|(_, k)| placeholder(k))
        .collect::<Vec<_>>()
        .join("\n");
    for t in with_files(&[("Shablon_act_vidachiGarantii.htm", &html)]).await {
        let l = legacy(&t, DocumentKind::WarrantyIssueAct).await.unwrap();
        assert_eq!(l.text, want, "{}", t.label);
    }
}

#[tokio::test]
async fn no_legacy_template_is_none() {
    for t in with_files(&[
        ("Shablon_Act_Priema.htm", INTAKE_HTML),
        ("Shablon_Priem.htm", "<html>Акт</html>"),
    ])
    .await
    {
        for kind in [
            DocumentKind::Sticker,
            DocumentKind::IssueAct,
            DocumentKind::WarrantyIntakeAct,
            DocumentKind::BlankIntakeAct,
        ] {
            assert!(legacy(&t, kind).await.is_none(), "{} {kind:?}", t.label);
        }
    }
    // Импорта не было вовсе.
    for t in cores().await {
        assert!(legacy(&t, DocumentKind::IntakeAct).await.is_none());
    }
}

#[tokio::test]
async fn files_map_to_kinds() {
    let files = [
        ("Shablon_Act_Priema.htm", DocumentKind::IntakeAct),
        ("Shablon_Act_vidachi.htm", DocumentKind::IssueAct),
        (
            "Shablon_act_priemaGarantii.htm",
            DocumentKind::WarrantyIntakeAct,
        ),
        (
            "Shablon_act_vidachiGarantii.htm",
            DocumentKind::WarrantyIssueAct,
        ),
    ];
    let rows: Vec<(&str, &str)> = files.iter().map(|(f, _)| (*f, "<p>FIOPRINT</p>")).collect();
    for t in with_files(&rows).await {
        for (file, kind) in files {
            let l = legacy(&t, kind).await.unwrap();
            assert_eq!(l.file, file, "{}", t.label);
        }
    }
}
