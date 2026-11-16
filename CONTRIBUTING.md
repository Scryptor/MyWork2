# Участие в разработке

1. Заведите issue с описанием задачи: https://github.com/Scryptor/MyWork2/issues.
2. Создайте ветку от `main` под эту задачу.
3. Откройте pull request в https://github.com/Scryptor/MyWork2: все проверки (fmt, clippy, тесты, для интерфейса — `npm run verify` и `npm run test:e2e`) должны быть зелёными.
4. Правила работы с кодом описаны в [AGENTS.md](AGENTS.md).

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
