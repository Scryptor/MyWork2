//! Переименование и объединение значений справочника вместе с заказами и клиентами.
#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use chrono::Duration;
use common::{
    TestCore, actor_with, admin, assert_validation, audit_details, cores, count_audit, last_audit,
    list_ids, phones, sample_new_order, sorted,
};
use mw_core::{
    Core, CoreError, DictionaryMerge, DictionaryRename, NewClient, NewOrder, Order, OrderFilter,
    OrderPatch, PageRequest,
};
use mw_db::entity::dictionary_item;
use mw_domain::dictionary::DictionaryKind;
use mw_domain::permission::Permission;
use mw_domain::warranty::WarrantyTerm;
use sea_orm::{ActiveModelTrait, NotSet, Set};
use serde_json::json;

async fn order_with(t: &TestCore, f: impl FnOnce(&mut NewOrder)) -> Order {
    let mut new = sample_new_order();
    f(&mut new);
    t.core.create_order(&admin(), new).await.unwrap()
}

async fn reload(t: &TestCore, o: &Order) -> Order {
    t.core.get_order(&admin(), o.id).await.unwrap()
}

async fn item_id(core: &Core, kind: DictionaryKind, value: &str) -> i64 {
    core.list_dictionary(kind, true)
        .await
        .unwrap()
        .iter()
        .find(|i| i.value == value)
        .unwrap_or_else(|| panic!("нет значения {value}"))
        .id
}

/// Значение в обход проверки дублей — так в базу попадают «чехол» рядом с «Чехол» из MyWork 2.
async fn insert_item_directly(t: &TestCore, kind: DictionaryKind, value: &str) -> i64 {
    dictionary_item::ActiveModel {
        id: NotSet,
        kind: Set(kind.as_str().to_owned()),
        value: Set(value.to_owned()),
        sort_order: Set(100),
        archived: Set(false),
    }
    .insert(t.core.db())
    .await
    .unwrap()
    .id
}

fn rename(value: &str, in_orders: bool) -> DictionaryRename {
    DictionaryRename {
        value: value.to_owned(),
        in_orders,
    }
}

fn merge(target_id: i64, source_ids: &[i64]) -> DictionaryMerge {
    DictionaryMerge {
        target_id,
        source_ids: source_ids.to_vec(),
        in_orders: true,
    }
}

#[tokio::test]
async fn rename_in_orders_edits_matching_orders_like_a_normal_edit() -> Result<(), CoreError> {
    for t in cores().await {
        let upper = order_with(&t, |o| o.brand = "Samsung".into()).await;
        let lower = order_with(&t, |o| o.brand = "samsung".into()).await;
        let apple = order_with(&t, |o| o.brand = "Apple".into()).await;
        let gone = order_with(&t, |o| o.brand = "Samsung".into()).await;
        t.core.delete_order(&admin(), gone.id, gone.version).await?;
        t.clock.advance(Duration::minutes(5));
        let id = item_id(&t.core, DictionaryKind::Brand, "Samsung").await;

        let r = t
            .core
            .rename_dictionary_item_everywhere(&admin(), id, rename(" Samsung Electronics ", true))
            .await?;
        assert_eq!(
            (r.item.id, r.item.value.as_str(), r.records_changed),
            (id, "Samsung Electronics", 2),
            "{}",
            t.label
        );
        for (before, old) in [(&upper, "Samsung"), (&lower, "samsung")] {
            let after = reload(&t, before).await;
            assert_eq!(
                (after.brand.as_str(), after.version, after.updated_at),
                ("Samsung Electronics", before.version + 1, t.core.now()),
                "{}",
                t.label
            );
            let last = last_audit(&t.core, before.id).await;
            assert_eq!(
                (last.action.as_str(), last.summary.as_str()),
                ("order.updated", "Изменено: бренд")
            );
            assert_eq!(
                last.details.unwrap()["changes"],
                json!({ "brand": [old, "Samsung Electronics"] })
            );
        }
        let history = t
            .core
            .order_history(&admin(), lower.id, PageRequest::default())
            .await?;
        assert!(history.items.iter().any(|e| e.action == "order.updated"
            && e.details.as_ref().unwrap()["changes"]["brand"]
                == json!(["samsung", "Samsung Electronics"])));

        let untouched = reload(&t, &apple).await;
        assert_eq!(
            (untouched.brand.as_str(), untouched.version),
            ("Apple", apple.version)
        );
        let deleted = reload(&t, &gone).await;
        assert_eq!(
            (deleted.brand.as_str(), deleted.version),
            ("Samsung", gone.version + 1),
            "удалённый заказ не трогается"
        );

        let found = list_ids(
            &t,
            OrderFilter {
                query: "electronics".into(),
                ..Default::default()
            },
        )
        .await;
        assert_eq!(
            sorted(found),
            sorted(vec![upper.id, lower.id]),
            "{}",
            t.label
        );
        let by_key = list_ids(
            &t,
            OrderFilter {
                brand: Some("SAMSUNG ELECTRONICS".into()),
                ..Default::default()
            },
        )
        .await;
        assert_eq!(sorted(by_key), sorted(vec![upper.id, lower.id]));

        // Панель с прежней версией получает обычный спор версий.
        let stale = t
            .core
            .update_order(
                &admin(),
                upper.id,
                upper.version,
                OrderPatch {
                    model: Some("A53".into()),
                    ..Default::default()
                },
            )
            .await;
        assert!(
            matches!(stale, Err(CoreError::Conflict { .. })),
            "{stale:?}"
        );

        assert_eq!(count_audit(&t.core, "dictionary.renamed").await, 1);
        let d = &audit_details(&t.core, "dictionary.renamed").await[0];
        assert_eq!(
            (&d["old"], &d["new"], &d["in_orders"], &d["records_changed"]),
            (
                &json!("Samsung"),
                &json!("Samsung Electronics"),
                &json!(true),
                &json!(2)
            )
        );
    }
    Ok(())
}

#[tokio::test]
async fn rename_without_in_orders_changes_only_the_dictionary() -> Result<(), CoreError> {
    for t in cores().await {
        let o = order_with(&t, |o| o.brand = "Samsung".into()).await;
        let id = item_id(&t.core, DictionaryKind::Brand, "Samsung").await;
        let r = t
            .core
            .rename_dictionary_item_everywhere(&admin(), id, rename("Samsung Electronics", false))
            .await?;
        assert_eq!(
            (r.item.value.as_str(), r.records_changed),
            ("Samsung Electronics", 0),
            "{}",
            t.label
        );
        let after = reload(&t, &o).await;
        assert_eq!(
            (after.brand.as_str(), after.version),
            ("Samsung", o.version)
        );
        assert_eq!(count_audit(&t.core, "order.updated").await, 0);
        assert_eq!(count_audit(&t.core, "dictionary.renamed").await, 1);
    }
    Ok(())
}

#[tokio::test]
async fn rename_to_the_same_value_fixes_case_and_untouched_rename_writes_nothing()
-> Result<(), CoreError> {
    for t in cores().await {
        let lower = order_with(&t, |o| o.brand = "samsung".into()).await;
        let upper = order_with(&t, |o| o.brand = "Samsung".into()).await;
        let id = item_id(&t.core, DictionaryKind::Brand, "Samsung").await;

        let r = t
            .core
            .rename_dictionary_item_everywhere(&admin(), id, rename("Samsung", false))
            .await?;
        assert_eq!(r.records_changed, 0);
        assert_eq!(count_audit(&t.core, "dictionary.renamed").await, 0);

        let r = t
            .core
            .rename_dictionary_item_everywhere(&admin(), id, rename(" Samsung ", true))
            .await?;
        assert_eq!(
            (r.item.value.as_str(), r.records_changed),
            ("Samsung", 1),
            "{}",
            t.label
        );
        let fixed = reload(&t, &lower).await;
        assert_eq!(
            (fixed.brand.as_str(), fixed.version),
            ("Samsung", lower.version + 1)
        );
        assert_eq!(reload(&t, &upper).await.version, upper.version);
        assert_eq!(count_audit(&t.core, "dictionary.renamed").await, 1);
        assert_eq!(count_audit(&t.core, "order.updated").await, 1);

        let r = t
            .core
            .rename_dictionary_item_everywhere(&admin(), id, rename("Samsung", true))
            .await?;
        assert_eq!(r.records_changed, 0);
        assert_eq!(count_audit(&t.core, "dictionary.renamed").await, 1);
        assert_eq!(count_audit(&t.core, "order.updated").await, 1);
    }
    Ok(())
}

#[tokio::test]
async fn rename_checks_the_value_like_a_plain_rename() -> Result<(), CoreError> {
    for t in cores().await {
        let o = order_with(&t, |o| o.brand = "Samsung".into()).await;
        let id = item_id(&t.core, DictionaryKind::Brand, "Samsung").await;
        assert_validation(
            t.core
                .rename_dictionary_item_everywhere(&admin(), id, rename("apple", true))
                .await,
            &[("value", "duplicate")],
        );
        assert_validation(
            t.core
                .rename_dictionary_item_everywhere(&admin(), id, rename("  ", true))
                .await,
            &[("value", "required")],
        );
        assert!(matches!(
            t.core
                .rename_dictionary_item_everywhere(&admin(), 999_999, rename("X", true))
                .await,
            Err(CoreError::NotFound { .. })
        ));
        assert_eq!(reload(&t, &o).await.version, o.version, "{}", t.label);
    }
    Ok(())
}

#[tokio::test]
async fn merge_rewrites_whole_list_elements_and_collapses_repeats() -> Result<(), CoreError> {
    for t in cores().await {
        let target = item_id(&t.core, DictionaryKind::Completeness, "Чехол").await;
        let lower = insert_item_directly(&t, DictionaryKind::Completeness, "чехол").await;
        let silicone = t
            .core
            .add_dictionary_item(&admin(), DictionaryKind::Completeness, "Чехол силиконовый")
            .await?
            .id;
        let mixed = order_with(&t, |o| o.completeness = "чехол, Зарядка, Чехол".into()).await;
        let spaced = order_with(&t, |o| o.completeness = "Кабель,чехол ,, Коробка".into()).await;
        let other = order_with(&t, |o| o.completeness = "Чехол силиконовый".into()).await;
        let clean = order_with(&t, |o| o.completeness = "Чехол, Зарядка".into()).await;

        let r = t
            .core
            .merge_dictionary_items(
                &admin(),
                DictionaryKind::Completeness,
                merge(target, &[lower]),
            )
            .await?;
        assert_eq!(
            (r.item.id, r.item.value.as_str(), r.records_changed),
            (target, "Чехол", 2),
            "{}",
            t.label
        );
        let after = reload(&t, &mixed).await;
        assert_eq!(
            (after.completeness.as_str(), after.version),
            ("Чехол, Зарядка", mixed.version + 1),
            "{}",
            t.label
        );
        assert_eq!(
            last_audit(&t.core, mixed.id).await.details.unwrap()["changes"],
            json!({ "completeness": ["чехол, Зарядка, Чехол", "Чехол, Зарядка"] })
        );
        assert_eq!(
            reload(&t, &spaced).await.completeness,
            "Кабель, Чехол, Коробка"
        );
        for o in [&other, &clean] {
            let same = reload(&t, o).await;
            assert_eq!(
                (same.completeness.as_str(), same.version),
                (o.completeness.as_str(), o.version)
            );
        }

        let ids: Vec<i64> = t
            .core
            .list_dictionary(DictionaryKind::Completeness, true)
            .await?
            .iter()
            .map(|i| i.id)
            .collect();
        assert!(ids.contains(&target) && ids.contains(&silicone));
        assert!(!ids.contains(&lower), "источник удалён");
        assert_eq!(count_audit(&t.core, "dictionary.merged").await, 1);
        let d = &audit_details(&t.core, "dictionary.merged").await[0];
        assert_eq!(
            (&d["target_id"], &d["records_changed"], &d["in_orders"]),
            (&json!(target), &json!(2), &json!(true))
        );
        assert_eq!(d["sources"], json!([{ "id": lower, "value": "чехол" }]));
    }
    Ok(())
}

#[tokio::test]
async fn merge_client_sources_edits_clients() -> Result<(), CoreError> {
    for t in cores().await {
        let target = item_id(&t.core, DictionaryKind::ClientSource, "Интернет").await;
        let lower = insert_item_directly(&t, DictionaryKind::ClientSource, "интернет").await;
        let mut clients = Vec::new();
        for (i, source) in ["интернет", "интернет", "Интернет", "Авито"]
            .into_iter()
            .enumerate()
        {
            clients.push(
                t.core
                    .create_client(
                        &admin(),
                        NewClient {
                            full_name: format!("Клиент {i}"),
                            phones: phones(&[&format!("+7 921 000-00-0{i}")]),
                            source: source.into(),
                            ..Default::default()
                        },
                    )
                    .await?,
            );
        }
        let r = t
            .core
            .merge_dictionary_items(
                &admin(),
                DictionaryKind::ClientSource,
                merge(target, &[lower]),
            )
            .await?;
        assert_eq!(
            (r.item.value.as_str(), r.records_changed),
            ("Интернет", 2),
            "{}",
            t.label
        );
        let mut got = Vec::new();
        for c in &clients {
            let now = t.core.get_client(&admin(), c.id).await?;
            assert_eq!(now.phones, c.phones, "телефоны не меняются");
            got.push((now.source, now.version));
        }
        assert_eq!(
            got,
            [
                ("Интернет".to_owned(), 2),
                ("Интернет".to_owned(), 2),
                ("Интернет".to_owned(), 1),
                ("Авито".to_owned(), 1),
            ],
            "{}",
            t.label
        );
        assert_eq!(
            audit_details(&t.core, "client.updated").await,
            [
                json!({ "source": "Интернет" }),
                json!({ "source": "Интернет" })
            ]
        );
        let usage = t
            .core
            .dictionary_usage(&admin(), DictionaryKind::ClientSource)
            .await?;
        assert!(usage.iter().any(|u| u.id == target && u.count == 3));
    }
    Ok(())
}

#[tokio::test]
async fn rename_recomputes_master_key_and_warranty_term() -> Result<(), CoreError> {
    for t in cores().await {
        let ivanov = t
            .core
            .add_dictionary_item(&admin(), DictionaryKind::Master, "Иванов")
            .await?
            .id;
        let o = order_with(&t, |o| {
            o.master = "иванов".into();
            o.warranty = "14 дней".into();
        })
        .await;
        let other = order_with(&t, |o| o.warranty = "30 дней".into()).await;
        let r = t
            .core
            .rename_dictionary_item_everywhere(&admin(), ivanov, rename("Петров", true))
            .await?;
        assert_eq!(r.records_changed, 1, "{}", t.label);
        let master = |name: &str| OrderFilter {
            master: Some(name.into()),
            ..Default::default()
        };
        assert_eq!(
            list_ids(&t, master("ПЕТРОВ")).await,
            vec![o.id],
            "{}",
            t.label
        );
        assert!(list_ids(&t, master("иванов")).await.is_empty());

        let w = item_id(&t.core, DictionaryKind::Warranty, "14 дней").await;
        let r = t
            .core
            .rename_dictionary_item_everywhere(&admin(), w, rename("2 месяца", true))
            .await?;
        assert_eq!(r.records_changed, 1);
        let after = reload(&t, &o).await;
        assert_eq!(
            (after.warranty.as_str(), after.warranty_term, after.version),
            ("2 месяца", Some(WarrantyTerm::months(2)), o.version + 2),
            "{}",
            t.label
        );
        let last = last_audit(&t.core, o.id).await;
        assert_eq!(last.summary, "Изменено: гарантия");
        assert_eq!(
            last.details.unwrap()["changes"],
            json!({ "warranty": ["14 дней", "2 месяца"] })
        );
        let same = reload(&t, &other).await;
        assert_eq!(
            (same.warranty_term, same.version),
            (Some(WarrantyTerm::days(30)), other.version)
        );
    }
    Ok(())
}

/// Поле заказа, в котором хранится значение справочника этого вида.
fn order_field(o: &Order, kind: DictionaryKind) -> &str {
    match kind {
        DictionaryKind::DeviceType => &o.device_type,
        DictionaryKind::Brand => &o.brand,
        DictionaryKind::Color => &o.color,
        DictionaryKind::Completeness => &o.completeness,
        DictionaryKind::Condition => &o.device_condition,
        DictionaryKind::Malfunction => &o.malfunction,
        DictionaryKind::WorkDone => &o.work_done,
        DictionaryKind::Master => &o.master,
        DictionaryKind::Warranty => &o.warranty,
        DictionaryKind::ClientSource => unreachable!(),
    }
}

const ORDER_KINDS: [DictionaryKind; 9] = [
    DictionaryKind::DeviceType,
    DictionaryKind::Brand,
    DictionaryKind::Color,
    DictionaryKind::Completeness,
    DictionaryKind::Condition,
    DictionaryKind::Malfunction,
    DictionaryKind::WorkDone,
    DictionaryKind::Master,
    DictionaryKind::Warranty,
];

/// Каждый вид меняет своё поле заказа и только его.
#[tokio::test]
async fn every_order_kind_rewrites_its_own_field() -> Result<(), CoreError> {
    for t in cores().await {
        let created = t.core.create_order(&admin(), sample_new_order()).await?;
        let old = |k: DictionaryKind| format!("old-{}", k.as_str());
        let mut o = t
            .core
            .update_order(
                &admin(),
                created.id,
                created.version,
                OrderPatch {
                    device_type: Some(old(DictionaryKind::DeviceType)),
                    brand: Some(old(DictionaryKind::Brand)),
                    color: Some(old(DictionaryKind::Color)),
                    completeness: Some(old(DictionaryKind::Completeness)),
                    device_condition: Some(old(DictionaryKind::Condition)),
                    malfunction: Some(old(DictionaryKind::Malfunction)),
                    work_done: Some(old(DictionaryKind::WorkDone)),
                    master: Some(old(DictionaryKind::Master)),
                    warranty: Some(old(DictionaryKind::Warranty)),
                    ..Default::default()
                },
            )
            .await?;
        for kind in ORDER_KINDS {
            let id = t
                .core
                .add_dictionary_item(&admin(), kind, &old(kind).to_uppercase())
                .await?
                .id;
            let new = format!("new-{}", kind.as_str());
            let r = t
                .core
                .rename_dictionary_item_everywhere(&admin(), id, rename(&new, true))
                .await?;
            assert_eq!(r.records_changed, 1, "{}: {kind:?}", t.label);
            let after = reload(&t, &o).await;
            for k in ORDER_KINDS {
                let expected = if k == kind {
                    new.clone()
                } else {
                    order_field(&o, k).to_owned()
                };
                assert_eq!(
                    order_field(&after, k),
                    expected,
                    "{}: {kind:?} → {k:?}",
                    t.label
                );
            }
            assert_eq!(after.version, o.version + 1);
            o = after;
        }
    }
    Ok(())
}

#[tokio::test]
async fn merge_rejects_bad_requests_and_both_need_settings_manage() -> Result<(), CoreError> {
    for t in cores().await {
        let k = DictionaryKind::Completeness;
        let target = item_id(&t.core, k, "Чехол").await;
        let charger = item_id(&t.core, k, "Зарядка").await;
        let apple = item_id(&t.core, DictionaryKind::Brand, "Apple").await;
        let bad = |target_id: i64, source_ids: &[i64]| merge(target_id, source_ids);
        assert_validation(
            t.core
                .merge_dictionary_items(&admin(), k, bad(target, &[]))
                .await,
            &[("source_ids", "required")],
        );
        assert_validation(
            t.core
                .merge_dictionary_items(&admin(), k, bad(target, &[charger, target]))
                .await,
            &[("source_ids", "invalid")],
        );
        assert_validation(
            t.core
                .merge_dictionary_items(&admin(), k, bad(target, &[apple]))
                .await,
            &[("source_ids", "invalid")],
        );
        assert_validation(
            t.core
                .merge_dictionary_items(&admin(), k, bad(apple, &[charger]))
                .await,
            &[("target_id", "invalid")],
        );
        assert_validation(
            t.core
                .merge_dictionary_items(&admin(), k, bad(999_999, &[charger]))
                .await,
            &[("target_id", "invalid")],
        );

        let clerk = actor_with(&[
            Permission::OrdersCreate,
            Permission::OrdersEdit,
            Permission::ClientsEdit,
        ]);
        assert!(matches!(
            t.core
                .merge_dictionary_items(&clerk, k, bad(target, &[charger]))
                .await,
            Err(CoreError::Forbidden(Permission::SettingsManage))
        ));
        assert!(matches!(
            t.core
                .rename_dictionary_item_everywhere(&clerk, target, rename("Чехлы", true))
                .await,
            Err(CoreError::Forbidden(Permission::SettingsManage))
        ));
        assert!(
            t.core
                .list_dictionary(k, true)
                .await?
                .iter()
                .any(|i| i.id == charger)
        );
        assert_eq!(
            count_audit(&t.core, "dictionary.merged").await,
            0,
            "{}",
            t.label
        );
    }
    Ok(())
}
