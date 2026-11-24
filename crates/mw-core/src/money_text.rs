//! Суммы в текстах программы: знак валюты из настроек, цифры — как их группирует `Money`. Записи
//! журнала хранятся как есть, поэтому знак в них — тот, что был в настройках в момент записи.

use mw_domain::money::{Kopecks, Money};

use crate::{AppSettings, Core};

/// Знак валюты, если в настройках он пуст (как `DEFAULT_CURRENCY` интерфейса).
const DEFAULT_CURRENCY: &str = "₽";

/// Знак валюты для текстов (документы, журнал): без пробелов по краям, пустой — «₽», как
/// `setCurrencySymbol` интерфейса.
pub(crate) fn currency_symbol(settings: &AppSettings) -> &str {
    match settings.currency_symbol.trim() {
        "" => DEFAULT_CURRENCY,
        symbol => symbol,
    }
}

/// Как сумма пишется в записи журнала.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MoneyText {
    /// «5 000 ₽»: неразрывные пробелы, копейки — только если они есть (запчасти заказа).
    Short,
    /// «4 000,00 ₽»: простые пробелы, копейки всегда (было и стало «Расходов» при переносе склада).
    Exact,
}

impl Core {
    /// Сумма для записи журнала со знаком валюты из настроек.
    pub(crate) fn money_text(&self, sum: Money, style: MoneyText) -> String {
        let settings = self.settings();
        let symbol = currency_symbol(&settings);
        match style {
            MoneyText::Short => format!("{}\u{a0}{symbol}", sum.grouped('\u{a0}', Kopecks::IfAny)),
            MoneyText::Exact => format!("{} {symbol}", sum.grouped(' ', Kopecks::Always)),
        }
    }
}
