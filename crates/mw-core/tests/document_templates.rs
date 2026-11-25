#![allow(clippy::unwrap_used, clippy::expect_used)]

mod common;

use common::*;
use mw_core::documents::{DocumentOutput, DocumentRequest};
use mw_core::{CoreError, TemplateSave};
use mw_db::entity::document_template;
use mw_docs::{RenderOptions, standard_template};
use mw_domain::document_template::{
    Align, BlockBody, Layout, Margins, PageSize, Template, TemplateFile,
};
use mw_domain::documents::DocumentKind;
use mw_domain::permission::Permission;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};

fn sample(kind: DocumentKind, draft: Option<Template>) -> DocumentRequest {
    DocumentRequest {
        kind,
        order_id: None,
        sample: true,
        options: RenderOptions::default(),
        draft,
        pages: None,
    }
}

/// Стандартный шаблон, у которого первый текстовый блок заменён на `text`.
fn with_text(kind: DocumentKind, text: &str) -> Template {
    let mut template = standard_template(kind);
    let block = template
        .blocks
        .iter_mut()
        .find(|b| matches!(b.body, BlockBody::Text { .. }))
        .expect("в стандартном шаблоне есть текст");
    block.body = BlockBody::Text {
        text: text.to_owned(),
        size_pt: None,
        bold: false,
        align: Align::Left,
        one_line: false,
        frame: false,
        numbered: false,
    };
    template
}

fn save(template: Template, version: i64) -> TemplateSave {
    TemplateSave { template, version }
}

#[tokio::test]
async fn standard_until_saved() -> Result<(), CoreError> {
    for t in cores().await {
        let got = t
            .core
            .get_document_template(&admin(), DocumentKind::IntakeAct)
            .await?;
        assert!(!got.custom, "{}", t.label);
        assert_eq!(got.version, 0);
        assert_eq!(got.id, DocumentKind::IntakeAct.ordinal());
        assert_eq!(got.updated_at, None);
        assert_eq!(got.template, standard_template(DocumentKind::IntakeAct));
        assert_eq!(got.standard, got.template);
    }
    Ok(())
}

#[tokio::test]
async fn save_then_get_and_render_use_it() -> Result<(), CoreError> {
    for t in cores().await {
        let kind = DocumentKind::IntakeAct;
        let custom = with_text(kind, "Проверка шаблона");
        let saved = t
            .core
            .save_document_template(&admin(), kind, save(custom.clone(), 0))
            .await?;
        assert_eq!(saved.version, 1, "{}", t.label);
        assert!(saved.custom);
        assert_eq!(saved.template, custom);
        assert_eq!(saved.standard, standard_template(kind));
        assert!(saved.updated_at.is_some());
        let got = t.core.get_document_template(&admin(), kind).await?;
        assert_eq!(got, saved);
        let rendered = t.core.render_document(&admin(), sample(kind, None)).await?;
        assert!(rendered.text().contains("Проверка шаблона"), "{}", t.label);
        let info = t.core.list_document_templates(&admin()).await?;
        let row = info.iter().find(|i| i.kind == kind).unwrap();
        assert!(row.custom);
        assert_eq!(row.version, 1);
        assert_eq!(count_audit(&t.core, "document_template.updated").await, 1);
        assert_eq!(
            audit_json(&t.core, "document_template.updated").await,
            r#"Изменён шаблон «Акт приёма» {"kind":"intake_act"}"#
        );
    }
    Ok(())
}

#[tokio::test]
async fn stale_version_conflicts() -> Result<(), CoreError> {
    for t in cores().await {
        let kind = DocumentKind::IntakeAct;
        t.core
            .save_document_template(&admin(), kind, save(with_text(kind, "Один"), 0))
            .await?;
        let err = t
            .core
            .save_document_template(&admin(), kind, save(with_text(kind, "Два"), 0))
            .await
            .unwrap_err();
        match err {
            CoreError::Conflict {
                entity,
                id,
                current_version,
            } => {
                assert_eq!(entity, "document_template");
                assert_eq!(id, kind.ordinal());
                assert_eq!(current_version, 1, "{}", t.label);
            }
            other => panic!("{}: {other:?}", t.label),
        }
    }
    Ok(())
}

#[tokio::test]
async fn unchanged_save_is_noop() -> Result<(), CoreError> {
    for t in cores().await {
        let kind = DocumentKind::IntakeAct;
        let custom = with_text(kind, "Один");
        t.core
            .save_document_template(&admin(), kind, save(custom.clone(), 0))
            .await?;
        let again = t
            .core
            .save_document_template(&admin(), kind, save(custom, 0))
            .await?;
        assert_eq!(again.version, 1, "{}", t.label);
        assert_eq!(count_audit(&t.core, "document_template.updated").await, 1);
        // Сброс уже стандартного шаблона тоже ничего не пишет.
        let other = DocumentKind::IssueAct;
        let reset = t
            .core
            .save_document_template(&admin(), other, save(standard_template(other), 0))
            .await?;
        assert_eq!((reset.version, reset.custom), (0, false));
        assert_eq!(count_audit(&t.core, "document_template.updated").await, 1);
    }
    Ok(())
}

#[tokio::test]
async fn saving_the_standard_resets() -> Result<(), CoreError> {
    for t in cores().await {
        let kind = DocumentKind::IntakeAct;
        t.core
            .save_document_template(&admin(), kind, save(with_text(kind, "Один"), 0))
            .await?;
        let reset = t
            .core
            .save_document_template(&admin(), kind, save(standard_template(kind), 1))
            .await?;
        assert!(!reset.custom, "{}", t.label);
        assert_eq!(reset.version, 2);
        assert_eq!(reset.template, standard_template(kind));
        let log = audit_json(&t.core, "document_template.updated").await;
        assert!(
            log.contains("Шаблон «Акт приёма» сброшен к стандартному"),
            "{}: {log}",
            t.label
        );
        assert!(!log.contains("Один"));
    }
    Ok(())
}

#[tokio::test]
async fn needs_settings_manage() -> Result<(), CoreError> {
    for t in cores().await {
        let kind = DocumentKind::IntakeAct;
        let nobody = actor_with(&[]);
        let denied = |r: Result<_, CoreError>| match r {
            Err(CoreError::Forbidden(Permission::SettingsManage)) => {}
            other => panic!("{}: {other:?}", t.label),
        };
        denied(
            t.core
                .save_document_template(&nobody, kind, save(standard_template(kind), 0))
                .await
                .map(|_| ()),
        );
        denied(
            t.core
                .import_document_template(&nobody, kind, serde_json::json!({}))
                .await
                .map(|_| ()),
        );
        t.core.get_document_template(&nobody, kind).await?;
        t.core.list_document_templates(&nobody).await?;
    }
    Ok(())
}

#[tokio::test]
async fn kind_mismatch_and_invalid() {
    for t in cores().await {
        let kind = DocumentKind::IntakeAct;
        let other = standard_template(DocumentKind::IssueAct);
        assert_validation(
            t.core
                .save_document_template(&admin(), kind, save(other, 0))
                .await,
            &[("kind", "mismatch")],
        );
        let mut bad = standard_template(kind);
        bad.page.size = PageSize::A5;
        bad.layout = Layout::TwoPerA4;
        assert_validation(
            t.core
                .save_document_template(&admin(), kind, save(bad, 0))
                .await,
            &[("layout", "requires_a4")],
        );
    }
}

#[tokio::test]
async fn calibration_is_not_editable() -> Result<(), CoreError> {
    for t in cores().await {
        let kind = DocumentKind::Calibration;
        let not_editable = &[("kind", "not_editable")];
        assert_validation(
            t.core.get_document_template(&admin(), kind).await,
            not_editable,
        );
        assert_validation(
            t.core
                .save_document_template(&admin(), kind, save(standard_template(kind), 0))
                .await,
            not_editable,
        );
        assert_validation(
            t.core
                .import_document_template(&admin(), kind, serde_json::json!({}))
                .await,
            not_editable,
        );
        let list = t.core.list_document_templates(&admin()).await?;
        assert_eq!(list.len(), 6, "{}", t.label);
        assert!(list.iter().all(|i| i.kind != kind));
        let order: Vec<_> = list.iter().map(|i| i.kind).collect();
        let expected: Vec<_> = DocumentKind::ALL
            .into_iter()
            .filter(|k| k.editable())
            .collect();
        assert_eq!(order, expected);
        assert!(
            list.iter()
                .all(|i| !i.custom && i.version == 0 && i.updated_at.is_none())
        );
    }
    Ok(())
}

#[tokio::test]
async fn draft_renders_without_saving() -> Result<(), CoreError> {
    for t in cores().await {
        let kind = DocumentKind::IntakeAct;
        let draft = with_text(kind, "Черновик");
        let rendered = t
            .core
            .render_document(&admin(), sample(kind, Some(draft.clone())))
            .await?;
        assert!(rendered.text().contains("Черновик"), "{}", t.label);
        assert!(!t.core.get_document_template(&admin(), kind).await?.custom);
        match t
            .core
            .render_document(&actor_with(&[]), sample(kind, Some(draft.clone())))
            .await
            .map(|_| ())
        {
            Err(CoreError::Forbidden(Permission::SettingsManage)) => {}
            other => panic!("{}: {other:?}", t.label),
        }
        // Черновик другого вида или негодный — ошибка проверки до рендера.
        assert_validation(
            t.core
                .render_document(
                    &admin(),
                    sample(kind, Some(standard_template(DocumentKind::IssueAct))),
                )
                .await
                .map(|_| ()),
            &[("kind", "mismatch")],
        );
        let mut bad = draft;
        bad.page.size = PageSize::A5;
        bad.layout = Layout::TwoPerA4;
        assert_validation(
            t.core
                .render_document(&admin(), sample(kind, Some(bad)))
                .await
                .map(|_| ()),
            &[("layout", "requires_a4")],
        );
        // Калибровка черновиком не печатается.
        assert_validation(
            t.core
                .render_document(
                    &admin(),
                    sample(
                        DocumentKind::Calibration,
                        Some(standard_template(DocumentKind::Calibration)),
                    ),
                )
                .await
                .map(|_| ()),
            &[("kind", "not_editable")],
        );
    }
    Ok(())
}

/// Мелкая своя страница и длинный текст: страниц сильно больше предела.
fn many_pages(kind: DocumentKind) -> Template {
    let mut template = with_text(kind, &"ж ".repeat(5_000));
    template.page.size = PageSize::Custom {
        width_mm: 15.0,
        height_mm: 15.0,
    };
    template.page.margins_mm = Margins {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };
    template.layout = Layout::OnePerPage;
    template.font_size_pt = 5.0;
    template
}

#[tokio::test]
async fn too_many_pages_is_a_validation_error() -> Result<(), CoreError> {
    for t in cores().await {
        let kind = DocumentKind::IntakeAct;
        let req = || sample(kind, Some(many_pages(kind)));
        assert_validation(
            t.core.render_document(&admin(), req()).await.map(|_| ()),
            &[("template", "too_many_pages")],
        );
        assert_validation(
            t.core
                .export_document(&admin(), req(), DocumentOutput::Pdf)
                .await
                .map(|_| ()),
            &[("template", "too_many_pages")],
        );
    }
    Ok(())
}

#[tokio::test]
async fn raster_pixels_are_capped() -> Result<(), CoreError> {
    for t in cores().await {
        let kind = DocumentKind::IntakeAct;
        let mut huge = with_text(kind, "Большая");
        huge.layout = Layout::OnePerPage;
        huge.page.size = PageSize::Custom {
            width_mm: 420.0,
            height_mm: 420.0,
        };
        assert_validation(
            t.core
                .export_document(
                    &admin(),
                    sample(kind, Some(huge.clone())),
                    DocumentOutput::Png { dpi: 600 },
                )
                .await
                .map(|_| ()),
            &[("dpi", "too_many_pixels")],
        );
        // Тот же лист на 300 dpi и A4 на 600 dpi проходят.
        t.core
            .export_document(
                &admin(),
                sample(kind, Some(huge)),
                DocumentOutput::Png { dpi: 100 },
            )
            .await?;
        t.core
            .export_document(
                &admin(),
                sample(kind, None),
                DocumentOutput::Png { dpi: 600 },
            )
            .await?;
    }
    Ok(())
}

#[tokio::test]
async fn unreadable_stored_model_prints_standard() -> Result<(), CoreError> {
    for t in cores().await {
        let kind = DocumentKind::IntakeAct;
        let broken = |model: String| document_template::ActiveModel {
            kind: Set(kind.as_str().to_owned()),
            model: Set(Some(model)),
            version: Set(1),
            updated_at: Set(chrono::NaiveDate::from_ymd_opt(2026, 11, 11)
                .unwrap()
                .and_hms_opt(9, 0, 0)
                .unwrap()),
        };
        broken("{".to_owned()).insert(t.core.db()).await.unwrap();
        let got = t.core.get_document_template(&admin(), kind).await?;
        assert!(!got.custom, "{}", t.label);
        assert_eq!(got.template, standard_template(kind));
        t.core.render_document(&admin(), sample(kind, None)).await?;
        // Разбирается, но не проходит проверку — тоже стандартный.
        let mut bad = standard_template(kind);
        bad.page.size = PageSize::A5;
        bad.layout = Layout::TwoPerA4;
        document_template::Entity::delete_by_id(kind.as_str())
            .exec(t.core.db())
            .await
            .unwrap();
        broken(serde_json::to_string(&bad).unwrap())
            .insert(t.core.db())
            .await
            .unwrap();
        assert!(!t.core.get_document_template(&admin(), kind).await?.custom);
        t.core.render_document(&admin(), sample(kind, None)).await?;
    }
    Ok(())
}

#[tokio::test]
async fn import_replaces_kind() -> Result<(), CoreError> {
    for t in cores().await {
        let file = TemplateFile::new(with_text(DocumentKind::IntakeAct, "Из файла"));
        let value = serde_json::to_value(&file).unwrap();
        let imported = t
            .core
            .import_document_template(&admin(), DocumentKind::IssueAct, value)
            .await?;
        assert_eq!(imported.kind, DocumentKind::IssueAct, "{}", t.label);
        assert!(
            !t.core
                .get_document_template(&admin(), DocumentKind::IssueAct)
                .await?
                .custom
        );
        assert_eq!(count_audit(&t.core, "document_template.updated").await, 0);
        assert_validation(
            t.core
                .import_document_template(
                    &admin(),
                    DocumentKind::IssueAct,
                    serde_json::json!({"format": "нет"}),
                )
                .await,
            &[("format", "invalid")],
        );
    }
    Ok(())
}

#[tokio::test]
async fn fields_are_listed() {
    for t in cores().await {
        let fields = t.core.document_fields();
        assert_eq!(fields.len(), 35 + 4, "{}", t.label);
        assert_eq!(fields[0].placeholder, "заказ.номер");
        assert_eq!(fields[0].key, "order.number");
        assert_eq!(fields[0].label, "Номер заказа");
    }
}

fn first_block(template: &Template, wanted: impl Fn(&BlockBody) -> bool) -> usize {
    template
        .blocks
        .iter()
        .position(|b| wanted(&b.body))
        .expect("в стандартном шаблоне есть такой блок")
}

#[tokio::test]
async fn block_code_of_a_text_block() -> Result<(), CoreError> {
    for t in cores().await {
        let kind = DocumentKind::IntakeAct;
        let template = standard_template(kind);
        let at = first_block(
            &template,
            |b| matches!(b, BlockBody::Text { text, .. } if text.contains('{')),
        );
        let code = t
            .core
            .document_block_code(&admin(), kind, template, vec![at])
            .await?;
        assert!(code.contains("v(\""), "{}: {code}", t.label);
    }
    Ok(())
}

#[tokio::test]
async fn block_code_refuses_a_barcode() {
    for t in cores().await {
        let kind = DocumentKind::Sticker;
        let template = standard_template(kind);
        let at = first_block(&template, |b| matches!(b, BlockBody::Barcode { .. }));
        assert_validation(
            t.core
                .document_block_code(&admin(), kind, template.clone(), vec![at])
                .await,
            &[("path", "code_unavailable")],
        );
        assert_validation(
            t.core
                .document_block_code(&admin(), kind, template, vec![999])
                .await,
            &[("path", "code_unavailable")],
        );
    }
}

#[tokio::test]
async fn block_code_checks_in_order() {
    for t in cores().await {
        let kind = DocumentKind::IntakeAct;
        assert_validation(
            t.core
                .document_block_code(
                    &admin(),
                    DocumentKind::Calibration,
                    standard_template(DocumentKind::Calibration),
                    vec![0],
                )
                .await,
            &[("kind", "not_editable")],
        );
        assert_validation(
            t.core
                .document_block_code(
                    &admin(),
                    kind,
                    standard_template(DocumentKind::IssueAct),
                    vec![0],
                )
                .await,
            &[("kind", "mismatch")],
        );
        let mut bad = standard_template(kind);
        bad.page.size = PageSize::A5;
        bad.layout = Layout::TwoPerA4;
        assert_validation(
            t.core
                .document_block_code(&admin(), kind, bad, vec![0])
                .await,
            &[("layout", "requires_a4")],
        );
    }
}

#[tokio::test]
async fn block_code_needs_settings_manage() {
    for t in cores().await {
        let kind = DocumentKind::IntakeAct;
        let r = t
            .core
            .document_block_code(&actor_with(&[]), kind, standard_template(kind), vec![0])
            .await;
        assert!(
            matches!(r, Err(CoreError::Forbidden(Permission::SettingsManage))),
            "{}: {r:?}",
            t.label
        );
    }
}

#[tokio::test]
async fn fields_have_keys() {
    for t in cores().await {
        let fields = t.core.document_fields();
        let number = fields
            .iter()
            .find(|f| f.placeholder == "заказ.номер")
            .expect("поле «заказ.номер» есть");
        assert_eq!(number.key, "order.number", "{}", t.label);
        assert!(fields.iter().all(|f| !f.key.is_empty()), "{}", t.label);
    }
}

#[tokio::test]
async fn block_code_of_an_empty_path_is_unavailable() {
    for t in cores().await {
        let kind = DocumentKind::IntakeAct;
        assert_validation(
            t.core
                .document_block_code(&admin(), kind, standard_template(kind), vec![])
                .await,
            &[("path", "code_unavailable")],
        );
    }
}

#[tokio::test]
async fn block_code_of_a_text_inside_columns() -> Result<(), CoreError> {
    for t in cores().await {
        let kind = DocumentKind::IntakeAct;
        // Первый блок акта — колонки; в первой колонке второй блок — текст с полем фирмы.
        let template = standard_template(kind);
        let code = t
            .core
            .document_block_code(&admin(), kind, template, vec![0, 0, 1])
            .await?;
        assert!(code.contains("v(\""), "{}: {code}", t.label);
    }
    Ok(())
}

/// После статических полей идут ссылки из настроек по порядку; выключенная помечена.
#[tokio::test]
async fn document_fields_list_the_links() {
    use mw_domain::links::{FirmLink, default_links};
    for t in cores().await {
        let tail = |t: &TestCore| {
            let fields = t.core.document_fields();
            fields[35..].to_vec()
        };
        let got = tail(&t);
        assert_eq!(got.len(), 4, "{}", t.label);
        assert_eq!(
            got[0],
            mw_core::DocumentField {
                key: "link.yandex_maps".into(),
                placeholder: "ссылка.yandex_maps".into(),
                label: "Ссылка: Яндекс Карты".into(),
            },
            "{}",
            t.label
        );
        let mut list = default_links();
        list[2].enabled = false;
        list.push(FirmLink {
            id: "site".into(),
            name: "Сайт".into(),
            url: "https://example.com".into(),
            enabled: true,
        });
        t.core
            .update_settings(
                &admin(),
                mw_core::SettingsPatch {
                    firm_links: Some(list),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let labels: Vec<String> = tail(&t).into_iter().map(|f| f.label).collect();
        assert_eq!(
            labels,
            [
                "Ссылка: Яндекс Карты",
                "Ссылка: 2ГИС",
                "Ссылка: Telegram (выключена)",
                "Ссылка: MAX",
                "Ссылка: Сайт"
            ],
            "{}",
            t.label
        );
        let all = t.core.document_fields();
        assert!(
            all.iter().all(|f| !f.key.starts_with("firm.telegram")),
            "{}",
            t.label
        );
        assert_eq!(all.last().unwrap().key, "link.site", "{}", t.label);
    }
}
