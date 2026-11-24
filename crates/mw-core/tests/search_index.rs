#![allow(clippy::unwrap_used)]
mod common;

use common::{
    actor_with, admin, cores, count_audit, ids, phones, set_search_text_directly,
    set_setting_directly, setting_value,
};
use mw_core::{Core, CoreError, NewClient, NewStockItem, SEARCH_INDEX_VERSION, StockFilter};
use mw_domain::permission::Permission;
use sea_orm::EntityTrait;
use serde_json::json;

#[tokio::test]
async fn open_reindexes_when_version_differs() -> Result<(), CoreError> {
    for t in cores().await {
        let c = t
            .core
            .create_client(
                &admin(),
                NewClient {
                    full_name: "Пётр".into(),
                    phones: phones(&["8 911 765 43 21"]),
                    ..Default::default()
                },
            )
            .await?;
        set_search_text_directly(&t.core, c.id, "мусор").await;
        set_setting_directly(&t.core, "search_index_version", json!(1)).await;
        let reopened = Core::open(t.core.db().clone(), t.clock.clone(), "UTC").await?;
        assert_eq!(
            ids(reopened
                .find_clients(&admin(), "петр 911", 50, 0)
                .await?
                .items),
            vec![c.id],
            "{}",
            t.label
        );
        assert_eq!(
            common::setting_value(&reopened, "search_index_version").await,
            json!(SEARCH_INDEX_VERSION)
        );
    }
    Ok(())
}

#[tokio::test]
async fn decomposed_input_is_found_both_ways() -> Result<(), CoreError> {
    for t in cores().await {
        let c = t
            .core
            .create_client(
                &admin(),
                NewClient {
                    full_name: "Се\u{0308}мин".into(),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(
            ids(t.core.find_clients(&admin(), "Сёмин", 50, 0).await?.items),
            vec![c.id],
            "{}",
            t.label
        );
        assert_eq!(
            ids(t.core.find_clients(&admin(), "семин", 50, 0).await?.items),
            vec![c.id],
            "{}",
            t.label
        );
    }
    Ok(())
}

#[tokio::test]
async fn manual_reindex_needs_settings_right_and_audits() -> Result<(), CoreError> {
    for t in cores().await {
        assert!(matches!(
            t.core.reindex_search(&actor_with(&[])).await,
            Err(CoreError::Forbidden(Permission::SettingsManage))
        ));
        let stats = t.core.reindex_search(&admin()).await?;
        assert_eq!((stats.clients, stats.orders), (0, 0), "{}", t.label);
        assert_eq!(count_audit(&t.core, "search.reindexed").await, 1);
        assert_eq!(
            setting_value(&t.core, "search_index_version").await,
            json!(SEARCH_INDEX_VERSION),
            "{}",
            t.label
        );
    }
    Ok(())
}

#[tokio::test]
async fn open_fills_order_sort_keys_of_old_rows() -> Result<(), CoreError> {
    for t in cores().await {
        let yozhikov = common::make_master(&t.core, "Ёжиков").await;
        let o = common::create_order_with(&t, |n| {
            n.brand = "Samsung".into();
            n.master_user_id = Some(yozhikov);
        })
        .await;
        common::set_order_keys_directly(&t, o.id, None, "").await;
        set_setting_directly(&t.core, "search_index_version", json!(2)).await;
        let reopened = Core::open(t.core.db().clone(), t.clock.clone(), "UTC").await?;
        let row = mw_db::entity::order::Entity::find_by_id(o.id)
            .one(reopened.db())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            (
                row.brand_key.as_str(),
                row.master_key.as_str(),
                row.device_type_key.as_str()
            ),
            ("samsung", "ежиков", "телефон"),
            "{}",
            t.label
        );
        assert_eq!(row.model_key, "a52");
    }
    Ok(())
}

/// Портит индекс позиции склада в обход `Core` (имитация устаревших правил).
async fn spoil_stock_index(core: &Core, id: i64) {
    use sea_orm::{ActiveModelTrait, Set};
    mw_db::entity::stock_item::ActiveModel {
        id: Set(id),
        search_text: Set("мусор".into()),
        name_key: Set("мусор".into()),
        ..Default::default()
    }
    .update(core.db())
    .await
    .unwrap();
}

fn stock_query(q: &str) -> StockFilter {
    StockFilter {
        query: q.into(),
        ..Default::default()
    }
}

#[tokio::test]
async fn reindex_counts_stock_items() -> Result<(), CoreError> {
    for t in cores().await {
        let it = t
            .core
            .create_stock_item(
                &admin(),
                NewStockItem {
                    part_type: "Дисплей".into(),
                    brand: "Samsung".into(),
                    ..Default::default()
                },
            )
            .await?;
        let found = async |core: &Core| {
            core.list_stock(&admin(), &stock_query("дисплей samsung"), 50, 0)
                .await
                .unwrap()
                .items
                .into_iter()
                .map(|i| i.id)
                .collect::<Vec<_>>()
        };
        assert_eq!(found(&t.core).await, vec![it.id], "{}", t.label);
        assert_eq!(t.core.reindex_search(&admin()).await?.stock_items, 0);

        spoil_stock_index(&t.core, it.id).await;
        assert!(found(&t.core).await.is_empty(), "{}", t.label);
        let stats = t.core.reindex_search(&admin()).await?;
        assert_eq!(
            (stats.clients, stats.orders, stats.stock_items),
            (0, 0, 1),
            "{}",
            t.label
        );
        assert_eq!(found(&t.core).await, vec![it.id]);
        let row = mw_db::entity::stock_item::Entity::find_by_id(it.id)
            .one(t.core.db())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.name_key, "дисплей samsung");
        assert_eq!(
            common::audit_details(&t.core, "search.reindexed").await[1],
            json!({ "clients": 0, "orders": 0, "stock_items": 1 })
        );

        // Индекс прежней версии (4 — без склада) пересчитывается при открытии.
        spoil_stock_index(&t.core, it.id).await;
        set_setting_directly(&t.core, "search_index_version", json!(4)).await;
        let reopened = Core::open(t.core.db().clone(), t.clock.clone(), "UTC").await?;
        assert_eq!(found(&reopened).await, vec![it.id], "{}", t.label);
        assert_eq!(
            setting_value(&reopened, "search_index_version").await,
            json!(SEARCH_INDEX_VERSION)
        );
    }
    Ok(())
}
