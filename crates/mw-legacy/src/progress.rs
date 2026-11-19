//! Ход импорта по этапам: обратный вызов `import_folder` и состояние задачи для интерфейса.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// Этап импорта, по порядку.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ImportStage {
    /// Чтение папки и базы MyWork 2.
    Reading,
    /// Разбор данных.
    Converting,
    /// Копирование файлов в промежуточную папку.
    Copying,
    /// Запись в базу одной транзакцией.
    Writing,
    /// Установка файлов на место.
    Placing,
    /// Готово.
    Done,
}

/// Обратный вызов хода: этап и готовность внутри этапа, 0–100. Вызывается и из потоков копирования.
pub type Progress = Arc<dyn Fn(ImportStage, u8) + Send + Sync>;

/// Состояние импорта, запущенного из интерфейса (одна задача на сервер).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct ImportJob {
    pub stage: ImportStage,
    /// Готовность этапа, 0–100.
    pub percent: u8,
    /// Импорт остановлен с ошибкой (текст по-русски); `stage` — этап, на котором это случилось.
    #[schema(required = true)]
    pub error: Option<String>,
    /// Номер записи импорта, когда база уже записана (см. `GET /legacy-imports/{id}`).
    #[schema(required = true)]
    pub import_id: Option<i64>,
}

impl ImportJob {
    /// Только что запущенный импорт.
    pub fn started() -> ImportJob {
        ImportJob {
            stage: ImportStage::Reading,
            percent: 0,
            error: None,
            import_id: None,
        }
    }

    /// Ещё идёт: не готов и не остановлен ошибкой.
    pub fn is_running(&self) -> bool {
        self.error.is_none() && self.stage != ImportStage::Done
    }
}

/// Счётчик хода по шагам внутри этапа: вызывает обратный вызов, только когда меняется процент.
pub(crate) struct StepProgress<'a> {
    progress: Option<&'a Progress>,
    stage: ImportStage,
    total: usize,
    done: usize,
    shown: Option<u8>,
}

impl<'a> StepProgress<'a> {
    pub(crate) fn new(progress: Option<&'a Progress>, stage: ImportStage, total: usize) -> Self {
        let mut step = StepProgress {
            progress,
            stage,
            total,
            done: 0,
            shown: None,
        };
        step.show();
        step
    }

    pub(crate) fn step(&mut self) {
        self.done = (self.done + 1).min(self.total);
        self.show();
    }

    fn show(&mut self) {
        let percent = (self.done * 100).checked_div(self.total).unwrap_or(0) as u8;
        if self.shown != Some(percent) {
            self.shown = Some(percent);
            if let Some(progress) = self.progress {
                progress(self.stage, percent);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn steps_report_only_changed_percents() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        let progress: Progress = Arc::new(move |s, p| sink.lock().unwrap().push((s, p)));
        let mut step = StepProgress::new(Some(&progress), ImportStage::Copying, 300);
        for _ in 0..300 {
            step.step();
        }
        let seen = seen.lock().unwrap().clone();
        assert_eq!(seen.len(), 101);
        assert_eq!(seen.first(), Some(&(ImportStage::Copying, 0)));
        assert_eq!(seen.last(), Some(&(ImportStage::Copying, 100)));
        StepProgress::new(None, ImportStage::Copying, 0).step();
    }

    #[test]
    fn stage_codes_are_snake_case() {
        let codes: Vec<_> = [
            ImportStage::Reading,
            ImportStage::Converting,
            ImportStage::Copying,
            ImportStage::Writing,
            ImportStage::Placing,
            ImportStage::Done,
        ]
        .iter()
        .map(|s| serde_json::to_value(s).unwrap())
        .collect();
        assert_eq!(
            serde_json::Value::Array(codes),
            serde_json::json!([
                "reading",
                "converting",
                "copying",
                "writing",
                "placing",
                "done"
            ])
        );
        let schema =
            serde_json::to_value(<ImportStage as utoipa::PartialSchema>::schema()).unwrap();
        assert_eq!(
            schema["enum"],
            serde_json::json!([
                "reading",
                "converting",
                "copying",
                "writing",
                "placing",
                "done"
            ])
        );
        assert!(ImportJob::started().is_running());
    }
}
