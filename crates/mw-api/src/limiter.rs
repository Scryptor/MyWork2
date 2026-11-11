//! Ограничитель неудачных попыток входа (в памяти).
//!
//! Окно начинается с первой неудачи по ключу и длится `window`; набрав `max_failures`
//! неудач внутри окна, ключ заблокирован до его конца. После окна ключ забывается.

use std::collections::HashMap;
use std::sync::Mutex;

use chrono::{DateTime, Duration, Utc};

/// Сколько записей можно накопить, прежде чем при добавлении удалять истёкшие.
const PRUNE_THRESHOLD: usize = 10_000;

#[derive(Debug, Clone, Copy)]
struct Entry {
    /// Время первой неудачи окна.
    started: DateTime<Utc>,
    failures: u32,
}

pub(crate) struct LoginLimiter {
    max_failures: u32,
    window: Duration,
    // Замок не держится через `.await`.
    entries: Mutex<HashMap<String, Entry>>,
}

impl LoginLimiter {
    pub(crate) fn new(max_failures: u32, window: Duration) -> Self {
        LoginLimiter {
            max_failures,
            window,
            entries: Mutex::new(HashMap::new()),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Entry>> {
        self.entries.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Допускает попытку и сразу её засчитывает (под замком, поэтому параллельные запросы
    /// не обходят предел). `Err(секунды)` — ключ заблокирован ещё столько (с округлением
    /// вверх, не меньше 1); такая попытка не засчитывается.
    pub(crate) fn begin(&self, key: &str, now: DateTime<Utc>) -> Result<(), u64> {
        let mut entries = self.lock();
        if let Some(entry) = entries.get_mut(key) {
            let end = entry.started + self.window;
            if now >= end {
                *entry = Entry {
                    started: now,
                    failures: 1,
                };
                return Ok(());
            }
            if entry.failures >= self.max_failures {
                let millis = (end - now).num_milliseconds().max(0) as u64;
                return Err(millis.div_ceil(1000).max(1));
            }
            entry.failures += 1;
            return Ok(());
        }
        if entries.len() > PRUNE_THRESHOLD {
            let window = self.window;
            entries.retain(|_, e| now < e.started + window);
        }
        entries.insert(
            key.to_owned(),
            Entry {
                started: now,
                failures: 1,
            },
        );
        Ok(())
    }

    /// Отменяет одну засчитанную попытку (исход не считается неудачей).
    pub(crate) fn forgive(&self, key: &str) {
        let mut entries = self.lock();
        if let Some(entry) = entries.get_mut(key) {
            entry.failures = entry.failures.saturating_sub(1);
            if entry.failures == 0 {
                entries.remove(key);
            }
        }
    }

    pub(crate) fn reset(&self, key: &str) {
        self.lock().remove(key);
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.lock().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone, Utc};

    fn t0() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 11, 9, 0, 0).unwrap()
    }

    #[test]
    fn blocks_after_limit_until_window_ends() {
        let l = LoginLimiter::new(3, Duration::minutes(15));
        let t0 = t0();
        for _ in 0..3 {
            assert_eq!(l.begin("k", t0), Ok(()));
        }
        assert_eq!(l.begin("k", t0 + Duration::minutes(1)), Err(840));
        assert_eq!(l.begin("other", t0), Ok(()));
        assert_eq!(l.begin("k", t0 + Duration::minutes(15)), Ok(()));
    }

    #[test]
    fn reset_forgets_key() {
        let l = LoginLimiter::new(3, Duration::minutes(15));
        for _ in 0..3 {
            assert_eq!(l.begin("r", t0()), Ok(()));
        }
        l.reset("r");
        assert_eq!(l.len(), 0);
        assert_eq!(l.begin("r", t0()), Ok(()));
    }

    #[test]
    fn forgive_makes_room_for_one_more_attempt() {
        let l = LoginLimiter::new(2, Duration::minutes(15));
        assert_eq!(l.begin("k", t0()), Ok(()));
        assert_eq!(l.begin("k", t0()), Ok(()));
        assert_eq!(l.begin("k", t0()), Err(900));
        l.forgive("k");
        assert_eq!(l.begin("k", t0()), Ok(()));
        assert_eq!(l.begin("k", t0()), Err(900));
        l.forgive("k");
        l.forgive("k");
        l.forgive("k");
        assert_eq!(l.len(), 0);
    }

    #[test]
    fn expired_entries_are_pruned() {
        let l = LoginLimiter::new(1, Duration::minutes(1));
        for i in 0..10_001 {
            assert_eq!(l.begin(&format!("k{i}"), t0()), Ok(()));
        }
        assert_eq!(l.begin("fresh", t0() + Duration::minutes(2)), Ok(()));
        assert_eq!(l.len(), 1);
    }

    #[test]
    fn remaining_time_rounds_up_and_is_at_least_one_second() {
        let l = LoginLimiter::new(1, Duration::seconds(10));
        assert_eq!(l.begin("k", t0()), Ok(()));
        assert_eq!(l.begin("k", t0() + Duration::milliseconds(9_500)), Err(1));
        assert_eq!(l.begin("k", t0() + Duration::milliseconds(500)), Err(10));
    }
}
