// The print settings of this computer (spec 5.2): the printers, the label, the offset, the printer's
// resolution, the copies and what is printed by itself at the intake and the issue. Kept in the
// browser's storage, not on the server: each workplace has its own printers.

import { readStorage, writeStorage } from '#lib/design/storage.js';

export type PrinterDpi = 203 | 300 | 600;

export interface PrintSettings {
	/** The printer of the acts; null — the system's default printer. */
	actPrinter: string | null;
	/** The printer of the stickers and the calibration label; null — the system's default printer. */
	labelPrinter: string | null;
	/** The label, width × height, mm. */
	labelMm: [number, number];
	/** The content moved right and down, mm. */
	offsetMm: [number, number];
	/** Dots per inch of the printer: the barcode is fitted to its dots. */
	dpi: PrinterDpi;
	actCopies: number;
	labelCopies: number;
	/** Printed by itself when an order is taken in. */
	onIntake: { act: boolean; sticker: boolean };
	/** Printed by itself when an order is issued. */
	onIssue: { act: boolean };
}

/** A change: the switches of the intake and the issue each on its own. */
export type PrintSettingsPatch = Partial<Omit<PrintSettings, 'onIntake' | 'onIssue'>> & {
	onIntake?: Partial<PrintSettings['onIntake']>;
	onIssue?: Partial<PrintSettings['onIssue']>;
};

export const PRINT_KEY = 'mw.print';
/** Stored without this version (or with another one), the settings are not read: the defaults. */
export const PRINT_VERSION = 1;

/** The limits the server has too (`mw-core::documents`); the copies are the desktop program's. */
export const LABEL_MM = { min: 15, max: 200 } as const;
export const OFFSET_MM = { min: -10, max: 10 } as const;
export const COPIES = { min: 1, max: 99 } as const;
export const PRINTER_DPIS: readonly PrinterDpi[] = [203, 300, 600];
/** The labels of the list on «Печать»; any other size is «Свой». */
export const LABEL_PRESETS: readonly (readonly [number, number])[] = [
	[58, 40],
	[40, 30]
];

/** Frozen: the settings share its pairs until they are changed, nobody changes them in place. */
export const DEFAULT_PRINT_SETTINGS: PrintSettings = Object.freeze({
	actPrinter: null,
	labelPrinter: null,
	labelMm: Object.freeze([58, 40]) as [number, number],
	offsetMm: Object.freeze([0, 0]) as [number, number],
	dpi: 203,
	actCopies: 1,
	labelCopies: 1,
	onIntake: Object.freeze({ act: true, sticker: false }),
	onIssue: Object.freeze({ act: true })
});

const inRange = (n: unknown, r: { min: number; max: number }): n is number =>
	typeof n === 'number' && Number.isFinite(n) && n >= r.min && n <= r.max;

/** A pair of numbers in `r`, or `fallback`: one side out of it refuses the pair. */
function pair(v: unknown, r: { min: number; max: number }, fallback: [number, number]) {
	return Array.isArray(v) && v.length === 2 && inRange(v[0], r) && inRange(v[1], r)
		? ([v[0], v[1]] as [number, number])
		: fallback;
}
const printer = (v: unknown, fallback: string | null) =>
	v === null || (typeof v === 'string' && v !== '') ? v : fallback;
const copies = (v: unknown, fallback: number) =>
	Number.isInteger(v) && inRange(v, COPIES) ? v : fallback;
const flag = (v: unknown, fallback: boolean) => (typeof v === 'boolean' ? v : fallback);
const record = (v: unknown): Record<string, unknown> =>
	typeof v === 'object' && v !== null && !Array.isArray(v) ? (v as Record<string, unknown>) : {};

/** Field by field: what does not fit is taken from `base`. */
function merge(o: Record<string, unknown>, base: PrintSettings): PrintSettings {
	const onIntake = record(o.onIntake);
	const onIssue = record(o.onIssue);
	return {
		actPrinter: 'actPrinter' in o ? printer(o.actPrinter, base.actPrinter) : base.actPrinter,
		labelPrinter:
			'labelPrinter' in o ? printer(o.labelPrinter, base.labelPrinter) : base.labelPrinter,
		labelMm: pair(o.labelMm, LABEL_MM, base.labelMm),
		offsetMm: pair(o.offsetMm, OFFSET_MM, base.offsetMm),
		dpi: PRINTER_DPIS.find((d) => d === o.dpi) ?? base.dpi,
		actCopies: copies(o.actCopies, base.actCopies),
		labelCopies: copies(o.labelCopies, base.labelCopies),
		onIntake: {
			act: flag(onIntake.act, base.onIntake.act),
			sticker: flag(onIntake.sticker, base.onIntake.sticker)
		},
		onIssue: { act: flag(onIssue.act, base.onIssue.act) }
	};
}

/** Never throws: garbage or another version gives the defaults, a field that does not fit its default. */
export function parsePrintSettings(raw: string | null): PrintSettings {
	let obj: unknown;
	try {
		obj = raw === null ? null : JSON.parse(raw);
	} catch {
		obj = null;
	}
	const o = record(obj);
	if (o.v !== PRINT_VERSION) return DEFAULT_PRINT_SETTINGS;
	return merge(o, DEFAULT_PRINT_SETTINGS);
}

/** A number of millimetres as typed: «57,5», «1.5», «−2»; anything else is null. */
export function parseMm(text: string): number | null {
	const t = text.trim().replace('−', '-');
	if (!/^-?\d+([.,]\d+)?$/.test(t)) return null;
	return Number(t.replace(',', '.'));
}

/** Millimetres as the fields show them: a decimal comma. */
export const formatMm = (n: number): string => String(n).replace('.', ',');

let value = $state.raw<PrintSettings>(parsePrintSettings(readStorage(PRINT_KEY)));

/** The print settings of this computer; `set` keeps a change in the storage at once. */
export const printSettings = {
	get actPrinter() {
		return value.actPrinter;
	},
	get labelPrinter() {
		return value.labelPrinter;
	},
	get labelMm() {
		return value.labelMm;
	},
	get offsetMm() {
		return value.offsetMm;
	},
	get dpi() {
		return value.dpi;
	},
	get actCopies() {
		return value.actCopies;
	},
	get labelCopies() {
		return value.labelCopies;
	},
	get onIntake() {
		return value.onIntake;
	},
	get onIssue() {
		return value.onIssue;
	},
	/** A value that does not fit (a label of 10 mm, 100 copies) is not taken: the setting stays. */
	set(patch: PrintSettingsPatch): void {
		value = merge(patch as Record<string, unknown>, value);
		// storage unavailable: the settings live in memory until the page is closed
		writeStorage(PRINT_KEY, JSON.stringify({ v: PRINT_VERSION, ...value }));
	}
};
