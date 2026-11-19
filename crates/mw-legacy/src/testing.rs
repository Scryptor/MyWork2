//! Точки остановки импорта для тестов других крейтов (фича `testing`): импорт из заданной папки
//! один раз останавливается на этапе и ждёт, пока тест его отпустит. Рабочая сборка их не содержит.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::mpsc;

use tokio::sync::oneshot;

use crate::ImportStage;

/// Остановка, назначенная тестом.
pub struct Hold {
    /// Приходит, когда импорт дошёл до точки и ждёт.
    pub reached: oneshot::Receiver<()>,
    /// Отправка (или сброс) отпускает импорт.
    pub release: mpsc::Sender<()>,
}

struct Point {
    root: PathBuf,
    stage: ImportStage,
    reached: oneshot::Sender<()>,
    release: mpsc::Receiver<()>,
}

static POINTS: Mutex<Vec<Point>> = Mutex::new(Vec::new());

/// Импорт из папки `source` один раз остановится: `Copying` — перед первым файлом, до проверки отмены;
/// `Writing` — в начале записи, после последней проверки отмены.
pub fn hold(source: &Path, stage: ImportStage) -> Hold {
    let (reached_tx, reached) = oneshot::channel();
    let (release, release_rx) = mpsc::channel();
    POINTS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(Point {
            root: source.to_path_buf(),
            stage,
            reached: reached_tx,
            release: release_rx,
        });
    Hold { reached, release }
}

/// Останавливает поток, если для этой папки и этапа назначена точка (блокирующее ожидание).
pub(crate) fn pause(root: &Path, stage: ImportStage) {
    let point = {
        let mut points = POINTS.lock().unwrap_or_else(|e| e.into_inner());
        let at = points
            .iter()
            .position(|p| p.root == root && p.stage == stage);
        at.map(|i| points.remove(i))
    };
    if let Some(point) = point {
        point.reached.send(()).ok();
        point.release.recv().ok();
    }
}
