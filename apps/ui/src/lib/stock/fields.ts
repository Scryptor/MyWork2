// The fields of a stock position: their labels and the words for the server's refusals. The add dialog
// and the panel both show them.
import { ApiError } from '#lib/api/client.js';
import { currencySymbol } from '#lib/format/currency.svelte.js';
import { ru } from '#lib/i18n/ru.js';

/** The keys of `NewStockItem` / `StockItemPatch` and what they are called on screen. */
export const STOCK_FIELDS = {
	name: 'Наименование',
	part_type: 'Тип запчасти',
	device_type: 'Тип устройства',
	brand: 'Бренд',
	model: 'Модель',
	color: 'Цвет',
	note: 'Примечание',
	quantity: 'Количество',
	price: 'Цена',
	restock_threshold: 'Докупить при'
} as const;

export type StockField = keyof typeof STOCK_FIELDS;

/** Limits of the server (`mw_domain::stock`): shown in the words of the refusals. */
/** Photos of one position (`mw_domain::stock::MAX_STOCK_PHOTOS`). */
export const MAX_STOCK_PHOTOS = 3;
export const MAX_STOCK_QUANTITY = 1_000_000;
const MAX_TEXT_CHARS = 200;
const MAX_NOTE_CHARS = 2000;

const label = (field: string): string =>
	Object.hasOwn(STOCK_FIELDS, field) ? STOCK_FIELDS[field as StockField] : field;

/** What a refusal of the server (`required`, `too_long`, `out_of_range`, `deleted`) says about a field. */
export function stockFieldMessage(field: string, code: string): string {
	const t = ru.stock.errors;
	switch (code) {
		case 'required':
			return t.nameRequired;
		case 'too_long':
			return t.tooLong(label(field), field === 'note' ? MAX_NOTE_CHARS : MAX_TEXT_CHARS);
		case 'out_of_range':
			return field === 'price'
				? t.priceRange(label(field), currencySymbol())
				: t.quantityRange(label(field));
		case 'deleted':
			return t.deleted;
		default:
			return t.invalid(label(field));
	}
}

/** The messages of a validation error of the server by field; anything else has none. */
export function stockFieldErrors(e: unknown): Record<string, string> {
	if (!(e instanceof ApiError) || e.code !== 'validation') return {};
	return Object.fromEntries(
		(e.body?.fields ?? []).map((f) => [f.field, stockFieldMessage(f.field, f.code)])
	);
}
