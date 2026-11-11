mod common;

use common::*;
use mw_core::{
    ClientPatch, CoreError, NewClient, NewRole, NewUser, RolePatch, ServicePointPatch,
    SettingsPatch, StatusPatch, UserPatch,
};
use mw_domain::dictionary::DictionaryKind;
use mw_domain::permission::PermissionSet;
use serde_json::json;

#[tokio::test]
async fn same_values_change_nothing_anywhere() -> Result<(), CoreError> {
    for t in cores().await {
        let s = t.core.settings();
        t.core
            .update_settings(&admin(), SettingsPatch::default())
            .await?;
        t.core
            .update_settings(
                &admin(),
                SettingsPatch {
                    master_percent: Some(s.master_percent),
                    timezone: Some(s.timezone.clone()),
                    ..Default::default()
                },
            )
            .await?;
        let st = t.core.list_statuses(false).await?.remove(0);
        assert_eq!(
            t.core
                .update_status(
                    &admin(),
                    st.id,
                    StatusPatch {
                        name: Some(format!(" {} ", st.name)),
                        color: Some(st.color.clone()),
                        ..Default::default()
                    }
                )
                .await?,
            st
        );
        let status_ids: Vec<i64> = t
            .core
            .list_statuses(true)
            .await?
            .iter()
            .map(|s| s.id)
            .collect();
        t.core.reorder_statuses(&admin(), &status_ids).await?;
        let p = t.core.create_service_point(&admin(), "Центр").await?;
        assert_eq!(
            t.core
                .update_service_point(
                    &admin(),
                    p.id,
                    ServicePointPatch {
                        name: Some("Центр ".into()),
                        archived: Some(false)
                    }
                )
                .await?,
            p
        );
        t.core.reorder_service_points(&admin(), &[p.id]).await?;
        let brands = t.core.list_dictionary(DictionaryKind::Brand).await?;
        assert_eq!(
            t.core
                .rename_dictionary_item(&admin(), brands[0].id, &format!("{} ", brands[0].value))
                .await?,
            brands[0]
        );
        t.core
            .reorder_dictionary(
                &admin(),
                DictionaryKind::Brand,
                &brands.iter().map(|i| i.id).collect::<Vec<_>>(),
            )
            .await?;
        for action in [
            "settings.updated",
            "status.updated",
            "statuses.reordered",
            "service_point.updated",
            "service_points.reordered",
            "dictionary.renamed",
            "dictionary.reordered",
        ] {
            assert_eq!(
                count_audit(&t.core, action).await,
                0,
                "{} {action}",
                t.label
            );
        }
    }
    Ok(())
}

#[tokio::test]
async fn stale_version_matters_only_when_something_changes() -> Result<(), CoreError> {
    for t in cores().await {
        let c = t
            .core
            .create_client(
                &admin(),
                NewClient {
                    full_name: "Анна".into(),
                    ..Default::default()
                },
            )
            .await?;
        let c2 = t
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
        assert_eq!(
            t.core
                .update_client(
                    &admin(),
                    c.id,
                    1,
                    ClientPatch {
                        address: Some(" Ленина, 1 ".into()),
                        ..Default::default()
                    }
                )
                .await?,
            c2
        );
        assert!(matches!(
            t.core
                .update_client(
                    &admin(),
                    c.id,
                    1,
                    ClientPatch {
                        address: Some("Мира, 2".into()),
                        ..Default::default()
                    }
                )
                .await,
            Err(CoreError::Conflict {
                current_version: 2,
                ..
            })
        ));
        let role = role_id_by_name(&t.core, "Администратор").await;
        let u = t
            .core
            .create_user(
                &admin(),
                NewUser {
                    login: "ирина".into(),
                    display_name: String::new(),
                    role_id: role,
                    password: "1234".into(),
                },
            )
            .await?;
        let u2 = t
            .core
            .update_user(
                &admin(),
                u.id,
                1,
                UserPatch {
                    display_name: Some("Ирина".into()),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(
            t.core
                .update_user(
                    &admin(),
                    u.id,
                    1,
                    UserPatch {
                        display_name: Some("Ирина".into()),
                        active: Some(true),
                        ..Default::default()
                    }
                )
                .await?,
            u2
        );
        assert!(matches!(
            t.core
                .update_user(
                    &admin(),
                    u.id,
                    1,
                    UserPatch {
                        display_name: Some("И.".into()),
                        ..Default::default()
                    }
                )
                .await,
            Err(CoreError::Conflict {
                current_version: 2,
                ..
            })
        ));
        assert_eq!(
            t.core
                .update_user(&admin(), u.id, 2, UserPatch::default())
                .await?
                .version,
            2
        );
        assert_eq!(count_audit(&t.core, "user.updated").await, 1);
        let r = t
            .core
            .create_role(
                &admin(),
                NewRole {
                    name: "Приёмщик".into(),
                    permissions: PermissionSet::from_codes(["orders_create"]),
                },
            )
            .await?;
        assert_eq!(
            t.core
                .update_role(
                    &admin(),
                    r.id,
                    7,
                    RolePatch {
                        name: Some(" Приёмщик".into()),
                        permissions: Some(r.permissions.clone())
                    }
                )
                .await?,
            r
        );
        assert_eq!(count_audit(&t.core, "role.updated").await, 0);
    }
    Ok(())
}

#[tokio::test]
async fn only_changed_fields_reach_the_journal() -> Result<(), CoreError> {
    for t in cores().await {
        let st = t.core.list_statuses(false).await?.remove(0);
        t.core
            .update_status(
                &admin(),
                st.id,
                StatusPatch {
                    name: Some(st.name.clone()),
                    color: Some("pink".into()),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(
            audit_details(&t.core, "status.updated").await,
            vec![json!({ "id": st.id, "changed": { "color": "pink" } })]
        );
        t.core
            .update_settings(
                &admin(),
                SettingsPatch {
                    master_percent: Some(40),
                    currency_symbol: Some("₽".into()),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(
            audit_details(&t.core, "settings.updated").await,
            vec![json!({ "master_percent": 40 })]
        );
    }
    Ok(())
}
