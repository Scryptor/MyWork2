import { describe, expect, it, vi } from 'vitest';
import type { FirmLink } from '$api/FirmLink';
import { BUILTIN_IDS, isBuiltin, newLinkId, newLinkName } from './firm-links.js';

const link = (id: string): FirmLink => ({ id, name: id, url: '', enabled: true });

describe('newLinkId', () => {
	it('gives link_… from a-z, 0-9 and _', () => {
		expect(newLinkId([])).toMatch(/^link_[a-z0-9]+$/);
	});

	it('differs from the ids that exist, even within one millisecond', () => {
		vi.spyOn(Date, 'now').mockReturnValue(1_700_000_000_000);
		const first = newLinkId([]);
		const second = newLinkId([link(first)]);
		const third = newLinkId([link(first), link(second)]);
		expect(new Set([first, second, third]).size).toBe(3);
		for (const id of [second, third]) expect(id).toMatch(/^link_[a-z0-9_]+$/);
		vi.restoreAllMocks();
	});

	it('fits the 40 characters of the server', () => {
		expect(newLinkId([]).length).toBeLessThanOrEqual(40);
	});
});

describe('isBuiltin', () => {
	it('knows the four built-in links', () => {
		expect(BUILTIN_IDS).toEqual(['yandex_maps', '2gis', 'telegram', 'max']);
		expect(isBuiltin('2gis')).toBe(true);
		expect(isBuiltin('link_x')).toBe(false);
		expect(isBuiltin('google_maps')).toBe(false);
	});
});

describe('newLinkName', () => {
	it('takes the next free name', () => {
		const names = (...n: string[]) => n.map((name) => ({ ...link(name), name }));
		expect(newLinkName([], 'Новая ссылка')).toBe('Новая ссылка');
		expect(newLinkName(names('Новая ссылка'), 'Новая ссылка')).toBe('Новая ссылка 2');
		expect(newLinkName(names('новая ссылка', 'Новая ссылка 2'), 'Новая ссылка')).toBe(
			'Новая ссылка 3'
		);
	});
});
