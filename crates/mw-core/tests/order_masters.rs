//! Мастер заказа — пользователь с отметкой «Мастер»: выбор, снимок имени, история, список, выдача,
//! мастер по умолчанию; справочник «Мастера» закрыт для новых значений.

mod common;

use common::*;
use mw_core::{
    CoreError, DictionaryMerge, DictionaryRename, NewOrder, OrderFilter, OrderPatch, SettingsPatch,
    UserPatch,
};
use mw_domain::dictionary::DictionaryKind;
use serde_json::json;

fn with_master(id: Option<i64>) -> NewOrder {
    NewOrder {
        master_user_id: id,
        ..sample_new_order()
    }
}

fn set_master(id: Option<i64>) -> OrderPatch {
    OrderPatch {
        master_user_id: Some(id),
        ..Default::default()
    }
}

/// Ровно эта ошибка поля и текст «Мастер не найден или отключён».
fn assert_unavailable<T: std::fmt::Debug>(res: Result<T, CoreError>, field: &str) {
    match res {
        Err(CoreError::Validation(fields)) => {
            assert_eq!(fields.len(), 1, "{fields:?}");
            assert_eq!(
                (fields[0].field.as_str(), fields[0].code.as_str()),
                (field, "invalid")
            );
            assert_eq!(fields[0].message_ru(), "Мастер не найден или отключён");
        }
        other => panic!("ожидалась ошибка мастера, получено {other:?}"),
    }
}

#[tokio::test]
async fn the_master_is_a_user_and_its_name_is_written_into_the_order() -> Result<(), CoreError> {
    for t in cores().await {
        let anton = make_master(&t.core, " Антон ").await;
        let o = t
            .core
            .create_order(&admin(), with_master(Some(anton)))
            .await?;
        assert_eq!(
            (o.master_user_id, o.master.as_str()),
            (Some(anton), "Антон"),
            "{}",
            t.label
        );
        let none = t.core.create_order(&admin(), with_master(None)).await?;
        assert_eq!((none.master_user_id, none.master.as_str()), (None, ""));

        // Не мастер, отключённый мастер, нет такого пользователя — отказ.
        let clerk = role_id_by_name(&t.core, "Приёмщик").await;
        let plain = insert_user_row(&t.core, "plain", clerk, true).await;
        let gone = make_master(&t.core, "Уволен").await;
        let u = t.core.get_order(&admin(), o.id).await?;
        let gone_user = t.core.list_users(&admin()).await?;
        let gone_version = gone_user.iter().find(|u| u.id == gone).unwrap().version;
        t.core
            .update_user(
                &admin(),
                gone,
                gone_version,
                UserPatch {
                    active: Some(false),
                    ..Default::default()
                },
            )
            .await?;
        for id in [plain, gone, 999_999] {
            assert_unavailable(
                t.core.create_order(&admin(), with_master(Some(id))).await,
                "master_user_id",
            );
            assert_unavailable(
                t.core
                    .update_order(&admin(), u.id, u.version, set_master(Some(id)))
                    .await,
                "master_user_id",
            );
        }

        let listed = t
            .core
            .list_orders(
                &admin(),
                &OrderFilter::default(),
                mw_core::OrderSort::default(),
                50,
                0,
            )
            .await?;
        let item = listed.items.iter().find(|i| i.id == o.id).unwrap();
        assert_eq!(
            (item.master_user_id, item.master.as_str()),
            (Some(anton), "Антон")
        );
    }
    Ok(())
}

#[tokio::test]
async fn changing_the_master_writes_history_as_before() -> Result<(), CoreError> {
    for t in cores().await {
        let anton = make_master(&t.core, "Антон").await;
        let boris = make_master(&t.core, "Борис").await;
        let o = t
            .core
            .create_order(&admin(), with_master(Some(anton)))
            .await?;
        let u = t
            .core
            .update_order(&admin(), o.id, 1, set_master(Some(boris)))
            .await?;
        assert_eq!(
            (u.master_user_id, u.master.as_str(), u.version),
            (Some(boris), "Борис", 2)
        );
        let last = last_audit(&t.core, o.id).await;
        assert_eq!(
            (last.action.as_str(), last.summary.as_str()),
            ("order.updated", "Изменено: мастер"),
            "{}",
            t.label
        );
        let changes = &last.details.unwrap()["changes"];
        assert_eq!(changes["master"], json!(["Антон", "Борис"]));
        assert_eq!(changes["master_user_id"], json!([anton, boris]));

        // Тот же мастер — правки нет: ни версии, ни записи журнала.
        let same = t
            .core
            .update_order(&admin(), o.id, 2, set_master(Some(boris)))
            .await?;
        assert_eq!(same.version, 2);
        assert_eq!(audit_summaries(&t.core, o.id).await.len(), 2);

        let cleared = t
            .core
            .update_order(&admin(), o.id, 2, set_master(None))
            .await?;
        assert_eq!(
            (cleared.master_user_id, cleared.master.as_str()),
            (None, "")
        );
        assert_eq!(
            last_audit(&t.core, o.id).await.details.unwrap()["changes"]["master"],
            json!(["Борис", ""])
        );
    }
    Ok(())
}

#[tokio::test]
async fn renaming_or_disabling_the_user_keeps_old_orders() -> Result<(), CoreError> {
    for t in cores().await {
        let anton = make_master(&t.core, "Антон").await;
        let old = t
            .core
            .create_order(&admin(), with_master(Some(anton)))
            .await?;
        let user = |id| {
            let core = t.core.clone();
            async move {
                core.list_users(&admin())
                    .await
                    .unwrap()
                    .into_iter()
                    .find(|u| u.id == id)
                    .unwrap()
            }
        };
        let v = user(anton).await.version;
        t.core
            .update_user(
                &admin(),
                anton,
                v,
                UserPatch {
                    display_name: Some("Антон Петров".into()),
                    ..Default::default()
                },
            )
            .await?;
        let kept = t.core.get_order(&admin(), old.id).await?;
        assert_eq!(
            (kept.master_user_id, kept.master.as_str(), kept.version),
            (Some(anton), "Антон", 1),
            "{}",
            t.label
        );
        let fresh = t
            .core
            .create_order(&admin(), with_master(Some(anton)))
            .await?;
        assert_eq!(fresh.master, "Антон Петров");

        // Отключённый мастер остаётся в старом заказе, и другие правки этого заказа проходят.
        let v = user(anton).await.version;
        t.core
            .update_user(
                &admin(),
                anton,
                v,
                UserPatch {
                    active: Some(false),
                    ..Default::default()
                },
            )
            .await?;
        let edited = t
            .core
            .update_order(
                &admin(),
                old.id,
                1,
                OrderPatch {
                    master_user_id: Some(Some(anton)),
                    model: Some("A53".into()),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(
            (
                edited.master_user_id,
                edited.master.as_str(),
                edited.model.as_str()
            ),
            (Some(anton), "Антон", "A53")
        );
    }
    Ok(())
}

#[tokio::test]
async fn the_list_filters_by_master_user_and_still_by_name() -> Result<(), CoreError> {
    for t in cores().await {
        let anton = make_master(&t.core, "Антон").await;
        let boris = make_master(&t.core, "Борис").await;
        let a = t
            .core
            .create_order(&admin(), with_master(Some(anton)))
            .await?;
        let b = t
            .core
            .create_order(&admin(), with_master(Some(boris)))
            .await?;
        t.core.create_order(&admin(), with_master(None)).await?;
        let by_user = OrderFilter {
            master_user_id: Some(anton),
            ..Default::default()
        };
        assert_eq!(list_ids(&t, by_user).await, [a.id], "{}", t.label);
        // Старый сохранённый вид — по имени без учёта регистра.
        let by_name = OrderFilter {
            master: Some("БОРИС".into()),
            ..Default::default()
        };
        assert_eq!(list_ids(&t, by_name).await, [b.id], "{}", t.label);
    }
    Ok(())
}

#[tokio::test]
async fn issue_needs_a_master_user() -> Result<(), CoreError> {
    for t in cores().await {
        let anton = make_master(&t.core, "Антон").await;
        let o = t
            .core
            .create_order(
                &admin(),
                NewOrder {
                    master_user_id: None,
                    ..sample_new_order()
                },
            )
            .await?;
        let o = t
            .core
            .update_order(
                &admin(),
                o.id,
                1,
                OrderPatch {
                    work_done: Some("Замена дисплея".into()),
                    final_price: Some(Some(mw_domain::money::Money::from_kopecks(100_000))),
                    ..Default::default()
                },
            )
            .await?;
        assert_validation(
            t.core.issue_order(&admin(), o.id, o.version).await,
            &[("master", "required_for_issue")],
        );
        let o = t
            .core
            .update_order(&admin(), o.id, o.version, set_master(Some(anton)))
            .await?;
        assert!(t.core.issue_order(&admin(), o.id, o.version).await.is_ok());
    }
    Ok(())
}

#[tokio::test]
async fn the_default_master_is_an_available_master_user() -> Result<(), CoreError> {
    for t in cores().await {
        assert_eq!(t.core.settings().default_master_user_id, None);
        let anton = make_master(&t.core, "Антон").await;
        let s = t
            .core
            .update_settings(
                &admin(),
                SettingsPatch {
                    default_master_user_id: Some(Some(anton)),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(s.default_master_user_id, Some(anton), "{}", t.label);
        assert!(
            audit_json(&t.core, "settings.updated")
                .await
                .contains("мастер по умолчанию")
        );
        let clerk = role_id_by_name(&t.core, "Приёмщик").await;
        let plain = insert_user_row(&t.core, "plain", clerk, true).await;
        assert_unavailable(
            t.core
                .update_settings(
                    &admin(),
                    SettingsPatch {
                        default_master_user_id: Some(Some(plain)),
                        ..Default::default()
                    },
                )
                .await,
            "default_master_user_id",
        );
        // Отметку «Мастер» сняли — мастер по умолчанию больше не выбран (и в кэше).
        let v = t
            .core
            .list_users(&admin())
            .await?
            .into_iter()
            .find(|u| u.id == anton)
            .unwrap()
            .version;
        t.core
            .update_user(
                &admin(),
                anton,
                v,
                UserPatch {
                    is_master: Some(false),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(
            t.core.settings().default_master_user_id,
            None,
            "{}",
            t.label
        );
        t.core
            .update_user(
                &admin(),
                anton,
                v + 1,
                UserPatch {
                    is_master: Some(true),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(t.core.settings().default_master_user_id, Some(anton));
        let s = t
            .core
            .update_settings(
                &admin(),
                SettingsPatch {
                    default_master_user_id: Some(None),
                    ..Default::default()
                },
            )
            .await?;
        assert_eq!(s.default_master_user_id, None);
    }
    Ok(())
}

#[tokio::test]
async fn the_masters_dictionary_takes_no_new_values() -> Result<(), CoreError> {
    for t in cores().await {
        let closed = |res: Result<_, CoreError>| match res {
            Err(CoreError::Validation(f)) => {
                assert_eq!(
                    (f[0].field.as_str(), f[0].code.as_str()),
                    ("kind", "closed")
                );
                assert!(
                    f[0].message_ru()
                        .contains("пользователи с отметкой «Мастер»")
                );
            }
            other => panic!("ожидался отказ, получено {other:?}"),
        };
        closed(
            t.core
                .add_dictionary_item(&admin(), DictionaryKind::Master, "Антон")
                .await
                .map(|_| ()),
        );
        // Значения, оставшиеся от прежних версий (в архиве), можно удалить, но не вернуть.
        let old = insert_dictionary_row(&t.core, DictionaryKind::Master, "Прежний", true).await;
        let other = insert_dictionary_row(&t.core, DictionaryKind::Master, "Другой", true).await;
        closed(
            t.core
                .update_dictionary_item(
                    &admin(),
                    old,
                    mw_core::DictionaryItemPatch {
                        archived: Some(false),
                        ..Default::default()
                    },
                )
                .await
                .map(|_| ()),
        );
        closed(
            t.core
                .rename_dictionary_item_everywhere(
                    &admin(),
                    old,
                    DictionaryRename {
                        value: "Новый".into(),
                        in_orders: true,
                    },
                )
                .await
                .map(|_| ()),
        );
        closed(
            t.core
                .merge_dictionary_items(
                    &admin(),
                    DictionaryKind::Master,
                    DictionaryMerge {
                        target_id: old,
                        source_ids: vec![other],
                        in_orders: false,
                    },
                )
                .await
                .map(|_| ()),
        );
        assert!(
            t.core
                .list_dictionary(DictionaryKind::Master, false)
                .await?
                .is_empty()
        );
        assert_eq!(
            t.core
                .list_dictionary(DictionaryKind::Master, true)
                .await?
                .len(),
            2
        );
        t.core.delete_dictionary_item(&admin(), other).await?;
        assert_eq!(
            t.core
                .list_dictionary(DictionaryKind::Master, true)
                .await?
                .len(),
            1,
            "{}",
            t.label
        );
    }
    Ok(())
}
