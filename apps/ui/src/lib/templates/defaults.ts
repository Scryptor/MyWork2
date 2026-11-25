import type { Block } from '$api/Block';
import type { BlockBody } from '$api/BlockBody';
import { ru } from '#lib/i18n/ru.js';
import { linkSubstitution } from './qr.js';

/** The order of the «Add block» menu; calibration marks are not offered. */
export const BLOCK_TYPES = [
	'text',
	'fields',
	'barcode',
	'logo',
	'qr',
	'signatures',
	'separator',
	'spacer',
	'columns',
	'copy_label',
	'typst_code'
] as const satisfies readonly BlockBody['type'][];

/** The longest code of a «Typst-код» block, as `MAX_CODE_CHARS` of the model. */
export const CODE_MAX = 20_000;

/** The longest link of a «QR-код» block, as `MAX_QR_LINK_CHARS` of the model. */
export const QR_LINK_MAX = 500;

function body(type: (typeof BLOCK_TYPES)[number]): BlockBody {
	const d = ru.templates.defaults;
	switch (type) {
		case 'text':
			return { type, text: d.text, size_pt: null, bold: false, align: 'left', one_line: false };
		case 'fields':
			return {
				type,
				rows: [{ label: d.fieldLabel, value: '', hide_if_empty: false, bold: false }]
			};
		case 'barcode':
			return { type, height_mm: 12, digits: true, align: 'center', fill: false, stretch: false };
		case 'logo':
			return { type, width_mm: 40, height_mm: 15, align: 'left' };
		case 'qr':
			return {
				type,
				link: linkSubstitution('telegram'),
				size_mm: 20,
				caption: d.qrCaptions.telegram,
				align: 'center'
			};
		case 'signatures':
			return { type, left: d.signatureLeft, right: d.signatureRight };
		case 'separator':
			return { type };
		case 'spacer':
			return { type, mm: 4 };
		case 'columns':
			return { type, columns: [[], []], gap_mm: 6 };
		case 'copy_label':
			return { type, align: 'right' };
		case 'typst_code':
			return { type, code: d.code };
	}
}

/** A new block of the type with its starting values, printed in both copies. */
export function newBlock(type: (typeof BLOCK_TYPES)[number]): Block {
	// Calibration marks are the calibration label's own: the types are checked at run time too.
	if (!(BLOCK_TYPES as readonly string[]).includes(type)) {
		throw new Error(`A block of the type ${type} is not made here`);
	}
	return { copies: 'both', body: body(type) };
}
