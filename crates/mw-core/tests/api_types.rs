//! Типы API: serde, TypeScript и OpenAPI согласованы между собой (без БД).
// Помощники ниже — не `#[test]`-функции, поэтому разрешение из clippy.toml на них не действует.
#![allow(clippy::unwrap_used)]

use std::fmt::Debug;

use mw_core::{
    AppSettings, AuditEntry, Client, ClientPatch, ClientRef, DictionaryItem, DictionaryMerge,
    DictionaryRename, IssuedFilter, MasterUser, NewClient, NewOrder, NewRole, NewStatus, NewUser,
    Order, OrderListItem, OrderPatch, OrderSortField, RewriteResult, Role, RolePatch, ServicePoint,
    ServicePointPatch, SettingsPatch, Status, StatusPatch, User, UserPatch,
};
use mw_domain::dictionary::DictionaryKind;
use mw_domain::money::Money;
use mw_domain::permission::Permission;
use mw_domain::status::StatusKind;
use mw_domain::validation::FieldError;
use serde::de::DeserializeOwned;
use serde_json::json;
use ts_rs::TS;

fn cfg() -> ts_rs::Config {
    ts_rs::Config::new().with_large_int("number")
}

/// Текст типа поля `{field}?: …` в TS-объявлении: до ближайшей запятой.
fn ts_field_type(decl: &str, field: &str) -> String {
    let marker = format!("{field}?: ");
    let mut from = 0;
    while let Some(found) = decl[from..].find(&marker) {
        let at = from + found;
        let boundary = decl[..at]
            .chars()
            .next_back()
            .is_none_or(|c| c == ' ' || c == '\n');
        if boundary {
            let rest = &decl[at + marker.len()..];
            let end = rest.find(',').unwrap_or(rest.len());
            return rest[..end].trim_end_matches(['}', ' ', '\n']).to_owned();
        }
        from = at + marker.len();
    }
    panic!("поле {field} не найдено в {decl}");
}

const NULLABLE: &[(&str, &str)] = &[
    ("SettingsPatch", "default_service_point_id"),
    ("SettingsPatch", "default_master_user_id"),
    ("OrderPatch", "service_point_id"),
    ("OrderPatch", "master_user_id"),
    ("OrderPatch", "issued_at"),
    ("OrderPatch", "estimate_from"),
    ("OrderPatch", "estimate_to"),
    ("OrderPatch", "final_price"),
];

/// serde, TypeScript и OpenAPI согласны по каждому полю правки: необязательно, можно ли `null`.
fn check_patch<T>()
where
    T: DeserializeOwned + TS + utoipa::ToSchema + Default + PartialEq + Debug,
{
    let name = <T as utoipa::ToSchema>::name();
    let schema = serde_json::to_value(<T as utoipa::PartialSchema>::schema()).unwrap();
    let decl = T::decl(&cfg());
    let props = schema["properties"].as_object().unwrap();
    assert!(!props.is_empty(), "{name}");
    assert_eq!(schema["additionalProperties"], json!(false), "{name}");
    assert!(
        schema.get("required").is_none(),
        "{name}: у правки нет обязательных полей"
    );
    for (field, prop) in props {
        let nullable = NULLABLE.contains(&(name.as_ref(), field.as_str()));
        let ts_type = ts_field_type(&decl, field);
        assert_eq!(
            ts_type.contains("| null"),
            nullable,
            "{name}.{field}: TS «{ts_type}»"
        );
        assert_eq!(
            prop.to_string().contains("\"null\""),
            nullable,
            "{name}.{field}: OpenAPI {prop}"
        );
        assert_eq!(
            serde_json::from_value::<T>(json!({ field.as_str(): null })).is_ok(),
            nullable,
            "{name}.{field}: serde"
        );
    }
    assert_eq!(
        serde_json::from_value::<T>(json!({})).unwrap(),
        T::default()
    );
    assert!(
        serde_json::from_value::<T>(json!({ "no_such_field": 1 })).is_err(),
        "{name}"
    );
}

#[test]
fn patches_agree_on_absent_null_and_unknown_fields() {
    check_patch::<SettingsPatch>();
    check_patch::<StatusPatch>();
    check_patch::<ServicePointPatch>();
    check_patch::<ClientPatch>();
    check_patch::<OrderPatch>();
    check_patch::<UserPatch>();
    check_patch::<RolePatch>();
}

#[test]
fn order_patch_json_semantics() {
    let p: OrderPatch = serde_json::from_value(
        json!({ "brand": "Apple", "estimate_from": null, "warranty": "30 дней" }),
    )
    .unwrap();
    assert_eq!(
        p,
        OrderPatch {
            brand: Some("Apple".into()),
            estimate_from: Some(None),
            warranty: Some("30 дней".into()),
            ..Default::default()
        }
    );
}

#[test]
fn codes_money_and_refs_on_the_wire() {
    assert_eq!(
        serde_json::to_value(Permission::OrderDatesEdit).unwrap(),
        json!("order_dates_edit")
    );
    assert_eq!(
        serde_json::to_value(DictionaryKind::ClientSource).unwrap(),
        json!("client_source")
    );
    assert_eq!(
        serde_json::to_value(StatusKind::Issued).unwrap(),
        json!("issued")
    );
    assert_eq!(
        serde_json::to_value(IssuedFilter::InService).unwrap(),
        json!("in_service")
    );
    assert_eq!(
        serde_json::to_value(OrderSortField::ClientName).unwrap(),
        json!("client_name")
    );
    assert_eq!(
        serde_json::to_value(Money::from_kopecks(650_050)).unwrap(),
        json!(650050)
    );
    assert_eq!(
        serde_json::from_value::<ClientRef>(json!({ "existing": 5 })).unwrap(),
        ClientRef::Existing(5)
    );
    assert!(
        serde_json::from_value::<NewClient>(json!({ "full_name": "А", "phones": [], "address": "",
            "note": "", "source": "", "blacklisted": false, "vip": true }))
        .is_err()
    );
    assert!(serde_json::from_value::<Permission>(json!("bogus")).is_err());
}

#[test]
fn response_types_are_complete_and_hide_secrets() {
    let c = cfg();
    for decl in [
        Order::decl(&c),
        OrderListItem::decl(&c),
        Client::decl(&c),
        User::decl(&c),
        Role::decl(&c),
        Status::decl(&c),
        AuditEntry::decl(&c),
        AppSettings::decl(&c),
        ServicePoint::decl(&c),
        DictionaryItem::decl(&c),
        RewriteResult::decl(&c),
        MasterUser::decl(&c),
    ] {
        assert!(!decl.contains("?:") && !decl.contains("bigint"), "{decl}");
    }
    // Признак «пароль задан» — не секрет; ни хэша, ни схемы пароля в ответе нет.
    let user = User::decl(&c);
    assert!(
        !user
            .replace("has_password: boolean", "")
            .contains("password"),
        "{user}"
    );
    assert!(!MasterUser::decl(&c).contains("login"));
    assert!(Order::decl(&c).contains("estimate_from: Money | null"));
}

#[test]
fn field_error_messages_are_russian() {
    for code in [
        "required",
        "invalid",
        "too_short",
        "duplicate",
        "out_of_range",
        "estimate_range",
        "deleted",
        "last_admin",
        "last_issued_status",
        "too_many",
        "too_long",
        "too_large",
        "no_room",
        "text_budget",
        "too_many_pages",
        "too_many_pixels",
        "requires_a4",
        "requires_a4_landscape",
        "not_allowed",
        "nested_columns",
        "fill_in_columns",
        "unsupported",
        "mismatch",
        "not_editable",
        "period_reversed",
    ] {
        assert!(
            FieldError::new("x", code)
                .message_ru()
                .starts_with(char::is_uppercase),
            "{code}"
        );
    }
    assert_eq!(
        FieldError::new("x", "last_admin").message_ru(),
        "Нельзя оставить программу без администратора"
    );
    assert_eq!(
        FieldError::new("x", "что-то новое").message_ru(),
        "Недопустимое значение"
    );
}

/// У тела запроса все поля обязательны во всех трёх представлениях: serde, TS и OpenAPI.
fn check_new<T>(complete: serde_json::Value)
where
    T: DeserializeOwned + TS + utoipa::ToSchema,
{
    let name = <T as utoipa::ToSchema>::name();
    let schema = serde_json::to_value(<T as utoipa::PartialSchema>::schema()).unwrap();
    let props: std::collections::BTreeSet<String> = schema["properties"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    let required: std::collections::BTreeSet<String> = schema["required"]
        .as_array()
        .unwrap_or_else(|| panic!("{name}: нет списка required"))
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    assert_eq!(required, props, "{name}: required в OpenAPI");
    assert!(!T::decl(&cfg()).contains("?:"), "{name}: TS");
    let body = complete.as_object().unwrap();
    let keys: std::collections::BTreeSet<String> = body.keys().cloned().collect();
    assert_eq!(keys, props, "{name}: полное тело должно содержать все поля");
    serde_json::from_value::<T>(complete.clone())
        .unwrap_or_else(|e| panic!("{name}: полное тело не разобрано: {e}"));
    for field in body.keys() {
        let mut partial = body.clone();
        partial.remove(field);
        assert!(
            serde_json::from_value::<T>(serde_json::Value::Object(partial)).is_err(),
            "{name}.{field}: serde принял тело без поля"
        );
    }
}

#[test]
fn request_bodies_require_every_field() {
    check_new::<NewStatus>(json!({ "name": "А", "kind": "open", "color": "blue" }));
    check_new::<NewClient>(
        json!({ "full_name": "А", "phones": [], "address": "", "note": "",
        "source": "", "blacklisted": false }),
    );
    check_new::<NewUser>(
        json!({ "login": "a", "display_name": "А", "role_id": 1, "password": "pw12",
            "can_sign_in": true, "is_master": false }),
    );
    check_new::<NewRole>(json!({ "name": "А", "permissions": ["orders_create"] }));
    check_new::<DictionaryRename>(json!({ "value": "А", "in_orders": true }));
    check_new::<DictionaryMerge>(json!({ "target_id": 1, "source_ids": [2], "in_orders": false }));
    check_new::<NewOrder>(json!({
        "client": { "existing": 1 }, "service_point_id": null, "status_id": null,
        "device_type": "", "brand": "", "model": "", "serial": "", "color": "",
        "device_password": "", "device_pattern": "", "completeness": "", "device_condition": "",
        "malfunction": "", "internal_comment": "", "master_user_id": null, "estimate_from": null,
        "estimate_to": null,
        "prepayment": 0, "warranty": "30 дней", "need_call": false, "need_parts": false,
        "attachment_draft": null, "request_key": null
    }));
}

/// Коды перечисления одинаковы в serde, TypeScript и OpenAPI, в порядке объявления.
fn check_code_enum<T>(all: &[T], codes: &[&str])
where
    T: serde::Serialize + TS + utoipa::ToSchema + Copy,
{
    let name = <T as utoipa::ToSchema>::name();
    let schema = serde_json::to_value(<T as utoipa::PartialSchema>::schema()).unwrap();
    println!("{name}: {schema}");
    assert_eq!(schema["enum"], json!(codes), "{name}: OpenAPI");
    assert_eq!(schema["type"], json!("string"), "{name}: OpenAPI type");
    let wire: Vec<_> = all
        .iter()
        .map(|v| serde_json::to_value(v).unwrap())
        .collect();
    assert_eq!(
        wire,
        json!(codes).as_array().unwrap().clone(),
        "{name}: serde"
    );
    let union = codes
        .iter()
        .map(|c| format!("\"{c}\""))
        .collect::<Vec<_>>()
        .join(" | ");
    assert_eq!(
        T::decl(&cfg()),
        format!("type {name} = {union};"),
        "{name}: TS"
    );
}

#[test]
fn code_enums_use_wire_codes_everywhere() {
    use mw_domain::dictionary::DictionaryKind as K;
    let permissions: Vec<&str> = Permission::ALL.iter().map(|p| p.as_str()).collect();
    check_code_enum(&Permission::ALL, &permissions);
    check_code_enum(
        &[
            StatusKind::Open,
            StatusKind::Diagnostics,
            StatusKind::Ready,
            StatusKind::Issued,
        ],
        &["open", "diagnostics", "ready", "issued"],
    );
    check_code_enum(
        &[
            K::DeviceType,
            K::Brand,
            K::Color,
            K::PartType,
            K::Completeness,
            K::Condition,
            K::Malfunction,
            K::WorkDone,
            K::Master,
            K::ClientSource,
            K::Warranty,
        ],
        &[
            "device_type",
            "brand",
            "color",
            "part_type",
            "completeness",
            "condition",
            "malfunction",
            "work_done",
            "master",
            "client_source",
            "warranty",
        ],
    );
}
