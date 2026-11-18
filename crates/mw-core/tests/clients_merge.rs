mod common;

use common::{
    actor_with, admin, assert_validation, cores, count_audit, ids, make_client, make_order_for,
};
use mw_core::{ClientPhone, CoreError, MergeClients, OrderFilter, OrderSort, VersionedId};
use mw_domain::permission::Permission;

fn merge(target: &mw_core::Client, sources: &[(i64, i64)]) -> MergeClients {
    MergeClients {
        target_id: target.id,
        target_version: target.version,
        sources: sources
            .iter()
            .map(|&(id, version)| VersionedId { id, version })
            .collect(),
        full_name: target.full_name.clone(),
        address: String::new(),
        note: String::new(),
        source: String::new(),
        blacklisted: false,
    }
}

#[tokio::test]
async fn merge_moves_orders_merges_phones_and_keeps_snapshots() -> Result<(), CoreError> {
    for t in cores().await {
        let a = t
            .core
            .create_client(
                &admin(),
                mw_core::NewClient {
                    full_name: "Анна Ким".into(),
                    phones: vec![ClientPhone {
                        phone: "8 911 000-00-01".into(),
                        note: "жена".into(),
                    }],
                    ..Default::default()
                },
            )
            .await?;
        let b = t
            .core
            .create_client(
                &admin(),
                mw_core::NewClient {
                    full_name: "Ким Анна".into(),
                    phones: vec![
                        ClientPhone {
                            phone: "+7 911 000-00-01".into(),
                            note: "дубль".into(),
                        },
                        ClientPhone {
                            phone: "8 911 000-00-02".into(),
                            note: "раб".into(),
                        },
                    ],
                    ..Default::default()
                },
            )
            .await?;
        make_order_for(&t, a.id).await;
        make_order_for(&t, b.id).await;
        make_order_for(&t, b.id).await;

        let merged = t
            .core
            .merge_clients(&admin(), merge(&a, &[(b.id, b.version)]))
            .await?;
        assert_eq!(merged.id, a.id);
        assert_eq!(merged.version, a.version + 1);
        assert_eq!(
            merged.phones,
            vec![
                ClientPhone {
                    phone: "89110000001".into(),
                    note: "жена".into()
                },
                ClientPhone {
                    phone: "89110000002".into(),
                    note: "раб".into()
                },
            ],
            "{}",
            t.label
        );
        assert!(matches!(
            t.core.get_client(&admin(), b.id).await,
            Err(CoreError::NotFound { .. })
        ));
        let orders = t
            .core
            .list_orders(
                &admin(),
                &OrderFilter {
                    client_id: Some(a.id),
                    ..Default::default()
                },
                OrderSort::default(),
                50,
                0,
            )
            .await?;
        assert_eq!(orders.total, 3, "{}", t.label);
        assert_eq!(count_audit(&t.core, "client.merged").await, 1);
        let log = common::audit_json(&t.core, "client.merged").await;
        assert!(log.contains("Ким Анна"), "{log}");
        assert!(log.contains("89110000002"), "{log}");
        // найденный по телефону источника клиент — цель
        assert_eq!(
            ids(t
                .core
                .find_clients_by_phone(&admin(), "89110000002")
                .await?),
            vec![a.id]
        );
    }
    Ok(())
}

#[tokio::test]
async fn merge_with_a_stale_version_changes_nothing() -> Result<(), CoreError> {
    for t in cores().await {
        let a = make_client(&t, "Анна", &["89110000001"]).await;
        let b = make_client(&t, "Анна", &["89110000002"]).await;
        make_order_for(&t, b.id).await;
        let res = t
            .core
            .merge_clients(&admin(), merge(&a, &[(b.id, b.version - 1)]))
            .await;
        assert!(
            matches!(res, Err(CoreError::Conflict { id, .. }) if id == b.id),
            "{} {res:?}",
            t.label
        );
        let stale_target = MergeClients {
            target_version: a.version + 5,
            ..merge(&a, &[(b.id, b.version)])
        };
        assert!(matches!(
            t.core.merge_clients(&admin(), stale_target).await,
            Err(CoreError::Conflict { id, .. }) if id == a.id
        ));
        assert_eq!(t.core.get_client(&admin(), b.id).await?.version, b.version);
        assert_eq!(t.core.get_client(&admin(), a.id).await?, a);
        assert_eq!(count_audit(&t.core, "client.merged").await, 0);
    }
    Ok(())
}

#[tokio::test]
async fn merge_validates_sources_and_name() -> Result<(), CoreError> {
    for t in cores().await {
        let a = make_client(&t, "Анна", &["89110000001"]).await;
        let b = make_client(&t, "Борис", &[]).await;
        let (bid, bv) = (b.id, b.version);
        assert_validation(
            t.core.merge_clients(&admin(), merge(&a, &[])).await,
            &[("sources", "invalid")],
        );
        assert_validation(
            t.core
                .merge_clients(&admin(), merge(&a, &[(bid, bv), (bid, bv)]))
                .await,
            &[("sources", "invalid")],
        );
        assert_validation(
            t.core
                .merge_clients(&admin(), merge(&a, &[(a.id, a.version)]))
                .await,
            &[("sources", "invalid")],
        );
        // имя пустое и телефонов нет → full_name обязательно
        let c = make_client(&t, "Вера", &[]).await;
        let d = make_client(&t, "Дима", &[]).await;
        let mut m = merge(&c, &[(d.id, d.version)]);
        m.full_name = " ".into();
        assert_validation(
            t.core.merge_clients(&admin(), m).await,
            &[("full_name", "required")],
        );
    }
    Ok(())
}

#[tokio::test]
async fn merge_needs_clients_merge_permission() -> Result<(), CoreError> {
    for t in cores().await {
        let a = make_client(&t, "Анна", &[]).await;
        let b = make_client(&t, "Анна", &[]).await;
        assert!(matches!(
            t.core
                .merge_clients(
                    &actor_with(&[Permission::ClientsEdit]),
                    merge(&a, &[(b.id, b.version)])
                )
                .await,
            Err(CoreError::Forbidden(Permission::ClientsMerge))
        ));
        assert!(t.core.get_client(&admin(), b.id).await.is_ok());
    }
    Ok(())
}
