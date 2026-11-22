#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use mw_core::documents::{DocumentBytes, DocumentOutput, DocumentRequest, plain_text};
use mw_core::{ClientRef, CoreError, NewClient, NewOrder, OrderPatch, SettingsPatch};
use mw_docs::{RenderOptions, font_covers};
use mw_domain::documents::DocumentKind;
use mw_domain::money::{DOC_CURRENCY_REPLACEMENTS, Money, format_money_doc};

/// Между разрядами — узкий неразрывный пробел, перед знаком валюты — неразрывный, как в интерфейсе.
fn sum(text: &str) -> String {
    text.replace(' ', "\u{202f}").replace('_', "\u{a0}")
}

fn request(kind: DocumentKind, order_id: Option<i64>) -> DocumentRequest {
    DocumentRequest {
        kind,
        order_id,
        sample: false,
        options: RenderOptions::default(),
    }
}

async fn text(t: &TestCore, req: DocumentRequest) -> String {
    t.core
        .render_document(&admin(), req)
        .await
        .unwrap_or_else(|e| panic!("{}: {req:?}: {e}", t.label))
        .text()
}

/// Каждая строка — по разу в каждом экземпляре акта.
fn assert_twice(label: &str, out: &str, rows: &[String]) {
    for row in rows {
        assert_eq!(
            out.matches(row.as_str()).count(),
            2,
            "{label}: {row:?} в {out:?}"
        );
    }
}

fn order_with_client(phones_list: &[&str]) -> NewOrder {
    NewOrder {
        client: ClientRef::New(NewClient {
            full_name: "Пётр Иванов".into(),
            phones: phones(phones_list),
            ..Default::default()
        }),
        ..sample_new_order()
    }
}

#[tokio::test]
async fn intake_act_prints_the_order_its_client_and_the_firm() -> Result<(), CoreError> {
    for t in cores().await {
        t.core
            .update_settings(
                &admin(),
                SettingsPatch {
                    firm_name: Some("Сервис «Тест»".into()),
                    ..Default::default()
                },
            )
            .await?;
        let order = t
            .core
            .create_order(
                &admin(),
                NewOrder {
                    serial: "SN-1".into(),
                    prepayment: Money::from_rubles(1500),
                    ..order_with_client(&["8 (921) 123 45 67", "123456"])
                },
            )
            .await?;
        let out = text(&t, request(DocumentKind::IntakeAct, Some(order.id))).await;
        assert_twice(
            t.label,
            &out,
            &[
                format!("Акт приёма № {} от 11 ноября 2026", order.id),
                "КлиентПётр Иванов".into(),
                "Телефон+7 921 123-45-67".into(),
                "АдресНе указан".into(),
                "УстройствоТелефон Samsung A52".into(),
                "Серийный номерSN-1".into(),
                "НеисправностьРазбит экран".into(),
                "Предварительная стоимостьНе оговорена".into(),
                format!("Предоплата{}", sum("1 500_руб.")),
                "Сервис «Тест»".into(),
                order.barcode.clone(),
            ],
        );
        assert!(!out.contains("12-34-56"), "{}: {out:?}", t.label);
        assert!(font_covers(&out).is_empty(), "{}: {out:?}", t.label);

        // The sign of the settings, as it is when the font has it.
        t.core
            .update_settings(
                &admin(),
                SettingsPatch {
                    currency_symbol: Some("€".into()),
                    ..Default::default()
                },
            )
            .await?;
        let out = text(&t, request(DocumentKind::IntakeAct, Some(order.id))).await;
        assert_twice(t.label, &out, &[format!("Предоплата{}", sum("1 500_€"))]);

        let sticker = text(&t, request(DocumentKind::Sticker, Some(order.id))).await;
        for line in [
            format!("№ {} · 11 ноября 2026", order.id),
            "Пётр Иванов".into(),
            "+7 921 123-45-67".into(),
            "Телефон Samsung A52".into(),
            order.barcode.clone(),
        ] {
            assert_eq!(
                sticker.matches(line.as_str()).count(),
                1,
                "{}: {line:?} в {sticker:?}",
                t.label
            );
        }
    }
    Ok(())
}

#[test]
fn plain_text_turns_mywork2_html_into_lines() {
    let examples = [
        (
            "ИП Иванов И. И.<br>ИНН 000000000000",
            "ИП Иванов И. И.\nИНН 000000000000",
        ),
        (
            "Строка 1<br/>Строка 2<BR />Строка 3",
            "Строка 1\nСтрока 2\nСтрока 3",
        ),
        (
            "<b>Внимание!</b> Срок хранения&nbsp;— 30 дней",
            "Внимание! Срок хранения\u{a0}— 30 дней",
        ),
        ("ООО &quot;Пример&quot;", "ООО \"Пример\""),
        ("Договор &#8470; 5, &#x2116; 6", "Договор № 5, № 6"),
        ("&amp; &lt;приложение&gt; &#39;А&#39;", "& <приложение> 'А'"),
        ("<p>Первый</p>\r\n<p>Второй</p>", "Первый\nВторой"),
        ("<div>а</div><div>б</div>", "а\nб"),
        ("<ul><li>а</li><li>б</li></ul>", "а\nб"),
        ("Сдал<br>\r\nПринял<br>\n", "Сдал\nПринял"),
        ("а<br><br><br><br>б", "а\n\nб"),
        ("а\n\n\n\nб", "а\n\nб"),
        ("  <br>текст<br>  ", "текст"),
        ("<font color=\"red\">Важно</font>", "Важно"),
        ("<!-- <br> -->текст", "текст"),
        ("&copy; 2026 &#0; &#xZZ;", "&copy; 2026 &#0; &#xZZ;"),
    ];
    for (html, text) in examples {
        assert_eq!(plain_text(html), text, "{html:?}");
    }
    for text in [
        "Срок ремонта — до 45 дней.\nГарантия 30 дней.",
        "Абзац\n\nДругой абзац",
        "Скидка < 5 % & цена > 0",
        "R&D; 3 <4",
        "",
    ] {
        assert_eq!(plain_text(text), text, "{text:?}");
    }
}

#[tokio::test]
async fn html_in_the_settings_prints_as_lines_and_stays_saved() -> Result<(), CoreError> {
    for t in cores().await {
        let legal = "ИП Примеров П. П.<br>ИНН 000000000000<br/>Р/с&nbsp;0000";
        t.core
            .update_settings(
                &admin(),
                SettingsPatch {
                    firm_legal_details: Some(legal.into()),
                    intake_terms: Some("<p>Устройство принято.</p>\r\n<p>Срок — 3 дня.</p>".into()),
                    ..Default::default()
                },
            )
            .await?;
        let order = t.core.create_order(&admin(), sample_new_order()).await?;
        let out = text(&t, request(DocumentKind::IntakeAct, Some(order.id))).await;
        assert_twice(
            t.label,
            &out,
            &[
                "ИП Примеров П. П.\nИНН 000000000000\nР/с\u{a0}0000".into(),
                "Устройство принято.\nСрок — 3 дня.".into(),
            ],
        );
        for markup in ["<", "&nbsp;", "&#"] {
            assert!(!out.contains(markup), "{}: {markup} в {out:?}", t.label);
        }
        assert_eq!(t.core.settings().firm_legal_details, legal, "{}", t.label);
    }
    Ok(())
}

#[tokio::test]
async fn issue_act_prints_an_order_not_issued_yet_without_the_issue_date() -> Result<(), CoreError>
{
    for t in cores().await {
        let order = t
            .core
            .create_order(
                &admin(),
                NewOrder {
                    prepayment: Money::from_rubles(1500),
                    ..sample_new_order()
                },
            )
            .await?;
        let title = format!("Акт выдачи № {} от", order.id);
        let out = text(&t, request(DocumentKind::IssueAct, Some(order.id))).await;
        let after_title: Vec<&str> = out
            .match_indices(&title)
            .map(|(at, _)| out[at + title.len()..].trim_start())
            .collect();
        assert_eq!(after_title.len(), 2, "{}: {out:?}", t.label);
        for after in after_title {
            assert!(after.starts_with("Клиент"), "{}: {out:?}", t.label);
        }

        let master = make_master(&t.core, "Мастер Иванов").await;
        let order = t
            .core
            .update_order(
                &admin(),
                order.id,
                order.version,
                OrderPatch {
                    final_price: Some(Some(Money::from_rubles(5000))),
                    discount: Some(Money::from_rubles(500)),
                    work_done: Some("Замена экрана".into()),
                    master_user_id: Some(Some(master)),
                    ..Default::default()
                },
            )
            .await?;
        t.core
            .issue_order(&admin(), order.id, order.version)
            .await?;
        let out = text(&t, request(DocumentKind::IssueAct, Some(order.id))).await;
        assert_twice(
            t.label,
            &out,
            &[
                format!("Акт выдачи № {} от 11 ноября 2026", order.id),
                "Выполненные работыЗамена экрана".into(),
                "МастерМастер Иванов".into(),
                "Гарантия30 дней".into(),
                format!("Стоимость{}", sum("5 000_руб.")),
                format!("Скидка{}", sum("500_руб.")),
                format!("Предоплата{}", sum("1 500_руб.")),
                // «К оплате» = итог − предоплата − скидка.
                format!("К оплате{}", sum("3 000_руб.")),
            ],
        );
    }
    Ok(())
}

#[tokio::test]
async fn order_documents_need_an_order_or_the_sample() -> Result<(), CoreError> {
    for t in cores().await {
        assert_validation(
            t.core
                .render_document(&admin(), request(DocumentKind::IssueAct, None))
                .await
                .map(|doc| doc.text()),
            &[("order_id", "required")],
        );
        let missing = t
            .core
            .render_document(&admin(), request(DocumentKind::IssueAct, Some(999_999)))
            .await;
        assert!(
            matches!(missing, Err(CoreError::NotFound { id: 999_999, .. })),
            "{}: {:?}",
            t.label,
            missing.map(|d| d.text())
        );

        let sample = DocumentRequest {
            sample: true,
            ..request(DocumentKind::IssueAct, None)
        };
        let out = text(&t, sample).await;
        assert_twice(
            t.label,
            &out,
            &["Акт выдачи № 1234 от 25 ноября 2026".into()],
        );

        // The same rights as viewing the order: no permission is needed.
        let order = t.core.create_order(&admin(), sample_new_order()).await?;
        let nobody = actor_with(&[]);
        t.core.get_order(&nobody, order.id).await?;
        let doc = t
            .core
            .render_document(&nobody, request(DocumentKind::IntakeAct, Some(order.id)))
            .await?;
        assert_eq!(doc.page_count(), 1, "{}", t.label);
    }
    Ok(())
}

#[tokio::test]
async fn blank_act_and_calibration_print_without_an_order() -> Result<(), CoreError> {
    for t in cores().await {
        t.core
            .update_settings(
                &admin(),
                SettingsPatch {
                    firm_name: Some("Сервис «Тест»".into()),
                    ..Default::default()
                },
            )
            .await?;
        let order = t.core.create_order(&admin(), sample_new_order()).await?;
        for order_id in [None, Some(order.id)] {
            let out = text(&t, request(DocumentKind::BlankIntakeAct, order_id)).await;
            assert_twice(
                t.label,
                &out,
                &["Сервис «Тест»".into(), "Акт приёма №".into()],
            );
            for absent in ["Пётр Иванов", "Сервисный центр «Пример»", "Не указан"]
            {
                assert!(!out.contains(absent), "{}: {absent} в {out:?}", t.label);
            }
        }

        let calibration = DocumentRequest {
            options: RenderOptions {
                label_mm: Some((40.0, 30.0)),
                offset_mm: (0.0, 0.0),
                dpi: Some(203),
            },
            ..request(DocumentKind::Calibration, None)
        };
        let doc = t.core.render_document(&admin(), calibration).await?;
        let (w, h) = doc.page_size_mm();
        assert!(
            (w - 40.0).abs() < 0.01 && (h - 30.0).abs() < 0.01,
            "{w}x{h}"
        );
        assert!(
            doc.text().contains("40 × 30 мм · 203 dpi"),
            "{}",
            doc.text()
        );
    }
    Ok(())
}

/// The table of replacements in `format_money_doc` is exactly what the document font lacks.
#[test]
fn currency_signs_the_document_font_lacks_are_replaced_by_words() {
    for (sign, word) in DOC_CURRENCY_REPLACEMENTS {
        assert_eq!(
            font_covers(sign),
            sign.chars().collect::<Vec<_>>(),
            "{sign}"
        );
        assert!(
            font_covers(word).is_empty(),
            "{word}: {:?}",
            font_covers(word)
        );
        let printed = format_money_doc(Money::from_kopecks(-1_234_567), sign);
        assert!(font_covers(&printed).is_empty(), "{printed:?}");
    }
    for sign in ["$", "€"] {
        assert!(font_covers(sign).is_empty(), "{sign}");
    }
}

/// Ruling 12: the sample is the sample order on the firm's own details; an empty setting keeps the
/// sample value. («Пример» alone is not checked: the sample client lives on «ул. Примерная».)
#[tokio::test]
async fn sample_prints_the_firms_own_details_where_they_are_set() -> Result<(), CoreError> {
    let sample = DocumentRequest {
        sample: true,
        ..request(DocumentKind::IntakeAct, None)
    };
    for t in cores().await {
        let out = text(&t, sample).await;
        assert_twice(
            t.label,
            &out,
            &[
                "Сервисный центр «Пример»".into(),
                "ИП Примеров П. П., ИНН 000000000000".into(),
            ],
        );

        t.core
            .update_settings(
                &admin(),
                SettingsPatch {
                    firm_name: Some("Сервис «Тест»".into()),
                    firm_legal_details: Some("ИП Тестов Т. Т.<br>ИНН 111111111111".into()),
                    ..Default::default()
                },
            )
            .await?;
        let out = text(&t, sample).await;
        assert_twice(
            t.label,
            &out,
            &[
                "Сервис «Тест»".into(),
                "ИП Тестов Т. Т.\nИНН 111111111111".into(),
                // The order, the client and the firm's details left empty come from the sample.
                "Акт приёма № 1234 от 22 ноября 2026".into(),
                "КлиентПетров Пётр Петрович".into(),
                "г. Энск, ул. Мастеров, д. 5".into(),
            ],
        );
        for absent in ["«Пример»", "ИП Примеров"] {
            assert!(!out.contains(absent), "{}: {absent} в {out:?}", t.label);
        }
    }
    Ok(())
}

/// Width and height of a PNG from its IHDR chunk.
fn png_size(png: &[u8]) -> (u32, u32) {
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"), "не PNG");
    let be = |at: usize| u32::from_be_bytes(png[at..at + 4].try_into().unwrap());
    (be(16), be(20))
}

#[tokio::test]
async fn export_document_gives_a_pdf_or_png_pages() -> Result<(), CoreError> {
    for t in cores().await {
        let order = t.core.create_order(&admin(), sample_new_order()).await?;
        let act = t
            .core
            .export_document(
                &admin(),
                request(DocumentKind::IntakeAct, Some(order.id)),
                DocumentOutput::Pdf,
            )
            .await?;
        let DocumentBytes::Pdf(pdf) = &act.bytes else {
            panic!("{}: {act:?}", t.label);
        };
        assert!(pdf.starts_with(b"%PDF-"), "{}", t.label);
        assert_eq!(act.page_count, 1, "{}", t.label);
        let (w, h) = act.page_size_mm;
        assert!(
            (w - 210.0).abs() < 0.01 && (h - 297.0).abs() < 0.01,
            "{w}x{h}"
        );

        let label = DocumentRequest {
            options: RenderOptions {
                label_mm: Some((58.0, 40.0)),
                offset_mm: (0.0, 0.0),
                dpi: Some(203),
            },
            ..request(DocumentKind::Sticker, Some(order.id))
        };
        let sticker = t
            .core
            .export_document(&admin(), label, DocumentOutput::Png { dpi: 203 })
            .await?;
        let DocumentBytes::Png(pages) = &sticker.bytes else {
            panic!("{}: {sticker:?}", t.label);
        };
        assert_eq!((pages.len(), sticker.page_count), (1, 1), "{}", t.label);
        let (w, h) = sticker.page_size_mm;
        assert!(
            (w - 58.0).abs() < 0.01 && (h - 40.0).abs() < 0.01,
            "{w}x{h}"
        );
        let (px_w, px_h) = png_size(&pages[0]);
        let px = |mm: f64| mm / 25.4 * 203.0;
        assert!(
            (f64::from(px_w) - px(58.0)).abs() <= 1.0 && (f64::from(px_h) - px(40.0)).abs() <= 1.0,
            "{}: {px_w}x{px_h}",
            t.label
        );
    }
    Ok(())
}

/// The same checks as `render_document`: the export goes the same way to the data.
#[tokio::test]
async fn export_document_fails_as_render_document_does() {
    for t in cores().await {
        for req in [
            request(DocumentKind::IssueAct, None),
            request(DocumentKind::IssueAct, Some(999_999)),
        ] {
            let rendered = t.core.render_document(&admin(), req).await.err();
            let exported = t
                .core
                .export_document(&admin(), req, DocumentOutput::Pdf)
                .await
                .err();
            assert!(rendered.is_some(), "{}: {req:?}", t.label);
            assert_eq!(
                format!("{exported:?}"),
                format!("{rendered:?}"),
                "{}: {req:?}",
                t.label
            );
        }
        let sample = DocumentRequest {
            sample: true,
            ..request(DocumentKind::IssueAct, None)
        };
        assert_validation(
            t.core
                .export_document(&admin(), sample, DocumentOutput::Png { dpi: 0 })
                .await,
            &[("dpi", "out_of_range")],
        );
    }
}
