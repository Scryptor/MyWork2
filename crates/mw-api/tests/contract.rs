//! Контракт API: `openapi.json` и `bindings/*.ts` должны совпадать с кодом.
//! `MW_UPDATE_CONTRACT=1` перезаписывает их вместо сравнения.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const STALE: &str = "контракт API устарел: выполните `MW_UPDATE_CONTRACT=1 cargo test -p mw-api --test contract` и закоммитьте openapi.json и bindings/";

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn updating() -> bool {
    std::env::var("MW_UPDATE_CONTRACT").is_ok_and(|v| v == "1")
}

/// Новая пустая папка во временном каталоге.
fn temp_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("mw-api-{tag}-{}-{nanos}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn check_or_update_file(path: &Path, expected: String) {
    if updating() {
        fs::write(path, expected).unwrap();
        return;
    }
    let actual = fs::read_to_string(path).unwrap_or_default();
    assert!(actual == expected, "{STALE} ({})", path.display());
}

/// Все `*.ts` папки: относительный путь (через `/`) -> содержимое.
fn read_tree(dir: &Path) -> BTreeMap<String, String> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, out);
            } else if path.extension().is_some_and(|e| e == "ts") {
                let rel = path
                    .strip_prefix(root)
                    .unwrap()
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                out.insert(rel, fs::read_to_string(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

fn check_or_update_tree(committed: &Path, fresh: &Path) {
    if updating() {
        let _ = fs::remove_dir_all(committed);
        for (rel, text) in read_tree(fresh) {
            let target = committed.join(&rel);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, text).unwrap();
        }
        return;
    }
    let (old, new) = (read_tree(committed), read_tree(fresh));
    let missing: Vec<_> = new.keys().filter(|k| !old.contains_key(*k)).collect();
    let extra: Vec<_> = old.keys().filter(|k| !new.contains_key(*k)).collect();
    let changed: Vec<_> = new
        .iter()
        .filter(|(k, v)| old.get(*k).is_some_and(|o| o != *v))
        .map(|(k, _)| k)
        .collect();
    assert!(
        missing.is_empty() && extra.is_empty() && changed.is_empty(),
        "{STALE}\nнет в bindings/: {missing:?}\nлишние в bindings/: {extra:?}\nизменились: {changed:?}"
    );
}

#[test]
fn openapi_json_is_up_to_date() {
    check_or_update_file(
        &manifest_dir().join("openapi.json"),
        mw_api::openapi().to_pretty_json().unwrap() + "\n",
    );
}

#[test]
fn typescript_bindings_are_up_to_date() {
    let fresh = temp_dir("bindings");
    mw_api::export_typescript(&fresh).unwrap();
    check_or_update_tree(&manifest_dir().join("bindings"), &fresh);
    let _ = fs::remove_dir_all(&fresh);
}

#[test]
fn every_openapi_schema_has_a_typescript_type() {
    let bindings = manifest_dir().join("bindings");
    for name in mw_api::openapi().components.unwrap().schemas.keys() {
        assert!(
            bindings.join(format!("{name}.ts")).exists(),
            "нет TS-типа для схемы {name}"
        );
    }
}

#[test]
fn every_schema_reference_resolves() {
    // utoipa-axum собирает схемы из тел и ответов, но не из параметров пути и запроса:
    // такие схемы (DictionaryKind, IssuedFilter, OrderSortField, OrderValueField) регистрируются в #[openapi(components(schemas(…)))].
    let text = mw_api::openapi().to_json().unwrap();
    let api: serde_json::Value = serde_json::from_str(&text).unwrap();
    let schemas = api["components"]["schemas"].as_object().unwrap();
    for (i, m) in text.match_indices("#/components/schemas/") {
        let name = text[i + m.len()..].split('"').next().unwrap();
        assert!(
            schemas.contains_key(name),
            "ссылка на несуществующую схему {name}"
        );
    }
}

#[test]
fn parameters_match_their_paths() {
    const REQUIRED_QUERY: [&str; 6] = ["version", "phone", "code", "field", "name", "format"];
    let api: serde_json::Value =
        serde_json::from_str(&mw_api::openapi().to_json().unwrap()).unwrap();
    for (path, item) in api["paths"].as_object().unwrap() {
        let placeholders: Vec<&str> = path
            .split('/')
            .filter_map(|s| s.strip_prefix('{').and_then(|s| s.strip_suffix('}')))
            .collect();
        for (method, op) in item.as_object().unwrap() {
            let params = op["parameters"].as_array().cloned().unwrap_or_default();
            let at = format!("{method} {path}");
            let mut path_names = Vec::new();
            for p in &params {
                let name = p["name"].as_str().unwrap();
                match p["in"].as_str().unwrap() {
                    "path" => {
                        assert!(placeholders.contains(&name), "{at}: нет {{{name}}} в пути");
                        assert_eq!(p["required"], true, "{at}: параметр пути {name}");
                        path_names.push(name);
                    }
                    "query" => {
                        let required = p["required"].as_bool().unwrap_or(false);
                        assert!(
                            !required || REQUIRED_QUERY.contains(&name),
                            "{at}: параметр запроса {name} обязателен"
                        );
                        let ty = &p["schema"]["type"];
                        let nullable = ty.as_array().is_some_and(|a| a.contains(&"null".into()))
                            || ty == "null"
                            || p["schema"]["nullable"] == true;
                        assert!(!nullable, "{at}: параметр запроса {name} допускает null");
                    }
                    other => panic!("{at}: неожиданное расположение {other}"),
                }
            }
            for ph in &placeholders {
                assert!(path_names.contains(ph), "{at}: нет параметра пути {ph}");
            }
        }
    }
}

/// Каждая строка «Карты API» (110, включая `openapi.json`) описана в документе OpenAPI.
#[test]
fn documented_operations_match_the_api_map() {
    let api = serde_json::to_value(mw_api::openapi()).unwrap();
    let mut ops: Vec<String> = Vec::new();
    for (path, item) in api["paths"].as_object().unwrap() {
        for method in ["get", "post", "put", "patch", "delete"] {
            if item.get(method).is_some() {
                ops.push(format!("{} {path}", method.to_uppercase()));
            }
        }
    }
    assert_eq!(ops.len(), 111, "{ops:#?}");
    for expected in [
        "GET /api/v1/server-info",
        "GET /api/v1/openapi.json",
        "POST /api/v1/auth/password",
        "PUT /api/v1/statuses/order",
        "DELETE /api/v1/dictionary-items/{id}",
        "GET /api/v1/clients/by-phone",
        "GET /api/v1/reports/moves",
        "POST /api/v1/clients/merge",
        "GET /api/v1/clients/{id}/similar",
        "POST /api/v1/orders/{id}/issue",
        "GET /api/v1/orders/{id}/history",
        "GET /api/v1/orders/{id}/comments",
        "POST /api/v1/orders/{id}/comments",
        "POST /api/v1/orders/{id}/warranty-case",
        "PUT /api/v1/orders/next-number",
        "GET /api/v1/orders/next-number",
        "GET /api/v1/about",
        "GET /api/v1/statuses/usage",
        "GET /api/v1/service-points/usage",
        "GET /api/v1/dictionaries/{kind}/usage",
        "POST /api/v1/dictionary-items/{id}/rename",
        "POST /api/v1/dictionaries/{kind}/merge",
        "GET /api/v1/orders/summary",
        "GET /api/v1/orders/values",
        "PUT /api/v1/users/{id}/password",
        "PATCH /api/v1/roles/{id}",
        "DELETE /api/v1/roles/{id}",
        "GET /api/v1/masters",
        "GET /api/v1/audit",
        "GET /api/v1/stock",
        "GET /api/v1/reports/orders",
        "GET /api/v1/reports/orders/xlsx",
        "GET /api/v1/exports/orders",
        "GET /api/v1/exports/clients",
        "POST /api/v1/stock",
        "GET /api/v1/stock/summary",
        "GET /api/v1/stock/{id}",
        "PATCH /api/v1/stock/{id}",
        "DELETE /api/v1/stock/{id}",
        "GET /api/v1/stock/{id}/history",
        "GET /api/v1/stock/{id}/photos",
        "POST /api/v1/stock/{id}/photos",
        "GET /api/v1/orders/{id}/parts",
        "POST /api/v1/orders/{id}/parts",
        "DELETE /api/v1/orders/{id}/parts/{part_id}",
        "POST /api/v1/orders/{id}/attachments",
        "GET /api/v1/attachments/{id}/thumb",
        "DELETE /api/v1/intake-drafts/{key}",
        "POST /api/v1/setup",
        "POST /api/v1/legacy-import/check",
        "POST /api/v1/legacy-import",
        "GET /api/v1/legacy-import/current",
        "GET /api/v1/legacy-imports",
        "GET /api/v1/legacy-imports/{id}",
        "GET /api/v1/legacy-import/stock",
        "POST /api/v1/legacy-import/stock",
        "GET /api/v1/orders/{id}/documents/{kind}",
        "GET /api/v1/documents/{kind}",
        "POST /api/v1/documents/{kind}/batch",
        "GET /api/v1/document-templates",
        "GET /api/v1/document-templates/{kind}",
        "PUT /api/v1/document-templates/{kind}",
        "POST /api/v1/document-templates/{kind}/preview",
        "POST /api/v1/document-templates/{kind}/block-code",
        "GET /api/v1/document-templates/{kind}/legacy",
        "GET /api/v1/document-templates/{kind}/file",
        "POST /api/v1/document-templates/{kind}/file",
        "GET /api/v1/document-fields",
        "GET /api/v1/settings/logo",
        "GET /api/v1/settings/logo/info",
        "PUT /api/v1/settings/logo",
        "DELETE /api/v1/settings/logo",
    ] {
        assert!(ops.iter().any(|o| o == expected), "нет операции {expected}");
    }
}

#[test]
fn response_schemas_require_every_property() {
    // Тела ответов всегда содержат все поля (пустое значение — `null`); у запросов
    // (`additionalProperties: false`) обязательность задаётся по смыслу поля.
    let text = mw_api::openapi().to_json().unwrap();
    let api: serde_json::Value = serde_json::from_str(&text).unwrap();
    let mut problems = Vec::new();
    for (name, schema) in api["components"]["schemas"].as_object().unwrap() {
        let Some(properties) = schema["properties"].as_object() else {
            continue;
        };
        if schema["additionalProperties"] == serde_json::json!(false) {
            continue;
        }
        let required: std::collections::BTreeSet<&str> = schema["required"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();
        let missing: Vec<&str> = properties
            .keys()
            .map(String::as_str)
            .filter(|k| !required.contains(k))
            .collect();
        if !missing.is_empty() {
            problems.push(format!("{name}: {missing:?}"));
        }
    }
    assert!(
        problems.is_empty(),
        "поля ответов не помечены обязательными:\n{}",
        problems.join("\n")
    );
}
