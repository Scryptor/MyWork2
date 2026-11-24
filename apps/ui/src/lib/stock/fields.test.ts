import { describe, expect, it } from 'vitest';
import { ApiError } from '#lib/api/client.js';
import { STOCK_FIELDS, stockFieldErrors, stockFieldMessage } from './fields.js';

describe('stock item fields', () => {
	it('names every field of the position in Russian', () => {
		expect(STOCK_FIELDS.name).toBe('Наименование');
		expect(STOCK_FIELDS.part_type).toBe('Тип запчасти');
		expect(STOCK_FIELDS.restock_threshold).toBe('Докупить при');
		expect(STOCK_FIELDS.quantity).toBe('Количество');
		expect(STOCK_FIELDS.price).toBe('Цена');
	});

	it('says what the limits are', () => {
		expect(stockFieldMessage('price', 'out_of_range')).toBe('Цена — от 0 до 10 000 000 ₽');
		expect(stockFieldMessage('quantity', 'out_of_range')).toBe('Количество — от 0 до 1 000 000');
		expect(stockFieldMessage('restock_threshold', 'out_of_range')).toBe(
			'Докупить при — от 0 до 1 000 000'
		);
		expect(stockFieldMessage('model', 'too_long')).toBe('Модель — не длиннее 200 знаков');
		expect(stockFieldMessage('note', 'too_long')).toBe('Примечание — не длиннее 2000 знаков');
		expect(stockFieldMessage('name', 'required')).toMatch(/наименование/i);
		expect(stockFieldMessage('id', 'deleted')).toBe('Позиция удалена');
	});

	it('a code it does not know still says something', () => {
		expect(stockFieldMessage('brand', 'strange')).toBe('Бренд: недопустимое значение');
	});

	it('takes the fields of a validation error of the server', () => {
		const e = new ApiError(422, 'validation', 'Неверно', {
			code: 'validation',
			message: 'Неверно',
			fields: [
				{ field: 'price', code: 'out_of_range', message: 'x' },
				{ field: 'name', code: 'required', message: 'y' }
			],
			current_version: null,
			permission: null,
			retry_after_secs: null,
			diagnostics: []
		});
		expect(stockFieldErrors(e)).toEqual({
			price: 'Цена — от 0 до 10 000 000 ₽',
			name: stockFieldMessage('name', 'required')
		});
		expect(stockFieldErrors(new Error('x'))).toEqual({});
	});
});
