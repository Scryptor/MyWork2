//! Чтение копии базы и папки MyWork 2 и перенос их в MyWork 3.

#[allow(dead_code)] // используется со следующих задач плана
mod convert;
mod error;
#[allow(dead_code)] // используется со следующих задач плана
mod folder;
#[allow(dead_code, unused_imports)] // используется со следующих задач плана
mod parse;
#[allow(dead_code)] // используется со следующих задач плана
mod report;
#[allow(dead_code)] // используется со следующих задач плана
mod source;
#[allow(dead_code)] // используется со следующих задач плана
mod text;

#[cfg(feature = "fixtures")]
pub mod fixture;

#[cfg(test)]
mod test_support;

pub use error::LegacyError;
pub use report::{Severity, Warning, WarningCode};
pub use source::SchemaSummary;
