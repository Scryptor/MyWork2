//! Помощники тестов других крейтов (фича `testing`), рабочая сборка их не содержит:
//! - точки остановки импорта и проверки папки: импорт (или проверка) из заданной папки один раз
//!   останавливается и ждёт, пока тест его отпустит;
//! - импорт «как до плана 8.1» — база для тестов переноса склада из прошлого импорта.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::mpsc;

use mw_core::{Actor, Core, ImportedOrder, ImportedStock};
use mw_domain::dictionary::{DEFAULT_DICTIONARY, DictionaryKind};
use tokio::sync::oneshot;

use crate::convert::ConvertSteps;
use crate::convert::stock::StockConversion;
use crate::folder::LegacyFolder;
use crate::report::{Warning, Warnings};
use crate::source::LegacyDb;
use crate::{ImportOptions, ImportReport, ImportStage, LegacyError, WarningCode};

/// Импорт папки так, как его делала программа до плана 8.1 (база, которую потом обновили до 8.1):
/// склада нет, «расходы» заказов — затраты MyWork 2 как есть, следы `Stock`, `StockMap` и фото склада —
/// без цели, справочники — без значений склада, «Типы запчастей» — стандартные (их завела миграция
/// `m20261124_000002_stock_data`) и без строки отчёта, в отчёте — `stock_preserved` и нет строк о
/// ячейках и колонках `Stock`/`StockMap` и о файле `TypeOvZIP.txt` (их тогда не проверяли). Остальное
/// (пакет, файлы, отчёт) — как у `import_folder`.
pub async fn import_before_stock(
    core: &Core,
    actor: &Actor,
    options: &ImportOptions,
) -> Result<ImportReport, LegacyError> {
    let steps = ConvertSteps {
        stock: preserved_stock,
        warnings: warnings_before_stock,
    };
    crate::import_folder_with(core, actor, options, steps).await
}

/// Шаг склада до плана 8.1: таблицы склада только сохраняются следами; «Типы запчастей» — стандартные.
fn preserved_stock(
    db: &LegacyDb,
    _folder: &LegacyFolder,
    _orders: &mut [ImportedOrder],
    dictionary: &mut [(DictionaryKind, Vec<String>)],
    w: &mut Warnings,
) -> StockConversion {
    let defaults = DEFAULT_DICTIONARY
        .iter()
        .find(|(kind, _)| *kind == DictionaryKind::PartType)
        .map(|(_, values)| values.iter().map(|v| (*v).to_owned()).collect())
        .unwrap_or_default();
    if let Some((_, values)) = dictionary
        .iter_mut()
        .find(|(kind, _)| *kind == DictionaryKind::PartType)
    {
        *values = defaults;
    }
    if let Some(stock) = db.table("Stock").filter(|t| !t.rows.is_empty()) {
        w.push(
            WarningCode::StockPreserved,
            "Stock",
            "",
            "",
            &stock.rows.len().to_string(),
            "Склад сохранён в исходном виде до появления модуля склада.",
        );
    }
    StockConversion {
        stock: ImportedStock::default(),
        cost_changes: Vec::new(),
        dictionary_additions: Vec::new(),
        stock_targets: Vec::new(),
        part_targets: Vec::new(),
    }
}

/// Строк, которых до плана 8.1 в отчёте не было, нет.
fn warnings_before_stock(w: &mut Warnings) {
    w.retain(|x| !added_by_stock_module(x));
}

/// Строка отчёта, которую выдаёт только импорт со складом: испорченная ячейка или неизвестная
/// колонка `Stock`/`StockMap`, строка справочника «Типы запчастей», строки о файле `TypeOvZIP.txt`.
fn added_by_stock_module(x: &Warning) -> bool {
    crate::migrate_stock::stock_cell_warning(x)
        || (x.code == WarningCode::DictionaryFromOrders
            && x.key == DictionaryKind::PartType.label_ru())
        || x.key == "settings/Stock/TypeOvZIP.txt"
}

/// Остановка, назначенная тестом.
pub struct Hold {
    /// Приходит, когда импорт дошёл до точки и ждёт.
    pub reached: oneshot::Receiver<()>,
    /// Отправка (или сброс) отпускает импорт.
    pub release: mpsc::Sender<()>,
}

struct Point {
    root: PathBuf,
    /// Этап импорта; `None` — проверка папки.
    stage: Option<ImportStage>,
    reached: oneshot::Sender<()>,
    release: mpsc::Receiver<()>,
}

static POINTS: Mutex<Vec<Point>> = Mutex::new(Vec::new());

/// Импорт из папки `source` один раз остановится: `Copying` — перед первым файлом, до проверки отмены;
/// `Writing` — в начале записи, после последней проверки отмены.
pub fn hold(source: &Path, stage: ImportStage) -> Hold {
    hold_at(source, Some(stage))
}

/// Проверка папки `source` (`check_folder`) один раз остановится перед чтением папки.
pub fn hold_check(source: &Path) -> Hold {
    hold_at(source, None)
}

fn hold_at(source: &Path, stage: Option<ImportStage>) -> Hold {
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
    pause_at(root, Some(stage));
}

/// То же для проверки папки.
pub(crate) fn pause_check(root: &Path) {
    pause_at(root, None);
}

fn pause_at(root: &Path, stage: Option<ImportStage>) {
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
