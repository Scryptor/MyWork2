import { describe, expect, it } from 'vitest';
import { item } from '#lib/app/fixtures.test-helper.js';
import { formatDate } from '#lib/format/date.js';
import { orderBrief, phoneLine } from './brief.js';

// formatMoney: a narrow no-break space between the groups, a no-break space before the symbol.
const rub = (text: string) => text.replace(' ', ' ') + ' ₽';

const issued = item(41, {
	brand: 'Samsung',
	model: 'Galaxy A52',
	malfunction: 'Не включается\nпосле падения',
	received_at: '2026-11-02T10:24:00',
	issued_at: '2026-11-12T16:05:00',
	final_price: 250000,
	discount: 20000,
	prepayment: 0
});

describe('orderBrief', () => {
	it('an issued order: device, malfunction, total, discount and the day it was issued', () => {
		expect(orderBrief(issued, 'Выдан')).toEqual({
			title: '№ 41 · ' + formatDate(issued.received_at),
			lines: [
				'Samsung Galaxy A52',
				'Не включается',
				'Итог: ' + rub('2 500'),
				'Скидка: ' + rub('200'),
				'Выдан ' + formatDate(issued.issued_at!)
			]
		});
	});

	it('no total says so; no discount, no line; a prepayment comes after the total', () => {
		const o = { ...issued, final_price: null, discount: 0, prepayment: 50000 };
		expect(orderBrief(o, 'Выдан').lines).toEqual([
			'Samsung Galaxy A52',
			'Не включается',
			'Итог не указан',
			'Предоплата: ' + rub('500'),
			'Выдан ' + formatDate(issued.issued_at!)
		]);
	});

	it('an order not issued shows its status instead; a blank malfunction has no line', () => {
		const o = { ...issued, issued_at: null, malfunction: '  ' };
		expect(orderBrief(o, 'Диагностика').lines).toEqual([
			'Samsung Galaxy A52',
			'Итог: ' + rub('2 500'),
			'Скидка: ' + rub('200'),
			'Диагностика'
		]);
	});

	it('no status name, no line for it', () => {
		const o = { ...issued, issued_at: null };
		expect(orderBrief(o, '').lines.at(-1)).toBe('Скидка: ' + rub('200'));
	});

	it('a malfunction that starts with empty lines shows its first written line', () => {
		const o = { ...issued, malfunction: '\n  Разбит экран  \nи корпус' };
		expect(orderBrief(o, 'Выдан').lines[1]).toBe('Разбит экран');
	});
});

describe('phoneLine', () => {
	it('the total and the day it was issued', () => {
		expect(phoneLine(issued)).toBe(rub('2 500') + ' · выдан ' + formatDate(issued.issued_at!));
	});

	it('only what there is: no total, not issued, nothing', () => {
		expect(phoneLine({ ...issued, final_price: null })).toBe(
			'выдан ' + formatDate(issued.issued_at!)
		);
		expect(phoneLine({ ...issued, issued_at: null })).toBe(rub('2 500'));
		expect(phoneLine({ ...issued, final_price: null, issued_at: null })).toBe('');
	});
});
