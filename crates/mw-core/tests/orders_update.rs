mod common;

use chrono::Duration;
use common::*;
use mw_core::{CoreError, NewOrder, OrderPatch};
use mw_domain::permission::Permission;
use serde_json::json;

#[tokio::test]
async fn patch_bumps_version_audits_diff_and_masks_password() -> Result<(), CoreError> {
    for t in cores().await {
        let o = t.core.create_order(&admin(), sample_new_order()).await?;
        let u = t
            .core
            .update_order(
                &admin(),
                o.id,
                1,
                OrderPatch {
                    brand: Some("Apple".into()),
                    device_password: Some("1234".into()),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!((u.brand.as_str(), u.version), ("Apple", 2));
        let last = last_audit(&t.core, o.id).await;
        assert_eq!(last.action, "order.updated");
        assert_eq!(last.summary, "Изменено: бренд, пароль устройства");
        assert_eq!(
            last.details.clone().unwrap()["changes"]["brand"],
            json!(["Samsung", "Apple"])
        );
        assert_eq!(
            last.details.unwrap()["changes"]["device_password"],
            json!(["***", "***"])
        );
        assert!(
            !serde_json::to_string(&last_audit(&t.core, o.id).await.details)
                .unwrap()
                .contains("1234")
        );
        assert_eq!(
            t.core
                .update_order(&admin(), o.id, 2, OrderPatch::default())
                .await?
                .version,
            2
        );

        // stale_version_is_conflict_and_first_edit_survives
        let stale = t
            .core
            .update_order(
                &admin(),
                o.id,
                1,
                OrderPatch {
                    model: Some("A53".into()),
                    ..Default::default()
                },
            )
            .await;
        assert!(matches!(
            stale,
            Err(CoreError::Conflict {
                current_version: 2,
                ..
            })
        ));
        assert_eq!(t.core.get_order(&admin(), o.id).await?.model, "A52");

        // dates_need_extra_permission
        let editor = actor_with(&[Permission::OrdersEdit]);
        let patch = OrderPatch {
            received_at: Some(o.received_at - Duration::days(1)),
            ..Default::default()
        };
        assert!(matches!(
            t.core.update_order(&editor, o.id, 2, patch.clone()).await,
            Err(CoreError::Forbidden(Permission::OrderDatesEdit))
        ));
        assert!(t.core.update_order(&admin(), o.id, 2, patch).await.is_ok());
    }
    Ok(())
}

#[tokio::test]
async fn issue_requires_the_default_set_and_keeps_first_issue_date() -> Result<(), CoreError> {
    for t in cores().await {
        let o = t.core.create_order(&admin(), sample_new_order()).await?;
        assert_validation(
            t.core.issue_order(&admin(), o.id, 1).await,
            &[
                ("final_price", "required_for_issue"),
                ("master", "required_for_issue"),
                ("work_done", "required_for_issue"),
            ],
        );
        let o = t
            .core
            .update_order(
                &admin(),
                o.id,
                1,
                OrderPatch {
                    master: Some("Антон".into()),
                    work_done: Some("Замена дисплея".into()),
                    final_price: Some(Some(mw_domain::money::Money::from_kopecks(100_000))),
                    ..Default::default()
                },
            )
            .await?;
        let first = t.core.issue_order(&admin(), o.id, o.version).await?;
        assert_eq!(first.issued_at.unwrap().to_string(), "2026-11-11 09:00:00");
        t.clock.advance(Duration::days(10));
        let warranty = status_id_by_name(&t.core, "Принят по гарантии").await;
        let back = t
            .core
            .change_status(&admin(), o.id, first.version, warranty)
            .await?;
        assert_eq!(back.issued_at, first.issued_at);
        t.clock.advance(Duration::days(2));
        let again = t.core.issue_order(&admin(), o.id, back.version).await?;
        assert_eq!(again.issued_at, first.issued_at);
        assert_eq!(
            last_audit(&t.core, o.id).await.summary,
            "Статус: «Принят по гарантии» → «Выдан»"
        );
        let same = t
            .core
            .change_status(&admin(), o.id, again.version, again.status_id)
            .await?;
        assert_eq!(same.version, again.version);
    }
    Ok(())
}

#[tokio::test]
async fn prepayment_and_clearing_rules() -> Result<(), CoreError> {
    use mw_domain::money::Money;
    for t in cores().await {
        let o = t.core.create_order(&admin(), sample_new_order()).await?;
        let u = t
            .core
            .update_order(
                &admin(),
                o.id,
                1,
                OrderPatch {
                    prepayment: Some(Money::from_rubles(500)),
                    final_price: Some(Some(Money::from_rubles(3000))),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(u.prepaid_at, Some(t.core.now()));
        let u = t
            .core
            .update_order(
                &admin(),
                o.id,
                2,
                OrderPatch {
                    prepayment: Some(Money::from_rubles(0)),
                    final_price: Some(None),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!((u.prepaid_at, u.final_price), (None, None));
        assert_validation(
            t.core
                .update_order(
                    &admin(),
                    o.id,
                    3,
                    OrderPatch {
                        estimate_from: Some(Some(Money::from_rubles(9))),
                        estimate_to: Some(Some(Money::from_rubles(1))),
                        client_id: Some(9999),
                        ..Default::default()
                    },
                )
                .await,
            &[("client_id", "invalid"), ("estimate_to", "estimate_range")],
        );
    }
    Ok(())
}

#[tokio::test]
async fn delete_is_soft_and_blocks_edits() -> Result<(), CoreError> {
    for t in cores().await {
        let o = t.core.create_order(&admin(), sample_new_order()).await?;
        assert!(matches!(
            t.core
                .delete_order(&actor_with(&[Permission::OrdersEdit]), o.id, 1)
                .await,
            Err(CoreError::Forbidden(Permission::OrdersDelete))
        ));
        t.core.delete_order(&admin(), o.id, 1).await?;
        assert!(t.core.get_order(&admin(), o.id).await?.deleted_at.is_some());
        assert_eq!(last_audit(&t.core, o.id).await.action, "order.deleted");
        assert_validation(
            t.core
                .update_order(
                    &admin(),
                    o.id,
                    2,
                    OrderPatch {
                        model: Some("X".into()),
                        ..Default::default()
                    },
                )
                .await,
            &[("id", "deleted")],
        );
        assert_validation(
            t.core.issue_order(&admin(), o.id, 2).await,
            &[("id", "deleted")],
        );
    }
    Ok(())
}

#[tokio::test]
async fn negative_money_is_rejected_on_update() -> Result<(), CoreError> {
    use mw_domain::money::Money;
    for t in cores().await {
        let o = t.core.create_order(&admin(), sample_new_order()).await?;
        let a = admin();
        let patch = |p: OrderPatch| t.core.update_order(&a, o.id, 1, p);
        assert_validation(
            patch(OrderPatch {
                discount: Some(Money::from_kopecks(-1)),
                ..Default::default()
            })
            .await,
            &[("discount", "out_of_range")],
        );
        assert_validation(
            patch(OrderPatch {
                extra_costs: Some(Money::from_kopecks(-1)),
                final_price: Some(Some(Money::from_kopecks(-5))),
                ..Default::default()
            })
            .await,
            &[
                ("extra_costs", "out_of_range"),
                ("final_price", "out_of_range"),
            ],
        );
    }
    Ok(())
}

#[tokio::test]
async fn caller_supplied_dates_are_truncated_to_whole_seconds() -> Result<(), CoreError> {
    use chrono::{NaiveDate, Timelike};
    for t in cores().await {
        let o = t.core.create_order(&admin(), sample_new_order()).await?;
        let at = NaiveDate::from_ymd_opt(2026, 3, 4)
            .unwrap()
            .and_hms_nano_opt(10, 11, 12, 123_456_789)
            .unwrap();
        let u = t
            .core
            .update_order(
                &admin(),
                o.id,
                1,
                OrderPatch {
                    received_at: Some(at),
                    issued_at: Some(Some(at)),
                    ..Default::default()
                },
            )
            .await?;
        let read = t.core.get_order(&admin(), o.id).await?;
        for v in [
            u.received_at,
            u.issued_at.unwrap(),
            read.received_at,
            read.issued_at.unwrap(),
        ] {
            assert_eq!(v.nanosecond(), 0, "{}", t.label);
            assert_eq!(v.second(), 12);
        }
    }
    Ok(())
}

#[tokio::test]
async fn warranty_text_is_kept_and_term_is_derived() -> Result<(), CoreError> {
    use mw_domain::warranty::WarrantyTerm;
    for t in cores().await {
        let o = t
            .core
            .create_order(
                &admin(),
                NewOrder {
                    warranty: " 3 мес ".into(),
                    ..sample_new_order()
                },
            )
            .await?;
        assert_eq!(
            (o.warranty.as_str(), o.warranty_term),
            ("3 мес", Some(WarrantyTerm::months(3))),
            "{}",
            t.label
        );
        let o = t
            .core
            .update_order(
                &admin(),
                o.id,
                o.version,
                OrderPatch {
                    warranty: Some("Без гарантии".into()),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(o.warranty_term, Some(WarrantyTerm::days(0)));
        assert_eq!(
            last_audit(&t.core, o.id).await.details.unwrap()["changes"]["warranty"],
            json!(["3 мес", "Без гарантии"])
        );
        let o = t
            .core
            .update_order(
                &admin(),
                o.id,
                o.version,
                OrderPatch {
                    warranty: Some("30 дней — чистка, 6 мес блок питания".into()),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(o.warranty_term, None);
        assert_eq!(
            t.core.get_order(&admin(), o.id).await?.warranty,
            "30 дней — чистка, 6 мес блок питания"
        );
        // Та же запись другими пробелами — не правка.
        let same = t
            .core
            .update_order(
                &admin(),
                o.id,
                o.version,
                OrderPatch {
                    warranty: Some(format!(" {} ", o.warranty)),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(same.version, o.version);
    }
    Ok(())
}

#[tokio::test]
async fn status_change_can_skip_the_issue_date() -> Result<(), CoreError> {
    for t in cores().await {
        let o = t.core.create_order(&admin(), sample_new_order()).await?;
        let o = t
            .core
            .update_order(
                &admin(),
                o.id,
                1,
                OrderPatch {
                    master: Some("Антон".into()),
                    work_done: Some("Замена дисплея".into()),
                    final_price: Some(Some(mw_domain::money::Money::from_kopecks(100_000))),
                    ..Default::default()
                },
            )
            .await?;
        t.clock.advance(Duration::hours(2));
        let issued = status_id_by_kind(&t.core, mw_domain::status::StatusKind::Issued).await;
        let done = t
            .core
            .change_status_with(&admin(), o.id, o.version, issued, false)
            .await?;
        assert_eq!(done.status_id, issued, "{}", t.label);
        assert_eq!(done.issued_at, None, "{}", t.label);
        assert_eq!(
            status_changed_at(&t.core, done.id).await,
            Some("2026-11-11 11:00:00".to_owned()),
            "{}",
            t.label
        );
    }
    Ok(())
}

/// Заказ в статусе «Готов» с заданными полями; цена, мастер, работы и гарантия — по флажкам.
async fn order_with(
    t: &TestCore,
    price: bool,
    master: bool,
    work: bool,
    warranty: bool,
) -> Result<mw_core::Order, CoreError> {
    use mw_domain::money::Money;
    let o = t.core.create_order(&admin(), sample_new_order()).await?;
    t.core
        .update_order(
            &admin(),
            o.id,
            o.version,
            OrderPatch {
                final_price: price.then(|| Some(Money::from_kopecks(50_000))),
                master: master.then(|| "Антон".to_owned()),
                work_done: work.then(|| "Работа".to_owned()),
                warranty: Some(if warranty { "30 дней" } else { " " }.to_owned()),
                ..Default::default()
            },
        )
        .await
}

fn set_required(list: &[mw_domain::issue::IssueRequirement]) -> mw_core::SettingsPatch {
    mw_core::SettingsPatch {
        issue_required: Some(list.to_vec()),
        ..Default::default()
    }
}

#[tokio::test]
async fn default_set_blocks_missing_price_master_and_work() -> Result<(), CoreError> {
    use mw_domain::issue::IssueRequirement as R;
    for t in cores().await {
        assert_eq!(
            t.core.settings().issue_required,
            vec![R::FinalPrice, R::Master, R::WorkDone]
        );
        let o = order_with(&t, false, false, false, false).await?;
        let blocked = [
            ("final_price", "required_for_issue"),
            ("master", "required_for_issue"),
            ("work_done", "required_for_issue"),
        ];
        assert_validation(
            t.core.issue_order(&admin(), o.id, o.version).await,
            &blocked,
        );
        let issued = status_id_by_name(&t.core, "Выдан").await;
        assert_validation(
            t.core
                .change_status(&admin(), o.id, o.version, issued)
                .await,
            &blocked,
        );
        // Гарантия не в наборе по умолчанию; не «Выдан» — правило не действует.
        let ready = status_id_by_name(&t.core, "Готов").await;
        t.core
            .change_status(&admin(), o.id, o.version, ready)
            .await?;
        let full = order_with(&t, true, true, true, false).await?;
        assert!(
            t.core
                .issue_order(&admin(), full.id, full.version)
                .await
                .is_ok()
        );
        // Пустое после trim — тоже пусто.
        let ws = t.core.create_order(&admin(), sample_new_order()).await?;
        let ws = t
            .core
            .update_order(
                &admin(),
                ws.id,
                ws.version,
                OrderPatch {
                    master: Some("  ".into()),
                    work_done: Some("Работа".into()),
                    final_price: Some(Some(mw_domain::money::Money::from_kopecks(1))),
                    ..Default::default()
                },
            )
            .await?;
        assert_validation(
            t.core.issue_order(&admin(), ws.id, ws.version).await,
            &[("master", "required_for_issue")],
        );
    }
    Ok(())
}

#[tokio::test]
async fn empty_set_issues_an_empty_order() -> Result<(), CoreError> {
    for t in cores().await {
        t.core.update_settings(&admin(), set_required(&[])).await?;
        let o = order_with(&t, false, false, false, false).await?;
        assert!(t.core.issue_order(&admin(), o.id, o.version).await.is_ok());
    }
    Ok(())
}

#[tokio::test]
async fn ticked_warranty_blocks_an_empty_warranty_and_unticked_price_does_not()
-> Result<(), CoreError> {
    use mw_domain::issue::IssueRequirement as R;
    for t in cores().await {
        t.core
            .update_settings(&admin(), set_required(&[R::Warranty]))
            .await?;
        let o = order_with(&t, false, false, false, false).await?;
        assert_validation(
            t.core.issue_order(&admin(), o.id, o.version).await,
            &[("warranty", "required_for_issue")],
        );
        let ok = order_with(&t, false, false, false, true).await?;
        assert!(
            t.core
                .issue_order(&admin(), ok.id, ok.version)
                .await
                .is_ok()
        );
        // Повторы в наборе отклоняются.
        assert_validation(
            t.core
                .update_settings(&admin(), set_required(&[R::Master, R::Master]))
                .await,
            &[("issue_required", "invalid")],
        );
    }
    Ok(())
}

#[tokio::test]
async fn issued_to_issued_is_checked_like_any_move_into_issued() -> Result<(), CoreError> {
    use mw_domain::issue::IssueRequirement as R;
    for t in cores().await {
        let o = order_with(&t, true, true, true, true).await?;
        let first = t.core.issue_order(&admin(), o.id, o.version).await?;
        // Вторая выдача при потом ужесточённых требованиях: цену стёрли, статус «Выдан» другой.
        t.core
            .update_settings(&admin(), set_required(&[R::FinalPrice, R::Warranty]))
            .await?;
        let cleared = t
            .core
            .update_order(
                &admin(),
                o.id,
                first.version,
                OrderPatch {
                    final_price: Some(None),
                    ..Default::default()
                },
            )
            .await?;
        let other = t
            .core
            .create_status(
                &admin(),
                mw_core::NewStatus {
                    name: "Выдан со склада".into(),
                    kind: mw_domain::status::StatusKind::Issued,
                    color: "gray".into(),
                },
            )
            .await?;
        assert_validation(
            t.core
                .change_status(&admin(), o.id, cleared.version, other.id)
                .await,
            &[("final_price", "required_for_issue")],
        );
    }
    Ok(())
}
