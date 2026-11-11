use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use sea_orm::DbErr;
use sha1::{Digest, Sha1};

use crate::CoreError;

/// Схема хранения пароля: коды в колонке `users.password_scheme`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordScheme {
    /// Argon2id, PHC-строка.
    Argon2,
    /// SHA-1 от UTF-16LE без соли (пароли из MyWork 2); при первой смене заменяется на Argon2.
    LegacySha1,
}

impl PasswordScheme {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Argon2 => "argon2",
            Self::LegacySha1 => "legacy_sha1",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "argon2" => Some(Self::Argon2),
            "legacy_sha1" => Some(Self::LegacySha1),
            _ => None,
        }
    }
}

/// Argon2id с параметрами крейта по умолчанию и случайной солью из ОС; результат — PHC-строка.
pub fn hash_password(password: &str) -> Result<String, CoreError> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|h| h.to_string())
        .map_err(|_| CoreError::Db(DbErr::Custom("не удалось вычислить хэш пароля".into())))
}

/// SHA-1 от байтов UTF-16LE, hex в нижнем регистре — как хранил MyWork 2.
pub fn legacy_sha1_hex(password: &str) -> String {
    let bytes: Vec<u8> = password
        .encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect();
    hex::encode(Sha1::digest(&bytes))
}

/// Проверяет пароль по сохранённому хэшу; любой сбой разбора хэша — «не подходит».
pub fn verify_password(scheme: PasswordScheme, stored: &str, password: &str) -> bool {
    match scheme {
        PasswordScheme::Argon2 => Argon2::default()
            .verify_password(password.as_bytes(), stored)
            .is_ok(),
        PasswordScheme::LegacySha1 => constant_time_eq(
            stored.to_ascii_lowercase().as_bytes(),
            legacy_sha1_hex(password).as_bytes(),
        ),
    }
}

/// Сравнение без раннего выхода по содержимому; разная длина — сразу `false`.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// `hash_password` вне рабочего потока async-рантайма (Argon2 занимает ЦП и ~19 МиБ).
pub(crate) async fn hash_password_blocking(password: String) -> Result<String, CoreError> {
    tokio::task::spawn_blocking(move || hash_password(&password))
        .await
        .map_err(|_| CoreError::Db(DbErr::Custom("сбой вычисления хэша пароля".into())))?
}

/// `verify_password` вне рабочего потока async-рантайма; сбой задачи — «не подходит».
#[allow(dead_code)] // используется входом в систему (задача 14)
pub(crate) async fn verify_password_blocking(
    scheme: PasswordScheme,
    stored: String,
    password: String,
) -> bool {
    tokio::task::spawn_blocking(move || verify_password(scheme, &stored, &password))
        .await
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_compare_ignores_case_of_stored_hex_but_not_content() {
        let upper = "139F69C93C042496A8E958EC5930662C6CCCAFBF";
        assert!(verify_password(PasswordScheme::LegacySha1, upper, "1234"));
        let off_by_one = "139f69c93c042496a8e958ec5930662c6cccafbe";
        assert!(!verify_password(
            PasswordScheme::LegacySha1,
            off_by_one,
            "1234"
        ));
        assert!(!verify_password(PasswordScheme::LegacySha1, "139f", "1234"));
    }
}
