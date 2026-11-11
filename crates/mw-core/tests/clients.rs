mod common;

use common::{actor_with, admin, assert_validation, cores, count_audit, ids};
use mw_core::{ClientPatch, CoreError, NewClient};
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
                    phone: "+7 (921) 123-45-67".into(),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(
            (c.full_name.as_str(), c.phone.as_str(), c.version),
            ("ПЁТР ИВАНОВ", "79211234567", 1)
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
                    phone: Some("8 900 111 22 33".into()),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!((v3.version, v3.phone.as_str()), (3, "89001112233"));
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
                    phone: "89211234567".into(),
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
