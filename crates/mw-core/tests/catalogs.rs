mod common;

use common::{actor_with, admin, assert_validation, cores, count_audit};
use mw_core::{CoreError, NewStatus, ServicePointPatch, StatusPatch};
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
                        color: "gray".into(),
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
                        color: "gray".into(),
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
            .update_service_point(
                &admin(),
                p.id,
                ServicePointPatch {
                    archived: Some(true),
                    ..Default::default()
                },
            )
            .await?;
        assert!(t.core.list_service_points(false).await?.is_empty());
        assert_eq!(t.core.list_service_points(true).await?.len(), 1);
        assert_eq!(count_audit(&t.core, "service_point.updated").await, 1);
    }
    Ok(())
}

fn new_status(name: &str) -> NewStatus {
    NewStatus {
        name: name.into(),
        kind: StatusKind::Open,
        color: "gray".into(),
    }
}

#[tokio::test]
async fn new_status_is_appended_last() -> Result<(), CoreError> {
    for t in cores().await {
        let max = t
            .core
            .list_statuses(true)
            .await?
            .iter()
            .map(|s| s.sort_order)
            .max()
            .unwrap();
        let created = t.core.create_status(&admin(), new_status("Новый")).await?;
        assert_eq!(created.sort_order, max + 1, "{}", t.label);
        assert_eq!(
            t.core.list_statuses(true).await?.last().unwrap().id,
            created.id
        );
    }
    Ok(())
}

#[tokio::test]
async fn status_not_found_and_forbidden() -> Result<(), CoreError> {
    for t in cores().await {
        let clerk = actor_with(&[Permission::OrdersCreate]);
        assert!(matches!(
            t.core
                .update_status(&admin(), 999_999, StatusPatch::default())
                .await,
            Err(CoreError::NotFound { .. })
        ));
        assert!(matches!(
            t.core.create_status(&clerk, new_status("Новый")).await,
            Err(CoreError::Forbidden(Permission::SettingsManage))
        ));
        let first = t.core.list_statuses(true).await?[0].id;
        assert!(matches!(
            t.core
                .update_status(&clerk, first, StatusPatch::default())
                .await,
            Err(CoreError::Forbidden(Permission::SettingsManage))
        ));
        let ids: Vec<i64> = t
            .core
            .list_statuses(true)
            .await?
            .iter()
            .map(|s| s.id)
            .collect();
        assert!(matches!(
            t.core.reorder_statuses(&clerk, &ids).await,
            Err(CoreError::Forbidden(Permission::SettingsManage))
        ));
    }
    Ok(())
}

#[tokio::test]
async fn status_rename_to_own_name_and_collision() -> Result<(), CoreError> {
    for t in cores().await {
        let all = t.core.list_statuses(true).await?;
        let (a, b) = (&all[0], &all[1]);
        let same = t
            .core
            .update_status(
                &admin(),
                a.id,
                StatusPatch {
                    name: Some(a.name.to_uppercase()),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(same.name, a.name.to_uppercase());
        assert_validation(
            t.core
                .update_status(
                    &admin(),
                    a.id,
                    StatusPatch {
                        name: Some(b.name.clone()),
                        ..Default::default()
                    },
                )
                .await,
            &[("name", "duplicate")],
        );
    }
    Ok(())
}

#[tokio::test]
async fn reorder_rejects_duplicated_ids() -> Result<(), CoreError> {
    for t in cores().await {
        let mut ids: Vec<i64> = t
            .core
            .list_statuses(true)
            .await?
            .iter()
            .map(|s| s.id)
            .collect();
        ids[1] = ids[0];
        assert_validation(
            t.core.reorder_statuses(&admin(), &ids).await,
            &[("ordered_ids", "invalid")],
        );

        let items = t.core.list_dictionary(DictionaryKind::Color).await?;
        let mut ids: Vec<i64> = items.iter().map(|i| i.id).collect();
        ids[1] = ids[0];
        assert_validation(
            t.core
                .reorder_dictionary(&admin(), DictionaryKind::Color, &ids)
                .await,
            &[("ordered_ids", "invalid")],
        );
        // id из другого справочника не подходит
        let brand = t.core.list_dictionary(DictionaryKind::Brand).await?[0].id;
        let mut ids: Vec<i64> = items.iter().map(|i| i.id).collect();
        ids[0] = brand;
        assert_validation(
            t.core
                .reorder_dictionary(&admin(), DictionaryKind::Color, &ids)
                .await,
            &[("ordered_ids", "invalid")],
        );

        let a = t.core.create_service_point(&admin(), "А").await?;
        let b = t.core.create_service_point(&admin(), "Б").await?;
        assert_validation(
            t.core.reorder_service_points(&admin(), &[a.id, a.id]).await,
            &[("ordered_ids", "invalid")],
        );
        assert_validation(
            t.core.reorder_service_points(&admin(), &[a.id]).await,
            &[("ordered_ids", "invalid")],
        );
        let _ = b;
    }
    Ok(())
}

#[tokio::test]
async fn dictionary_reorder_changes_list_order() -> Result<(), CoreError> {
    for t in cores().await {
        let items = t.core.list_dictionary(DictionaryKind::Color).await?;
        let ids: Vec<i64> = items.iter().map(|i| i.id).rev().collect();
        t.core
            .reorder_dictionary(&admin(), DictionaryKind::Color, &ids)
            .await?;
        let after: Vec<i64> = t
            .core
            .list_dictionary(DictionaryKind::Color)
            .await?
            .iter()
            .map(|i| i.id)
            .collect();
        assert_eq!(after, ids, "{}", t.label);
        assert_eq!(count_audit(&t.core, "dictionary.reordered").await, 1);
        let clerk = actor_with(&[Permission::OrdersCreate]);
        assert!(matches!(
            t.core
                .reorder_dictionary(&clerk, DictionaryKind::Color, &ids)
                .await,
            Err(CoreError::Forbidden(Permission::SettingsManage))
        ));
    }
    Ok(())
}

#[tokio::test]
async fn dictionary_rename_edge_cases() -> Result<(), CoreError> {
    for t in cores().await {
        let items = t.core.list_dictionary(DictionaryKind::Brand).await?;
        let (a, b) = (&items[0], &items[1]);
        // то же имя и смена только регистра — не дубль самого себя
        let same = t
            .core
            .rename_dictionary_item(&admin(), a.id, &a.value)
            .await?;
        assert_eq!(same.value, a.value);
        let upper = t
            .core
            .rename_dictionary_item(&admin(), a.id, &a.value.to_uppercase())
            .await?;
        assert_eq!(upper.value, a.value.to_uppercase());
        // столкновение с другим значением того же справочника
        assert_validation(
            t.core
                .rename_dictionary_item(&admin(), a.id, &b.value)
                .await,
            &[("value", "duplicate")],
        );
        assert_validation(
            t.core.rename_dictionary_item(&admin(), a.id, "  ").await,
            &[("value", "required")],
        );
        assert!(matches!(
            t.core.rename_dictionary_item(&admin(), 999_999, "X").await,
            Err(CoreError::NotFound { .. })
        ));
        assert!(matches!(
            t.core.delete_dictionary_item(&admin(), 999_999).await,
            Err(CoreError::NotFound { .. })
        ));
        let clerk = actor_with(&[Permission::OrdersCreate]);
        assert!(matches!(
            t.core.rename_dictionary_item(&clerk, a.id, "Y").await,
            Err(CoreError::Forbidden(Permission::SettingsManage))
        ));
        // то же значение допустимо в другом справочнике
        t.core
            .add_dictionary_item(&admin(), DictionaryKind::Color, &b.value)
            .await?;
        // сводки журнала по-русски
        assert!(
            common::audit_json(&t.core, "dictionary.renamed")
                .await
                .contains("Бренды")
        );
    }
    Ok(())
}

#[tokio::test]
async fn service_point_validation_not_found_and_forbidden() -> Result<(), CoreError> {
    for t in cores().await {
        let p = t.core.create_service_point(&admin(), "Древлянка").await?;
        assert_validation(
            t.core.create_service_point(&admin(), " древлянка ").await,
            &[("name", "duplicate")],
        );
        assert_validation(
            t.core.create_service_point(&admin(), "  ").await,
            &[("name", "required")],
        );
        let other = t.core.create_service_point(&admin(), "Центр").await?;
        assert_eq!(other.sort_order, p.sort_order + 1);
        // собственное имя и смена регистра допустимы, чужое — дубль
        t.core
            .update_service_point(
                &admin(),
                p.id,
                ServicePointPatch {
                    name: Some("Древлянка".into()),
                    ..Default::default()
                },
            )
            .await?;
        let renamed = t
            .core
            .update_service_point(
                &admin(),
                p.id,
                ServicePointPatch {
                    name: Some("ДРЕВЛЯНКА".into()),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(renamed.name, "ДРЕВЛЯНКА");
        assert_validation(
            t.core
                .update_service_point(
                    &admin(),
                    p.id,
                    ServicePointPatch {
                        name: Some("центр".into()),
                        ..Default::default()
                    },
                )
                .await,
            &[("name", "duplicate")],
        );
        assert!(matches!(
            t.core
                .update_service_point(
                    &admin(),
                    999_999,
                    ServicePointPatch {
                        archived: Some(true),
                        ..Default::default()
                    }
                )
                .await,
            Err(CoreError::NotFound { .. })
        ));
        let clerk = actor_with(&[Permission::OrdersCreate]);
        assert!(matches!(
            t.core.create_service_point(&clerk, "Новая").await,
            Err(CoreError::Forbidden(Permission::SettingsManage))
        ));
        assert!(matches!(
            t.core
                .update_service_point(
                    &clerk,
                    p.id,
                    ServicePointPatch {
                        archived: Some(true),
                        ..Default::default()
                    }
                )
                .await,
            Err(CoreError::Forbidden(Permission::SettingsManage))
        ));
        assert!(matches!(
            t.core
                .reorder_service_points(&clerk, &[p.id, other.id])
                .await,
            Err(CoreError::Forbidden(Permission::SettingsManage))
        ));
        t.core
            .reorder_service_points(&admin(), &[other.id, p.id])
            .await?;
        let order: Vec<i64> = t
            .core
            .list_service_points(true)
            .await?
            .iter()
            .map(|s| s.id)
            .collect();
        assert_eq!(order, [other.id, p.id], "{}", t.label);
        assert_eq!(count_audit(&t.core, "service_points.reordered").await, 1);
    }
    Ok(())
}

#[tokio::test]
async fn unchanged_names_skip_the_collision_check_for_legacy_data() -> Result<(), CoreError> {
    use mw_db::entity::{service_point, status};
    use sea_orm::{ActiveModelTrait, ActiveValue::NotSet, Set};
    for t in cores().await {
        let ready = t
            .core
            .list_statuses(true)
            .await?
            .into_iter()
            .find(|s| s.name == "Готов")
            .unwrap();
        status::ActiveModel {
            id: NotSet,
            name: Set("готов".into()),
            kind: Set("open".into()),
            color: Set("gray".into()),
            sort_order: Set(100),
            archived: Set(false),
        }
        .insert(t.core.db())
        .await
        .unwrap();
        let updated = t
            .core
            .update_status(
                &admin(),
                ready.id,
                StatusPatch {
                    name: Some("Готов".into()),
                    color: Some("pink".into()),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(
            (updated.name.as_str(), updated.color.as_str()),
            ("Готов", "pink"),
            "{}",
            t.label
        );

        let mut ids = Vec::new();
        for (i, name) in ["Центр", "центр"].into_iter().enumerate() {
            let row = service_point::ActiveModel {
                id: NotSet,
                name: Set(name.into()),
                sort_order: Set(i as i32),
                archived: Set(false),
            }
            .insert(t.core.db())
            .await
            .unwrap();
            ids.push(row.id);
        }
        let point = t
            .core
            .update_service_point(
                &admin(),
                ids[0],
                ServicePointPatch {
                    name: Some("Центр".into()),
                    archived: Some(true),
                },
            )
            .await?;
        assert!(point.archived, "{}", t.label);
    }
    Ok(())
}

#[tokio::test]
async fn status_color_must_be_a_palette_name() -> Result<(), CoreError> {
    use mw_db::entity::status;
    use sea_orm::{ActiveModelTrait, Set};
    for t in cores().await {
        let new = |color: &str| NewStatus {
            name: "Цветной".into(),
            kind: StatusKind::Open,
            color: color.into(),
        };
        assert_validation(
            t.core.create_status(&admin(), new("#ff0000")).await,
            &[("color", "invalid")],
        );
        let ok = t.core.create_status(&admin(), new("pink")).await?;
        assert_eq!(ok.color, "pink", "{}", t.label);
        assert_validation(
            t.core
                .update_status(
                    &admin(),
                    ok.id,
                    StatusPatch {
                        color: Some("magenta".into()),
                        ..Default::default()
                    },
                )
                .await,
            &[("color", "invalid")],
        );
        status::ActiveModel {
            id: Set(ok.id),
            color: Set("#123456".into()),
            ..Default::default()
        }
        .update(t.core.db())
        .await
        .unwrap();
        let renamed = t
            .core
            .update_status(
                &admin(),
                ok.id,
                StatusPatch {
                    name: Some("Другой".into()),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(renamed.color, "#123456", "{}", t.label);
    }
    Ok(())
}
