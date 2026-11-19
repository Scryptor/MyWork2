import { describe, expect, it } from 'vitest';
import { KIND_HINTS, kindChangeWarning, startKindChange } from './status-kind.js';

describe('kindChangeWarning', () => {
	it('nothing to warn about when no order is in the status', () => {
		expect(kindChangeWarning('open', 'issued', 0)).toBeNull();
	});

	it('into «Выдан»', () => {
		expect(kindChangeWarning('open', 'issued', 1)).toBe(
			'1 заказ станет выданным и попадёт в отчёты'
		);
		expect(kindChangeWarning('ready', 'issued', 5)).toBe(
			'5 заказов станут выданными и попадут в отчёты'
		);
	});

	it('out of «Выдан»', () => {
		expect(kindChangeWarning('issued', 'open', 2)).toBe(
			'2 заказа перестанут считаться выданными и пропадут из отчётов'
		);
	});

	it('into «Готов»', () => {
		expect(kindChangeWarning('open', 'ready', 3)).toBe(
			'3 заказа попадут в „Готовы“ и „Давно готовы“'
		);
	});

	it('any other change names the kind', () => {
		expect(kindChangeWarning('open', 'diagnostics', 1)).toBe('1 заказ сменит вид на „Диагностика“');
		expect(kindChangeWarning('ready', 'open', 11)).toBe('11 заказов сменят вид на „Открыт“');
	});

	it('with the count unknown it always warns, without a number', () => {
		expect(kindChangeWarning('open', 'issued', null)).toBe(
			'Не удалось узнать, сколько заказов в статусе. Все они сменят вид на „Выдан“'
		);
		expect(kindChangeWarning('open', 'open', null)).toBeNull();
	});

	it('a hint for every kind', () => {
		expect(Object.keys(KIND_HINTS).sort()).toEqual(['diagnostics', 'issued', 'open', 'ready']);
		expect(KIND_HINTS.open).toBe('заказ в работе');
	});
});

describe('startKindChange', () => {
	it('makes the warning once, from the count of the moment the window opens', () => {
		const change = startKindChange({ id: 7, name: 'Выдан', kind: 'issued' }, 'open', 3);
		expect(change).toMatchObject({ id: 7, name: 'Выдан', from: 'issued', to: 'open' });
		expect(change.warning).toBe(kindChangeWarning('issued', 'open', 3));
		// A later reading with another count does not touch what was made.
		expect(kindChangeWarning('issued', 'open', 0)).toBeNull();
		expect(change.warning).not.toBeNull();
	});
	it('no warning when no order is in the status', () => {
		expect(startKindChange({ id: 1, name: 'X', kind: 'open' }, 'ready', 0).warning).toBeNull();
	});
});
