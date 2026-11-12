# MyWork2

Рабочее название проекта. Программа учёта заказов для ремонтной мастерской: переписывание старой Windows-программы на Rust, кроссплатформенно.

**Статус:** HTTP API и сервер готовы (план 2); следующий шаг — импорт MyWork 2 (план 3).

- [Спецификация](docs/superpowers/specs/2026-11-11-mywork3-crossplatform-design.md)
- [Дорожная карта и планы](docs/superpowers/plans/README.md)
- [Разбор старой программы](docs/legacy/mywork2-analysis.md)
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

## Лицензия

[MIT](LICENSE)
