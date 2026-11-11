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
        PasswordScheme::LegacySha1 => stored.eq_ignore_ascii_case(&legacy_sha1_hex(password)),
    }
}
