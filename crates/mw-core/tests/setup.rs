mod common;

use common::*;
use mw_core::{CoreError, ImportMode, SetupRequest};
use mw_domain::dictionary::DictionaryKind;
use mw_domain::permission::Permission;

fn request(masters: &[&str]) -> SetupRequest {
    SetupRequest {
        firm_name: "  Сервис «Север»  ".into(),
        point_name: "Центральная".into(),
        masters: masters.iter().map(|m| (*m).to_owned()).collect(),
    }
}

#[tokio::test]
async fn setup_is_required_only_on_a_fresh_empty_database() -> Result<(), CoreError> {
    for t in cores().await {
        assert!(t.core.setup_required().await?, "{}: новая база", t.label);
        t.core.run_setup(&admin(), request(&[])).await?;
        assert!(
            !t.core.setup_required().await?,
            "{}: после настройки",
            t.label
        );
    }
    Ok(())
}

#[tokio::test]
async fn setup_is_not_required_when_the_database_already_has_an_order() -> Result<(), CoreError> {
    for t in cores().await {
        let c = make_client(&t, "Клиент", &["+79001112233"]).await;
        make_order_for(&t, c.id).await;
        assert!(!t.core.setup_required().await?, "{}", t.label);
    }
    Ok(())
}

#[tokio::test]
async fn an_import_marks_setup_done() -> Result<(), CoreError> {
    for t in cores().await {
        assert!(t.core.setup_required().await?, "{}", t.label);
        t.core
            .import_bundle(&admin(), sample_bundle(), ImportMode::IntoEmpty)
            .await?;
        assert!(!t.core.setup_required().await?, "{}", t.label);
        assert_eq!(
            setting_value(&t.core, "setup_done").await,
            true,
            "{}",
            t.label
        );
    }
    Ok(())
}

#[tokio::test]
async fn setup_writes_everything_and_skips_duplicate_masters() -> Result<(), CoreError> {
    for t in cores().await {
        t.core
            .run_setup(&admin(), request(&["Иван", " иван ", "", "Пётр", "ИВАН"]))
            .await?;
        let s = t.core.settings();
        assert_eq!(s.firm_name, "Сервис «Север»", "{}", t.label);
        let point_id = service_point_id_by_name(&t.core, "Центральная").await;
        assert_eq!(s.default_service_point_id, Some(point_id), "{}", t.label);
        let masters: Vec<String> = t
            .core
            .list_dictionary(DictionaryKind::Master, false)
            .await?
            .into_iter()
            .map(|i| i.value)
            .collect();
        assert_eq!(masters, ["Иван", "Пётр"], "{}", t.label);
        assert_eq!(
            count_audit(&t.core, "setup.completed").await,
            1,
            "{}",
            t.label
        );
    }
    Ok(())
}

#[tokio::test]
async fn second_setup_conflicts_and_changes_nothing() -> Result<(), CoreError> {
    for t in cores().await {
        t.core.run_setup(&admin(), request(&["Иван"])).await?;
        let again = SetupRequest {
            firm_name: "Другая".into(),
            point_name: "Вторая".into(),
            masters: vec!["Ольга".into()],
        };
        let err = t.core.run_setup(&admin(), again).await;
        assert!(
            matches!(err, Err(CoreError::Conflict { .. })),
            "{}",
            t.label
        );
        assert_eq!(t.core.settings().firm_name, "Сервис «Север»", "{}", t.label);
        assert_eq!(
            t.core.list_service_points(false).await?.len(),
            1,
            "{}",
            t.label
        );
    }
    Ok(())
}

#[tokio::test]
async fn setup_needs_the_settings_right_and_names() -> Result<(), CoreError> {
    for t in cores().await {
        let err = t.core.run_setup(&actor_with(&[]), request(&[])).await;
        assert!(
            matches!(err, Err(CoreError::Forbidden(Permission::SettingsManage))),
            "{}",
            t.label
        );
        let blank = SetupRequest {
            firm_name: " ".into(),
            point_name: "".into(),
            masters: vec![],
        };
        assert_validation(
            t.core.run_setup(&admin(), blank).await,
            &[("firm_name", "required"), ("point_name", "required")],
        );
        assert!(
            t.core.setup_required().await?,
            "{}: ничего не записано",
            t.label
        );
    }
    Ok(())
}
