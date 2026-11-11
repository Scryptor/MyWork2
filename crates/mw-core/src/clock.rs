use std::sync::Mutex;

use chrono::{DateTime, Duration, Utc};

/// Источник текущего времени (UTC). Единственный способ узнать «сейчас» — `Core::now()`.
pub trait Clock: Send + Sync {
    fn now_utc(&self) -> DateTime<Utc>;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now_utc(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// Управляемые часы для тестов.
pub struct FixedClock(Mutex<DateTime<Utc>>);

impl FixedClock {
    pub fn new(at: DateTime<Utc>) -> Self {
        Self(Mutex::new(at))
    }

    pub fn set(&self, at: DateTime<Utc>) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = at;
    }

    pub fn advance(&self, by: Duration) {
        let mut guard = self.0.lock().unwrap_or_else(|e| e.into_inner());
        *guard += by;
    }
}

impl Clock for FixedClock {
    fn now_utc(&self) -> DateTime<Utc> {
        *self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}
