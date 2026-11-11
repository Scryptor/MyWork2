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

    /// `Err(секунды)` — ключ заблокирован ещё столько (с округлением вверх, не меньше 1).
    pub(crate) fn check(&self, key: &str, now: DateTime<Utc>) -> Result<(), u64> {
        let entries = self.lock();
        let Some(entry) = entries.get(key) else {
            return Ok(());
        };
        let end = entry.started + self.window;
        if now >= end || entry.failures < self.max_failures {
            return Ok(());
        }
        let millis = (end - now).num_milliseconds().max(0) as u64;
        Err(millis.div_ceil(1000).max(1))
    }

    pub(crate) fn record_failure(&self, key: &str, now: DateTime<Utc>) {
        let mut entries = self.lock();
        if let Some(entry) = entries.get_mut(key) {
            if now >= entry.started + self.window {
                *entry = Entry {
                    started: now,
                    failures: 1,
                };
            } else {
                entry.failures = entry.failures.saturating_add(1);
            }
            return;
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
            assert_eq!(l.check("k", t0), Ok(()));
            l.record_failure("k", t0);
        }
        assert_eq!(l.check("k", t0 + Duration::minutes(1)), Err(840));
        assert_eq!(l.check("other", t0), Ok(()));
        assert_eq!(l.check("k", t0 + Duration::minutes(15)), Ok(()));
    }

    #[test]
    fn reset_forgets_key() {
        let l = LoginLimiter::new(3, Duration::minutes(15));
        for _ in 0..3 {
            l.record_failure("r", t0());
        }
        l.reset("r");
        assert_eq!(l.check("r", t0()), Ok(()));
    }

    #[test]
    fn expired_entries_are_pruned() {
        let l = LoginLimiter::new(1, Duration::minutes(1));
        for i in 0..10_001 {
            l.record_failure(&format!("k{i}"), t0());
        }
        l.record_failure("fresh", t0() + Duration::minutes(2));
        assert_eq!(l.len(), 1);
    }

    #[test]
    fn remaining_time_rounds_up_and_is_at_least_one_second() {
        let l = LoginLimiter::new(1, Duration::seconds(10));
        l.record_failure("k", t0());
        assert_eq!(l.check("k", t0() + Duration::milliseconds(9_500)), Err(1));
        assert_eq!(l.check("k", t0() + Duration::milliseconds(500)), Err(10));
    }
}
