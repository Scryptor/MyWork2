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
            ::serde::Serialize, ::serde::Deserialize, ::ts_rs::TS, ::utoipa::ToSchema,
        )]
        pub enum $name { $(#[serde(rename = $code)] $variant),+ }

        impl $name {
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $code),+ }
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
pub mod money;
pub mod permission;
pub mod phone;
pub mod search;
pub mod status;
pub mod validation;
