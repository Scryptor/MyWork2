import { describe, expect, it } from 'vitest';
import { filterCommands, type CommandItem } from './commands.js';

const ITEMS: CommandItem[] = [
	{ id: 'intake', label: 'Принять устройство' },
	{ id: 'theme', label: 'Сменить тему', keywords: ['тема', 'оформление'] },
	{ id: 'tree', label: 'Праздник', keywords: ['ёлка'] }
];

describe('filterCommands', () => {
	it('matches a label prefix', () => {
		expect(filterCommands(ITEMS, 'прин').map((i) => i.id)).toEqual(['intake']);
	});
	it('matches keywords', () => {
		expect(filterCommands(ITEMS, 'тем').map((i) => i.id)).toEqual(['theme']);
	});
	it('folds case and ё', () => {
		expect(filterCommands(ITEMS, 'ЁЛ').map((i) => i.id)).toEqual(['tree']);
	});
	it('needs every word of the query', () => {
		expect(filterCommands(ITEMS, 'сменить оформ').map((i) => i.id)).toEqual(['theme']);
		expect(filterCommands(ITEMS, 'сменить ёлка')).toEqual([]);
	});
	it('returns everything for an empty query', () => {
		expect(filterCommands(ITEMS, '  ')).toEqual(ITEMS);
	});
});
