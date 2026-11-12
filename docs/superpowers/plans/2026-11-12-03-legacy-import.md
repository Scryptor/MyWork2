# План 3. Импорт MyWork 2 — план реализации

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** команда `mw-server import-legacy <папка>` переносит папку MyWork 2 любой версии (база с 24–32 колонками
`Catalog`, `Config.ini`, `settings/*.txt` в Windows-1251, тексты актов, файлы клиентов, фото склада) в базу MyWork 3
на SQLite или PostgreSQL одной транзакцией и ничего не теряет: всё неразобранное попадает в отчёт и сохраняется
в исходном виде.

**Architecture:** сначала модель данных доводится под решения автора: у клиента список телефонов, гарантия хранится
как текст плюс срок, реквизиты и тексты актов лежат в настройках, поиск идёт с NFC, у индекса есть версия и
автоматическая переиндексация. Новый крейт `mw-legacy` копирует старую базу во временную папку, читает её через `sqlx`,
разбирает значения и собирает `mw_core::ImportBundle` с предупреждениями. Новый метод `Core::import_bundle` в одной
транзакции записи стирает стартовые данные и пишет всё с исходными номерами, сдвигает нумерацию и сверяет число строк.
Файлы копируются в `attachments/` через промежуточную папку и встают на место только после успешной записи в базу.
Запуск — подкоманда `mw-server`.

**Tech Stack:** Rust 1.95 (edition 2024); sqlx 0.9 (SQLite, только чтение копии старой базы); encoding_rs
(Windows-1251); unicode-normalization (NFC); sha2 (контроль копий файлов); tempfile; SeaORM 2.0, Axum, utoipa и ts-rs
из планов 1–2.

**Spec:** `docs/superpowers/specs/2026-11-11-mywork3-crossplatform-design.md`: раздел 2 (анти-цели), 4.2, «Где лежат
данные», 5 (п. 3, 4, 8, 9, 10), 6 (весь), 7, 10. Также разбор старой программы `docs/legacy/mywork2-analysis.md`,
старый код `legacy/MyWork2/` (прежде всего `BDWorker.cs`, `Form1.cs`, `Settings.cs`) и наблюдения по реальной базе
автора `.superpowers/legacy-real-db-findings.md` (локальный файл, не в git). Дорожная карта с заметками «План 3» —
`docs/superpowers/plans/README.md`. Образцы стиля — планы 1 и 2.

## Global Constraints

- Корень репозитория — `/Users/scrypto/dev/MyWork2`. Работа идёт в ветке `plan-3-legacy-import`; в `main` она вливается перемоткой после зелёных проверок и ревью всей ветки. Удалённого репозитория нет, ничего не отправляется.
- Rust toolchain `1.95.0`, edition 2024, крейты в `crates/`, имена `mw-*`. Новый крейт наследует `edition`, `license`, `rust-version` и `[lints]` из воркспейса.
- Перед каждым коммитом зелёные: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo check --workspace && cargo test --workspace`. PostgreSQL в тестах — `MW_TEST_PG_URL=postgres://postgres:postgres@localhost:55432/postgres`, контейнер `mw-pg` (`docker start mw-pg`).
- Каждый тест с базой MyWork 3 идёт по `mw_db::testing::all_backends()` (в `mw-core` и `mw-legacy` — через `cores()`). Старая база-источник — всегда SQLite-файл, собранный фикстурой.
- Деньги — только `Money`. Время — `Core::now()` и `Core::now_utc()`; даты из старой базы переносятся как местное время без пересчёта. Поиск без учёта регистра — только `mw_domain::search`. Любая запись в базу MyWork 3 — только через `Core::begin_write()`.
- `mw-domain` без ввода-вывода. В базу MyWork 3 ходят только `mw-db` и `mw-core`, права проверяет `mw-core`. `mw-legacy` читает только копию старой базы и файлы старой папки, а пишет только через `Core::import_bundle`.
- Идентификаторы — английские. Всё, что видит человек (отчёт, предупреждения, вывод команды, новые статусы, сводки журнала), — по-русски.
- Реальная база автора открывается только на чтение (`file:…?mode=ro&immutable=1`). Её нельзя копировать в репозиторий, фикстуры, тесты и логи. В переписку и в итоговые документы попадают только счётчики и формы значений («99-99-9999»), без ФИО, телефонов, адресов и имён мастеров.
- Фикстуры только генерируются кодом (`mw_legacy::fixture`). Бинарных баз в git нет, все имена и номера в фикстурах вымышлены.
- Исходная папка MyWork 2 не меняется: база копируется во временную папку и читается оттуда, остальные файлы только читаются.
- Секции `[SMSSEND]` (в ней токен) и `[ACTIVATION]` из `Config.ini` не читаются и нигде не сохраняются. Пароли и хэши паролей не попадают ни в журнал, ни в логи, ни в отчёт.
- Логи импорта (`tracing`) содержат только этапы и счётчики, без значений из старой базы.
- Выпущенные миграции не меняются, только добавляются новые.
- После изменения API: `MW_UPDATE_CONTRACT=1 cargo test -p mw-api --test contract -- --test-threads=1` (если `every_openapi_schema_has_a_typescript_type` один раз упал — затем `cargo test -p mw-api --test contract`), коммит `openapi.json` и `bindings/` вместе с кодом. Входные типы — `deny_unknown_fields`, правки — `patch::present` / `patch::nullable`, `#[ts(export)]` не ставить.
- Зависимости только MIT / Apache-2.0 / BSD / ISC / Zlib / Unicode. Новые: `encoding_rs` (Apache-2.0 OR MIT, данные BSD-3-Clause), `unicode-normalization` (MIT OR Apache-2.0), `tempfile` (MIT OR Apache-2.0).
- Каждое сообщение коммита заканчивается строкой `Co-Authored-By: Claude <модель исполнителя> <noreply@anthropic.com>`.

## Review Focus

1. **Сбой посреди импорта.** Неверная строка, ошибка СУБД или файловой системы на любом этапе: в базе MyWork 3 ничего не изменилось (стартовые статусы на месте), промежуточная папка с файлами удалена, повторный запуск проходит. Тесты: `broken_bundle_rolls_back_everything` (задача 7), `staging_is_removed_on_abort` и `failed_import_leaves_attachments_untouched` (задача 12).
2. **Исходная папка не меняется.** Хэши всех файлов старой папки, включая `baza.sqlite` и `baza.sqlite-journal`, после импорта совпадают с прежними; временная копия базы удалена. Тесты: `source_folder_is_not_modified` (задача 8), `source_folder_is_untouched_by_full_import` (задача 12).
3. **Нумерация после импорта на обеих СУБД.** Если последние заказы в MyWork 2 удаляли (`sqlite_sequence` больше наибольшего id), новый заказ получает следующий за `sqlite_sequence` номер, а новый клиент — номер после восстановленных клиентов. Это одинаково на SQLite и PostgreSQL. Тесты: `import_into_empty_database_keeps_numbers_and_continues_them` (задача 7), `every_schema_version_imports_without_losing_rows` (задача 12).
4. **Вход после импорта.** Старые пароли подходят. Логины, отличавшиеся в MyWork 2 только регистром, оба работают (второй переименован). Хотя бы у одного пользователя есть право «Пользователи и права». Если в MyWork 2 не было пользователей, остаётся режим без входа. Тесты: `access_maps_groups_logins_and_hashes` (задача 10), `legacy_users_log_in_after_import` и `open_mode_kept_when_old_program_had_no_users` (задача 12).
5. **Секреты и персональные данные.** Токен SMS из `Config.ini` не попадает ни в одну таблицу и ни в отчёт. Краткая сводка (то, что печатает команда) не содержит ни ФИО, ни телефонов, ни адресов. Тесты: `settings_come_from_config_and_act_texts` (задача 10), `sms_token_is_never_stored_and_summary_has_no_personal_data` (задача 12), `import_legacy_command_imports_fixture_and_writes_report` (задача 13).

## Решения этого плана

**Решения с автором.** Автор не ответил на вопросы о телефонах и гарантии, поэтому взяты рекомендованные варианты. Их можно изменить при утверждении плана.

- **Телефоны — список.** Таблица `client_phones`: номер (только цифры), ключ сравнения и пометка («жена», «раб»). Первый номер основной: его копия остаётся в `clients.phone` и `clients.phone_key`, поэтому списки заказов и акты не меняются. Поиск, автопоиск при приёме и поиск «по номеру» идут по любому номеру клиента. В API `Client.phones: ClientPhone[]` заменяет `phone`.
- **Гарантия — текст и срок.** Текст хранится в `orders.warranty_text` и печатается в акте как есть. Срок выводится из текста по правилам `parse_warranty_term`: в днях (`warranty_days`), в месяцах (`warranty_months`) или не определён. «Без гарантии» и «0» дают 0 дней. Составные тексты остаются без срока. В API `NewOrder.warranty` и `OrderPatch.warranty` — текст, `Order.warranty_term` — только для чтения. Настройка `default_warranty` — текст, по умолчанию «30 дней». Варианты для выбора — справочник `warranty`, импорт берёт его из `Garanty.txt`.

**Решения Claude** (записаны, чтобы ревью и следующие планы на них опирались):

- **Склад переносится в исходном виде.** Строки `Stock` и `StockMap` и файлы `settings/Stock/*.txt` целиком сохраняются в `legacy_rows`, фото копируются в `attachments/stock/<id позиции>/`. Таблицы склада и перенос в них — план 8: склад с картой мест хранения проектируется с автором. `Zatrati` заказа переносится как есть, то есть вместе с запчастями из `StockMap`; план 8 при вводе запчастей заказа не должен их задвоить.
- **Вложения.** Файлы копируются в окончательные папки `attachments/orders/<номер заказа>/…` (с подпапками) и `attachments/stock/<id>/…`. Учёт файлов ведётся строками `legacy_rows` (источник `ClientFiles` или `Stock.Photo`, размер, sha256, куда скопирован). Таблицу `attachments` и показ файлов в интерфейсе делает план 5. Файлы без заказа (папка несуществующего заказа или файл прямо в папке клиента) идут в `attachments/legacy/ClientFiles/…` и попадают в отчёт.
- **Следы импорта.** Таблица `legacy_imports` — один импорт и его отчёт в JSON. Таблица `legacy_rows` — по строке на каждую исходную строку, ключ настройки и файл, с полным исходным значением в JSON и ссылкой на новую запись (`target_table`, `target_id`). UNIQUE(`import_id`, `source`, `source_key`) не даёт записать одну исходную строку дважды. Строки `StatesMap` и `HistoryBD` переходят в `audit_log`, исходная строка лежит в `details.legacy` самой записи журнала.
- **История в журнале.** Коды действий: `legacy.status`, `legacy.master`, `legacy.flag`, `legacy.message`, `legacy.event` (из `StatesMap`), `legacy.history` (из `HistoryBD`), `legacy.imported` (сам импорт). Сводки сохраняют исходные слова, например «Установлен статус «Ждет ЗИП»» или «СОХРАНЕНИЕ». Записи `StatesMap` идут без автора, у записей `HistoryBD` автор — `WHO` как есть.
- **Пустая база.** В ней нет клиентов, заказов, пользователей и записей журнала. Импорт заменяет целиком статусы, точки, справочники, роли и настройки (кроме часового пояса), поэтому правки стартовых данных до импорта не сохраняются.
- **Повторный импорт (`--replace`) — только для SQLite.** Сначала `VACUUM INTO` в `backups/<время>/mywork.db`, затем в одной транзакции всё стирается и пишется заново. Старые вложения после успешной записи переносятся в `backups/<время>/attachments/`. На PostgreSQL импорт идёт только в пустую базу: копию делает `pg_dump`, замену с копией из программы добавит план 9.
- **Чтение старой базы через `sqlx`, а не `rusqlite` из спеки.** `sqlx` уже собирает SQLite (`libsqlite3-sys` 0.37 bundled), а второй экземпляр `libsqlite3-sys` Cargo не разрешит (`links = "sqlite3"`). Ячейка читается как класс хранения (`typeof`) плюс байты (`CAST(… AS BLOB)`), поэтому числа, NULL, BLOB и испорченный UTF-8 сохраняются точно.
- **INI — свой разборщик, без `rust-ini`.** Причины: кодировка определяется до разбора, а повторы ключей и регистр нужно обрабатывать как `GetPrivateProfileString` (первый ключ побеждает, имена без учёта регистра).
- **Кодировка текстовых файлов.** Если есть BOM, файл читается по нему. Иначе корректный UTF-8 хотя бы с одним не-ASCII символом читается как UTF-8 (со сведением в отчёте). Всё остальное читается как Windows-1251.
- **Имена файлов и папок старой программы** (`baza.sqlite`, `Config.ini`, `settings`, `Akts`, `Stock`, `Photos`, `ClientFiles`, `*.txt`, `Shablon_*.htm`) ищутся без учёта регистра: папку могли скопировать с Windows на macOS или Linux.
- **Даты.** Сначала пробуются форматы, которые писала MyWork 2, затем общий разбор «день первым» (ru-RU) с разделителями `-` `.` `/` и необязательным временем `H:mm[:ss]`, `HH-mm` и AM/PM. Дата через «/» с месяцем больше 12 читается как «месяц/день» с предупреждением. Год 1990–2100, иначе дата не разобрана. Дата без времени — 00:00.
- **Деньги.** «Предварительная стоимость» переходит в диапазон оценки с равными границами; «0» и пусто — оценки нет. Итоговая стоимость «0» у невыданного заказа (статус не вида `issued` и нет даты выдачи) — итога ещё нет; у выданного — 0 ₽ («без ремонта»). Предоплата, затраты и скидка: пусто — 0. Сумма больше 1 000 000 ₽ переносится как есть, с предупреждением.
- **Статусы.** «Ждет ЗИП» переименовывается в «Ждёт запчасть» (как делала MyWork 2 при обновлении базы). Совпадение со стандартным статусом определяется после `normalize_search`. Нестандартный статус получает вид `issued`, если у всех его заказов есть дата выдачи, иначе `open`; цвет `gray` или `blue`. Пустой статус переносится как «Без статуса» по тому же правилу.
- **Клиенты.** `ClientsMap` — источник истины, номера клиентов сохраняются. Для заказа с пустым или битым `ClientId` клиент восстанавливается из полей заказа и объединяется по ключу (ФИО и телефон после нормализации). Если в заказе нет ни ФИО, ни телефона, создаётся отдельный клиент «Клиент не указан» на каждый такой заказ. В базах без `ClientId` все клиенты строятся из заказов по тому же ключу. Похожие клиенты не объединяются: объединение — функция интерфейса.
- **Заказ без даты приёма.** Подставляется самая ранняя дата из его истории, иначе дата приёма ближайшего заказа с меньшим номером, иначе с большим, иначе время импорта. Каждая подстановка — предупреждение.
- **Удалённые заказы (`Deleted` = «1»).** Дата удаления — последняя запись «УДАЛЕНИЕ» в `HistoryBD` для этого заказа, иначе время импорта.
- **Права групп** — по таблице в «Карте переноса». Флаг `settings` даёт ещё `users_manage` и `audit_view`: в MyWork 2 пользователи и журнал были внутри настроек. У флагов `sms` и `dates` нет соответствия. Если пользователи есть, но права «Пользователи и права» нет ни у одной их роли, оно и `audit_view` выдаются роли первого пользователя (с предупреждением).
- **Логины, совпадающие без учёта регистра.** Первый (по id) остаётся как был, следующие получают суффикс « (2)», « (3)». Пароль у них тот же.
- **Справочники** берутся из `settings/*.txt`. Если файла нет или он совпадает со стартовым файлом MyWork 2 (пользователь его не правил), берутся стартовые значения MyWork 3. Строки-подсказки «Этот текст можно», «поменять в файле», «settings/….txt» пропускаются. Значения из заказов в справочники не добавляются.
- **Тексты актов** из `settings/Akts/*.txt` попадают в настройки. `<br>` с последующим переводом строки становится переводом строки, прочий HTML остаётся как есть (это забота плана 6). Стартовые заготовки «Название Вашей Фирмы», «тел.: Вашей фирмы» и «ИП Какойктото…» не переносятся.
- **`Config.ini`.** Секция компьютера — любая, кроме `ACTS`, `BARCODE`, `CHECKBOX`, `PROGRAMM_SETTINGS`, `SMSSEND`, `EXPORT`, `ACTIVATION`. Если таких секций несколько, используется первая по порядку в файле, остальные попадают в отчёт. Определить серийник диска нынешнего компьютера на macOS и Linux нельзя. Ключи без соответствия (цвета, ширины колонок, путь копий, размер штрихкода…) сохраняются в `legacy_rows` для планов 5, 6 и 8.
- **Поиск.** `normalize_search` сначала приводит строку к NFC. `SEARCH_INDEX_VERSION` хранится в настройке `search_index_version`. Если версия в базе другая, `Core::open` переиндексирует клиентов и заказы. Ручная переиндексация — `Core::reindex_search`.
- **Импорт запускается только командой.** Это `mw-server import-legacy`. Экран отчёта, запуск из интерфейса и долгая операция с прогрессом — план 5, загрузка папки на удалённый сервер — план 9.
- **Даты записей.** У заказа `created_at` = `updated_at` = дата приёма. У клиента — дата из `ClientsMap`, иначе самая ранняя дата приёма его заказов, иначе время импорта. У пользователей и ролей — время импорта. Версия всех записей — 1.

## Карта переноса

| Источник | Куда |
|---|---|
| `Catalog` | `orders` (номер = `id`), трек-номера → `order_tracking`, строка целиком → `legacy_rows` |
| `ClientsMap` | `clients` (номер = `id`) + `client_phones`, строка → `legacy_rows` |
| `StatesMap` | `audit_log` (`legacy.status` / `.master` / `.flag` / `.message` / `.event`), строка → `details.legacy` |
| `HistoryBD` | `audit_log` (`legacy.history`), строка → `details.legacy` |
| `GroupDostup` | `roles` + роль «Администратор» со всеми правами (всегда первая) |
| `Users` | `users`, схема пароля `legacy_sha1`, хэш в нижнем регистре, `display_name` = логин |
| `Stock`, `StockMap`, неизвестные таблицы | `legacy_rows` |
| `sqlite_sequence` | следующий номер заказа и клиента; строки → `legacy_rows` |
| `settings/ustrojstvo.txt`, `brands.txt`, `DeviceColour.txt`, `komplektonst.txt`, `sostoyaniePriema.txt`, `neispravnost.txt`, `vipolnRaboti.txt`, `masters.txt`, `aboutUs.txt`, `Garanty.txt` | справочники `device_type`, `brand`, `color`, `completeness`, `condition`, `malfunction`, `work_done`, `master`, `client_source`, `warranty` |
| `settings/AdresSC.txt` и значения `AdressSC` в заказах | `service_points` |
| `settings/Akts/FirmName.txt`, `Phone.txt`, `DannieOFirme.txt`, `URDannie.txt`, `DogovorTextPriem.txt`, `DogovorTextVidacha.txt` | настройки `firm_name`, `firm_phone`, `firm_details`, `firm_legal_details`, `intake_terms`, `issue_terms` |
| остальные `settings/**/*.txt`, `Shablon_*.htm` | `legacy_rows` (источник `file`) |
| `Config.ini` | настройки (таблица ниже), все ключи → `legacy_rows` (источник `Config.ini`) |
| `ClientFiles/<клиент>/<заказ>/**`, фото из `Stock.Photo*` | `attachments/…`, учёт → `legacy_rows` |
| `settings/backup/*` | не переносится (резервные копии самой MyWork 2) |

**Колонки `Catalog` → `orders`** (отсутствующая колонка = пусто; текст обрезается по краям, регистр не меняется):
`Data_priema` → `received_at`; `Data_vidachi` → `issued_at`; `Data_predoplaty` → `prepaid_at`;
`WhatRemont` → `device_type`; `brand`; `model`; `SerialNumber` → `serial`; `DeviceColour` → `color`;
`komplektonst` → `completeness`; `sostoyanie` → `device_condition`; `polomka` → `malfunction`;
`vipolnenie_raboti` → `work_done`; `kommentarij` → `internal_comment`; `master`; `Barcode` → `barcode` (пусто →
`order_barcode(id)`); `predvaritelnaya_stoimost` → `estimate_from` = `estimate_to`; `Predoplata` → `prepayment`;
`Zatrati` → `extra_costs`; `okonchatelnaya_stoimost_remonta` → `final_price`; `Skidka` → `discount`;
`Garanty` → `warranty`; `Status_remonta` → `status_id`; `AdressSC` → `service_point_id`; `Image_key` → `need_call`;
`wait_zakaz` → `need_parts`; `Deleted` → `deleted_at`; `ClientId` → `client_id`. Колонки `surname`, `phone`, `Adress`,
`AboutUs` читаются, только когда клиента нет в `ClientsMap`.

**Флаги групп → права:** `addZapis` → `orders_create`; `saveZapis` → `orders_edit`; `delZapis` → `orders_delete`;
`editDates` → `order_dates_edit`; `graf` → `reports_view`; `stock` → `stock_view`; `stockAdd` → `stock_create`;
`stockEdit` → `stock_edit`; `stockDel` → `stock_delete`; `clients` → `clients_view` + `clients_edit`;
`clientAdd` → `clients_create`; `clientDel` → `clients_delete`; `clientConcat` → `clients_merge`;
`settings` → `settings_manage` + `users_manage` + `audit_view`; `sms`, `dates` — без соответствия.

**`Config.ini` → настройки:**

| Секция и ключ | Настройка |
|---|---|
| `[PROGRAMM_SETTINGS] valuta` | `currency_symbol` (1–8 символов) |
| `[PROGRAMM_SETTINGS] Mpersent` | `master_percent` (0–100) |
| `[PROGRAMM_SETTINGS] daysDiagnostik` | `diagnostics_overdue_days` = модуль числа (MyWork 2 хранила его со знаком минус), 1–365 |
| `[PROGRAMM_SETTINGS] NessesaryCHB` | `intake_required_fields`: `Unchecked` → false, `Checked` → true |
| `[CHECKBOX] garantyDefault` = `Checked` | `default_warranty` = «Без гарантии» |
| секция компьютера `AdressSCDefault` | `default_service_point_id`: номер строки `AdresSC.txt` (с нуля, все строки файла по порядку, как в выпадающем списке MyWork 2) |
| секция компьютера `MasterDefault` | `default_master` |

Значение, которое не разбирается, даёт предупреждение `setting_invalid`, и настройка остаётся стандартной.

## Предупреждения отчёта

У каждого предупреждения есть код, источник (`Catalog`, `ClientsMap`, `StatesMap`, `HistoryBD`, `Users`, `GroupDostup`,
`Stock`, `Config.ini`, `file`), ключ (id строки, штрихкод, имя секции или путь), поле, исходное значение и пояснение
по-русски. Коды стабильны, тесты сверяют их.

| Код | Уровень | Заголовок в отчёте |
|---|---|---|
| `date_unparsed` | warning | Не разобрана дата |
| `date_month_first` | warning | Дата прочитана как «месяц/день» |
| `received_at_missing` | warning | Нет даты приёма — подставлена |
| `money_unparsed` | warning | Не разобрана сумма |
| `amount_suspicious` | warning | Подозрительно большая сумма |
| `flag_unrecognized` | warning | Не распознан флаг |
| `status_empty` | warning | Заказ без статуса |
| `issued_without_date` | warning | Выдан без даты выдачи |
| `order_without_client` | warning | Заказ без клиента |
| `client_missing` | warning | Клиент не найден — восстановлен из заказа |
| `client_empty` | warning | Клиент без имени и телефона |
| `barcode_duplicate` | warning | Одинаковый штрихкод у нескольких заказов |
| `barcode_invalid` | warning | Штрихкод не по формату UPC-A |
| `phone_unusual` | warning | Необычный номер телефона |
| `phone_no_digits` | warning | Телефон без цифр перенесён в заметку |
| `history_orphan` | warning | История ссылается на несуществующий заказ |
| `login_renamed` | warning | Логин переименован |
| `user_group_missing` | warning | Группа пользователя не найдена |
| `user_password_invalid` | warning | Пароль пользователя не перенесён |
| `users_manage_granted` | warning | Право «Пользователи и права» выдано роли |
| `config_section_ignored` | warning | Пропущены настройки другого компьютера |
| `setting_invalid` | warning | Не разобрана настройка |
| `text_invalid_utf8` | warning | Испорченный текст в базе |
| `unknown_column` | warning | Неизвестная колонка старой базы |
| `unknown_table` | warning | Неизвестная таблица старой базы |
| `file_missing` | warning | Файл не найден |
| `file_unreadable` | warning | Файл не прочитан |
| `status_renamed` | info | Статус переименован |
| `status_custom` | info | Добавлен статус из старой базы |
| `phone_split` | info | Телефоны разложены по номерам |
| `client_duplicates` | info | Похожие клиенты (одинаковые ФИО и телефон) |
| `barcode_generated` | info | Создан штрихкод |
| `tracking_found` | info | Найдены трек-номера |
| `role_renamed` | info | Группа переименована |
| `dictionary_placeholder_skipped` | info | Пропущены строки-подсказки |
| `dictionary_default_kept` | info | Список не менялся — взят стандартный |
| `act_text_default_skipped` | info | Заготовка текста акта не перенесена |
| `text_decoded_utf8` | info | Файл прочитан как UTF-8 |
| `setting_preserved` | info | Настройка сохранена до появления в программе |
| `stock_preserved` | info | Склад сохранён до модуля склада |
| `client_files_orphan` | info | Файлы без заказа |
| `next_number` | info | Следующий номер заказа |

## Схема (новые миграции)

Строится построителем SeaQuery, одинаково для SQLite и PostgreSQL; новые миграции идут в транзакции, как первая.

| Миграция | Что делает |
|---|---|
| `m20261112_000001_legacy_import` (задача 2) | `client_phones`: `client_id` → clients (CASCADE), `position INT NOT NULL` (0 — основной), `phone`, `phone_key`, `note` (`TEXT NOT NULL DEFAULT ''`); UNIQUE(`client_id`, `position`), INDEX(`phone_key`). `orders`: `warranty_text TEXT NOT NULL DEFAULT ''`, `warranty_months INT NULL` (`warranty_days` остаётся — срок в днях). `order_tracking`: `order_id` → orders (RESTRICT), `position INT NOT NULL`, `number TEXT NOT NULL`, `description`; UNIQUE(`order_id`, `position`). `legacy_imports`: `imported_at TIMESTAMP NOT NULL`, `source_path`, `mode` (`into_empty` / `replace`), `report TEXT NOT NULL` (JSON). `legacy_rows`: `import_id` → legacy_imports (CASCADE), `source TEXT NOT NULL`, `source_key TEXT NOT NULL`, `target_table TEXT NULL`, `target_id BIGINT NULL`, `raw TEXT NOT NULL` (JSON); UNIQUE(`import_id`, `source`, `source_key`), INDEX(`target_table`, `target_id`). Перенос существующих строк: клиент с непустым `phone` → строка `client_phones` (position 0, тот же `phone_key`, пустая пометка); `warranty_text` = «Без гарантии» при `warranty_days = 0`, «{N} дн.» при N > 0, пусто при NULL. |
| `m20261112_000002_warranty_dictionary` (задача 2) | справочник `warranty` литералами: «Без гарантии», «14 дней», «30 дней», «3 месяца», «6 месяцев», «1 год» (`sort_order` 0–5) |
| `m20261112_000003_default_warranty_text` (задача 5) | настройка `default_warranty_days` (JSON-число N или null) → `default_warranty` (JSON-строка: «Без гарантии» при 0, «{N} дн.» при N > 0, «» при null); старый ключ удаляется |

## Файловая структура

```
Cargo.toml                         + unicode-normalization, encoding_rs, tempfile в [workspace.dependencies]
crates/mw-domain/src/              search.rs (NFC) warranty.rs (новый) dictionary.rs (warranty) validation.rs (коды) lib.rs
crates/mw-db/src/                  backup.rs (новый) testing.rs lib.rs
crates/mw-db/src/entity/           client_phone.rs order_tracking.rs legacy_import.rs legacy_row.rs (новые) order.rs mod.rs
crates/mw-db/src/migration/        m20261112_000001_legacy_import.rs m20261112_000002_warranty_dictionary.rs
                                   m20261112_000003_default_warranty_text.rs mod.rs
crates/mw-core/src/                clients.rs settings.rs search_index.rs (новый) core.rs lib.rs orders/{types,create,update}.rs
crates/mw-core/src/import/         mod.rs (типы) write.rs (запись)
crates/mw-core/tests/              search_index.rs import.rs (новые) + правки clients/orders/settings/api_types/common
crates/mw-api/                     openapi.json bindings/ tests/{clients,orders,catalogs,common}.rs
crates/mw-legacy/                  Cargo.toml examples/make_fixture.rs
crates/mw-legacy/src/              lib.rs error.rs source.rs text.rs folder.rs report.rs files.rs fixture.rs
crates/mw-legacy/src/parse/        mod.rs dates.rs money.rs flags.rs phones.rs states.rs tracking.rs
crates/mw-legacy/src/convert/      mod.rs catalogs.rs settings.rs access.rs clients.rs orders.rs history.rs
crates/mw-legacy/tests/            common/mod.rs import.rs real_db.rs
crates/mw-server/src/              cli.rs import.rs (новый) lib.rs config.rs (ServerError::Import)
crates/mw-server/tests/            import.rs (новый)
AGENTS.md  README.md  docs/superpowers/plans/README.md
```

## Синтетическая папка MyWork 2 (фикстура)

`mw_legacy::fixture::write_fixture(dir, &FixtureOptions { version, extra_orders })` собирает папку старой программы
кодом (sqlx и `std::fs`). Все имена и номера в ней вымышлены. Ниже — эталон для версии `V32`, на нём стоят тесты
задач 8–12; остальные версии получают те же строки без отсутствующих колонок и таблиц.

**Версии** (набор колонок `Catalog` и таблиц; добавленные колонки создаются `ALTER TABLE Catalog ADD COLUMN X` без
типа, как это делала MyWork 2):

| Версия | `Catalog` | Другие таблицы |
|---|---|---|
| `V24` | `id, Data_priema, Data_vidachi, Data_predoplaty, surname, phone, AboutUs, WhatRemont, brand, model, SerialNumber, sostoyanie, komplektonst, polomka, kommentarij, predvaritelnaya_stoimost, Predoplata, Zatrati, okonchatelnaya_stoimost_remonta, Skidka, Status_remonta, master, vipolnenie_raboti, Garanty` (все `TEXT`, `id INTEGER PRIMARY KEY AUTOINCREMENT`) | — |
| `V25` | + `wait_zakaz` | — |
| `V26` | + `Adress` (обновление оборвалось на полпути) | — |
| `V27` | + `Adress`, `Image_key` | `StatesMap` |
| `V29` | + `AdressSC`, `DeviceColour` | + `Stock` |
| `V30` | + `ClientId` | + `ClientsMap`, `StockMap` |
| `V31` | + `Barcode` | + `HistoryBD`, `Users`, `GroupDostup` |
| `V32` | + `Deleted` | все |

Схемы `ClientsMap`, `StatesMap`, `HistoryBD`, `Stock`, `StockMap`, `Users`, `GroupDostup` — как в `CREATE TABLE`
из `BDWorker.cs`. В `sqlite_sequence` записаны `Catalog` = 15 и `ClientsMap` = 12 (последние строки «удалены»).
`FixtureOptions.extra_orders` добавляет N чистых заказов с номерами от 100 (каждый со своим клиентом и одной записью
`StatesMap`) — для проверки объёма.

**`Catalog` (V32).** Не указанное: текст пусто; деньги «0»; `WhatRemont` «ТЕЛЕФОН», `brand` «SAMSUNG», `model` «A52»,
`polomka` «Разбит экран», `master` «ПЕТРОВ»; `Barcode` — корректный UPC-A, уникальный для строки; в версиях до `V30`
поля `surname`/`phone` заполнены данными клиента (у заказа 8 — «иванов иван » с тем же телефоном, что у заказа 1;
у заказа 9 — пусто).

| id | Data_priema | Data_vidachi | Data_predoplaty | Status_remonta | Прочее |
|---|---|---|---|---|---|
| 1 | `2014-11-02` | `2014-11-05` | | `Выдан` | ClientId 1; итог `1500`; Garanty `30 дней`; работы `Замена дисплея`; Barcode `036000291452` |
| 2 | `05-03-2015 14:20` | | `05.03.2015` | `Ждет ЗИП` | ClientId 1; оценка `1 500`; предоплата `500`; wait_zakaz `Заказать`; Garanty `3 мес`; kommentarij `Позвонить завтра\r\n###RA123456789RU посылка`; Barcode `123456789012` |
| 3 | `06-03-2015 09:05` | `07-03-2015 18:00` | | `Куплен` | ClientId 2; Image_key `1`; итог `0`; Barcode `123456789012` |
| 4 | `вчера` | | | `Готов` | ClientId 2; оценка `abc`; Image_key `True`; Barcode пусто |
| 5 | `31-02-2015 10:00` | | | `Отдан сервисному центру` | ClientId 3; Barcode `12345` |
| 6 | `12.03.2015 15:04:05` | `13/03/2015` | | `Выдан` | ClientId 3; итог `15000000`; AdressSC `ВОКЗАЛЬНАЯ` |
| 7 | `14-03-2015 10-30` | | | `Выдан` | ClientId 4 |
| 8 | `15-03-2015 11:00` | | | пусто | ClientId 5; Deleted `1` |
| 9 | `16-03-2015 12:00` | | | `Диагностика` | ClientId пусто; surname и phone пусто |
| 10 | `17-03-2015 12:00` | | | `Диагностика` | ClientId `77`; surname `ПЕТРОВ`; phone `89110000000` |
| 12 | `03/20/2015 9:15:00 PM` | | | `Принят в работу` | ClientId 6; wait_zakaz `что-то`; AdressSC `ЛЕНИНА, 1` |

**`ClientsMap` (V30+):**

| id | FIO | Phone | Primechanie | Blist | date | aboutUs |
|---|---|---|---|---|---|---|
| 1 | `ИВАНОВ ИВАН` | `89211234567,89117654321` | | `0` | `2014-11-02` | `Интернет` |
| 2 | `СИДОРОВА АННА` | `8921123456789117654321` | | `1` | `05-03-2015 14:20` | `интернет` |
| 3 | `КИМ ОЛЕГ` | `89211234567жена` | `постоянный` | `-1` | пусто | |
| 4 | `ОРЛОВ` | `-` | | `True` | `07-03-2015 10:00` | |
| 5 | `ИВАНОВ ИВАН` | `89211234567,89117654321` | | `x` | `08-03-2015 10:00` | |
| 6 | `ЗАЙЦЕВ` | `89211234567/89117654321(раб)` | | `0` | `09-03-2015 10:00` | |
| 7 | `ГРОМОВ` | `нет телефона` | | `0` | `10-03-2015 10:00` | |
| 8 | `БЕЛОВ` | `89211234567123456` | | `0` | `11-03-2015 10:00` | |
| 9 | `ЧЕРНОВ` | `1234` | | `0` | `12-03-2015 10:00` | |

**`StatesMap` (V27+)** — `(id, clientId, State, date)`: `(1, 2, "05-03-2015 14-20", "Установлен статус\r\nДиагностика")`,
`(2, 2, "Ждет ЗИП", "06-03-2015 10:00")`, `(3, 2, "06-03-2015 10-05", "Установлена галочка\r\nТребует заказа")`,
`(4, 3, "06-03-2015 09:10", "Назначен мастер \r\nПЕТРОВ")`, `(5, 4, "07-03-2015 11-00", "Установлен статус\r\nГотов")`,
`(6, 999, "07-03-2015 12-00", "Установлен статус\r\nВыдан")`, `(7, 3, "мусор", "Сообщение отправлено\r\nВаш заказ готов")`,
`(8, 3, "07-03-2015 18:00", "Снята галочка\r\nНужно позвонить")`.

**`HistoryBD` (V31+)** — `(id, WHO, WHAT, FULLWHAT, DATA, IDINCATALOG)`: `(1, "ADMIN", "ДОБАВЛЕНИЕ НОВОЙ ЗАПИСИ", "", "02-11-2014 10:00", "1")`,
`(2, "ИРИНА АДМИНЫ", "СОХРАНЕНИЕ", "Бренд: SAMSUNG → APPLE", "05-03-2015 15:00", "2")`, `(3, "ADMIN", "УДАЛЕНИЕ", "", "16-03-2015 12:00", "8")`,
`(4, "ADMIN", "ПЕЧАТЬ АКТА ВЫДАЧИ", "", "31-13-2015 10:00", "3")`, `(5, "ADMIN", "СОХРАНЕНИЕ", "", "01-04-2015 10:00", "555")`.

**`GroupDostup` и `Users`.** V32: группы `(1, "Админы", все флаги "1")`, `(2, "Мастера", saveZapis "True", clients "1",
остальные "0" или "False")`, `(3, "Администратор", все "0")`; пользователи `(1, "Админы", "Ирина", "1", sha1("1234"))`,
`(2, "Мастера", "ирина", "2", sha1("пароль"))`, `(3, "Мастера", "Пётр", "9", SHA1("qwerty") в верхнем регистре)`,
`(4, "Мастера", "Олег", "2", "zzz")`. Здесь sha1 — `legacy_sha1_hex`. V31: группа `(1, "Мастера", saveZapis "1", остальные "0")`,
пользователь `(1, "Мастера", "Мастер", "1", sha1("1111"))`.

**`Stock` (V29+)**: `(1, "Дисплей", "Запчасти", "Дисплеи", "Чёрный", "SAMSUNG", "A52", CountOf "3", Price "2500",
Napominanie "1", Photo "settings\Stock\Photos\abc123.jpg", Photo2 "settings\Stock\Photos\missing.jpg")`,
`(2, "Аккумулятор", CountOf "два", остальное пусто)`. **`StockMap` (V30+)**: `(1, clientId "2", ZIPId "1", count "1", price "2500")`.

**Файлы** (Windows-1251, если не сказано иначе; во всех версиях, кроме оговорённых):
- `Config.ini`:
  ```
  [PROGRAMM_SETTINGS]
  valuta=руб.
  Mpersent=40
  daysDiagnostik=-5
  NessesaryCHB=Unchecked
  colorDiagnostik=-16711936
  colorCheckBox=Checked
  [CHECKBOX]
  garantyDefault=Checked
  EveryDayBackup=Checked
  [SMSSEND]
  Token=fixture-sms-token-7f3a
  [ACTIVATION]
  WD-WCC123=deadbeef
  [WD-WCC123]
  AdressSCDefault=1
  MasterDefault=ПЕТРОВ
  BlistColor=-65536
  backupPath=D:\Backup
  surname=150
  [S2Z9]
  MasterDefault=СИДОРОВ
  [ACTS]
  BarcodeH=40
  BarcodeW=120
  ```
- `settings/AdresSC.txt`: `Мира, 5\r\nЛенина, 1\r\n`; `settings/brands.txt` — UTF-8 с BOM: `Samsung\nApple\nsamsung\n`;
  `settings/masters.txt`: `ПЕТРОВ\r\nСИДОРОВ\r\nЭтот текст можно\r\nпоменять в файле\r\nsettings/masters.txt\r\n`;
  `settings/ustrojstvo.txt` — ровно стартовый файл MyWork 2 (`Ноутбук`, `Телефон`, `Патифон` и три строки-подсказки);
  `settings/Garanty.txt` — UTF-8 без BOM: `30 дней\n3 месяца\nБез гарантии\n`; файла `aboutUs.txt` нет.
- `settings/Akts/`: `FirmName.txt` «СЕРВИС-ПЛЮС», `Phone.txt` «8 (800) 000-00-00», `DannieOFirme.txt`
  «Режим работы: 10–19<br>\r\nг. Город, ул. Примерная, 1», `URDannie.txt` «ИП Примеров П.П., ИНН 000000000000»,
  `DogovorTextPriem.txt` «Строка договора 1<br>\r\nСтрока договора 2», `DogovorTextVidacha.txt` «Условия выдачи».
- `settings/Stock/brands.txt` «SAMSUNG»; `settings/Stock/Photos/abc123.jpg` (байты `stock-photo`, V29+); `Shablon_Priem.htm` «<html>Акт</html>».
- `ClientFiles/` (V30+): `1/1/photo.jpg` (`photo-1`), `1/1/docs/act.pdf` (`%PDF-fixture`), `2/3/video.mp4` (`video-3`),
  `2/404/lost.jpg` (`lost`), `2/note.txt` (`note`).

---

### Task 1: Домен — NFC в поиске и срок гарантии

**Files:**
- Modify: `Cargo.toml` (`unicode-normalization = "0.1"` в `[workspace.dependencies]`), `crates/mw-domain/Cargo.toml`, `crates/mw-domain/src/search.rs`, `crates/mw-domain/src/lib.rs`
- Create: `crates/mw-domain/src/warranty.rs`

**Interfaces:**
- Produces:
  - `search::normalize_search` первым шагом приводит строку к NFC (`UnicodeNormalization::nfc`). `search_tokens` и `index_text` получают это поведение через неё.
  - `warranty::WarrantyUnit` — `code_enum!` с `Days => "days"`, `Months => "months"`.
  - `warranty::WarrantyTerm { pub unit: WarrantyUnit, pub value: u16 }`: `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema`; `WarrantyTerm::days(u16)`, `WarrantyTerm::months(u16)`. `value = 0` — «без гарантии».
  - `warranty::MAX_WARRANTY_DAYS: u16 = 3650`, `warranty::MAX_WARRANTY_MONTHS: u16 = 120`.
  - `warranty::parse_warranty_term(text: &str) -> Option<WarrantyTerm>`. Правила, и только они:
    - текст проходит `normalize_search`, затем отбрасываются точки в конце слов и начальное «гарантия» с необязательными `:` или `-`;
    - пусто → `None`;
    - «без гарантии», «без гар», «нет», «нет гарантии», «-», «0» → `days(0)`;
    - одно число N → `days(N)`;
    - N и единица (пробел между ними необязателен):
      - «д», «дн», «день», «дня», «дней», «сут», «суток», «сутки» → `days(N)`;
      - «нед», «неделя», «недели», «недель» → `days(7N)`;
      - «мес», «месяц», «месяца», «месяцев» → `months(N)`;
      - «г», «год», «года», «лет» → `months(12N)`;
    - всё остальное → `None`: несколько сроков, лишние слова, число прописью, срок больше `MAX_WARRANTY_DAYS` дней или `MAX_WARRANTY_MONTHS` месяцев.

- [ ] **Step 1: Write the failing tests**

```rust
// search.rs
#[test]
fn normalize_search_composes_unicode_first() {
    assert_eq!(normalize_search("Пе\u{0308}тр"), "петр");               // «ё» двумя кодами
    assert_eq!(normalize_search("И\u{0306}ошкар-Ола"), "йошкар-ола");
    assert_eq!(search_tokens("ПЕ\u{0308}ТР"), vec!["петр"]);
    assert_eq!(index_text(&["Пе\u{0308}тр"]), "петр");
}

// warranty.rs
#[test]
fn parses_simple_warranty_texts() {
    let t = parse_warranty_term;
    assert_eq!(t("30 дней"), Some(WarrantyTerm::days(30)));
    assert_eq!(t("3 дня"), Some(WarrantyTerm::days(3)));
    assert_eq!(t("90"), Some(WarrantyTerm::days(90)));
    assert_eq!(t("2 недели"), Some(WarrantyTerm::days(14)));
    assert_eq!(t("Гарантия: 14 дн."), Some(WarrantyTerm::days(14)));
    assert_eq!(t("3 мес"), Some(WarrantyTerm::months(3)));
    assert_eq!(t("6 мес."), Some(WarrantyTerm::months(6)));
    assert_eq!(t("4 месяцев"), Some(WarrantyTerm::months(4)));
    assert_eq!(t("1 год"), Some(WarrantyTerm::months(12)));
    assert_eq!(t("2 года"), Some(WarrantyTerm::months(24)));
    for none in ["Без гарантии", "без гарантии", "0", "нет", "-"] {
        assert_eq!(t(none), Some(WarrantyTerm::days(0)), "{none}");
    }
}

#[test]
fn composite_or_unknown_warranty_has_no_term() {
    for s in ["", "  ", "До установки.", "30 дней - чистка, 6 месяцев блок питания",
              "ОС-30 дней; ssd-3 года", "99999 дней", "121 месяц", "три месяца"] {
        assert_eq!(parse_warranty_term(s), None, "{s}");
    }
}

#[test]
fn warranty_term_json_uses_codes() {
    assert_eq!(serde_json::to_string(&WarrantyTerm::months(3)).unwrap(), r#"{"unit":"months","value":3}"#);
}
```

- [ ] **Step 2: Run tests to verify they fail** — `cargo test -p mw-domain` → ошибка компиляции (`warranty` нет), а `normalize_search_composes_unicode_first` после добавления модуля падает на сравнении.
- [ ] **Step 3: Implement** — `warranty.rs` по Interfaces; NFC в `normalize_search`.
- [ ] **Step 4: Run tests to verify they pass** — `cargo test --workspace` → всё зелёное (старые тесты поиска не меняются).
- [ ] **Step 5: Commit** — `feat(domain): NFC in search normalization, warranty term parsing`.

---

### Task 2: База — телефоны, гарантия текстом, трек-номера, следы импорта, справочник гарантий, копия SQLite

**Files:**
- Create: `crates/mw-db/src/migration/m20261112_000001_legacy_import.rs`, `crates/mw-db/src/migration/m20261112_000002_warranty_dictionary.rs`, `crates/mw-db/src/entity/{client_phone.rs, order_tracking.rs, legacy_import.rs, legacy_row.rs}`, `crates/mw-db/src/backup.rs`
- Modify: `crates/mw-db/src/{lib.rs, testing.rs, migration/mod.rs, entity/mod.rs, entity/order.rs}`, `crates/mw-db/Cargo.toml` (dev: `tempfile`), `crates/mw-db/tests/schema.rs`, `crates/mw-domain/src/dictionary.rs`, `crates/mw-core/src/orders/{create.rs, types.rs}` (только новые поля модели пустыми значениями, смысл — задача 4), `crates/mw-api/openapi.json`, `crates/mw-api/bindings/` (новое значение `DictionaryKind`)

**Interfaces:**
- Consumes: `phone_key` из `mw-domain` — только для сверки в тесте; миграция копирует уже посчитанный `clients.phone_key`.
- Produces:
  - Таблицы и колонки — по разделу «Схема», две первые миграции.
  - `order::Model` + `warranty_text: String`, `warranty_months: Option<i32>`. Сущности `client_phone`, `order_tracking`, `legacy_import`, `legacy_row` с колонками из схемы.
  - `DictionaryKind::Warranty` (код `warranty`). В `DEFAULT_DICTIONARY` последним видом добавляется `(Warranty, ["Без гарантии", "14 дней", "30 дней", "3 месяца", "6 месяцев", "1 год"])`.
  - `mw_db::backup::vacuum_into(db: &DatabaseConnection, path: &Path) -> Result<(), DbErr>` — только SQLite (`VACUUM INTO ?` со связанным параметром). Создаёт папку назначения. Если файл уже есть, ошибка, файл не трогается. На другой СУБД — `DbErr::Custom("копия файлом возможна только для SQLite")`.
  - `mw_db::testing::all_backends_up_to(steps: u32) -> Vec<(&'static str, DatabaseConnection)>` — то же, что `all_backends()`, но применены только первые `steps` миграций. `sqlite_memory()` и `postgres_isolated()` по-прежнему применяют все.

- [ ] **Step 1: Write the failing tests** (`crates/mw-db/tests/schema.rs`; помощники `insert_client_row_with_phone(&db, phone) -> i64` и `insert_order_row_with_warranty(&db, client_id, Option<i32>) -> i64` заполняют только колонки первой миграции)

```rust
#[tokio::test]
async fn legacy_import_migration_backfills_phones_and_warranty_text() {
    for (label, db) in mw_db::testing::all_backends_up_to(1).await {
        let client = insert_client_row_with_phone(&db, "79211234567").await;
        let _no_phone = insert_client_row_with_phone(&db, "").await;
        let thirty = insert_order_row_with_warranty(&db, client, Some(30)).await;
        let none = insert_order_row_with_warranty(&db, client, Some(0)).await;
        let unknown = insert_order_row_with_warranty(&db, client, None).await;
        mw_db::migrate(&db).await.unwrap();
        let phones = client_phone::Entity::find().all(&db).await.unwrap();
        assert_eq!(phones.iter().map(|p| (p.client_id, p.position, p.phone.as_str(), p.phone_key.as_str(), p.note.as_str()))
                         .collect::<Vec<_>>(),
                   vec![(client, 0, "79211234567", "9211234567", "")], "{label}");
        let text = |id: i64| { let db = db.clone(); async move {
            order::Entity::find_by_id(id).one(&db).await.unwrap().unwrap().warranty_text } };
        assert_eq!(text(thirty).await, "30 дн.", "{label}");
        assert_eq!(text(none).await, "Без гарантии", "{label}");
        assert_eq!(text(unknown).await, "", "{label}");
    }
}

#[tokio::test]
async fn legacy_rows_are_unique_per_source_row() {
    for (label, db) in mw_db::testing::all_backends().await {
        let import = insert_legacy_import(&db).await;                      // помощник: строка legacy_imports
        insert_legacy_row(&db, import, "Catalog", "12").await.unwrap();
        assert!(insert_legacy_row(&db, import, "Catalog", "12").await.is_err(), "{label}");
        assert!(insert_legacy_row(&db, import, "ClientsMap", "12").await.is_ok(), "{label}");
    }
}

#[tokio::test]
async fn vacuum_into_writes_a_consistent_copy_once() {
    let dir = tempfile::tempdir().unwrap();
    let db = mw_db::connect(&mw_db::sqlite_url(&dir.path().join("live.db"))).await.unwrap();
    mw_db::migrate(&db).await.unwrap();
    let copy = dir.path().join("backups").join("copy.db");
    mw_db::backup::vacuum_into(&db, &copy).await.unwrap();
    let restored = mw_db::connect(&mw_db::sqlite_url(&copy)).await.unwrap();
    assert_eq!(status::Entity::find().count(&restored).await.unwrap(), 8);
    assert!(mw_db::backup::vacuum_into(&db, &copy).await.is_err());
}
```

Существующий `seed_data_equals_current_domain_constants` без изменений проверяет новый справочник: он сравнивает
базу с `DEFAULT_DICTIONARY`.

- [ ] **Step 2: Run tests to verify they fail** — `cargo test -p mw-db` → ошибка компиляции (нет сущностей, `backup`, `all_backends_up_to`).
- [ ] **Step 3: Implement** — две миграции, сущности, `backup.rs`, `testing.rs`, `DictionaryKind::Warranty`; в `mw-core` новые поля модели заполняются пусто (`String::new()`, `None`).
- [ ] **Step 4: Обновить контракт API** — `MW_UPDATE_CONTRACT=1 cargo test -p mw-api --test contract -- --test-threads=1`; в диффе `bindings/DictionaryKind.ts` и `openapi.json` — только новое значение `warranty`.
- [ ] **Step 5: Run tests to verify they pass** — `cargo test --workspace` с `MW_TEST_PG_URL` и без → всё зелёное.
- [ ] **Step 6: Commit** — `feat(db): client phones, warranty text, tracking numbers, legacy import traces, warranty dictionary, SQLite file copy`.

---

### Task 3: Ядро — список телефонов клиента

**Files:**
- Modify: `crates/mw-core/src/{clients.rs, lib.rs, orders/create.rs}`, `crates/mw-core/tests/{common/mod.rs, clients.rs, orders_create.rs, orders_list.rs, api_types.rs}`, `crates/mw-api/tests/{common/mod.rs, clients.rs, orders.rs, logging.rs}`, `crates/mw-api/openapi.json`, `crates/mw-api/bindings/`

**Interfaces:**
- Consumes: `client_phone` (задача 2), `normalize_phone`, `phone_key`, `index_text`.
- Produces:
  - `pub struct ClientPhone { pub phone: String, pub note: String }` — `Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema`, `deny_unknown_fields`. Одно описание для ввода и вывода.
  - `pub const MAX_CLIENT_PHONES: usize = 10`.
  - `Client { id, full_name, phones: Vec<ClientPhone>, address, note, source, blacklisted, created_at, updated_at, version }` — поле `phone` убрано.
  - `NewClient { full_name, phones: Vec<ClientPhone>, address, note, source, blacklisted }`; `ClientPatch.phones: Option<Vec<ClientPhone>>` (`patch::present`, заменяет список целиком).
  - Нормализация при записи:
    - у номера остаются только цифры, пометка обрезается;
    - запись без цифр и без пометки отбрасывается;
    - запись без цифр, но с пометкой → `Validation[phones: invalid]`;
    - два номера с одинаковым `phone_key` → `phones: duplicate`;
    - больше `MAX_CLIENT_PHONES` → `phones: out_of_range`;
    - пустые ФИО и список → `full_name: required`.
  - Порядок ошибок `create_client` и `update_client`: `full_name`, `phones`.
  - Хранение: `client_phones` с `position` 0..n (при правке строки удаляются и пишутся заново); `clients.phone` и `clients.phone_key` — первый номер или пусто; `search_text` = `index_text([full_name, все номера])`.
  - `find_clients_by_phone` — клиенты, у которых `phone_key` любого номера равен ключу запроса. Сортировка по `search_text`, затем `id`; удалённые не выводятся.
  - Списки и `get_client` читают номера одним запросом на страницу.
  - Журнал `client.updated` показывает новый список под ключом `phones`.

- [ ] **Step 1: Write the failing tests** (`tests/clients.rs`; в `common` — `phones(&[&str]) -> Vec<ClientPhone>` с пустыми пометками; старые тесты переходят с `phone: "…"` на `phones: phones(&["…"])`)

```rust
// phones_list_keeps_order_and_any_number_finds_the_client
let c = t.core.create_client(&admin(), NewClient { full_name: "ИВАНОВ ИВАН".into(),
    phones: vec![ClientPhone { phone: "+7 (921) 123-45-67".into(), note: String::new() },
                 ClientPhone { phone: "8 911 765 43 21".into(), note: " жена ".into() }], ..Default::default() }).await?;
assert_eq!(c.phones, vec![ClientPhone { phone: "79211234567".into(), note: String::new() },
                          ClientPhone { phone: "89117654321".into(), note: "жена".into() }]);
assert_eq!(ids(t.core.find_clients_by_phone(&admin(), "+7 911 765-43-21").await?), vec![c.id]);
assert_eq!(ids(t.core.find_clients(&admin(), "911 765", 50, 0).await?.items), vec![c.id]);
let o = t.core.create_order(&admin(), NewOrder { client: ClientRef::Existing(c.id), ..sample_new_order() }).await?;
let page = t.core.list_orders(&admin(), &OrderFilter::default(), OrderSort::default(), 50, 0).await?;
assert_eq!(page.items.iter().find(|i| i.id == o.id).unwrap().client_phone, "79211234567");

// replacing_phones_drops_old_numbers_from_lookup
let u = t.core.update_client(&admin(), c.id, c.version, ClientPatch { phones: Some(phones(&["8 911 765 43 21"])), ..Default::default() }).await?;
assert_eq!(u.phones.len(), 1);
assert!(t.core.find_clients_by_phone(&admin(), "9211234567").await?.is_empty());

// phone_list_validation
let bad = |p: Vec<ClientPhone>| NewClient { full_name: "А".into(), phones: p, ..Default::default() };
assert_validation(t.core.create_client(&admin(), bad(vec![ClientPhone { phone: "нет".into(), note: "x".into() }])).await, &[("phones", "invalid")]);
assert_validation(t.core.create_client(&admin(), bad(phones(&["8 921 123-45-67", "+7 921 1234567"]))).await, &[("phones", "duplicate")]);
assert_validation(t.core.create_client(&admin(), bad(phones(&["1","2","3","4","5","6","7","8","9","10","11"]))).await, &[("phones", "out_of_range")]);
assert_validation(t.core.create_client(&admin(), NewClient { phones: vec![ClientPhone { phone: " ".into(), note: " ".into() }], ..Default::default() }).await,
                  &[("full_name", "required")]);
```

`crates/mw-api/tests/clients.rs`: `POST /clients` с
`"phones": [{"phone": "+7 921 123-45-67", "note": ""}, {"phone": "8 911 765-43-21", "note": "жена"}]` → 201,
`body["phones"][1] == {"phone": "89117654321", "note": "жена"}`; `GET /clients/by-phone?phone=89117654321` → массив из
этого клиента; тело с полем `"phone"` → 400.

- [ ] **Step 2: Run tests to verify they fail** — `cargo test -p mw-core --test clients` → ошибка компиляции (`ClientPhone`, `phones`).
- [ ] **Step 3: Implement** — `clients.rs` по Interfaces; `insert_client` пишет и номера.
- [ ] **Step 4: Обновить контракт API** (команда из Global Constraints).
- [ ] **Step 5: Run tests to verify they pass** — `cargo test --workspace` с `MW_TEST_PG_URL` и без → всё зелёное.
- [ ] **Step 6: Commit** — `feat(core,api): list of client phones, lookup by any number`.

---

### Task 4: Ядро — гарантия: текст и срок

**Files:**
- Modify: `crates/mw-core/src/orders/{types.rs, create.rs, update.rs}`, `crates/mw-core/tests/{common/mod.rs, orders_create.rs, orders_update.rs, api_types.rs}`, `crates/mw-api/tests/{orders.rs, common/mod.rs}`, `crates/mw-api/openapi.json`, `crates/mw-api/bindings/`

**Interfaces:**
- Consumes: `parse_warranty_term`, `WarrantyTerm` (задача 1); колонки `warranty_text`, `warranty_days`, `warranty_months` (задача 2).
- Produces:
  - `NewOrder.warranty: String` вместо `warranty_days`; `OrderPatch.warranty: Option<String>` (`patch::present`) вместо `warranty_days`.
  - `Order.warranty: String` и `Order.warranty_term: Option<WarrantyTerm>` вместо `warranty_days`.
  - Запись: `warranty_text` — текст без пробелов по краям. Срок `parse_warranty_term(текст)`: `days(N)` → (`warranty_days` = N, `warranty_months` = NULL), `months(N)` → (NULL, N), `None` → (NULL, NULL). Чтение собирает `warranty_term` обратно из колонок.
  - Константа `MAX_WARRANTY_DAYS` и проверка `warranty_days: out_of_range` в ядре удаляются: границы держит `parse_warranty_term`.
  - Журнал `order.updated`: поле `warranty` с подписью «гарантия», `changes.warranty = [было, стало]` текстом.

- [ ] **Step 1: Write the failing tests** (`tests/orders_update.rs`; `sample_new_order()` получает `warranty: "30 дней"`)

```rust
// warranty_text_is_kept_and_term_is_derived
let o = t.core.create_order(&admin(), NewOrder { warranty: " 3 мес ".into(), ..sample_new_order() }).await?;
assert_eq!((o.warranty.as_str(), o.warranty_term), ("3 мес", Some(WarrantyTerm::months(3))));
let o = t.core.update_order(&admin(), o.id, o.version, OrderPatch { warranty: Some("Без гарантии".into()), ..Default::default() }).await?;
assert_eq!(o.warranty_term, Some(WarrantyTerm::days(0)));
assert_eq!(last_audit(&t.core, o.id).await.details.unwrap()["changes"]["warranty"], json!(["3 мес", "Без гарантии"]));
let o = t.core.update_order(&admin(), o.id, o.version,
    OrderPatch { warranty: Some("30 дней — чистка, 6 мес блок питания".into()), ..Default::default() }).await?;
assert_eq!(o.warranty_term, None);
assert_eq!(t.core.get_order(&admin(), o.id).await?.warranty, "30 дней — чистка, 6 мес блок питания");
```

`crates/mw-api/tests/orders.rs`: `POST /orders` с `"warranty": "30 дней"` →
`body["warranty_term"] == {"unit": "days", "value": 30}`; `PATCH` с `{"warranty_days": 30}` → 400.

- [ ] **Step 2: Run tests to verify they fail** — `cargo test -p mw-core --test orders_update` → ошибка компиляции.
- [ ] **Step 3: Implement** — по Interfaces.
- [ ] **Step 4: Обновить контракт API.**
- [ ] **Step 5: Run tests to verify they pass** — `cargo test --workspace` с `MW_TEST_PG_URL` и без.
- [ ] **Step 6: Commit** — `feat(core,api): warranty as printed text with derived term`.

---

### Task 5: Ядро — настройки: гарантия по умолчанию текстом, точка и мастер по умолчанию, реквизиты и тексты актов

**Files:**
- Create: `crates/mw-db/src/migration/m20261112_000003_default_warranty_text.rs`
- Modify: `crates/mw-db/src/migration/mod.rs`, `crates/mw-db/tests/schema.rs`, `crates/mw-core/src/settings.rs`, `crates/mw-core/tests/settings.rs`, `crates/mw-api/tests/catalogs.rs`, `crates/mw-api/openapi.json`, `crates/mw-api/bindings/`

**Interfaces:**
- Produces:
  - `AppSettings` (поля по порядку, в скобках — умолчание):
    - `currency_symbol` («₽»), `master_percent` (50), `diagnostics_overdue_days` (4);
    - `default_warranty: String` («30 дней») — заменяет `default_warranty_days`;
    - `intake_required_fields` (true);
    - `default_service_point_id: Option<i64>` (`None`), `default_master: String` («»);
    - `firm_name`, `firm_phone`, `firm_details`, `firm_legal_details`, `intake_terms`, `issue_terms`: `String` («»);
    - `timezone`.
  - `SettingsPatch` — те же поля в `Option` (`patch::present`), `default_service_point_id: Option<Option<i64>>` (`patch::nullable`).
  - Правила:
    - текстовые поля обрезаются по краям, переводы строк внутри сохраняются;
    - `default_service_point_id` должен указывать на существующую неархивную точку, иначе `default_service_point_id: invalid`;
    - ошибки идут в порядке полей `AppSettings`.
  - Подписи в журнале: «гарантия по умолчанию», «точка по умолчанию», «мастер по умолчанию», «название фирмы», «телефон фирмы», «данные фирмы», «реквизиты», «условия приёма», «условия выдачи».
  - Миграция `m20261112_000003_default_warranty_text` — по разделу «Схема».

- [ ] **Step 1: Write the failing tests**

```rust
// mw-core tests/settings.rs: new_settings_have_defaults_and_validate_service_point
let s = t.core.settings();
assert_eq!((s.default_warranty.as_str(), s.default_service_point_id, s.default_master.as_str(), s.firm_name.as_str()),
           ("30 дней", None, "", ""));
let p = t.core.create_service_point(&admin(), "Мира, 5").await?;
let s = t.core.update_settings(&admin(), SettingsPatch { default_service_point_id: Some(Some(p.id)),
    intake_terms: Some("Строка 1\nСтрока 2\n".into()), ..Default::default() }).await?;
assert_eq!((s.default_service_point_id, s.intake_terms.as_str()), (Some(p.id), "Строка 1\nСтрока 2"));
assert_validation(t.core.update_settings(&admin(), SettingsPatch { default_service_point_id: Some(Some(9_999)), ..Default::default() }).await,
                  &[("default_service_point_id", "invalid")]);

// mw-db tests/schema.rs: default_warranty_days_setting_becomes_text
for (label, db) in mw_db::testing::all_backends_up_to(2).await {
    put_setting_row(&db, "default_warranty_days", "45").await;           // помощник: строка settings
    mw_db::migrate(&db).await.unwrap();
    assert_eq!(setting_row(&db, "default_warranty").await.as_deref(), Some("\"45 дн.\""), "{label}");
    assert_eq!(setting_row(&db, "default_warranty_days").await, None, "{label}");
}
```

`crates/mw-api/tests/catalogs.rs`: `PATCH /settings` с `{"default_warranty": "6 месяцев"}` → 200 и это значение;
`{"default_warranty_days": 30}` → 400.

- [ ] **Step 2: Run tests to verify they fail** — `cargo test -p mw-core --test settings` → ошибка компиляции.
- [ ] **Step 3: Implement** — `settings.rs` и миграция.
- [ ] **Step 4: Обновить контракт API.**
- [ ] **Step 5: Run tests to verify they pass** — `cargo test --workspace` с `MW_TEST_PG_URL` и без.
- [ ] **Step 6: Commit** — `feat(core,api): default warranty text, default service point and master, firm details and act texts in settings`.

---

### Task 6: Ядро — общий построитель `search_text` и переиндексация

**Files:**
- Create: `crates/mw-core/src/search_index.rs`, `crates/mw-core/tests/search_index.rs`
- Modify: `crates/mw-core/src/{clients.rs, core.rs, lib.rs, orders/types.rs, orders/create.rs, orders/update.rs}`, `crates/mw-core/tests/common/mod.rs`

**Interfaces:**
- Produces:
  - `pub const SEARCH_INDEX_VERSION: i64 = 2` (1 — планы 1–2: без NFC и с одним телефоном). Правило для `AGENTS.md`: любая правка построителей или `normalize_search` увеличивает версию.
  - `pub(crate) fn client_search_text(full_name: &str, phones: &[String]) -> String` и `pub(crate) fn order_search_text(m: &order::Model) -> String` (переносится из `orders/types.rs` без изменений набора полей) — единственные построители. Ими пользуются `clients.rs`, `orders/*` и импорт (задача 7).
  - `pub struct ReindexStats { pub clients: u64, pub orders: u64 }` — сколько строк пришлось переписать.
  - `pub(crate) async fn reindex_all(conn: &impl ConnectionTrait) -> Result<ReindexStats, CoreError>` — пересчитывает всех клиентов (вместе с номерами из `client_phones`) и все заказы, пишет только отличающиеся строки.
  - `Core::open` после `ensure_timezone`: если в `settings` нет ключа `search_index_version` или он не равен `SEARCH_INDEX_VERSION`, то в одной транзакции записи `reindex_all` и запись версии. Журнал не пишется. Ключ не входит в `AppSettings`, загрузчик настроек его игнорирует.
  - `Core::reindex_search(&self, actor: &Actor) -> Result<ReindexStats, CoreError>` — право `settings_manage`, журнал `search.reindexed` со сводкой «Поиск переиндексирован: клиентов {N}, заказов {M}».

- [ ] **Step 1: Write the failing tests** (`tests/search_index.rs`; помощники в `common`: `set_search_text_directly(&Core, client_id, &str)`, `set_setting_directly(&Core, key, Value)`, `setting_value(&Core, key) -> Value`)

```rust
// open_reindexes_when_version_differs
for t in cores().await {
    let c = t.core.create_client(&admin(), NewClient { full_name: "Пётр".into(), phones: phones(&["8 911 765 43 21"]), ..Default::default() }).await?;
    set_search_text_directly(&t.core, c.id, "мусор").await;
    set_setting_directly(&t.core, "search_index_version", json!(1)).await;
    let reopened = Core::open(t.core.db().clone(), t.clock.clone(), "UTC").await?;
    assert_eq!(ids(reopened.find_clients(&admin(), "петр 911", 50, 0).await?.items), vec![c.id], "{}", t.label);
    assert_eq!(setting_value(&reopened, "search_index_version").await, json!(SEARCH_INDEX_VERSION));
}

// decomposed_input_is_found_both_ways
let c = t.core.create_client(&admin(), NewClient { full_name: "Се\u{0308}мин".into(), ..Default::default() }).await?;
assert_eq!(ids(t.core.find_clients(&admin(), "Сёмин", 50, 0).await?.items), vec![c.id]);
assert_eq!(ids(t.core.find_clients(&admin(), "семин", 50, 0).await?.items), vec![c.id]);

// manual_reindex_needs_settings_right_and_audits
assert!(matches!(t.core.reindex_search(&actor_with(&[])).await, Err(CoreError::Forbidden(Permission::SettingsManage))));
let stats = t.core.reindex_search(&admin()).await?;
assert_eq!((stats.clients, stats.orders), (0, 0));
assert_eq!(count_audit(&t.core, "search.reindexed").await, 1);
```

- [ ] **Step 2: Run tests to verify they fail** — `cargo test -p mw-core --test search_index` → ошибка компиляции.
- [ ] **Step 3: Implement** — `search_index.rs`, вызов из `Core::open`, переход `clients.rs` и `orders/*` на общие построители.
- [ ] **Step 4: Run tests to verify they pass** — `cargo test --workspace` с `MW_TEST_PG_URL` и без.
- [ ] **Step 5: Commit** — `feat(core): shared search_text builders, index version and reindexing`.

---

### Task 7: Ядро — запись импорта `Core::import_bundle`

**Files:**
- Create: `crates/mw-core/src/import/{mod.rs, write.rs}`, `crates/mw-core/tests/import.rs`
- Modify: `crates/mw-core/src/{lib.rs, core.rs}`, `crates/mw-core/Cargo.toml` (dev: `tempfile`), `crates/mw-core/tests/common/mod.rs`, `crates/mw-domain/src/validation.rs` (`message_ru`: `not_empty` → «База не пуста», `replace_unsupported` → «Замена возможна только для базы SQLite»)

**Interfaces:**
- Consumes: `NewStatus`, `NewRole`, `SettingsPatch`, `ClientPhone`, `MAX_CLIENT_PHONES`, `client_search_text`, `order_search_text`, `parse_warranty_term`, `normalize_search`, `mw_db::ids::set_next_id`, `mw_db::backup::vacuum_into`, сущности задачи 2.
- Produces (`mw_core::import`, всё реэкспортируется из корня крейта):
  ```rust
  pub enum DatabaseKind { Sqlite, Postgres }                        // Core::database_kind(&self) -> DatabaseKind
  pub enum ImportMode { IntoEmpty, Replace { backup_to: PathBuf } }
  pub struct ImportTarget { pub clients: u64, pub orders: u64, pub users: u64, pub audit_entries: u64 }  // is_empty()
  pub struct ImportBundle {
      pub source_path: String,
      pub statuses: Vec<NewStatus>,                     // в порядке sort_order
      pub service_points: Vec<String>,                  // в порядке sort_order
      pub dictionary: Vec<(DictionaryKind, Vec<String>)>,  // каждый вид ровно один раз
      pub roles: Vec<NewRole>,
      pub users: Vec<ImportedUser>,
      pub settings: ImportedSettings,
      pub clients: Vec<ImportedClient>,
      pub orders: Vec<ImportedOrder>,
      pub history: Vec<ImportedHistory>,                // в порядке записи (по времени)
      pub legacy_rows: Vec<ImportedLegacyRow>,
      pub next_order_id: i64,
      pub next_client_id: i64,
      pub report: serde_json::Value,                    // пишется в legacy_imports.report
  }
  pub struct ImportedUser { pub login: String, pub role: String, pub legacy_sha1: String }
  pub struct ImportedSettings { pub values: SettingsPatch, pub default_service_point: Option<String> }
      // values.timezone и values.default_service_point_id должны быть None
  pub struct ImportedClient { pub id: i64, pub full_name: String, pub phones: Vec<ClientPhone>, pub address: String,
      pub note: String, pub source: String, pub blacklisted: bool, pub created_at: NaiveDateTime }
  pub struct ImportedOrder { pub id: i64, pub client_id: i64, pub status: String, pub service_point: Option<String>,
      pub received_at: NaiveDateTime, pub issued_at: Option<NaiveDateTime>, pub prepaid_at: Option<NaiveDateTime>,
      pub device_type: String, pub brand: String, pub model: String, pub serial: String, pub color: String,
      pub completeness: String, pub device_condition: String, pub malfunction: String, pub work_done: String,
      pub internal_comment: String, pub master: String, pub barcode: String,
      pub estimate_from: Option<Money>, pub estimate_to: Option<Money>, pub final_price: Option<Money>,
      pub prepayment: Money, pub extra_costs: Money, pub discount: Money, pub warranty: String,
      pub need_call: bool, pub need_parts: bool, pub deleted_at: Option<NaiveDateTime>,
      pub tracking: Vec<(String, String)> }                // (номер, описание)
  pub struct ImportedHistory { pub at: NaiveDateTime, pub user_name: String, pub action: String,
      pub order_id: Option<i64>, pub client_id: Option<i64>, pub summary: String, pub details: Option<serde_json::Value> }
  pub struct ImportedLegacyRow { pub source: String, pub source_key: String, pub target: Option<LegacyTarget>, pub raw: serde_json::Value }
  pub enum LegacyTarget { Order(i64), Client(i64), Role(String), User(String) }   // роль и пользователь — по имени и логину
  pub struct ImportSummary { pub import_id: i64, pub statuses: u64, pub service_points: u64, pub dictionary_items: u64,
      pub roles: u64, pub users: u64, pub clients: u64, pub client_phones: u64, pub orders: u64, pub deleted_orders: u64,
      pub tracking_numbers: u64, pub history: u64, pub legacy_rows: u64, pub next_order_id: i64, pub next_client_id: i64 }
  ```
  - `Core::import_target_state(&self) -> Result<ImportTarget, CoreError>`.
  - `Core::import_bundle(&self, actor: &Actor, bundle: ImportBundle, mode: ImportMode) -> Result<ImportSummary, CoreError>`, по шагам:
    1. Права `settings_manage` и `users_manage`; при отказе — `Forbidden` первого недостающего.
    2. Проверка пакета; нарушение → `CoreError::Internal("импорт: …")`. Проверяется:
       - имена статусов, точек и ролей уникальны после `normalize_search`, есть статус вида `issued`;
       - каждый вид справочника ровно один раз;
       - логины уникальны по `login_key`, роли пользователей существуют;
       - если пользователи есть, хотя бы у одного роль с `users_manage`;
       - номера клиентов и заказов уникальны и больше 0, ссылки заказов на клиента, статус и точку существуют;
       - номеров у клиента не больше `MAX_CLIENT_PHONES`, без повторов по `phone_key`, номер без цифр не допускается;
       - цели `legacy_rows` существуют, `default_service_point` есть среди точек;
       - `next_order_id` больше наибольшего номера заказа, `next_client_id` — наибольшего номера клиента.
    3. `Replace` на PostgreSQL → `Validation[mode: replace_unsupported]`; на SQLite — `vacuum_into(backup_to)` до транзакции, ошибка копии прерывает импорт.
    4. Одна транзакция `begin_write()`:
       - `IntoEmpty` при непустой базе → `Validation[database: not_empty]`;
       - стереть `sessions`, `legacy_rows`, `legacy_imports`, `order_tracking`, `audit_log`, `orders`, `client_phones`, `clients`, `users`, `roles`, `statuses`, `dictionary_items`, `service_points` и `settings` (кроме `timezone`);
       - записать пакет пачками не больше 30 000 связанных параметров на запрос; роли и пользователи пишутся по одному, их id нужны для ссылок;
       - пользователи: `login_key = normalize_search(login)`, `display_name` = логин, `password_scheme = legacy_sha1`, `active`;
       - заказы: `search_text` общим построителем, срок гарантии из текста, `created_at` = `updated_at` = `received_at`, `version` = 1;
       - журнал: история пакета, затем запись `legacy.imported` от `actor` со сводкой «Импорт из MyWork 2: заказов {N}, клиентов {M}» и `details = {"import_id": …}`;
       - `legacy_imports` (время `now()`, путь, режим `into_empty` / `replace`, отчёт) и `legacy_rows` с найденными целями;
       - `set_next_id` для заказов и клиентов;
       - все поля `AppSettings`: стандартные значения, поверх них пакет, `timezone` прежний; затем `search_index_version`;
       - сверка числа строк каждой таблицы с пакетом: расхождение → `Internal`.
    5. После `commit` перечитать кэш настроек.

- [ ] **Step 1: Write the failing tests** (`tests/import.rs`; помощники в `common`: `sample_bundle()` — пакет ниже, `legacy_target(&Core, source, key) -> Option<(String, i64)>`, `service_point_id_by_name(&Core, &str) -> i64`)

`sample_bundle()`: восемь `DEFAULT_STATUSES` плюс «Куплен» (`Issued`, `gray`); точки «Мира, 5», «Ленина, 1»;
справочник `DEFAULT_DICTIONARY`, где `master` = [«ПЕТРОВ»]; роли «Администратор» (все права) и «Мастера»
(`orders_edit`); пользователи «Ирина» (Администратор, `legacy_sha1_hex("1234")`) и «Олег» (Мастера, `legacy_sha1_hex("qwerty")`);
настройки: валюта «руб.», точка по умолчанию «Ленина, 1», `firm_name` «СЕРВИС». Клиенты 7 («ИВАНОВ ИВАН»,
`89211234567` и `89117654321` с пометкой «жена») и 9 («КИМ», без номеров). Заказы:
- 5 — клиент 7, «Выдан», принят 2015-03-05 14:20, выдан 2015-03-07, итог 1500 ₽, гарантия «30 дней», штрихкод `123456789012`;
- 6 — клиент 7, «Куплен», удалён, штрихкод `123456789012`;
- 10 — клиент 9, «Диагностика», точка «Ленина, 1», оценка 1500–1500 ₽, трек `("RA123456789RU", "посылка")`.

История заказа 5: `legacy.status` (2015-03-05 14:20, «Установлен статус «Диагностика»») и `legacy.history`
(2015-03-07 10:00, автор «ADMIN», «ПЕЧАТЬ АКТА ВЫДАЧИ»). Следы: `Catalog/5` → Order(5), `ClientsMap/7` → Client(7),
`Users/1` → User(«Ирина»), `GroupDostup/1` → Role(«Мастера»), `Stock/1` без цели. Следующие номера: заказ 16, клиент 15.

```rust
// import_into_empty_database_keeps_numbers_and_continues_them
let s = t.core.import_bundle(&admin(), sample_bundle(), ImportMode::IntoEmpty).await?;
assert_eq!((s.orders, s.deleted_orders, s.clients, s.client_phones, s.history, s.legacy_rows), (3, 1, 2, 2, 2, 5));
let o = t.core.get_order(&admin(), 10).await?;
assert_eq!((o.client_id, status_name(&t.core, o.status_id).await.as_str()), (9, "Диагностика"));
assert_eq!((o.estimate_from, o.estimate_to), (Some(Money::from_rubles(1500)), Some(Money::from_rubles(1500))));
assert_eq!(t.core.get_order(&admin(), 5).await?.warranty_term, Some(WarrantyTerm::days(30)));
assert!(t.core.get_order(&admin(), 6).await?.deleted_at.is_some());
assert_eq!(ids(t.core.find_orders_by_barcode(&admin(), "123456789012").await?), vec![5]);
assert_eq!(ids(t.core.find_clients_by_phone(&admin(), "89117654321").await?), vec![7]);
assert_eq!(t.core.create_order(&admin(), sample_new_order()).await?.id, 16);
assert_eq!(t.core.create_client(&admin(), NewClient { full_name: "Новый".into(), ..Default::default() }).await?.id, 15);
let st = t.core.settings();
assert_eq!((st.currency_symbol.as_str(), st.timezone.as_str(), st.firm_name.as_str()), ("руб.", "UTC", "СЕРВИС"));
assert_eq!(st.default_service_point_id, Some(service_point_id_by_name(&t.core, "Ленина, 1").await));
let h = t.core.list_audit(&admin(), &AuditFilter { order_id: Some(5), ..Default::default() }, 50, 0).await?;
assert_eq!(h.items.iter().map(|e| e.action.as_str()).collect::<Vec<_>>(), ["legacy.history", "legacy.status"]);
assert_eq!(h.items[0].user_name, "ADMIN");
assert_eq!(count_audit(&t.core, "legacy.imported").await, 1);
assert_eq!(legacy_target(&t.core, "Catalog", "5").await, Some(("orders".to_owned(), 5)));
assert_eq!(legacy_target(&t.core, "Users", "1").await.map(|x| x.0), Some("users".to_owned()));
assert_eq!(legacy_target(&t.core, "Stock", "1").await, None);
assert_eq!(t.core.list_statuses(true).await?.len(), 9);

// legacy_passwords_work_after_import
t.core.import_bundle(&admin(), sample_bundle(), ImportMode::IntoEmpty).await?;
assert!(t.core.login("ирина", "1234", "x").await?.permissions.contains(Permission::UsersManage));
assert!(matches!(t.core.login("Олег", "1234", "x").await, Err(CoreError::InvalidCredentials)));
assert!(!t.core.is_open_mode().await?);

// import_refuses_non_empty_database_and_changes_nothing
t.core.create_client(&admin(), NewClient { full_name: "Уже есть".into(), ..Default::default() }).await?;
assert_validation(t.core.import_bundle(&admin(), sample_bundle(), ImportMode::IntoEmpty).await, &[("database", "not_empty")]);
assert_eq!(t.core.import_target_state().await?.clients, 1);
assert_eq!(t.core.list_statuses(true).await?.len(), 8);

// broken_bundle_rolls_back_everything
for broken in [
    { let mut b = sample_bundle(); b.orders[2].status = "Нет такого".into(); b },
    { let mut b = sample_bundle(); b.roles[0].permissions = PermissionSet::from_codes(["orders_create"]); b },
    { let mut b = sample_bundle(); b.next_order_id = 10; b },
    { let mut b = sample_bundle(); b.clients[0].phones.push(ClientPhone { phone: "+7 921 1234567".into(), note: String::new() }); b },
] {
    assert!(matches!(t.core.import_bundle(&admin(), broken, ImportMode::IntoEmpty).await, Err(CoreError::Internal(_))), "{}", t.label);
    assert!(t.core.import_target_state().await?.is_empty());
    assert_eq!(t.core.list_statuses(true).await?.len(), 8);
}

// import_needs_settings_and_users_rights
assert!(matches!(t.core.import_bundle(&actor_with(&[Permission::SettingsManage]), sample_bundle(), ImportMode::IntoEmpty).await,
                 Err(CoreError::Forbidden(Permission::UsersManage))));
```

```rust
// replace_mode_backs_up_sqlite_and_is_refused_on_postgres — отдельный тест на файловом SQLite и PostgreSQL,
// по образцу sqlite_file_concurrency
let dir = tempfile::tempdir()?;
let core = file_sqlite_core(&dir.path().join("mywork.db")).await;     // помощник в этом файле
core.import_bundle(&admin(), sample_bundle(), ImportMode::IntoEmpty).await?;
core.create_order(&admin(), sample_new_order()).await?;               // теперь 4 заказа
let backup = dir.path().join("backups").join("before.db");
core.import_bundle(&admin(), sample_bundle(), ImportMode::Replace { backup_to: backup.clone() }).await?;
assert_eq!(order_count_in_file(&backup).await, 4);
assert_eq!(core.import_target_state().await?.orders, 3);
assert!(core.import_bundle(&admin(), sample_bundle(), ImportMode::Replace { backup_to: backup }).await.is_err());
if let Some(pg) = mw_db::testing::postgres_isolated().await {
    let core = Core::open(pg, Arc::new(SystemClock), "UTC").await?;
    assert_validation(core.import_bundle(&admin(), sample_bundle(), ImportMode::Replace { backup_to: dir.path().join("x.db") }).await,
                      &[("mode", "replace_unsupported")]);
}
```

- [ ] **Step 2: Run tests to verify they fail** — `cargo test -p mw-core --test import` → ошибка компиляции.
- [ ] **Step 3: Implement** — `import/mod.rs` (типы), `import/write.rs` (`import_target_state`, `import_bundle`, проверка пакета, стирание, пачки, сверка), `Core::database_kind`.
- [ ] **Step 4: Run tests to verify they pass** — `cargo test --workspace` с `MW_TEST_PG_URL` и без.
- [ ] **Step 5: Commit** — `feat(core): legacy import writer in one transaction with explicit ids and checks`.

---

### Task 8: `mw-legacy` — каркас, чтение копии старой базы и папки, фикстуры

**Files:**
- Create: `crates/mw-legacy/Cargo.toml`, `crates/mw-legacy/src/{lib.rs, error.rs, source.rs, text.rs, folder.rs, fixture.rs, test_support.rs}`, `crates/mw-legacy/examples/make_fixture.rs`. Файл `test_support.rs` (`#[cfg(test)]`) — помощники модульных тестов: `fixture(version) -> TempDir`, `hash_tree(path)`. Задачи 10 и 11 дополняют его.
- Modify: `Cargo.toml` (`encoding_rs = "0.8"`, `tempfile = "3"` в `[workspace.dependencies]`)

**Interfaces:**
- `Cargo.toml` крейта:
  - зависимости — `mw-core`, `mw-domain`, `sqlx` (воркспейс, `sqlite`, `runtime-tokio`), `tokio` (`fs`, `rt`), `encoding_rs`, `serde`, `serde_json`, `chrono`, `sha2`, `hex`, `thiserror`, `tracing`, `tempfile`;
  - фича `fixtures = []`;
  - dev — `mw-legacy` с `fixtures`, `mw-core` с `testing`, `mw-db`, `tokio` (`macros`, `rt-multi-thread`);
  - `[[example]] name = "make_fixture"`, `required-features = ["fixtures"]`.
- Produces:
  - `pub enum LegacyError` (`thiserror`, сообщения по-русски):
    - `SourceNotFound(PathBuf)`, `NoDatabase(PathBuf)`, `NotMyWork2`;
    - `Io { context: String, source: std::io::Error }`, `Sqlite(String)`;
    - `Core(#[from] CoreError)`;
    - `TargetNotEmpty(ImportTarget)`, `AttachmentsNotEmpty(PathBuf)`, `ReplaceUnsupported`.
  - `source.rs`:
    - `pub(crate) enum LegacyValue { Null, Integer(i64), Real(f64), Text(String), InvalidText(Vec<u8>), Blob(Vec<u8>) }`: `as_text() -> String` (NULL → «», числа — десятичной записью, испорченный текст — с заменой символов) и `to_json() -> Value` (NULL → null, числа → числа, текст → строка, `InvalidText` и `Blob` → `{"hex": "…"}`).
    - `pub(crate) struct LegacyTable { pub name, pub columns: Vec<String>, pub rows: Vec<Vec<LegacyValue>> }`: `column(&self, name) -> Option<usize>` без учёта регистра (SQLite не различает регистр имён колонок, а MyWork 2 писала `Sostoyanie` и `sostoyanie`); `text(&self, row, name) -> String` (нет колонки → «»); `row_json(&self, row) -> Value` (объект «колонка → значение»).
    - `pub(crate) struct LegacyDb { pub tables: Vec<LegacyTable>, pub sequences: BTreeMap<String, i64> }`: `table(&self, name) -> Option<&LegacyTable>` без учёта регистра; `schema() -> SchemaSummary`.
    - `pub struct SchemaSummary { pub catalog_columns: usize, pub missing_columns: Vec<String>, pub tables: Vec<(String, u64)> }` — `missing_columns` из известного полного набора колонок `Catalog` (32).
    - `pub(crate) async fn read_database(db_file: &Path) -> Result<LegacyDb, LegacyError>`:
      - копирует файл и, если есть, `-journal`, `-wal`, `-shm` с тем же именем в `tempfile::TempDir`;
      - открывает копию (не `immutable`: SQLite должна сама откатить незавершённый журнал);
      - читает все таблицы, кроме `sqlite_*`, в порядке `sqlite_master`, плюс `sqlite_sequence`; ячейки — `typeof(c)` и `CAST(c AS BLOB)`;
      - закрывает соединение, временная папка удаляется.
      - Нет таблицы `Catalog` → `NotMyWork2`.
  - `text.rs`:
    - `pub(crate) enum TextEncoding { Utf8Bom, Utf16LeBom, Utf8, Windows1251 }`;
    - `pub(crate) fn decode_text(bytes: &[u8]) -> (String, TextEncoding)` — правило из «Решений»;
    - `pub(crate) fn text_lines(text: &str) -> Vec<String>` — как `StreamReader.ReadLine`: `\r\n`, `\n`, `\r`; без лишней пустой строки в конце;
    - `pub(crate) struct Ini { pub sections: Vec<IniSection> }`, `pub(crate) struct IniSection { pub name: String, pub entries: Vec<(String, String)> }`;
    - `pub(crate) fn parse_ini(text: &str) -> Ini` — `[секция]`, `ключ=значение` по первому `=`, края обрезаются, строки на `;` пропускаются; `Ini::get(section, key)` без учёта регистра, при повторе — первый ключ.
  - `folder.rs`:
    - `pub(crate) struct LegacyFolder` с полями:
      - `root: PathBuf`, `db_file: PathBuf`;
      - `config_ini: Option<Vec<u8>>`;
      - `text_files: BTreeMap<String, Vec<u8>>` — канонический путь вида `settings/brands.txt`, `settings/Akts/FirmName.txt`, `settings/Stock/brands.txt`, `Shablon_Priem.htm`;
      - `client_files: Vec<String>` — пути относительно `ClientFiles`, через `/`, по алфавиту;
    - `pub(crate) fn find_file(&self, relative: &str) -> Option<PathBuf>` — поиск без учёта регистра в каждом сегменте пути, нужен для фото склада;
    - `pub(crate) fn scan_folder(source: &Path) -> Result<LegacyFolder, LegacyError>` — `source` может быть папкой или файлом `baza.sqlite` в ней; все известные имена ищутся без учёта регистра; `settings/backup` не читается.
  - `fixture.rs` (`#[cfg(feature = "fixtures")]`):
    - `pub enum FixtureVersion { V24, V25, V26, V27, V29, V30, V31, V32 }`, `FixtureVersion::ALL`;
    - `pub struct FixtureOptions { pub version: FixtureVersion, pub extra_orders: usize }`;
    - `pub async fn write_fixture(dir: &Path, options: &FixtureOptions) -> std::io::Result<()>` — по разделу «Синтетическая папка MyWork 2»;
    - `pub const FIXTURE_SMS_TOKEN: &str = "fixture-sms-token-7f3a"`.
  - `examples/make_fixture.rs`: `cargo run -p mw-legacy --features fixtures --example make_fixture -- <папка> [v24|…|v32] [--extra-orders N]`.

- [ ] **Step 1: Write the failing tests** (модульные тесты в `source.rs`, `text.rs`, `folder.rs`; фикстуры пишутся во временные папки)

```rust
// source.rs
#[tokio::test]
async fn schema_is_detected_by_column_names_for_every_fixture_version() {
    for version in FixtureVersion::ALL {
        let dir = tempfile::tempdir().unwrap();
        write_fixture(dir.path(), &FixtureOptions { version, extra_orders: 0 }).await.unwrap();
        let db = read_database(&dir.path().join("baza.sqlite")).await.unwrap();
        let s = db.schema();
        let expected_columns = match version { V24 => 24, V25 => 25, V26 => 26, V27 => 27, V29 => 29, V30 => 30, V31 => 31, V32 => 32 };
        assert_eq!(s.catalog_columns, expected_columns, "{version:?}");
        assert_eq!(db.table("catalog").unwrap().rows.len(), 11, "{version:?}");
        assert_eq!(db.sequences.get("Catalog"), Some(&15), "{version:?}");
        if version == FixtureVersion::V26 {
            assert!(s.missing_columns.contains(&"Image_key".to_owned()));
            assert!(!s.missing_columns.contains(&"Adress".to_owned()));
        }
    }
}

#[tokio::test]
async fn cells_keep_their_storage_class() {
    // база-заготовка с Catalog и строкой: id=1, Data_priema=NULL, Predoplata=1500 (INTEGER), Zatrati=2.5 (REAL),
    // polomka=X'D0' (BLOB с неполным UTF-8), kommentarij = CAST(X'C0' AS TEXT)
    let row = &db.table("Catalog").unwrap().rows[0];
    let json = db.table("Catalog").unwrap().row_json(row);
    assert_eq!((&json["Data_priema"], &json["Predoplata"], &json["Zatrati"]), (&json!(null), &json!(1500), &json!(2.5)));
    assert_eq!(json["polomka"], json!({"hex": "d0"}));
    assert_eq!(json["kommentarij"], json!({"hex": "c0"}));
}

#[tokio::test]
async fn source_folder_is_not_modified() {
    let dir = fixture(V32).await;
    std::fs::write(dir.path().join("baza.sqlite-journal"), b"")?;     // пустой журнал рядом с базой
    let before = hash_tree(dir.path());                               // помощник: путь → sha256
    let _db = read_database(&dir.path().join("baza.sqlite")).await?;
    let _folder = scan_folder(dir.path())?;
    assert_eq!(hash_tree(dir.path()), before);
}

#[tokio::test]
async fn not_a_mywork2_database() {
    // SQLite-файл с одной таблицей t
    assert!(matches!(read_database(&path).await, Err(LegacyError::NotMyWork2)));
}

// text.rs
#[test]
fn decode_text_detects_bom_utf8_and_cp1251() {
    let (cp1251, _, _) = encoding_rs::WINDOWS_1251.encode("Ремонт");
    assert_eq!(decode_text(&cp1251), ("Ремонт".to_owned(), TextEncoding::Windows1251));
    assert_eq!(decode_text("\u{feff}Ремонт".as_bytes()), ("Ремонт".to_owned(), TextEncoding::Utf8Bom));
    assert_eq!(decode_text("Ремонт".as_bytes()), ("Ремонт".to_owned(), TextEncoding::Utf8));
    assert_eq!(decode_text(b"\xff\xfeA\x00"), ("A".to_owned(), TextEncoding::Utf16LeBom));
    assert_eq!(decode_text(b"plain"), ("plain".to_owned(), TextEncoding::Windows1251));
}

#[test]
fn text_lines_follow_readline() {
    assert_eq!(text_lines("a\r\nb\n\nc\r"), ["a", "b", "", "c"]);
    assert_eq!(text_lines("a\r\n"), ["a"]);
}

#[test]
fn ini_is_case_insensitive_and_first_key_wins() {
    let ini = parse_ini("[A]\nk=1\nK=2\n; комментарий\n[b]\n x = y z \n");
    assert_eq!(ini.get("a", "K"), Some("1"));
    assert_eq!(ini.get("B", "X"), Some("y z"));
}

// folder.rs
#[test]
fn scan_finds_files_case_insensitively() {
    // папка: BAZA.SQLITE, Settings/AKTS/firmname.TXT, CLIENTFILES/1/1/a.jpg, settings/backup/old.sqlite
    let f = scan_folder(&dir)?;
    assert!(f.text_files.contains_key("settings/Akts/FirmName.txt"));
    assert_eq!(f.client_files, ["1/1/a.jpg"]);
    assert!(!f.text_files.keys().any(|k| k.contains("backup")));
    assert_eq!(scan_folder(&dir.join("BAZA.SQLITE"))?.root, f.root);
}
```

- [ ] **Step 2: Run tests to verify they fail** — `cargo test -p mw-legacy` → ошибка компиляции.
- [ ] **Step 3: Implement** — модули по Interfaces; фикстура по разделу «Синтетическая папка MyWork 2».
- [ ] **Step 4: Run tests to verify they pass** — `cargo test -p mw-legacy` → всё зелёное; `cargo run -p mw-legacy --features fixtures --example make_fixture -- /tmp/mw-fixture-v32 v32` создаёт папку.
- [ ] **Step 5: Commit** — `feat(legacy): mw-legacy reads a copy of the MyWork 2 folder; synthetic fixtures of every schema version`.

---

### Task 9: `mw-legacy` — разбор значений

**Files:**
- Create: `crates/mw-legacy/src/parse/{mod.rs, dates.rs, money.rs, flags.rs, phones.rs, states.rs, tracking.rs}` (модульные тесты в каждом)

**Interfaces:**
- Produces:
  - `pub(crate) enum DateValue { Empty, Parsed(NaiveDateTime), MonthFirst(NaiveDateTime), Invalid }` и `pub(crate) fn parse_date(raw: &str) -> DateValue` — правило из «Решений».
  - `pub(crate) enum MoneyValue { Empty, Parsed(Money), Invalid }` и `pub(crate) fn parse_money(raw: &str) -> MoneyValue` — через `Money::parse`; отрицательные и прочие ошибки → `Invalid`; `pub(crate) const SUSPICIOUS_RUBLES: i64 = 1_000_000`.
  - `pub(crate) fn parse_flag(raw: &str) -> Option<bool>` — без учёта регистра и пробелов по краям:
    - `true`: «1», «true», «да», «yes», «checked», «заказать»;
    - `false`: «», «0», «-1», «false», «нет», «no», «unchecked»;
    - иначе `None`.
  - `pub(crate) struct PhoneSplit { pub phones: Vec<ClientPhone>, pub text_without_number: String, pub unusual: Vec<String>, pub reshaped: bool }` и `pub(crate) fn split_phones(raw: &str) -> PhoneSplit`.
    - Группы цифр: последовательности из цифр, пробелов и `+-().`. Слова — буквы. Разделители — `, ; / \ |` и перевод строки.
    - Классификация группы по числу цифр:
      - 11, начинается с 7 или 8 — один номер;
      - 10 и 5–7 — один номер;
      - 22, обе половины начинаются с 7 или 8 — два номера по 11;
      - 17 — 11 и 6, если первые 11 цифр начинаются с 7 или 8; иначе 6 и 11, если седьмая цифра 7 или 8;
      - иначе — один номер в `unusual`.
    - Слова «или», «и», «либо», «тел», «т» связывают номера и отбрасываются. Остальные слова — пометка ближайшего номера слева, а если слева нет номера — справа. Скобки вокруг пометки убираются.
    - «-», «?» и пустая строка — нет номеров. Текст без цифр → `text_without_number`.
    - Дубли по `phone_key` отбрасываются.
    - `reshaped` — номеров не один или есть пометка.
  - `pub(crate) enum StateEvent { Status(String), Master(String), FlagSet(String), FlagCleared(String), Message(String), Other(String) }`, `pub(crate) struct StatesRow { pub at: DateValue, pub event: StateEvent }` и `pub(crate) fn read_states_row(state: &str, date: &str) -> StatesRow`. Ориентация строки:
    - `state` разбирается как дата → дата из `state`, текст из `date`;
    - иначе `date` разбирается как дата → дата из `date`, а `state` — это имя статуса (`Status`);
    - иначе `at = Invalid`, текст — `date`, если он не пуст, иначе `state`.
    - Текст классифицируется по первой строке: «Установлен статус» → `Status`, «Назначен мастер» → `Master`, «Установлена галочка» → `FlagSet`, «Снята галочка» → `FlagCleared`, «Сообщение отправлено» → `Message`. Значение — вторая строка без пробелов по краям. Прочее → `Other(весь текст)`.
  - `pub(crate) fn extract_tracking(comment: &str) -> Vec<(String, String)>` — для каждой строки с `###`: номер — буквы, цифры и `_` сразу после `###`, описание — остаток строки без пробелов по краям (как `###(\w+)(.*)` в MyWork 2).

- [ ] **Step 1: Write the failing tests**

```rust
// dates.rs
let dt = |s: &str| NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").unwrap();
assert_eq!(parse_date("05-03-2015 14:20"), DateValue::Parsed(dt("2015-03-05 14:20:00")));
assert_eq!(parse_date("05-03-2015 14-20"), DateValue::Parsed(dt("2015-03-05 14:20:00")));
assert_eq!(parse_date("2014-11-02"), DateValue::Parsed(dt("2014-11-02 00:00:00")));
assert_eq!(parse_date("05.03.2015"), DateValue::Parsed(dt("2015-03-05 00:00:00")));
assert_eq!(parse_date("12.03.2015 15:04:05"), DateValue::Parsed(dt("2015-03-12 15:04:05")));
assert_eq!(parse_date("5.3.15 9:05"), DateValue::Parsed(dt("2015-03-05 09:05:00")));
assert_eq!(parse_date("2015-03-05T14:20:00"), DateValue::Parsed(dt("2015-03-05 14:20:00")));
assert_eq!(parse_date("03/04/2015"), DateValue::Parsed(dt("2015-04-03 00:00:00")));       // день первым
assert_eq!(parse_date("13/03/2015"), DateValue::Parsed(dt("2015-03-13 00:00:00")));
assert_eq!(parse_date("03/20/2015 9:15:00 PM"), DateValue::MonthFirst(dt("2015-03-20 21:15:00")));
assert_eq!(parse_date("  "), DateValue::Empty);
for bad in ["вчера", "31-02-2015 10:00", "31-13-2015 10:00", "01-01-0001 00:00", "05-03-2015 25:00", "2015"] {
    assert_eq!(parse_date(bad), DateValue::Invalid, "{bad}");
}

// money.rs
assert_eq!(parse_money("1500"), MoneyValue::Parsed(Money::from_rubles(1500)));
assert_eq!(parse_money(" 1 500 "), MoneyValue::Parsed(Money::from_rubles(1500)));
assert_eq!(parse_money("1500,50"), MoneyValue::Parsed(Money::from_kopecks(150_050)));
assert_eq!(parse_money("1500р"), MoneyValue::Parsed(Money::from_rubles(1500)));
assert_eq!(parse_money(""), MoneyValue::Empty);
for bad in ["abc", "-100", "1500.555"] { assert_eq!(parse_money(bad), MoneyValue::Invalid, "{bad}"); }

// flags.rs
for t in ["1", "True", "Да", "Заказать", " заказать ", "Checked"] { assert_eq!(parse_flag(t), Some(true), "{t}"); }
for f in ["", "0", "-1", "False", "нет", "Unchecked"] { assert_eq!(parse_flag(f), Some(false), "{f}"); }
assert_eq!(parse_flag("что-то"), None);

// phones.rs
fn nums(s: &PhoneSplit) -> Vec<(&str, &str)> { s.phones.iter().map(|p| (p.phone.as_str(), p.note.as_str())).collect() }
let two = vec![("89211234567", ""), ("89117654321", "")];
assert_eq!(nums(&split_phones("89211234567")), [("89211234567", "")]);
assert!(!split_phones("89211234567").reshaped);
assert_eq!(nums(&split_phones("+7 (921) 123-45-67")), [("79211234567", "")]);
for s in ["89211234567,89117654321", "8921123456789117654321", "89211234567 или 89117654321",
          "89211234567или89117654321", "8 921 123 45 67 8 911 765 43 21", "89211234567, 89211234567, 89117654321"] {
    assert_eq!(nums(&split_phones(s)), two, "{s}");
}
assert_eq!(nums(&split_phones("89211234567жена")), [("89211234567", "жена")]);
assert_eq!(nums(&split_phones("жена 89117654321")), [("89117654321", "жена")]);
assert_eq!(nums(&split_phones("89211234567/89117654321(раб)")), [("89211234567", ""), ("89117654321", "раб")]);
assert_eq!(nums(&split_phones("89211234567, 123456")), [("89211234567", ""), ("123456", "")]);
assert_eq!(nums(&split_phones("89211234567123456")), [("89211234567", ""), ("123456", "")]);
assert_eq!(nums(&split_phones("123456,89211234567")), [("123456", ""), ("89211234567", "")]);
let short = split_phones("1234");
assert_eq!((nums(&short), short.unusual.clone()), (vec![("1234", "")], vec!["1234".to_owned()]));
for empty in ["", " ", "-", "?"] {
    let s = split_phones(empty);
    assert!(s.phones.is_empty() && s.text_without_number.is_empty(), "{empty:?}");
}
let words = split_phones("нет телефона");
assert!(words.phones.is_empty());
assert_eq!(words.text_without_number, "нет телефона");

// states.rs
let r = read_states_row("05-03-2015 14-20", "Установлен статус\r\nДиагностика");
assert_eq!((r.at, r.event), (DateValue::Parsed(dt("2015-03-05 14:20:00")), StateEvent::Status("Диагностика".into())));
assert_eq!(read_states_row("Ждет ЗИП", "06-03-2015 10:00").event, StateEvent::Status("Ждет ЗИП".into()));
assert_eq!(read_states_row("06-03-2015 09:10", "Назначен мастер \r\nПЕТРОВ").event, StateEvent::Master("ПЕТРОВ".into()));
assert_eq!(read_states_row("x", "Установлена галочка\r\nТребует заказа").event, StateEvent::FlagSet("Требует заказа".into()));
assert_eq!(read_states_row("x", "Снята галочка\nНужно позвонить").event, StateEvent::FlagCleared("Нужно позвонить".into()));
assert_eq!(read_states_row("x", "Сообщение отправлено\r\nВаш заказ готов").event, StateEvent::Message("Ваш заказ готов".into()));
assert_eq!(read_states_row("мусор", "Установлен статус\r\nГотов").at, DateValue::Invalid);
assert_eq!(read_states_row("07-03-2015 12-00", "").event, StateEvent::Other(String::new()));

// tracking.rs
assert_eq!(extract_tracking("Позвонить\r\n###RA123456789RU посылка\r\n###80081234567890"),
           vec![("RA123456789RU".to_owned(), "посылка".to_owned()), ("80081234567890".to_owned(), String::new())]);
assert!(extract_tracking("### пусто").is_empty());
assert!(extract_tracking("без треков").is_empty());
```

- [ ] **Step 2: Run tests to verify they fail** — `cargo test -p mw-legacy parse` → ошибка компиляции.
- [ ] **Step 3: Implement** — модули по Interfaces.
- [ ] **Step 4: Run tests to verify they pass** — `cargo test -p mw-legacy` → всё зелёное.
- [ ] **Step 5: Commit** — `feat(legacy): parsers for legacy dates, money, flags, phones, status log and tracking numbers`.

---

### Task 10: `mw-legacy` — статусы, точки, справочники, настройки, роли и пользователи

**Files:**
- Create: `crates/mw-legacy/src/report.rs`, `crates/mw-legacy/src/convert/{mod.rs, catalogs.rs, settings.rs, access.rs}` (модульные тесты на фикстурах)
- Modify: `crates/mw-legacy/src/test_support.rs` (`prepared(version) -> (LegacyDb, LegacyFolder)`, `codes(&Warnings) -> Vec<(&str, &str)>`)

**Interfaces:**
- Consumes: `LegacyDb`, `LegacyFolder`, `decode_text`, `text_lines`, `parse_ini`, `parse_date`, `parse_flag`, `ImportedSettings`, `ImportedUser`, `ImportedLegacyRow`, `NewStatus`, `NewRole`, `DEFAULT_STATUSES`, `DEFAULT_DICTIONARY`, `normalize_search`, `legacy_sha1_hex`.
- Produces:
  - `report.rs`:
    - `pub enum Severity { Warning, Info }`;
    - `pub enum WarningCode` — все коды из таблицы «Предупреждения отчёта», с методами `as_str()`, `severity()`, `title_ru()`;
    - `pub struct Warning { pub code: WarningCode, pub source: String, pub key: String, pub field: String, pub value: String, pub message: String }` (`Serialize`);
    - `pub(crate) struct Warnings` (`Default`) с методами `push(code, source, key, field, value, message)` и `list(&self) -> &[Warning]`.
  - `catalogs.rs`:
    - `pub(crate) struct Catalogs { pub statuses: Vec<NewStatus>, pub service_points: Vec<String>, pub dictionary: Vec<(DictionaryKind, Vec<String>)> }` с методами `status_for(&self, legacy: &str) -> String` (имя статуса MyWork 3 для исходного текста) и `point_for(&self, legacy: &str) -> Option<String>`;
    - `pub(crate) fn convert_catalogs(db: &LegacyDb, folder: &LegacyFolder, issued: &dyn Fn(i64) -> bool, w: &mut Warnings) -> Catalogs`. Аргумент `issued(order_id)` говорит, есть ли у заказа разобранная дата выдачи. Правила — «Решения» (статусы, справочники) и «Карта переноса».
    - Стартовое содержимое файлов MyWork 2 (для `dictionary_default_kept`) хранится константой и переписано из `Form1.FilesExistsOrNot`.
  - `settings.rs`: `pub(crate) fn convert_settings(folder: &LegacyFolder, catalogs: &Catalogs, w: &mut Warnings) -> (ImportedSettings, Vec<ImportedLegacyRow>)`:
    - разбирает `Config.ini` и `settings/Akts/*`;
    - все ключи разрешённых секций идут в `legacy_rows` (источник `Config.ini`, ключ `[секция] ключ`), каждый текстовый файл — в `legacy_rows` (источник `file`, сырое `{ "encoding": …, "text": … }`);
    - `SMSSEND` и `ACTIVATION` пропускаются целиком.
  - `access.rs`: `pub(crate) fn convert_access(db: &LegacyDb, w: &mut Warnings) -> (Vec<NewRole>, Vec<ImportedUser>, Vec<ImportedLegacyRow>)` — по «Карте переноса» и «Решениям»; роль «Без прав» создаётся, только если нужна.

- [ ] **Step 1: Write the failing tests** (помощник `prepared(version) -> (LegacyDb, LegacyFolder)` пишет фикстуру во временную папку; `codes(&Warnings) -> Vec<(&str, &str)>` — пары (код, ключ) по порядку)

```rust
// catalogs.rs: statuses_points_and_dictionaries_follow_the_rules
let (db, folder) = prepared(V32).await;
let mut w = Warnings::default();
let c = convert_catalogs(&db, &folder, &|id| [1, 3, 6].contains(&id), &mut w);
assert_eq!(c.statuses.iter().skip(8).map(|s| (s.name.as_str(), s.kind)).collect::<Vec<_>>(),
           [("Куплен", StatusKind::Issued), ("Отдан сервисному центру", StatusKind::Open), ("Без статуса", StatusKind::Open)]);
assert_eq!(c.status_for("Ждет ЗИП"), "Ждёт запчасть");
assert_eq!(c.service_points, ["Мира, 5", "Ленина, 1", "ВОКЗАЛЬНАЯ"]);
assert_eq!(c.point_for("ЛЕНИНА, 1").as_deref(), Some("Ленина, 1"));
let dict = |k: DictionaryKind| c.dictionary.iter().find(|(kind, _)| *kind == k).unwrap().1.clone();
let defaults = |k: DictionaryKind| DEFAULT_DICTIONARY.iter().find(|(kind, _)| *kind == k).unwrap().1.iter().map(|s| s.to_string()).collect::<Vec<_>>();
assert_eq!(dict(DictionaryKind::Brand), ["Samsung", "Apple"]);
assert_eq!(dict(DictionaryKind::Master), ["ПЕТРОВ", "СИДОРОВ"]);
assert_eq!(dict(DictionaryKind::DeviceType), defaults(DictionaryKind::DeviceType));
assert_eq!(dict(DictionaryKind::ClientSource), defaults(DictionaryKind::ClientSource));
assert_eq!(dict(DictionaryKind::Warranty), ["30 дней", "3 месяца", "Без гарантии"]);
assert_eq!(c.dictionary.len(), DEFAULT_DICTIONARY.len());
let info: Vec<_> = w.list().iter().map(|x| x.code.as_str()).collect();
for code in ["status_renamed", "dictionary_placeholder_skipped", "dictionary_default_kept", "text_decoded_utf8"] {
    assert!(info.contains(&code), "{code}");
}
assert_eq!(info.iter().filter(|c| **c == "status_custom").count(), 3);

// settings.rs: settings_come_from_config_and_act_texts
let (s, rows) = convert_settings(&folder, &c, &mut w);
let v = &s.values;
assert_eq!((v.currency_symbol.as_deref(), v.master_percent, v.diagnostics_overdue_days, v.intake_required_fields),
           (Some("руб."), Some(40), Some(5), Some(false)));
assert_eq!((v.default_warranty.as_deref(), s.default_service_point.as_deref(), v.default_master.as_deref()),
           (Some("Без гарантии"), Some("Ленина, 1"), Some("ПЕТРОВ")));
assert_eq!((v.firm_name.as_deref(), v.firm_phone.as_deref()), (Some("СЕРВИС-ПЛЮС"), Some("8 (800) 000-00-00")));
assert_eq!(v.firm_details.as_deref(), Some("Режим работы: 10–19\nг. Город, ул. Примерная, 1"));
assert_eq!(v.intake_terms.as_deref(), Some("Строка договора 1\nСтрока договора 2"));
assert_eq!((v.timezone.clone(), v.default_service_point_id), (None, None));
assert_eq!(rows.iter().filter(|r| r.source == "Config.ini").count(), 16);
assert_eq!(rows.iter().filter(|r| r.source == "file").count(), 13);
assert!(!format!("{rows:?}").contains(FIXTURE_SMS_TOKEN));
assert_eq!(w.list().iter().filter(|x| x.code.as_str() == "setting_preserved").count(), 8);
assert!(codes(&w).contains(&("config_section_ignored", "S2Z9")));

// access.rs: access_maps_groups_logins_and_hashes
let (roles, users, rows) = convert_access(&db, &mut w);
assert_eq!(roles.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
           ["Администратор", "Админы", "Мастера", "Администратор (MyWork 2)", "Без прав"]);
assert_eq!(roles[0].permissions, PermissionSet::all());
assert_eq!(roles[1].permissions, PermissionSet::all());
assert_eq!(roles[2].permissions, PermissionSet::from_codes(["orders_edit", "clients_view", "clients_edit"]));
assert_eq!(users.iter().map(|u| (u.login.as_str(), u.role.as_str())).collect::<Vec<_>>(),
           [("Ирина", "Админы"), ("ирина (2)", "Мастера"), ("Пётр", "Без прав"), ("Олег", "Мастера")]);
assert_eq!(users[2].legacy_sha1, legacy_sha1_hex("qwerty"));
assert_eq!(rows.iter().filter(|r| r.source == "Users").count(), 4);
for pair in [("login_renamed", "2"), ("user_group_missing", "3"), ("user_password_invalid", "4")] {
    assert!(codes(&w).contains(&pair), "{pair:?}");
}
// V31: единственный пользователь в группе без флага settings
let (db31, _) = prepared(V31).await;
let mut w31 = Warnings::default();
let (roles31, users31, _) = convert_access(&db31, &mut w31);
let role = roles31.iter().find(|r| r.name == users31[0].role).unwrap();
assert!(role.permissions.contains(Permission::UsersManage) && role.permissions.contains(Permission::AuditView));
assert_eq!(codes(&w31), [("users_manage_granted", "Мастера")]);
```

- [ ] **Step 2: Run tests to verify they fail** — `cargo test -p mw-legacy convert` → ошибка компиляции.
- [ ] **Step 3: Implement** — `report.rs` и три модуля.
- [ ] **Step 4: Run tests to verify they pass** — `cargo test -p mw-legacy` → всё зелёное.
- [ ] **Step 5: Commit** — `feat(legacy): statuses, service points, dictionaries, settings, roles and users from MyWork 2`.

---

### Task 11: `mw-legacy` — клиенты, заказы, история, нумерация: сборка `ImportBundle`

**Files:**
- Create: `crates/mw-legacy/src/convert/{clients.rs, orders.rs, history.rs}`
- Modify: `crates/mw-legacy/src/convert/mod.rs` (сборка), модульные тесты в `convert/mod.rs`

**Interfaces:**
- Consumes: задачи 8–10, `ImportBundle` и его части (задача 7), `order_barcode`, `is_valid_upc_a`, `Money`, `MAX_CLIENT_PHONES`.
- Produces:
  - `pub(crate) struct PlannedFile { pub source: PathBuf, pub dest: PathBuf, pub legacy_source: &'static str, pub legacy_key: String, pub target: Option<LegacyTarget> }`:
    - `source` — путь относительно корня старой папки;
    - `dest` — путь относительно `attachments`;
    - `legacy_source` — `ClientFiles` или `Stock.Photo`.
  - `pub(crate) struct Conversion { pub bundle: ImportBundle, pub warnings: Warnings, pub files: Vec<PlannedFile>, pub schema: SchemaSummary }`.
  - `pub(crate) fn convert(db: &LegacyDb, folder: &LegacyFolder, now: NaiveDateTime) -> Conversion`. `now` — время импорта для подстановок. Порядок работы:
    1. Даты выдачи заказов.
    2. `convert_catalogs`.
    3. Клиенты:
       - `ClientsMap` — по «Решениям». Телефоны через `split_phones`, лишние сверх `MAX_CLIENT_PHONES` уходят в заметку. Текст без номера дописывается в заметку строкой «Телефон: {текст}». Пустые ФИО и номера → «Клиент без имени» и `client_empty`.
       - Восстановленные и «Клиент не указан» получают номера от max(`sqlite_sequence.ClientsMap`, max id) + 1 по возрастанию номера заказа.
    4. Заказы — по «Карте колонок» и «Решениям» (подстановка даты приёма идёт по возрастанию номера).
    5. История: `StatesMap` и `HistoryBD` → `ImportedHistory`.
       - Неразобранная дата → дата приёма заказа, иначе `now`.
       - `client_id` — клиент заказа.
       - `details = {"legacy": {"table": …, "id": …, "row": …}}`.
       - Сортировка по (дата, таблица, id).
    6. `legacy_rows` всех источников из «Карты переноса», кроме файлов: файлы добавит задача 12 после копирования.
    7. `ClientFiles` и `Stock.Photo*` → `files`. Обратные косые → `/`. Абсолютный путь Windows → поиск имени файла в `settings/Stock/Photos`. Папка не найденного заказа и файлы прямо в папке клиента → `attachments/legacy/ClientFiles/…`.
    8. `next_order_id` = max(`sqlite_sequence.Catalog`, max id) + 1; `next_client_id` — после всех клиентов и `sqlite_sequence.ClientsMap`. Сведение `next_number`.
    9. Дубли штрихкодов — одно предупреждение на группу: ключ — штрихкод, значение — «заказы 2, 3».

- [ ] **Step 1: Write the failing tests** (`convert/mod.rs`; `conversion_of(version)` пишет фикстуру и вызывает `convert` с `now` = `2026-11-12 10:00:00`; `dt`, `rub`, `order(b, id)`, `client(b, id)` — помощники)

```rust
// v32_bundle_matches_the_fixture
let c = conversion_of(V32).await;
let b = &c.bundle;
assert_eq!((b.orders.len(), b.clients.len(), b.next_order_id, b.next_client_id), (11, 11, 16, 15));
let o2 = order(b, 2);
assert_eq!((o2.received_at, o2.prepaid_at, o2.status.as_str()), (dt("2015-03-05 14:20:00"), Some(dt("2015-03-05 00:00:00")), "Ждёт запчасть"));
assert_eq!((o2.estimate_from, o2.estimate_to, o2.final_price, o2.prepayment), (Some(rub(1500)), Some(rub(1500)), None, rub(500)));
assert_eq!((o2.need_parts, o2.warranty.as_str()), (true, "3 мес"));
assert_eq!(o2.tracking, [("RA123456789RU".to_owned(), "посылка".to_owned())]);
assert!(o2.internal_comment.ends_with("###RA123456789RU посылка"));
let o3 = order(b, 3);
assert_eq!((o3.status.as_str(), o3.final_price, o3.need_call), ("Куплен", Some(Money::ZERO), true));
let o4 = order(b, 4);
assert_eq!((o4.received_at, o4.estimate_from, o4.barcode.as_str()), (dt("2015-03-07 11:00:00"), None, order_barcode(4).unwrap().as_str()));
assert_eq!(order(b, 5).received_at, dt("2015-03-07 11:00:00"));
assert_eq!(order(b, 6).issued_at, Some(dt("2015-03-13 00:00:00")));
assert_eq!(order(b, 8).deleted_at, Some(dt("2015-03-16 12:00:00")));
assert_eq!(order(b, 8).status, "Без статуса");
assert_eq!((order(b, 9).client_id, client(b, 13).full_name.as_str()), (13, "Клиент не указан"));
assert_eq!((order(b, 10).client_id, client(b, 14).full_name.as_str()), (14, "ПЕТРОВ"));
assert_eq!(order(b, 12).service_point.as_deref(), Some("Ленина, 1"));
assert_eq!(order(b, 6).service_point.as_deref(), Some("ВОКЗАЛЬНАЯ"));
let ph = |id| client(b, id).phones.iter().map(|p| (p.phone.clone(), p.note.clone())).collect::<Vec<_>>();
assert_eq!(ph(1), [("89211234567".into(), String::new()), ("89117654321".into(), String::new())]);
assert_eq!(ph(3), [("89211234567".into(), "жена".to_owned())]);
assert_eq!(ph(8), [("89211234567".into(), String::new()), ("123456".into(), String::new())]);
assert_eq!(client(b, 7).note, "Телефон: нет телефона");
assert!(client(b, 2).blacklisted && client(b, 4).blacklisted && !client(b, 5).blacklisted);
let h2: Vec<_> = b.history.iter().filter(|h| h.order_id == Some(2)).map(|h| (h.action.as_str(), h.summary.as_str())).collect();
assert_eq!(h2, [("legacy.status", "Установлен статус «Диагностика»"), ("legacy.history", "СОХРАНЕНИЕ"),
                ("legacy.status", "Установлен статус «Ждет ЗИП»"), ("legacy.flag", "Установлена галочка «Требует заказа»")]);
assert!(b.history.windows(2).all(|w| w[0].at <= w[1].at));
assert_eq!(b.history.len(), 13);
let count = |src: &str| b.legacy_rows.iter().filter(|r| r.source == src).count();
assert_eq!((count("Catalog"), count("ClientsMap"), count("GroupDostup"), count("Stock"), count("StockMap")), (11, 9, 3, 2, 1));
assert_eq!(c.files.len(), 7);   // 5 из ClientFiles и 2 фото склада (одно из них не существует)

// v32_warnings_are_exactly_expected
let mut got: Vec<_> = c.warnings.list().iter().filter(|w| w.code.severity() == Severity::Warning)
    .map(|w| (w.code.as_str(), w.source.as_str(), w.key.as_str())).collect();
got.sort();
assert_eq!(got, [
    ("amount_suspicious", "Catalog", "6"), ("barcode_duplicate", "Catalog", "123456789012"), ("barcode_invalid", "Catalog", "5"),
    ("client_missing", "Catalog", "10"), ("config_section_ignored", "Config.ini", "S2Z9"), ("date_month_first", "Catalog", "12"),
    ("date_unparsed", "Catalog", "4"), ("date_unparsed", "Catalog", "5"), ("date_unparsed", "HistoryBD", "4"),
    ("date_unparsed", "StatesMap", "7"), ("flag_unrecognized", "Catalog", "12"), ("flag_unrecognized", "ClientsMap", "5"),
    ("history_orphan", "HistoryBD", "5"), ("history_orphan", "StatesMap", "6"), ("issued_without_date", "Catalog", "7"),
    ("login_renamed", "Users", "2"), ("money_unparsed", "Catalog", "4"), ("order_without_client", "Catalog", "9"),
    ("phone_no_digits", "ClientsMap", "7"), ("phone_unusual", "ClientsMap", "9"), ("received_at_missing", "Catalog", "4"),
    ("received_at_missing", "Catalog", "5"), ("status_empty", "Catalog", "8"), ("user_group_missing", "Users", "3"),
    ("user_password_invalid", "Users", "4"),
]);
let info = |code: &str| c.warnings.list().iter().filter(|w| w.code.as_str() == code).count();
assert_eq!((info("phone_split"), info("client_duplicates"), info("barcode_generated"), info("tracking_found"),
            info("stock_preserved"), info("next_number")), (6, 1, 1, 1, 1, 1));

// old_schema_builds_clients_from_orders
let c = conversion_of(V25).await;
let b = &c.bundle;
assert_eq!((b.clients.len(), b.next_client_id, b.history.len()), (7, 8, 0));
assert_eq!(order(b, 8).client_id, order(b, 1).client_id);                     // «иванов иван » слился с «ИВАНОВ ИВАН»
assert_eq!(client(b, order(b, 9).client_id).full_name, "Клиент не указан");
assert!(b.orders.iter().all(|o| !o.need_call));                                // в V25 нет Image_key
```

- [ ] **Step 2: Run tests to verify they fail** — `cargo test -p mw-legacy convert::` → ошибка компиляции.
- [ ] **Step 3: Implement** — три модуля и `convert`.
- [ ] **Step 4: Run tests to verify they pass** — `cargo test -p mw-legacy` → всё зелёное.
- [ ] **Step 5: Commit** — `feat(legacy): clients, orders, history and numbering into an import bundle`.

---

### Task 12: `mw-legacy` — копии файлов, запуск импорта, отчёт

**Files:**
- Create: `crates/mw-legacy/src/files.rs`, `crates/mw-legacy/tests/{common/mod.rs, import.rs}`
- Modify: `crates/mw-legacy/src/{lib.rs, report.rs}`

**Interfaces:**
- Consumes: всё из задач 7–11.
- Produces:
  - `pub struct ImportOptions { pub source: PathBuf, pub attachments_dir: PathBuf, pub backups_dir: PathBuf, pub replace: bool }`.
  - `pub struct FileStats { pub copied: u64, pub bytes: u64, pub missing: u64 }`.
  - `pub struct ImportReport { pub started_at: NaiveDateTime, pub source: PathBuf, pub schema: SchemaSummary, pub summary: ImportSummary, pub files: FileStats, pub backup: Option<PathBuf>, pub warnings: Vec<Warning> }` (`Serialize`; `backup` — папка `backups_dir/<время>` с `mywork.db` и `attachments/`, только при замене):
    - `summary_text(&self) -> String` — заголовок, версия базы, блок «Перенесено» с числами по сущностям, блок «Требуют внимания — N» и «Сведения — M» с числом предупреждений по каждому коду (`title_ru` — число). Никаких значений из старой базы;
    - `full_text(&self) -> String` — сводка и все предупреждения построчно: заголовок, источник, ключ, поле, исходное значение, пояснение;
    - `to_json(&self) -> serde_json::Value`, `count(&self, code: WarningCode) -> usize`.
  - `files.rs`:
    - `pub(crate) struct Staging` — папка `attachments/.import-<ГГГГММДД-ЧЧММСС>`;
    - `stage(folder, files, &mut Warnings) -> Result<(Staging, Vec<ImportedLegacyRow>, FileStats), LegacyError>` — копирует с подсчётом sha256 и размера. Сырое значение строки следа — `{"stored_as": "attachments/…", "size": N, "sha256": "…"}`. Отсутствие файла → `file_missing`, ошибка чтения → `file_unreadable`. При совпадении имён в одной папке к имени добавляется « (2)»;
    - `Staging::abort(self)` удаляет промежуточную папку;
    - `Staging::commit(self, attachments_dir, previous: Option<&Path>) -> Result<(), LegacyError>`: при `previous = Some(папка копии)` существующие `orders`, `stock` и `legacy` переносятся туда, затем содержимое промежуточной папки встаёт на место и она удаляется.
  - `pub async fn import_folder(core: &Core, actor: &Actor, options: &ImportOptions) -> Result<ImportReport, LegacyError>`:
    1. `scan_folder`, `read_database`.
    2. Предварительные проверки:
       - `replace` на PostgreSQL → `ReplaceUnsupported`;
       - без `replace` при непустой базе → `TargetNotEmpty`;
       - без `replace` при непустых `attachments/orders`, `stock` или `legacy` → `AttachmentsNotEmpty`.
    3. `convert(… core.now())`.
    4. `stage`.
    5. В `bundle.report` — JSON со схемой, предупреждениями и `FileStats`.
    6. `core.import_bundle` в режиме `IntoEmpty` или `Replace { backup_to: backups_dir/<время>/mywork.db }`. При ошибке — `abort` и ошибка.
    7. `commit` (при `replace` старые вложения уходят в `backups_dir/<время>/attachments`).
    8. Отчёт.
  - `tracing::info!` на каждом этапе — только счётчики.

- [ ] **Step 1: Write the failing tests** (`tests/import.rs`, каждый тест по `cores()`). Помощники в `common`:
  - `cores()`, `admin()`, `sample_new_order()` — по образцу `mw-core/tests/common`;
  - `fixture_dir(version) -> TempDir` и `fixture_dir_with(version, extra_orders) -> TempDir`;
  - `options(src, data, replace) -> ImportOptions` (`attachments_dir` = `data/attachments`, `backups_dir` = `data/backups`);
  - `hash_tree(path)` — путь → sha256;
  - `dump_all_tables(&Core) -> String` — JSON всех строк всех таблиц;
  - `source_counts(path) -> SourceCounts` — прямые `COUNT(*)` по исходной базе;
  - `money_totals(&Core) -> [i64; 5]` — суммы копеек `estimate_from`, `prepayment`, `final_price`, `extra_costs`, `discount` по всем заказам;
  - `legacy_target(&Core, source, key)`;
  - `file_sqlite_core(path) -> Core` — файловый SQLite с миграциями;
  - `order_count_in_file(path) -> u64`.

```rust
// every_schema_version_imports_without_losing_rows
for version in FixtureVersion::ALL {
    for t in cores().await {
        let (src, data) = (fixture_dir(version).await, tempfile::tempdir()?);
        let r = import_folder(&t.core, &admin(), &options(src.path(), data.path(), false)).await?;
        let n = source_counts(src.path()).await;
        let label = format!("{version:?} {}", t.label);
        assert_eq!((r.summary.orders, r.summary.history), (n.catalog, n.states_map + n.history_bd), "{label}");
        assert_eq!(r.summary.clients, if n.clients_map > 0 { n.clients_map + 2 } else { 7 }, "{label}");
        assert_eq!(r.summary.next_order_id, 16, "{label}");
        assert_eq!(money_totals(&t.core).await, [150_000, 50_000, 1_500_150_000, 0, 0], "{label}");
        assert_eq!(t.core.create_order(&admin(), sample_new_order()).await?.id, 16, "{label}");
    }
}

// v32_files_and_warnings_round_trip
let r = import_folder(&t.core, &admin(), &options(src.path(), data.path(), false)).await?;
let att = data.path().join("attachments");
assert_eq!(std::fs::read(att.join("orders/1/docs/act.pdf"))?, b"%PDF-fixture");
assert_eq!(std::fs::read(att.join("orders/3/video.mp4"))?, b"video-3");
assert_eq!(std::fs::read(att.join("stock/1/abc123.jpg"))?, b"stock-photo");
assert!(att.join("legacy/ClientFiles/2/404/lost.jpg").exists() && att.join("legacy/ClientFiles/2/note.txt").exists());
assert!(!std::fs::read_dir(&att)?.any(|e| e.unwrap().file_name().to_string_lossy().starts_with(".import")));
assert_eq!((r.files.copied, r.files.missing), (6, 1));
assert_eq!((r.count(WarningCode::FileMissing), r.count(WarningCode::ClientFilesOrphan)), (1, 2));
assert_eq!(legacy_target(&t.core, "ClientFiles", "1/1/photo.jpg").await, Some(("orders".to_owned(), 1)));
assert_eq!(r.warnings.iter().filter(|w| w.code.severity() == Severity::Warning).count(), 26);   // 25 из задачи 11 + file_missing

// source_folder_is_untouched_by_full_import
let before = hash_tree(src.path());
import_folder(&t.core, &admin(), &options(src.path(), data.path(), false)).await?;
assert_eq!(hash_tree(src.path()), before);

// sms_token_is_never_stored_and_summary_has_no_personal_data
let r = import_folder(&t.core, &admin(), &options(src.path(), data.path(), false)).await?;
assert!(!dump_all_tables(&t.core).await.contains(FIXTURE_SMS_TOKEN));
assert!(!r.full_text().contains(FIXTURE_SMS_TOKEN));
let s = r.summary_text();
for personal in ["ИВАНОВ", "89211234567", "Ленина", "ПЕТРОВ", "Ирина"] { assert!(!s.contains(personal), "{personal}"); }
assert!(s.contains("Перенесено") && s.contains("заказов — 11"));

// legacy_users_log_in_after_import
import_folder(&t.core, &admin(), &options(fixture_dir(V32).await.path(), data.path(), false)).await?;
t.core.login("ирина", "1234", "x").await?;        // «Ирина»: логин теперь без учёта регистра
t.core.login("ирина (2)", "пароль", "x").await?;
t.core.login("Пётр", "qwerty", "x").await?;
assert!(matches!(t.core.login("Олег", "zzz", "x").await, Err(CoreError::InvalidCredentials)));
assert!(!t.core.is_open_mode().await?);

// open_mode_kept_when_old_program_had_no_users
import_folder(&t.core, &admin(), &options(fixture_dir(V30).await.path(), data.path(), false)).await?;
assert!(t.core.is_open_mode().await?);

// second_import_needs_replace_and_replace_keeps_a_backup — только файловый SQLite
let r1 = import_folder(&core, &admin(), &options(src.path(), data.path(), false)).await?;
assert!(matches!(import_folder(&core, &admin(), &options(src.path(), data.path(), false)).await, Err(LegacyError::TargetNotEmpty(_))));
core.create_order(&admin(), sample_new_order()).await?;
let r2 = import_folder(&core, &admin(), &options(src.path(), data.path(), true)).await?;
let backup = r2.backup.clone().unwrap();
assert_eq!(order_count_in_file(&backup.join("mywork.db")).await, 12);
assert!(backup.join("attachments/orders/1/photo.jpg").exists());
assert_eq!((r2.summary.orders, core.import_target_state().await?.orders), (11, 11));
assert_ne!(r1.summary.import_id, r2.summary.import_id);

// failed_import_leaves_attachments_untouched
t.core.create_client(&admin(), NewClient { full_name: "Уже есть".into(), ..Default::default() }).await?;
assert!(import_folder(&t.core, &admin(), &options(src.path(), data.path(), false)).await.is_err());
assert!(!data.path().join("attachments").exists() || std::fs::read_dir(data.path().join("attachments"))?.next().is_none());

// large_fixture_imports_on_every_backend — объём (extra_orders 3000), без порогов времени
let src = fixture_dir_with(V32, 3000).await;
let r = import_folder(&t.core, &admin(), &options(src.path(), data.path(), false)).await?;
assert_eq!(r.summary.orders, 3011);
```

`files.rs`, модульный тест `staging_is_removed_on_abort`: `stage` двух файлов во временную `attachments`, затем
`abort` → в `attachments` не осталось ни промежуточной папки, ни файлов.

- [ ] **Step 2: Run tests to verify they fail** — `cargo test -p mw-legacy --test import` → ошибка компиляции.
- [ ] **Step 3: Implement** — `files.rs`, `import_folder`, отчёт.
- [ ] **Step 4: Run tests to verify they pass** — `cargo test --workspace` с `MW_TEST_PG_URL` и без.
- [ ] **Step 5: Commit** — `feat(legacy): file copies, import orchestration and report`.

---

### Task 13: `mw-server` — команда `import-legacy`

**Files:**
- Create: `crates/mw-server/src/import.rs`, `crates/mw-server/tests/import.rs`
- Modify: `crates/mw-server/{Cargo.toml (mw-legacy; dev: mw-legacy с fixtures, tempfile), src/cli.rs, src/lib.rs, src/config.rs}`

**Interfaces:**
- Consumes: `import_folder`, `ImportOptions`, `ImportReport`, `LegacyError`; `connect_and_migrate`, `Core::open`, `system_timezone` (план 2).
- Produces:
  - `Command::ImportLegacy { source: PathBuf, #[arg(long)] replace: bool }`. Справка:
    - «Перенести данные из папки MyWork 2 (или файла baza.sqlite в ней). Импорт — только в пустую базу; --replace заменяет данные SQLite-базы, сначала сохранив копию.»
  - `pub struct ImportOutcome { pub report: ImportReport, pub text_report: PathBuf, pub json_report: PathBuf }`.
  - `pub async fn run_import_with_db(config: &Config, db: DatabaseConnection, source: &Path, replace: bool) -> Result<ImportOutcome, ServerError>`:
    - `Core::open` и `import_folder` от `Actor::unrestricted("Импорт MyWork 2")`;
    - `attachments_dir = data_dir/attachments`, `backups_dir = data_dir/backups`;
    - отчёты — `data_dir/import-reports/mywork2-<ГГГГММДД-ЧЧММСС>.txt` (`full_text`) и `.json`.
  - `ServerError::Import(LegacyError)`. Для `TargetNotEmpty` сообщение команды добавляет подсказку про `--replace` и про то, что для PostgreSQL нужна новая пустая база.
  - В `run`: `connect_and_migrate`, `run_import_with_db`, закрыть базу. Затем напечатать:
    - `summary_text()` и пути отчётов;
    - если пользователей нет — строку «Пользователей нет: на этом компьютере вход без пароля, как в MyWork 2. Для работы по сети создайте администратора: mw-server create-admin»;
    - строку «Если сервер уже запущен с этой базой, перезапустите его».

- [ ] **Step 1: Write the failing tests** (`tests/import.rs`, по `all_backends()`; `Config` с временной папкой данных)

```rust
#[tokio::test]
async fn import_legacy_command_imports_fixture_and_writes_report() {
    for (label, db) in mw_db::testing::all_backends().await {
        let (src, data) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        write_fixture(src.path(), &FixtureOptions { version: FixtureVersion::V32, extra_orders: 0 }).await.unwrap();
        let config = test_config(data.path());                                    // помощник: listen 127.0.0.1:0, data_dir
        let out = run_import_with_db(&config, db.clone(), src.path(), false).await.unwrap();
        assert_eq!(out.report.summary.orders, 11, "{label}");
        let text = std::fs::read_to_string(&out.text_report).unwrap();
        assert!(text.contains("Перенесено") && text.contains("Не разобрана дата"), "{label}");
        assert!(!text.contains(FIXTURE_SMS_TOKEN));
        assert!(serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(&out.json_report).unwrap()).is_ok());
        let again = run_import_with_db(&config, db, src.path(), false).await.unwrap_err().to_string();
        assert!(again.contains("--replace"), "{label}: {again}");
    }
}
```

`cli.rs`, модульный тест: `Cli::try_parse_from(["mw-server", "import-legacy", "/old", "--replace"])` даёт
`Command::ImportLegacy { source: "/old", replace: true }`.

- [ ] **Step 2: Run tests to verify they fail** — `cargo test -p mw-server --test import` → ошибка компиляции.
- [ ] **Step 3: Implement** — по Interfaces.
- [ ] **Step 4: Run tests to verify they pass** — `cargo test --workspace` с `MW_TEST_PG_URL` и без; вручную `cargo run -p mw-legacy --features fixtures --example make_fixture -- <tmp>/old v32 && cargo run -p mw-server -- --data-dir <tmp>/data import-legacy <tmp>/old` печатает сводку.
- [ ] **Step 5: Commit** — `feat(server): import-legacy command`.

---

### Task 14: Проверка на реальной базе автора, документация, финальная проверка

**Files:**
- Create: `crates/mw-legacy/tests/real_db.rs`
- Modify: `AGENTS.md`, `README.md`, `docs/superpowers/plans/README.md`; по итогам проверки — `crates/mw-legacy/src/fixture.rs` и разборщики (только если найдены новые формы значений)

**Interfaces:**
- Produces:
  - `real_db.rs` — один тест `#[ignore]` `real_mywork2_folder_imports_without_loss`. Папка берётся из `MW_LEGACY_REAL_DIR`; без переменной тест сразу завершается. Импорт по `all_backends()`, проверки:
    - заказов = строк `Catalog`, удалённых = строк с `Deleted` = «1»;
    - `need_call` = `Image_key` = «1», `need_parts` = распознанные `wait_zakaz`, в чёрном списке = `Blist` «1»;
    - клиентов = строк `ClientsMap` + восстановленные;
    - записей истории = `StatesMap` + `HistoryBD`;
    - следующий номер = max(`sqlite_sequence`, max id) + 1;
    - сумма каждого денежного поля в копейках = сумме разобранных исходных значений.
    - Печатает только числа: строки по сущностям, предупреждения по кодам и время импорта на каждой СУБД.
  - `AGENTS.md`, новые правила:
    - импорт MyWork 2 (`mw-legacy`): всё неразобранное — в отчёт и в исходном виде (`legacy_rows`, `details.legacy`); новая форма значения из старой базы — сначала строка в синтетической фикстуре, потом код;
    - реальные базы клиентов не попадают в репозиторий, фикстуры, тесты и логи; фикстуры только генерируются кодом;
    - правка построителей `search_text` или `normalize_search` → увеличить `SEARCH_INDEX_VERSION`;
    - телефоны клиента — список `client_phones`, первый номер основной; гарантия — текст и выводимый срок.
  - `README.md`: статус «импорт MyWork 2 готов (план 3); следующий шаг — дизайн-система (план 4)» и раздел «Импорт из MyWork 2»: `cargo run -p mw-server -- import-legacy <папка MyWork 2>`, `--replace`, где лежат отчёт, копия и вложения.
  - `docs/superpowers/plans/README.md`: строка плана 3 со ссылкой на этот файл и статусом «выполнен», заметки «План 3» убрать. Добавить заметки:
    - **План 5:**
      - экран отчёта импорта (`legacy_imports.report`) и запуск импорта из интерфейса — выбор папки Tauri-командой, долгая операция с прогрессом;
      - таблица `attachments` и регистрация файлов из `attachments/orders/<id>/` по строкам `legacy_rows` (`ClientFiles`);
      - ширины и порядок колонок, `BlistColor`, `colorDiagnostik`, `colorCheckBox`, `Poloski` из `legacy_rows` (`Config.ini`);
      - список телефонов клиента и гарантия «текст + срок» в формах;
      - выбор из нескольких заказов по одному штрихкоду.
    - **План 6:** реквизиты и тексты договоров уже в настройках, в них может остаться HTML MyWork 2; размер штрихкода `[ACTS] BarcodeH/BarcodeW` и шаблоны `Shablon_*.htm` — в `legacy_rows`.
    - **План 8:**
      - склад из `legacy_rows` (`Stock`, `StockMap`, `settings/Stock/*.txt`) и фото `attachments/stock/<id>/` — перенос в таблицы склада в импорт и для уже импортированных баз;
      - `Zatrati` заказов уже включают запчасти из `StockMap` — не задвоить;
      - отчёты по источнику — группировать без учёта регистра;
      - `backupPath`, `EveryDayBackup` из `legacy_rows`.
    - **План 9:** повторный импорт с заменой на PostgreSQL (копия — экспортом в SQLite); импорт на удалённый сервер — загрузка папки или архива.

- [ ] **Step 1: Write `real_db.rs`** — по Interfaces. Без `MW_LEGACY_REAL_DIR` `cargo test -p mw-legacy --test real_db -- --ignored` проходит и ничего не делает.
- [ ] **Step 2: Прогнать на реальной базе.**
  Подготовка: папка вне репозитория (scratchpad сессии) с символической ссылкой `baza.sqlite` на файл автора (путь —
  в `.superpowers/legacy-real-db-findings.md`). Исходный файл не копируется и не меняется: импорт сам копирует его во
  временную папку ОС и удаляет копию.
  Run: `MW_LEGACY_REAL_DIR=<папка> MW_TEST_PG_URL=postgres://postgres:postgres@localhost:55432/postgres cargo test -p mw-legacy --test real_db -- --ignored --nocapture`
  Expected: тест зелёный на обеих СУБД. Числа совпадают с наблюдениями:
  - заказов 8396, удалённых 28, следующий номер 8443;
  - клиентов 5149 + 2 «Клиент не указан» (у двух заказов пуст `ClientId` и нет данных клиента);
  - истории 31623 + 25273;
  - групп дублей штрихкодов 277, `need_call` 4, `need_parts` 10, в чёрном списке 12;
  - статусов 8 + 2 («Куплен» и «Отдан сервисному центру» вида `issued`);
  - `date_unparsed` = 0, `money_unparsed` = 0.
- [ ] **Step 3: Прогнать команду на реальной базе в SQLite.**
  Run: `cargo run --release -p mw-server -- --data-dir <scratch>/data import-legacy <папка>`
  Expected: сводка без персональных данных; время работы записать. Затем удалить `<scratch>/data`: там персональные данные.
- [ ] **Step 4: Если появились неразобранные значения.** Формы значений (`99-99-9999`, без самих данных) снять из JSON-отчёта скриптом в scratchpad, добавить такую же синтетическую строку в фикстуру с ожидаемым результатом в тестах задач 9–11, поправить разбор (сначала тест), повторить шаги 2–3.
- [ ] **Step 5: Записать итог проверки** (только числа и формы) в заметку передачи в папке заметок сессий — вне репозитория.
- [ ] **Step 6: Обновить три документа** — по Interfaces.
- [ ] **Step 7: Финальная проверка**
  Run: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo check --workspace`
  Expected: код 0, без предупреждений.
  Run: `MW_TEST_PG_URL=postgres://postgres:postgres@localhost:55432/postgres cargo test --workspace`
  Expected: `test result: ok` у каждого набора, 0 failed.
  Run: `env -u MW_TEST_PG_URL cargo test --workspace`
  Expected: всё зелёное только на SQLite.
  Run: `MW_UPDATE_CONTRACT=1 cargo test -p mw-api --test contract -- --test-threads=1 && git status --short crates/mw-api`
  Expected: пусто — контракт в репозитории актуален.
  Run: `git status --short` и `git ls-files | grep -Ei '\.(sqlite|db)$'`
  Expected: ни одной базы в git, в рабочем дереве нет лишних файлов.
- [ ] **Step 8: Commit** — `docs: mark plan 3 done, import rules; test: real-folder import check`.

## После плана

Ревью всей ветки, одна волна исправлений и слияние перемоткой в `main` — по тому же правилу, что в плане 2.
Затем `docker stop mw-pg`.
