//! Платежи по заказу: список, приём, удаление, журнал, версия заказа, повтор запроса, права.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use chrono::{Duration, NaiveDate};
use common::{
    TestCore, actor_with, admin, assert_validation, cores, count_audit, insert_user_row,
    last_audit, role_id_by_name, sample_new_order, status_id_by_kind, status_id_by_name,
};
use mw_core::{
    Actor, CoreError, IssueRequest, NewOrder, NewOrderPayment, NewStatus, NewWarrantyCase, Order,
    OrderPatch, SettingsPatch,
};
use mw_db::entity::order_payment;
use mw_domain::money::Money;
use mw_domain::payment::{MAX_PAYMENT, PaymentStage};
use mw_domain::permission::{Permission, PermissionSet};
use mw_domain::status::StatusKind;
use sea_orm::{EntityTrait, PaginatorTrait};

/// Пользователь «Петрова» с полными правами; запись в журнал ссылается на настоящего пользователя.
async fn cashier(t: &TestCore) -> Actor {
    let id = insert_user_row(
        &t.core,
        "petrova",
        role_id_by_name(&t.core, "Приёмщик").await,
        true,
    )
    .await;
    Actor::for_tests(Some(id), "Петрова", PermissionSet::all())
}

fn day(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

fn pay(day: NaiveDate, rubles: i64, by: &str, key: &str) -> NewOrderPayment {
    NewOrderPayment {
        day,
        amount: Money::from_rubles(rubles),
        received_by: by.into(),
        request_key: key.into(),
    }
}

async fn order(t: &TestCore) -> Order {
    t.core
        .create_order(&admin(), sample_new_order())
        .await
        .unwrap()
}

async fn rows(t: &TestCore) -> u64 {
    order_payment::Entity::find()
        .count(t.core.db())
        .await
        .unwrap()
}

#[tokio::test]
async fn payment_is_listed_with_sums_and_journal() -> Result<(), CoreError> {
    for t in cores().await {
        let o = order(&t).await;
        let cashier = cashier(&t).await;
        let r = t
            .core
            .add_order_payment(
                &cashier,
                o.id,
                pay(day(2026, 11, 11), 1500, "Иванов", "k-1"),
            )
            .await?;
        assert_eq!(r.payments.len(), 1, "{}", t.label);
        let p = &r.payments[0];
        assert_eq!(p.received_by, "Иванов");
        assert_eq!(p.stage, PaymentStage::BeforeIssue);
        assert_eq!(p.added_by_name, "Петрова");
        assert_eq!(p.added_at, t.core.now());
        assert!(!p.legacy);
        assert_eq!(r.paid, Money::from_rubles(1500));
        assert_eq!(r.prepayment, Money::from_rubles(1500));
        assert_eq!(
            t.core.get_order(&admin(), o.id).await?.prepayment,
            Money::from_rubles(1500)
        );
        let a = last_audit(&t.core, o.id).await;
        assert_eq!(a.action, "order.payment_added");
        assert_eq!(
            a.summary,
            "Принята оплата 1\u{a0}500\u{a0}₽ (11.11.2026) · Иванов"
        );
        assert_eq!(a.details.unwrap()["amount"], 150_000);
        assert_eq!(t.core.list_order_payments(&admin(), o.id).await?, r);
    }
    Ok(())
}

#[tokio::test]
async fn payment_leaves_the_order_version() -> Result<(), CoreError> {
    for t in cores().await {
        let o = order(&t).await;
        let cashier = cashier(&t).await;
        t.clock.advance(Duration::hours(1));
        let r = t
            .core
            .add_order_payment(&cashier, o.id, pay(day(2026, 11, 11), 500, "", "k-1"))
            .await?;
        let after_add = t.core.get_order(&admin(), o.id).await?;
        assert_eq!(
            (after_add.version, after_add.updated_at),
            (o.version, o.updated_at),
            "{}",
            t.label
        );
        t.core
            .remove_order_payment(&cashier, o.id, r.payments[0].id)
            .await?;
        let after_remove = t.core.get_order(&admin(), o.id).await?;
        assert_eq!(
            (after_remove.version, after_remove.updated_at),
            (o.version, o.updated_at)
        );
        t.core
            .update_order(
                &admin(),
                o.id,
                o.version,
                OrderPatch {
                    work_done: Some("Заменён экран".into()),
                    ..Default::default()
                },
            )
            .await?;
    }
    Ok(())
}

#[tokio::test]
async fn payments_come_by_day_then_id() -> Result<(), CoreError> {
    for t in cores().await {
        let o = order(&t).await;
        let cashier = cashier(&t).await;
        t.clock.advance(Duration::days(2));
        for (d, key) in [(13, "a"), (12, "b"), (12, "c")] {
            t.core
                .add_order_payment(&cashier, o.id, pay(day(2026, 11, d), 100, key, key))
                .await?;
        }
        let r = t.core.list_order_payments(&admin(), o.id).await?;
        let got: Vec<(NaiveDate, &str)> = r
            .payments
            .iter()
            .map(|p| (p.day, p.received_by.as_str()))
            .collect();
        assert_eq!(
            got,
            vec![
                (day(2026, 11, 12), "b"),
                (day(2026, 11, 12), "c"),
                (day(2026, 11, 13), "a")
            ],
            "{}",
            t.label
        );
        assert!(r.payments[0].id < r.payments[1].id);
    }
    Ok(())
}

#[tokio::test]
async fn day_must_be_between_intake_and_today() -> Result<(), CoreError> {
    for t in cores().await {
        let o = order(&t).await;
        let cashier = cashier(&t).await;
        let before = count_audit(&t.core, "order.payment_added").await;
        for d in [day(2026, 11, 10), day(2026, 11, 12)] {
            assert_validation(
                t.core
                    .add_order_payment(&cashier, o.id, pay(d, 100, "", "k-1"))
                    .await,
                &[("day", "payment_day")],
            );
        }
        assert_eq!(rows(&t).await, 0, "{}", t.label);
        assert_eq!(count_audit(&t.core, "order.payment_added").await, before);
    }
    Ok(())
}

#[tokio::test]
async fn amount_received_by_and_key_are_checked() -> Result<(), CoreError> {
    for t in cores().await {
        let o = order(&t).await;
        let cashier = cashier(&t).await;
        let d = day(2026, 11, 11);
        for kopecks in [0, -1, MAX_PAYMENT.kopecks() + 1] {
            let mut p = pay(d, 1, "", "k-1");
            p.amount = Money::from_kopecks(kopecks);
            assert_validation(
                t.core.add_order_payment(&cashier, o.id, p).await,
                &[("amount", "out_of_range")],
            );
        }
        let mut max = pay(d, 1, "", "k-max");
        max.amount = MAX_PAYMENT;
        t.core.add_order_payment(&cashier, o.id, max).await?;
        assert_validation(
            t.core
                .add_order_payment(&cashier, o.id, pay(d, 1, &"я".repeat(101), "k-2"))
                .await,
            &[("received_by", "too_long")],
        );
        t.core
            .add_order_payment(&cashier, o.id, pay(d, 1, &"я".repeat(100), "k-3"))
            .await?;
        let r = t
            .core
            .add_order_payment(&cashier, o.id, pay(d, 1, " Иванов ", "k-4"))
            .await?;
        assert!(r.payments.iter().any(|p| p.received_by == "Иванов"));
        for key in ["", "a b"] {
            assert_validation(
                t.core
                    .add_order_payment(&cashier, o.id, pay(d, 1, "", key))
                    .await,
                &[("request_key", "invalid")],
            );
        }
        assert_eq!(rows(&t).await, 3, "{}", t.label);
    }
    Ok(())
}

#[tokio::test]
async fn same_request_key_adds_once() -> Result<(), CoreError> {
    for t in cores().await {
        let o = order(&t).await;
        let cashier = cashier(&t).await;
        let d = day(2026, 11, 11);
        let first = t
            .core
            .add_order_payment(&cashier, o.id, pay(d, 300, "Иванов", "same"))
            .await?;
        let again = t
            .core
            .add_order_payment(&cashier, o.id, pay(d, 300, "Иванов", "same"))
            .await?;
        assert_eq!(first, again, "{}", t.label);
        assert_eq!(rows(&t).await, 1);
        assert_eq!(count_audit(&t.core, "order.payment_added").await, 1);
        assert_validation(
            t.core
                .add_order_payment(&cashier, o.id, pay(d, 400, "Иванов", "same"))
                .await,
            &[("request_key", "duplicate")],
        );
        let other_id = insert_user_row(
            &t.core,
            "drugoy",
            role_id_by_name(&t.core, "Приёмщик").await,
            true,
        )
        .await;
        let other = Actor::for_tests(Some(other_id), "Другой", PermissionSet::all());
        assert_validation(
            t.core
                .add_order_payment(&other, o.id, pay(d, 300, "Иванов", "same"))
                .await,
            &[("request_key", "duplicate")],
        );
        assert_eq!(rows(&t).await, 1);
    }
    Ok(())
}

#[tokio::test]
async fn removing_recounts_and_journals() -> Result<(), CoreError> {
    for t in cores().await {
        let o = order(&t).await;
        let cashier = cashier(&t).await;
        let other = order(&t).await;
        let d = day(2026, 11, 11);
        let first = t
            .core
            .add_order_payment(&cashier, o.id, pay(d, 500, "", "a"))
            .await?;
        t.core
            .add_order_payment(&cashier, o.id, pay(d, 700, "", "b"))
            .await?;
        let foreign = t
            .core
            .add_order_payment(&cashier, other.id, pay(d, 10, "", "c"))
            .await?;
        let id = first.payments[0].id;
        let r = t.core.remove_order_payment(&cashier, o.id, id).await?;
        assert_eq!(r.paid, Money::from_rubles(700), "{}", t.label);
        assert_eq!(r.prepayment, Money::from_rubles(700));
        assert_eq!(
            t.core.get_order(&admin(), o.id).await?.prepayment,
            Money::from_rubles(700)
        );
        let a = last_audit(&t.core, o.id).await;
        assert_eq!(a.action, "order.payment_removed");
        assert_eq!(a.summary, "Удалён платёж 500\u{a0}₽ (11.11.2026)");
        assert!(matches!(
            t.core.remove_order_payment(&cashier, o.id, id).await,
            Err(CoreError::NotFound { .. })
        ));
        assert!(matches!(
            t.core
                .remove_order_payment(&cashier, o.id, foreign.payments[0].id)
                .await,
            Err(CoreError::NotFound { .. })
        ));
    }
    Ok(())
}

#[tokio::test]
async fn a_payment_of_an_issued_order_comes_after_issue() -> Result<(), CoreError> {
    for t in cores().await {
        t.core
            .update_settings(
                &admin(),
                SettingsPatch {
                    issue_required: Some(vec![]),
                    ..Default::default()
                },
            )
            .await?;
        let o = order(&t).await;
        let cashier = cashier(&t).await;
        t.core.issue_order(&admin(), o.id, o.version).await?;
        let r = t
            .core
            .add_order_payment(&cashier, o.id, pay(day(2026, 11, 11), 300, "", "k"))
            .await?;
        assert_eq!(r.payments[0].stage, PaymentStage::AfterIssue, "{}", t.label);
        assert_eq!(r.paid, Money::from_rubles(300));
        assert_eq!(r.prepayment, Money::ZERO);
    }
    Ok(())
}

#[tokio::test]
async fn deleted_order_payments_are_read_only() -> Result<(), CoreError> {
    for t in cores().await {
        let o = order(&t).await;
        let cashier = cashier(&t).await;
        let d = day(2026, 11, 11);
        let r = t
            .core
            .add_order_payment(&cashier, o.id, pay(d, 100, "", "a"))
            .await?;
        t.core.delete_order(&admin(), o.id, o.version).await?;
        assert_eq!(t.core.list_order_payments(&admin(), o.id).await?, r);
        assert_validation(
            t.core
                .add_order_payment(&cashier, o.id, pay(d, 100, "", "b"))
                .await,
            &[("id", "deleted")],
        );
        assert_validation(
            t.core
                .remove_order_payment(&cashier, o.id, r.payments[0].id)
                .await,
            &[("id", "deleted")],
        );
    }
    Ok(())
}

#[tokio::test]
async fn rights() -> Result<(), CoreError> {
    for t in cores().await {
        let o = order(&t).await;
        let cashier = cashier(&t).await;
        let d = day(2026, 11, 11);
        let created = actor_with(&[Permission::OrdersCreate]);
        let r = t
            .core
            .add_order_payment(&cashier, o.id, pay(d, 100, "", "a"))
            .await?;
        assert!(matches!(
            t.core
                .add_order_payment(&created, o.id, pay(d, 100, "", "b"))
                .await,
            Err(CoreError::Forbidden(Permission::OrdersEdit))
        ));
        assert!(matches!(
            t.core
                .remove_order_payment(&created, o.id, r.payments[0].id)
                .await,
            Err(CoreError::Forbidden(Permission::OrdersEdit))
        ));
        assert_eq!(
            t.core
                .list_order_payments(&actor_with(&[]), o.id)
                .await?
                .payments
                .len(),
            1
        );
        assert!(matches!(
            t.core.list_order_payments(&admin(), 999_999).await,
            Err(CoreError::NotFound { .. })
        ));
    }
    Ok(())
}

#[tokio::test]
async fn open_mode_leaves_received_by_empty() -> Result<(), CoreError> {
    for t in cores().await {
        let created = t
            .core
            .create_order(
                &Actor::unrestricted("Без входа"),
                NewOrder {
                    prepayment: Money::from_rubles(100),
                    ..sample_new_order()
                },
            )
            .await?;
        let o = t.core.list_order_payments(&admin(), created.id).await?;
        assert_eq!(o.payments.len(), 1, "{}", t.label);
        assert_eq!(o.payments[0].received_by, "");
        assert_eq!(o.payments[0].added_by_name, "Без входа");
    }
    Ok(())
}

/// Выключает «Обязательно при выдаче».
async fn issuable(t: &TestCore) {
    t.core
        .update_settings(
            &admin(),
            SettingsPatch {
                issue_required: Some(vec![]),
                ..Default::default()
            },
        )
        .await
        .unwrap();
}

/// Заказ с предоплатой 500 и итогом 3 500.
async fn priced(t: &TestCore) -> Order {
    let created = t
        .core
        .create_order(
            &admin(),
            NewOrder {
                prepayment: Money::from_rubles(500),
                ..sample_new_order()
            },
        )
        .await
        .unwrap();
    t.core
        .update_order(
            &admin(),
            created.id,
            created.version,
            OrderPatch {
                final_price: Some(Some(Money::from_rubles(3500))),
                ..Default::default()
            },
        )
        .await
        .unwrap()
}

fn with_payment(rubles: i64) -> IssueRequest {
    IssueRequest {
        payment: Some(Money::from_rubles(rubles)),
        ..Default::default()
    }
}

async fn bought(t: &TestCore) -> i64 {
    t.core
        .create_status(
            &admin(),
            NewStatus {
                name: "Куплен".into(),
                kind: StatusKind::Issued,
                color: "gray".into(),
            },
        )
        .await
        .unwrap()
        .id
}

#[tokio::test]
async fn issue_with_payment_writes_one_at_issue_payment() -> Result<(), CoreError> {
    for t in cores().await {
        issuable(&t).await;
        let o = priced(&t).await;
        let before = count_audit(&t.core, "order.payment_added").await;
        let issued = t
            .core
            .issue_order_with(&admin(), o.id, o.version, with_payment(3000))
            .await?;
        let r = t.core.list_order_payments(&admin(), o.id).await?;
        assert_eq!(r.payments.len(), 2, "{}", t.label);
        assert_eq!(r.payments[0].amount, Money::from_rubles(500));
        assert_eq!(r.payments[0].stage, PaymentStage::BeforeIssue);
        assert_eq!(r.payments[1].amount, Money::from_rubles(3000));
        assert_eq!(r.payments[1].stage, PaymentStage::AtIssue);
        assert_eq!(r.payments[1].day, day(2026, 11, 11));
        assert_eq!(r.paid, Money::from_rubles(3500));
        assert_eq!(r.prepayment, Money::from_rubles(500));
        assert_eq!(issued.prepayment, Money::from_rubles(500));
        let a = last_audit(&t.core, o.id).await;
        assert_eq!(a.action, "order.status_changed", "{}", t.label);
        assert!(
            a.summary.ends_with("; принята оплата 3\u{a0}000\u{a0}₽"),
            "{}",
            a.summary
        );
        assert_eq!(a.details.unwrap()["payment"]["amount"], 300_000);
        assert_eq!(count_audit(&t.core, "order.payment_added").await, before);
    }
    Ok(())
}

#[tokio::test]
async fn issue_refuses_a_wrong_payment_or_status() -> Result<(), CoreError> {
    for t in cores().await {
        issuable(&t).await;
        let o = priced(&t).await;
        assert_validation(
            t.core
                .issue_order_with(&admin(), o.id, o.version, with_payment(0))
                .await,
            &[("payment", "out_of_range")],
        );
        let in_work = status_id_by_kind(&t.core, StatusKind::Ready).await;
        assert_validation(
            t.core
                .issue_order_with(
                    &admin(),
                    o.id,
                    o.version,
                    IssueRequest {
                        status_id: Some(in_work),
                        payment: Some(Money::from_rubles(10)),
                    },
                )
                .await,
            &[("status_id", "invalid")],
        );
        let now = t.core.get_order(&admin(), o.id).await?;
        assert_eq!(now.version, o.version, "{}", t.label);
        assert_eq!(now.status_id, o.status_id);
        assert_eq!(rows_of(&t, o.id).await, 1);
    }
    Ok(())
}

async fn rows_of(t: &TestCore, id: i64) -> u64 {
    t.core
        .list_order_payments(&admin(), id)
        .await
        .unwrap()
        .payments
        .len() as u64
}

#[tokio::test]
async fn issue_to_a_picked_issued_status() -> Result<(), CoreError> {
    for t in cores().await {
        issuable(&t).await;
        let bought = bought(&t).await;
        let o = priced(&t).await;
        let issued = t
            .core
            .issue_order_with(
                &admin(),
                o.id,
                o.version,
                IssueRequest {
                    status_id: Some(bought),
                    payment: Some(Money::from_rubles(3000)),
                },
            )
            .await?;
        assert_eq!(issued.status_id, bought, "{}", t.label);
        let r = t.core.list_order_payments(&admin(), o.id).await?;
        assert_eq!(r.payments[1].stage, PaymentStage::AtIssue);
    }
    Ok(())
}

#[tokio::test]
async fn repeated_issue_takes_no_second_payment() -> Result<(), CoreError> {
    for t in cores().await {
        issuable(&t).await;
        let o = priced(&t).await;
        t.core
            .issue_order_with(&admin(), o.id, o.version, with_payment(3000))
            .await?;
        let again = t
            .core
            .issue_order_with(&admin(), o.id, o.version, with_payment(3000))
            .await?;
        assert_eq!(
            again.status_id,
            status_id_by_kind(&t.core, StatusKind::Issued).await
        );
        let r = t.core.list_order_payments(&admin(), o.id).await?;
        assert_eq!(r.payments.len(), 2, "{}", t.label);
        assert_eq!(r.paid, Money::from_rubles(3500));
    }
    Ok(())
}

#[tokio::test]
async fn reissue_after_return_to_work_counts_every_payment() -> Result<(), CoreError> {
    for t in cores().await {
        issuable(&t).await;
        let o = priced(&t).await;
        let issued = t
            .core
            .issue_order_with(&admin(), o.id, o.version, with_payment(3000))
            .await?;
        let ready = status_id_by_name(&t.core, "Готов").await;
        let back = t
            .core
            .change_status(&admin(), o.id, issued.version, ready)
            .await?;
        let r = t.core.list_order_payments(&admin(), o.id).await?;
        assert!(
            r.payments
                .iter()
                .all(|p| p.stage == PaymentStage::BeforeIssue),
            "{}",
            t.label
        );
        assert_eq!(r.prepayment, Money::from_rubles(3500));
        assert_eq!(back.prepayment, Money::from_rubles(3500));
        let again = t
            .core
            .issue_order_with(&admin(), o.id, back.version, IssueRequest::default())
            .await?;
        let due = again.final_price.unwrap().kopecks()
            - again.prepayment.kopecks()
            - again.discount.kopecks();
        assert_eq!(due, 0, "{}", t.label);

        // Тот же перевод через приём по гарантии.
        let warranty = t
            .core
            .accept_warranty_case(
                &admin(),
                o.id,
                again.version,
                NewWarrantyCase {
                    complaint: "Снова".into(),
                    at: None,
                },
            )
            .await?;
        assert_eq!(warranty.prepayment, Money::from_rubles(3500));
    }
    Ok(())
}

#[tokio::test]
async fn warranty_case_restages_at_issue_payments() -> Result<(), CoreError> {
    for t in cores().await {
        issuable(&t).await;
        let o = priced(&t).await;
        let issued = t
            .core
            .issue_order_with(&admin(), o.id, o.version, with_payment(3000))
            .await?;
        t.core
            .accept_warranty_case(
                &admin(),
                o.id,
                issued.version,
                NewWarrantyCase {
                    complaint: "Снова".into(),
                    at: None,
                },
            )
            .await?;
        let r = t.core.list_order_payments(&admin(), o.id).await?;
        assert!(
            r.payments
                .iter()
                .all(|p| p.stage == PaymentStage::BeforeIssue),
            "{}",
            t.label
        );
        assert_eq!(r.prepayment, Money::from_rubles(3500));
    }
    Ok(())
}

#[tokio::test]
async fn restaging_needs_a_change_of_kind() -> Result<(), CoreError> {
    for t in cores().await {
        issuable(&t).await;
        let bought = bought(&t).await;
        let o = priced(&t).await;
        let issued = t
            .core
            .issue_order_with(&admin(), o.id, o.version, with_payment(3000))
            .await?;
        t.core
            .change_status(&admin(), o.id, issued.version, bought)
            .await?;
        let r = t.core.list_order_payments(&admin(), o.id).await?;
        assert_eq!(r.payments[1].stage, PaymentStage::AtIssue, "{}", t.label);
        assert_eq!(r.prepayment, Money::from_rubles(500));
    }
    Ok(())
}

#[tokio::test]
async fn issue_without_payment_keeps_the_old_summary() -> Result<(), CoreError> {
    for t in cores().await {
        issuable(&t).await;
        let o = priced(&t).await;
        t.core.issue_order(&admin(), o.id, o.version).await?;
        let a = last_audit(&t.core, o.id).await;
        assert_eq!(a.action, "order.status_changed", "{}", t.label);
        assert!(a.summary.starts_with("Статус: «") && !a.summary.contains("оплата"));
        assert_eq!(a.summary.matches('«').count(), 2);
        assert!(a.details.unwrap().get("payment").is_none());
        assert_eq!(rows_of(&t, o.id).await, 1);
    }
    Ok(())
}
