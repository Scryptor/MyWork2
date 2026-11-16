mod common;

use common::{
    actor_with, admin, cores, count_audit, ids, phones, set_search_text_directly,
    set_setting_directly, setting_value,
};
use mw_core::{Core, CoreError, NewClient, SEARCH_INDEX_VERSION};
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
        let o = common::create_order_with(&t, |n| {
            n.brand = "Samsung".into();
            n.master = "Ёжиков".into();
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
