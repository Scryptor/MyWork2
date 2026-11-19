//! Дополнительные операции над `Option` — интерфейс крейта `option-ext` 0.2.0, написанный заново под MIT.

#![no_std]
#![forbid(unsafe_code)]

/// Дополнительные операции над `Option<T>`.
pub trait OptionExt<T> {
    /// Есть ли значение и равно ли оно `x`.
    fn contains<U>(&self, x: &U) -> bool
    where
        U: PartialEq<T>;

    /// `map_or` с обратным порядком аргументов: сначала функция, потом значение по умолчанию.
    fn map_or2<U, F: FnOnce(T) -> U>(self, f: F, default: U) -> U;

    /// `map_or_else` с обратным порядком аргументов: сначала функция, потом функция по умолчанию.
    fn map_or_else2<U, F: FnOnce(T) -> U, D: FnOnce() -> U>(self, f: F, default: D) -> U;
}

impl<T> OptionExt<T> for Option<T> {
    fn contains<U>(&self, x: &U) -> bool
    where
        U: PartialEq<T>,
    {
        self.as_ref().is_some_and(|value| *x == *value)
    }

    fn map_or2<U, F: FnOnce(T) -> U>(self, f: F, default: U) -> U {
        match self {
            Some(value) => f(value),
            None => default,
        }
    }

    fn map_or_else2<U, F: FnOnce(T) -> U, D: FnOnce() -> U>(self, f: F, default: D) -> U {
        match self {
            Some(value) => f(value),
            None => default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::OptionExt;

    #[test]
    fn contains_compares_only_a_present_value() {
        assert!(Some(3).contains(&3));
        assert!(!Some(3).contains(&4));
        assert!(!None::<i32>.contains(&3));
        // Сравнение через `PartialEq<T>` другого типа: `&str` со `String` (как в dirs-sys).
        assert!(Some("XDG_MUSIC_DIR").contains(&"XDG_MUSIC_DIR"));
    }

    #[test]
    fn map_or2_takes_the_function_first() {
        assert_eq!(Some(2).map_or2(|v| v * 10, 0), 20);
        assert_eq!(None::<i32>.map_or2(|v| v * 10, 7), 7);
    }

    #[test]
    fn map_or_else2_calls_the_default_only_without_a_value() {
        assert_eq!(Some(2).map_or_else2(|v| v + 1, || unreachable!()), 3);
        assert_eq!(None::<i32>.map_or_else2(|_| unreachable!(), || 9), 9);
    }
}
