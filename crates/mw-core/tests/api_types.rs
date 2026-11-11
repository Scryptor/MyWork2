//! Типы API: serde, TypeScript и OpenAPI согласованы между собой (без БД).
// Помощники ниже — не `#[test]`-функции, поэтому разрешение из clippy.toml на них не действует.
#![allow(clippy::unwrap_used)]

use std::fmt::Debug;

use mw_core::{
    AppSettings, AuditEntry, Client, ClientPatch, ClientRef, DictionaryItem, IssuedFilter,
    NewClient, Order, OrderListItem, OrderPatch, OrderSortField, Role, RolePatch, ServicePoint,
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
    ("SettingsPatch", "default_warranty_days"),
    ("OrderPatch", "service_point_id"),
    ("OrderPatch", "issued_at"),
    ("OrderPatch", "estimate_from"),
    ("OrderPatch", "estimate_to"),
    ("OrderPatch", "final_price"),
    ("OrderPatch", "warranty_days"),
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
        json!({ "brand": "Apple", "estimate_from": null, "warranty_days": 30 }),
    )
    .unwrap();
    assert_eq!(
        p,
        OrderPatch {
            brand: Some("Apple".into()),
            estimate_from: Some(None),
            warranty_days: Some(Some(30)),
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
        serde_json::from_value::<NewClient>(json!({ "full_name": "А", "phone": "", "address": "",
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
    ] {
        assert!(!decl.contains("?:") && !decl.contains("bigint"), "{decl}");
    }
    assert!(!User::decl(&c).contains("password"));
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
