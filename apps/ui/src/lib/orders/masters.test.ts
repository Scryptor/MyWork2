import { describe, expect, it } from 'vitest';
import type { MasterUser } from '$api/MasterUser';
import { NO_MASTER, masterItems, masterName, masterOf, masterValue } from './masters.js';

const masters: MasterUser[] = [
	{ id: 3, display_name: 'Антон', active: true },
	{ id: 5, display_name: 'Борис', active: true },
	{ id: 7, display_name: 'Уволен', active: false }
];

describe('masters of an order', () => {
	it('offers «Не выбран» and the active masters only', () => {
		expect(masterItems(masters)).toEqual([
			{ value: NO_MASTER, label: 'Не выбран' },
			{ value: 3, label: 'Антон' },
			{ value: 5, label: 'Борис' }
		]);
	});

	it('keeps the disabled master of an older order as a disabled item', () => {
		expect(masterItems(masters, 7).at(-1)).toEqual({ value: 7, label: 'Уволен', disabled: true });
		expect(masterItems([], 9, 'Прежний').at(-1)).toEqual({
			value: 9,
			label: 'Прежний',
			disabled: true
		});
		expect(masterItems(masters, 3)).toHaveLength(3);
	});

	it('maps «Не выбран» to null and back', () => {
		expect(masterValue(null)).toBe(NO_MASTER);
		expect(masterOf(NO_MASTER)).toBeNull();
		expect(masterOf(null)).toBeNull();
		expect(masterOf(5)).toBe(5);
		expect(masterName(masters, 5)).toBe('Борис');
		expect(masterName(masters, 9, 'Прежний')).toBe('Прежний');
		expect(masterName(masters, null)).toBe('');
	});
});
