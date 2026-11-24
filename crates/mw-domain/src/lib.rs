// Доменная модель MyWork2: чистые типы и правила без ввода-вывода.

/// Неизвестный строковый код перечисления.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("неизвестный код: {0}")]
pub struct UnknownCode(pub String);

/// Перечисление со стабильными строковыми кодами: `as_str`, `FromStr`, serde, TypeScript и OpenAPI по коду.
macro_rules! code_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => $code:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord,
            ::serde::Serialize, ::serde::Deserialize, ::ts_rs::TS,
        )]
        pub enum $name { $(#[serde(rename = $code)] $variant),+ }

        impl $name {
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $code),+ }
            }
        }

        // `ToSchema` не выводится: utoipa не видит `#[serde(rename = $code)]`, когда `$code` пришёл
        // из макроса, и подставил бы имена вариантов. Схема собирается из тех же кодов.
        impl ::utoipa::PartialSchema for $name {
            fn schema() -> ::utoipa::openapi::RefOr<::utoipa::openapi::schema::Schema> {
                ::utoipa::openapi::ObjectBuilder::new()
                    .schema_type(::utoipa::openapi::schema::Type::String)
                    .enum_values(Some([$($code),+]))
                    .into()
            }
        }

        impl ::utoipa::ToSchema for $name {
            fn name() -> ::std::borrow::Cow<'static, str> {
                ::std::borrow::Cow::Borrowed(stringify!($name))
            }
        }

        impl std::str::FromStr for $name {
            type Err = $crate::UnknownCode;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($code => Ok(Self::$variant),)+
                    other => Err($crate::UnknownCode(other.to_owned())),
                }
            }
        }
    };
}

pub mod barcode;
pub mod dictionary;
pub mod document_template;
pub mod documents;
pub mod issue;
pub mod money;
pub mod permission;
pub mod phone;
pub mod search;
pub mod spelling;
pub mod status;
pub mod stock;
pub mod validation;
pub mod warranty;
