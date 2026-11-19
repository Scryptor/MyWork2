import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ImportJob } from '$api/ImportJob';
import { ApiError } from '#lib/api/client.js';

// «Отчёт о переносе» (`settings/import/latest`) while the import writes a base with users of its own: the
// progress answers 401, and the page asks to sign in — as information with «Войти», not as a failure.
const api = vi.hoisted(() => ({ currentLegacyImport: vi.fn(), listLegacyImports: vi.fn() }));
vi.mock('#lib/api/import.js', () => api);
const nav = vi.hoisted(() => ({ goto: vi.fn() }));
vi.mock('$app/navigation', () => nav);
vi.mock('$app/paths', () => ({ resolve: (p: string) => p }));
const session = vi.hoisted(() => ({ clearToken: vi.fn() }));
vi.mock('#lib/api/session.js', async (actual) => ({
	...(await actual<typeof import('#lib/api/session.js')>()),
	clearToken: session.clearToken
}));

import Page from '../../routes/(app)/settings/import/latest/+page.svelte';

const running: ImportJob = {
	stage: 'writing',
	percent: 60,
	error: null,
	import_id: null
};

beforeEach(() => {
	for (const f of [...Object.values(api), nav.goto, session.clearToken]) f.mockReset();
	api.currentLegacyImport
		.mockResolvedValueOnce(running)
		.mockRejectedValue(new ApiError(401, 'unauthenticated', 'Войдите', null));
});

describe('the report of the last transfer, once the base has users of its own', () => {
	it('says how to sign in, as information, and «Войти» leads to the sign-in and back here', async () => {
		render(Page);
		const text = await screen.findByText(/войдите с логином и паролем из MyWork 2/, undefined, {
			timeout: 3000
		});
		const callout = text.closest('.callout');
		expect(callout).toHaveClass('info');
		expect(callout).toHaveAttribute('role', 'status');
		expect(screen.queryByRole('alert')).not.toBeInTheDocument();

		await userEvent.click(screen.getByRole('button', { name: 'Войти' }));
		expect(session.clearToken).toHaveBeenCalledOnce();
		expect(nav.goto).toHaveBeenCalledWith('/login?next=%2Fsettings%2Fimport%2Flatest');
	});
});
