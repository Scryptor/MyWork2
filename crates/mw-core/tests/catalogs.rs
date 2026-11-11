mod common;

use common::{actor_with, admin, assert_validation, cores, count_audit};
use mw_core::{CoreError, NewStatus, StatusPatch};
use mw_domain::dictionary::DictionaryKind;
use mw_domain::permission::Permission;
use mw_domain::status::StatusKind;

#[tokio::test]
async fn statuses_seeded_and_last_issued_status_is_protected() -> Result<(), CoreError> {
    for t in cores().await {
        let all = t.core.list_statuses(false).await?;
        assert_eq!(all.len(), 8, "{}", t.label);
        let issued = all.iter().find(|s| s.kind == StatusKind::Issued).unwrap();
        let err = t
            .core
            .update_status(
                &admin(),
                issued.id,
                StatusPatch {
                    archived: Some(true),
                    ..Default::default()
                },
            )
            .await;
        assert_validation(err, &[("archived", "last_issued_status")]);
        let err = t
            .core
            .update_status(
                &admin(),
                issued.id,
                StatusPatch {
                    kind: Some(StatusKind::Open),
                    ..Default::default()
                },
            )
            .await;
        assert_validation(err, &[("kind", "last_issued_status")]);
        let second = t
            .core
            .create_status(
                &admin(),
                NewStatus {
                    name: "Выдан курьером".into(),
                    kind: StatusKind::Issued,
                    color: "gray".into(),
                },
            )
            .await?;
        t.core
            .update_status(
                &admin(),
                issued.id,
                StatusPatch {
                    archived: Some(true),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(
            t.core.list_statuses(false).await?.last().unwrap().id,
            second.id
        );
        assert_validation(
            t.core
                .create_status(
                    &admin(),
                    NewStatus {
                        name: " готов ".into(),
                        kind: StatusKind::Open,
                        color: "x".into(),
                    },
                )
                .await,
            &[("name", "duplicate")],
        );
        assert_validation(
            t.core
                .create_status(
                    &admin(),
                    NewStatus {
                        name: "  ".into(),
                        kind: StatusKind::Open,
                        color: "x".into(),
                    },
                )
                .await,
            &[("name", "required")],
        );
        assert_eq!(count_audit(&t.core, "status.created").await, 1);
        assert_eq!(count_audit(&t.core, "status.updated").await, 1);
    }
    Ok(())
}

#[tokio::test]
async fn reorder_requires_full_permutation() -> Result<(), CoreError> {
    for t in cores().await {
        let ids: Vec<i64> = t
            .core
            .list_statuses(true)
            .await?
            .iter()
            .map(|s| s.id)
            .rev()
            .collect();
        assert_validation(
            t.core.reorder_statuses(&admin(), &ids[1..]).await,
            &[("ordered_ids", "invalid")],
        );
        t.core.reorder_statuses(&admin(), &ids).await?;
        assert_eq!(t.core.list_statuses(true).await?[0].name, "Выдан");
        assert_eq!(count_audit(&t.core, "statuses.reordered").await, 1);
    }
    Ok(())
}

#[tokio::test]
async fn dictionary_add_from_intake_and_manage_from_settings() -> Result<(), CoreError> {
    for t in cores().await {
        let clerk = actor_with(&[Permission::OrdersCreate]);
        let item = t
            .core
            .add_dictionary_item(&clerk, DictionaryKind::Brand, "  Nothing ")
            .await?;
        assert_eq!(item.value, "Nothing");
        assert_validation(
            t.core
                .add_dictionary_item(&clerk, DictionaryKind::Brand, "nothing")
                .await,
            &[("value", "duplicate")],
        );
        assert!(matches!(
            t.core.delete_dictionary_item(&clerk, item.id).await,
            Err(CoreError::Forbidden(Permission::SettingsManage))
        ));
        t.core
            .rename_dictionary_item(&admin(), item.id, "Nothing Phone")
            .await?;
        t.core.delete_dictionary_item(&admin(), item.id).await?;
        assert!(
            !t.core
                .list_dictionary(DictionaryKind::Brand)
                .await?
                .iter()
                .any(|i| i.id == item.id)
        );
        assert_eq!(count_audit(&t.core, "dictionary.added").await, 1);
        assert_eq!(count_audit(&t.core, "dictionary.renamed").await, 1);
        assert_eq!(count_audit(&t.core, "dictionary.deleted").await, 1);
    }
    Ok(())
}

#[tokio::test]
async fn service_points_archive_hides_from_default_list() -> Result<(), CoreError> {
    for t in cores().await {
        let p = t.core.create_service_point(&admin(), "Древлянка").await?;
        t.core
            .update_service_point(&admin(), p.id, None, Some(true))
            .await?;
        assert!(t.core.list_service_points(false).await?.is_empty());
        assert_eq!(t.core.list_service_points(true).await?.len(), 1);
        assert_eq!(count_audit(&t.core, "service_point.updated").await, 1);
    }
    Ok(())
}
