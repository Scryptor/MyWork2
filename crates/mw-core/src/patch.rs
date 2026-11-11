//! Разбор полей правок (`…Patch`) из JSON: «поля нет» и `null` — разные вещи.

use serde::{Deserialize, Deserializer};

/// Поле, которое нельзя очистить: значение даёт `Some`, `null` — ошибку.
/// Отсутствие поля даёт `None` через `#[serde(default)]`.
pub(crate) fn present<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}

/// Очищаемое поле: значение даёт `Some(Some(v))`, `null` — `Some(None)`.
/// Отсутствие поля даёт `None` через `#[serde(default)]`.
pub(crate) fn nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<Option<T>>, D::Error> {
    Option::<T>::deserialize(deserializer).map(Some)
}

/// Обязательное поле, у которого допустим `null`: ключ обязан быть в теле запроса
/// (без `#[serde(default)]` отсутствие ключа — ошибка), `null` даёт `None`.
pub(crate) fn required_nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(deserializer)
}
