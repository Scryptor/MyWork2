#![allow(clippy::unwrap_used, clippy::expect_used)]

use mw_legacy::fixture::{FIXTURE_SMS_TOKEN, FixtureOptions, FixtureVersion, write_fixture};
use mw_server::Config;
use mw_server::import::run_import_with_db;
use std::path::Path;

fn test_config(data_dir: &Path) -> Config {
    Config {
        listen: "127.0.0.1:0".parse().unwrap(),
        data_dir: data_dir.to_path_buf(),
        database_url: None,
        timezone: Some("UTC".into()),
    }
}

#[tokio::test]
async fn import_legacy_command_imports_fixture_and_writes_report() {
    for (label, db) in mw_db::testing::all_backends().await {
        let (src, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        write_fixture(
            src.path(),
            &FixtureOptions {
                version: FixtureVersion::V32,
                extra_orders: 0,
            },
        )
        .await
        .unwrap();
        let config = test_config(data.path());
        let out = run_import_with_db(&config, db.clone(), src.path(), false)
            .await
            .unwrap();
        assert_eq!(out.report.summary.orders, 11, "{label}");
        let text = std::fs::read_to_string(&out.text_report).unwrap();
        assert!(
            text.contains("Перенесено") && text.contains("Не разобрана дата"),
            "{label}"
        );
        assert!(!text.contains(FIXTURE_SMS_TOKEN));
        assert!(
            serde_json::from_str::<serde_json::Value>(
                &std::fs::read_to_string(&out.json_report).unwrap()
            )
            .is_ok()
        );
        let again = run_import_with_db(&config, db, src.path(), false)
            .await
            .unwrap_err()
            .to_string();
        assert!(again.contains("--replace"), "{label}: {again}");
    }
}

#[tokio::test]
async fn open_mode_follows_imported_users() {
    for (label, db) in mw_db::testing::all_backends().await {
        let (src, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        write_fixture(
            src.path(),
            &FixtureOptions {
                version: FixtureVersion::V30,
                extra_orders: 0,
            },
        )
        .await
        .unwrap();
        let out = run_import_with_db(&test_config(data.path()), db, src.path(), false)
            .await
            .unwrap();
        assert!(out.open_mode, "{label}: V30 has no users");
    }
    for (label, db) in mw_db::testing::all_backends().await {
        let (src, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        write_fixture(
            src.path(),
            &FixtureOptions {
                version: FixtureVersion::V32,
                extra_orders: 0,
            },
        )
        .await
        .unwrap();
        let out = run_import_with_db(&test_config(data.path()), db, src.path(), false)
            .await
            .unwrap();
        assert!(!out.open_mode, "{label}: V32 has users");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn files_not_placed_still_writes_reports_and_names_staging() {
    for (label, db) in mw_db::testing::all_backends().await {
        let (src, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        write_fixture(
            src.path(),
            &FixtureOptions {
                version: FixtureVersion::V32,
                extra_orders: 0,
            },
        )
        .await
        .unwrap();
        let attachments = data.path().join("attachments");
        std::fs::create_dir_all(&attachments).unwrap();
        // Висячая ссылка: проверка перед импортом её пропускает, а поставить папку на её место нельзя.
        std::os::unix::fs::symlink(data.path().join("nowhere"), attachments.join("orders"))
            .unwrap();
        let error = run_import_with_db(&test_config(data.path()), db, src.path(), false)
            .await
            .unwrap_err();
        let text = error.to_string();
        let mw_server::ServerError::ImportFilesNotPlaced {
            staging,
            text_report,
            json_report,
            ..
        } = &error
        else {
            panic!("{label}: {error:?}");
        };
        assert!(
            text.contains(&staging.display().to_string()),
            "{label}: {text}"
        );
        assert!(text_report.exists() && json_report.exists(), "{label}");
    }
}

#[tokio::test]
async fn data_dir_inside_the_old_folder_is_refused_before_connecting() {
    use clap::Parser;
    let src = tempfile::tempdir().unwrap();
    write_fixture(
        src.path(),
        &FixtureOptions {
            version: FixtureVersion::V32,
            extra_orders: 0,
        },
    )
    .await
    .unwrap();
    let before: Vec<_> = std::fs::read_dir(src.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    for data in [src.path().join("data"), src.path().join("missing/../data")] {
        let cli = mw_server::Cli::parse_from([
            "mw-server".as_ref(),
            "--data-dir".as_ref(),
            data.as_os_str(),
            "import-legacy".as_ref(),
            src.path().as_os_str(),
        ]);
        let error = mw_server::run(cli).await.unwrap_err().to_string();
        assert!(error.contains("лежит внутри старой папки"), "{error}");
        let after: Vec<_> = std::fs::read_dir(src.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(before, after, "в старой папке ничего не создаётся");
        assert!(!src.path().join("data").exists());
    }
}
