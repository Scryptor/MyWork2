mod common;

use chrono::NaiveDateTime;
use common::*;
use mw_core::{CoreError, IssuedFilter, OrderFilter, OrderSort, OrderSortField, Page};
use mw_domain::permission::Permission;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

#[tokio::test]
async fn search_filters_sorting_paging_barcode_and_numbering() -> Result<(), CoreError> {
    for t in cores().await {
        let Seed {
            a,
            b,
            c,
            d,
            a_barcode,
        } = seed_orders(&t).await;
        let q = |s: &str| OrderFilter {
            query: s.into(),
            ..Default::default()
        };
        assert_eq!(list_ids(&t, q("петр a52")).await, vec![a]);
        assert_eq!(list_ids(&t, q("ИВАНОВ")).await, vec![a]);
        assert_eq!(list_ids(&t, q("8 921 123-45-67")).await, vec![a]);
        assert_eq!(list_ids(&t, q("a52 ким")).await, Vec::<i64>::new()); // слова соединяются через И
        assert_eq!(list_ids(&t, q("%")).await, Vec::<i64>::new());
        assert!(list_ids(&t, q(&a.to_string())).await.contains(&a)); // номер ищется как подстрока, совпасть могут и другие
        assert_eq!(list_ids(&t, q("")).await, vec![c, b, a]); // по дате приёма, новые сверху; D скрыт
        let f = |issued| OrderFilter {
            issued,
            ..Default::default()
        };
        assert_eq!(list_ids(&t, f(IssuedFilter::InService)).await, vec![b, a]);
        assert_eq!(list_ids(&t, f(IssuedFilter::Issued)).await, vec![c]);
        assert_eq!(
            list_ids(
                &t,
                OrderFilter {
                    need_call: Some(true),
                    ..Default::default()
                }
            )
            .await,
            vec![b]
        );
        assert_eq!(
            list_ids(
                &t,
                OrderFilter {
                    overdue_diagnostics: true,
                    ..Default::default()
                }
            )
            .await,
            vec![a]
        );
        let by_name = t
            .core
            .list_orders(
                &admin(),
                &OrderFilter::default(),
                OrderSort {
                    field: OrderSortField::ClientName,
                    descending: false,
                },
                50,
                0,
            )
            .await?;
        assert_eq!(ids(by_name.items), vec![b, c, a]); // анна, олег, петр
        let page = t
            .core
            .list_orders(
                &admin(),
                &OrderFilter::default(),
                OrderSort::default(),
                1,
                1,
            )
            .await?;
        assert_eq!((ids(page.items), page.total), (vec![b], 3));
        assert!(matches!(
            t.core
                .list_orders(
                    &actor_with(&[]),
                    &OrderFilter {
                        include_deleted: true,
                        ..Default::default()
                    },
                    OrderSort::default(),
                    50,
                    0
                )
                .await,
            Err(CoreError::Forbidden(Permission::OrdersDelete))
        ));
        // штрихкод: сканер EAN-13 с ведущим нулём и старые дубли
        assert_eq!(
            ids(t
                .core
                .find_orders_by_barcode(&admin(), &format!("0{}\r\n", a_barcode))
                .await?),
            vec![a]
        );
        set_barcode_directly(&t, b, "123456789012").await;
        set_barcode_directly(&t, c, "123456789012").await;
        assert_eq!(
            t.core
                .find_orders_by_barcode(&admin(), "123456789012")
                .await?
                .len(),
            2
        );
        // следующий номер заказа
        assert_validation(
            t.core.set_next_order_number(&admin(), d).await,
            &[("next", "out_of_range")],
        );
        // верхняя граница: штрихкод покрывает номера до 9_999_999_999
        assert_validation(
            t.core.set_next_order_number(&admin(), 10_000_000_000).await,
            &[("next", "out_of_range")],
        );
        assert_eq!(count_audit(&t.core, "orders.next_number_set").await, 0);
        t.core.set_next_order_number(&admin(), d + 100).await?;
        assert_eq!(
            t.core.create_order(&admin(), sample_new_order()).await?.id,
            d + 100
        );
    }
    Ok(())
}

#[tokio::test]
async fn every_sort_field_is_deterministic_and_deleted_are_opt_in() -> Result<(), CoreError> {
    for t in cores().await {
        let Seed { a, b, c, d, .. } = seed_orders(&t).await;
        let sorted = |field, descending| {
            let t = &t;
            async move {
                let p: Page<_> = t
                    .core
                    .list_orders(
                        &admin(),
                        &OrderFilter::default(),
                        OrderSort { field, descending },
                        50,
                        0,
                    )
                    .await
                    .unwrap();
                ids(p.items)
            }
        };
        use OrderSortField::*;
        assert_eq!(sorted(Id, false).await, vec![a, b, c]);
        assert_eq!(sorted(Id, true).await, vec![c, b, a]);
        assert_eq!(sorted(ReceivedAt, false).await, vec![a, b, c]);
        // выдан только C; пустые даты в конце при любом направлении
        assert_eq!(sorted(IssuedAt, false).await, vec![c, b, a]);
        assert_eq!(sorted(IssuedAt, true).await, vec![c, b, a]);
        assert_eq!(sorted(Brand, false).await, vec![b, a, c]); // Apple, Samsung, Xiaomi
        assert_eq!(sorted(Model, false).await, vec![a, b, c]); // A52, MacBook, Note 10
        assert_eq!(sorted(DeviceType, false).await, vec![b, c, a]); // Ноутбук, Телефон(c), Телефон(a): id DESC
        assert_eq!(sorted(Master, false).await, vec![b, a, c]); // "", "", "Мастер"
        assert_eq!(sorted(Status, false).await, vec![a, b, c]); // Диагностика, Готов, Выдан
        assert_eq!(sorted(FinalPrice, false).await, vec![c, b, a]); // цена только у C; пустые в конце
        assert_eq!(sorted(FinalPrice, true).await, vec![c, b, a]);
        let with_deleted = OrderFilter {
            include_deleted: true,
            ..Default::default()
        };
        let all = t
            .core
            .list_orders(&admin(), &with_deleted, OrderSort::default(), 50, 0)
            .await?;
        assert_eq!(ids(all.items), vec![d, c, b, a]);
        assert_eq!(all.total, 4);
        assert!(matches!(
            t.core
                .list_orders(
                    &admin(),
                    &OrderFilter::default(),
                    OrderSort::default(),
                    0,
                    0
                )
                .await,
            Err(CoreError::Validation(_))
        ));
        assert!(matches!(
            t.core.set_next_order_number(&actor_with(&[]), 10_000).await,
            Err(CoreError::Forbidden(Permission::SettingsManage))
        ));
        assert_eq!(count_audit(&t.core, "orders.next_number_set").await, 0);
    }
    Ok(())
}

#[tokio::test]
async fn nameless_client_keeps_raw_empty_name() -> Result<(), CoreError> {
    for t in cores().await {
        let o = t
            .core
            .create_order(
                &admin(),
                mw_core::NewOrder {
                    client: mw_core::ClientRef::New(mw_core::NewClient {
                        phones: common::phones(&["+7 900 111-22-33"]),
                        ..Default::default()
                    }),
                    ..sample_new_order()
                },
            )
            .await?;
        let items = t
            .core
            .list_orders(
                &admin(),
                &OrderFilter::default(),
                OrderSort::default(),
                50,
                0,
            )
            .await?
            .items;
        let item = items.iter().find(|i| i.id == o.id).unwrap();
        assert_eq!(item.client_name, "");
        assert!(!item.client_phone.is_empty());
    }
    Ok(())
}

#[tokio::test]
async fn huge_offset_is_validation_error_not_panic() {
    for t in cores().await {
        for offset in [u64::MAX, i64::MAX as u64 + 1] {
            assert_validation(
                t.core
                    .list_orders(
                        &admin(),
                        &OrderFilter::default(),
                        OrderSort::default(),
                        50,
                        offset,
                    )
                    .await,
                &[("offset", "out_of_range")],
            );
        }
    }
}

#[tokio::test]
async fn filters_and_sorting_ignore_case() -> Result<(), CoreError> {
    for t in cores().await {
        // Два пользователя с одним именем в разном регистре и третий.
        let (anton_lower, anton, yozhikov) = (
            make_master(&t.core, "антон").await,
            make_master(&t.core, "Антон").await,
            make_master(&t.core, "Ёжиков").await,
        );
        let a = create_order_with(&t, |n| {
            n.brand = "samsung".into();
            n.master_user_id = Some(anton_lower);
        })
        .await
        .id;
        let b = create_order_with(&t, |n| {
            n.brand = "Samsung".into();
            n.master_user_id = Some(anton);
        })
        .await
        .id;
        let c = create_order_with(&t, |n| {
            n.brand = "Apple".into();
            n.master_user_id = Some(yozhikov);
            n.device_type = "ТЕЛЕФОН".into();
        })
        .await
        .id;
        let f = |brand: &str| OrderFilter {
            brand: Some(brand.into()),
            ..Default::default()
        };
        assert_eq!(
            sorted(list_ids(&t, f("SAMSUNG")).await),
            sorted(vec![a, b]),
            "{}",
            t.label
        );
        let by_master = OrderFilter {
            master: Some("АНТОН".into()),
            ..Default::default()
        };
        assert_eq!(sorted(list_ids(&t, by_master).await), sorted(vec![a, b]));
        let by_type = OrderFilter {
            device_type: Some("телефон".into()),
            ..Default::default()
        };
        assert_eq!(sorted(list_ids(&t, by_type).await), sorted(vec![a, b, c]));
        let asc = list_sorted(&t, OrderSortField::Brand, false).await;
        assert_eq!(asc, vec![c, b, a], "{}", t.label); // apple < samsung; внутри ключа id убыв.
    }
    Ok(())
}

#[tokio::test]
async fn stale_ready_uses_the_last_status_change_or_receipt() -> Result<(), CoreError> {
    for t in cores().await {
        let ready = status_id_by_name(&t.core, "Готов").await;
        let day = chrono::Duration::days(1);
        let start = t.core.now_utc();
        // A: принят 60 дней назад, «Готов» сегодня.
        t.clock.set(start - day * 60);
        let a = create_order_with(&t, |_| {}).await.id;
        let b = create_order_with(&t, |_| {}).await.id;
        let c = create_order_with(&t, |_| {}).await.id;
        t.clock.set(start - day * 31);
        t.core.change_status(&admin(), b, 1, ready).await?;
        t.clock.set(start);
        t.core.change_status(&admin(), a, 1, ready).await?;
        // C: «Готов» без истории, принят 40 дней назад.
        t.core.change_status(&admin(), c, 1, ready).await?;
        set_order_keys_directly(&t, c, None, "").await;
        mw_db::entity::order::Entity::update_many()
            .col_expr(
                mw_db::entity::order::Column::ReceivedAt,
                sea_orm::sea_query::Expr::value((start - day * 40).naive_utc()),
            )
            .filter(mw_db::entity::order::Column::Id.eq(c))
            .exec(t.core.db())
            .await
            .unwrap();
        let stale = OrderFilter {
            stale_ready: true,
            ..Default::default()
        };
        assert_eq!(
            sorted(list_ids(&t, stale).await),
            sorted(vec![b, c]),
            "{}",
            t.label
        );
    }
    Ok(())
}

#[tokio::test]
async fn received_and_issued_periods_are_inclusive() -> Result<(), CoreError> {
    for t in cores().await {
        let Seed { a, b, c, .. } = seed_orders(&t).await;
        let received = |from: Option<NaiveDateTime>, to: Option<NaiveDateTime>| OrderFilter {
            received_from: from,
            received_to: to,
            ..Default::default()
        };
        let d = |day: u32, h: u32| {
            chrono::NaiveDate::from_ymd_opt(2026, 11, day)
                .unwrap()
                .and_hms_opt(h, 0, 0)
                .unwrap()
        };
        // A принят 06.11 09:00; границы включительно.
        assert_eq!(
            list_ids(&t, received(Some(d(6, 9)), Some(d(6, 9)))).await,
            vec![a],
            "{}",
            t.label
        );
        assert_eq!(
            sorted(list_ids(&t, received(Some(d(10, 9)), None)).await),
            sorted(vec![b, c])
        );
        assert_eq!(
            list_ids(&t, received(None, Some(d(6, 8)))).await,
            Vec::<i64>::new()
        );
        // C выдан 11.11 08:00.
        let issued = |from: Option<NaiveDateTime>, to: Option<NaiveDateTime>| OrderFilter {
            issued_from: from,
            issued_to: to,
            ..Default::default()
        };
        assert_eq!(
            list_ids(&t, issued(Some(d(11, 8)), Some(d(11, 8)))).await,
            vec![c]
        );
        assert_eq!(
            list_ids(&t, issued(Some(d(11, 9)), None)).await,
            Vec::<i64>::new()
        );
    }
    Ok(())
}
