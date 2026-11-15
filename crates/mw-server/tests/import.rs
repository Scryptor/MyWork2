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
