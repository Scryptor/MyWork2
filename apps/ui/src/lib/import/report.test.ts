import { describe, expect, it } from 'vitest';
import type { ImportReportView } from '$api/ImportReportView';
import type { ReportGroup } from '$api/ReportGroup';
import {
	LATEST_REPORT,
	ORDERS_SHOWN,
	entryWhere,
	reportFileName,
	reportRoute,
	reportSections,
	reportTiles,
	visibleOrders
} from './report.js';

const group = (over: Partial<ReportGroup>): ReportGroup => ({
	code: 'date_unparsed',
	severity: 'warning',
	title: 'Не разобрана дата',
	count: 1,
	order_ids: [],
	entries: [],
	entries_count: 0,
	...over
});

const view = (over: Partial<ImportReportView> = {}): ImportReportView => ({
	id: 2,
	imported_at: '2026-11-19T14:05:09',
	source_path: '/home/user/MyWork2',
	mode: 'into_empty',
	orders: 11,
	clients: 7,
	users: 4,
	files: { copied: 6, bytes: 2048, missing: 0 },
	groups: [],
	warnings_text: 'Отчёт',
	files_pending: false,
	...over
});

describe('reportSections', () => {
	it('«Требует внимания» first, then «Сведения», each in the order of the server', () => {
		const dates = group({ code: 'date_unparsed', count: 3, order_ids: [4, 9] });
		const split = group({ code: 'phone_split', severity: 'info', title: 'Телефоны разложены' });
		const codes = group({ code: 'barcode_duplicate', title: 'Одинаковые штрихкоды', count: 2 });
		expect(reportSections([dates, split, codes])).toEqual([
			{ id: 'attention', title: 'Требует внимания', groups: [dates, codes] },
			{ id: 'info', title: 'Сведения', groups: [split] }
		]);
	});

	it('a section without groups is left out', () => {
		const split = group({ code: 'phone_split', severity: 'info' });
		expect(reportSections([split]).map((s) => s.id)).toEqual(['info']);
		expect(reportSections([])).toEqual([]);
	});
});

describe('reportTiles', () => {
	it('the totals; «Требуют внимания» counts the warnings of every group that needs it', () => {
		const r = view({
			orders: 12345,
			files: { copied: 6, bytes: 3 * 1024 * 1024, missing: 0 },
			groups: [
				group({ count: 3 }),
				group({ code: 'barcode_duplicate', count: 2 }),
				group({ code: 'phone_split', severity: 'info', count: 40 })
			]
		});
		expect(reportTiles(r)).toEqual([
			// Digits in groups of three with a narrow no-break space.
			{ label: 'Заказов', value: '12\u202f345' },
			{ label: 'Клиентов', value: '7' },
			{ label: 'Пользователей', value: '4' },
			{ label: 'Файлов', value: '6', hint: '3 МБ' },
			{ label: 'Требуют внимания', value: '5', tone: 'warning' }
		]);
	});

	it('files not found are a tile of their own; nothing to look at is not a warning', () => {
		const tiles = reportTiles(view({ files: { copied: 5, bytes: 10, missing: 2 } }));
		expect(tiles).toContainEqual({ label: 'Файлов не найдено', value: '2', tone: 'warning' });
		expect(tiles.at(-1)).toEqual({ label: 'Требуют внимания', value: '0' });
	});
});

describe('visibleOrders', () => {
	it('a long list shows its first numbers until all are asked for', () => {
		const ids = Array.from({ length: ORDERS_SHOWN + 5 }, (_, i) => i + 1);
		expect(visibleOrders(ids, false)).toHaveLength(ORDERS_SHOWN);
		expect(visibleOrders(ids, true)).toHaveLength(ORDERS_SHOWN + 5);
		expect(visibleOrders([3, 7], false)).toEqual([3, 7]);
	});
});

describe('reportFileName and reportRoute', () => {
	it('the file is named by the moment of the import', () => {
		expect(reportFileName(view())).toBe('mywork2-import-2026-11-19-1405.txt');
	});

	it('the report of an import has its own address in «Настройки»', () => {
		expect(reportRoute(7)).toBe('settings/import/7');
		expect(reportRoute('latest')).toBe('settings/import/latest');
		expect(LATEST_REPORT).toBe(`/${reportRoute('latest')}`);
	});
});

describe('entryWhere', () => {
	const e = { source: 'Catalog', key: '5', field: 'ClientId', value: '77', message: '' };
	it('an order: «Заказ № 5», the field in the words of the program', () => {
		expect(entryWhere(e)).toBe('Заказ № 5 · Клиент');
		expect(entryWhere({ ...e, field: 'Data_priema' })).toBe('Заказ № 5 · Дата приёма');
		expect(entryWhere({ ...e, field: 'Barcode' })).toBe('Заказ № 5 · Штрихкод');
	});
	it('a client, a user, a file, a setting', () => {
		expect(entryWhere({ ...e, source: 'ClientsMap', key: '12', field: 'Phone' })).toBe(
			'Клиент № 12 · Телефон'
		);
		expect(entryWhere({ ...e, source: 'Users', key: '3', field: 'user_pwd' })).toBe(
			'Пользователь · 3 · Пароль'
		);
		expect(entryWhere({ ...e, source: 'file', key: '1/1/a.jpg', field: '', value: '' })).toBe(
			'Файл · 1/1/a.jpg'
		);
		expect(entryWhere({ ...e, source: 'Config.ini', key: '[A] b', field: 'b', value: 'x' })).toBe(
			'Настройки MyWork 2 · [A] b'
		);
	});
	it('the same text is never repeated: status_custom', () => {
		const text = 'Согласование с клиентом';
		expect(
			entryWhere({
				source: 'Catalog',
				key: text,
				field: 'Status_remonta',
				value: text,
				message: ''
			})
		).toBe('Заказ · Статус');
	});
	it('the key of a repeated barcode is the barcode, never a number of an order', () => {
		const dup = {
			source: 'Catalog',
			key: '123456789012',
			field: 'Barcode',
			value: 'заказы 3, 7',
			message: ''
		};
		expect(entryWhere(dup, 'barcode_duplicate')).toBe('Заказ · 123456789012 · Штрихкод');
		// A barcode not in UPC-A or made anew is of one order: its key is that order's number.
		expect(entryWhere({ ...e, field: 'Barcode' }, 'barcode_invalid')).toBe('Заказ № 5 · Штрихкод');
	});
	it('next_number has no «Catalog · Catalog»', () => {
		expect(
			entryWhere({ source: 'Catalog', key: 'Catalog', field: '', value: '16', message: '' })
		).toBe('Заказ');
	});
	it('an unknown source and field stay as they are', () => {
		expect(entryWhere({ source: 'Zzz', key: 'k', field: 'Weird', value: 'v', message: '' })).toBe(
			'Zzz · k · Weird'
		);
	});
});
