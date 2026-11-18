mod common;

use common::{actor_with, admin, assert_validation, cores, count_audit, ids, phones};
use mw_core::{
    ClientPatch, ClientPhone, ClientRef, CoreError, NewClient, NewOrder, OrderFilter, OrderSort,
};
use mw_domain::permission::Permission;

#[tokio::test]
async fn create_search_and_optimistic_locking() -> Result<(), CoreError> {
    for t in cores().await {
        // create_normalizes_and_finds_by_any_phone_format
        let c = t
            .core
            .create_client(
                &admin(),
                NewClient {
                    full_name: " ПЁТР ИВАНОВ ".into(),
                    phones: phones(&["+7 (921) 123-45-67"]),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(
            (c.full_name.as_str(), c.phones, c.version),
            ("ПЁТР ИВАНОВ", phones(&["79211234567"]), 1)
        );
        for q in ["8 921 123 45 67", "9211234567", "+79211234567"] {
            assert_eq!(
                ids(t.core.find_clients_by_phone(&admin(), q).await?),
                vec![c.id],
                "{} {q}",
                t.label
            );
        }
        assert!(
            t.core
                .find_clients_by_phone(&admin(), "8 900 000 00 00")
                .await?
                .is_empty()
        );
        assert!(t.core.find_clients_by_phone(&admin(), "").await?.is_empty());
        assert_eq!(count_audit(&t.core, "client.created").await, 1);

        // search_is_case_and_yo_insensitive_and_literal
        let other = t
            .core
            .create_client(
                &admin(),
                NewClient {
                    full_name: "Анна Ким".into(),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(
            ids(t.core.find_clients(&admin(), "пётр", 50, 0).await?.items),
            vec![c.id]
        );
        assert_eq!(
            ids(t
                .core
                .find_clients(&admin(), "иванов 921", 50, 0)
                .await?
                .items),
            vec![c.id]
        );
        assert_eq!(
            ids(t
                .core
                .find_clients(&admin(), "+7 (921) 123-45-67", 50, 0)
                .await?
                .items),
            vec![c.id]
        );
        assert!(
            t.core
                .find_clients(&admin(), "%", 50, 0)
                .await?
                .items
                .is_empty()
        );
        assert!(
            t.core
                .find_clients(&admin(), "_", 50, 0)
                .await?
                .items
                .is_empty()
        );
        assert_eq!(t.core.find_clients(&admin(), "", 50, 0).await?.total, 2);
        assert_eq!(
            ids(t.core.find_clients(&admin(), "", 1, 0).await?.items),
            vec![other.id]
        ); // «анна» раньше «петр»

        // stale_version_conflicts_and_keeps_first_edit
        let v2 = t
            .core
            .update_client(
                &admin(),
                c.id,
                1,
                ClientPatch {
                    address: Some("Ленина, 1".into()),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(v2.version, 2);
        let stale = t
            .core
            .update_client(
                &admin(),
                c.id,
                1,
                ClientPatch {
                    address: Some("Мира, 2".into()),
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
        assert_eq!(
            t.core.get_client(&admin(), c.id).await?.address,
            "Ленина, 1"
        );

        // no-op update keeps the version and writes no audit
        let same = t
            .core
            .update_client(
                &admin(),
                c.id,
                2,
                ClientPatch {
                    address: Some(" Ленина, 1 ".into()),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(same.version, 2);
        assert_eq!(count_audit(&t.core, "client.updated").await, 1);

        // phone change recomputes keys; unknown id
        let v3 = t
            .core
            .update_client(
                &admin(),
                c.id,
                2,
                ClientPatch {
                    phones: Some(phones(&["8 900 111 22 33"])),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!((v3.version, v3.phones), (3, phones(&["89001112233"])));
        assert_eq!(
            ids(t
                .core
                .find_clients_by_phone(&admin(), "+7 900 111 22 33")
                .await?),
            vec![c.id]
        );
        assert!(
            t.core
                .find_clients_by_phone(&admin(), "9211234567")
                .await?
                .is_empty()
        );
        assert!(matches!(
            t.core
                .update_client(&admin(), 9999, 1, ClientPatch::default())
                .await,
            Err(CoreError::NotFound { .. })
        ));
    }
    Ok(())
}

#[tokio::test]
async fn rights_and_validation() -> Result<(), CoreError> {
    for t in cores().await {
        let viewer = actor_with(&[Permission::ReportsView]);
        assert!(matches!(
            t.core.create_client(&viewer, NewClient::default()).await,
            Err(CoreError::Forbidden(Permission::ClientsCreate))
        ));
        assert!(matches!(
            t.core.find_clients(&viewer, "", 10, 0).await,
            Err(CoreError::Forbidden(Permission::ClientsView))
        ));
        assert_validation(
            t.core.create_client(&admin(), NewClient::default()).await,
            &[("full_name", "required")],
        );
        assert_validation(
            t.core.find_clients(&admin(), "", 0, 0).await,
            &[("limit", "out_of_range")],
        );
        let intake = actor_with(&[Permission::OrdersCreate]);
        let c = t
            .core
            .create_client(
                &intake,
                NewClient {
                    phones: phones(&["89211234567"]),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(t.core.find_clients(&intake, "", 10, 0).await?.total, 1);
        assert!(matches!(
            t.core
                .update_client(&viewer, c.id, 1, ClientPatch::default())
                .await,
            Err(CoreError::Forbidden(Permission::ClientsEdit))
        ));
    }
    Ok(())
}

#[tokio::test]
async fn phones_list_keeps_order_and_any_number_finds_the_client() -> Result<(), CoreError> {
    for t in cores().await {
        let c = t
            .core
            .create_client(
                &admin(),
                NewClient {
                    full_name: "ИВАНОВ ИВАН".into(),
                    phones: vec![
                        ClientPhone {
                            phone: "+7 (921) 123-45-67".into(),
                            note: String::new(),
                        },
                        ClientPhone {
                            phone: "8 911 765 43 21".into(),
                            note: " жена ".into(),
                        },
                    ],
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(
            c.phones,
            vec![
                ClientPhone {
                    phone: "79211234567".into(),
                    note: String::new()
                },
                ClientPhone {
                    phone: "89117654321".into(),
                    note: "жена".into()
                }
            ]
        );
        assert_eq!(
            ids(t
                .core
                .find_clients_by_phone(&admin(), "+7 911 765-43-21")
                .await?),
            vec![c.id]
        );
        assert_eq!(
            ids(t.core.find_clients(&admin(), "911 765", 50, 0).await?.items),
            vec![c.id]
        );
        let o = t
            .core
            .create_order(
                &admin(),
                NewOrder {
                    client: ClientRef::Existing(c.id),
                    ..common::sample_new_order()
                },
            )
            .await?;
        let page = t
            .core
            .list_orders(
                &admin(),
                &OrderFilter::default(),
                OrderSort::default(),
                50,
                0,
            )
            .await?;
        assert_eq!(
            page.items
                .iter()
                .find(|i| i.id == o.id)
                .unwrap()
                .client_phone,
            "79211234567"
        );

        // replacing_phones_drops_old_numbers_from_lookup
        let u = t
            .core
            .update_client(
                &admin(),
                c.id,
                c.version,
                ClientPatch {
                    phones: Some(phones(&["8 911 765 43 21"])),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(u.phones.len(), 1);
        let same = t
            .core
            .update_client(
                &admin(),
                c.id,
                1,
                ClientPatch {
                    phones: Some(phones(&["8 (911) 765-43-21"])),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(same, u);
        assert_eq!(t.core.get_client(&admin(), c.id).await?, u);
        assert!(
            t.core
                .find_clients_by_phone(&admin(), "9211234567")
                .await?
                .is_empty()
        );
    }
    Ok(())
}

#[tokio::test]
async fn phone_list_validation() -> Result<(), CoreError> {
    for t in cores().await {
        let bad = |p: Vec<ClientPhone>| NewClient {
            full_name: "А".into(),
            phones: p,
            ..Default::default()
        };
        assert_validation(
            t.core
                .create_client(
                    &admin(),
                    bad(vec![ClientPhone {
                        phone: "нет".into(),
                        note: "x".into(),
                    }]),
                )
                .await,
            &[("phones", "invalid")],
        );
        assert_validation(
            t.core
                .create_client(
                    &admin(),
                    bad(phones(&["8 921 123-45-67", "+7 921 1234567"])),
                )
                .await,
            &[("phones", "duplicate")],
        );
        assert_validation(
            t.core
                .create_client(
                    &admin(),
                    bad(phones(&[
                        "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11",
                    ])),
                )
                .await,
            &[("phones", "out_of_range")],
        );
        assert_validation(
            t.core
                .create_client(
                    &admin(),
                    NewClient {
                        phones: vec![ClientPhone {
                            phone: " ".into(),
                            note: " ".into(),
                        }],
                        ..Default::default()
                    },
                )
                .await,
            &[("full_name", "required")],
        );
    }
    Ok(())
}

#[tokio::test]
async fn phone_input_is_validated_on_update_even_when_cleaned_list_is_unchanged()
-> Result<(), CoreError> {
    for t in cores().await {
        let c = t
            .core
            .create_client(
                &admin(),
                NewClient {
                    full_name: "А".into(),
                    phones: phones(&["89211234567"]),
                    ..Default::default()
                },
            )
            .await?;
        let patch = |p: Vec<ClientPhone>| ClientPatch {
            phones: Some(p),
            ..Default::default()
        };
        let mut with_note_only = phones(&["89211234567"]);
        with_note_only.push(ClientPhone {
            phone: "нет".into(),
            note: "x".into(),
        });
        assert_validation(
            t.core
                .update_client(&admin(), c.id, c.version, patch(with_note_only))
                .await,
            &[("phones", "invalid")],
        );
        assert_validation(
            t.core
                .update_client(
                    &admin(),
                    c.id,
                    c.version,
                    patch(phones(&["8 921 123-45-67", "+7 921 1234567"])),
                )
                .await,
            &[("phones", "duplicate")],
        );
        let many: Vec<String> = (1..=11).map(|i| i.to_string()).collect();
        let many: Vec<&str> = many.iter().map(String::as_str).collect();
        assert_validation(
            t.core
                .update_client(&admin(), c.id, c.version, patch(phones(&many)))
                .await,
            &[("phones", "out_of_range")],
        );
        assert_eq!(t.core.get_client(&admin(), c.id).await?, c);
    }
    Ok(())
}

#[tokio::test]
async fn list_items_carry_order_counts_and_sort_by_last_order() -> Result<(), CoreError> {
    use chrono::{TimeZone, Utc};
    use mw_core::{ClientFilter, ClientSort, ClientSortField};
    for t in cores().await {
        let a = common::make_client(&t, "Анна", &["8 911 000-00-01"]).await;
        let b = common::make_client(&t, "Борис", &["8 911 000-00-02"]).await;
        let c = common::make_client(&t, "Вера", &[]).await;
        t.clock
            .set(Utc.with_ymd_and_hms(2026, 11, 5, 9, 0, 0).unwrap());
        common::make_order_for(&t, a.id).await;
        t.clock
            .set(Utc.with_ymd_and_hms(2026, 11, 9, 9, 0, 0).unwrap());
        common::make_order_for(&t, b.id).await;
        common::make_order_for(&t, b.id).await;
        let deleted = common::make_order_for(&t, a.id).await;
        t.core.delete_order(&admin(), deleted.id, 1).await?;

        let page = t
            .core
            .list_clients(&admin(), &ClientFilter::default(), 50, 0)
            .await?;
        // по умолчанию: последний заказ, новые сверху, без заказов в конце
        assert_eq!(
            ids(page.items.clone()),
            vec![b.id, a.id, c.id],
            "{}",
            t.label
        );
        let counts: Vec<u64> = page.items.iter().map(|i| i.orders_count).collect();
        assert_eq!(counts, vec![2, 1, 0], "{}", t.label);
        assert!(page.items[0].last_order_at.is_some());
        assert_eq!(page.items[2].last_order_at, None);
        assert_eq!(page.items[0].phones, phones(&["89110000002"]));

        let sorted = |field, descending| ClientFilter {
            sort: ClientSort { field, descending },
            ..Default::default()
        };
        let asc = t
            .core
            .list_clients(&admin(), &sorted(ClientSortField::LastOrder, false), 50, 0)
            .await?;
        assert_eq!(ids(asc.items), vec![a.id, b.id, c.id], "{}", t.label);
        let by_count = t
            .core
            .list_clients(&admin(), &sorted(ClientSortField::OrdersCount, true), 50, 0)
            .await?;
        assert_eq!(ids(by_count.items), vec![b.id, a.id, c.id], "{}", t.label);
        let by_name = t
            .core
            .list_clients(&admin(), &sorted(ClientSortField::Name, true), 50, 0)
            .await?;
        assert_eq!(ids(by_name.items), vec![c.id, b.id, a.id], "{}", t.label);
    }
    Ok(())
}

#[tokio::test]
async fn duplicates_filter_groups_same_name_and_shared_phone() -> Result<(), CoreError> {
    use mw_core::ClientFilter;
    for t in cores().await {
        let i1 = common::make_client(&t, "ИВАНОВ ИВАН", &["8 900 111-11-11"]).await;
        let i2 = common::make_client(&t, "Иванов Иван", &["8 900 222-22-22"]).await;
        let p = common::make_client(&t, "Петров", &["8 921 123-45-67"]).await;
        let s = common::make_client(&t, "Сидоров", &["+7 921 123-45-67"]).await;
        let _kim = common::make_client(&t, "Ким", &["12345", "8 900 333-33-33"]).await;
        let _short = common::make_client(&t, "Короткий", &["12345"]).await;
        let _empty1 = common::make_client(&t, "", &["8 900 444-44-44"]).await;
        let _empty2 = common::make_client(&t, "", &["8 900 555-55-55"]).await;

        let page = t
            .core
            .list_clients(
                &admin(),
                &ClientFilter {
                    duplicates: true,
                    ..Default::default()
                },
                50,
                0,
            )
            .await?;
        assert_eq!(page.total, 4, "{}", t.label);
        let mut got = ids(page.items);
        got.sort();
        assert_eq!(got, vec![i1.id, i2.id, p.id, s.id], "{}", t.label);
        assert_eq!(
            ids(t.core.similar_clients(&admin(), i1.id).await?),
            vec![i2.id],
            "{}",
            t.label
        );
        assert_eq!(
            ids(t.core.similar_clients(&admin(), p.id).await?),
            vec![s.id],
            "{}",
            t.label
        );
        assert!(t.core.similar_clients(&admin(), _kim.id).await?.is_empty());
        assert!(matches!(
            t.core.similar_clients(&admin(), 9999).await,
            Err(CoreError::NotFound { .. })
        ));
    }
    Ok(())
}

/// 20 000 выдуманных клиентов прямо в базу: каждый 97-й делит имя со следующим, каждый 89-й —
/// телефон с предыдущим. Возвращает, сколько из них похожи на кого-то.
#[allow(clippy::unwrap_used)]
async fn seed_many_clients(t: &common::TestCore, n: usize) -> u64 {
    use std::collections::HashMap;

    use mw_db::entity::{client, client_phone};
    use mw_domain::phone::phone_key;
    use mw_domain::search::normalize_search;
    use sea_orm::{EntityTrait, Set};

    let at = chrono::NaiveDate::from_ymd_opt(2026, 1, 1)
        .unwrap()
        .and_hms_opt(9, 0, 0)
        .unwrap();
    let name_of = |i: usize| format!("Клиент {}", if i.is_multiple_of(97) { i + 1 } else { i });
    let phone_of = |i: usize| {
        let j = if i % 89 == 5 { i - 1 } else { i };
        format!("8 9{:09}", j * 7)
    };
    let mut by_name: HashMap<String, Vec<usize>> = HashMap::new();
    let mut by_phone: HashMap<String, Vec<usize>> = HashMap::new();
    for i in 0..n {
        by_name
            .entry(normalize_search(&name_of(i)))
            .or_default()
            .push(i);
        by_phone.entry(phone_key(&phone_of(i))).or_default().push(i);
    }
    let similar = (0..n)
        .filter(|&i| {
            by_name[&normalize_search(&name_of(i))].len() > 1
                || by_phone[&phone_key(&phone_of(i))].len() > 1
        })
        .count();

    let db = t.core.db();
    for chunk in (0..n).collect::<Vec<_>>().chunks(500) {
        let clients = chunk.iter().map(|&i| {
            let name = name_of(i);
            let phone = phone_of(i);
            client::ActiveModel {
                id: Set(i64::try_from(i).unwrap() + 1),
                full_name: Set(name.clone()),
                phone: Set(phone.clone()),
                phone_key: Set(phone_key(&phone)),
                address: Set(String::new()),
                note: Set(String::new()),
                source: Set(String::new()),
                blacklisted: Set(false),
                created_at: Set(at),
                updated_at: Set(at),
                deleted_at: Set(None),
                version: Set(1),
                search_text: Set(normalize_search(&name)),
                name_key: Set(normalize_search(&name)),
            }
        });
        client::Entity::insert_many(clients).exec(db).await.unwrap();
        let phones = chunk.iter().map(|&i| {
            let phone = phone_of(i);
            client_phone::ActiveModel {
                id: Set(i64::try_from(i).unwrap() + 1),
                client_id: Set(i64::try_from(i).unwrap() + 1),
                position: Set(0),
                phone_key: Set(phone_key(&phone)),
                phone: Set(phone),
                note: Set(String::new()),
            }
        });
        client_phone::Entity::insert_many(phones)
            .exec(db)
            .await
            .unwrap();
    }
    u64::try_from(similar).unwrap()
}

/// Фильтр «Похожие» на базе в 20 000 клиентов отвечает быстрее секунды (был квадратичным).
#[tokio::test]
#[ignore = "объёмная проверка: cargo test -p mw-core --test clients -- --ignored"]
async fn duplicates_filter_is_fast_on_many_clients() -> Result<(), CoreError> {
    use mw_core::ClientFilter;
    for t in cores().await {
        let expected = seed_many_clients(&t, 20_000).await;
        assert!(expected > 400, "{expected}");
        let filter = ClientFilter {
            duplicates: true,
            ..Default::default()
        };
        let started = std::time::Instant::now();
        let page = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            t.core.list_clients(&admin(), &filter, 50, 0),
        )
        .await
        .unwrap_or_else(|_| panic!("{}: страница «Похожие» дольше секунды", t.label))?;
        let elapsed = started.elapsed();
        eprintln!("{}: {elapsed:?}", t.label);
        assert_eq!(page.total, expected, "{}", t.label);
        assert_eq!(page.items.len(), 50, "{}", t.label);
        assert!(
            elapsed < std::time::Duration::from_secs(1),
            "{}: {elapsed:?}",
            t.label
        );
    }
    Ok(())
}
