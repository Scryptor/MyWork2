import type { Density } from '#lib/design/prefs.svelte.js';

// Row height in rem by density; must equal `--row-height` in tokens.css (checked by tokens.test.ts).
export const ROW_REM: Record<Density, number> = { comfortable: 2, compact: 1.75 };
