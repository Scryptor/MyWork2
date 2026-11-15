import { describe, expect, it } from 'vitest';
import { matchesQuery, normalizeSearch } from './search.js';

describe('search', () => {
	it('normalizes', () => {
		expect(normalizeSearch('  ЁЛКИН\tПётр ')).toBe('елкин петр');
		expect(normalizeSearch('ё')).toBe('е');
		expect(normalizeSearch('Й')).toBe('й');
	});
	it('matches', () => {
		expect(matchesQuery('ИВАНОВ ИВАН', 'иван ив')).toBe(true);
		expect(matchesQuery('ИВАНОВ', 'петр')).toBe(false);
	});
});
