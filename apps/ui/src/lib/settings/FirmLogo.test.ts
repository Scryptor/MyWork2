import { render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { logoUrl, releaseLogoUrl } from '#lib/api/logo.js';
import FirmLogo from './FirmLogo.svelte';

// The picture of the logo is a `blob:` address, one alive at a time (`#lib/api/logo.js`): a screen lets
// its own go and never the one another screen shows. The server is replaced by a fetch that answers the
// picture when the test says so.
let seq = 0;
const revoked: string[] = [];
/** The answers to the requests of the picture not given yet. */
const pending: (() => void)[] = [];
let holdPicture = false;

const info = () =>
	new Response(JSON.stringify({ width: 4, height: 2, size: 70 }), {
		headers: { 'Content-Type': 'application/json' }
	});

beforeEach(() => {
	seq = 0;
	revoked.length = 0;
	pending.length = 0;
	holdPicture = false;
	URL.createObjectURL = () => `blob:logo/${++seq}`;
	URL.revokeObjectURL = (u: string) => void revoked.push(u);
	vi.stubGlobal(
		'fetch',
		vi.fn(async (url: string) => {
			if (new URL(url, 'http://127.0.0.1').pathname.endsWith('/info')) return info();
			if (!holdPicture) return new Response('png');
			return new Promise<Response>((answer) => pending.push(() => answer(new Response('png'))));
		})
	);
});
afterEach(() => {
	releaseLogoUrl();
	vi.unstubAllGlobals();
});

const picture = () => screen.getByRole('img', { name: 'Логотип сервиса' });

describe('FirmLogo', () => {
	it('an address that comes after the screen is gone is let go; the next screen keeps its own', async () => {
		holdPicture = true;
		const first = render(FirmLogo, { canEdit: false });
		await waitFor(() => expect(pending).toHaveLength(1));
		first.unmount();
		pending[0]!();
		// The address made for the screen that is gone is revoked at once, not left for the next one.
		await waitFor(() => expect(revoked).toEqual(['blob:logo/1']));

		holdPicture = false;
		render(FirmLogo, { canEdit: false });
		await waitFor(() => expect(picture()).toHaveAttribute('src', 'blob:logo/2'));
		expect(revoked).toEqual(['blob:logo/1']);
	});

	it('leaving lets go of its own address only, never the one another screen shows now', async () => {
		const shown = render(FirmLogo, { canEdit: false });
		await waitFor(() => expect(picture()).toHaveAttribute('src', 'blob:logo/1'));
		// Another screen takes the picture: the first address goes, as only one is alive.
		const other = await logoUrl();
		expect(revoked).toEqual(['blob:logo/1']);
		shown.unmount();
		expect(revoked).toEqual(['blob:logo/1']);
		expect(revoked).not.toContain(other);
	});
});
