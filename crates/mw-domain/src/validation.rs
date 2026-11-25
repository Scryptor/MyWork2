use serde::{Deserialize, Serialize};

/// Ошибка валидации поля: имя поля и машинный код (`required`, `invalid`, `too_short`,
/// `duplicate`, `out_of_range`, `estimate_range`, `deleted`, `last_admin`, `last_issued_status`,
/// `comma`, `changes_term`, `stale`, `closed`, `too_many`, `too_long`, `too_large`, `no_room`, `text_budget`, `code_budget`, `typst_error`, `code_timeout`, `code_crashed`, `too_slow`, `code_unavailable`, `too_many_pages`, `too_many_pixels`, `requires_a4`,
/// `requires_a4_landscape`, `not_allowed`, `nested_columns`, `fill_in_columns`, `unsupported`,
/// `mismatch`, `not_editable`, `not_enough_stock`, `builtin_missing`, `not_a_link`, `period_reversed`,
/// `too_many_rows`, `nothing_to_export`, `folder_relative`, `folder_missing`, `folder_inside_data`,
/// `folder_read_only`, `backup_missing`, `backup_damaged`, `backup_too_new`, `backup_pending`, `rack_code`, `rack_too_big`, `places_exceed`, `no_such_place`, `payment_day`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema)]
pub struct FieldError {
    /// Имя поля запроса.
    pub field: String,
    /// Машинный код ошибки.
    pub code: String,
    /// Особое сообщение для человека вместо общего по коду; в API не передаётся отдельным полем.
    #[serde(skip)]
    #[ts(skip)]
    #[schema(ignore)]
    pub message: Option<&'static str>,
}

impl FieldError {
    pub fn new(field: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            code: code.into(),
            message: None,
        }
    }

    /// Ошибка с собственным сообщением (для кода, который в разных местах значит разное).
    pub fn with_message(
        field: impl Into<String>,
        code: impl Into<String>,
        message: &'static str,
    ) -> Self {
        Self {
            message: Some(message),
            ..Self::new(field, code)
        }
    }

    pub fn required(field: impl Into<String>) -> Self {
        Self::new(field, "required")
    }

    /// Сообщение для человека по коду ошибки; неизвестный код — общее сообщение.
    pub fn message_ru(&self) -> &'static str {
        if let Some(m) = self.message {
            return m;
        }
        if self.field == "logo" && self.code == "unsupported" {
            return "Логотип — картинка PNG, JPEG или WebP";
        }
        if self.field == "order_ids" && self.code == "too_many" {
            return "Не больше 100 заказов за раз";
        }
        if self.field == "pages" && self.code == "out_of_range" {
            return "В документе нет выбранных страниц";
        }
        match self.code.as_str() {
            "required" => "Заполните поле",
            "too_short" => "Слишком короткое значение",
            "duplicate" => "Такое значение уже есть",
            "out_of_range" => "Значение вне допустимого диапазона",
            "estimate_range" => "Нижняя граница больше верхней",
            "deleted" => "Запись удалена",
            "last_admin" => "Нельзя оставить программу без администратора",
            "last_issued_status" => "Нужен хотя бы один статус вида «Выдан»",
            "line_break" => "Введите одно значение в одну строку",
            "comma" => "В списке запятая разделяет значения: введите одно значение без запятой",
            "stale" => "Порядок тем временем изменили: обновите список и повторите",
            "changes_term" => {
                "Срок гарантии в заказах изменился бы: замените значение только в справочнике"
            }
            "not_empty" => "База не пуста",
            "replace_unsupported" => "Замена возможна только для базы SQLite",
            "too_many" => "Слишком много элементов",
            "too_long" => "Слишком длинный текст",
            "too_large" => "Файл слишком большой",
            "no_room" => "Поля страницы не оставляют места для содержимого",
            "text_budget" => {
                "В шаблоне слишком много текста: сократите тексты (не больше 20 000 знаков)"
            }
            "code_budget" => "Весь Typst-код шаблона — не больше 50 000 знаков",
            "typst_error" => "Ошибка в Typst-коде",
            "code_timeout" => "Свой Typst-код строится дольше 5 секунд — проверьте циклы",
            "code_crashed" => {
                "Построение документа прервалось — возможно, своему Typst-коду не хватило памяти"
            }
            "too_slow" => "Документы строятся дольше 25 секунд — напечатайте пачку частями",
            "code_unavailable" => "Этот блок нельзя превратить в Typst-код",
            "too_many_pages" => {
                "Документ получается слишком большим (больше 50 страниц на заказ или 200 в пачке): уберите лишнее или увеличьте страницу"
            }
            "too_many_pixels" => "Страница слишком велика для такого разрешения: уменьшите dpi",
            "requires_a4" => {
                "Раскладка «2 на лист A4» — только для страницы A4 в книжной ориентации"
            }
            "requires_a4_landscape" => {
                "Раскладка «Два рядом на альбомном A4» — только для A4 в альбомной ориентации"
            }
            "not_allowed" => "Не подходит для этого документа",
            "nested_columns" => "Колонки нельзя вкладывать в колонки",
            "fill_in_columns" => "Растягивать штрихкод на высоту нельзя внутри колонок",
            "unsupported" => "Файл сделан в более новой версии программы",
            "mismatch" => "Шаблон другого документа",
            "not_editable" => "Этот документ не настраивается",
            "not_enough_stock" => "На складе столько нет",
            "period_reversed" => "Начало периода позже конца",
            "too_many_rows" => {
                "Слишком много строк для одного файла — не больше 50 000, сузьте выборку"
            }
            "nothing_to_export" => "Нет клиентов с телефонами для выгрузки",
            "folder_relative" => "Укажите полный путь к папке",
            "folder_missing" => "Такой папки нет на этом компьютере",
            "folder_inside_data" => {
                "Папка копий не может лежать внутри папки данных — выберите другую или оставьте папку по умолчанию"
            }
            "folder_read_only" => "В эту папку нельзя записать",
            "backup_missing" => "Такой копии нет",
            "backup_damaged" => "Копия повреждена — выберите другую",
            "backup_too_new" => {
                "Копия сделана более новой версией программы — сначала обновите программу"
            }
            "backup_pending" => "Эта копия ждёт восстановления — сначала отмените его",
            "rack_code" => "Код шкафа — от 1 до 8 латинских букв и цифр, хотя бы одна буква",
            "rack_too_big" => "В шкафу не больше 100 ячеек",
            "places_exceed" => "Столько нет — больше, чем не разложено или лежит в ячейке",
            "no_such_place" => "Такого места на складе нет",
            "payment_day" => "День оплаты — не раньше приёма заказа и не позже сегодня",
            _ => "Недопустимое значение",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_codes_have_russian_messages() {
        assert_eq!(
            FieldError::new("rows", "too_many_rows").message_ru(),
            "Слишком много строк для одного файла — не больше 50 000, сузьте выборку"
        );
        assert_eq!(
            FieldError::new("rows", "nothing_to_export").message_ru(),
            "Нет клиентов с телефонами для выгрузки"
        );
    }

    #[test]
    fn not_enough_stock_has_its_own_message() {
        assert_eq!(
            FieldError::new("quantity", "not_enough_stock").message_ru(),
            "На складе столько нет"
        );
    }

    #[test]
    fn import_codes_have_russian_messages() {
        assert_eq!(
            FieldError::new("database", "not_empty").message_ru(),
            "База не пуста"
        );
        assert_eq!(
            FieldError::new("mode", "replace_unsupported").message_ru(),
            "Замена возможна только для базы SQLite"
        );
    }

    #[test]
    fn logo_unsupported_has_its_own_message() {
        assert_eq!(
            FieldError::new("logo", "unsupported").message_ru(),
            "Логотип — картинка PNG, JPEG или WebP"
        );
        assert_ne!(
            FieldError::new("logo", "unsupported").message_ru(),
            FieldError::new("file", "unsupported").message_ru()
        );
    }

    /// Обе раскладки «на A4» называют ориентацию, которой ждут, — и при импорте файла шаблона тоже.
    #[test]
    fn a4_layouts_name_their_orientation() {
        assert_eq!(
            FieldError::new("layout", "requires_a4").message_ru(),
            "Раскладка «2 на лист A4» — только для страницы A4 в книжной ориентации"
        );
        assert_eq!(
            FieldError::new("layout", "requires_a4_landscape").message_ru(),
            "Раскладка «Два рядом на альбомном A4» — только для A4 в альбомной ориентации"
        );
    }

    /// Страницы за концом документа — свой текст, а не общий «вне допустимого диапазона».
    #[test]
    fn pages_out_of_range_has_its_own_message() {
        assert_eq!(
            FieldError::new("pages", "out_of_range").message_ru(),
            "В документе нет выбранных страниц"
        );
        assert_ne!(
            FieldError::new("dpi", "out_of_range").message_ru(),
            "В документе нет выбранных страниц"
        );
    }

    #[test]
    fn field_error_required() {
        assert_eq!(
            FieldError::required("brand"),
            FieldError::new("brand", "required")
        );
    }
}
