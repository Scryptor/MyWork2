import { afterEach, describe, expect, it, vi } from 'vitest';
import {
	DEFAULT_PRINT_SETTINGS,
	parseMm,
	parsePrintSettings,
	printSettings,
	PRINT_KEY,
	PRINT_VERSION
} from './settings.svelte.js';

// The print settings of this computer (`mw.print`): what is stored is read field by field, anything
// that does not fit falls back to its default; an older version or garbage gives the defaults.

afterEach(() => {
	printSettings.set(DEFAULT_PRINT_SETTINGS);
	localStorage.clear();
	vi.resetModules();
});

const stored = (fields: Record<string, unknown>) => JSON.stringify({ v: PRINT_VERSION, ...fields });

describe('parsePrintSettings', () => {
	it('defaults: 58 × 40, no offset, 203 dpi, one copy, the intake act and the issue act', () => {
		expect(DEFAULT_PRINT_SETTINGS).toEqual({
			actPrinter: null,
			labelPrinter: null,
			labelMm: [58, 40],
			offsetMm: [0, 0],
			dpi: 203,
			actCopies: 1,
			labelCopies: 1,
			onIntake: { act: true, sticker: false },
			onIssue: { act: true }
		});
	});

	it('an empty storage, garbage and not an object give the defaults', () => {
		for (const raw of [null, '', '{', 'null', '42', '"text"', '[]', '[1, 2]']) {
			expect(parsePrintSettings(raw), String(raw)).toEqual(DEFAULT_PRINT_SETTINGS);
		}
	});

	it('another version (or none) gives the defaults, whatever its fields', () => {
		const fields = { labelMm: [40, 30], dpi: 300, actPrinter: 'Акты' };
		for (const raw of [
			JSON.stringify(fields),
			JSON.stringify({ v: 0, ...fields }),
			JSON.stringify({ v: PRINT_VERSION + 1, ...fields }),
			JSON.stringify({ v: String(PRINT_VERSION), ...fields })
		]) {
			expect(parsePrintSettings(raw), raw).toEqual(DEFAULT_PRINT_SETTINGS);
		}
	});

	it('reads every field of this version', () => {
		const all = {
			actPrinter: 'HP LaserJet (2 этаж)',
			labelPrinter: 'Xprinter_XP-365B',
			labelMm: [57.5, 30],
			offsetMm: [-1.5, 2],
			dpi: 300,
			actCopies: 2,
			labelCopies: 99,
			onIntake: { act: false, sticker: true },
			onIssue: { act: false }
		};
		expect(parsePrintSettings(stored(all))).toEqual(all);
	});

	it('a field that does not fit falls back alone; the others stay', () => {
		expect(parsePrintSettings(stored({ labelMm: [40, 30], dpi: 250, actCopies: 0 }))).toEqual({
			...DEFAULT_PRINT_SETTINGS,
			labelMm: [40, 30]
		});
		expect(
			parsePrintSettings(stored({ onIntake: { act: 'да', sticker: true }, onIssue: 'нет' }))
		).toEqual({ ...DEFAULT_PRINT_SETTINGS, onIntake: { act: true, sticker: true } });
	});

	it('a label side outside 15..=200 mm is refused: the default size', () => {
		for (const labelMm of [
			[14.9, 40],
			[58, 14],
			[201, 40],
			[58, 200.5],
			[0, 0],
			[-58, 40],
			[58],
			[58, 40, 1],
			[58, '40'],
			[58, null],
			'58x40',
			null
		]) {
			expect(parsePrintSettings(stored({ labelMm })).labelMm, JSON.stringify(labelMm)).toEqual([
				58, 40
			]);
		}
		expect(parsePrintSettings(stored({ labelMm: [15, 200] })).labelMm).toEqual([15, 200]);
	});

	it('an offset outside −10..=10 mm, a DPI not of 203/300/600, copies outside 1..=99 are refused', () => {
		for (const offsetMm of [[10.5, 0], [0, -11], [0], ['1', 0]]) {
			expect(parsePrintSettings(stored({ offsetMm })).offsetMm, JSON.stringify(offsetMm)).toEqual([
				0, 0
			]);
		}
		expect(parsePrintSettings(stored({ offsetMm: [-10, 10] })).offsetMm).toEqual([-10, 10]);
		for (const dpi of [0, 72, 204, '300', null]) {
			expect(parsePrintSettings(stored({ dpi })).dpi, String(dpi)).toBe(203);
		}
		expect(parsePrintSettings(stored({ dpi: 600 })).dpi).toBe(600);
		for (const copies of [0, 100, 1.5, -1, '2']) {
			const parsed = parsePrintSettings(stored({ actCopies: copies, labelCopies: copies }));
			expect([parsed.actCopies, parsed.labelCopies], String(copies)).toEqual([1, 1]);
		}
	});

	it('a printer is a name or «по умолчанию» (null); an empty name or not a string is the default', () => {
		for (const printer of ['', 7, true, ['Акты']]) {
			const parsed = parsePrintSettings(stored({ actPrinter: printer, labelPrinter: printer }));
			expect([parsed.actPrinter, parsed.labelPrinter], JSON.stringify(printer)).toEqual([
				null,
				null
			]);
		}
	});
});

describe('the store', () => {
	it('keeps a change on this computer: the next start reads it', async () => {
		printSettings.set({ labelMm: [40, 30], labelPrinter: 'Наклейки', onIntake: { sticker: true } });
		expect(printSettings.labelMm).toEqual([40, 30]);
		expect(printSettings.onIntake).toEqual({ act: true, sticker: true });
		expect(JSON.parse(localStorage.getItem(PRINT_KEY)!)).toMatchObject({
			v: PRINT_VERSION,
			labelMm: [40, 30],
			labelPrinter: 'Наклейки'
		});
		vi.resetModules();
		const fresh = await import('./settings.svelte.js');
		expect(fresh.printSettings.labelMm).toEqual([40, 30]);
		expect(fresh.printSettings.labelPrinter).toBe('Наклейки');
		expect(fresh.printSettings.onIntake).toEqual({ act: true, sticker: true });
	});

	it('a value that does not fit is not taken: the setting stays as it was', () => {
		printSettings.set({ labelMm: [40, 30], dpi: 300 });
		printSettings.set({ labelMm: [10, 30] });
		printSettings.set({ dpi: 250 as never, actCopies: 100 });
		expect(printSettings.labelMm).toEqual([40, 30]);
		expect(printSettings.dpi).toBe(300);
		expect(printSettings.actCopies).toBe(1);
	});

	it('starts from the defaults when nothing is stored', async () => {
		vi.resetModules();
		const fresh = await import('./settings.svelte.js');
		expect(fresh.printSettings.labelMm).toEqual([58, 40]);
		expect(fresh.printSettings.actPrinter).toBeNull();
	});
});

describe('parseMm', () => {
	it('takes a comma or a point and a minus of either kind', () => {
		expect(parseMm('58')).toBe(58);
		expect(parseMm(' 57,5 ')).toBe(57.5);
		expect(parseMm('1.5')).toBe(1.5);
		expect(parseMm('-2')).toBe(-2);
		expect(parseMm('−2,5')).toBe(-2.5);
	});

	it('anything else is not a number', () => {
		for (const text of ['', ' ', 'abc', '5 мм', '1,2,3', '1e2', '--1', ',5', '5,']) {
			expect(parseMm(text), text).toBeNull();
		}
	});
});
