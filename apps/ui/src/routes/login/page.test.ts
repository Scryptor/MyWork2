import { render } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { queueLoginNotice } from '#lib/app/login-notice.js';
import { toast } from '#lib/ui/toast/toast.svelte.js';

vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
vi.mock('$app/state', () => ({
	page: { url: new URL('http://127.0.0.1:4174/login?next=%2Forders') }
}));
vi.mock('#lib/api/auth.js', () => ({ login: vi.fn() }));

import LoginPage from './+page.svelte';

afterEach(() => toast.clear());

// Why the user is on the login page (expired session, another tab signed in) is a toast. The page resets
// the session when it mounts, and that clears every toast: the notice has to come after the reset.
describe('login page', () => {
	it('shows the queued notice after its own reset', () => {
		queueLoginNotice({ message: 'Сеанс истёк — войдите снова' });
		render(LoginPage);
		expect(toast.items.map((t) => t.message)).toEqual(['Сеанс истёк — войдите снова']);
	});

	it('shows it once: the next visit has no stale notice', () => {
		queueLoginNotice({ message: 'один раз' });
		render(LoginPage).unmount();
		toast.clear();
		render(LoginPage);
		expect(toast.items).toEqual([]);
	});

	it('the race: a toast shown before the page mounts is cleared by the reset', () => {
		// What the layout used to do after `await goto` when the page mounted late.
		toast.show({ message: 'Сеанс истёк — войдите снова' });
		render(LoginPage);
		expect(toast.items).toEqual([]);
	});
});
