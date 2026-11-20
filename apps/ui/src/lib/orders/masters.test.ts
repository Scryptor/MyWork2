import { describe, expect, it } from 'vitest';
import type { MasterUser } from '$api/MasterUser';
import {
	NO_MASTER,
	filterMasterItems,
	masterChoices,
	masterItems,
	pendingMaster,
	pendingMasterItems,
	masterName,
	masterOf,
	masterValue
} from './masters.js';

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
		expect(masterItems(masters, 7).at(-1)).toEqual({
			value: 7,
			label: 'Уволен (отключён)',
			disabled: true
		});
		expect(masterItems([], 9, 'Прежний').at(-1)).toEqual({
			value: 9,
			label: 'Прежний (отключён)',
			disabled: true
		});
		expect(masterItems(masters, 3)).toHaveLength(3);
	});

	it('while the masters load, or when they do not, the chosen one still has a name', () => {
		expect(pendingMasterItems(4, false)).toEqual([
			{ value: NO_MASTER, label: 'Не выбран' },
			{ value: 4, label: 'Загрузка…', disabled: true }
		]);
		expect(pendingMasterItems(4, true).at(-1)).toEqual({
			value: 4,
			label: 'Мастер № 4',
			disabled: true
		});
		expect(pendingMasterItems(null, false)).toEqual([{ value: NO_MASTER, label: 'Не выбран' }]);
	});

	it('the order knows the name of its master: shown as it is while the masters load or fail', () => {
		expect(pendingMasterItems(3, false, 'Антон').at(-1)).toEqual({
			value: 3,
			label: 'Антон',
			disabled: true
		});
		expect(pendingMaster(3, true, 'Антон').label).toBe('Антон');
		expect(pendingMaster(3, true).label).toBe('Мастер № 3');
	});

	it('before the masters are read nobody is «(отключён)»; after, the list decides', () => {
		const labels = (items: { label: string }[]) => items.map((i) => i.label);
		expect(labels(masterChoices(null, 3, 'Антон'))).toEqual(['Не выбран', 'Антон']);
		expect(labels(masterChoices('failed', 3, 'Антон'))).toEqual(['Не выбран', 'Антон']);
		expect(labels(masterChoices('failed', 3))).toEqual(['Не выбран', 'Мастер № 3']);
		expect(labels(masterChoices(null, null))).toEqual(['Не выбран']);
		expect(masterChoices(masters, 7, 'Уволен')).toEqual(masterItems(masters, 7, 'Уволен'));
		expect(masterName(null, 3, 'Антон')).toBe('Антон');
		expect(masterName('failed', 3, '3')).toBe('3');
	});

	it('«Фильтры»: every master, the disabled marked; before they are read only the chosen one, named', () => {
		expect(filterMasterItems(masters, undefined).map((i) => i.label)).toEqual([
			'Антон',
			'Борис',
			'Уволен (отключён)'
		]);
		expect(filterMasterItems(null, 5)).toEqual([{ value: 5, label: 'Загрузка…', disabled: true }]);
		expect(filterMasterItems('failed', 5)).toEqual([
			{ value: 5, label: 'Мастер № 5', disabled: true }
		]);
		expect(filterMasterItems(null, undefined)).toEqual([]);
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
