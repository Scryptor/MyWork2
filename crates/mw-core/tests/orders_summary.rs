//! Сводка по заказам и подсказки значений.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use common::*;
use mw_core::{CoreError, OrderPatch, OrderSummary, OrderValueField};

fn count_of(s: &OrderSummary, status_id: i64) -> u64 {
    s.by_status
        .iter()
        .find(|c| c.status_id == status_id)
        .map_or(0, |c| c.count)
}

#[tokio::test]
async fn summary_counts_statuses_and_attention_in_service_only() -> Result<(), CoreError> {
    for t in cores().await {
        let diagnostics = status_id_by_name(&t.core, "Диагностика").await;
        let ready = status_id_by_name(&t.core, "Готов").await;
        let issued = status_id_by_name(&t.core, "Выдан").await;
        let start = t.core.now_utc();
        let day = chrono::Duration::days(1);

        t.clock.set(start - day * 60);
        let _d1 = create_order_with(&t, |n| n.need_call = true).await.id;
        let _d2 = create_order_with(&t, |_| {}).await.id;
        let r = create_order_with(&t, |n| n.need_parts = true).await.id;
        let i = create_order_with(&t, |n| n.need_call = true).await.id;
        let gone = create_order_with(&t, |n| n.need_call = true).await.id;
        t.core.change_status(&admin(), r, 1, ready).await?;
        t.core
            .update_order(
                &admin(),
                i,
                1,
                OrderPatch {
                    master: Some("Мастер".into()),
                    work_done: Some("Работа".into()),
                    final_price: Some(Some(mw_domain::money::Money::from_kopecks(1_000))),
                    ..Default::default()
                },
            )
            .await?;
        t.core.issue_order(&admin(), i, 2).await?;
        t.core.delete_order(&admin(), gone, 1).await?;
        t.clock.set(start);

        let s = t.core.order_summary(&admin(), None).await?;
        assert_eq!(count_of(&s, diagnostics), 2, "{}", t.label);
        assert_eq!(count_of(&s, ready), 1);
        assert_eq!(count_of(&s, issued), 1);
        assert!(s.by_status.iter().all(|c| c.count > 0));
        // Звонок нужен одному заказу в сервисе (выданный и удалённый не считаются).
        assert_eq!(
            (
                s.need_call,
                s.need_parts,
                s.overdue_diagnostics,
                s.stale_ready
            ),
            (1, 1, 2, 1),
            "{}",
            t.label
        );

        let point_b = t.core.create_service_point(&admin(), "Вторая").await?.id;
        let other = t.core.order_summary(&admin(), Some(point_b)).await?;
        assert!(other.by_status.is_empty(), "{}", t.label);
        assert_eq!(other.need_call, 0);
    }
    Ok(())
}

#[tokio::test]
async fn values_are_distinct_by_key_most_frequent_first() -> Result<(), CoreError> {
    for t in cores().await {
        for (brand, model) in [
            ("Samsung", "A52"),
            ("Samsung", "A53"),
            ("samsung", "A52"),
            ("Apple", "A5"),
        ] {
            create_order_with(&t, |n| {
                n.brand = brand.into();
                n.model = model.into();
            })
            .await;
        }
        let v = |field, q: &str, brand: Option<&str>| {
            let t = &t;
            let q = q.to_owned();
            let brand = brand.map(str::to_owned);
            async move {
                t.core
                    .order_values(&admin(), field, &q, brand.as_deref())
                    .await
                    .unwrap()
            }
        };
        assert_eq!(
            v(OrderValueField::Brand, "", None).await,
            vec!["Samsung", "Apple"],
            "{}",
            t.label
        );
        assert_eq!(v(OrderValueField::Brand, "APP", None).await, vec!["Apple"]);
        assert_eq!(
            v(OrderValueField::Model, "a5", Some("SAMSUNG")).await,
            vec!["A52", "A53"]
        );
        assert_eq!(
            v(OrderValueField::Brand, "%", None).await,
            Vec::<String>::new()
        );
    }
    Ok(())
}

/// Подсказок не больше 20: самые частые, при равенстве — по ключу.
#[tokio::test]
async fn values_are_limited_to_the_most_frequent_twenty() -> Result<(), CoreError> {
    for t in cores().await {
        // Бренд `bNN` встречается (NN % 5 + 1) раз; всего 25 брендов.
        for i in 0..25usize {
            for _ in 0..(i % 5 + 1) {
                create_order_with(&t, |n| n.brand = format!("b{i:02}")).await;
            }
        }
        let got = t
            .core
            .order_values(&admin(), OrderValueField::Brand, "", None)
            .await?;
        let mut expected: Vec<(usize, String)> =
            (0..25).map(|i| (i % 5 + 1, format!("b{i:02}"))).collect();
        expected.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        let expected: Vec<String> = expected.into_iter().take(20).map(|e| e.1).collect();
        assert_eq!(got, expected, "{}", t.label);
    }
    Ok(())
}
