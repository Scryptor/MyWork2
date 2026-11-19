#![allow(clippy::unwrap_used, clippy::expect_used)]
mod common;

use common::{actor_with, admin, assert_validation, cores, count_audit, sample_new_order};
use mw_core::{CatalogUsage, Core, CoreError, DictionaryItemPatch, NewClient, NewOrder};
use mw_domain::dictionary::DictionaryKind;
use mw_domain::permission::Permission;

async fn order_with(core: &Core, f: impl FnOnce(&mut NewOrder)) -> i64 {
    let mut new = sample_new_order();
    f(&mut new);
    core.create_order(&admin(), new).await.unwrap().id
}

async fn delete(core: &Core, id: i64) {
    let o = core.get_order(&admin(), id).await.unwrap();
    core.delete_order(&admin(), id, o.version).await.unwrap();
}

async fn item_id(core: &Core, kind: DictionaryKind, value: &str) -> i64 {
    let items = core.list_dictionary(kind, true).await.unwrap();
    items
        .iter()
        .find(|i| i.value == value)
        .unwrap_or_else(|| panic!("нет значения {value}"))
        .id
}

fn usage(list: &[CatalogUsage]) -> Vec<(i64, u64)> {
    list.iter().map(|u| (u.id, u.count)).collect()
}

#[tokio::test]
async fn archived_items_hidden_by_default() -> Result<(), CoreError> {
    for t in cores().await {
        let apple = item_id(&t.core, DictionaryKind::Brand, "Apple").await;
        let archive = DictionaryItemPatch {
            archived: Some(true),
            ..Default::default()
        };
        let item = t
            .core
            .update_dictionary_item(&admin(), apple, archive.clone())
            .await?;
        assert!(item.archived, "{}", t.label);
        let visible = t.core.list_dictionary(DictionaryKind::Brand, false).await?;
        assert!(visible.iter().all(|i| i.id != apple), "{}", t.label);
        let all = t.core.list_dictionary(DictionaryKind::Brand, true).await?;
        assert!(all.iter().any(|i| i.id == apple && i.archived));
        assert_eq!(count_audit(&t.core, "dictionary.archived").await, 1);
        // повтор — без изменений и без записи
        t.core
            .update_dictionary_item(&admin(), apple, archive)
            .await?;
        assert_eq!(count_audit(&t.core, "dictionary.archived").await, 1);
        let back = t
            .core
            .update_dictionary_item(
                &admin(),
                apple,
                DictionaryItemPatch {
                    archived: Some(false),
                    ..Default::default()
                },
            )
            .await?;
        assert!(!back.archived);
        assert_eq!(count_audit(&t.core, "dictionary.restored").await, 1);
    }
    Ok(())
}

#[tokio::test]
async fn update_dictionary_item_value_rules() -> Result<(), CoreError> {
    for t in cores().await {
        let items = t.core.list_dictionary(DictionaryKind::Brand, true).await?;
        let (a, b) = (&items[0], &items[1]);
        let dup = t
            .core
            .update_dictionary_item(
                &admin(),
                a.id,
                DictionaryItemPatch {
                    value: Some(b.value.to_uppercase()),
                    ..Default::default()
                },
            )
            .await;
        assert_validation(dup, &[("value", "duplicate")]);
        let same = t
            .core
            .update_dictionary_item(
                &admin(),
                a.id,
                DictionaryItemPatch {
                    value: Some(format!(" {} ", a.value)),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(&same, a);
        assert_eq!(count_audit(&t.core, "dictionary.renamed").await, 0);
        let renamed = t
            .core
            .update_dictionary_item(
                &admin(),
                a.id,
                DictionaryItemPatch {
                    value: Some("Новое".into()),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(renamed.value, "Новое");
        assert_eq!(count_audit(&t.core, "dictionary.renamed").await, 1);
        assert!(matches!(
            t.core
                .update_dictionary_item(&admin(), 999_999, DictionaryItemPatch::default())
                .await,
            Err(CoreError::NotFound { .. })
        ));
    }
    Ok(())
}

#[tokio::test]
async fn dictionary_usage_whole_value_ignores_case_and_deleted() -> Result<(), CoreError> {
    for t in cores().await {
        order_with(&t.core, |o| o.brand = "Samsung".into()).await;
        order_with(&t.core, |o| o.brand = "samsung".into()).await;
        order_with(&t.core, |o| o.brand = "Apple".into()).await;
        let gone = order_with(&t.core, |o| o.brand = "Samsung".into()).await;
        delete(&t.core, gone).await;
        let samsung = item_id(&t.core, DictionaryKind::Brand, "Samsung").await;
        let apple = item_id(&t.core, DictionaryKind::Brand, "Apple").await;
        let got = t
            .core
            .dictionary_usage(&admin(), DictionaryKind::Brand)
            .await?;
        let mut expected = vec![(samsung, 2), (apple, 1)];
        expected.sort();
        let mut got = usage(&got);
        got.sort();
        assert_eq!(got, expected, "{}", t.label);
    }
    Ok(())
}

#[tokio::test]
async fn dictionary_usage_splits_lists_and_counts_order_once() -> Result<(), CoreError> {
    for t in cores().await {
        let cover = item_id(&t.core, DictionaryKind::Completeness, "Чехол").await;
        let charger = item_id(&t.core, DictionaryKind::Completeness, "Зарядка").await;
        order_with(&t.core, |o| o.completeness = "Чехол, зарядка".into()).await;
        order_with(&t.core, |o| o.completeness = "чехол".into()).await;
        order_with(&t.core, |o| o.completeness = "Чехол, чехол".into()).await;
        let got = t
            .core
            .dictionary_usage(&admin(), DictionaryKind::Completeness)
            .await?;
        let got: std::collections::HashMap<_, _> = usage(&got).into_iter().collect();
        assert_eq!(got.get(&cover), Some(&3), "{}", t.label);
        assert_eq!(got.get(&charger), Some(&1), "{}", t.label);
    }
    Ok(())
}

#[tokio::test]
async fn dictionary_usage_counts_clients_for_source() -> Result<(), CoreError> {
    for t in cores().await {
        for (i, source) in ["Интернет", "интернет", "Авито"].into_iter().enumerate()
        {
            t.core
                .create_client(
                    &admin(),
                    NewClient {
                        full_name: format!("Клиент {i}"),
                        phones: common::phones(&[&format!("+7 921 000-00-0{i}")]),
                        source: source.into(),
                        ..Default::default()
                    },
                )
                .await?;
        }
        let ad = item_id(&t.core, DictionaryKind::ClientSource, "Интернет").await;
        let got = t
            .core
            .dictionary_usage(&admin(), DictionaryKind::ClientSource)
            .await?;
        let got: std::collections::HashMap<_, _> = usage(&got).into_iter().collect();
        assert_eq!(got.get(&ad), Some(&2), "{}", t.label);
    }
    Ok(())
}

#[tokio::test]
async fn warranty_usage_uses_warranty_text() -> Result<(), CoreError> {
    for t in cores().await {
        order_with(&t.core, |o| o.warranty = "30 дней".into()).await;
        let id = item_id(&t.core, DictionaryKind::Warranty, "30 дней").await;
        let got = t
            .core
            .dictionary_usage(&admin(), DictionaryKind::Warranty)
            .await?;
        assert_eq!(usage(&got), vec![(id, 1)], "{}", t.label);
    }
    Ok(())
}

#[tokio::test]
async fn status_and_service_point_usage() -> Result<(), CoreError> {
    for t in cores().await {
        let sp = t.core.list_service_points(true).await?;
        let sp_id = match sp.first() {
            Some(p) => p.id,
            None => t.core.create_service_point(&admin(), "Центр").await?.id,
        };
        order_with(&t.core, |o| o.service_point_id = Some(sp_id)).await;
        order_with(&t.core, |o| o.service_point_id = Some(sp_id)).await;
        order_with(&t.core, |_| {}).await;
        let gone = order_with(&t.core, |o| o.service_point_id = Some(sp_id)).await;
        delete(&t.core, gone).await;
        let points = t.core.service_point_usage(&admin()).await?;
        assert_eq!(usage(&points), vec![(sp_id, 2)], "{}", t.label);
        let statuses = t.core.status_usage(&admin()).await?;
        assert_eq!(statuses.len(), 1, "{}", t.label);
        assert_eq!(statuses[0].count, 3, "{}", t.label);
    }
    Ok(())
}

#[tokio::test]
async fn next_order_number_reads_the_sequence() -> Result<(), CoreError> {
    for t in cores().await {
        t.core.set_next_order_number(&admin(), 500).await?;
        assert_eq!(
            t.core.next_order_number(&admin()).await?,
            500,
            "{}",
            t.label
        );
        order_with(&t.core, |_| {}).await;
        assert_eq!(
            t.core.next_order_number(&admin()).await?,
            501,
            "{}",
            t.label
        );
    }
    Ok(())
}

#[tokio::test]
async fn new_reads_need_settings_manage() -> Result<(), CoreError> {
    for t in cores().await {
        let clerk = actor_with(&[Permission::OrdersCreate]);
        let forbidden = |r: Result<(), CoreError>| matches!(r, Err(CoreError::Forbidden(_)));
        assert!(forbidden(
            t.core
                .dictionary_usage(&clerk, DictionaryKind::Brand)
                .await
                .map(drop)
        ));
        assert!(forbidden(t.core.status_usage(&clerk).await.map(drop)));
        assert!(forbidden(
            t.core.service_point_usage(&clerk).await.map(drop)
        ));
        assert!(forbidden(t.core.next_order_number(&clerk).await.map(drop)));
        assert!(forbidden(t.core.about_totals(&clerk).await.map(drop)));
        assert!(forbidden(
            t.core
                .update_dictionary_item(&clerk, 1, DictionaryItemPatch::default())
                .await
                .map(drop)
        ));
    }
    Ok(())
}
