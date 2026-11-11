mod common;

use common::*;
use mw_core::{CoreError, IssuedFilter, OrderFilter, OrderSort, OrderSortField, Page};
use mw_domain::permission::Permission;

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
