# MyWork2

Рабочее название проекта. Программа учёта заказов для ремонтной мастерской: переписывание старой Windows-программы на Rust, кроссплатформенно.

**Статус:** дизайн-система интерфейса готова (план 4); следующий шаг — десктоп и основные экраны (план 5).

- [Спецификация](docs/superpowers/specs/2026-11-11-mywork3-crossplatform-design.md)
- [Дорожная карта и планы](docs/superpowers/plans/README.md)
- [Разбор старой программы](docs/legacy/mywork2-analysis.md)
- [Репозиторий на GitHub](https://github.com/Scryptor/MyWork2) — issues и pull request'ы
- [Спецификация интерфейса](docs/superpowers/specs/2026-11-15-ui-design-system-design.md)
- [`legacy/`](legacy/) — исходники старой версии на C#, только для чтения

## Запуск сервера

Создать администратора и запустить сервер:

```bash
cargo run -p mw-server -- create-admin --login admin
cargo run -p mw-server
```

Настройки — переменные окружения:

- `MW_LISTEN` — адрес, по умолчанию `127.0.0.1:8080` (режим без входа — только на локальном адресе (loopback));
- `MW_DATA_DIR` — папка данных (по умолчанию `MyWork3` в папке данных ОС или `data` рядом с программой при наличии `portable.txt`);
- `MW_DATABASE_URL` — база; по умолчанию SQLite в папке данных;
- `MW_TIMEZONE` — часовой пояс предприятия при первом запуске (имя IANA, например Europe/Moscow; дальше меняется в настройках программы);
- `MW_LOG` — уровень логов, по умолчанию `info,sqlx=warn`.

API лежит под `/api/v1`, описание — `/api/v1/openapi.json`.

## Импорт из MyWork 2

Перенос данных из папки старой программы (с `baza.sqlite`, `Config.ini`, файлами клиентов) в пустую базу MyWork 3:

```bash
cargo run -p mw-server -- import-legacy <папка MyWork 2>
cargo run -p mw-server -- import-legacy <папка MyWork 2> --replace
```

Перед импортом закройте MyWork 2: база и её журнал копируются раздельно, и при работающей программе копия может оказаться неполной. Исходная папка только читается. Без `--replace` база должна быть пустой; `--replace` (только SQLite) сначала снимает копию и заменяет данные.

- полные отчёты (`.txt` и `.json`) — `<папка данных>/import-reports/`; **в них есть данные клиентов** (телефоны, логины, значения полей), храните их в тайне; в консоль выводятся только числа;
- копия базы и вложений перед заменой — `<папка данных>/backups/`;
- файлы клиентов — `<папка данных>/attachments/`.

Если в MyWork 2 не было пользователей, программа остаётся в режиме без пароля на этом компьютере.

## Интерфейс

Нужен Node ≥ 22.17.

```bash
cd apps/ui
npm ci
npx playwright install chromium webkit   # один раз, для сквозных тестов
npm run dev                              # http://localhost:5173/design
```

- `npm run verify` — svelte-check, Prettier, ESLint, Vitest, сборка и проверка лицензий;
- `npm run test:e2e` — сквозные тесты Playwright (Chromium и WebKit).

## Запуск программы для компьютера

Программа для компьютера (`apps/desktop`, Tauri 2) поднимает встроенный сервер на `127.0.0.1` и открывает окно на его
адресе; интерфейс тот же, что в браузере, и говорит с сервером по HTTP. Через Tauri идут только выбор папки и события
окна.

Нужны Rust, Node ≥ 22.17 и то, на чём Tauri рисует окно: на macOS — Xcode Command Line Tools, на Windows — WebView2
(есть в Windows 10 и 11), на Linux — `libwebkit2gtk-4.1-dev libsoup-3.0-dev librsvg2-dev libayatana-appindicator3-dev`.

```bash
cd apps/ui && npm ci                     # интерфейс входит в программу ресурсом
cd ../desktop && npm ci
npm run tauri dev                        # собирает интерфейс и открывает окно
npm run tauri build -- --debug           # пакет; на macOS — src-tauri/target/debug/bundle/macos/MyWork.app
```

- Данные — в папке данных ОС (`~/Library/Application Support/MyWork3`, `%APPDATA%\MyWork3`, `~/.local/share/MyWork3`),
  при `portable.txt` рядом с исполняемым файлом — в `data` рядом с ним; журнал — `<папка данных>/logs/`.
- Порт выбирается при первом запуске и запоминается в `<папка данных>/local.json`: адрес окна не меняется, и вместе с
  ним остаются настройки вида, сохранённые виды и вход.
- Одна копия на папку данных (замок `<папка данных>/mywork.lock`): вторая копия сообщает «Программа уже запущена».
- Закрытие окна останавливает сервер штатно: перенос файлов черновиков и начатая запись импорта доходят до конца,
  импорт до записи отменяется, база закрывается. Ошибка запуска и сбой показываются окном с путём к журналу.
- Это отдельный Cargo-workspace (`apps/desktop/src-tauri`, свой `Cargo.lock`): проверки основного workspace его не
  собирают. Его проверки — `cd apps/desktop/src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings
  && cargo test`; перед ними нужна собранная `apps/ui/build` (`npm run build` в `apps/ui`).

## Лицензия

[MIT](LICENSE)
