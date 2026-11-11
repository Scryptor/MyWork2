use std::sync::Arc;

use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use sha1::{Digest, Sha1};
use tokio::sync::Semaphore;

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
        .map_err(|_| CoreError::Internal("не удалось вычислить хэш пароля".into()))
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

/// Ограничитель одновременных вычислений Argon2: каждое занимает ЦП и ~19 МиБ памяти.
/// Дёшево клонируется; разрешение живёт до конца вычисления, даже если вызывающий отказался ждать.
#[derive(Clone)]
pub(crate) struct HashGate {
    permits: Arc<Semaphore>,
}

impl HashGate {
    pub(crate) fn new(permits: usize) -> Self {
        Self {
            permits: Arc::new(Semaphore::new(permits)),
        }
    }

    /// Ждёт свободное разрешение и выполняет `work` вне рабочего потока async-рантайма.
    pub(crate) async fn run<R: Send + 'static>(
        &self,
        work: impl FnOnce() -> R + Send + 'static,
    ) -> Result<R, CoreError> {
        let permit = Arc::clone(&self.permits)
            .acquire_owned()
            .await
            .map_err(|_| CoreError::Internal("ограничитель вычислений закрыт".into()))?;
        tokio::task::spawn_blocking(move || {
            let result = work();
            drop(permit);
            result
        })
        .await
        .map_err(|_| CoreError::Internal("сбой вычисления хэша пароля".into()))
    }

    #[cfg(test)]
    fn available(&self) -> usize {
        self.permits.available_permits()
    }
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

    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
    use std::time::Duration;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn gate_limits_concurrent_work() {
        let gate = HashGate::new(2);
        let (now, peak) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
        let tasks: Vec<_> = (0..8)
            .map(|_| {
                let (gate, now, peak) = (gate.clone(), now.clone(), peak.clone());
                tokio::spawn(async move {
                    gate.run(move || {
                        let n = now.fetch_add(1, SeqCst) + 1;
                        peak.fetch_max(n, SeqCst);
                        std::thread::sleep(Duration::from_millis(30));
                        now.fetch_sub(1, SeqCst);
                    })
                    .await
                })
            })
            .collect();
        for t in tasks {
            t.await.unwrap().unwrap();
        }
        assert!(peak.load(SeqCst) <= 2);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn permit_is_held_until_work_ends_even_if_caller_gives_up() {
        let gate = HashGate::new(1);
        let g = gate.clone();
        let caller = tokio::spawn(async move {
            g.run(|| std::thread::sleep(Duration::from_millis(200)))
                .await
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        caller.abort();
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(gate.available(), 0);
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(gate.available(), 1);
    }
}
