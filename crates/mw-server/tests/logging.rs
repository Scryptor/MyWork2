#![allow(clippy::unwrap_used, clippy::expect_used)]

use mw_server::logging::init_logging;

#[test]
fn init_logging_writes_a_line_into_the_log_folder() {
    let dir = tempfile::tempdir().unwrap();
    let logs = dir.path().join("logs");
    {
        let _guard = init_logging(Some(&logs)).unwrap();
        tracing::info!("строка для файла журнала");
    }
    let file = std::fs::read_dir(&logs)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            name.starts_with("mywork") && name.ends_with(".log")
        })
        .expect("файл журнала не создан");
    let text = std::fs::read_to_string(file).unwrap();
    assert!(text.contains("строка для файла журнала"), "{text}");
}
