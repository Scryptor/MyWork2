#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use mw_core::documents::{
    DocumentBatchRequest, DocumentBytes, DocumentOutput, DocumentRequest, plain_text,
};
use mw_core::{ClientRef, CoreError, NewClient, NewOrder, OrderPatch, SettingsPatch};
use mw_db::entity::order;
use mw_docs::{RenderOptions, font_covers};
use mw_domain::documents::DocumentKind;
use mw_domain::money::{DOC_CURRENCY_REPLACEMENTS, Money, format_money_doc};
use mw_domain::pages::PageSelection;
use sea_orm::{ActiveModelTrait, Set};

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
        draft: None,
        pages: None,
    }
}

async fn text(t: &TestCore, req: DocumentRequest) -> String {
    t.core
        .render_document(&admin(), req.clone())
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
        ("&copy; 2026 &#0; &#xZZ;", "© 2026 &#0; &#xZZ;"),
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

/// Разметкой считаются только знакомые теги HTML: адреса, ссылки и слова в угловых скобках
/// остаются текстом.
#[test]
fn plain_text_keeps_angle_brackets_that_are_not_html() {
    for text in [
        "Пишите: <info@service.ru>",
        "Сайт <https://service.ru/price>",
        "<Важно> Без чека не выдаём",
        "a < b > c",
        "<a.b> <bra> <pre-order> <b1> <brx>",
        "Цена &laquo без точки с запятой",
        "&unknown; &laquo",
    ] {
        assert_eq!(plain_text(text), text, "{text:?}");
    }
    let examples = [
        (
            "&laquo;Пример&raquo; &mdash; &ndash; &hellip;",
            "«Пример» — – …",
        ),
        ("Д&apos;Артаньян &copy; 2026", "Д'Артаньян © 2026"),
        ("<p>A<p>B", "A\nB"),
        ("Шапка<div>Текст</div>", "Шапка\nТекст"),
        (
            "<table><tr><td>а</td></tr><tr><td>б</td></tr></table>",
            "а\nб",
        ),
        (
            "Начало<H2 class=\"x\">Раздел</H2>Текст",
            "Начало\nРаздел\nТекст",
        ),
        ("<ol><li>раз<li>два</ol>", "раз\nдва"),
        ("<P>A</P><P>B</P>", "A\nB"),
        ("<p>A</p>\n<p>\nB</p>", "A\nB"),
        ("<!DOCTYPE html><p>Текст</p>", "Текст"),
        (
            "<b>жирно</b> <I>курсив</I> <a href=\"x\">ссылка</a>",
            "жирно курсив ссылка",
        ),
        ("<img src=\"logo.png\"/>Текст<hr>", "Текст"),
        (
            "<html><head><title></title></head><body>Текст</body></html>",
            "Текст",
        ),
    ];
    for (html, text) in examples {
        assert_eq!(plain_text(html), text, "{html:?}");
    }
}

/// Строка из одних пробелов и табуляций пуста: «больше двух переводов строк» действует через неё.
#[test]
fn plain_text_collapses_blank_lines() {
    assert_eq!(plain_text("а\n   \n \t \n\nб"), "а\n\nб");
}

/// Инструкции `<?…?>`, теги с двоеточием (`<o:p>`) и `tfoot`/`code` из Word и редакторов — не текст.
#[test]
fn plain_text_drops_office_tags() {
    let html = "<p>Текст<o:p></o:p></p><?xml version=\"1.0\"?><table><tfoot><tr><td>Итог</td></tr></tfoot></table>";
    assert_eq!(plain_text(html), "Текст\nИтог");
    assert_eq!(plain_text("<code>а</code> <o:p>б</o:p>"), "а б");
    assert_eq!(
        plain_text("а<v:shape id=\"x\">б</v:shape><O:P class=\"y\">в"),
        "абв"
    );
}

/// Двоеточие в `<…>` — тег только из букв по обе стороны (`o:p`); время и адреса остаются текстом.
#[test]
fn plain_text_keeps_colon_text_that_is_not_a_tag() {
    for text in [
        "работаем <10:30>",
        "до </10:30>",
        "<1a:b> <a:1> <a:> <:b> <a:b:c> <mailto:x@y> <https://x.ru>",
    ] {
        assert_eq!(plain_text(text), text, "{text:?}");
    }
}

/// `<?` без `?>` дальше по тексту — обычный текст.
#[test]
fn plain_text_keeps_question_text() {
    assert_eq!(plain_text("a <? b"), "a <? b");
}

/// Без закрывающих `>` и `;` разбор не просматривает остаток текста заново на каждом `<` и `&`.
#[test]
fn plain_text_is_linear_on_unclosed_brackets_and_ampersands() {
    let inputs = [
        "<&".repeat(50_000),
        "<a&b".repeat(25_000),
        "<br ".repeat(25_000),
        "&#x".repeat(33_000),
        "<!--".repeat(25_000),
        "<?".repeat(25_000),
    ];
    let started = std::time::Instant::now();
    for input in &inputs {
        let out = plain_text(input);
        assert!(out.len() >= input.len() / 2, "{}", out.len());
    }
    // Квадратичный разбор просматривал бы на каждом `<` и `&` остаток в десятки тысяч знаков: миллиарды
    // шагов, десятки секунд в отладочной сборке. Линейный укладывается в доли секунды; запас до 5 с —
    // для загруженных машин CI (на macOS бывало 1,15 с).
    let elapsed = started.elapsed();
    assert!(elapsed < std::time::Duration::from_secs(5), "{elapsed:?}");
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

/// Пароль и графический ключ — поля шаблона: стандартные документы заказа их не печатают (акт
/// отдают клиенту), шаблон мастерской, где они написаны, — печатает.
#[tokio::test]
async fn standard_acts_do_not_print_password_or_pattern() -> Result<(), CoreError> {
    use mw_domain::document_template::{Align, Block, BlockBody, CopyFilter};
    for t in cores().await {
        let order = t
            .core
            .create_order(
                &admin(),
                NewOrder {
                    device_password: "Zx-0000-pw".into(),
                    device_pattern: "1236".into(),
                    ..sample_new_order()
                },
            )
            .await?;
        for kind in [
            DocumentKind::IntakeAct,
            DocumentKind::IssueAct,
            DocumentKind::WarrantyIntakeAct,
            DocumentKind::WarrantyIssueAct,
            DocumentKind::Sticker,
        ] {
            let out = text(&t, request(kind, Some(order.id))).await;
            assert!(out.contains("Samsung"), "{}: {kind:?}: {out:?}", t.label);
            for secret in ["Zx-0000-pw", "1 · 2 · 3 · 6", "1236"] {
                assert!(
                    !out.contains(secret),
                    "{}: {kind:?}: {secret} в {out:?}",
                    t.label
                );
            }
        }

        let mut own = mw_docs::standard_template(DocumentKind::IntakeAct);
        own.blocks.push(Block {
            copies: CopyFilter::Both,
            body: BlockBody::Text {
                text: "Пароль: {заказ.пароль}. Ключ: {заказ.графический_ключ}.".into(),
                size_pt: None,
                bold: false,
                align: Align::Left,
                one_line: false,
            },
        });
        t.core
            .save_document_template(
                &admin(),
                DocumentKind::IntakeAct,
                mw_core::TemplateSave {
                    template: own,
                    version: 0,
                },
            )
            .await?;
        let out = text(&t, request(DocumentKind::IntakeAct, Some(order.id))).await;
        assert_twice(
            t.label,
            &out,
            &["Пароль: Zx-0000-pw. Ключ: 1 · 2 · 3 · 6.".into()],
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
        let out = text(&t, sample.clone()).await;
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

/// Знаки, которых нет в шрифте, печатаются словами и в свободном тексте: в неисправности, работах,
/// договорах и сведениях о фирме — так же, как в суммах.
#[tokio::test]
async fn currency_signs_in_free_text_print_as_words() -> Result<(), CoreError> {
    for t in cores().await {
        t.core
            .update_settings(
                &admin(),
                SettingsPatch {
                    issue_terms: Some("Диагностика — 500 ₽, для Казахстана 2000 ₸.".into()),
                    firm_details: Some("Оплата в ₴ не принимается".into()),
                    ..Default::default()
                },
            )
            .await?;
        let order = t
            .core
            .create_order(
                &admin(),
                NewOrder {
                    malfunction: "Не включается после ремонта за 3000 ₽".into(),
                    ..sample_new_order()
                },
            )
            .await?;
        let order = t
            .core
            .update_order(
                &admin(),
                order.id,
                order.version,
                OrderPatch {
                    work_done: Some("Замена платы (₸ 15 000)".into()),
                    ..Default::default()
                },
            )
            .await?;
        let out = text(&t, request(DocumentKind::IssueAct, Some(order.id))).await;
        assert!(
            font_covers(&out).is_empty(),
            "{}: {:?} в {out:?}",
            t.label,
            font_covers(&out)
        );
        assert_twice(
            t.label,
            &out,
            &[
                "Диагностика — 500 руб., для Казахстана 2000 тг.".into(),
                "Оплата в грн не принимается".into(),
                "НеисправностьНе включается после ремонта за 3000 руб.".into(),
                "Выполненные работыЗамена платы (тг 15 000)".into(),
            ],
        );
    }
    Ok(())
}

/// Знак валюты с пробелами по краям — как в интерфейсе: обрезанный, а пустой — «₽» (в документе
/// «руб.»).
#[tokio::test]
async fn a_blank_or_padded_currency_symbol_prints_as_the_interface_shows_it()
-> Result<(), CoreError> {
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
        for (symbol, shown) in [("  ", "руб."), (" ₽ ", "руб."), (" $ ", "$")] {
            t.core
                .update_settings(
                    &admin(),
                    SettingsPatch {
                        currency_symbol: Some(symbol.into()),
                        ..Default::default()
                    },
                )
                .await?;
            let out = text(&t, request(DocumentKind::IntakeAct, Some(order.id))).await;
            assert_twice(
                t.label,
                &out,
                &[format!("Предоплата{}", sum(&format!("1 500_{shown}")))],
            );
        }
    }
    Ok(())
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
        let out = text(&t, sample.clone()).await;
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
        let out = text(&t, sample.clone()).await;
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
            let rendered = t.core.render_document(&admin(), req.clone()).await.err();
            let exported = t
                .core
                .export_document(&admin(), req.clone(), DocumentOutput::Pdf)
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

/// Границы размера наклейки, смещения и разрешения проверяет ядро: ни один вызывающий не заставит
/// его строить огромный растр.
#[tokio::test]
async fn export_document_checks_label_offset_and_dpi_bounds() {
    let with = |options: RenderOptions| DocumentRequest {
        sample: true,
        options,
        ..request(DocumentKind::Sticker, None)
    };
    let label = |w, h| RenderOptions {
        label_mm: Some((w, h)),
        ..RenderOptions::default()
    };
    let offset = |x, y| RenderOptions {
        offset_mm: (x, y),
        ..RenderOptions::default()
    };
    let printer = |dpi| RenderOptions {
        dpi: Some(dpi),
        ..RenderOptions::default()
    };
    type Case = (
        DocumentRequest,
        DocumentOutput,
        Vec<(&'static str, &'static str)>,
    );
    let cases: Vec<Case> = vec![
        (
            with(label(14.9, 40.0)),
            DocumentOutput::Pdf,
            vec![("label_w_mm", "out_of_range")],
        ),
        (
            with(label(58.0, 200.1)),
            DocumentOutput::Pdf,
            vec![("label_h_mm", "out_of_range")],
        ),
        (
            with(label(500.0, 1.0)),
            DocumentOutput::Pdf,
            vec![
                ("label_w_mm", "out_of_range"),
                ("label_h_mm", "out_of_range"),
            ],
        ),
        (
            with(label(f32::NAN, 40.0)),
            DocumentOutput::Pdf,
            vec![("label_w_mm", "out_of_range")],
        ),
        (
            with(offset(10.1, 0.0)),
            DocumentOutput::Pdf,
            vec![("offset_x_mm", "out_of_range")],
        ),
        (
            with(offset(0.0, -10.1)),
            DocumentOutput::Pdf,
            vec![("offset_y_mm", "out_of_range")],
        ),
        (
            with(printer(71)),
            DocumentOutput::Pdf,
            vec![("printer_dpi", "out_of_range")],
        ),
        (
            with(printer(601)),
            DocumentOutput::Pdf,
            vec![("printer_dpi", "out_of_range")],
        ),
        (
            with(RenderOptions::default()),
            DocumentOutput::Png { dpi: 601 },
            vec![("dpi", "out_of_range")],
        ),
        (
            with(RenderOptions::default()),
            DocumentOutput::Png { dpi: 71 },
            vec![("dpi", "out_of_range")],
        ),
    ];
    for t in cores().await {
        for (req, output, expected) in &cases {
            assert_validation(
                t.core.export_document(&admin(), req.clone(), *output).await,
                expected,
            );
        }
        // Границы включены.
        let edge = with(RenderOptions {
            label_mm: Some((15.0, 200.0)),
            offset_mm: (-10.0, 10.0),
            dpi: Some(600),
        });
        t.core
            .export_document(&admin(), edge, DocumentOutput::Png { dpi: 72 })
            .await
            .unwrap_or_else(|e| panic!("{}: {e}", t.label));
    }
}

/// Заказ из MyWork 2 с кодом не в UPC-A: наклейка с Code 128 строится, сканер находит заказ.
#[tokio::test]
async fn legacy_barcode_is_printed_and_found() -> Result<(), CoreError> {
    for t in cores().await {
        let order = t.core.create_order(&admin(), sample_new_order()).await?;
        // Как у импортированных заказов: значение пишется в обход создания заказа.
        order::ActiveModel {
            id: Set(order.id),
            barcode: Set("A-1053".to_owned()),
            ..Default::default()
        }
        .update(t.core.db())
        .await
        .unwrap();

        let sticker = text(&t, request(DocumentKind::Sticker, Some(order.id))).await;
        assert_eq!(
            sticker.matches("A-1053").count(),
            1,
            "{}: {sticker:?}",
            t.label
        );
        let exported = t
            .core
            .export_document(
                &admin(),
                request(DocumentKind::Sticker, Some(order.id)),
                DocumentOutput::Pdf,
            )
            .await?;
        assert_eq!(exported.page_count, 1, "{}", t.label);

        let found = t.core.find_orders_by_barcode(&admin(), "A-1053\n").await?;
        assert_eq!(
            found.iter().map(|o| o.id).collect::<Vec<_>>(),
            vec![order.id],
            "{}",
            t.label
        );
    }
    Ok(())
}

/// A small PNG of one colour, the firm's logo.
fn logo_png() -> Vec<u8> {
    let img = image::RgbImage::from_pixel(80, 30, image::Rgb([20, 60, 160]));
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut out, image::ImageFormat::Png)
        .unwrap();
    out.into_inner()
}

#[tokio::test]
async fn act_prints_uploaded_logo() -> Result<(), CoreError> {
    for (label, db) in mw_db::testing::all_backends().await {
        let clock = std::sync::Arc::new(mw_core::FixedClock::new(
            chrono::TimeZone::with_ymd_and_hms(&chrono::Utc, 2026, 11, 11, 9, 0, 0).unwrap(),
        ));
        let dir = tempfile::tempdir().unwrap();
        let core = mw_core::Core::open(db, clock, "UTC")
            .await?
            .with_attachments_dir(dir.path().to_path_buf());
        let order = core.create_order(&admin(), sample_new_order()).await?;
        let actor = admin();
        let export = |output: DocumentOutput| {
            core.export_document(
                &actor,
                request(DocumentKind::IntakeAct, Some(order.id)),
                output,
            )
        };
        let pages = || async {
            match export(DocumentOutput::Png { dpi: 96 }).await?.bytes {
                DocumentBytes::Png(pages) => Ok::<_, CoreError>(pages),
                other => panic!("{label}: {other:?}"),
            }
        };
        let before = pages().await?;
        core.set_firm_logo(&admin(), logo_png()).await?;
        assert_ne!(pages().await?, before, "{label}: логотипа на листе нет");
        export(DocumentOutput::Pdf).await?;
        core.remove_firm_logo(&admin()).await?;
        assert_eq!(pages().await?, before, "{label}: логотип остался на листе");
        export(DocumentOutput::Pdf).await?;
    }
    Ok(())
}

fn batch(kind: DocumentKind, order_ids: Vec<i64>) -> DocumentBatchRequest {
    DocumentBatchRequest {
        kind,
        order_ids,
        options: RenderOptions {
            label_mm: Some((58.0, 40.0)),
            ..RenderOptions::default()
        },
    }
}

async fn orders(t: &TestCore, n: usize) -> Vec<i64> {
    let mut ids = Vec::new();
    for _ in 0..n {
        ids.push(
            t.core
                .create_order(&admin(), sample_new_order())
                .await
                .unwrap()
                .id,
        );
    }
    ids
}

#[tokio::test]
async fn batch_pdf_has_a_document_per_order() -> Result<(), CoreError> {
    for t in cores().await {
        let ids = orders(&t, 3).await;
        let stickers = t
            .core
            .export_documents(&admin(), batch(DocumentKind::Sticker, ids.clone()))
            .await?;
        let DocumentBytes::Pdf(pdf) = &stickers.bytes else {
            panic!("{}: {stickers:?}", t.label);
        };
        assert!(pdf.starts_with(b"%PDF-"), "{}", t.label);
        assert_eq!(stickers.page_count, 3, "{}", t.label);

        // Акт приёма «2 на лист A4»: на заказ один лист.
        let acts = t
            .core
            .export_documents(
                &admin(),
                DocumentBatchRequest {
                    options: RenderOptions::default(),
                    ..batch(DocumentKind::IntakeAct, ids)
                },
            )
            .await?;
        assert_eq!(acts.page_count, 3, "{}", t.label);
        let (w, h) = acts.page_size_mm;
        assert!(
            (w - 210.0).abs() < 0.01 && (h - 297.0).abs() < 0.01,
            "{w}x{h}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn batch_limits() {
    for t in cores().await {
        assert_validation(
            t.core
                .export_documents(&admin(), batch(DocumentKind::Sticker, vec![]))
                .await,
            &[("order_ids", "required")],
        );
        assert_validation(
            t.core
                .export_documents(&admin(), batch(DocumentKind::Sticker, (1..=101).collect()))
                .await,
            &[("order_ids", "too_many")],
        );
        // Повторы не считаются: 100 разных номеров по два раза проходят проверку числа.
        let doubled: Vec<i64> = (1..=100).flat_map(|n| [n, n]).collect();
        let res = t
            .core
            .export_documents(&admin(), batch(DocumentKind::Sticker, doubled))
            .await;
        assert!(
            matches!(res, Err(CoreError::NotFound { .. })),
            "{}: {res:?}",
            t.label
        );
        assert_validation(
            t.core
                .export_documents(&admin(), batch(DocumentKind::Calibration, vec![1]))
                .await,
            &[("kind", "not_allowed")],
        );
        assert_validation(
            t.core
                .export_documents(&admin(), batch(DocumentKind::BlankIntakeAct, vec![1]))
                .await,
            &[("kind", "not_allowed")],
        );
    }
}

#[tokio::test]
async fn batch_drops_repeats() -> Result<(), CoreError> {
    for t in cores().await {
        let ids = orders(&t, 2).await;
        let file = t
            .core
            .export_documents(
                &admin(),
                batch(DocumentKind::Sticker, vec![ids[0], ids[1], ids[0]]),
            )
            .await?;
        assert_eq!(file.page_count, 2, "{}", t.label);
    }
    Ok(())
}

#[tokio::test]
async fn batch_missing_order_names_it() -> Result<(), CoreError> {
    for t in cores().await {
        let ids = orders(&t, 1).await;
        let res = t
            .core
            .export_documents(
                &admin(),
                batch(DocumentKind::Sticker, vec![ids[0], 999_999]),
            )
            .await;
        assert!(
            matches!(res, Err(CoreError::NotFound { id: 999_999, .. })),
            "{}: {res:?}",
            t.label
        );
    }
    Ok(())
}

/// Наклейка 58 × 40 мм, текст которой не влезает в одну страницу: ровно три страницы на заказ.
fn three_page_sticker() -> mw_domain::document_template::Template {
    use mw_domain::document_template::{Align, Block, BlockBody, CopyFilter};
    let mut template = mw_docs::standard_template(DocumentKind::Sticker);
    template.blocks = vec![Block {
        copies: CopyFilter::Both,
        body: BlockBody::Text {
            text: vec!["слово"; WORDS_FOR_THREE_PAGES].join(" "),
            size_pt: None,
            bold: false,
            align: Align::Left,
            one_line: false,
        },
    }];
    template
}
const WORDS_FOR_THREE_PAGES: usize = 200;

#[tokio::test]
async fn batch_total_page_cap() -> Result<(), CoreError> {
    for t in cores().await {
        let saved = t
            .core
            .save_document_template(
                &admin(),
                DocumentKind::Sticker,
                mw_core::TemplateSave {
                    template: three_page_sticker(),
                    version: 0,
                },
            )
            .await?;
        let _ = saved;
        let ids = orders(&t, 67).await;
        let one = t
            .core
            .export_documents(&admin(), batch(DocumentKind::Sticker, vec![ids[0]]))
            .await?;
        assert_eq!(one.page_count, 3, "{}: шаблон даёт три страницы", t.label);
        // 66 × 3 = 198 страниц проходят, 67 × 3 = 201 — нет.
        let ok = t
            .core
            .export_documents(&admin(), batch(DocumentKind::Sticker, ids[..66].to_vec()))
            .await?;
        assert_eq!(ok.page_count, 198, "{}", t.label);
        assert_validation(
            t.core
                .export_documents(&admin(), batch(DocumentKind::Sticker, ids))
                .await,
            &[("template", "too_many_pages")],
        );
    }
    Ok(())
}

/// Блок своего Typst-кода для обоих экземпляров.
fn code_block(code: &str) -> mw_domain::document_template::Block {
    use mw_domain::document_template::{Block, BlockBody, CopyFilter};
    Block {
        copies: CopyFilter::Both,
        body: BlockBody::TypstCode { code: code.into() },
    }
}

/// Превью образца по черновику `draft`.
fn draft_request(draft: mw_domain::document_template::Template) -> DocumentRequest {
    let kind = draft.kind;
    DocumentRequest {
        sample: true,
        draft: Some(draft),
        ..request(kind, None)
    }
}

async fn png_pages(t: &TestCore, req: DocumentRequest) -> Result<Vec<Vec<u8>>, CoreError> {
    match t
        .core
        .export_document(&admin(), req, DocumentOutput::Png { dpi: 72 })
        .await?
        .bytes
    {
        DocumentBytes::Png(pages) => Ok(pages),
        other => panic!("{}: ждали PNG: {other:?}", t.label),
    }
}

/// Черновик со своим кодом строится и показывается: код печатается на листе.
#[tokio::test]
async fn draft_with_code_previews() -> Result<(), CoreError> {
    for t in code_cores().await {
        let plain = mw_docs::standard_template(DocumentKind::IntakeAct);
        let mut with_code = plain.clone();
        with_code.blocks.push(code_block("КОД-ПРЕВЬЮ"));
        let pages = png_pages(&t, draft_request(with_code)).await?;
        assert!(!pages.is_empty(), "{}", t.label);
        assert_ne!(
            pages,
            png_pages(&t, draft_request(plain)).await?,
            "{}: кода нет на листе",
            t.label
        );
    }
    Ok(())
}

/// Ошибка своего кода в превью — `TemplateCode` с блоком и строкой, без запасного пути.
#[tokio::test]
async fn draft_code_error_names_block_and_line() {
    for t in code_cores().await {
        let mut draft = mw_docs::standard_template(DocumentKind::IntakeAct);
        draft.blocks.insert(2, code_block("#nope"));
        let res = png_pages(&t, draft_request(draft)).await;
        let Err(CoreError::TemplateCode(diagnostics)) = &res else {
            panic!("{}: ждали ошибку кода: {res:?}", t.label);
        };
        let place = diagnostics[0]
            .place
            .as_ref()
            .unwrap_or_else(|| panic!("{}: без места: {diagnostics:?}", t.label));
        assert_eq!(
            (place.block.as_slice(), place.line),
            (&[2][..], 1),
            "{}: {diagnostics:?}",
            t.label
        );
    }
}

/// Без пула и без `with_code_in_process` шаблон со своим кодом не строится (503), без кода — как
/// раньше.
#[tokio::test]
async fn code_without_renderer_is_unavailable() -> Result<(), CoreError> {
    for t in cores().await {
        let plain = mw_docs::standard_template(DocumentKind::IntakeAct);
        let mut with_code = plain.clone();
        with_code.blocks.push(code_block("КОД"));
        let res = png_pages(&t, draft_request(with_code)).await;
        assert!(
            matches!(res, Err(CoreError::Unavailable(_))),
            "{}: {res:?}",
            t.label
        );
        assert!(!png_pages(&t, draft_request(plain)).await?.is_empty());
    }
    Ok(())
}

/// Сохранённый шаблон с битым кодом печатается стандартным: документ и пачка не ломаются. И в ядре,
/// где код строится, и в ядре без рендерера.
#[tokio::test]
async fn saved_broken_code_prints_the_standard() -> Result<(), CoreError> {
    let all = code_cores()
        .await
        .into_iter()
        .map(|t| ("код в процессе", t))
        .chain(cores().await.into_iter().map(|t| ("без рендерера", t)));
    for (how, t) in all {
        let ids = orders(&t, 2).await;
        let sticker = || request(DocumentKind::Sticker, Some(ids[0]));
        let standard = png_pages(&t, sticker()).await?;
        let mut template = mw_docs::standard_template(DocumentKind::Sticker);
        template.blocks.push(code_block("#nope"));
        t.core
            .save_document_template(
                &admin(),
                DocumentKind::Sticker,
                mw_core::TemplateSave {
                    template,
                    version: 0,
                },
            )
            .await?;
        assert_eq!(
            png_pages(&t, sticker()).await?,
            standard,
            "{} {how}: напечатан не стандартный шаблон",
            t.label
        );
        let both = t
            .core
            .export_documents(&admin(), batch(DocumentKind::Sticker, ids.clone()))
            .await?;
        assert_eq!(both.page_count, 2, "{} {how}", t.label);
    }
    Ok(())
}

/// PDF документа или пачки.
fn pdf_of(label: &str, file: mw_core::documents::DocumentFile) -> Vec<u8> {
    match file.bytes {
        DocumentBytes::Pdf(pdf) => pdf,
        other => panic!("{label}: ждали PDF: {other:?}"),
    }
}

/// Сохраняет шаблон наклейки: стандартный и блок своего кода `code` в конце.
async fn save_sticker_with_code(t: &TestCore, code: &str) -> Result<(), CoreError> {
    let mut template = mw_docs::standard_template(DocumentKind::Sticker);
    template.blocks.push(code_block(code));
    t.core
        .save_document_template(
            &admin(),
            DocumentKind::Sticker,
            mw_core::TemplateSave {
                template,
                version: 0,
            },
        )
        .await?;
    Ok(())
}

/// Сохранённый шаблон с рабочим кодом печатается своим кодом, а не стандартным: и документ, и пачка.
#[tokio::test]
async fn saved_working_code_prints_its_code() -> Result<(), CoreError> {
    for t in code_cores().await {
        let ids = orders(&t, 2).await;
        let sticker = || request(DocumentKind::Sticker, Some(ids[0]));
        let both = || batch(DocumentKind::Sticker, ids.clone());
        let standard = png_pages(&t, sticker()).await?;
        let standard_batch = pdf_of(t.label, t.core.export_documents(&admin(), both()).await?);
        // PDF строится одинаково: иначе неравенство ниже ничего не доказывает.
        let again = pdf_of(t.label, t.core.export_documents(&admin(), both()).await?);
        assert_eq!(again, standard_batch, "{}: PDF пачки меняется сам", t.label);

        save_sticker_with_code(&t, "КОД-ПЕЧАТЬ").await?;
        assert_ne!(
            png_pages(&t, sticker()).await?,
            standard,
            "{}: документ напечатан стандартным шаблоном",
            t.label
        );
        let file = t.core.export_documents(&admin(), both()).await?;
        assert_eq!(file.page_count, 2, "{}", t.label);
        assert_ne!(
            pdf_of(t.label, file),
            standard_batch,
            "{}: пачка напечатана стандартным шаблоном",
            t.label
        );
    }
    Ok(())
}

/// Свой код, который компилируется, но не выгружается в PDF (файл приложен дважды — ошибка только
/// typst-pdf; превью в редакторе — PNG, автор её не видит): PDF документа и пачки печатается
/// стандартным шаблоном, а не 500.
#[tokio::test]
async fn saved_code_failing_only_in_pdf_prints_the_standard() -> Result<(), CoreError> {
    for t in code_cores().await {
        let ids = orders(&t, 2).await;
        let sticker = || request(DocumentKind::Sticker, Some(ids[0]));
        let both = || batch(DocumentKind::Sticker, ids.clone());
        let export = || async {
            t.core
                .export_document(&admin(), sticker(), DocumentOutput::Pdf)
                .await
        };
        let standard = pdf_of(t.label, export().await?);
        let standard_batch = pdf_of(t.label, t.core.export_documents(&admin(), both()).await?);

        save_sticker_with_code(
            &t,
            r#"#pdf.attach("a.txt", bytes("1"))#pdf.attach("a.txt", bytes("2"))"#,
        )
        .await?;
        assert_eq!(
            pdf_of(t.label, export().await?),
            standard,
            "{}: документ напечатан не стандартным шаблоном",
            t.label
        );
        assert_eq!(
            pdf_of(t.label, t.core.export_documents(&admin(), both()).await?),
            standard_batch,
            "{}: пачка напечатана не стандартным шаблоном",
            t.label
        );
    }
    Ok(())
}

/// Ссылки фирмы печатаются текстом; пустая настройка — пустое место.
#[tokio::test]
async fn firm_links_print_as_text() -> Result<(), CoreError> {
    use mw_domain::document_template::{Align, Block, BlockBody, CopyFilter};
    for t in cores().await {
        t.core
            .update_settings(
                &admin(),
                SettingsPatch {
                    firm_links: Some({
                        let mut list = mw_domain::links::default_links();
                        list[2].url = "https://t.me/example_service".into();
                        list
                    }),
                    ..Default::default()
                },
            )
            .await?;
        let mut template = mw_docs::standard_template(DocumentKind::IntakeAct);
        template.blocks = vec![Block {
            copies: CopyFilter::Both,
            body: BlockBody::Text {
                text: "TG={фирма.телеграм}; YA={ФИРМА.ЯНДЕКС_КАРТЫ}.".into(),
                size_pt: None,
                bold: false,
                align: Align::Left,
                one_line: false,
            },
        }];
        let order = t.core.create_order(&admin(), sample_new_order()).await?;
        let req = DocumentRequest {
            sample: false,
            order_id: Some(order.id),
            ..draft_request(template)
        };
        let out = text(&t, req).await;
        assert!(
            out.contains("TG=https://t.me/example_service; YA=."),
            "{}: {out:?}",
            t.label
        );
    }
    Ok(())
}

fn link_text(text: &str) -> mw_domain::document_template::Block {
    use mw_domain::document_template::{Align, Block, BlockBody, CopyFilter};
    Block {
        copies: CopyFilter::Both,
        body: BlockBody::Text {
            text: text.into(),
            size_pt: None,
            bold: false,
            align: Align::Left,
            one_line: false,
        },
    }
}

fn link_qr(link: &str) -> mw_domain::document_template::Block {
    use mw_domain::document_template::{Align, Block, BlockBody, CopyFilter};
    Block {
        copies: CopyFilter::Both,
        body: BlockBody::Qr {
            link: link.into(),
            size_mm: 20.0,
            caption: String::new(),
            align: Align::Left,
        },
    }
}

fn links_patch(list: Vec<mw_domain::links::FirmLink>) -> SettingsPatch {
    SettingsPatch {
        firm_links: Some(list),
        ..Default::default()
    }
}

fn firm_link(id: &str, name: &str, url: &str, enabled: bool) -> mw_domain::links::FirmLink {
    mw_domain::links::FirmLink {
        id: id.into(),
        name: name.into(),
        url: url.into(),
        enabled,
    }
}

/// Лист заказа по черновику из `blocks` на месте блоков стандартного акта приёма.
async fn blocks_png(
    t: &TestCore,
    order_id: i64,
    blocks: Vec<mw_domain::document_template::Block>,
) -> Vec<Vec<u8>> {
    let mut template = mw_docs::standard_template(DocumentKind::IntakeAct);
    template.blocks = blocks;
    let req = DocumentRequest {
        sample: false,
        order_id: Some(order_id),
        ..draft_request(template)
    };
    png_pages(t, req).await.unwrap()
}

/// Шаблон, написанный до списка ссылок, печатает те же адреса: QR и текст по старым именам и
/// `firm.*` в своём коде.
#[tokio::test]
async fn legacy_link_substitutions_print_as_before() -> Result<(), CoreError> {
    for t in code_cores().await {
        let mut list = mw_domain::links::default_links();
        list[0].url = "https://yandex.ru/maps/?text=example".into();
        list[2].url = "https://t.me/example_service".into();
        list.push(firm_link(
            "google_maps",
            "Google Карты",
            "https://maps.google.com/?q=example",
            true,
        ));
        t.core.update_settings(&admin(), links_patch(list)).await?;
        let order = t.core.create_order(&admin(), sample_new_order()).await?;
        let qr = || link_qr("{фирма.телеграм}");
        let text = || link_text("{фирма.яндекс_карты}");
        let code = |qr_call: &str| code_block(&format!("#v(\"firm.google_maps\") {qr_call}"));
        let call = "#qr-image(\"firm.telegram\", 20mm)";
        let full = vec![qr(), text(), code(call)];
        let doc = t
            .core
            .render_document(&admin(), {
                let mut template = mw_docs::standard_template(DocumentKind::IntakeAct);
                template.blocks = full.clone();
                DocumentRequest {
                    sample: false,
                    order_id: Some(order.id),
                    ..draft_request(template)
                }
            })
            .await?;
        let out = doc.text();
        assert!(
            out.contains("https://yandex.ru/maps/?text=example"),
            "{}: {out:?}",
            t.label
        );
        assert!(
            out.contains("https://maps.google.com/?q=example"),
            "{}: {out:?}",
            t.label
        );
        let all = blocks_png(&t, order.id, full).await;
        let no_block = blocks_png(&t, order.id, vec![text(), code(call)]).await;
        let no_call = blocks_png(&t, order.id, vec![qr(), text(), code("")]).await;
        assert_ne!(all, no_block, "{}: нет QR-блока", t.label);
        assert_ne!(all, no_call, "{}: нет qr-image", t.label);
    }
    Ok(())
}

/// Переименование и перестановка ссылки шаблонам не мешают: они знают её по `id`.
#[tokio::test]
async fn renamed_link_keeps_printing() -> Result<(), CoreError> {
    for t in cores().await {
        let mut list = mw_domain::links::default_links();
        list[2].url = "https://t.me/example_service".into();
        t.core
            .update_settings(&admin(), links_patch(list.clone()))
            .await?;
        let order = t.core.create_order(&admin(), sample_new_order()).await?;
        let with = || vec![link_qr("{ссылка.telegram}")];
        let before = blocks_png(&t, order.id, with()).await;
        let none = blocks_png(&t, order.id, vec![]).await;
        assert_ne!(before, none, "{}: QR печатается", t.label);
        let mut telegram = list.remove(2);
        telegram.name = "Наш Telegram".into();
        list.push(telegram);
        t.core.update_settings(&admin(), links_patch(list)).await?;
        assert_eq!(
            blocks_png(&t, order.id, with()).await,
            before,
            "{}",
            t.label
        );
    }
    Ok(())
}

/// Выключенная ссылка пуста в документах и в образце; `{ссылка.<нет такой>}` — тоже пусто, а не текст.
#[tokio::test]
async fn disabled_link_is_empty_in_documents() -> Result<(), CoreError> {
    for t in cores().await {
        let mut list = mw_domain::links::default_links();
        list[2].url = "https://t.me/example_service".into();
        t.core
            .update_settings(&admin(), links_patch(list.clone()))
            .await?;
        let order = t.core.create_order(&admin(), sample_new_order()).await?;
        let template = |text: &str| {
            let mut template = mw_docs::standard_template(DocumentKind::IntakeAct);
            template.blocks = vec![link_text(text)];
            template
        };
        let probe = "T={ссылка.telegram}|F={фирма.телеграм}|G={ссылка.link_gone}.";
        let on = DocumentRequest {
            sample: false,
            order_id: Some(order.id),
            ..draft_request(template(probe))
        };
        let out = text(&t, on).await;
        assert!(
            out.contains("T=https://t.me/example_service|F=https://t.me/example_service|G=."),
            "{}: {out:?}",
            t.label
        );
        list[2].enabled = false;
        t.core.update_settings(&admin(), links_patch(list)).await?;
        for sample in [false, true] {
            let req = DocumentRequest {
                sample,
                order_id: (!sample).then_some(order.id),
                ..draft_request(template(probe))
            };
            let out = text(&t, req).await;
            assert!(
                out.contains("T=|F=|G=."),
                "{} sample={sample}: {out:?}",
                t.label
            );
        }
    }
    Ok(())
}

/// Образец: адрес включённой ссылки из настроек, иначе пример; пример свой у каждой ссылки.
#[tokio::test]
async fn sample_links_come_from_settings_or_examples() -> Result<(), CoreError> {
    for t in cores().await {
        let mut list = mw_domain::links::default_links();
        list[0].url = "https://yandex.ru/maps/?text=mine".into();
        t.core.update_settings(&admin(), links_patch(list)).await?;
        let mut template = mw_docs::standard_template(DocumentKind::IntakeAct);
        template.blocks = vec![link_text(
            "Y={ссылка.yandex_maps}|T={ссылка.telegram}|C={ссылка.link_own}",
        )];
        let out = text(&t, draft_request(template)).await;
        assert!(
            out.contains("Y=https://yandex.ru/maps/?text=mine|T=https://t.me/example_service|C="),
            "{}: {out:?}",
            t.label
        );
    }
    Ok(())
}

/// Акт приёма, где каждый экземпляр на своём листе: документ из двух страниц без своего кода.
fn sheet_per_copy_act() -> mw_domain::document_template::Template {
    let mut template = mw_docs::standard_template(DocumentKind::IntakeAct);
    template.layout = mw_domain::document_template::Layout::SheetPerCopy;
    template
}

fn pages(text: &str) -> Option<PageSelection> {
    Some(PageSelection::parse(text).unwrap())
}

/// Выбор страниц печатает только их: в превью черновика и в акте заказа по сохранённому шаблону,
/// в PNG и в PDF.
#[tokio::test]
async fn a_page_selection_prints_only_those_pages() -> Result<(), CoreError> {
    for t in cores().await {
        let all = png_pages(&t, draft_request(sheet_per_copy_act())).await?;
        assert_eq!(all.len(), 2, "{}", t.label);
        assert_ne!(all[0], all[1], "{}: экземпляры различимы", t.label);
        let second = DocumentRequest {
            pages: pages("2"),
            ..draft_request(sheet_per_copy_act())
        };
        assert_eq!(
            png_pages(&t, second.clone()).await?,
            all[1..],
            "{}",
            t.label
        );
        let pdf = t
            .core
            .export_document(&admin(), second, DocumentOutput::Pdf)
            .await?;
        assert_eq!(pdf.page_count, 1, "{}", t.label);

        // Печать заказа: сохранённый шаблон, без черновика.
        t.core
            .save_document_template(
                &admin(),
                DocumentKind::IntakeAct,
                mw_core::TemplateSave {
                    template: sheet_per_copy_act(),
                    version: 0,
                },
            )
            .await?;
        let order = t.core.create_order(&admin(), sample_new_order()).await?;
        let act = request(DocumentKind::IntakeAct, Some(order.id));
        let whole = png_pages(&t, act.clone()).await?;
        assert_eq!(whole.len(), 2, "{}", t.label);
        let first = DocumentRequest {
            pages: pages("1"),
            ..act
        };
        assert_eq!(
            png_pages(&t, first.clone()).await?,
            whole[..1],
            "{}",
            t.label
        );
        let pdf = t
            .core
            .export_document(&admin(), first, DocumentOutput::Pdf)
            .await?;
        assert_eq!(pdf.page_count, 1, "{}", t.label);
        assert!(pdf_of(t.label, pdf).starts_with(b"%PDF-"));
    }
    Ok(())
}

/// Страница за концом документа — ошибка поля `pages`, а не пустой или неполный файл.
#[tokio::test]
async fn pages_beyond_the_document_are_refused() -> Result<(), CoreError> {
    for t in cores().await {
        let order = t.core.create_order(&admin(), sample_new_order()).await?;
        let act = request(DocumentKind::IntakeAct, Some(order.id));
        let whole = png_pages(&t, act.clone()).await?;
        assert_eq!(whole.len(), 1, "{}: акт в одну страницу", t.label);
        for (selection, output) in [
            ("5", DocumentOutput::Pdf),
            ("5", DocumentOutput::Png { dpi: 72 }),
            ("1-2", DocumentOutput::Pdf),
        ] {
            let req = DocumentRequest {
                pages: pages(selection),
                ..act.clone()
            };
            assert_validation(
                t.core.export_document(&admin(), req, output).await,
                &[("pages", "out_of_range")],
            );
        }
    }
    Ok(())
}
