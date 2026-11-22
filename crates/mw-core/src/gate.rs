//! Ограничитель тяжёлых вычислений (хэши паролей, рендер документов): не больше заданного числа
//! одновременно и вне рабочих потоков async-рантайма.

use std::sync::Arc;

use tokio::sync::Semaphore;

use crate::CoreError;

/// Дёшево клонируется; разрешение живёт до конца вычисления, даже если вызывающий отказался ждать.
#[derive(Clone)]
pub(crate) struct BlockingGate {
    permits: Arc<Semaphore>,
    /// Текст внутренней ошибки, если вычисление оборвалось (паника).
    failure: &'static str,
}

impl BlockingGate {
    pub(crate) fn new(permits: usize, failure: &'static str) -> Self {
        Self {
            permits: Arc::new(Semaphore::new(permits)),
            failure,
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
        .map_err(|_| CoreError::Internal(self.failure.into()))
    }

    #[cfg(test)]
    pub(crate) fn available(&self) -> usize {
        self.permits.available_permits()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering::SeqCst};
    use std::time::Duration;

    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn gate_limits_concurrent_work() {
        let gate = BlockingGate::new(2, "сбой");
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
        let gate = BlockingGate::new(1, "сбой");
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

    #[tokio::test]
    async fn a_panic_in_the_work_is_an_internal_error_with_the_gates_text() {
        let gate = BlockingGate::new(1, "сбой построения документа");
        let result = gate.run(|| -> u8 { panic!("тестовый сбой") }).await;
        assert!(
            matches!(&result, Err(CoreError::Internal(m)) if m == "сбой построения документа"),
            "{result:?}"
        );
        assert_eq!(gate.available(), 1);
    }
}
