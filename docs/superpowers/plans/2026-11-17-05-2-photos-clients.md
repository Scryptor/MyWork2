# План 5.2. Фото и клиенты — план реализации

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** у заказа появляются фото и файлы. Их можно снять веб-камерой, выбрать, перетащить или вставить: при
приёме, в панели и в карточке. Файлы старой программы (`ClientFiles`) видны в заказах. Появляется экран «Клиенты»:
- список;
- панель клиента с правкой на месте;
- заказы клиента;
- «Новый заказ»;
- похожие клиенты и их объединение.

**Architecture:**
- Файлы лежат в папке данных, `attachments/`. Записи о них — в новой таблице `attachments`.
- Сервер принимает фото телом запроса `application/octet-stream`: это не «простой» тип для CORS, поэтому защита
  «изменяющие запросы — только не простые типы» сохраняется. Сервер поворачивает фото по EXIF, удаляет метаданные,
  уменьшает до 2560 px по длинной стороне, делает миниатюру. Оригинал не хранится.
- До сохранения заказа фото приёма привязаны к черновику (ключ черновика). `POST /orders` переносит их в заказ.
  Брошенные черновики сервер удаляет через сутки.
- Интерфейс получает файлы через `client.ts` (`Authorization`), показывает через `blob:`-адреса.
- Экран клиентов устроен как экран заказов: список + панель, вид в адресе.

**Tech Stack:** Rust 1.95, Axum 0.8, SeaORM 2. Новые зависимости, все MIT или Apache-2.0:
- `image` 0.25 (фичи `jpeg`, `png`, `webp`, `gif`, без `default-features`);
- `uuid` 1 (`v4`);
- `mime_guess` 2.

Интерфейс — SvelteKit 3 + Svelte 5, Playwright 1.65, новых npm-пакетов нет.

**Spec:**
- [`2026-11-15-ui-design-system-design.md`](../specs/2026-11-15-ui-design-system-design.md) — разделы 6.2 (фото в
  панели), 6.3 (фото и файлы в карточке), 6.5 (фото при приёме), 6.6 (клиенты), 6.12, 9, 11;
- основная спека [`2026-11-11-mywork3-crossplatform-design.md`](../specs/2026-11-11-mywork3-crossplatform-design.md):
  5.4 (фото при приёме и в заказе), «Где лежат данные», 6 (`ClientFiles` → `attachments/orders/<id>/`), 5.0 п. 4
  (клиенты);
- дорожная карта [`README.md`](README.md), «Заметки → План 5.2».

## Global Constraints

- **Ветка:** worktree `/Users/scrypto/dev/MyWork2/.superpowers/worktrees/plan-5-2`, ветка `plan-5-2-photos-clients`.
  Ветку пушить только для CI перед слиянием, после слияния — `git push origin main`.
- **Все правила `AGENTS.md`**, в том числе:
  - сначала тест, тесты с БД — по `all_backends()`;
  - `Money`, `Core::now()`, `mw_domain::search`;
  - запись — только через `Core::begin_write()`;
  - только новые миграции;
  - правка построителей поиска или ключей увеличивает `SEARCH_INDEX_VERSION`;
  - входные типы — `deny_unknown_fields`;
  - после изменения API — `MW_UPDATE_CONTRACT=1 cargo test -p mw-api --test contract -- --test-threads=1`;
  - `fetch` — только в `client.ts`;
  - вид списка — в адресе;
  - автосохранение — через `createAutosave`;
  - Ctrl/⌘+Enter в диалогах — `submitShortcut`.
- **Проверки перед коммитом:**
  - Rust: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo check --workspace && cargo test --workspace`,
    в задачах 1–4 ещё и с `MW_TEST_PG_URL=postgres://postgres:postgres@localhost:55432/postgres`;
  - интерфейс: `cd apps/ui && npm run verify && npm run test:e2e`.
- **Пределы:**
  - файл до 25 МБ (`MAX_ATTACHMENT_BYTES = 25 * 1024 * 1024`);
  - до 10 фото в форме приёма (как в демо);
  - фото — длинная сторона до 2560 px, JPEG качества 85;
  - миниатюра — длинная сторона 320 px, JPEG качества 80;
  - черновик живёт 24 ч.
- **Тестовые картинки** генерируются кодом в тестах (`image`, `ImageBuffer`). EXIF для теста поворота — минимальный
  блок APP1, собранный в тесте байтами. Никаких реальных фото в репозитории.
- **Тексты** — раздел 9 спеки.
- **Камера в e2e:**
  - Chromium запускается с `--use-fake-device-for-media-stream --use-fake-ui-for-media-stream`, это проект
    `chromium` в `playwright.config.ts`;
  - в WebKit поддельной камеры нет, там e2e проверяет запасной путь («Камера недоступна — добавьте фото файлами»);
  - это исключение из правила «e2e в обоих движках» записать в AGENTS.md (задача 10).

## Review Focus

1. **Большие и «неправильные» файлы.** Возможные случаи: снимок телефона 12 МП с EXIF «повёрнут на 90°», HEIC с
   iPhone, PNG с прозрачностью, файл 30 МБ, повреждённый JPEG. Ожидается:
   - повёрнутое — прямо, без GPS в файле;
   - HEIC — понятная ошибка «Формат HEIC не поддерживается: сохраните фото как JPEG»;
   - прозрачное — на белом;
   - больше 25 МБ — ошибка с пределом;
   - битое — ошибка, а не 500.

   Тесты: задача 2, `photo_processing_*`.
2. **Приём отменили или бросили.** Фото черновика не попадают в чужой заказ и не копятся на диске. «Отмена» удаляет
   черновик и его файлы, брошенный удаляется через 24 ч. Повторный `POST /orders` с тем же ключом не создаёт второй
   заказ. Тесты: задача 2, `draft_attachments_*` и `create_order_is_idempotent_by_request_key`; задача 7, e2e «cancel
   removes draft photos».
3. **Объединение клиентов необратимо.** Заказы всех объединяемых переходят к выбранному. Телефоны сливаются без
   дублей, основной — выбранного клиента. Старые данные остаются в журнале. Устаревшая версия любого из клиентов
   даёт 409, и тогда ничего не меняется. Тесты: задача 4, `merge_*`.
4. **Файлы старой программы.** У перенесённой базы фото и видео из `ClientFiles` видны в заказе. Если файл пропал с
   диска, заказ открывается, а у файла пометка «Файл не найден». Тесты: задача 1,
   `legacy_client_files_become_attachments` и `missing_file_reads_as_not_found`; задача 6, e2e «missing legacy file
   shows a note».
5. **Нет камеры или нет разрешения.** На Linux WebKitGTK камеры может не быть, на macOS доступ могут запретить. Блок
   «Фото» тогда говорит, что делать («Камера недоступна — добавьте фото файлами»), и остальное работает. Тесты:
   задача 5, `CameraDialog` в WebKit e2e и модульный тест разбора ошибок `getUserMedia`.

---

## Файловая структура

```
crates/mw-db/src/migration/m20261117_000001_attachments.rs  таблица attachments, orders.request_key, clients.name_key
crates/mw-db/src/entity/attachment.rs
crates/mw-db/src/backfill.rs                                + legacy_client_files(conn): ClientFiles → attachments
crates/mw-core/src/attachments/{mod.rs,store.rs,photo.rs,drafts.rs}
crates/mw-core/src/clients_merge.rs                          похожие, объединение
crates/mw-api/src/routes/attachments.rs
apps/ui/src/lib/api/attachments.ts                           загрузка, blob-адреса, кэш
apps/ui/src/lib/ui/{CameraDialog.svelte,PhotoViewer.svelte,FileList.svelte}  + разделы витрины
apps/ui/src/lib/orders/{OrderPhotos.svelte,OrderFiles.svelte,photos.svelte.ts}
apps/ui/src/lib/intake/IntakePhotos.svelte
apps/ui/src/lib/clients/{view.ts,columns.ts,list.svelte.ts,ClientPanel.svelte,ClientFields.svelte,ClientOrders.svelte,
                         MergeDialog.svelte,merge.ts,similar.ts}
apps/ui/src/routes/(app)/clients/+page.svelte
```

---

### Task 1: Таблица вложений, файлы старой программы, чтение и удаление

**Files:**
- Create: `crates/mw-db/src/migration/m20261117_000001_attachments.rs`, `crates/mw-db/src/entity/attachment.rs`,
  `crates/mw-core/src/attachments/{mod.rs,store.rs}`
- Modify: `crates/mw-db/src/{migration/mod.rs,entity/mod.rs,backfill.rs}`, `crates/mw-core/src/{lib.rs,core.rs}`,
  `crates/mw-core/src/import/write.rs`, `crates/mw-core/Cargo.toml` (`uuid`, `mime_guess`)
- Test: `crates/mw-core/tests/attachments.rs`, `crates/mw-core/tests/import.rs`

**Interfaces:**
- Produces (БД, одна миграция на весь план):
  - `attachments`:
    - `id` (pk);
    - `order_id` (пусто можно, индекс);
    - `draft_key` (текст, пусто можно, индекс);
    - `kind` (`photo` | `file`);
    - `name` (имя файла для человека);
    - `caption` (текст, `''`);
    - `content_type`, `size` (i64);
    - `width`, `height` (пусто можно);
    - `path` (относительно папки данных, например `attachments/orders/10481/<uuid>.jpg`);
    - `thumb_path` (пусто можно);
    - `legacy` (bool);
    - `created_at`;
  - `orders.request_key` (текст, пусто можно, уникальный индекс) — нужен задаче 2;
  - `clients.name_key` (текст, `''`, индекс) — нужен задаче 4;
  - `mw_db::backfill::legacy_client_files(conn) -> Result<u64, DbErr>`:
    - вставляет `attachments` по строкам `legacy_rows` с `source = 'ClientFiles'`, `target_table = 'orders'`;
    - `path` = `raw.stored_as`, `size` = `raw.size`;
    - `kind` = `photo` для расширений jpg/jpeg/png/webp/gif/bmp, иначе `file`;
    - `name` — последний сегмент пути, `legacy = true`, `created_at` = время импорта;
    - повторный запуск ничего не дублирует (по `path`);
    - вызывается из новой миграции и в конце записи импорта.
- Produces (ядро):
  - `Core::with_attachments_dir(self, data_dir: PathBuf) -> Core`. Без папки операции с файлами дают
    `CoreError::Unavailable` с текстом «Хранилище файлов не настроено»;
  - тип API `Attachment`:

    ```rust
    pub struct Attachment {
        pub id: i64,
        pub order_id: Option<i64>,
        pub kind: AttachmentKind,      // Photo | File, serde "photo"/"file"
        pub name: String,
        pub caption: String,
        pub content_type: String,
        pub size: i64,
        pub width: Option<i32>,
        pub height: Option<i32>,
        pub legacy: bool,
        pub missing: bool,             // файла нет на диске
        pub created_at: NaiveDateTime,
    }
    ```

  - методы:
    - `list_order_attachments(actor, order_id) -> Vec<Attachment>` — фото, потом файлы, по `created_at`;
    - `attachment_file(actor, id, variant: AttachmentVariant) -> AttachmentFile { path: PathBuf, content_type, name }`,
      где `variant` — `Full` или `Thumb`;
      - `Thumb` у фото без миниатюры (старые файлы) строит её при первом запросе и запоминает `thumb_path`;
      - у `file` миниатюры нет → `NotFound`;
      - если файла нет на диске → `NotFound`;
    - `update_attachment_caption(actor, id, caption) -> Attachment` — право `orders_edit`, запись журнала
      `attachment.caption`;
    - `delete_attachment(actor, id)` — право `orders_edit`. Файлы удаляются после фиксации транзакции. Запись
      журнала `attachment.deleted` с `order_id` и `{ name, path }` в `details`;
  - `store.rs`:
    - пути — только внутри `<data_dir>/attachments`;
    - имена файлов на диске — `<uuid>.<ext>`, для миниатюры — `<uuid>.thumb.jpg`;
    - функция `resolve(data_dir, rel) -> Option<PathBuf>` отказывает путям с `..`, абсолютным и с `\`.

- [ ] **Step 1: Тесты** (`attachments.rs`; файлы — во временной папке, `cores()`)

```rust
#[tokio::test]
async fn legacy_client_files_become_attachments() -> Result<(), CoreError> {
    // синтетическая фикстура mw_legacy::fixture с ClientFiles (фото .jpg и видео .mp4 у заказа 1) → импорт →
    // list_order_attachments(1) == [photo(legacy, name "photo.jpg"), file("video.mp4")]; повторный backfill не дублирует
}
#[tokio::test]
async fn missing_file_reads_as_not_found() -> Result<(), CoreError> {
    // удалить файл с диска → list показывает missing = true; attachment_file(.., Full) → NotFound
}
#[tokio::test]
async fn legacy_photo_gets_a_thumbnail_on_first_request() -> Result<(), CoreError> {
    // фото 1000×500 из теста; attachment_file(.., Thumb) → файл 320×160; второй запрос не пересоздаёт (mtime тот же)
}
#[tokio::test]
async fn delete_removes_files_after_commit_and_writes_history() -> Result<(), CoreError> { /* … */ }
#[test]
fn resolve_refuses_paths_outside_attachments() { /* "../x", "/etc/x", "attachments\\..\\x" → None */ }
```

- [ ] **Step 2: Запустить — тесты падают**
- [ ] **Step 3: Реализовать: миграция, сущность, `backfill::legacy_client_files`, хранилище, методы, вызов в импорте**
- [ ] **Step 4: Полная проверка Rust на SQLite и PostgreSQL**
- [ ] **Step 5: Commit** — `feat(core): attachments table, legacy client files, read and delete`

---

### Task 2: Загрузка фото и файлов, черновики приёма, ключ повтора приёма

**Files:**
- Create: `crates/mw-core/src/attachments/{photo.rs,drafts.rs}`
- Modify: `crates/mw-core/src/attachments/mod.rs`, `crates/mw-core/src/orders/{create.rs,types.rs}`,
  `crates/mw-core/Cargo.toml` (`image`)
- Test: `crates/mw-core/tests/attachments_upload.rs`, `crates/mw-core/tests/orders_create.rs`

**Interfaces:**
- Consumes: задача 1.
- Produces:
  - `photo.rs`: `process_photo(bytes: &[u8]) -> Result<ProcessedPhoto, PhotoError>`, где
    `ProcessedPhoto { jpeg: Vec<u8>, width: u32, height: u32, thumb: Vec<u8> }`:
    - формат по сигнатуре (`ImageReader::with_guessed_format`);
    - поворот по EXIF: `ImageDecoder::orientation()` + `DynamicImage::apply_orientation`;
    - прозрачность — на белом фоне;
    - длинная сторона > 2560 — уменьшить до 2560 (`FilterType::Triangle`);
    - кодирование JPEG 85 — метаданных в выходе нет;
    - миниатюра 320 px, JPEG 80;
    - `PhotoError::Unsupported { format: Option<String> }` (HEIC и прочее), `PhotoError::Broken`;
  - `Core::add_order_attachment(actor, order_id, name: &str, bytes: Vec<u8>) -> Attachment`:
    - право `orders_edit`;
    - больше `MAX_ATTACHMENT_BYTES` → ошибка проверки: поле `file`, код `too_large`, «Файл больше 25 МБ»;
    - распознанная картинка → `photo` через `process_photo`, расширение `.jpg`, тип `image/jpeg`;
    - нераспознанные байты → `file` как есть, тип — `mime_guess` по имени, иначе `application/octet-stream`;
    - HEIC (сигнатура `ftypheic`/`ftypheix`/`ftypmif1`) → поле `file`, код `unsupported`, «Формат HEIC не
      поддерживается: сохраните фото как JPEG»;
    - битая картинка с расширением картинки → поле `file`, код `broken`, «Не удалось прочитать картинку»;
    - обработка — в `tokio::task::spawn_blocking`;
    - файл пишется до записи в БД. Если запись в БД не удалась, файл удаляется;
    - журнал `attachment.added`;
  - `Core::add_draft_attachment(actor, draft_key: &str, name, bytes) -> Attachment`:
    - право `orders_create`;
    - `draft_key` — 1–64 символов `[A-Za-z0-9-]`, иначе поле `draft_key` / `invalid`;
    - файлы — в `attachments/drafts/<draft_key>/`;
    - `order_id` пустой; не больше 10 вложений на черновик (поле `file`, `too_many`, «В черновике уже 10 фото»);
  - `Core::list_draft_attachments(actor, draft_key) -> Vec<Attachment>`, `Core::discard_draft(actor, draft_key)`;
  - `NewOrder` получает:
    - `attachment_draft: Option<String>` — в той же транзакции вложения черновика получают `order_id`, `draft_key`
      пустеет; файлы переносятся в `attachments/orders/<id>/` после фиксации (неудача переноса — в журнал, вложение
      остаётся по старому пути);
    - `request_key: Option<String>` (1–64 `[A-Za-z0-9-]`):
      - повтор с тем же ключом возвращает уже созданный заказ (тот же `id`), без второго заказа и без записи в журнал;
      - тот же ключ с другим клиентом — всё равно тот же заказ: ключ определяет попытку приёма;
  - `Core::cleanup_drafts(older_than: chrono::Duration) -> Result<u64, CoreError>` — удаляет записи и папки
    черновиков старше срока. Вызывает сервер, задача 3.

- [ ] **Step 1: Тесты**

```rust
#[tokio::test]
async fn photo_processing_rotates_by_exif_and_strips_metadata() {
    // JPEG 400×200 с APP1 Orientation=6 (собран в тесте) → ProcessedPhoto 200×400; в выходе нет "Exif"
}
#[tokio::test]
async fn photo_processing_scales_down_and_makes_thumb() { /* 6000×3000 → 2560×1280; thumb 320×160 */ }
#[tokio::test]
async fn photo_processing_flattens_transparency_on_white() { /* PNG 2×2 прозрачный → пиксель (255,255,255) */ }
#[tokio::test]
async fn photo_processing_refuses_heic_and_broken_files() -> Result<(), CoreError> {
    // b"\0\0\0\x18ftypheic..." → validation ("file","unsupported"); "x.jpg" с мусором → ("file","broken")
}
#[tokio::test]
async fn too_large_upload_is_refused() -> Result<(), CoreError> { /* 25 МБ + 1 байт → ("file","too_large") */ }
#[tokio::test]
async fn draft_attachments_move_to_the_created_order() -> Result<(), CoreError> {
    // add_draft_attachment("k1", фото) ×2 → create_order(attachment_draft: "k1") → list_order_attachments(id).len() == 2,
    // файлы лежат в attachments/orders/<id>/, папки drafts/k1 нет
}
#[tokio::test]
async fn draft_attachments_are_discarded_and_cleaned() -> Result<(), CoreError> {
    // discard_draft("k2") → файлов и записей нет; черновик старше 24 ч (clock.advance) → cleanup_drafts(24 h) == 1
}
#[tokio::test]
async fn create_order_is_idempotent_by_request_key() -> Result<(), CoreError> {
    // create_order(request_key "r1") дважды → один id; count_audit("order.created") == 1
}
```

- [ ] **Step 2: Запустить — тесты падают**
- [ ] **Step 3: Реализовать**
- [ ] **Step 4: Полная проверка Rust на SQLite и PostgreSQL**
- [ ] **Step 5: Commit** — `feat(core): photo processing, uploads, intake drafts, idempotent intake`

---

### Task 3: API вложений и сервер

**Files:**
- Create: `crates/mw-api/src/routes/attachments.rs`
- Modify: `crates/mw-api/src/{lib.rs,middleware.rs,routes/mod.rs,dto.rs}`, `crates/mw-server/src/{server.rs,config.rs}`
- Test: `crates/mw-api/tests/attachments.rs`, тесты `mw-server`

**Interfaces:**
- Consumes: задачи 1–2.
- Produces (маршруты):

| Метод и путь | Тело / ответ | Право |
|---|---|---|
| `GET /api/v1/orders/{id}/attachments` | → `Attachment[]` | вход |
| `POST /api/v1/orders/{id}/attachments?name=` | байты `application/octet-stream` → `Attachment` | `orders_edit` |
| `POST /api/v1/intake-drafts/{key}/attachments?name=` | байты → `Attachment` | `orders_create` |
| `GET /api/v1/intake-drafts/{key}/attachments` | → `Attachment[]` | `orders_create` |
| `DELETE /api/v1/intake-drafts/{key}` | → 204 | `orders_create` |
| `GET /api/v1/attachments/{id}/file` и `/thumb` | файл | вход |
| `PATCH /api/v1/attachments/{id}` | `{ caption }` (`deny_unknown_fields`) → `Attachment` | `orders_edit` |
| `DELETE /api/v1/attachments/{id}` | → 204 | `orders_edit` |

  - `require_json_content_type` пропускает `application/octet-stream` только для двух маршрутов загрузки.
    `multipart/form-data`, `text/plain` и `application/x-www-form-urlencoded` по-прежнему 415;
  - предел тела у маршрутов загрузки — `MAX_ATTACHMENT_BYTES + 64 КБ`, у остальных прежний (1 МБ);
  - отдача файла:
    - `Content-Type` из записи;
    - `Content-Disposition`: `inline` для `image/*`, иначе `attachment`; имя файла — RFC 5987 (`filename*=UTF-8''…`);
    - `Cache-Control: private, max-age=31536000, immutable` — файл под данным `id` не меняется;
    - `Content-Security-Policy: sandbox; default-src 'none'` и прочие заголовки безопасности;
  - `NewOrder.attachment_draft` и `NewOrder.request_key` есть в API;
  - `mw-server`:
    - `Core::with_attachments_dir(<папка данных>)`;
    - при запуске и раз в час — `cleanup_drafts(24 h)` в фоне, ошибки — в лог.

- [ ] **Step 1: Тесты API** (`apis(...)`): загрузка фото → `GET thumb` отдаёт `image/jpeg`;
  - загрузка с `Content-Type: multipart/form-data` → 415;
  - `text/plain` → 415;
  - 26 МБ → 413 или 422 `too_large`;
  - `GET file` у PDF → `Content-Disposition: attachment; filename*=UTF-8''%D0%B0%D0%BA%D1%82.pdf` для «акт.pdf»;
  - `PATCH` с лишним полем → 400;
  - черновик → `POST /orders` с `attachment_draft` → вложения в заказе;
  - повтор `POST /orders` с тем же `request_key` → тот же `id`.
- [ ] **Step 2: Запустить — тесты падают**
- [ ] **Step 3: Реализовать; обновить контракт**
- [ ] **Step 4: Полная проверка Rust на SQLite и PostgreSQL**
- [ ] **Step 5: Commit** — `feat(api,server): attachment routes, uploads, draft cleanup`

---

### Task 4: Сервер: список клиентов, похожие, объединение

**Files:**
- Create: `crates/mw-core/src/clients_merge.rs`
- Modify: `crates/mw-core/src/{clients.rs,search_index.rs,lib.rs}`, `crates/mw-api/src/routes/clients.rs`, `crates/mw-api/src/dto.rs`
- Test: `crates/mw-core/tests/clients.rs`, `crates/mw-core/tests/clients_merge.rs`, `crates/mw-api/tests/clients.rs`

**Interfaces:**
- Produces:
  - `clients.name_key` = `normalize_search(full_name)` — пишется при создании и правке и заполняется построителем
    индекса; `SEARCH_INDEX_VERSION` = 4;
  - `ClientListItem`:

    ```rust
    pub struct ClientListItem {
        pub id: i64, pub full_name: String, pub phones: Vec<ClientPhone>, pub source: String,
        pub blacklisted: bool, pub orders_count: u64, pub last_order_at: Option<NaiveDateTime>,
    }
    ```

    `ClientPage { items: Vec<ClientListItem>, total }`. Палитра и приём уже пользуются `listClients` — их тип
    обновить;
  - `ClientListQuery` получает:
    - `sort: ClientSortField` = `name | last_order | orders_count`, по умолчанию `last_order` убыв., пустые в конце;
    - `descending: bool`;
    - `duplicates: bool` — только клиенты, у которых есть похожий; сортировка тогда по `name_key`, чтобы группы шли
      подряд;
  - «похожий» = тот же непустой `name_key` или общий телефон: сравниваются последние 10 цифр, номера короче 6 цифр
    не сравниваются. `Core::similar_clients(actor, id) -> Vec<ClientListItem>` →
    `GET /api/v1/clients/{id}/similar`;
  - `Core::merge_clients(actor, m: MergeClients) -> Client`, право `clients_merge`:

    ```rust
    pub struct MergeClients {           // deny_unknown_fields
        pub target_id: i64,
        pub target_version: i64,
        pub sources: Vec<VersionedId>,  // { id, version }, 1..=10, без target и повторов
        pub full_name: String, pub address: String, pub note: String, pub source: String, pub blacklisted: bool,
    }
    ```

    - одна транзакция;
    - любая версия устарела → 409, ничего не меняется;
    - телефоны: сначала телефоны цели, потом остальных в порядке `sources`, без повторов по цифрам; пометки
      сохраняются;
    - заказы источников → `client_id = target_id`;
    - источники удаляются;
    - журнал `client.merged` у цели: `details` = снимки удалённых клиентов целиком, с телефонами;
    - маршрут `POST /api/v1/clients/merge` → `Client`.

- [ ] **Step 1: Тесты**

```rust
#[tokio::test]
async fn list_items_carry_order_counts_and_sort_by_last_order() -> Result<(), CoreError> { /* … */ }
#[tokio::test]
async fn duplicates_filter_groups_same_name_and_shared_phone() -> Result<(), CoreError> {
    // «ИВАНОВ ИВАН» и «Иванов Иван» (name_key равны), «Петров» и «Сидоров» с общим 8 921 123-45-67 → четыре в
    // duplicates, «Ким» без пары — нет; similar_clients(иванов1) == [иванов2]
}
#[tokio::test]
async fn merge_moves_orders_merges_phones_and_keeps_snapshots() -> Result<(), CoreError> {
    // цель A (тел. 1), источник B (тел. 1 и 2, 2 заказа) → у A телефоны [1, 2], 3 заказа; B нет; audit client.merged с B
}
#[tokio::test]
async fn merge_with_a_stale_version_changes_nothing() -> Result<(), CoreError> { /* версия B −1 → Conflict; B на месте */ }
#[tokio::test]
async fn merge_needs_clients_merge_permission() -> Result<(), CoreError> { /* actor_with(ClientsEdit) → Forbidden */ }
```

- [ ] **Step 2: Запустить — тесты падают**
- [ ] **Step 3: Реализовать; обновить контракт**
- [ ] **Step 4: Полная проверка Rust на SQLite и PostgreSQL**
- [ ] **Step 5: Commit** — `feat(core,api): client list counts, duplicates, similar and merge`

---

### Task 5: Интерфейс: клиент вложений, камера, просмотр фото, список файлов

**Files:**
- Create:
  - `apps/ui/src/lib/api/attachments.ts`;
  - `apps/ui/src/lib/ui/{CameraDialog.svelte,PhotoViewer.svelte,FileList.svelte,camera.ts}` с тестами к `.ts`;
  - разделы витрины в `src/routes/design/sections/Media.svelte`;
  - `apps/ui/e2e/media.spec.ts`.
- Modify: `apps/ui/src/lib/api/client.ts` (`upload`, `requestBlob`), `apps/ui/src/lib/ui/index.ts`,
  `apps/ui/playwright.config.ts` (флаги поддельной камеры для `chromium`), `ru.ts`

**Interfaces:**
- Produces:
  - `client.ts`:
    - `upload<T>(path, body: Blob, query?)` — `POST`, `Content-Type: application/octet-stream`; ошибки — как у `request`;
    - `requestBlob(path, signal?) -> Promise<Blob>`;
  - `attachments.ts`:
    - `listOrderAttachments(orderId)`, `uploadOrderAttachment(orderId, file: File)`,
      `uploadDraftAttachment(key, file)`, `listDraftAttachments(key)`, `discardDraft(key)`,
      `updateAttachmentCaption(id, caption)`, `deleteAttachment(id)`;
    - `attachmentUrl(id, variant: 'thumb' | 'file'): Promise<string>` — `blob:`-адрес из кэша (до 200 миниатюр,
      вытеснение по давности, `URL.revokeObjectURL` при вытеснении);
    - `downloadAttachment(a: Attachment)` — скачивание через `<a download>`;
  - `camera.ts`: `cameraErrorText(e: unknown): string`:
    - `NotAllowedError` → «Нет доступа к камере. Разрешите его в настройках системы или добавьте фото файлами»;
    - `NotFoundError`, `OverconstrainedError` и отсутствие `navigator.mediaDevices` → «Камера недоступна — добавьте
      фото файлами»;
    - прочее → «Камера не запустилась — добавьте фото файлами»;
  - `CameraDialog`:
    - пропсы `open` (bindable), `max` (сколько ещё можно снять), `oncapture(file: File)`;
    - живое превью и выбор камеры (`enumerateDevices`, если камер больше одной);
    - «Снять» (пробел) — кадр в JPEG 0.9 с именем `camera-<время>.jpg`, сразу в `oncapture`; можно несколько подряд;
    - «Готово» закрывает и останавливает поток (все дорожки `stop()`), ошибки — `cameraErrorText`;
  - `PhotoViewer`:
    - пропсы `photos: Attachment[]`, `index` (bindable), `open` (bindable), `oncaption(id, text)`, `ondelete(id)`,
      `readonly`;
    - на весь экран; ← → листают, Esc закрывает;
    - подпись правится на месте: сохраняется при уходе из поля и Enter;
    - «Удалить» — без подтверждения, с уведомлением «Фото удалено» (удаление записано в журнал);
  - `FileList`:
    - пропсы `files: Attachment[]`, `ondownload`, `ondelete`, `readonly`;
    - имя, размер (`formatBytes` в `#lib/format/bytes.ts`: «1,2 МБ»), дата, пометка «Файл не найден» у `missing`.

- [ ] **Step 1: Модульные тесты** — `cameraErrorText` для трёх видов ошибок; кэш `attachmentUrl`: второй запрос без
  `requestBlob`, вытеснение 201-го отзывает адрес; `formatBytes(1_234_567) == '1,2 МБ'`, `formatBytes(900) == '900 Б'`.
- [ ] **Step 2: Сквозные тесты** (витрина):
  - Chromium: открыть камеру → превью играет → «Снять» дважды → `oncapture` вызван 2 раза (счётчик на образце);
  - WebKit: камера → текст «Камера недоступна — добавьте фото файлами» (или «Нет доступа…»), кнопка «Закрыть»
    работает;
  - просмотр фото: → листает, Esc закрывает, подпись сохраняется по Enter.
- [ ] **Step 3: Запустить — тесты падают**
- [ ] **Step 4: Реализовать**
- [ ] **Step 5: Проверка** `npm run verify && npm run test:e2e`
- [ ] **Step 6: Commit** — `feat(ui): attachments client, camera dialog, photo viewer, file list`

---

### Task 6: Фото и файлы в панели и карточке заказа

**Files:**
- Create: `apps/ui/src/lib/orders/{OrderPhotos.svelte,OrderFiles.svelte,photos.svelte.ts}`, `photos.test.ts`,
  `apps/ui/e2e/order-photos.spec.ts`
- Modify: `apps/ui/src/lib/orders/{OrderPanel.svelte,OrderFields.svelte}`, `apps/ui/src/routes/(app)/orders/[id]/+page.svelte`,
  `apps/ui/e2e/support/api.ts` (`uploadPhoto(request, orderId, bytes)`), `ru.ts`

**Interfaces:**
- Consumes: задачи 3, 5.
- Produces:
  - `photos.svelte.ts`: `createOrderMedia(orderId: number)` → `{ photos, files, loading, error, add(files: File[]), caption(id, text), remove(id), reload() }`:
    - загрузки идут по одной;
    - у каждой плитка «Загружаю…»;
    - ошибка сервера (`too_large`, `unsupported`, `broken`) — уведомление с текстом сервера и именем файла;
    - сеть — «Не удалось загрузить <имя>: нет связи с сервером. Повторить»;
  - `OrderPhotos`:
    - `PhotoStrip` + `PhotoViewer`;
    - «＋» открывает меню «Снять камерой» / «Выбрать файлы»;
    - перетаскивание и вставка из буфера (`FileDrop`) на блок;
    - не больше 10 фото на одно добавление;
  - в панели — раздел «Фото» под «Комментарием»;
  - в карточке — боковая колонка «Фото» и «Файлы» (`OrderFiles` = `FileList` + «Добавить файл»); на узком экране —
    вкладка «Файлы»;
  - на телефоне в нижней полосе панели — «📷 Фото»: `input type=file accept="image/*" capture="environment"`;
  - у выданного заказа фото и файлы добавлять и удалять можно (это не статус).

- [ ] **Step 1: Модульные тесты** `photos.test.ts`: загрузки по одной; ошибка `too_large` → уведомление с именем файла,
  остальные файлы грузятся; `remove` убирает плитку сразу и возвращает при ошибке.
- [ ] **Step 2: Сквозные тесты** (`order-photos.spec.ts`, оба движка, фото — PNG, собранный в тесте):
  - добавить два фото файлом → две плитки; перезагрузка → на месте;
  - открыть просмотр → подпись «царапина» → Enter → в API `caption = 'царапина'`;
  - удалить → плитки нет, уведомление «Фото удалено»;
  - в карточке «Файлы»: загрузить PDF (байты из теста) → строка с именем, «Скачать» отдаёт файл (`page.waitForEvent('download')`);
  - missing legacy file shows a note: вложение старой программы, файл которого удалён на сервере (подготовка через
    `mw-server import-legacy` в `server.mjs` не нужна — `page.route` отдаёт `Attachment` с `missing: true`) →
    «Файл не найден»;
  - Chromium: «Снять камерой» → «Снять» → «Готово» → плитка появилась.
- [ ] **Step 3: Запустить — тесты падают**
- [ ] **Step 4: Реализовать**
- [ ] **Step 5: Проверка** `npm run verify && npm run test:e2e`
- [ ] **Step 6: Commit** — `feat(ui): photos and files in the order panel and card`

---

### Task 7: Фото в приёме и доводка приёма

**Files:**
- Create: `apps/ui/src/lib/intake/IntakePhotos.svelte`
- Modify: `apps/ui/src/lib/intake/{IntakeSheet.svelte,ClientStep.svelte,draft.ts,intake.svelte.ts}`, `ru.ts`
- Test: `draft.test.ts`, `intake.test.ts`, `apps/ui/e2e/intake.spec.ts`

**Interfaces:**
- Consumes: задачи 3, 5.
- Produces:
  - у черновика есть `key: string` (UUID, `crypto.randomUUID()`). Он создаётся вместе с черновиком и хранится с ним;
  - блок «Фото» (раздел 6.5):
    - «Снять камерой» (`CameraDialog`) и «Выбрать файлы»;
    - перетаскивание и вставка;
    - до 10 фото;
    - загрузка сразу в черновик (`uploadDraftAttachment`);
    - при открытии листа фото черновика читаются (`listDraftAttachments`);
  - «Снять телефоном»:
    - кнопка видна всегда;
    - в локальном режиме (сервер на loopback, `ServerInfo` подсказывает, что сеть закрыта) открывает пояснение:
      «Телефон подключается к компьютеру по сети. Включите режим хоста в настройках «Сеть» — он появится в
      следующих версиях»;
    - QR и съёмка — план 10;
  - `toNewOrder` передаёт `attachment_draft: key` и `request_key: key`. Повторное «Принять» после таймаута получает
    тот же заказ, а не новый. Заметка про ключ повтора из дорожной карты закрыта;
  - «Отмена» вызывает `discardDraft(key)` (ошибка сети не мешает закрыть, черновик сервер удалит сам через сутки);
  - после успешного приёма — новый `key` для следующего черновика;
  - доводка из заметок:
    - карточки найденных клиентов очищаются, как только телефон изменился;
    - `submitError` сбрасывается при любой правке;
    - пометка «уже принят» держится при правке ФИО, телефона или устройства и снимается только «Отменой», успешным
      «Принять» или выбором другого клиента. Вместо `sameOrder` — сравнение `request_key`: пометка привязана к
      ключу, а ключ меняется только с новым черновиком;
    - приём с `prefill` другого клиента заменяет сохранённый черновик сразу при открытии, если тот был помечен «уже
      принят».

- [ ] **Step 1: Модульные тесты** `draft.test.ts`:
  - `toNewOrder` содержит `attachment_draft` и `request_key` = `key`;
  - `parseDraft` старого вида (без `key`) даёт новый `key`;
  - пометка «уже принят» держится при правке ФИО.
- [ ] **Step 2: Сквозные тесты** (`intake.spec.ts`):
  - фото файлом в приёме → «Принять» → у созданного заказа 1 фото;
  - cancel removes draft photos: фото → «Отмена» → `GET /intake-drafts/<key>/attachments` пусто;
  - повтор после таймаута: первый `POST /orders` задержан дольше 30 с через `page.route` (ответ потом пропускается) →
    «Нет ответа» → «Принять» снова → заказ один (по API, по метке);
  - «Снять телефоном» → пояснение про режим хоста;
  - карточки клиента исчезают при правке телефона.
- [ ] **Step 3: Запустить — тесты падают**
- [ ] **Step 4: Реализовать**
- [ ] **Step 5: Проверка** `npm run verify && npm run test:e2e`
- [ ] **Step 6: Commit** — `feat(ui): intake photos, idempotent accept, intake polish`

---

### Task 8: Экран «Клиенты»: список и панель

**Files:**
- Create: `apps/ui/src/routes/(app)/clients/+page.svelte`,
  `apps/ui/src/lib/clients/{view.ts,columns.ts,list.svelte.ts,ClientPanel.svelte,ClientFields.svelte,ClientOrders.svelte,editor.svelte.ts}`
  и тесты к `.ts`, `apps/ui/src/lib/api/clients.ts` (дополнить), `apps/ui/e2e/clients.spec.ts`
- Modify: `apps/ui/src/lib/app/{AppSidebar.svelte,palette.ts}`, `apps/ui/src/routes/(app)/+layout.svelte`,
  `apps/ui/src/lib/orders/OrderFields.svelte` («открыть» у клиента), `ru.ts`

**Interfaces:**
- Consumes: задача 4; `createAutosave`; `orderPanel.open(id, { back })`; `intake.open({ clientId })`.
- Produces:
  - `clients.ts` (API):
    - `listClients(q: ClientListQuery)` (тип `ClientPage` новый);
    - `getClient`, `updateClient(id, version, patch: ClientPatch)`, `similarClients(id)`, `mergeClients(m: MergeClients)`;
  - `view.ts`:
    - `ClientsView { query: string; sort: SortState; duplicates: boolean }`;
    - `clientsViewFromSearch` / `clientsViewToSearch`, параметры `q`, `sort`, `desc`, `dup`;
  - `columns.ts`:
    - колонки `name` (ФИО, `displayName`), `phones` (основной + «＋N»), `orders` (число), `last` (последний заказ,
      `formatRelative`), `source`, `flags` (чёрный список);
    - сортируются `name`, `orders`, `last`;
  - `editor.svelte.ts`:
    - одна очередь записи на клиента;
    - `createAutosave` для текста (600 мс), выбор сразу;
    - 409 → свежая версия, `mergeRemote` и `Callout`, как у заказа;
  - `/clients`:
    - строка поиска (`/`), фишки «Все» / «Похожие» (`duplicates`);
    - `DataTable`, на телефоне — `ListCard`;
    - клик или Enter → `ClientPanel`;
    - выделение флажками → в панели выделения «Объединить» (задача 9);
  - `ClientPanel` (`SlideOver`, раздел 6.6):
    - ФИО;
    - телефоны: правка на месте, у каждого пометка, «＋ номер», «Сделать основным» (переносит в начало), удаление;
    - адрес, «откуда узнали» (`Combobox` по `client_source`), заметка, переключатель «Чёрный список»;
    - «Заказы клиента» — `listOrders({ client_id, limit: 50 })`: строки № · дата · устройство · статус. Клик →
      `orderPanel.open(id, { back: { label: '← к клиенту', href: '/clients?client=<id>' } })`;
    - действия: «Новый заказ» (`intake.open({ clientId })`), «Объединить с…» (задача 9);
    - подсказка «Похожие клиенты: N» → список похожих (`similarClients`), клик открывает похожего;
  - в меню слева появляется «Клиенты». Палитра: клиент → `/clients?client=<id>`. В панели заказа «открыть» у клиента
    → `/clients?client=<id>`;
  - `?client=<id>` в адресе открывает панель клиента.

- [ ] **Step 1: Модульные тесты** — `view` туда-обратно через адрес; колонки и сопоставление сортировки;
  `editor` — очередь и конфликт (подделка сервера с версиями, как в `orders/editor.test.ts`).
- [ ] **Step 2: Сквозные тесты** (`clients.spec.ts`):
  - список ищет по телефону;
  - сортировка по числу заказов;
  - панель: правка ФИО сохраняется сама;
  - «＋ номер» с пометкой «жена» → в API у клиента два телефона;
  - «Сделать основным» второго → он первый;
  - «Заказы клиента» → клик → панель заказа с «← к клиенту» → назад к клиенту;
  - «Новый заказ» → лист приёма с этим клиентом;
  - «Клиенты» в меню; на телефоне — карточки.
- [ ] **Step 3: Запустить — тесты падают**
- [ ] **Step 4: Реализовать**
- [ ] **Step 5: Проверка** `npm run verify && npm run test:e2e`
- [ ] **Step 6: Commit** — `feat(ui): clients screen with panel, phones, orders and new order`

---

### Task 9: Похожие клиенты и объединение

**Files:**
- Create: `apps/ui/src/lib/clients/{MergeDialog.svelte,merge.ts}`, `merge.test.ts`, `apps/ui/e2e/clients-merge.spec.ts`
- Modify: `apps/ui/src/routes/(app)/clients/+page.svelte`, `ClientPanel.svelte`, `ru.ts`

**Interfaces:**
- Consumes: задачи 4, 8.
- Produces:
  - `merge.ts`:
    - `defaultChoice(clients: Client[]): MergeChoice` — цель — клиент с большим числом заказов, при равенстве —
      меньший `id`. По каждому полю (`full_name`, `address`, `note`, `source`, `blacklisted`) — значение цели, если
      оно не пустое, иначе первое непустое. `blacklisted` — true, если у кого-то true;
    - `toMergeRequest(choice, clients): MergeClients`;
    - `mergedPhones(clients, targetId): ClientPhone[]` — для предпросмотра, тот же порядок, что на сервере;
  - `MergeDialog` (размер `lg`):
    - колонка на каждого клиента: ФИО, телефоны, адрес, источник, заметка, чёрный список, число заказов;
    - выбор цели — «Оставить этого» в шапке колонки;
    - по каждому полю — переключатель «взять отсюда»;
    - внизу итог: «Останется: <ФИО> · N заказов · телефоны …»;
    - «Объединить» открывает `ConfirmDialog` «Объединить N клиентов в одного? Это нельзя отменить»;
    - после успеха — уведомление «Клиенты объединены», выделение снимается, открывается панель итогового клиента;
    - 409 — «Клиента изменили, пока вы выбирали. Проверьте и объедините снова» с перечитанными данными;
  - вход в объединение:
    - из списка — 2–10 выделенных → «Объединить»;
    - из панели — «Объединить с…»: поиск клиента (`Combobox` по `listClients`) → диалог с двумя;
    - из «Похожих» — «Объединить с этим»;
  - без права `clients_merge` кнопок нет.

- [ ] **Step 1: Модульные тесты** `merge.test.ts` — выбор цели по числу заказов; первое непустое поле; чёрный список
  «или»; порядок телефонов без повторов по цифрам.
- [ ] **Step 2: Сквозные тесты** (`clients-merge.spec.ts`):
  - два клиента «ИВАНОВ ИВАН <метка>» и «Иванов Иван <метка>» с общим телефоном и по заказу → фишка «Похожие» →
    оба в списке → выделить → «Объединить» → у второго выбрать адрес → «Объединить» → подтвердить → один клиент с
    двумя заказами и адресом второго (по API);
  - устаревшая версия (между открытием диалога и подтверждением `PATCH` второго клиента через API) → сообщение,
    данные перечитаны.
- [ ] **Step 3: Запустить — тесты падают**
- [ ] **Step 4: Реализовать**
- [ ] **Step 5: Проверка** `npm run verify && npm run test:e2e`
- [ ] **Step 6: Commit** — `feat(ui): similar clients and merge`

---

### Task 10: Правила, документы, финальная проверка

**Files:**
- Modify: `AGENTS.md`, `CONTRIBUTING.md`, `docs/superpowers/specs/2026-11-15-ui-design-system-design.md` (раздел 11 —
  решения ниже, раздел 12), `docs/superpowers/specs/2026-11-11-mywork3-crossplatform-design.md` (5.4 — что сделано,
  что осталось в плане 10), `docs/superpowers/plans/README.md`

- [ ] **Step 1: `AGENTS.md`:**
  - вложения:
    - файлы — только через `mw-core::attachments`, пути — через `store::resolve`;
    - загрузка — телом `application/octet-stream`; `multipart/form-data` и прочие «простые» типы — нет: это защита от
      CSRF;
  - картинки в интерфейсе — только через `attachmentUrl` (`blob:`), не прямыми ссылками на API;
  - камера в e2e — только Chromium; в WebKit проверяется запасной путь.
- [ ] **Step 2: Спека интерфейса, раздел 11 — решения:**
  - загрузка телом `octet-stream`;
  - фото — JPEG, длинная сторона до 2560 px, миниатюра 320 px; оригинал не хранится; старые файлы — как есть,
    миниатюра при первом запросе;
  - черновик приёма — ключ UUID, он же ключ повтора `POST /orders`;
  - удаление фото без подтверждения (есть журнал);
  - похожие — по `name_key` или последним 10 цифрам телефона;
  - объединение — цель с большим числом заказов по умолчанию, снимки в журнале;
  - «Снять телефоном» — пояснение до режима хоста.
- [ ] **Step 3: Дорожная карта:** строка 5.2 — «выполнен»; заметки 5.2 закрыть или перенести; новые заметки
  дописать.
- [ ] **Step 4: Финальная проверка** — Rust на SQLite и PostgreSQL, `npm run verify && npm run test:e2e`, всё зелёное.
- [ ] **Step 5: Commit** — `docs: plan 5.2 done; attachments and clients rules`

---

## После плана

- Итоговое ревью всей ветки (`opus`), одна волна исправлений.
- CI на GitHub для ветки, затем перемотка `main`, `git push origin main`, удаление worktree и ветки.
- Доложить автору: что работает, «Решения, которые я принял», что дальше (5.3).
